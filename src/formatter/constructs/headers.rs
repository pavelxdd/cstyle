use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::braces::rewrite::is_standard_add_braces_header;
use crate::formatter::constructs::assembly::is_asm_block_header;
use crate::formatter::constructs::switch_cases::{
    is_case_label_start, is_default_label_start, label_line_holds_braceless_header,
};
use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, tokenize};
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::frame::{BraceFrame, BraceSemanticKind, HeaderFrame};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::syntax::language;
use crate::formatter::text::columns::{leading_visual_width, visual_column_at};
use crate::formatter::text::line_scan::{
    ContainsAnyByte, is_comment_line, is_comment_only_line, line_brace_imbalance,
    preprocessor_directive, unmatched_open_paren_column, unmatched_open_paren_columns,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::literals::starts_string_literal_token;
use crate::formatter::tokens::operators::head_ends_binary_operator;
use crate::source::lex::{
    is_identifier_continue, is_identifier_start, is_word_char, leading_identifier, trailing_word,
};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct HeaderParenState {
    pub(crate) depth: Option<usize>,
    pub(crate) just_closed: bool,
    pub(crate) post_paren: bool,
}

pub(crate) fn starts_header_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

pub(crate) fn is_conditional_header_line(line: &str) -> bool {
    let mut trimmed = line.trimmed_start();
    if let Some(rest) = trimmed.strip_prefix("else")
        && rest.starts_with(char::is_whitespace)
    {
        trimmed = rest.trimmed_start();
    }
    let word: String = trimmed
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect();
    matches!(word.as_str(), "if" | "for" | "while" | "switch")
}

pub(crate) fn is_braceless_header_line(line: &str) -> bool {
    starts_header_word(line, "if")
        || line.starts_with("else if")
        || starts_header_word(line, "for")
        || starts_header_word(line, "while")
        || starts_header_word(line, "switch")
}

pub(crate) fn line_is_control_body_header(line: &str) -> bool {
    let trimmed = line.trimmed_end();
    if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
        return false;
    }
    ["if", "else if", "else", "for", "while", "switch"]
        .iter()
        .any(|keyword| {
            trimmed == *keyword
                || trimmed
                    .strip_prefix(keyword)
                    .is_some_and(|rest| rest.starts_with(' ') || rest.starts_with('('))
        })
}

pub(crate) fn same_line_nested_header_extra(line: &str) -> usize {
    let code = line.trimmed_end().trim_end_matches('{').trimmed_end();
    let else_header = code.starts_with("else while")
        || code.starts_with("else for")
        || code.starts_with("else do")
        || code.starts_with("else switch");
    // Only a second header counts; candidates bound the headers from above.
    if nested_header_word_candidates(code) + usize::from(else_header) <= 1 {
        return 0;
    }
    // Words in literals and comments are no headers.
    let count = tokenize(code)
        .iter()
        .filter(|token| {
            matches!(token, Token::Word(word)
                if matches!(word.as_str(), "if" | "for" | "while" | "switch" | "do"))
        })
        .count();
    (count + usize::from(else_header)).saturating_sub(1)
}

/// How many times a header word that [`same_line_nested_header_extra`]
/// counts may stand in `code`: each spot apart from the letters around it, a
/// cheap bound before tokenizing.
fn nested_header_word_candidates(code: &str) -> usize {
    let bytes = code.as_bytes();
    let mut count = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        if !matches!(byte, b'i' | b'f' | b'w' | b's' | b'd')
            || index
                .checked_sub(1)
                .is_some_and(|at| bytes[at].is_ascii_alphabetic() || bytes[at] == b'_')
        {
            continue;
        }
        let rest = &bytes[index..];
        for word in ["if", "for", "while", "switch", "do"] {
            if rest.starts_with(word.as_bytes())
                && !rest
                    .get(word.len())
                    .is_some_and(|&after| after.is_ascii_alphanumeric() || after == b'_')
            {
                count += 1;
            }
        }
    }
    count
}

fn is_split_loop_header(line: &str) -> bool {
    (starts_header_word(line, "while") || starts_header_word(line, "for"))
        && unmatched_open_paren_column(line).is_some()
}

pub(crate) struct ElseBodyLayout {
    pub(crate) indent_level: Option<usize>,
    pub(crate) indent_spaces: usize,
}

impl FormatEngine<'_> {
    pub(crate) fn ready_non_paren_header_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !["for ", "while ", "switch "]
            .iter()
            .any(|header| line_start.starts_with(header))
            || line_start.contains('(')
        {
            return None;
        }
        let previous_index = (0..self.output.len())
            .rev()
            .find(|&index| !self.output.trimmed(index).is_empty())?;
        let previous_spaces = self
            .output
            .lead_width(previous_index, self.options.tab_width);
        (previous_spaces > leading_visual_width(line, self.options.tab_width))
            .then_some(previous_spaces)
    }

    pub(crate) fn maximum_length_conditional_continuation_floor(
        &self,
        line: &str,
        base_indent_width: usize,
    ) -> Option<usize> {
        (is_conditional_header_line(line) && !self.options.indent_after_parens)
            .then(|| base_indent_width + min_conditional_indent_spaces(self.options))
    }

    pub(crate) fn active_split_else_header_continuation_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"#{}")
            || !self.preprocessor_split_else_active()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !head_ends_binary_operator(previous_code)
            || !line_is_control_body_header(previous_code.trimmed_start())
        {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + min_conditional_indent_spaces(self.options),
        )
    }

    pub(crate) fn active_split_else_multiline_header_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !self.preprocessor_split_else_active()
            || line.trimmed_start().starts_with_any(b"{}#")
        {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if previous_code.trimmed_start().starts_with(')')
            && let Some(header_indent) = self.current_closing_multiline_header_indent()
        {
            let nested_header_group = self.split_else_body_indent_active()
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip(1)
                    .take(16)
                    .any(|line| {
                        let code = self.output.code_trimmed_of(line);
                        let trimmed = code.trimmed_start();
                        let header = trimmed
                            .strip_prefix("}else")
                            .or_else(|| trimmed.strip_prefix("} else"))
                            .map(str::trim_start)
                            .unwrap_or(trimmed);
                        let starts_nested_group = header
                            .strip_prefix("else if")
                            .or_else(|| header.strip_prefix("if"))
                            .or_else(|| header.strip_prefix("while"))
                            .or_else(|| header.strip_prefix("for"))
                            .or_else(|| header.strip_prefix("switch"))
                            .is_some_and(|tail| tail.trimmed_start().starts_with("( ("));
                        let guarded_header = self
                            .output
                            .scoped()
                            .iter()
                            .rev()
                            .skip_while(|candidate| candidate.as_str() != line.as_str())
                            .skip(1)
                            .find(|line| !line.trimmed().is_empty())
                            .is_some_and(|line| {
                                preprocessor_directive(line.trimmed_start()).is_some()
                            });
                        starts_nested_group
                            && !guarded_header
                            && self.paren_imbalance_of(code).1.len() > 1
                    });
            return Some(if nested_header_group {
                header_indent
            } else {
                header_indent + self.options.indent_width
            });
        }
        None
    }

    pub(crate) fn opening_conditional_directive_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"#{}") {
            return None;
        }
        let previous_index = self
            .output
            .iter()
            .rposition(|line| !line.trimmed().is_empty())?;
        if !preprocessor_directive(self.output[previous_index].trimmed_start())
            .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        {
            return None;
        }
        let before_index = self.output[..previous_index]
            .iter()
            .rposition(|line| !line.trimmed().is_empty())?;
        let before = &self.output[before_index];
        let before_trimmed = self.output.code_body(before_index);
        if before_trimmed == "else"
            || before_trimmed.ends_with("} else")
            || before_trimmed.ends_with("}else")
        {
            return Some(
                leading_visual_width(before, self.options.tab_width) + self.options.indent_width,
            );
        }
        // A code line led by `*` holds tokens; a comment line holds none.
        // An indented preprocessor block indents its body past the comment.
        let block_indent = if self.preprocessor.indented_block_stack.last() == Some(&true) {
            self.options.indent_width
        } else {
            0
        };
        (is_comment_line(before.trimmed_start()) && self.output.line_tokens(before_index).is_none())
            .then(|| {
                self.output
                    .comment_indent_width(before_index, self.options.tab_width)
                    + block_indent
            })
    }

    pub(crate) fn active_split_else_open_header_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"#{}")
            || !self.preprocessor_split_else_active()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with('{') {
            return None;
        }
        let open_index = (0..self.output.len()).rev().find(|&index| {
            !self.output[index].trimmed().is_empty()
                && self.output.code_trimmed(index).ends_with('{')
        })?;
        let mut header_index = open_index;
        for index in (0..=open_index).rev() {
            let trimmed = self.output.code_trimmed(index);
            if starts_header_word(trimmed, "if")
                || starts_header_word(trimmed, "for")
                || starts_header_word(trimmed, "while")
                || starts_header_word(trimmed, "switch")
                || trimmed.starts_with("else if")
                || trimmed.starts_with("} else")
                || trimmed.starts_with("}else")
            {
                header_index = index;
                break;
            }
            if index < open_index
                && (trimmed.ends_with(';') || trimmed == "}" || trimmed.starts_with('#'))
            {
                return None;
            }
        }
        let mut depth = 0usize;
        for index in header_index..=open_index {
            let (closes, opens) = self.paren_imbalance_of(self.output.code(index));
            depth = depth.saturating_sub(closes);
            depth += opens.len();
        }
        (depth > 0).then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn pending_split_else_braceless_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        natural_indent_spaces: usize,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"#{}/")
            || is_header(self.options, leading_identifier(line))
            || self.layout.pending_braceless_block_bias.is_none()
            || !(self.split_else_braceless_body_active()
                || self
                    .output
                    .last_line_outside_comment()
                    .is_some_and(|previous| is_comment_only_line(previous.trimmed_start())))
        {
            return None;
        }
        let header = self
            .layout
            .frame_stack
            .active_header()
            .filter(|header| is_add_braces_header(self.options, &header.header))?;
        if self.split_else_braceless_body_active() {
            return Some(header.body_indent_spaces);
        }
        (current_indent_spaces.unwrap_or(natural_indent_spaces) < header.body_indent_spaces)
            .then_some(header.body_indent_spaces)
    }

    pub(crate) fn split_condition_closing_paren_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with(')') {
            return None;
        }
        let mut close_line_pending = line.paren_imbalance().0;
        let mut intervening_closes = 0usize;
        let mut candidate = None;
        for index in self.output.scoped_range().rev().take(64) {
            let previous = &self.output.as_slice()[index];
            if self.output.trimmed(index).is_empty() {
                continue;
            }
            let code = self.output.code_before_comment(index).trimmed_end();
            let trimmed = &code[self.output.lead_bytes(index).min(code.len())..];
            if trimmed.starts_with('#') {
                continue;
            }
            if code.ends_with_any(b";{}") {
                return None;
            }
            let (closes, mut opens) = self.output_code_paren_imbalance(index);
            if opens.is_empty() {
                intervening_closes += closes;
                continue;
            }
            let opens_total = opens.len();
            let code_chars = || code.chars().collect::<Vec<_>>();
            let first_open = opens[0];
            let open_column =
                || visual_column_at(&code_chars(), first_open, self.options.tab_width);
            let is_header = starts_header_word(trimmed, "if")
                || starts_header_word(trimmed, "while")
                || starts_header_word(trimmed, "for")
                || starts_header_word(trimmed, "do")
                || trimmed.starts_with("else if")
                || trimmed.starts_with("} else");
            let matched_by_intervening = intervening_closes.min(opens_total);
            for _ in 0..matched_by_intervening {
                opens.pop();
            }
            intervening_closes -= matched_by_intervening;
            let matched_by_close_line = close_line_pending.min(opens.len());
            if matched_by_close_line > 0 {
                if !is_header {
                    candidate = Some(leading_visual_width(previous, self.options.tab_width));
                }
                for _ in 0..matched_by_close_line {
                    opens.pop();
                }
                close_line_pending -= matched_by_close_line;
            }
            if !is_header {
                intervening_closes += closes;
                continue;
            }
            if self.split_else_body_indent_active()
                && !code.ends_with('(')
                && !(trimmed.starts_with("else if")
                    || trimmed.starts_with("} else if")
                    || trimmed.starts_with("}else if"))
            {
                let header_indent = leading_visual_width(previous, self.options.tab_width);
                let opens_all = self.paren_imbalance_of(code).1;
                let starts_nested_group = trimmed
                    .strip_prefix("else if")
                    .or_else(|| trimmed.strip_prefix("if"))
                    .or_else(|| trimmed.strip_prefix("while"))
                    .or_else(|| trimmed.strip_prefix("for"))
                    .or_else(|| trimmed.strip_prefix("switch"))
                    .is_some_and(|tail| tail.trimmed_start().starts_with("( ("));
                let guarded_header = self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trimmed().is_empty())
                    .is_some_and(|line| preprocessor_directive(line.trimmed_start()).is_some());
                return Some(
                    if opens_all.len() > 1 && starts_nested_group && !guarded_header {
                        header_indent
                    } else {
                        opens_all.first().map_or(header_indent, |&column| {
                            visual_column_at(&code_chars(), column, self.options.tab_width)
                        })
                    },
                );
            }
            if let Some(spaces) = candidate {
                return Some(spaces);
            }
            if matched_by_close_line == 0 {
                return None;
            }
            if opens_total > 1 {
                return Some(open_column() + 1);
            }
            if code.ends_with('(') {
                return Some(leading_visual_width(previous, self.options.tab_width));
            }
            return Some(open_column());
        }
        None
    }

    pub(crate) fn current_closing_multiline_header_indent(&self) -> Option<usize> {
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            let trimmed = self.output.code_trimmed(index);
            if depth == 0
                && meta.opens() > 0
                && (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            {
                return None;
            }
            depth += meta.closes();
            if meta.opens() > depth {
                if trimmed.ends_with('{')
                    && !starts_header_word(trimmed, "if")
                    && !starts_header_word(trimmed, "for")
                    && !starts_header_word(trimmed, "while")
                    && !starts_header_word(trimmed, "switch")
                    && !trimmed.starts_with("else if")
                    && !trimmed.starts_with("} else")
                    && !trimmed.starts_with("}else")
                {
                    // The header stands in the lines right before the brace:
                    // a line ending a statement or block, or a bare `else`,
                    // owns it instead.
                    for line in self.output[..index].iter().rev().take(16) {
                        let code = self.output.code_trimmed_of(line);
                        let trimmed = code.trimmed_start();
                        if !unmatched_open_paren_columns(code).is_empty()
                            && (starts_header_word(trimmed, "if")
                                || starts_header_word(trimmed, "for")
                                || starts_header_word(trimmed, "while")
                                || starts_header_word(trimmed, "switch")
                                || trimmed.starts_with("else if")
                                || trimmed.starts_with("} else")
                                || trimmed.starts_with("}else"))
                        {
                            return Some(leading_visual_width(line, self.options.tab_width));
                        }
                        if code.ends_with_any(b";{}") || trimmed == "else" {
                            return None;
                        }
                    }
                    return None;
                }
                return None;
            }
            depth = depth.saturating_sub(meta.opens());
        }
        None
    }

    pub(crate) fn else_split_header_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        let mut previous = self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty());
        let first = previous.next()?;
        let second = previous.next()?;
        if line.trimmed() == ";" {
            let third = previous.next()?;
            let header = second.trimmed_start();
            if first.trimmed_end().ends_with(')')
                && third.trimmed() == "else"
                && is_split_loop_header(header)
            {
                return Some(
                    leading_visual_width(second, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            return None;
        }
        let header = first.trimmed_start();
        if second.trimmed() == "else" && is_split_loop_header(header) {
            return Some(
                leading_visual_width(first, self.options.tab_width) + self.options.indent_width * 2,
            );
        }
        None
    }

    pub(crate) fn braceless_else_output_level(&self) -> Option<usize> {
        let mut body_index = self.output.len().checked_sub(1)?;
        while body_index > 0 && self.output[body_index].trimmed().is_empty() {
            body_index -= 1;
        }
        let body = self.output[body_index].trimmed_end();
        let body_code = self.output.code_trimmed_of(body);
        if !body_code.ends_with(';') {
            return None;
        }
        let body_width = leading_visual_width(body, self.options.tab_width);
        if !body_width.is_multiple_of(self.options.indent_width) {
            return None;
        }
        let mut expected = body_width / self.options.indent_width;
        let mut scan = body_index;
        while scan > 0 {
            scan -= 1;
            let line = self.output[scan].trimmed_end();
            if line.is_empty() {
                continue;
            }
            let width = leading_visual_width(line, self.options.tab_width);
            if !width.is_multiple_of(self.options.indent_width) {
                break;
            }
            let level = width / self.options.indent_width;
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            let is_header = is_braceless_header_line(trimmed);
            if level + 1 != expected {
                if level >= expected && trimmed.ends_with(')') && !is_header {
                    continue;
                }
                break;
            }
            if starts_header_word(trimmed, "if") || trimmed.starts_with("else if") {
                return Some(level + self.layout.line_adjuster.total_case_unindent_depth());
            }
            if starts_header_word(trimmed, "for")
                || starts_header_word(trimmed, "while")
                || starts_header_word(trimmed, "switch")
            {
                expected = level;
                continue;
            }
            if !trimmed.ends_with(')') {
                break;
            }
            break;
        }
        None
    }

    pub(crate) fn multiline_else_header_continuation_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line
                .trimmed_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            || !self.output.may_have_else()
            // A line inside parentheses nested in the condition keeps theirs.
            || self.output.pending_tokens().is_some_and(|span| {
                self.tree
                    .groups
                    .enclosing(span.first)
                    .and_then(|group| self.tree.previous_code_token(self.tree.groups.get(group).open))
                    .is_some_and(|before| {
                        !matches!(&self.tree.tokens[before], Token::Word(word)
                            if matches!(word.as_str(), "if" | "while" | "for" | "switch"))
                    })
            })
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let mut spaces = if previous_code.trimmed_start().starts_with("else ") {
            self.open_paren_column_of(previous_code)
                .map(|open| open + 1)
        } else {
            None
        };
        if previous
            .trimmed_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && let Some(header) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
            && header.trimmed_start().starts_with("else ")
            && unmatched_open_paren_column(self.output.code_of(header)).is_some()
            && self.paren_closes_of(previous_code) == 0
        {
            spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        spaces
    }

    pub(crate) fn plain_else_body_layout(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<ElseBodyLayout> {
        if line_kind != LineKind::Normal || self.split_else_body_indent_active() {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if previous.trimmed() != "else" {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        if line.trimmed() == "{" {
            return Some(ElseBodyLayout {
                indent_level: Some(previous_indent / self.options.indent_width),
                indent_spaces: previous_indent,
            });
        }
        if !self.output.may_have_else() || line.trimmed_start().starts_with_any(b"{#") {
            return None;
        }
        Some(ElseBodyLayout {
            indent_level: None,
            indent_spaces: previous_indent
                + self.options.indent_width
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        })
    }

    pub(crate) fn else_while_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"{#") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_trimmed = previous.trimmed_start();
        if !previous_trimmed.starts_with("else while")
            || head_ends_binary_operator(previous_trimmed)
            || self.open_paren_column_of(previous_trimmed).is_some()
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + self.options.indent_width * 2)
    }

    pub(crate) fn multiline_control_header_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"{}#") {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.trimmed_start().starts_with(')') || !previous_code.ends_with('{') {
            return None;
        }
        let header_indent = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .take(16)
            .find_map(|line| {
                let code = self.output.code_trimmed_of(line);
                let trimmed = code.trimmed_start();
                let is_plain_multiline_header = self.open_paren_column_of(code).is_some()
                    && (starts_header_word(trimmed, "if")
                        || starts_header_word(trimmed, "for")
                        || starts_header_word(trimmed, "while")
                        || starts_header_word(trimmed, "switch"))
                    && !trimmed.starts_with("else if")
                    && !trimmed.starts_with("} else if")
                    && !trimmed.starts_with("}else if");
                is_plain_multiline_header
                    .then(|| leading_visual_width(line, self.options.tab_width))
            })?;
        Some(header_indent + self.options.indent_width)
    }

    pub(crate) fn separated_else_header_body_indent_floor(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"{#")
            || !(self.output.may_have_else()
                || self.output.may_have_hash()
                || self.output.may_have_comment())
        {
            return None;
        }
        let header = self.output.last_line_outside_comment()?;
        let header_trimmed = header.trimmed_start();
        if !header_trimmed.ends_with('{')
            || !(starts_header_word(header_trimmed, "if")
                || starts_header_word(header_trimmed, "while")
                || starts_header_word(header_trimmed, "for")
                || header_trimmed.starts_with("else if"))
        {
            return None;
        }
        let separator = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != header.as_str())
            .skip(1)
            .find(|line| !line.trimmed().is_empty())?;
        let separated = if is_comment_line(separator.trimmed_start()) {
            true
        } else {
            let separator_code = self.output.code_trimmed_of(separator);
            preprocessor_directive(separator_code.trimmed_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip_while(|candidate| candidate.as_str() != separator.as_str())
                    .skip(1)
                    .find(|candidate| !candidate.trimmed().is_empty())
                    .is_some_and(|candidate| {
                        let trimmed = self.output.code_trimmed_of(candidate).trimmed_start();
                        trimmed == "else" || trimmed.ends_with("} else")
                    })
        };
        separated.then(|| {
            leading_visual_width(header, self.options.tab_width) + self.options.indent_width
        })
    }

    pub(crate) fn block_comment_separated_header_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"{#")
            || !self.output.may_have_comment()
        {
            return None;
        }
        let comment = self.output.last_line_outside_comment()?;
        if !comment.trimmed_start().starts_with("/*") {
            return None;
        }
        let scoped = self.output.scoped();
        let comment_index = scoped
            .iter()
            .rposition(|line| line.as_str() == comment.as_str())?;
        let header_index = scoped[..comment_index]
            .iter()
            .rposition(|line| !line.trimmed().is_empty())?;
        // A directive's continued lines hold no header.
        if self
            .output
            .is_directive_line(self.output.len() - scoped.len() + header_index)
        {
            return None;
        }
        let header = &scoped[header_index];
        let header_code = self.output.code_trimmed_of(header);
        let header_trimmed = header_code.trimmed_start();
        if header_code.ends_with('{')
            && (starts_header_word(header_trimmed, "if")
                || starts_header_word(header_trimmed, "for")
                || starts_header_word(header_trimmed, "while")
                || starts_header_word(header_trimmed, "do")
                || header_trimmed.starts_with("else"))
        {
            return Some(
                leading_visual_width(comment, self.options.tab_width)
                    + self.layout.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if !header_trimmed.starts_with("} ") || header_trimmed.ends_with(';') {
            return None;
        }
        Some(if header_trimmed.ends_with('{') {
            leading_visual_width(comment, self.options.tab_width)
        } else {
            self.options.indent_width
        })
    }

    pub(crate) fn else_body_after_comments_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"{#")
            || !self.output.may_have_comment()
        {
            return None;
        }
        let run = self.output.comment_run();
        let previous = &self.output[run.before?];
        if (previous.trimmed() == "else" || previous.trimmed().ends_with("} else"))
            && let Some(index) = run.indent_line()
        {
            let spaces = leading_visual_width(&self.output[index], self.options.tab_width);
            let else_indent = leading_visual_width(previous, self.options.tab_width);
            return Some(if spaces > else_indent {
                spaces
            } else {
                else_indent + self.options.indent_width
            });
        }
        None
    }

    pub(crate) fn control_header_line_comment_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with("//") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_trimmed = previous.trimmed_start();
        if !line_is_control_body_header(previous_trimmed)
            || head_ends_binary_operator(previous_trimmed)
            || self.open_paren_column_of(previous_trimmed).is_some()
        {
            return None;
        }
        let extra = if previous_trimmed.starts_with("else while") {
            self.options.indent_width * 2
        } else {
            self.options.indent_width
        };
        Some(leading_visual_width(previous, self.options.tab_width) + extra)
    }

    pub(crate) fn none_style_conditional_else_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || line_start.starts_with_any(b"{#")
            || self.options.brace_style != BraceStyle::None
        {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if (line_start.starts_with("} else") || line_start.starts_with("}else"))
            && preprocessor_directive(previous_code.trimmed_start()).is_some()
            && let Some(header) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(32)
                .find(|line| {
                    let code = self.output.code_trimmed_of(line);
                    let trimmed = code.trimmed_start();
                    code.ends_with('{')
                        && (starts_header_word(trimmed, "if")
                            || trimmed.starts_with("} else")
                            || trimmed.starts_with("}else"))
                })
        {
            return Some(leading_visual_width(header, self.options.tab_width));
        }
        if self
            .output
            .last()
            .is_some_and(|line| line.trimmed().is_empty())
            || !preprocessor_directive(previous_code.trimmed_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        {
            return None;
        }
        let header = self.output.scoped().iter().rev().skip(1).find(|line| {
            let trimmed = line.trimmed_start();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })?;
        let trimmed = self.output.code_trimmed_of(header).trimmed_start();
        if trimmed != "else" && !trimmed.ends_with("} else") {
            return None;
        }
        Some(leading_visual_width(header, self.options.tab_width) + self.options.indent_width)
    }

    pub(crate) fn else_structural_indent_after_braced_statement_level(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with("else")
            || self.split_else_body_indent_active()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        (previous.trimmed_end().ends_with(';') && previous.contains('}'))
            .then(|| self.layout.indentation.indent())
    }

    pub(crate) fn else_after_closed_nested_header_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with("else") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if self.output.code_of(previous).trimmed() != "}" {
            return None;
        }
        for header in self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .filter(|line| !line.trimmed().is_empty())
        {
            let header_code = self.output.code_trimmed_of(header);
            if header_code.trimmed() == "}" {
                return None;
            }
            if header_code.ends_with('{') {
                let extra = same_line_nested_header_extra(header_code.trimmed_start());
                return (extra > 0).then(|| {
                    leading_visual_width(header, self.options.tab_width)
                        + extra * self.options.indent_width
                });
            }
        }
        None
    }

    pub(crate) fn else_after_braceless_body_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with("else") {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with(';') {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        for header in self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .filter(|line| !line.trimmed().is_empty())
            .take(64)
        {
            let header_code = self.output.code_trimmed_of(header);
            let header_trimmed = header_code.trimmed_start();
            let header_indent = leading_visual_width(header, self.options.tab_width);
            if starts_header_word(header_trimmed, "if") && header_indent < previous_indent {
                return Some(header_indent);
            }
            if header_code.ends_with('{') || header_trimmed.starts_with("else") {
                return None;
            }
        }
        None
    }

    pub(crate) fn current_closes_same_line_else_block(&self) -> bool {
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            if meta.opens() > depth {
                let trimmed = self.output.code_trimmed(index);
                return trimmed.starts_with("} else") || trimmed.starts_with("}else");
            }
            depth = depth.saturating_sub(meta.opens());
            depth += meta.closes();
        }
        false
    }

    pub(crate) fn next_keeps_braceless_block(&self, next: Option<&Token>, base: usize) -> bool {
        match next {
            Some(Token::Word(word)) if word == "catch" => true,
            Some(Token::Word(word)) if word == "else" => self.braceless_header_accepts_else(base),
            Some(Token::Word(word)) if word == "while" => self.braceless_header_accepts_while(base),
            _ => false,
        }
    }

    fn braceless_header_accepts_while(&self, base: usize) -> bool {
        if self
            .layout
            .frame_stack
            .active_braceless_header()
            .is_some_and(|frame| {
                frame.header == "do"
                    && frame.header_indent_spaces == base * self.options.indent_width
            })
        {
            return true;
        }
        let target = base * self.options.indent_width;
        for line in self.output.scoped().iter().rev() {
            let trimmed = line.trimmed_start();
            if trimmed.is_empty()
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with("*/")
            {
                continue;
            }
            if leading_visual_width(line, self.options.tab_width) == target {
                return starts_header_word(trimmed, "do");
            }
        }
        false
    }

    pub(crate) fn braceless_header_accepts_else(&self, base: usize) -> bool {
        if self
            .layout
            .frame_stack
            .active_braceless_header()
            .is_some_and(|frame| {
                frame.can_match_else
                    && frame.header_indent_spaces == base * self.options.indent_width
            })
        {
            return true;
        }
        let target = base * self.options.indent_width;
        for line in self.output.scoped().iter().rev() {
            let trimmed = line.trimmed_start();
            if trimmed.is_empty()
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with("*/")
            {
                continue;
            }
            if leading_visual_width(line, self.options.tab_width) == target {
                return starts_header_word(trimmed, "if") || trimmed.starts_with("else if");
            }
        }
        false
    }

    pub(crate) fn enclosing_if_level(
        &self,
        body_index: usize,
        body_level: usize,
        default: usize,
    ) -> usize {
        let mut expected = body_level;
        let mut scan = body_index;
        let mut saw_condition_closer = false;
        while scan > 0 {
            scan -= 1;
            let line = self.output[scan].trimmed_end();
            if line.is_empty() {
                continue;
            }
            let width = leading_visual_width(line, self.options.tab_width);
            if !width.is_multiple_of(self.options.indent_width) {
                break;
            }
            let level = width / self.options.indent_width;
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            let is_header = header_word_is(trimmed, "if")
                || trimmed.starts_with("else if")
                || header_word_is(trimmed, "for")
                || header_word_is(trimmed, "while")
                || header_word_is(trimmed, "switch");
            if level + 1 != expected {
                if level >= expected
                    && !is_header
                    && (saw_condition_closer || trimmed.ends_with(')'))
                {
                    saw_condition_closer = true;
                    continue;
                }
                break;
            }
            let closes_condition = trimmed.ends_with(')') || (is_header && saw_condition_closer);
            if !closes_condition {
                break;
            }
            if header_word_is(trimmed, "if") || trimmed.starts_with("else if") {
                return level;
            }
            if header_word_is(trimmed, "for")
                || header_word_is(trimmed, "while")
                || header_word_is(trimmed, "switch")
            {
                expected = level;
                saw_condition_closer = false;
                continue;
            }
            if trimmed.ends_with(')') && !is_header {
                expected = level;
                continue;
            }
            break;
        }
        default
    }

    fn active_else_expects_body(&self) -> bool {
        self.output.scoped().iter().rev().find_map(|line| {
            let code = &self.output.code_of(line);
            let trimmed = code.trimmed();
            if trimmed.is_empty() || trimmed.starts_with('#') || is_comment_line(line) {
                return None;
            }
            Some(trimmed == "else" || trimmed.ends_with("} else"))
        }) == Some(true)
    }

    fn record_header_frame(&mut self, word: &str) {
        let attached_closing_indent =
            (word == "else" && self.current.trimmed_start().starts_with('}')).then(|| {
                let closed = self.layout.frame_stack.last_closed_brace();
                // After an else's block, an `else` belongs to the braceless
                // `if` around that chain.
                closed
                    .filter(|frame| frame.header.as_deref() == Some("else"))
                    .and_then(|_| self.layout.frame_stack.active_braceless_header())
                    .filter(|frame| frame.can_match_else)
                    .map(|frame| frame.header_indent_spaces)
                    .or_else(|| closed.map(|frame| frame.sibling_indent_column))
                    .unwrap_or_else(|| self.current_line_indent_spaces())
            });
        // Only an `else` right after the closing brace pairs with that block.
        let follows_closing_brace = self.current.trimmed_start().starts_with('}')
            || self.current_is_blank()
                && self
                    .output
                    .last_line_outside_comment()
                    .is_some_and(|line| self.output.code_trimmed_of(line).ends_with('}'));
        // A block kept on one line records no frame; the last closed one
        // counts only when its brace ended the line before.
        let closed_on_previous_line = |frame: &BraceFrame| {
            self.current.trimmed_start().starts_with('}')
                || frame.close_output_line.is_none()
                || frame.close_output_line == self.output.last_non_empty_index()
        };
        let closed_if_indent = (word == "else" && follows_closing_brace)
            .then(|| {
                self.layout
                    .frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.header.as_deref() == Some("if"))
                    .filter(|frame| closed_on_previous_line(frame))
                    .map(|frame| {
                        // The `if` of a broken else-if stands a level past
                        // the column its frame recorded; its body holds it.
                        if matches!(
                            self.options.brace_style,
                            BraceStyle::Ratliff | BraceStyle::Whitesmith
                        ) {
                            frame.header_indent_column.max(
                                frame
                                    .body_indent_column
                                    .saturating_sub(self.options.indent_width),
                            )
                        } else {
                            frame.header_indent_column
                        }
                    })
            })
            .flatten();
        let open_if_indent = (word == "else")
            .then(|| {
                self.layout
                    .frame_stack
                    .active_header()
                    .filter(|frame| frame.header == "if")
                    .map(|frame| frame.line_indent_spaces)
            })
            .flatten();
        let sequential_after_close_indent = (!is_attachable_closing_header(word)
            && word != "while"
            && self
                .current
                .trimmed()
                .strip_prefix('}')
                .is_some_and(|rest| rest.trimmed().is_empty()))
        .then(|| self.layout.indentation.indent() * self.options.indent_width);
        let starts_output_line = self.token_input.token_begins_source_line
            || (self.current_is_blank() && self.previous_was_newline);
        let keeps_return_continuation_column = self.options.brace_style == BraceStyle::Whitesmith
            && starts_output_line
            && self.output.last_line_outside_comment().is_some_and(|line| {
                let code = self.output.code_trimmed_of(line);
                code.trimmed_start().starts_with("return ") && code.ends_with(':')
            });
        let pending_line_indent = self
            .current_is_blank()
            .then_some(())
            .and_then(|()| {
                self.layout
                    .continuation_indent
                    .next_line_indent_spaces
                    .or_else(|| {
                        self.layout
                            .continuation_indent
                            .next_line_indent
                            .map(|level| level * self.options.indent_width)
                    })
            })
            .filter(|spaces| {
                !starts_output_line
                    || keeps_return_continuation_column
                    || spaces.is_multiple_of(self.options.indent_width)
            });
        let enclosing_else_body_indent = self.current_is_blank().then_some(()).and_then(|()| {
            self.layout
                .frame_stack
                .active_header()
                .filter(|frame| frame.header == "else" && self.active_else_expects_body())
                .map(|frame| frame.body_indent_spaces)
        });
        let inline_header_indent = self
            .layout
            .inline_nested_header_braceless_bias
            .filter(|_| word == "else" || !self.token_input.token_begins_source_line)
            .map(|level| level * self.options.indent_width);
        // A header after `case X:` on its line stands in the case body.
        let case_label_body_indent = (!self.current_is_blank())
            .then(|| self.output.code_of(&self.current))
            .filter(|code| {
                let code = code.trimmed();
                code.ends_with(':') && (is_case_label_start(code) || is_default_label_start(code))
            })
            .map(|_| {
                self.switch_label_indent_spaces().unwrap_or_else(|| {
                    (self
                        .layout
                        .indentation
                        .line_indent(LineKind::SwitchLabel, self.options)
                        + self.case_body_indent_extra(LineKind::SwitchLabel))
                        * self.options.indent_width
                }) + self.options.indent_width
            });
        let line_indent_spaces = attached_closing_indent
            .or(closed_if_indent)
            .or(open_if_indent)
            .or(inline_header_indent)
            .or(sequential_after_close_indent)
            .or(enclosing_else_body_indent)
            .or(pending_line_indent)
            .or(case_label_body_indent)
            .unwrap_or_else(|| {
                let current_indent = self.current_line_indent_spaces();
                let current_indent = if starts_output_line
                    && !current_indent.is_multiple_of(self.options.indent_width)
                {
                    current_indent - current_indent % self.options.indent_width
                } else {
                    current_indent
                };
                let structural_indent = current_indent
                    + self.layout.else_if_break_depths.len() * self.options.indent_width;
                self.layout
                    .frame_stack
                    .active_brace()
                    .filter(|frame| frame.semantic_kind == BraceSemanticKind::Command)
                    .map_or(structural_indent, |frame| {
                        structural_indent
                            .max(frame.header_indent_column + self.options.indent_width)
                    })
            });
        let parent_delimiter = self
            .layout
            .frame_stack
            .active_delimiter_with_id()
            .map(|(id, _)| id);
        self.layout.frame_stack.push_header(HeaderFrame {
            header: word.to_string(),
            line_indent_spaces,
            body_indent_spaces: line_indent_spaces + self.options.indent_width,
            parent_delimiter,
        });
    }

    pub(crate) fn update_command_word(&mut self, word: &str, next: Option<&Token>) {
        let word_is_macro_argument = matches!(next, Some(Token::Symbol(',')));
        let objc_header = self
            .current
            .trimmed_end()
            .ends_with('@')
            .then_some(match word {
                "autoreleasepool" => Some("autoreleasepool"),
                "try" => Some("@try"),
                "catch" => Some("@catch"),
                "finally" => Some("@finally"),
                _ => None,
            })
            .flatten();
        let header = if let Some(header) = objc_header {
            Some(header)
        } else if ((is_header(self.options, word) && !word_is_macro_argument)
            || is_asm_block_header(word) && matches!(next, Some(Token::Symbol('{'))))
            && self.word_can_be_header_here(word, next)
        {
            Some(word)
        } else {
            None
        };
        if let Some(header) = header {
            self.layout.command_state.current_header = Some(header.to_string());
            if matches!(header, "case" | "default") {
                self.layout.command_state.case_label_colon_emitted = false;
            }
            self.layout.command_state.header_broken_before_comment = false;
            self.layout.command_state.preprocessor_after_header = false;
            self.record_header_frame(header);
        }
        if let Some(header) = header {
            self.observe_block_spacing_header(header);
        }
        if (language::BLOCK_WORDS.contains(&word) || language::PRE_BLOCK_WORDS.contains(&word))
            && self.layout.nesting.paren_depth == 0
            && self.block_word_is_recognized(word, next)
            && !(matches!(word, "class" | "struct")
                && self.layout.command_state.pending_block_word.as_deref() == Some("enum"))
        {
            self.layout.command_state.pending_block_word = Some(word.to_string());
        }
        self.layout.command_state.observe_text(word);
    }

    fn word_can_be_header_here(&self, word: &str, next: Option<&Token>) -> bool {
        let current = self.current.trimmed_end();
        if current.trimmed_start().starts_with('#') || (word == "else" && current.ends_with('#')) {
            return false;
        }
        if word == "if" {
            let before = current.trimmed();
            let before = before.strip_prefix('}').map_or(before, str::trim_start);
            // A statement or a label may precede it on its line.
            let follows_statement = before.ends_with(';')
                || before.ends_with(':')
                    && !before.ends_with("::")
                    && self.layout.nesting.paren_depth == 0
                    && !self.layout.nesting.has_question_in_current_brace();
            if !(before.is_empty()
                || before == "else"
                || follows_statement
                || self
                    .layout
                    .command_state
                    .current_header
                    .as_deref()
                    .is_some_and(|header| is_add_braces_header(self.options, header)))
            {
                return false;
            }
            // A macro may stand for the parenthesized condition.
            return matches!(
                next,
                Some(Token::Symbol('(') | Token::Newline | Token::Comment(_, _) | Token::Word(_))
            );
        }
        if word == "else" && matches!(next, Some(Token::Symbol(','))) {
            return false;
        }
        // After the type of a declaration a `foreach` names a function.
        if matches!(word, "foreach" | "Q_FOREACH")
            && current
                .trimmed_start()
                .chars()
                .next_back()
                .is_some_and(|ch| ch == '_' || ch == '*' || ch.is_alphanumeric())
            && !matches!(trailing_word(current), "else" | "do")
        {
            return false;
        }
        if matches!(word, "for" | "while" | "switch") {
            return matches!(
                next,
                Some(Token::Symbol('(') | Token::Newline | Token::Comment(_, _))
            );
        }
        if word == "catch" {
            return matches!(
                next,
                Some(Token::Symbol('(') | Token::Newline | Token::Comment(_, _))
            ) && (current.ends_with('}')
                || self.layout.command_state.previous_non_ws_char == Some('}'));
        }
        if word == "do" {
            return !matches!(next, Some(Token::Operator(operator)) if operator == "::");
        }
        if !matches!(word, "try" | "__try") {
            return true;
        }
        current.is_empty() || current.ends_with('}') || current.ends_with(')')
    }

    fn block_word_is_recognized(&self, word: &str, next: Option<&Token>) -> bool {
        match word {
            "module" => {
                self.layout.command_state.previous_non_ws_char != Some(')')
                    && matches!(
                        next,
                        Some(Token::Word(name))
                            if name.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic())
                    )
            }
            "interface" => self.layout.command_state.pending_block_word.is_none(),
            _ => true,
        }
    }

    pub(crate) fn push_pre_brace_header(&mut self) -> Option<String> {
        let header = self
            .layout
            .command_state
            .current_header
            .take()
            .filter(|header| {
                if !matches!(header.as_str(), "case" | "default") {
                    return true;
                }
                let current = self.current.trimmed();
                is_case_label_start(current)
                    || is_default_label_start(current)
                    || (self.current_is_blank()
                        && self.output.last_line_outside_comment().is_some_and(|line| {
                            let line = line.trimmed();
                            is_case_label_start(line) || is_default_label_start(line)
                        }))
                    || (self.layout.command_state.case_label_colon_emitted
                        && self.layout.command_state.previous_non_ws_char == Some(':'))
            })
            .or_else(|| {
                let word = leading_identifier(self.current.trimmed_start());
                (is_header(self.options, word)
                    && (language::is_non_paren_header(word)
                        || self.layout.command_state.previous_command_char == Some(')')))
                .then(|| word.to_string())
            })
            .or_else(|| {
                if !self.current_is_blank()
                    || !self
                        .output
                        .last_non_empty_scoped()
                        .is_some_and(|line| line.trimmed_start().starts_with('#'))
                {
                    return None;
                }
                let header = self.layout.frame_stack.active_header()?.header.clone();
                let line = self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| {
                        !line.trimmed().is_empty() && !line.trimmed_start().starts_with('#')
                    })?
                    .split("//")
                    .next()
                    .unwrap_or_default()
                    .trimmed_end();
                if line.ends_with_any(b";{}")
                    || !line
                        .split(|ch: char| !is_word_char(ch))
                        .any(|word| word == header)
                {
                    return None;
                }
                Some(header)
            })?;
        self.observe_block_spacing_body_start();
        self.layout.command_state.preprocessor_after_header = false;
        self.layout
            .command_state
            .pre_brace_header_stack
            .push(header.clone());
        Some(header)
    }

    pub(crate) fn preprocessor_interrupted_header_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        indent: usize,
        current_spaces: Option<usize>,
        interrupted_header_context: impl FnOnce() -> bool,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        // A block given to a braceless header stands at that header.
        let header_body_block = line_start.starts_with('{')
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| line_is_control_body_header(previous.trimmed_start()));
        if header_body_block
            || line_kind != LineKind::Normal
            || line_start.starts_with_any(b"#}:)")
            || is_header(self.options, leading_identifier(line_start))
            || self.pending_line_in_parens()
            || self.pending_line_continues_statement()
            || !interrupted_header_context()
        {
            return None;
        }
        let current_opens_brace = self.output.code_trimmed_of(line).ends_with('{');
        let frame = if current_opens_brace {
            self.layout.frame_stack.enclosing_brace()
        } else {
            self.layout.frame_stack.active_brace()
        }?;
        let natural = indent * self.options.indent_width;
        if frame.semantic_kind != BraceSemanticKind::Command
            || frame.sibling_indent_column < natural
        {
            return None;
        }
        let current = current_spaces.unwrap_or(natural);
        let mut target = frame.body_indent_column;
        let mut multiline_header_body = false;
        if let Some(header_indent) = self.current_closing_multiline_header_indent() {
            target = header_indent + self.options.indent_width;
            multiline_header_body = true;
        }
        (target > current || multiline_header_body && current > target).then_some(target)
    }

    pub(crate) fn split_else_condition_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || !line.trimmed_start().starts_with(')')
            || !self.output.code_trimmed_of(line).ends_with('{')
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        self.output.scoped().iter().rev().take(16).find_map(|line| {
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            (self.open_paren_column_of(code).is_some()
                && (starts_header_word(trimmed, "if")
                    || starts_header_word(trimmed, "while")
                    || starts_header_word(trimmed, "for")
                    || trimmed.starts_with("else if")
                    || trimmed.starts_with("} else")))
            .then_some(
                leading_visual_width(line, self.options.tab_width) + self.options.indent_width,
            )
        })
    }

    pub(crate) fn preprocessor_interrupted_else_if_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(trimmed.starts_with("} else if") || trimmed.starts_with("}else if")) {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        preprocessor_directive(previous.trimmed_start())?;
        self.output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .take(32)
            .find(|line| {
                let code = self.output.code_trimmed_of(line);
                let trimmed = code.trimmed_start();
                code.ends_with('{')
                    && (starts_header_word(trimmed, "if")
                        || trimmed.starts_with("} else")
                        || trimmed.starts_with("}else"))
            })
            .map(|header| leading_visual_width(header, self.options.tab_width))
    }

    pub(crate) fn split_else_matching_if_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with("else")
            || (self.preprocessor.split_else.extra_indent && !self.preprocessor_split_else_active())
        {
            return None;
        }
        let close_index = (0..self.output.len())
            .rev()
            .find(|index| !self.output[*index].trimmed().is_empty())?;
        if self.output.code_trimmed(close_index) != "}" {
            return None;
        }
        let mut depth = 0usize;
        for index in (0..close_index).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes();
            if meta.opens() > depth {
                let trimmed = self.output.code_trimmed(index);
                let matches_if =
                    starts_header_word(trimmed, "if") || trimmed.starts_with("else if");
                return matches_if.then(|| {
                    current_spaces.unwrap_or(0).max(
                        self.output.lead_width(index, self.options.tab_width)
                            + self.layout.line_adjuster.next_line_case_unindent_depth()
                                * self.options.indent_width,
                    )
                });
            }
            depth = depth.saturating_sub(meta.opens());
        }
        None
    }

    pub(crate) fn else_indent_from_previous_if(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with("else")
            || self.preprocessor.split_else.extra_indent
        {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(';') {
            return None;
        }
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        let previous_trimmed = previous_code.trimmed_start();
        let previous_header = previous_trimmed
            .strip_prefix('}')
            .map(str::trim_start)
            .unwrap_or(previous_trimmed);
        if starts_header_word(previous_header, "if") || previous_header.starts_with("else if") {
            return Some(
                previous_spaces
                    + same_line_nested_header_extra(previous_header) * self.options.indent_width,
            );
        }
        if previous_spaces.is_multiple_of(self.options.indent_width)
            && !previous.trimmed_start().starts_with(',')
        {
            return None;
        }
        for line in self.output.scoped().iter().rev().skip(1) {
            let trimmed = line.trimmed_start();
            if trimmed.is_empty()
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with("*/")
                || trimmed.starts_with('?')
                || trimmed.starts_with(':')
            {
                continue;
            }
            let spaces = leading_visual_width(line, self.options.tab_width);
            if !spaces.is_multiple_of(self.options.indent_width) {
                continue;
            }
            let header = trimmed
                .strip_prefix('}')
                .map(str::trim_start)
                .unwrap_or(trimmed);
            if starts_header_word(header, "if") || header.starts_with("else if") {
                return Some(spaces);
            }
        }
        None
    }

    pub(crate) fn detached_else_nested_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with("else")
            || self.preprocessor.split_else.extra_indent
            || !matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
            || self
                .output
                .last_line_outside_comment()
                .is_none_or(|line| line.trimmed() != "}")
        {
            return None;
        }
        let mut open_indent: Option<usize> = None;
        for scan_index in self.output.scoped_range().rev().skip(1) {
            let previous = &self.output[scan_index];
            let previous_trimmed = self.output.code_body(scan_index);
            if previous_trimmed.is_empty() {
                continue;
            }
            if let Some(open_spaces) = open_indent {
                if (starts_header_word(previous_trimmed, "if")
                    || previous_trimmed.starts_with("else if"))
                    && previous_trimmed.contains(" if")
                {
                    let header_spaces = leading_visual_width(previous, self.options.tab_width);
                    let brace_indent = usize::from(
                        self.options.indent_braces
                            || matches!(
                                self.options.brace_style,
                                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Gnu
                            ),
                    ) * self.options.indent_width;
                    let target = open_spaces.saturating_sub(brace_indent);
                    return (target > header_spaces).then_some(target);
                }
                break;
            }
            if previous_trimmed == "{" {
                open_indent = Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        None
    }

    pub(crate) fn none_style_split_else_closing_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || !(trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            || self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| preprocessor_directive(line.trimmed_start()).is_some())
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let mut result = current_spaces;
        if let Some(previous) = self.output.last_line_outside_comment() {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous.trimmed() == "}"
                || (previous_code.ends_with(';') && !previous_code.ends_with("};"))
            {
                let target = leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if result.unwrap_or(output_spaces) < target {
                    result = Some(target);
                }
            }
        }
        let mut depth = 1usize;
        let mut matching_open = None;
        for candidate in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
        {
            let code = self.output.code_trimmed_of(candidate);
            let (closes, opens) = line_brace_imbalance(code);
            depth += closes;
            if opens >= depth {
                matching_open = Some(candidate);
                break;
            }
            depth = depth.saturating_sub(opens);
        }
        let matching_open = matching_open?;
        let matching_code = self.output.code_trimmed_of(matching_open);
        let matching_trimmed = matching_code.trimmed_start();
        let target = if matching_trimmed.starts_with(')') && matching_code.ends_with('{') {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != matching_open.as_str())
                .skip(1)
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    let trimmed = code.trimmed_start();
                    (self.open_paren_column_of(code).is_some()
                        && (starts_header_word(trimmed, "if")
                            || starts_header_word(trimmed, "while")
                            || starts_header_word(trimmed, "for")
                            || trimmed.starts_with("else if")
                            || trimmed.starts_with("} else")))
                    .then_some(leading_visual_width(line, self.options.tab_width))
                })
                .unwrap_or_else(|| leading_visual_width(matching_open, self.options.tab_width))
        } else {
            leading_visual_width(matching_open, self.options.tab_width)
        };
        if result.unwrap_or(output_spaces) > target {
            result = Some(target);
        }
        (result != current_spaces).then_some(result?)
    }

    pub(crate) fn preprocessor_interrupted_closing_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_directive = preprocessor_directive(previous_code.trimmed_start());
        let mut result = None;
        if previous_directive.is_some()
            && let Some(header) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(32)
                .find(|line| {
                    let code = self.output.code_trimmed_of(line);
                    let trimmed = code.trimmed_start();
                    trimmed.starts_with("} else") || trimmed.starts_with("}else")
                })
        {
            result = Some(leading_visual_width(header, self.options.tab_width));
        }
        if previous_directive == Some("endif") {
            let mut depth = 1usize;
            for candidate in self
                .output
                .scoped()
                .iter()
                .rev()
                .filter(|line| !line.trimmed().is_empty())
            {
                let code = self.output.code_trimmed_of(candidate);
                let (closes, opens) = line_brace_imbalance(code);
                if depth == 1
                    && opens > 0
                    && closes > 0
                    && code.trimmed_start().starts_with("} else")
                {
                    result = Some(leading_visual_width(candidate, self.options.tab_width));
                    break;
                }
                depth += closes;
                if opens >= depth {
                    result = Some(leading_visual_width(candidate, self.options.tab_width));
                    break;
                }
                depth = depth.saturating_sub(opens);
            }
        }
        if split_else_context && previous_code.trimmed() == "}" {
            result = Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width),
            );
        }
        if split_else_context
            && previous_code.ends_with(';')
            && !previous_code.ends_with("};")
            && !previous_code.trimmed_start().starts_with('(')
            && !self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
                .is_some_and(|line| {
                    let code = self.output.code_trimmed_of(line);
                    code.ends_with(',') || self.open_paren_column_of(code).is_some()
                })
        {
            result = Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width),
            );
        }
        result
    }

    pub(crate) fn split_else_endif_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_output_context: bool,
    ) -> Option<usize> {
        if !split_else_output_context || line.trimmed_start().starts_with_any(b"#{}") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if preprocessor_directive(previous.trimmed_start()) != Some("endif") {
            return None;
        }
        let sibling = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })?;
        let sibling_trimmed = self.output.code_trimmed_of(sibling).trimmed_start();
        if sibling_trimmed != "} else" && sibling_trimmed != "}else" {
            return None;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != sibling.as_str())
            .skip(1)
            .find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .map(|line| leading_visual_width(line, self.options.tab_width))
    }

    pub(crate) fn structural_split_else_closing_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let (open_spaces, _, open_code) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        // A header kept on a case label's line stands in the case body.
        let open_spaces = open_spaces
            + usize::from(
                open_code
                    .strip_suffix('{')
                    .is_some_and(|head| label_line_holds_braceless_header(head.trimmed_end())),
            ) * self.options.indent_width;
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        let closing_multiline_header_indent = self.current_closing_multiline_header_indent();
        let open_spaces = closing_multiline_header_indent.unwrap_or(open_spaces);
        let split_else_chain =
            structural_split_else_chain || self.output.recent_scoped_else_line(128);
        let recent_adjacent_string_call =
            self.output.scoped_range().rev().take(8).any(|index| {
                let code = self.output.code_before_comment_trimmed(index);
                code.ends_with(");") && starts_string_literal_token(code.trimmed_start())
            }) && self.output.scoped_range().rev().take(8).any(|index| {
                let code = self.output.code_before_comment_trimmed(index);
                self.open_paren_column_of(code).is_some()
                    && !starts_string_literal_token(code.trimmed_start())
                    && !code.ends_with(';')
            });
        if split_else_chain
            && closing_multiline_header_indent.is_some()
            && previous_code.ends_with(';')
            && current_spaces != open_spaces
            || recent_adjacent_string_call
                && previous_code.ends_with(");")
                && starts_string_literal_token(previous_code.trimmed_start())
                && current_spaces != open_spaces
            || preprocessor_directive(previous_code.trimmed_start()) == Some("endif")
                && current_spaces != open_spaces
            || structural_split_else_chain
                && (previous_code.ends_with(';') || previous_code.trimmed() == "}")
                && (current_spaces < open_spaces || closing_multiline_header_indent.is_some())
            || split_else_chain
                && (previous_code.ends_with(';') || previous_code.trimmed() == "}")
                && current_spaces > open_spaces
            || split_else_chain && previous_code.trimmed() == "}" && !line.contains('{')
        {
            return Some(open_spaces);
        }
        None
    }

    pub(crate) fn preprocessor_closing_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        normal_indent: usize,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let mut result = current_spaces;
        let split_else_inactive =
            !self.preprocessor.split_else.extra_indent && !self.preprocessor_split_else_active();
        if split_else_inactive && let Some(previous) = self.output.last_line_outside_comment() {
            let previous_code = self.output.code_trimmed_of(previous);
            if preprocessor_directive(previous_code.trimmed_start())
                .is_some_and(is_conditional_preprocessor)
            {
                let target = normal_indent * self.options.indent_width;
                if result.unwrap_or(output_spaces) > target {
                    result = Some(target);
                }
            }
        }
        if (trimmed.starts_with("} else if") || trimmed.starts_with("}else if"))
            && let Some(previous) = self.output.last_line_outside_comment()
            && self.output.code_trimmed_of(previous).ends_with('{')
            && let Some((open_spaces, _, open)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && (open.trimmed_start().starts_with("} else")
                || open.trimmed_start().starts_with("}else"))
            && result.unwrap_or(output_spaces) != open_spaces
        {
            result = Some(open_spaces);
        }
        if result.is_none()
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous_code.ends_with('{')
                && let Some((open_spaces, _, _)) = self
                    .output
                    .current_closing_brace_open(self.options.tab_width)
                && open_spaces < output_spaces
            {
                result = Some(open_spaces);
            }
        }
        if result.is_none()
            && split_else_inactive
            && self.current_closing_multiline_header_indent().is_none()
            && let Some((open_spaces, _, _)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && open_spaces > output_spaces
        {
            result = Some(open_spaces);
        }
        (result != current_spaces).then_some(result?)
    }

    pub(crate) fn closing_header_body_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        if current_spaces.is_some() || line.trimmed() != "{" {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.trimmed_start().starts_with("} else") {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (target > output_spaces).then_some(target)
    }

    pub(crate) fn recent_split_else_if_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        recent_split_else_chain: impl FnOnce() -> bool,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with("else if")
            || !recent_split_else_chain()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = self.output.code_of(previous).trimmed();
        if previous_code != "}" {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces.unwrap_or(usize::MAX) > target).then_some(target)
    }

    pub(crate) fn match_closing_while_to_braceless_do(&mut self) {
        while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
            && !self.braceless_header_accepts_while(base)
        {
            self.layout.indentation.exit_braceless_block();
        }
        if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
            && self.braceless_header_accepts_while(base)
        {
            self.layout
                .continuation_indent
                .set_next_line_level(base + self.layout.line_adjuster.total_case_unindent_depth());
            self.layout.indentation.exit_braceless_block();
            if self
                .layout
                .frame_stack
                .active_braceless_header()
                .is_some_and(|frame| frame.header == "do")
            {
                self.layout.frame_stack.pop_braceless_header();
            }
            return;
        }
        let mut pending_whiles = 0usize;
        for line in self.output.scoped().iter().rev() {
            let trimmed = line.trimmed_start();
            if trimmed.is_empty() {
                continue;
            }
            let code = self.output.code_trimmed_of(trimmed);
            if code.ends_with('{') || code.ends_with('}') || code.ends_with(')') {
                return;
            }
            let (do_count, rest) = leading_do_headers(code);
            if do_count > 0 && (rest.is_empty() || code.ends_with(';')) {
                let net = do_count.saturating_sub(inline_while_closers(rest));
                if net > pending_whiles {
                    let level = leading_visual_width(line, self.options.tab_width)
                        / self.options.indent_width;
                    self.layout.continuation_indent.set_next_line_level(
                        level + net - pending_whiles - 1
                            + self.layout.line_adjuster.total_case_unindent_depth(),
                    );
                    return;
                }
                pending_whiles -= net;
                continue;
            }
            if header_word_is(code, "while") && code.ends_with(';') {
                pending_whiles += 1;
                continue;
            }
            if !code.ends_with(';')
                || leading_visual_width(line, self.options.tab_width) < self.options.indent_width
            {
                return;
            }
        }
    }
}

fn leading_do_headers(code: &str) -> (usize, &str) {
    let mut rest = code;
    let mut count = 0;
    while let Some(after) = rest.strip_prefix("do") {
        if after.chars().next().is_some_and(is_identifier_continue) {
            break;
        }
        count += 1;
        rest = after.trimmed_start();
    }
    (count, rest)
}

fn inline_while_closers(rest: &str) -> usize {
    rest.split(';')
        .skip(1)
        .filter(|segment| header_word_is(segment.trimmed_start(), "while"))
        .count()
}

pub(crate) fn is_attachable_closing_header(word: &str) -> bool {
    matches!(
        word,
        "else" | "catch" | "@catch" | "@finally" | "__finally" | "__except"
    ) || word.ends_with("CATCH")
}

fn header_word_is(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|rest| matches!(rest.chars().next(), Some('(') | Some(' ')))
}

pub(crate) fn is_header(options: &FormatOptions, word: &str) -> bool {
    language::is_header(word) || options.control_headers.iter().any(|header| header == word)
}

pub(crate) fn is_add_braces_header(options: &FormatOptions, word: &str) -> bool {
    is_standard_add_braces_header(word)
        || options.control_headers.iter().any(|header| header == word)
}

#[cfg(test)]
mod tests {
    use super::nested_header_word_candidates;

    fn candidates_by_search(code: &str) -> usize {
        let bytes = code.as_bytes();
        ["if", "for", "while", "switch", "do"]
            .iter()
            .map(|word| {
                code.match_indices(word)
                    .filter(|&(index, _)| {
                        let before = index.checked_sub(1).map(|at| bytes[at]);
                        let after = bytes.get(index + word.len()).copied();
                        !before.is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                            && !after
                                .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    })
                    .count()
            })
            .sum()
    }

    #[test]
    fn counts_header_word_spots_as_a_search_for_each_word_does() {
        let pieces = [
            "if", "for", "while", "switch", "do", "i", "f", "o", "_", "1", " ", "(", "é", "w",
            "hile", "s", "d", "{",
        ];
        let mut state = 0x9e37_79b9_u32;
        for _ in 0..20_000 {
            let mut code = String::new();
            for _ in 0..(state % 9) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                code.push_str(pieces[state as usize % pieces.len()]);
            }
            state = state.wrapping_add(1);
            assert_eq!(
                nested_header_word_candidates(&code),
                candidates_by_search(&code),
                "{code:?}"
            );
        }
    }
}
