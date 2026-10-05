use crate::formatter::constructs::headers::is_header;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::functions::{FunctionHead, template_arguments_start};
use crate::formatter::syntax::function_name_start;
use crate::formatter::syntax::language::{self, is_non_type_keyword, is_type_like_pointer_word};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{
    find_outside_quotes, line_ends_with_comment, line_paren_imbalance,
    reverse_scan_skips_block_comment, unmatched_open_paren_column,
};
use crate::source::lex::is_identifier_continue;

impl FormatEngine<'_> {
    pub(crate) fn split_return_type_pointer_name_indent_spaces(&self, line: &str) -> Option<usize> {
        if !is_pointer_prefixed_function_part(line.trim_start()) {
            return None;
        }
        let previous = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        is_return_type_line(previous.trim())
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn split_trailing_return_arrow_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with("->") || current.starts_with("->*") {
            return None;
        }

        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(16)
        {
            let code = self.output.code_of(previous).trim_end();
            if reverse_scan_skips_block_comment(code, &mut in_block_comment) {
                continue;
            }
            if close_pending == 0 && !code.ends_with(')') {
                return None;
            }
            let (closes, mut opens) = line_paren_imbalance(code);
            if close_pending > 0
                && let Some(&column) = opens.last()
                && code[column..].starts_with('(')
            {
                let before = code[..column].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                if is_parameter_return_type_prefix(return_type)
                    && !name.is_empty()
                    && !is_header(self.options, name)
                {
                    return Some(leading_visual_width(previous, self.options.tab_width));
                }
            }
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if close_pending == 0
                && (code.ends_with(';') || code.ends_with('{') || code.ends_with('}'))
            {
                return None;
            }
        }
        None
    }

    fn recent_base_trailing_return_function_header_index(&self) -> Option<usize> {
        if self.layout.indentation.indent() == 0 {
            return None;
        }
        let mut closed_blocks = 0usize;
        for (index, line) in self.output.iter().enumerate().rev().take(24) {
            let code = self.output.code_of(line).trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('}') {
                closed_blocks += 1;
            }
            if code.ends_with('{') {
                if closed_blocks > 0 {
                    closed_blocks -= 1;
                    continue;
                }
                if code.contains(") ->") && !trimmed.starts_with("template ") {
                    return Some(index);
                }
                return None;
            }
        }
        None
    }

    pub(crate) fn recent_base_trailing_return_function_header(&self) -> bool {
        self.recent_base_trailing_return_function_header_index()
            .is_some()
    }

    pub(crate) fn recent_trailing_return_function_after_multiline_template_declaration(
        &self,
    ) -> bool {
        let Some(brace_index) = self.recent_base_trailing_return_function_header_index() else {
            return false;
        };
        let mut signature_start = brace_index;
        let mut index = brace_index;
        while index > 0 {
            index -= 1;
            let line = &self.output[index];
            let code = self.output.code_of(line).trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("template ")
                || code.ends_with(';')
                || code.ends_with('{')
                || code.ends_with('}')
            {
                break;
            }
            signature_start = index;
            if code.contains('(') {
                break;
            }
        }
        self.output_closes_multiline_template_declaration_before(signature_start)
    }

    pub(crate) fn trailing_return_function_parameter_tail_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.contains("= {}") || !current.contains(") ->") || !current.ends_with('{') {
            return None;
        }
        for (previous_index, previous) in self
            .output
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, line)| !line.trim().is_empty())
            .take(8)
        {
            let previous_code = self.output.code_of(previous).trim_end();
            if let Some(open) = unmatched_open_paren_column(previous_code) {
                let before = previous_code[..open].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                let prefixed_return_type = return_type
                    .split_whitespace()
                    .any(language::is_macro_like_word)
                    && is_parameter_return_type_prefix(return_type);
                if leading_visual_width(previous, self.options.tab_width) == 0
                    && (prefixed_return_type
                        || self.output_closes_multiline_template_declaration_before(previous_index))
                    && !name.is_empty()
                    && !is_header(self.options, name)
                {
                    return Some(0);
                }
                return None;
            }
            if previous_code.ends_with([';', '{', '}']) {
                return None;
            }
        }
        None
    }

    pub(crate) fn function_signature_parameter_continuation_indent_spaces(
        &self,
        current_line: &str,
        signature_line: &str,
        open_paren: usize,
    ) -> Option<usize> {
        let before = signature_line[..open_paren].trim_end();
        let name_start = function_name_start(before)?;
        let return_type = before[..name_start].trim_end();
        let name = before[name_start..].trim_start();
        if !is_parameter_return_type_prefix(return_type)
            || name.is_empty()
            || is_header(self.options, name)
        {
            return None;
        }
        let after_paren = &signature_line[open_paren + 1..];
        let after_paren_indent = after_paren.len() - after_paren.trim_start().len();
        let visual_open =
            visual_width_from(&signature_line[..open_paren], 0, self.options.tab_width);
        let visual_after = visual_width_from(
            &after_paren[..after_paren_indent],
            visual_open + 1,
            self.options.tab_width,
        );
        let spaces = if current_line.starts_with(')') {
            visual_open
        } else {
            visual_open + 1 + visual_after
        };
        (spaces <= self.options.max_continuation_indent).then_some(spaces)
    }

    /// Whether a return type option applies to `head`: `definition_option`
    /// for definitions, `declaration_option` for declarations.
    fn return_type_option_applies(
        head: &FunctionHead,
        definition_option: bool,
        declaration_option: bool,
    ) -> bool {
        if head.body.is_some() {
            definition_option
        } else {
            declaration_option
        }
    }

    /// Whether the return type of `head` can move to or from its own line:
    /// the name must sit at the same nesting as the return type, which rules
    /// out `void (*f(int))(void)` and `int (name)(...)`.
    fn has_movable_return_type(&self, head: &FunctionHead) -> bool {
        head.has_return_type()
            && self.tree.groups.enclosing(head.name) == self.tree.groups.enclosing(head.start)
            && !self.inside_extern_block(head.start)
    }

    /// Whether `index` is in the block of an `extern "C"`, where astyle
    /// leaves return types alone.
    fn inside_extern_block(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        self.tree.groups.enclosing(index).is_some_and(|group| {
            let open = self.tree.groups.get(group).open;
            matches!(tokens[open], Token::Symbol('{'))
                && self
                    .tree
                    .previous_code_token(open)
                    .filter(|&literal| matches!(tokens[literal], Token::StringLiteral(_)))
                    .and_then(|literal| self.tree.previous_code_token(literal))
                    .is_some_and(
                        |keyword| matches!(&tokens[keyword], Token::Word(word) if word == "extern"),
                    )
        })
    }

    /// Whether `head` starts a statement the way AStyle sees it: after `;`,
    /// a brace, a label colon, `template<...>`, or at the start of the file.
    /// After a macro invocation without a semicolon, such as
    /// `static GIT_PATH_FUNC(a, "b")`, AStyle reads the head as a
    /// continuation and neither breaks nor attaches its return type.
    fn head_starts_statement(&self, head: &FunctionHead) -> bool {
        let tokens = &self.tree.tokens;
        self.tree
            .previous_code_token(head.start)
            .is_none_or(|previous| match &tokens[previous] {
                Token::Symbol(';' | '{' | '}' | ':') => true,
                // A macro left without a semicolon, with no directive
                // between.
                Token::Word(_) => !tokens[previous..head.start]
                    .iter()
                    .any(|token| matches!(token, Token::Preprocessor(_))),
                // `template<class T>`
                Token::Operator(operator) if operator == ">" => {
                    template_arguments_start(tokens, previous)
                        .and_then(|open| self.tree.previous_code_token(open))
                        .is_some_and(
                            |word| matches!(&tokens[word], Token::Word(word) if word == "template"),
                        )
                }
                _ => false,
            })
    }

    /// Whether a preprocessor directive splits the parameter list, which
    /// keeps astyle from moving the return type.
    fn parameters_hold_directive(&self, head: &FunctionHead) -> bool {
        let params = self.tree.groups.get(head.params);
        params.close.is_some_and(|close| {
            self.tree.tokens[params.open..close]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        })
    }

    /// Whether the function name line starting at token `first` joins the
    /// return type on the last output line.
    pub(crate) fn attaches_return_type(&self, first: usize) -> bool {
        let Some(head) = self.tree.functions.named_at(first) else {
            return false;
        };
        if !Self::return_type_option_applies(
            head,
            self.options.attach_return_type,
            self.options.attach_return_type_decl,
        ) || !self.has_movable_return_type(head)
            || !self.head_starts_statement(head)
            || self.parameters_hold_directive(head)
        {
            return false;
        }
        let Some(previous_index) = self.output.len().checked_sub(1) else {
            return false;
        };
        let previous_is_return_type =
            self.output
                .line_tokens(previous_index)
                .is_some_and(|previous| {
                    // astyle sees only the line before the name, wherever
                    // the head starts.
                    // A pointer run such as `**` may record only its first
                    // star as the line's last token.
                    previous.first >= head.start
                        && self.tree.previous_code_token(head.name_start).is_some_and(
                            |before_name| {
                                before_name >= previous.last
                                    && !self.tree.tokens[previous.last..=before_name]
                                        .iter()
                                        .any(|token| matches!(token, Token::Newline))
                            },
                        )
                });
        let previous = &self.output[previous_index];
        // AStyle attaches only return types it recognizes as types, and never
        // a split `struct Type *`.
        let previous_trimmed = previous.trim();
        previous_is_return_type
            && !line_ends_with_comment(previous)
            && is_attachable_return_type_line(previous_trimmed)
            && !(previous_trimmed.starts_with("struct ") && previous_trimmed.ends_with('*'))
    }

    /// Joins a function name line to the return type on the previous output
    /// line (`--attach-return-type`, `--attach-return-type-decl`).
    pub(crate) fn try_publish_attached_return_type(&mut self, line: &str) -> bool {
        let Some(span) = self.output.pending_tokens() else {
            return false;
        };
        if !self.attaches_return_type(span.first) {
            return false;
        }
        let Some(head) = self.tree.functions.named_at(span.first).cloned() else {
            return false;
        };
        let previous = self.output.pop().expect("previous line exists");
        let previous_trimmed = previous.trim();
        let previous_prefix = &previous[..previous.len() - previous.trim_start().len()];
        let separator = if previous_trimmed.ends_with(['*', '&', '^']) {
            ""
        } else {
            " "
        };
        self.output.set_pending_tokens(Some(TokenSpan {
            first: head.start,
            last: span.last,
        }));
        let previous_indent = leading_visual_width(&previous, self.options.tab_width);
        let start_column = visual_width_from(
            &format!("{previous_prefix}{previous_trimmed}{separator}"),
            0,
            self.options.tab_width,
        );
        self.layout.frame_stack.move_joined_line_frames(
            self.output.len() + 1,
            self.output.len(),
            start_column,
            previous_indent,
        );
        // The joined head may outgrow the maximum code length.
        let joined = format!("{previous_trimmed}{separator}{}", line.trim_start());
        if self.options.max_code_length.is_some() && previous_prefix.chars().all(|ch| ch == ' ') {
            let level = previous_indent / self.options.indent_width.max(1);
            self.push_formatted_line_with_indent(
                &joined,
                level,
                ContinuationIndent::Spaces(previous_indent),
                ContinuationIndent::Spaces(
                    previous_indent + self.options.continuation_indent * self.options.indent_width,
                ),
            );
        } else {
            self.adjust_and_publish_line(format!("{previous_prefix}{joined}"));
        }
        true
    }

    /// Splits the return type of a function head onto its own line
    /// (`--break-return-type`, `--break-return-type-decl`).
    pub(crate) fn try_publish_split_return_type(
        &mut self,
        line: &str,
        indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> bool {
        let Some(span) = self.output.pending_tokens() else {
            return false;
        };
        // A head may start on an earlier line, as after a macro alone on
        // its line; this line then holds the rest of its return type.
        let Some(head) = self
            .tree
            .functions
            .starting_at(span.first)
            .or_else(|| {
                (span.first..=span.last)
                    .find_map(|token| self.tree.functions.named_at(token))
                    .filter(|head| {
                        head.start < span.first
                            && head.name_start > span.first
                            // astyle reads a head continuing a macro call,
                            // as `__attribute__((x))`, as one line.
                            && self.tree.previous_code_token(span.first).is_some_and(
                                |previous| matches!(self.tree.tokens[previous], Token::Word(_)),
                            )
                    })
            })
            .cloned()
        else {
            return false;
        };
        if !Self::return_type_option_applies(
            &head,
            self.options.break_return_type && !self.options.attach_return_type,
            self.options.break_return_type_decl && !self.options.attach_return_type_decl,
        ) || !self.has_movable_return_type(&head)
            || !span.contains(head.name_start)
            || !self.head_starts_statement(&head)
            // astyle reads a head led by `struct` or `union` as a type
            // definition and leaves it whole.
            || matches!(&self.tree.tokens[head.start], Token::Word(word)
                if matches!(word.as_str(), "struct" | "union"))
        {
            return false;
        }
        let params_open = self.tree.groups.get(head.params).open;
        let (Some(name_offset), Some(_)) = (
            self.tree
                .token_offset_in_line(line, span.first, head.name_start),
            self.tree
                .token_offset_in_line(line, span.first, params_open),
        ) else {
            return false;
        };
        let return_type = line[..name_offset].trim_end().to_string();
        let function_part = line[name_offset..].to_string();
        let return_type_last = self
            .tree
            .previous_code_token(head.name_start)
            .unwrap_or(span.first);
        let return_type_span = TokenSpan {
            first: span.first,
            last: return_type_last,
        };
        let function_span = TokenSpan {
            first: head.name_start,
            last: span.last,
        };
        self.output.set_pending_tokens(Some(return_type_span));
        let return_type_line = self.output.len();
        if let Some(spaces) = exact_indent_spaces {
            self.push_formatted_line_exact(&return_type, indent, spaces);
            self.output.set_pending_tokens(Some(function_span));
            self.push_formatted_line_exact(&function_part, indent, spaces);
        } else {
            self.push_formatted_line(&return_type, indent);
            self.output.set_pending_tokens(Some(function_span));
            self.push_formatted_line(&function_part, indent);
        }
        // The return type line opens the body of a directive's branch as
        // a line laid out whole would.
        if return_type_line < self.output.len() {
            let spaces = self
                .output
                .lead_width(return_type_line, self.options.tab_width);
            self.record_preprocessor_branch_body_indent(&return_type, spaces);
        }
        true
    }
}

/// A line astyle attaches a function name to: words and pointer marks
/// only, any word naming a type.
fn is_attachable_return_type_line(line: &str) -> bool {
    is_return_type_line(line)
        || !line.is_empty()
            && line.chars().all(|ch| {
                is_identifier_continue(ch) || ch.is_whitespace() || matches!(ch, '*' | '&' | ':')
            })
            && line
                .split(|ch: char| !is_identifier_continue(ch))
                .filter(|part| !part.is_empty())
                .all(|part| !language::is_non_type_keyword(part))
}

pub(crate) fn is_return_type_line(line: &str) -> bool {
    if line.is_empty()
        || line.contains("//")
        || line.contains("/*")
        || line.contains("*/")
        || line.starts_with('#')
    {
        return false;
    }
    let mut angle_depth: i32 = 0;
    for ch in line.chars() {
        match ch {
            '{' | '}' | ';' => return false,
            '<' => angle_depth += 1,
            '>' => angle_depth = (angle_depth - 1).max(0),
            '(' | ')' | '=' | ',' if angle_depth == 0 => return false,
            _ => {}
        }
    }
    if angle_depth != 0 {
        return false;
    }
    line.split(|ch: char| !is_identifier_continue(ch))
        .any(|part| !part.is_empty() && is_type_like_pointer_word(part))
}

pub(crate) fn is_parameter_return_type_prefix(line: &str) -> bool {
    let first_word = line
        .split(|ch: char| !is_identifier_continue(ch))
        .find(|word| !word.is_empty());
    if first_word.is_some_and(|word| {
        is_non_type_keyword(word)
            || matches!(
                word,
                "co_return" | "alignof" | "noexcept" | "typeid" | "requires" | "decltype"
            )
    }) {
        return false;
    }
    is_return_type_line(line)
        || (!line.trim().is_empty()
            && line.chars().all(|ch| {
                ch.is_whitespace()
                    || is_identifier_continue(ch)
                    || matches!(ch, ':' | '<' | '>' | '*' | '&')
            }))
}

fn is_pointer_prefixed_function_part(line: &str) -> bool {
    let rest =
        line.trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '*' | '&' | '^'));
    if rest == line {
        return false;
    }
    let Some(open_paren) = find_outside_quotes(rest, "(") else {
        return false;
    };
    let before = rest[..open_paren].trim_end();
    !before.is_empty()
        && !language::is_header(before)
        && function_name_start(before).is_some_and(|start| start == 0)
}
