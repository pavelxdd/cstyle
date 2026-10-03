use crate::formatter::constructs::headers;
use crate::formatter::constructs::headers::is_header;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, token_text};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::blocks::is_code_token;
use crate::formatter::syntax::SyntaxRole;
use crate::formatter::text::columns;
use crate::formatter::text::line_scan::has_unclosed_delimiter_after;
use crate::source::lex::leading_identifier;

impl FormatEngine<'_> {
    pub(crate) fn try_push_raw_standalone_macro_line(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        line_end: usize,
    ) -> bool {
        let line_tokens = &tokens[line_start..line_end];
        let line = line_tokens
            .iter()
            .filter(|token| !matches!(token, Token::Newline))
            .map(token_text)
            .collect::<String>();
        let line = collapse_empty_comma_arguments(&line);
        let trimmed = line.trim();
        if is_header(self.options, leading_identifier(trimmed)) {
            return false;
        }
        if self.options.pad_commas {
            return false;
        }
        let has_role = line_tokens.iter().enumerate().any(|(offset, token)| {
            matches!(token, Token::Word(_))
                && self.syntax_roles.role_at(line_start + offset)
                    == SyntaxRole::StandaloneMacroInvocation
        });
        let role_starts_line = line_tokens
            .iter()
            .enumerate()
            .find_map(|(offset, token)| {
                (!matches!(token, Token::Whitespace(_) | Token::Newline)).then_some(offset)
            })
            .is_some_and(|offset| {
                matches!(line_tokens.get(offset), Some(Token::Word(_)))
                    && self.syntax_roles.role_at(line_start + offset)
                        == SyntaxRole::StandaloneMacroInvocation
            });
        let standalone_line = is_standalone_macro_invocation_line(trimmed);
        if !(role_starts_line || standalone_line) {
            return false;
        }
        if !trimmed.ends_with(')') {
            return false;
        }
        if trimmed.contains(">{") {
            return false;
        }
        if has_role
            && (has_unclosed_delimiter_after(trimmed, "(", ")")
                || self
                    .layout
                    .continuation_indent
                    .next_line_indent_spaces
                    .is_some())
        {
            return false;
        }
        if self.layout.nesting.paren_depth > 0 || self.layout.indentation.statement_depth() > 0 {
            return false;
        }
        self.finish_line();
        if self.directly_in_switch_body() {
            self.update_case_body_indent(LineKind::Normal, trimmed);
        }
        // The line comes between a case label and a brace after it.
        self.update_case_brace_unindent(LineKind::Normal, trimmed);
        let indent =
            self.layout.indentation.indent() + self.case_body_indent_extra(LineKind::Normal);
        let exact_indent_spaces =
            self.layout
                .previous_pre_adjust_line
                .as_ref()
                .and_then(|previous| {
                    headers::line_is_control_body_header(previous.trim_start()).then(|| {
                        columns::leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width
                    })
                });
        // The line holds its tokens for the anchors of the code after it.
        if tokens.len() == self.tree.tokens.len() {
            let mut code = (line_start..line_end).filter(|&index| is_code_token(&tokens[index]));
            if let Some(first) = code.next() {
                let last = code.next_back().unwrap_or(first);
                self.output
                    .set_pending_tokens(Some(TokenSpan { first, last }));
            }
        }
        if self.take_block_spacing_blank(trimmed) {
            self.push_empty_line();
        }
        if let Some(spaces) = exact_indent_spaces {
            self.push_output_line_spaces(trimmed, self.layout.indentation.indent(), spaces);
        } else {
            self.push_output_line(trimmed, indent);
        }
        self.previous_was_newline = true;
        if trimmed.starts_with("Q_FOREACH(") {
            self.layout
                .continuation_indent
                .set_next_line_level(indent + 1);
            self.layout.pending_braceless_block_bias = Some(indent + 1);
        }
        true
    }
}

pub(crate) fn is_standalone_macro_invocation_line(line: &str) -> bool {
    let Some(open) = line.find('(') else {
        return false;
    };
    if !line.ends_with(')') || line.contains(';') || line.contains('{') || line.contains('}') {
        return false;
    }
    let mut depth = 0i32;
    for ch in line.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return false;
    }
    let name = line[..open].trim();
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn collapse_empty_comma_arguments(line: &str) -> String {
    let mut result = String::with_capacity(line.len());
    let chars = line.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    while index < chars.len() {
        let ch = chars[index];
        if let Some(quote_char) = quote {
            result.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            result.push(ch);
            index += 1;
            continue;
        }
        if ch == ',' {
            result.push(ch);
            let mut next = index + 1;
            while matches!(chars.get(next), Some(' ' | '\t')) {
                next += 1;
            }
            index = if matches!(chars.get(next), Some(',')) {
                next
            } else {
                index + 1
            };
            continue;
        }
        result.push(ch);
        index += 1;
    }
    result
}
