use super::FormatEngine;
use super::assembly::is_asm_block_header;
use super::columns::{leading_visual_width, visual_column_at};
use super::frame::{BraceSemanticKind, HeaderFrame};
use super::indentation::LineKind;

use super::language;

use super::line_scan::{is_comment_line, is_comment_only_line};
use super::line_scan::{
    line_brace_imbalance, line_paren_imbalance, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use super::literals::starts_string_literal_token;
use super::operators::head_ends_binary_operator;
use super::preprocessor::{is_conditional_preprocessor, preprocessor_directive};
use super::rewrite::is_add_braces_header;
use super::token::Token;
use crate::config::BraceStyle;
use crate::source::lex::{is_identifier_continue, leading_identifier};
use crate::source::lex::{is_identifier_start, is_word_char};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct HeaderParenState {
    pub(super) depth: Option<usize>,
    pub(super) just_closed: bool,
    pub(super) post_paren: bool,
}

pub(super) fn starts_header_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word).is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

pub(super) fn is_conditional_header_line(line: &str) -> bool {
    let mut trimmed = line.trim_start();
    if let Some(rest) = trimmed.strip_prefix("else")
        && rest.starts_with(char::is_whitespace)
    {
        trimmed = rest.trim_start();
    }
    let word: String = trimmed
        .chars()
        .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
        .collect();
    matches!(word.as_str(), "if" | "for" | "while" | "switch")
}

pub(super) fn is_braceless_header_line(line: &str) -> bool {
    starts_header_word(line, "if")
        || line.starts_with("else if")
        || starts_header_word(line, "for")
        || starts_header_word(line, "while")
        || starts_header_word(line, "switch")
}

pub(super) fn line_is_control_body_header(line: &str) -> bool {
    let trimmed = line.trim_end();
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

pub(super) fn same_line_nested_header_extra(line: &str) -> usize {
    let code = line.trim_end().trim_end_matches('{').trim_end();
    let mut count = 0usize;
    let mut index = 0;
    while index < code.len() {
        let rest = &code[index..];
        let Some(offset) = rest.find(is_identifier_start) else {
            break;
        };
        index += offset;
        let word_end = code[index..]
            .find(|ch: char| !is_identifier_continue(ch))
            .unwrap_or(code.len() - index);
        let word = &code[index..index + word_end];
        if matches!(word, "if" | "for" | "while" | "switch" | "do") {
            count += 1;
        }
        index += word.len();
    }
    if code.starts_with("else while")
        || code.starts_with("else for")
        || code.starts_with("else do")
        || code.starts_with("else switch")
    {
        count += 1;
    }
    count.saturating_sub(1)
}

fn is_split_loop_header(line: &str) -> bool {
    (starts_header_word(line, "while") || starts_header_word(line, "for"))
        && unmatched_open_paren_column(line).is_some()
}

pub(super) struct ElseBodyLayout {
    pub(super) indent_level: Option<usize>,
    pub(super) indent_spaces: usize,
}

impl FormatEngine<'_> {
    pub(super) fn ready_non_paren_header_indent_spaces(&self, line: &str) -> Option<usize> {
        let line_start = line.trim_start();
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

    pub(super) fn is_header(&self, word: &str) -> bool {
        language::is_header(word)
            || self
                .options
                .control_headers
                .iter()
                .any(|header| header == word)
    }

    pub(super) fn maximum_length_conditional_continuation_floor(
        &self,
        line: &str,
        base_indent_width: usize,
    ) -> Option<usize> {
        (is_conditional_header_line(line) && !self.options.indent_after_parens)
            .then(|| base_indent_width + self.min_conditional_indent_spaces())
    }

    pub(super) fn is_add_braces_header(&self, word: &str) -> bool {
        is_add_braces_header(word)
            || self
                .options
                .control_headers
                .iter()
                .any(|header| header == word)
    }

    pub(super) fn active_split_else_header_continuation_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
            || !self.preprocessor_split_else_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !head_ends_binary_operator(previous_code)
            || !line_is_control_body_header(previous_code.trim_start())
        {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.min_conditional_indent_spaces(),
        )
    }

    pub(super) fn active_split_else_multiline_header_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !self.preprocessor_split_else_active()
            || line.trim_start().starts_with(['{', '}', '#'])
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let mut result = previous_code.ends_with(") {").then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width / 2
        });
        if previous_code.trim_start().starts_with(')')
            && let Some(header_indent) = self.current_closing_multiline_header_indent()
        {
            let nested_header_group = self.split_else_body_indent_active()
                && self.output.iter().rev().skip(1).take(16).any(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
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
                        .is_some_and(|tail| tail.trim_start().starts_with("( ("));
                    let guarded_header = self
                        .output
                        .iter()
                        .rev()
                        .skip_while(|candidate| candidate.as_str() != line.as_str())
                        .skip(1)
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| preprocessor_directive(line.trim_start()).is_some());
                    starts_nested_group && !guarded_header && line_paren_imbalance(code).1.len() > 1
                });
            result = Some(if nested_header_group {
                header_indent
            } else {
                header_indent + self.options.indent_width
            });
        }
        result
    }

    pub(super) fn opening_conditional_directive_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['#', '{', '}']) {
            return None;
        }
        let previous_index = self
            .output
            .iter()
            .rposition(|line| !line.trim().is_empty())?;
        if !preprocessor_directive(self.output[previous_index].trim_start())
            .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        {
            return None;
        }
        let before_index = self.output[..previous_index]
            .iter()
            .rposition(|line| !line.trim().is_empty())?;
        let before = &self.output[before_index];
        let before_code = before[..trailing_comment_split_limit(before)].trim_end();
        let before_trimmed = before_code.trim_start();
        if before_trimmed == "else"
            || before_trimmed.ends_with("} else")
            || before_trimmed.ends_with("}else")
        {
            return Some(
                leading_visual_width(before, self.options.tab_width) + self.options.indent_width,
            );
        }
        is_comment_line(before.trim_start())
            .then(|| leading_visual_width(before, self.options.tab_width))
    }

    pub(super) fn active_split_else_open_header_brace_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
            || !self.preprocessor_split_else_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with('{') {
            return None;
        }
        let open_index = (0..self.output.len()).rev().find(|&index| {
            !self.output[index].trim().is_empty() && self.output.code_trimmed(index).ends_with('{')
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
            let (closes, opens) = line_paren_imbalance(self.output.code(index));
            depth = depth.saturating_sub(closes);
            depth += opens.len();
        }
        (depth > 0).then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn pending_split_else_braceless_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        natural_indent_spaces: usize,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}', '/'])
            || self.is_header(leading_identifier(line))
            || self.pending_braceless_block_bias.is_none()
            || !(self.split_else_braceless_body_active()
                || self
                    .output
                    .last_non_empty_line()
                    .is_some_and(|previous| is_comment_only_line(previous.trim_start())))
        {
            return None;
        }
        let header = self
            .frame_stack
            .active_header()
            .filter(|header| self.is_add_braces_header(&header.header))?;
        if self.split_else_braceless_body_active() {
            return Some(header.body_indent_spaces);
        }
        (current_indent_spaces.unwrap_or(natural_indent_spaces) < header.body_indent_spaces)
            .then_some(header.body_indent_spaces)
    }

    pub(super) fn split_condition_closing_paren_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with(')') {
            return None;
        }
        for previous in self
            .output
            .iter()
            .rev()
            .take(64)
            .filter(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            if code.ends_with([';', '{', '}']) {
                return None;
            }
            if trimmed == "(" && unmatched_open_paren_column(code).is_some() {
                return Some(leading_visual_width(previous, self.options.tab_width));
            }
            if let Some(open) = unmatched_open_paren_column(code)
                && (starts_header_word(trimmed, "if")
                    || starts_header_word(trimmed, "while")
                    || starts_header_word(trimmed, "for")
                    || starts_header_word(trimmed, "do")
                    || trimmed.starts_with("else if")
                    || trimmed.starts_with("} else"))
            {
                let code_chars = code.chars().collect::<Vec<_>>();
                let open_column = visual_column_at(&code_chars, open, self.options.tab_width);
                if trimmed.starts_with("else if")
                    || trimmed.starts_with("} else if")
                    || trimmed.starts_with("}else if")
                {
                    return Some(open_column);
                }
                let header_indent = leading_visual_width(previous, self.options.tab_width);
                if self.split_else_body_indent_active() {
                    let opens = line_paren_imbalance(code).1;
                    let starts_nested_group = trimmed
                        .strip_prefix("else if")
                        .or_else(|| trimmed.strip_prefix("if"))
                        .or_else(|| trimmed.strip_prefix("while"))
                        .or_else(|| trimmed.strip_prefix("for"))
                        .or_else(|| trimmed.strip_prefix("switch"))
                        .is_some_and(|tail| tail.trim_start().starts_with("( ("));
                    let guarded_header = self
                        .output
                        .iter()
                        .rev()
                        .skip_while(|line| line.as_str() != previous.as_str())
                        .skip(1)
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| preprocessor_directive(line.trim_start()).is_some());
                    return Some(
                        if opens.len() > 1 && starts_nested_group && !guarded_header {
                            header_indent
                        } else {
                            opens.first().map_or(open_column, |&column| {
                                visual_column_at(&code_chars, column, self.options.tab_width)
                            })
                        },
                    );
                }
                return Some(open_column.max(header_indent + self.options.indent_width / 2));
            }
        }
        None
    }

    pub(super) fn current_closing_multiline_header_indent(&self) -> Option<usize> {
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            let trimmed = self.output.code_trimmed(index);
            if depth == 0
                && meta.opens > 0
                && (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            {
                return None;
            }
            depth += meta.closes;
            if meta.opens > depth {
                if trimmed.ends_with('{')
                    && !starts_header_word(trimmed, "if")
                    && !starts_header_word(trimmed, "for")
                    && !starts_header_word(trimmed, "while")
                    && !starts_header_word(trimmed, "switch")
                    && !trimmed.starts_with("else if")
                    && !trimmed.starts_with("} else")
                    && !trimmed.starts_with("}else")
                {
                    return self.output[..index].iter().rev().take(16).find_map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        let trimmed = code.trim_start();
                        (unmatched_open_paren_column(code).is_some()
                            && (starts_header_word(trimmed, "if")
                                || starts_header_word(trimmed, "for")
                                || starts_header_word(trimmed, "while")
                                || starts_header_word(trimmed, "switch")
                                || trimmed.starts_with("else if")
                                || trimmed.starts_with("} else")
                                || trimmed.starts_with("}else")))
                        .then_some(leading_visual_width(line, self.options.tab_width))
                    });
                }
                return None;
            }
            depth = depth.saturating_sub(meta.opens);
        }
        None
    }

    pub(super) fn else_split_header_indent_spaces(&self, line: &str) -> Option<usize> {
        let mut previous = self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty());
        let first = previous.next()?;
        let second = previous.next()?;
        if line.trim() == ";" {
            let third = previous.next()?;
            let header = second.trim_start();
            if first.trim_end().ends_with(')')
                && third.trim() == "else"
                && is_split_loop_header(header)
            {
                return Some(
                    leading_visual_width(second, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            return None;
        }
        let header = first.trim_start();
        if second.trim() == "else" && is_split_loop_header(header) {
            return Some(
                leading_visual_width(first, self.options.tab_width) + self.options.indent_width * 2,
            );
        }
        None
    }

    pub(super) fn braceless_else_output_level(&self) -> Option<usize> {
        let mut body_idx = self.output.len().checked_sub(1)?;
        while body_idx > 0 && self.output[body_idx].trim().is_empty() {
            body_idx -= 1;
        }
        let body = self.output[body_idx].trim_end();
        let body_code = body[..trailing_comment_split_limit(body)].trim_end();
        if !body_code.ends_with(';') {
            return None;
        }
        let body_width = leading_visual_width(body, self.options.tab_width);
        if !body_width.is_multiple_of(self.options.indent_width) {
            return None;
        }
        let mut expected = body_width / self.options.indent_width;
        let mut scan = body_idx;
        while scan > 0 {
            scan -= 1;
            let line = self.output[scan].trim_end();
            if line.is_empty() {
                continue;
            }
            let width = leading_visual_width(line, self.options.tab_width);
            if !width.is_multiple_of(self.options.indent_width) {
                break;
            }
            let level = width / self.options.indent_width;
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            let is_header = is_braceless_header_line(trimmed);
            if level + 1 != expected {
                if level >= expected && trimmed.ends_with(')') && !is_header {
                    continue;
                }
                break;
            }
            if starts_header_word(trimmed, "if") || trimmed.starts_with("else if") {
                return Some(level + self.line_adjuster.total_case_unindent_depth());
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

    pub(super) fn multiline_else_header_continuation_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            || !self.output.may_have_else()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let mut spaces = if previous_code.trim_start().starts_with("else ") {
            unmatched_open_paren_column(previous_code).map(|open| open + 1)
        } else {
            None
        };
        if previous
            .trim_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && let Some(header) = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty())
            && header.trim_start().starts_with("else ")
            && unmatched_open_paren_column(&header[..trailing_comment_split_limit(header)])
                .is_some()
            && line_paren_imbalance(previous_code).0 == 0
        {
            spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        spaces
    }

    pub(super) fn plain_else_body_layout(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<ElseBodyLayout> {
        if line_kind != LineKind::Normal || self.split_else_body_indent_active() {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if previous.trim() != "else" {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        if line.trim() == "{" {
            return Some(ElseBodyLayout {
                indent_level: Some(previous_indent / self.options.indent_width),
                indent_spaces: previous_indent,
            });
        }
        if !self.output.may_have_else() || line.trim_start().starts_with(['{', '#']) {
            return None;
        }
        Some(ElseBodyLayout {
            indent_level: None,
            indent_spaces: previous_indent
                + self.options.indent_width
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        })
    }

    pub(super) fn else_while_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['{', '#']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_trimmed = previous.trim_start();
        if !previous_trimmed.starts_with("else while")
            || head_ends_binary_operator(previous_trimmed)
            || unmatched_open_paren_column(previous_trimmed).is_some()
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + self.options.indent_width * 2)
    }

    pub(super) fn multiline_control_header_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['{', '}', '#']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with(')') || !previous_code.ends_with('{') {
            return None;
        }
        let follows_multiline_header = self
            .output
            .iter()
            .rev()
            .skip(1)
            .take(16)
            .find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                unmatched_open_paren_column(code).is_some().then_some(
                    (starts_header_word(trimmed, "if")
                        || starts_header_word(trimmed, "for")
                        || starts_header_word(trimmed, "while")
                        || starts_header_word(trimmed, "switch"))
                        && !trimmed.starts_with("else if")
                        && !trimmed.starts_with("} else if")
                        && !trimmed.starts_with("}else if"),
                )
            })
            .unwrap_or(false);
        follows_multiline_header.then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width / 2
        })
    }

    pub(super) fn separated_else_header_body_indent_floor(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['{', '#'])
            || !(self.output.may_have_else()
                || self.output.may_have_hash()
                || self.output.may_have_comment())
        {
            return None;
        }
        let header = self.output.last_non_empty_line()?;
        let header_trimmed = header.trim_start();
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
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != header.as_str())
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        let separated = if is_comment_line(separator.trim_start()) {
            true
        } else {
            let separator_code = separator[..trailing_comment_split_limit(separator)].trim_end();
            preprocessor_directive(separator_code.trim_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
                && self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|candidate| candidate.as_str() != separator.as_str())
                    .skip(1)
                    .find(|candidate| !candidate.trim().is_empty())
                    .is_some_and(|candidate| {
                        let trimmed = candidate[..trailing_comment_split_limit(candidate)]
                            .trim_end()
                            .trim_start();
                        trimmed == "else" || trimmed.ends_with("} else")
                    })
        };
        separated.then(|| {
            leading_visual_width(header, self.options.tab_width) + self.options.indent_width
        })
    }

    pub(super) fn block_comment_separated_header_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['{', '#'])
            || !self.output.may_have_comment()
        {
            return None;
        }
        let comment = self.output.last_non_empty_line()?;
        if !comment.trim_start().starts_with("/*") {
            return None;
        }
        let header = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != comment.as_str())
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        let header_code = header[..trailing_comment_split_limit(header)].trim_end();
        let header_trimmed = header_code.trim_start();
        if header_code.ends_with('{')
            && (starts_header_word(header_trimmed, "if")
                || starts_header_word(header_trimmed, "for")
                || starts_header_word(header_trimmed, "while")
                || starts_header_word(header_trimmed, "do")
                || header_trimmed.starts_with("else"))
        {
            return Some(
                leading_visual_width(comment, self.options.tab_width)
                    + self.line_adjuster.next_line_case_unindent_depth()
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

    pub(super) fn else_body_after_comments_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['{', '#'])
            || !self.output.may_have_comment()
        {
            return None;
        }
        let mut comment_indent = None;
        for previous in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let trimmed = previous.trim_start();
            if trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with("*/")
                || trimmed == "*"
                || trimmed.starts_with("* ")
                || trimmed.starts_with("*\t")
                || trimmed.starts_with("**")
            {
                if !trimmed.starts_with("//") || comment_indent.is_none() {
                    comment_indent = Some(leading_visual_width(previous, self.options.tab_width));
                }
                continue;
            }
            if (previous.trim() == "else" || previous.trim().ends_with("} else"))
                && let Some(spaces) = comment_indent
            {
                let else_indent = leading_visual_width(previous, self.options.tab_width);
                return Some(if spaces > else_indent {
                    spaces
                } else {
                    else_indent + self.options.indent_width
                });
            }
            return None;
        }
        None
    }

    pub(super) fn control_header_line_comment_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with("//") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_trimmed = previous.trim_start();
        if !line_is_control_body_header(previous_trimmed)
            || head_ends_binary_operator(previous_trimmed)
            || unmatched_open_paren_column(previous_trimmed).is_some()
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

    pub(super) fn none_style_conditional_else_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['{', '#'])
            || self.options.brace_style != BraceStyle::None
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if (line.trim_start().starts_with("} else") || line.trim_start().starts_with("}else"))
            && preprocessor_directive(previous_code.trim_start()).is_some()
            && let Some(header) = self.output.iter().rev().skip(1).take(32).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
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
            .is_some_and(|line| line.trim().is_empty())
            || !preprocessor_directive(previous_code.trim_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        {
            return None;
        }
        let header = self.output.iter().rev().skip(1).find(|line| {
            let trimmed = line.trim_start();
            !trimmed.is_empty() && !trimmed.starts_with('#')
        })?;
        let trimmed = header[..trailing_comment_split_limit(header)]
            .trim_end()
            .trim_start();
        if trimmed != "else" && !trimmed.ends_with("} else") {
            return None;
        }
        Some(leading_visual_width(header, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn else_structural_indent_after_braced_statement_level(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trim_start().starts_with("else")
            || self.split_else_body_indent_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        (previous.trim_end().ends_with(';') && previous.contains('}')).then(|| self.state.indent())
    }

    pub(super) fn else_after_closed_nested_header_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !line.trim_start().starts_with("else") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if previous[..trailing_comment_split_limit(previous)].trim() != "}" {
            return None;
        }
        for header in self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .filter(|line| !line.trim().is_empty())
        {
            let header_code = header[..trailing_comment_split_limit(header)].trim_end();
            if header_code.trim() == "}" {
                return None;
            }
            if header_code.ends_with('{') {
                let extra = same_line_nested_header_extra(header_code.trim_start());
                return (extra > 0).then(|| {
                    leading_visual_width(header, self.options.tab_width)
                        + extra * self.options.indent_width
                });
            }
        }
        None
    }

    pub(super) fn else_after_braceless_body_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with("else") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(';') {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        for header in self
            .output
            .iter()
            .rev()
            .skip(1)
            .filter(|line| !line.trim().is_empty())
            .take(64)
        {
            let header_code = header[..trailing_comment_split_limit(header)].trim_end();
            let header_trimmed = header_code.trim_start();
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

    pub(super) fn current_closes_same_line_else_block(&self) -> bool {
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            if meta.opens > depth {
                let trimmed = self.output.code_trimmed(index);
                return trimmed.starts_with("} else") || trimmed.starts_with("}else");
            }
            depth = depth.saturating_sub(meta.opens);
            depth += meta.closes;
        }
        false
    }

    pub(super) fn next_keeps_braceless_block(&self, next: Option<&Token>, base: usize) -> bool {
        match next {
            Some(Token::Word(word)) if word == "catch" => true,
            Some(Token::Word(word)) if word == "else" => self.braceless_header_accepts_else(base),
            Some(Token::Word(word)) if word == "while" => self.braceless_header_accepts_while(base),
            _ => false,
        }
    }

    pub(super) fn braceless_header_accepts_while(&self, base: usize) -> bool {
        if self
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
        for line in self.output.iter().rev() {
            let trimmed = line.trim_start();
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

    pub(super) fn braceless_header_accepts_else(&self, base: usize) -> bool {
        if self
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
        for line in self.output.iter().rev() {
            let trimmed = line.trim_start();
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

    pub(super) fn enclosing_if_level(
        &self,
        body_idx: usize,
        body_level: usize,
        default: usize,
    ) -> usize {
        let mut expected = body_level;
        let mut scan = body_idx;
        let mut saw_condition_closer = false;
        while scan > 0 {
            scan -= 1;
            let line = self.output[scan].trim_end();
            if line.is_empty() {
                continue;
            }
            let width = leading_visual_width(line, self.options.tab_width);
            if !width.is_multiple_of(self.options.indent_width) {
                break;
            }
            let level = width / self.options.indent_width;
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
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
            if !trimmed.ends_with(')') && !(is_header && saw_condition_closer) {
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
        self.output.iter().rev().find_map(|line| {
            let code = &line[..trailing_comment_split_limit(line)];
            let trimmed = code.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || is_comment_line(line) {
                return None;
            }
            Some(trimmed == "else" || trimmed.ends_with("} else"))
        }) == Some(true)
    }

    fn record_header_frame(&mut self, word: &str) {
        let attached_closing_indent =
            (word == "else" && self.current.trim_start().starts_with('}')).then(|| {
                self.frame_stack.last_closed_brace().map_or_else(
                    || self.current_line_indent_spaces(),
                    |frame| frame.sibling_indent_column,
                )
            });
        let closed_if_indent = (word == "else")
            .then(|| {
                self.frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.header.as_deref() == Some("if"))
                    .map(|frame| frame.header_indent_column)
            })
            .flatten();
        let open_if_indent = (word == "else")
            .then(|| {
                self.frame_stack
                    .active_header()
                    .filter(|frame| frame.header == "if")
                    .map(|frame| frame.line_indent_spaces)
            })
            .flatten();
        let sequential_after_close_indent = (!is_attachable_closing_header(word)
            && word != "while"
            && self
                .current
                .trim()
                .strip_prefix('}')
                .is_some_and(|rest| rest.trim().is_empty()))
        .then(|| self.state.indent() * self.options.indent_width);
        let starts_output_line = self.token_input.token_begins_source_line
            || (self.current_is_blank() && self.previous_was_newline);
        let keeps_return_continuation_column = self.options.brace_style == BraceStyle::Whitesmith
            && starts_output_line
            && self.output.last_non_empty_line().is_some_and(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with("return ") && code.ends_with(':')
            });
        let pending_line_indent = self
            .current_is_blank()
            .then_some(())
            .and_then(|()| {
                self.continuation_indent
                    .next_line_indent_spaces
                    .or_else(|| {
                        self.continuation_indent
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
            self.frame_stack
                .active_header()
                .filter(|frame| frame.header == "else" && self.active_else_expects_body())
                .map(|frame| frame.body_indent_spaces)
        });
        let inline_header_indent = self
            .inline_nested_header_braceless_bias
            .filter(|_| word == "else" || !self.token_input.token_begins_source_line)
            .map(|level| level * self.options.indent_width);
        let line_indent_spaces = attached_closing_indent
            .or(closed_if_indent)
            .or(open_if_indent)
            .or(inline_header_indent)
            .or(sequential_after_close_indent)
            .or(enclosing_else_body_indent)
            .or(pending_line_indent)
            .unwrap_or_else(|| {
                let current_indent = self.current_line_indent_spaces();
                let current_indent = if starts_output_line
                    && !current_indent.is_multiple_of(self.options.indent_width)
                {
                    current_indent - current_indent % self.options.indent_width
                } else {
                    current_indent
                };
                let structural_indent =
                    current_indent + self.else_if_break_depths.len() * self.options.indent_width;
                self.frame_stack
                    .active_brace()
                    .filter(|frame| frame.semantic_kind == BraceSemanticKind::Command)
                    .map_or(structural_indent, |frame| {
                        structural_indent
                            .max(frame.header_indent_column + self.options.indent_width)
                    })
            });
        let parent_delimiter = self
            .frame_stack
            .active_delimiter_with_id()
            .map(|(id, _)| id);
        self.frame_stack.push_header(HeaderFrame {
            header: word.to_string(),
            line_indent_spaces,
            body_indent_spaces: line_indent_spaces + self.options.indent_width,
            parent_delimiter,
        });
    }

    pub(super) fn update_command_word(&mut self, word: &str, next: Option<&Token>) {
        let word_is_macro_argument = matches!(next, Some(Token::Symbol(',')));
        let objc_header = self
            .current
            .trim_end()
            .ends_with('@')
            .then(|| match word {
                "autoreleasepool" => Some("autoreleasepool"),
                "try" => Some("@try"),
                "catch" => Some("@catch"),
                "finally" => Some("@finally"),
                _ => None,
            })
            .flatten();
        let header = if let Some(header) = objc_header {
            Some(header)
        } else if ((self.is_header(word) && !word_is_macro_argument) || is_asm_block_header(word))
            && self.word_can_be_header_here(word, next)
        {
            Some(word)
        } else {
            None
        };
        if let Some(header) = header {
            self.command_state.current_header = Some(header.to_string());
            self.command_state.header_broken_before_comment = false;
            self.command_state.preprocessor_after_header = false;
            self.record_header_frame(header);
        }
        if let Some(header) = header {
            self.observe_block_spacing_header(header);
        }
        if (language::BLOCK_WORDS.contains(&word) || language::PRE_BLOCK_WORDS.contains(&word))
            && self.stack_state.paren_depth == 0
            && self.block_word_is_recognized(word, next)
            && !(matches!(word, "class" | "struct")
                && self.command_state.pending_block_word.as_deref() == Some("enum"))
        {
            self.command_state.pending_block_word = Some(word.to_string());
        }
        self.command_state.observe_text(word);
    }

    fn word_can_be_header_here(&self, word: &str, next: Option<&Token>) -> bool {
        let current = self.current.trim_end();
        if current.trim_start().starts_with('#') || (word == "else" && current.ends_with('#')) {
            return false;
        }
        if word == "if" {
            let before = current.trim();
            let before = before.strip_prefix('}').map_or(before, str::trim_start);
            if !(before.is_empty()
                || before == "else"
                || self
                    .command_state
                    .current_header
                    .as_deref()
                    .is_some_and(|header| self.is_add_braces_header(header)))
            {
                return false;
            }
            return matches!(
                next,
                Some(Token::Symbol('(') | Token::Newline | Token::Comment(_, _))
            ) || matches!(next, Some(Token::Word(word)) if word == "constexpr");
        }
        if word == "else" && matches!(next, Some(Token::Symbol(','))) {
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
                || self.command_state.previous_non_ws_char == Some('}'));
        }
        if word == "do" {
            return !matches!(next, Some(Token::Operator(operator)) if operator == "::");
        }
        if !matches!(word, "try" | "__try") {
            return true;
        }
        current.is_empty() || current.ends_with('}') || current.ends_with(')')
    }

    pub(super) fn block_word_is_recognized(&self, word: &str, next: Option<&Token>) -> bool {
        match word {
            "module" => {
                self.command_state.previous_non_ws_char != Some(')')
                    && matches!(
                        next,
                        Some(Token::Word(name))
                            if name.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic())
                    )
            }
            "interface" => self.command_state.pending_block_word.is_none(),
            _ => true,
        }
    }

    pub(super) fn push_pre_brace_header(&mut self) -> Option<String> {
        let header = self
            .command_state
            .current_header
            .take()
            .or_else(|| {
                let word = leading_identifier(self.current.trim_start());
                (self.is_header(word)
                    && (language::is_non_paren_header(word)
                        || self.command_state.previous_command_char == Some(')')))
                .then(|| word.to_string())
            })
            .or_else(|| {
                if !self.current_is_blank()
                    || !self
                        .output
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| line.trim_start().starts_with('#'))
                {
                    return None;
                }
                let header = self.frame_stack.active_header()?.header.clone();
                let line = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))?
                    .split("//")
                    .next()
                    .unwrap_or_default()
                    .trim_end();
                if line.ends_with([';', '{', '}'])
                    || !line
                        .split(|ch: char| !is_word_char(ch))
                        .any(|word| word == header)
                {
                    return None;
                }
                Some(header)
            })?;
        self.observe_block_spacing_body_start();
        self.command_state.preprocessor_after_header = false;
        self.command_state
            .pre_brace_header_stack
            .push(header.clone());
        Some(header)
    }

    pub(super) fn preprocessor_interrupted_header_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        indent: usize,
        current_spaces: Option<usize>,
        interrupted_header_context: bool,
    ) -> Option<usize> {
        if !interrupted_header_context
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '}', ':'])
            || self.is_header(leading_identifier(line.trim_start()))
        {
            return None;
        }
        let current_opens_brace = line[..trailing_comment_split_limit(line)]
            .trim_end()
            .ends_with('{');
        let frame = if current_opens_brace {
            self.frame_stack.enclosing_brace()
        } else {
            self.frame_stack.active_brace()
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

    pub(super) fn split_else_condition_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || !line.trim_start().starts_with(')')
            || !line[..trailing_comment_split_limit(line)]
                .trim_end()
                .ends_with('{')
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        self.output.iter().rev().take(16).find_map(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            (unmatched_open_paren_column(code).is_some()
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

    pub(super) fn preprocessor_interrupted_else_if_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("} else if") || trimmed.starts_with("}else if")) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        preprocessor_directive(previous.trim_start())?;
        self.output
            .iter()
            .rev()
            .skip(1)
            .take(32)
            .find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                code.ends_with('{')
                    && (starts_header_word(trimmed, "if")
                        || trimmed.starts_with("} else")
                        || trimmed.starts_with("}else"))
            })
            .map(|header| leading_visual_width(header, self.options.tab_width))
    }

    pub(super) fn split_else_matching_if_indent_spaces(
        &self,
        line: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !line.trim_start().starts_with("else")
            || (self.preprocessor.split_else.extra_indent && !self.preprocessor_split_else_active())
        {
            return None;
        }
        let close_index = (0..self.output.len())
            .rev()
            .find(|index| !self.output[*index].trim().is_empty())?;
        if self.output.code_trimmed(close_index) != "}" {
            return None;
        }
        let mut depth = 0usize;
        for index in (0..close_index).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes;
            if meta.opens > depth {
                let trimmed = self.output.code_trimmed(index);
                let matches_if =
                    starts_header_word(trimmed, "if") || trimmed.starts_with("else if");
                return matches_if.then(|| {
                    current_spaces.unwrap_or(0).max(
                        self.output.lead_width(index, self.options.tab_width)
                            + self.line_adjuster.next_line_case_unindent_depth()
                                * self.options.indent_width,
                    )
                });
            }
            depth = depth.saturating_sub(meta.opens);
        }
        None
    }

    pub(super) fn else_indent_from_previous_if(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trim_start().starts_with("else")
            || self.preprocessor.split_else.extra_indent
        {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(';') {
            return None;
        }
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        let previous_trimmed = previous_code.trim_start();
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
            && !previous.trim_start().starts_with(',')
        {
            return None;
        }
        for line in self.output.iter().rev().skip(1) {
            let trimmed = line.trim_start();
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

    pub(super) fn detached_else_nested_header_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trim_start().starts_with("else")
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
            || !self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim() == "}")
        {
            return None;
        }
        let mut open_indent: Option<usize> = None;
        for previous in self.output.iter().rev().skip(1) {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
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

    pub(super) fn none_style_split_else_closing_header_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || !(trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            || self
                .output
                .last_non_empty_line()
                .is_some_and(|line| preprocessor_directive(line.trim_start()).is_some())
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let mut result = current_spaces;
        if let Some(previous) = self.output.last_non_empty_line() {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous.trim() == "}"
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
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = candidate[..trailing_comment_split_limit(candidate)].trim_end();
            let (closes, opens) = line_brace_imbalance(code);
            depth += closes;
            if opens >= depth {
                matching_open = Some(candidate);
                break;
            }
            depth = depth.saturating_sub(opens);
        }
        let matching_open = matching_open?;
        let matching_code = matching_open[..trailing_comment_split_limit(matching_open)].trim_end();
        let matching_trimmed = matching_code.trim_start();
        let target = if matching_trimmed.starts_with(')') && matching_code.ends_with('{') {
            self.output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != matching_open.as_str())
                .skip(1)
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    (unmatched_open_paren_column(code).is_some()
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

    pub(super) fn preprocessor_interrupted_closing_header_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_directive = preprocessor_directive(previous_code.trim_start());
        let mut result = None;
        if previous_directive.is_some()
            && let Some(header) = self.output.iter().rev().skip(1).take(32).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                trimmed.starts_with("} else") || trimmed.starts_with("}else")
            })
        {
            result = Some(leading_visual_width(header, self.options.tab_width));
        }
        if previous_directive == Some("endif") {
            let mut depth = 1usize;
            for candidate in self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty())
            {
                let code = candidate[..trailing_comment_split_limit(candidate)].trim_end();
                let (closes, opens) = line_brace_imbalance(code);
                if depth == 1 && opens > 0 && closes > 0 && code.trim_start().starts_with("} else")
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
        if split_else_context && previous_code.trim() == "}" {
            result = Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width),
            );
        }
        if split_else_context
            && previous_code.ends_with(';')
            && !previous_code.ends_with("};")
            && !previous_code.trim_start().starts_with('(')
            && !self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    code.ends_with(',') || unmatched_open_paren_column(code).is_some()
                })
        {
            result = Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width),
            );
        }
        result
    }

    pub(super) fn split_else_endif_sibling_indent_spaces(
        &self,
        line: &str,
        split_else_output_context: bool,
    ) -> Option<usize> {
        if !split_else_output_context || line.trim_start().starts_with(['#', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if preprocessor_directive(previous.trim_start()) != Some("endif") {
            return None;
        }
        let sibling = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })?;
        let sibling_trimmed = sibling[..trailing_comment_split_limit(sibling)]
            .trim_end()
            .trim_start();
        if sibling_trimmed != "} else" && sibling_trimmed != "}else" {
            return None;
        }
        self.output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != sibling.as_str())
            .skip(1)
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
            .map(|line| leading_visual_width(line, self.options.tab_width))
    }

    pub(super) fn structural_split_else_closing_header_indent_spaces(
        &self,
        line: &str,
        current_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let (open_spaces, _, _) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let closing_multiline_header_indent = self.current_closing_multiline_header_indent();
        let open_spaces = closing_multiline_header_indent.unwrap_or(open_spaces);
        let split_else_chain = structural_split_else_chain
            || self
                .output
                .iter()
                .rev()
                .take(128)
                .any(|line| line.trim() == "else" || line.trim_end().ends_with("} else"));
        let recent_adjacent_string_call = self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.ends_with(");") && starts_string_literal_token(code.trim_start())
        }) && self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            unmatched_open_paren_column(code).is_some()
                && !starts_string_literal_token(code.trim_start())
                && !code.ends_with(';')
        });
        if split_else_chain
            && closing_multiline_header_indent.is_some()
            && previous_code.ends_with(';')
            && current_spaces != open_spaces
            || recent_adjacent_string_call
                && previous_code.ends_with(");")
                && starts_string_literal_token(previous_code.trim_start())
                && current_spaces != open_spaces
            || preprocessor_directive(previous_code.trim_start()) == Some("endif")
                && current_spaces != open_spaces
            || structural_split_else_chain
                && (previous_code.ends_with(';') || previous_code.trim() == "}")
                && (current_spaces < open_spaces || closing_multiline_header_indent.is_some())
            || split_else_chain
                && (previous_code.ends_with(';') || previous_code.trim() == "}")
                && current_spaces > open_spaces
            || split_else_chain && previous_code.trim() == "}" && !line.contains('{')
        {
            return Some(open_spaces);
        }
        None
    }

    pub(super) fn preprocessor_closing_header_indent_spaces(
        &self,
        line: &str,
        normal_indent: usize,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("} else") || trimmed.starts_with("}else")) {
            return None;
        }
        let mut result = current_spaces;
        let split_else_inactive =
            !self.preprocessor.split_else.extra_indent && !self.preprocessor_split_else_active();
        if split_else_inactive && let Some(previous) = self.output.last_non_empty_line() {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if preprocessor_directive(previous_code.trim_start())
                .is_some_and(is_conditional_preprocessor)
            {
                let target = normal_indent * self.options.indent_width;
                if result.unwrap_or(output_spaces) > target {
                    result = Some(target);
                }
            }
        }
        if (trimmed.starts_with("} else if") || trimmed.starts_with("}else if"))
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with('{')
            && let Some((open_spaces, _, open)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && (open.trim_start().starts_with("} else") || open.trim_start().starts_with("}else"))
            && result.unwrap_or(output_spaces) != open_spaces
        {
            result = Some(open_spaces);
        }
        if result.is_none()
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
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

    pub(super) fn closing_header_body_brace_indent_spaces(
        &self,
        line: &str,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        if current_spaces.is_some() || line.trim() != "{" {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with("} else") {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (target > output_spaces).then_some(target)
    }

    pub(super) fn recent_split_else_if_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        recent_split_else_chain: bool,
    ) -> Option<usize> {
        if !recent_split_else_chain
            || line_kind != LineKind::Normal
            || !line.trim_start().starts_with("else if")
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim();
        if previous_code != "}" {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces.unwrap_or(usize::MAX) > target).then_some(target)
    }

    pub(super) fn match_closing_while_to_braceless_do(&mut self) {
        while let Some((base, delta)) = self.state.last_braceless_block()
            && self.state.indent() == base + delta
            && !self.braceless_header_accepts_while(base)
        {
            self.state.exit_braceless_block();
        }
        if let Some((base, delta)) = self.state.last_braceless_block()
            && self.state.indent() == base + delta
            && self.braceless_header_accepts_while(base)
        {
            self.continuation_indent.next_line_indent =
                Some(base + self.line_adjuster.total_case_unindent_depth());
            self.continuation_indent.next_line_indent_spaces = None;
            self.state.exit_braceless_block();
            if self
                .frame_stack
                .active_braceless_header()
                .is_some_and(|frame| frame.header == "do")
            {
                self.frame_stack.pop_braceless_header();
            }
            return;
        }
        let mut pending_whiles = 0usize;
        for line in self.output.iter().rev() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            let code = trimmed[..trailing_comment_split_limit(trimmed)].trim_end();
            if code.ends_with('{') || code.ends_with('}') || code.ends_with(')') {
                return;
            }
            let (do_count, rest) = leading_do_headers(code);
            if do_count > 0 && (rest.is_empty() || code.ends_with(';')) {
                let net = do_count.saturating_sub(inline_while_closers(rest));
                if net > pending_whiles {
                    let level = leading_visual_width(line, self.options.tab_width)
                        / self.options.indent_width;
                    self.continuation_indent.next_line_indent = Some(
                        level + net - pending_whiles - 1
                            + self.line_adjuster.total_case_unindent_depth(),
                    );
                    self.continuation_indent.next_line_indent_spaces = None;
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
        rest = after.trim_start();
    }
    (count, rest)
}

fn inline_while_closers(rest: &str) -> usize {
    rest.split(';')
        .skip(1)
        .filter(|segment| header_word_is(segment.trim_start(), "while"))
        .count()
}

pub(super) fn is_attachable_closing_header(word: &str) -> bool {
    matches!(
        word,
        "else" | "catch" | "@catch" | "@finally" | "__finally" | "__except"
    ) || word.ends_with("CATCH")
}

fn header_word_is(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|rest| matches!(rest.chars().next(), Some('(') | Some(' ')))
}
