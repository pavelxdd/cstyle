use super::FormatEngine;

use super::brace_classification::{
    is_lambda_body_header, is_lambda_capture_header, line_opens_lambda_block,
};
use super::columns::{leading_visual_width, visual_width_from};
use super::compound_literals::line_ends_compound_literal_cast;
use super::frame::CommaRole;
use super::headers::{line_is_control_body_header, starts_header_word};
use super::indentation::LineKind;

use super::language;
use super::language::is_macro_like_word;
use super::line_scan::{is_comment_line, is_comment_only_line};
use super::line_scan::{
    line_brace_imbalance, line_has_brace, line_paren_imbalance, reverse_scan_skips_block_comment,
    trailing_comment_split_limit, unmatched_open_paren_column, unmatched_open_paren_columns,
};
use super::literals::{first_string_literal_start, starts_string_literal_token};
use super::operators::{find_assignment_operator, starts_ternary_arm, starts_with_chain_operator};
use super::preprocessor::output_has_active_preprocessor_branch;

use crate::config::{BraceStyle, MinConditionalIndent};
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(super) struct SplitElseCallLineLayout {
    pub(super) indent_spaces: usize,
    pub(super) clear_continuation_after_line: Option<usize>,
}

pub(super) fn assignment_call_value_column(line: &str, tab_width: usize) -> Option<usize> {
    if !line.trim_end().ends_with('(') {
        return None;
    }
    let (assignment, operator) = find_assignment_operator(line)?;
    let after_operator = assignment + operator.len();
    let value_start = line[after_operator..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(line.len(), |(offset, _)| after_operator + offset);
    Some(visual_width_from(&line[..value_start], 0, tab_width))
}

pub(super) fn casted_assignment_value_column(line: &str, tab_width: usize) -> Option<usize> {
    if !line.trim_end().ends_with(',') || unmatched_open_paren_column(line).is_none() {
        return None;
    }
    let (assignment, operator) = find_assignment_operator(line)?;
    let after_operator = assignment + operator.len();
    let value_start = line[after_operator..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(line.len(), |(offset, _)| after_operator + offset);
    if !line[value_start..].starts_with('(') {
        return None;
    }
    let mut depth = 0usize;
    for (offset, ch) in line[value_start..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let after_cast = value_start + offset + ch.len_utf8();
                    let rest = line[after_cast..].trim_start();
                    return (rest.chars().next().is_some_and(is_identifier_start)
                        && rest.contains('('))
                    .then(|| visual_width_from(&line[..value_start], 0, tab_width));
                }
            }
            _ => {}
        }
    }
    None
}

fn line_starts_call_expression(line: &str) -> bool {
    let word_end = line
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(line.len());
    word_end > 0 && line[word_end..].trim_start().starts_with('(')
}

fn has_top_level_comma_text(line: &str) -> bool {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for ch in line.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn simple_trailing_open_paren_line(line: &str) -> bool {
    let Some(before) = line.trim_end().strip_suffix('(') else {
        return false;
    };
    let before = before.trim_end();
    if before.is_empty() || before.contains('=') || before.ends_with([')', ']']) {
        return false;
    }
    let name = before
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' || ch == '~'))
        .next()
        .unwrap_or_default();
    name.chars()
        .next()
        .is_some_and(|ch| ch == '_' || ch == '~' || ch.is_ascii_alphabetic())
}

pub(super) fn callee_name_start_before_open(line: &str, open_column: usize) -> Option<usize> {
    let chars = line.chars().collect::<Vec<_>>();
    if open_column == 0 || open_column > chars.len() {
        return None;
    }
    let mut end = open_column;
    while end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        let ch = chars[start - 1];
        if ch == '_' || ch.is_ascii_alphanumeric() {
            start -= 1;
        } else {
            break;
        }
    }
    (start < end).then_some(start)
}

pub(super) fn closing_braced_call_argument_indent_spaces(
    line: &str,
    output: &[String],
    tab_width: usize,
) -> Option<usize> {
    if !line.trim_start().starts_with("})") {
        return None;
    }
    for previous in output.iter().rev().take(16) {
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let trimmed = code.trim();
        if trimmed.ends_with(';') || trimmed == "{" || trimmed == "}" {
            break;
        }
        if let Some(comma) = code.rfind(", {") {
            return Some(visual_width_from(&code[..comma + 2], 0, tab_width));
        }
    }
    None
}

pub(super) fn plain_call_opener_indent_for_closing_line(
    output: &[String],
    tab_width: usize,
) -> Option<usize> {
    let mut balance = 1isize;
    for line in output.iter().rev() {
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        if code.trim().is_empty() {
            continue;
        }
        for (index, ch) in code.char_indices().rev() {
            match ch {
                ')' => balance += 1,
                '(' => {
                    balance -= 1;
                    if balance == 0 {
                        let before = code[..index].trim_start();
                        let after = code[index + ch.len_utf8()..].trim();
                        return (after.is_empty()
                            && (before.is_empty() || !before.chars().any(char::is_whitespace)))
                        .then(|| leading_visual_width(line, tab_width));
                    }
                }
                _ => {}
            }
        }
    }
    None
}

impl FormatEngine<'_> {
    pub(super) fn split_else_string_comma_argument_layout(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<SplitElseCallLineLayout> {
        if !split_else_context
            || line.trim_start().starts_with(['#', '{', '}', ')'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !(previous_code.ends_with(',') || line.trim_start().starts_with(','))
            || !starts_string_literal_token(previous_code.trim_start())
        {
            return None;
        }
        if let Some(value_column) = self.output.iter().rev().skip(1).take(8).find_map(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            assignment_call_value_column(code, self.options.tab_width)
        }) {
            return Some(SplitElseCallLineLayout {
                indent_spaces: value_column + self.options.indent_width,
                clear_continuation_after_line: None,
            });
        }
        let clear_continuation_after_line = if line.trim_end().ends_with(");") {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                    .then_some(leading_visual_width(line, self.options.tab_width))
            })
        } else {
            None
        };
        Some(SplitElseCallLineLayout {
            indent_spaces: leading_visual_width(previous, self.options.tab_width),
            clear_continuation_after_line,
        })
    }

    pub(super) fn split_else_comma_argument_layout(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<SplitElseCallLineLayout> {
        if !split_else_context
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let open = previous_code
            .ends_with(',')
            .then(|| unmatched_open_paren_column(previous_code))??;
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        Some(SplitElseCallLineLayout {
            indent_spaces: (open + 1).max(previous_spaces),
            clear_continuation_after_line: line
                .trim_end()
                .ends_with(");")
                .then_some(previous_spaces),
        })
    }

    pub(super) fn none_style_split_else_comma_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_state_active: bool,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let open = previous_code
            .ends_with(',')
            .then(|| unmatched_open_paren_column(previous_code))??;
        Some((open + 1).max(leading_visual_width(previous, self.options.tab_width)))
    }

    pub(super) fn structural_split_else_string_comma_indent_spaces(
        &self,
        line: &str,
        structural_split_else_context: bool,
    ) -> Option<usize> {
        if !structural_split_else_context
            || starts_string_literal_token(line.trim_start())
            || line.trim_start().starts_with(['#', '{', '}', ')'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        ((previous_code.ends_with(',') || line.trim_start().starts_with(','))
            && starts_string_literal_token(previous_code.trim_start()))
        .then_some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn split_else_case_comma_argument_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || line.trim_start().starts_with(['{', '}', '#']) {
            return None;
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if case_unindent == 0 {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code
            .ends_with(',')
            .then(|| unmatched_open_paren_column(previous_code))?
            .map(|open| open + 1 + case_unindent)
    }

    pub(super) fn split_else_case_call_sibling_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || line.trim_start().starts_with(['{', '}', '#']) {
            return None;
        }
        if let Some(spaces) =
            self.split_else_case_comma_argument_indent_spaces(line, split_else_context)
        {
            return Some(spaces);
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if case_unindent == 0 {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code.trim_start().starts_with(");").then(|| {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                    .then(|| leading_visual_width(line, self.options.tab_width) + case_unindent)
            })
        })?
    }

    pub(super) fn split_else_comma_sibling_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || line.trim_start().starts_with(['#', '{', '}', ')']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        unmatched_open_paren_column(previous_code)
            .map(|open| open + 1)
            .or_else(|| {
                self.output
                    .iter()
                    .rev()
                    .skip(1)
                    .take(8)
                    .any(|line| {
                        unmatched_open_paren_column(&line[..trailing_comment_split_limit(line)])
                            .is_some()
                    })
                    .then_some(leading_visual_width(previous, self.options.tab_width))
            })
    }

    pub(super) fn split_else_call_closing_layout(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<SplitElseCallLineLayout> {
        if !split_else_context || !line.trim_start().starts_with(')') {
            return None;
        }
        let call = if line.trim_start().starts_with(");")
            && self
                .output
                .iter()
                .rev()
                .take(8)
                .any(|line| starts_string_literal_token(line.trim_start()))
        {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                if starts_string_literal_token(code.trim_start()) {
                    return None;
                }
                unmatched_open_paren_columns(code)
                    .last()
                    .copied()
                    .map(|open| {
                        (
                            leading_visual_width(line, self.options.tab_width),
                            assignment_call_value_column(code, self.options.tab_width)
                                .unwrap_or(open),
                        )
                    })
            })
        } else {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                assignment_call_value_column(code, self.options.tab_width).map(|value_column| {
                    (
                        leading_visual_width(line, self.options.tab_width),
                        value_column,
                    )
                })
            })
        }?;
        Some(SplitElseCallLineLayout {
            indent_spaces: call.1,
            clear_continuation_after_line: line.trim_end().ends_with(';').then_some(call.0),
        })
    }

    pub(super) fn split_else_following_assignment_call_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context
            || line.trim_start().starts_with(['#', '{', '}', ')'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code.trim_start().starts_with(");").then(|| {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                assignment_call_value_column(code, self.options.tab_width)
                    .map(|_| leading_visual_width(line, self.options.tab_width))
            })
        })?
    }

    pub(super) fn split_else_following_call_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context
            || line.trim_start().starts_with(['#', '{', '}', ')'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(");") {
            return None;
        }
        if starts_string_literal_token(previous_code.trim_start()) {
            return self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                    .then_some(leading_visual_width(line, self.options.tab_width))
            });
        }
        let call_start = self
            .output
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        let call_start_code = call_start[..trailing_comment_split_limit(call_start)].trim_end();
        if call_start_code.ends_with(',') && unmatched_open_paren_column(call_start_code).is_some()
        {
            return Some(leading_visual_width(call_start, self.options.tab_width));
        }
        (self
            .output
            .iter()
            .rev()
            .skip(1)
            .take(8)
            .any(|line| starts_string_literal_token(line.trim_start())))
        .then(|| {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                    .then_some(leading_visual_width(line, self.options.tab_width))
            })
        })?
    }

    pub(super) fn structural_split_else_string_call_close_indent_spaces(
        &self,
        line: &str,
        structural_split_else_context: bool,
    ) -> Option<usize> {
        if !structural_split_else_context || !line.trim_start().starts_with(')') {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !starts_string_literal_token(previous_code.trim_start()) {
            return None;
        }
        let call_start = self.output.iter().rev().skip(1).take(16).find(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            !unmatched_open_paren_columns(code).is_empty()
                && !starts_string_literal_token(code.trim_start())
        })?;
        let call_code = call_start[..trailing_comment_split_limit(call_start)].trim_end();
        assignment_call_value_column(call_code, self.options.tab_width)
            .is_none()
            .then(|| leading_visual_width(previous, self.options.tab_width).saturating_sub(1))
    }

    pub(super) fn structural_split_else_following_string_call_indent_spaces(
        &self,
        line: &str,
        structural_split_else_context: bool,
    ) -> Option<usize> {
        if !structural_split_else_context
            || line.trim_start().starts_with(['#', '{', '}', ')'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(");")
            || !starts_string_literal_token(previous_code.trim_start())
        {
            return None;
        }
        self.output.iter().rev().skip(1).take(16).find_map(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            (!unmatched_open_paren_columns(code).is_empty()
                && !starts_string_literal_token(code.trim_start()))
            .then_some(leading_visual_width(line, self.options.tab_width))
        })
    }

    pub(super) fn split_else_adjacent_string_call_close_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !line.trim_start().starts_with(");") || !self.recent_split_else_call_region_active() {
            return None;
        }
        let mut saw_string = false;
        for previous in self.output.iter().rev().take(16) {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            if starts_string_literal_token(trimmed) {
                saw_string = true;
                continue;
            }
            if let Some(open) = unmatched_open_paren_columns(code).last().copied() {
                if !saw_string {
                    return None;
                }
                let adjacent_strings = self
                    .output
                    .iter()
                    .rev()
                    .take(8)
                    .filter(|line| starts_string_literal_token(line.trim_start()))
                    .count()
                    > 1
                    || self
                        .output
                        .iter()
                        .rev()
                        .take(4)
                        .any(|line| line.trim_start().starts_with(','));
                return Some(
                    if code.trim_end().ends_with('(')
                        && adjacent_strings
                        && assignment_call_value_column(code, self.options.tab_width).is_none()
                    {
                        leading_visual_width(previous, self.options.tab_width)
                            + self
                                .line_adjuster
                                .total_case_unindent_depth()
                                .max(self.line_adjuster.next_line_case_unindent_depth())
                                * self.options.indent_width
                    } else {
                        assignment_call_value_column(code, self.options.tab_width).unwrap_or(open)
                            + self.adjusted_line_indent_delta(previous)
                    },
                );
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
        }
        None
    }

    pub(super) fn string_call_continuation_layout(
        &self,
        line: &str,
    ) -> Option<SplitElseCallLineLayout> {
        let current = line.trim_start();
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous.trim_start();
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        if starts_string_literal_token(current)
            && (is_comment_line(previous_trimmed) || is_comment_only_line(previous_trimmed))
            || starts_string_literal_token(current)
                && starts_string_literal_token(previous_code.trim_start())
        {
            return Some(SplitElseCallLineLayout {
                indent_spaces: previous_spaces,
                clear_continuation_after_line: None,
            });
        }
        if current.starts_with(',') && starts_string_literal_token(previous_code.trim_start()) {
            let clear_continuation_after_line = line.trim_end().ends_with(';').then(|| {
                self.output
                    .iter()
                    .rev()
                    .skip(1)
                    .take(128)
                    .find_map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        (!starts_string_literal_token(code.trim_start())
                            && !unmatched_open_paren_columns(code).is_empty())
                        .then_some(leading_visual_width(line, self.options.tab_width))
                    })
                    .unwrap_or(previous_spaces.saturating_sub(self.options.indent_width))
            });
            return Some(SplitElseCallLineLayout {
                indent_spaces: previous_spaces,
                clear_continuation_after_line,
            });
        }
        if previous_code.trim_start().starts_with(',')
            && previous_code.ends_with(");")
            && !current.starts_with(['#', '{', '}', ')'])
            && !starts_string_literal_token(current)
            && !is_comment_line(current)
        {
            let indent_spaces = self
                .output
                .iter()
                .rev()
                .skip(1)
                .take(128)
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    (!starts_string_literal_token(code.trim_start())
                        && !unmatched_open_paren_columns(code).is_empty())
                    .then_some(leading_visual_width(line, self.options.tab_width))
                })
                .unwrap_or(previous_spaces.saturating_sub(self.options.indent_width));
            return Some(SplitElseCallLineLayout {
                indent_spaces,
                clear_continuation_after_line: None,
            });
        }
        None
    }

    pub(super) fn string_call_closing_indent_spaces(
        &self,
        line: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !line.trim_start().starts_with(");") {
            return None;
        }
        let case_unindent = self
            .line_adjuster
            .total_case_unindent_depth()
            .max(self.line_adjuster.next_line_case_unindent_depth())
            * self.options.indent_width;
        let mut result = None;
        if !self.options.indent_cases
            && self
                .stack_state
                .brace_header_stack
                .iter()
                .any(|header| header.as_deref() == Some("case"))
            && self
                .output
                .iter()
                .rev()
                .take(8)
                .any(|line| starts_string_literal_token(line.trim_start()))
            && let Some((call_line, open)) = self.output.iter().rev().take(16).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                if starts_string_literal_token(code.trim_start()) {
                    return None;
                }
                unmatched_open_paren_columns(code)
                    .last()
                    .copied()
                    .map(|open| (line, open))
            })
        {
            let call_code = call_line[..trailing_comment_split_limit(call_line)].trim_end();
            let target = if call_code.ends_with('(') {
                leading_visual_width(call_line, self.options.tab_width) + case_unindent
            } else {
                open + self
                    .adjusted_line_indent_delta(call_line)
                    .max(case_unindent)
            };
            if current_spaces.unwrap_or(0) < target {
                result = Some(target);
            }
        }
        if (self
            .output
            .iter()
            .rev()
            .take(8)
            .filter(|line| starts_string_literal_token(line.trim_start()))
            .count()
            > 1
            || self
                .output
                .iter()
                .rev()
                .take(4)
                .any(|line| line.trim_start().starts_with(',')))
            && let Some(call_line) = self.output.iter().rev().take(16).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.ends_with('(')
                    && !starts_string_literal_token(code.trim_start())
                    && assignment_call_value_column(code, self.options.tab_width).is_none()
            })
        {
            result = Some(leading_visual_width(call_line, self.options.tab_width) + case_unindent);
        }
        result
    }

    pub(super) fn string_argument_after_comma_indent_floor(
        &self,
        line: &str,
        current_spaces: usize,
    ) -> Option<usize> {
        if !starts_string_literal_token(line.trim_start()) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let open = previous_code
            .ends_with(',')
            .then(|| unmatched_open_paren_column(previous_code))??;
        let target = open + 1 + self.adjusted_line_indent_delta(previous);
        (current_spaces < target).then_some(target)
    }

    pub(super) fn active_split_else_comma_argument_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if !self.split_else_body_indent_active()
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '(', ')', '{', '}'])
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let open = previous_code
            .ends_with(',')
            .then(|| unmatched_open_paren_column(previous_code))??;
        Some(open + 1 + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width)
    }

    pub(super) fn active_split_else_comma_and_string_indent_floor(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.split_else_body_indent_active()
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let split = trailing_comment_split_limit(previous);
        let previous_code = previous[..split].trim_end();
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let target = if previous_code.ends_with(',')
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            (if open < previous_indent {
                previous_indent + open + 1
            } else {
                open + 1
            }) + case_unindent
        } else if split < previous.len()
            && previous.trim_end().ends_with(',')
            && let Some(open) = unmatched_open_paren_column(previous)
        {
            open + 1 + case_unindent
        } else if previous_code.ends_with(',')
            && starts_string_literal_token(previous_code.trim_start())
        {
            leading_visual_width(previous, self.options.tab_width) + case_unindent
        } else if !starts_with_chain_operator(line.trim_start())
            && first_string_literal_start(line.trim_start()).is_some()
            && first_string_literal_start(previous_code.trim_start()).is_some()
            && let Some(open) = self
                .output
                .iter()
                .rev()
                .take_while(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    !(code.ends_with(';') || code.ends_with('{') || code.ends_with('}'))
                })
                .take(16)
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    (first_string_literal_start(code.trim_start()).is_some())
                        .then(|| unmatched_open_paren_column(code))?
                })
        {
            open + 1 + case_unindent
        } else if starts_string_literal_token(line.trim_start())
            && previous_code.trim_end().ends_with('(')
        {
            leading_visual_width(previous, self.options.tab_width)
                + self.options.indent_width
                + case_unindent
        } else if starts_string_literal_token(line.trim_start())
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            (if open < previous_indent {
                previous_indent + open + 1
            } else {
                open + 1
            }) + case_unindent
        } else if (starts_string_literal_token(line.trim_start())
            || line.trim_start().starts_with(','))
            && starts_string_literal_token(previous_code.trim_start())
        {
            leading_visual_width(previous, self.options.tab_width) + case_unindent
        } else {
            return None;
        };
        (current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(super) fn split_else_adjacent_string_argument_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
        structural_context: bool,
    ) -> Option<usize> {
        if !split_else_context || !starts_string_literal_token(line.trim_start()) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if is_comment_line(previous_code.trim_start()) {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if structural_context {
            return self.output.iter().rev().take(16).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (!starts_string_literal_token(code.trim_start())).then(|| {
                    assignment_call_value_column(code, self.options.tab_width)
                        .map(|column| column + self.options.indent_width)
                        .or_else(|| {
                            code.trim_end().ends_with('(').then(|| {
                                leading_visual_width(line, self.options.tab_width)
                                    + self.options.indent_width
                            })
                        })
                        .or_else(|| {
                            unmatched_open_paren_columns(code)
                                .last()
                                .map(|open| open + 1)
                        })
                        .or_else(|| {
                            code.ends_with('=').then(|| {
                                leading_visual_width(line, self.options.tab_width)
                                    + self.options.indent_width
                            })
                        })
                })?
            });
        }
        if let Some(open) = unmatched_open_paren_columns(previous_code).last().copied() {
            return Some(open + 1);
        }
        if starts_string_literal_token(previous_code.trim_start())
            && let Some(open) = self.output.iter().rev().skip(1).take(16).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (!starts_string_literal_token(code.trim_start()))
                    .then(|| unmatched_open_paren_columns(code).last().copied())?
            })
        {
            return Some(open + 1);
        }
        assignment_call_value_column(previous_code, self.options.tab_width)
            .or_else(|| {
                starts_string_literal_token(previous_code.trim_start()).then(|| {
                    self.output.iter().rev().skip(1).take(8).find_map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        assignment_call_value_column(code, self.options.tab_width)
                    })
                })?
            })
            .map(|column| column + self.options.indent_width)
    }

    pub(super) fn previous_call_argument_sibling_indent(&self, line: &str) -> Option<usize> {
        if !self.options.break_after_logical
            || self.in_initializer_brace()
            || self.output_has_open_initializer_brace()
            || self.in_aggregate_declaration_brace()
            || self.current_inline_array_column().is_some()
        {
            return None;
        }
        let trimmed_end = line.trim_end();
        let closes_enclosing_call = line_paren_imbalance(trimmed_end).0 > 0;
        if self.stack_state.paren_depth == 0 && !closes_enclosing_call {
            return None;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with(['}', ')', '.', '['])
            || starts_with_chain_operator(trimmed)
            || starts_ternary_arm(trimmed)
            || trimmed_end.ends_with('{')
        {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') || !line_paren_imbalance(previous_code).1.is_empty() {
            return None;
        }
        if !self.call_arguments_contain_brace_block() {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    fn call_arguments_contain_brace_block(&self) -> bool {
        for index in (0..self.output.len()).rev().take(64) {
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            if trimmed.is_empty() {
                continue;
            }
            if line_has_brace(code) {
                return true;
            }
            if code.ends_with(';') || !line_paren_imbalance(code).1.is_empty() {
                return false;
            }
        }
        false
    }

    pub(super) fn call_argument_source_indent(
        &self,
        trimmed: &str,
        current_spaces: usize,
        output_source: usize,
        source: usize,
    ) -> Option<usize> {
        if source == 0 || !self.options.break_after_logical {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') || trimmed.starts_with(['}', ')']) {
            return None;
        }
        let source_is_close_to_current =
            output_source < current_spaces && current_spaces.saturating_sub(output_source) <= 1;
        if source_is_close_to_current || line_starts_call_expression(trimmed) {
            return Some(output_source);
        }
        None
    }

    pub(super) fn call_shaped_brace_body_indent_floor(
        &self,
        line: &str,
        normal_indent: usize,
    ) -> Option<usize> {
        if line.trim_start().starts_with(['{', '}', '#']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        (code.ends_with('{')
            && line_starts_call_expression(code.trim_start())
            && previous_indent < normal_indent * self.options.indent_width)
            .then_some(previous_indent + self.options.indent_width)
    }

    pub(super) fn macro_call_continuation_output_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed_line = line.trim_start();
        if trimmed_line.is_empty()
            || trimmed_line.starts_with(['#', '{', '}', ')'])
            || trimmed_line.starts_with("&&")
            || trimmed_line.starts_with("||")
        {
            return None;
        }
        if trimmed_line
            .trim_end()
            .strip_suffix('{')
            .is_some_and(|prefix| line_ends_compound_literal_cast(prefix.trim_end()))
            && self.output.last_non_empty_line().is_some_and(|previous| {
                previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with(',')
            })
        {
            return None;
        }
        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self.output.iter().rev().take(8) {
            let trimmed = previous.trim_end();
            if reverse_scan_skips_block_comment(trimmed, &mut in_block_comment) {
                continue;
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
            let (closes, mut opens) = line_paren_imbalance(trimmed);
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if opens.is_empty() {
                continue;
            }
            let is_macro_paren = unmatched_open_paren_column(trimmed).is_some_and(|column| {
                opens.last() == Some(&column)
                    && trimmed.find('(') == Some(column)
                    && is_macro_like_word(trimmed[..column].trim())
            });
            if !is_macro_paren {
                return None;
            }
            let column = unmatched_open_paren_column(trimmed)?;
            let padding = trimmed
                .chars()
                .skip(column + 1)
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            let padding_width = visual_width_from(&padding, column + 1, self.options.tab_width);
            let spaces = column + 1 + padding_width + self.adjusted_line_indent_delta(previous);
            return (spaces <= self.options.max_continuation_indent).then_some(spaces);
        }
        None
    }

    pub(super) fn enclosing_macro_call_output_context(&self) -> bool {
        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self.output.iter().rev().take(8) {
            let trimmed = previous.trim_end();
            if reverse_scan_skips_block_comment(trimmed, &mut in_block_comment) {
                continue;
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
            let (closes, mut opens) = line_paren_imbalance(trimmed);
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if opens.into_iter().rev().any(|column| {
                trimmed.find('(') == Some(column) && is_macro_like_word(trimmed[..column].trim())
            }) {
                return true;
            }
        }
        false
    }

    pub(super) fn immediate_macro_call_opener_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}'])
            || !self.enclosing_macro_call_output_context()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous.trim_end();
        (simple_trailing_open_paren_line(previous_code)
            && !line_paren_imbalance(previous_code).1.is_empty())
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn macro_call_sibling_fallback_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}'])
            || !self.enclosing_macro_call_output_context()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        let has_split_opener = self
            .output
            .iter()
            .rev()
            .skip(1)
            .take_while(|line| {
                let code = line.trim_end();
                !(code.ends_with(';') || code == "{" || code == "}")
            })
            .any(|line| {
                let code = line.trim_end();
                simple_trailing_open_paren_line(code) && !line_paren_imbalance(code).1.is_empty()
            });
        has_split_opener.then(|| {
            leading_visual_width(previous, self.options.tab_width)
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        })
    }

    pub(super) fn split_call_opening_paren_indent_spaces(&self, line: &str) -> Option<usize> {
        if let Some(spaces) = self.split_call_opening_paren_frame_indent_spaces(line) {
            return Some(spaces);
        }
        if line.trim() != "(" {
            return None;
        }
        let previous = self
            .previous_pre_adjust_line
            .as_ref()
            .filter(|line| !line.trim().is_empty())
            .or_else(|| {
                self.output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
            })?;
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
            return None;
        }
        let first_word = code
            .trim_start()
            .split(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty())?;
        if language::is_header(first_word) {
            return None;
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let leading = leading_visual_width(previous, self.options.tab_width);
        if code.trim_start().starts_with("return new ") {
            return Some(leading + "return ".len() + case_unindent);
        }
        if (code.contains(" new ") && find_assignment_operator(code).is_some())
            || code.trim_start().starts_with("new ")
        {
            return Some(leading + case_unindent);
        }
        unmatched_open_paren_column(code).map(|column| column + 1)
    }

    fn split_call_opening_paren_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        if line.trim() != "(" {
            return None;
        }
        let delimiter = self.frame_stack.enclosing_delimiter()?;
        let call = delimiter.call.as_ref()?;
        call.first_argument_column
            .or_else(|| Some(delimiter.opener_output_column + 1))
    }

    pub(super) fn split_call_closing_paren_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with(')') {
            return None;
        }
        let mut close_pending = 0usize;
        let mut brace_depth = 0usize;
        let mut in_block_comment = false;
        for previous in self.output.iter().rev().take(32) {
            let trimmed = previous.trim_end();
            if reverse_scan_skips_block_comment(trimmed, &mut in_block_comment) {
                continue;
            }
            let (brace_closes, brace_opens) = line_brace_imbalance(trimmed);
            brace_depth += brace_closes;
            if brace_depth > 0 {
                brace_depth = brace_depth.saturating_sub(brace_opens);
                continue;
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
            let (closes, mut opens) = line_paren_imbalance(trimmed);
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if opens.is_empty() {
                continue;
            }
            if opens.len() != 1 {
                return None;
            }
            let code = trimmed[..trailing_comment_split_limit(trimmed)].trim_end();
            if !code.ends_with('(') {
                return None;
            }
            let leading = leading_visual_width(previous, self.options.tab_width);
            let body = code.trim_start();
            if body.starts_with("return ") {
                return Some(leading + "return ".len());
            }
            if body == "(" {
                return Some(
                    leading
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            }
            if code.contains(" new ") && find_assignment_operator(code).is_some() {
                return Some(leading);
            }
            return None;
        }
        None
    }

    pub(super) fn active_split_else_call_argument_fallback_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
            || !self.preprocessor_split_else_active()
            || self.output.last_non_empty_line().is_none()
        {
            return None;
        }
        self.recent_call_argument_indent_spaces()
    }

    pub(super) fn restore_split_else_call_argument_indent_after_emission(
        &mut self,
        line: &str,
        line_kind: LineKind,
    ) {
        if line_kind != LineKind::Normal
            || !line.trim_end().ends_with(");")
            || !self.recent_split_else_preprocessor_region_active()
        {
            return;
        }
        let Some(spaces) = self
            .output
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())
            .and_then(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                    .then_some(leading_visual_width(previous, self.options.tab_width))
            })
        else {
            return;
        };
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = Some(spaces);
        self.stack_state.clear_continuation_indents();
    }

    fn recent_call_argument_indent_spaces(&self) -> Option<usize> {
        let indent_width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        for line in self.output.iter().rev().take(16) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if code.ends_with(';') || code.ends_with('{') || trimmed == "}" {
                break;
            }
            if let Some(open) = unmatched_open_paren_columns(code).last()
                && !line_is_control_body_header(trimmed)
            {
                let call_indent = assignment_call_value_column(code, tab_width)
                    .map(|column| column + indent_width)
                    .unwrap_or_else(|| visual_width_from(&code[..open + 1], 0, tab_width));
                return Some(
                    call_indent + self.line_adjuster.total_case_unindent_depth() * indent_width,
                );
            }
        }
        None
    }

    fn new_over_max_call_base_indent_spaces(&self) -> Option<usize> {
        for line in self
            .output
            .iter()
            .rev()
            .take(64)
            .filter(|line| !line.trim().is_empty())
        {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            if code.ends_with(',') {
                let base = leading_visual_width(line, self.options.tab_width);
                let has_over_max_new_call =
                    unmatched_open_paren_columns(code).into_iter().any(|open| {
                        open.saturating_sub(base) >= self.options.max_continuation_indent
                            && code[..open].match_indices("new ").any(|(index, _)| {
                                code[..index]
                                    .chars()
                                    .next_back()
                                    .is_none_or(|ch| !is_identifier_continue(ch))
                                    && !code[index + "new ".len()..open].contains(['(', ')'])
                            })
                    });
                if has_over_max_new_call {
                    return Some(
                        base + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                    );
                }
            }
            let start = code.trim_start();
            if line_paren_imbalance(code).0 > 0 && !code.ends_with(',')
                || start.starts_with(')')
                || start.ends_with(';')
                || start.ends_with('{')
                || start.ends_with('}')
            {
                return None;
            }
        }
        None
    }

    pub(super) fn over_max_new_call_default_indent_spaces(&self, line: &str) -> Option<usize> {
        let base_indent = self.new_over_max_call_base_indent_spaces()?;
        Some(if line.trim_start().starts_with(')') {
            base_indent
        } else {
            base_indent + self.options.indent_width * 2
        })
    }

    pub(super) fn over_max_new_call_adjacent_string_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if self.options.indent_after_parens {
            return None;
        }
        let base_indent = self.new_over_max_call_base_indent_spaces()?;
        let comment_string_spaces = if starts_string_literal_token(line.trim_start()) {
            self.output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .and_then(|previous| {
                    is_comment_line(previous.trim_start())
                        .then_some(leading_visual_width(previous, self.options.tab_width))
                })
        } else {
            None
        };
        let comma_after_string_spaces = if line.trim_start().starts_with(',') {
            self.output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .and_then(|previous| {
                    starts_string_literal_token(previous.trim_start())
                        .then_some(leading_visual_width(previous, self.options.tab_width))
                })
        } else {
            None
        };
        let adjacent_string_call_close = if line.trim_start().starts_with(");")
            && self
                .output
                .iter()
                .rev()
                .take(8)
                .any(|line| starts_string_literal_token(line.trim_start()))
        {
            self.output.iter().rev().take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                if starts_string_literal_token(code.trim_start()) {
                    return None;
                }
                unmatched_open_paren_columns(code)
                    .last()
                    .copied()
                    .map(|open| {
                        assignment_call_value_column(code, self.options.tab_width).unwrap_or(open)
                    })
            })
        } else {
            None
        };
        Some(
            comment_string_spaces
                .or(comma_after_string_spaces)
                .or(adjacent_string_call_close)
                .unwrap_or_else(|| {
                    if line.trim_start().starts_with(')') {
                        base_indent
                    } else {
                        base_indent + self.options.indent_width * 2
                    }
                }),
        )
    }

    pub(super) fn has_over_max_new_call_context(&self) -> bool {
        self.new_over_max_call_base_indent_spaces().is_some()
    }

    fn new_empty_call_base_indent_spaces(&self) -> Option<usize> {
        for line in self
            .output
            .iter()
            .rev()
            .take(64)
            .filter(|line| !line.trim().is_empty())
        {
            let trimmed = line.trim_end();
            if trimmed.contains(" new ")
                && trimmed.ends_with('(')
                && !trimmed.trim_start().starts_with("return ")
            {
                let new_index = trimmed.rfind(" new ")?;
                let open_index = trimmed.rfind('(')?;
                if new_index < open_index
                    && !trimmed[new_index + "new ".len()..open_index].contains(['(', ')'])
                {
                    return Some(
                        leading_visual_width(line, self.options.tab_width)
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
            }
            let start = trimmed.trim_start();
            if line_paren_imbalance(trimmed).0 > 0
                || start.starts_with(')')
                || start.ends_with(';')
                || start.ends_with('{')
                || start.ends_with('}')
            {
                return None;
            }
        }
        None
    }

    pub(super) fn split_or_empty_new_call_indent_spaces(&self, line: &str) -> Option<usize> {
        let mut spaces = None;
        if line.trim() == "("
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.contains(" new ")
        {
            spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        if let Some(paren_indent) = self.split_new_call_paren_indent_spaces() {
            spaces = Some(if line.trim_start().starts_with(')') {
                paren_indent
            } else {
                paren_indent + self.options.indent_width
            });
        }
        if let Some(base_indent) = self.new_empty_call_base_indent_spaces() {
            spaces = Some(if line.trim_start().starts_with(')') {
                base_indent
            } else {
                base_indent + self.options.indent_width
            });
        }
        spaces
    }

    pub(super) fn argument_after_split_new_call_opener_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if previous.trim() != "(" {
            return None;
        }
        let before_paren = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous)
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        if !before_paren.contains(" new ") && !before_paren.contains("(new ") {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        let extra = if previous_indent >= self.options.max_continuation_indent {
            self.options.indent_width * 2
        } else {
            self.options.indent_width
        };
        Some(
            previous_indent
                + extra
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(super) fn maximum_length_new_call_argument_indent_spaces(&self) -> Option<usize> {
        if self.options.max_code_length.is_none() || self.options.indent_after_parens {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !previous[..trailing_comment_split_limit(previous)]
            .trim_end()
            .ends_with(',')
            || !previous
                .split(|ch: char| !is_identifier_continue(ch))
                .any(|word| word == "new")
        {
            return None;
        }
        self.active_output_paren_continuation_indent_spaces()
    }

    pub(super) fn maximum_length_capped_open_paren_argument_indent_spaces(
        &self,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.max_code_length.is_none()
            || self.options.indent_after_parens
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let base = self.continuation_base_indent() * self.options.indent_width;
        (previous_code.ends_with('(')
            && leading_visual_width(previous, self.options.tab_width) == base
            && unmatched_open_paren_column(previous_code)
                .is_some_and(|open| open + 1 > base + self.options.max_continuation_indent))
        .then_some(base + self.options.indent_width * 2)
    }

    pub(super) fn over_max_new_call_argument_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        let new_pos = previous_code
            .find(" new ")
            .or_else(|| previous_code.find("(new "))?;
        let opens = unmatched_open_paren_columns(previous_code);
        let (&outer, &inner) = (opens.first()?, opens.last()?);
        if inner <= outer {
            return None;
        }
        let base = leading_visual_width(previous, self.options.tab_width);
        if inner.saturating_sub(base) <= self.options.max_continuation_indent {
            return None;
        }
        if !unmatched_open_paren_columns(&previous_code[..new_pos]).is_empty() {
            let padding = previous_code
                .chars()
                .skip(outer + 1)
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            let padding_width = visual_width_from(&padding, outer + 1, self.options.tab_width);
            return Some(outer + 1 + padding_width);
        }
        (previous_code[..new_pos].contains('=') && previous_code[new_pos..].contains("(new "))
            .then_some(base + self.options.indent_width * 2)
    }

    pub(super) fn closed_over_max_new_call_sibling_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with("),") {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        for line in self.output.iter().rev().skip(1).take(8) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim();
            if trimmed.ends_with(';') || trimmed == "{" || trimmed == "}" {
                break;
            }
            if let Some(new_pos) = code.find(" new ").or_else(|| code.find("(new ")) {
                let opens = unmatched_open_paren_columns(code);
                if let Some(&inner) = opens.last() {
                    let base = leading_visual_width(line, self.options.tab_width);
                    if inner.saturating_sub(base) > self.options.max_continuation_indent
                        && unmatched_open_paren_columns(&code[..new_pos]).is_empty()
                        && code[..new_pos].contains('=')
                        && code[new_pos..].contains("(new ")
                        && previous_indent <= base + self.options.indent_width * 2
                    {
                        return Some(previous_indent);
                    }
                }
            }
        }
        None
    }

    pub(super) fn preprocessor_branch_new_call_fallback_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent {
            return None;
        }
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        if unmatched_open_paren_column(previous_code).is_none() {
            for line in self.output.iter().rev().skip(1).take(12) {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                    break;
                }
                if (code.contains(" new ") || code.contains("(new "))
                    && unmatched_open_paren_column(code).is_some()
                {
                    return Some(leading_visual_width(previous, self.options.tab_width));
                }
            }
        }
        if (previous_code.contains(" new ") || previous_code.contains("(new "))
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            let base = leading_visual_width(previous, self.options.tab_width);
            if open.saturating_sub(base) <= self.options.max_continuation_indent {
                return Some(open + 1);
            }
        }
        None
    }

    pub(super) fn split_new_call_owner_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        let open = unmatched_open_paren_column(previous_code)?;
        let mut nonempty = self
            .output
            .iter()
            .rev()
            .skip(1)
            .filter(|line| !line.trim().is_empty());
        while let Some(line) = nonempty.next() {
            let trimmed = line.trim();
            if trimmed == "(" {
                return nonempty
                    .next()
                    .is_some_and(|line| line.contains(" new ") || line.contains("(new "))
                    .then_some(open + 1);
            }
            if trimmed.starts_with(')')
                || trimmed.ends_with(';')
                || trimmed.ends_with('{')
                || trimmed.ends_with('}')
            {
                break;
            }
        }
        None
    }

    pub(super) fn split_new_call_sibling_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        let base = self.split_new_call_paren_indent_spaces()?;
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        if let Some(open) = unmatched_open_paren_column(previous_code)
            && open + 1 > previous_indent
        {
            return Some(open + 1);
        }
        (previous_indent > base + self.options.indent_width).then_some(previous_indent)
    }

    pub(super) fn split_new_call_paren_indent_spaces(&self) -> Option<usize> {
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if code.trim() != "(" && unmatched_open_paren_column(code).is_some() {
                return None;
            }
        }
        let mut nonempty = self
            .output
            .iter()
            .rev()
            .take(64)
            .filter(|line| !line.trim().is_empty());
        while let Some(line) = nonempty.next() {
            let trimmed = line.trim();
            if trimmed == "(" {
                let before_paren = nonempty.next()?;
                if before_paren.contains(" new ") || before_paren.contains("(new ") {
                    return Some(
                        leading_visual_width(line, self.options.tab_width)
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
                return None;
            }
            if line_paren_imbalance(trimmed).0 > 0
                || trimmed.starts_with(')')
                || trimmed.ends_with(';')
                || trimmed.ends_with('{')
                || trimmed.ends_with('}')
            {
                return None;
            }
        }
        None
    }

    pub(super) fn call_argument_sibling_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero {
            return None;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        if code.ends_with('{')
            && (line_opens_lambda_block(line)
                || line
                    .trim()
                    .strip_suffix('{')
                    .is_some_and(|head| is_lambda_capture_header(head.trim_end())))
        {
            return None;
        }
        if unmatched_open_paren_column(code).is_some() {
            return None;
        }
        let argument = self.frame_stack.last_argument()?;
        if argument.role != CommaRole::CallArgument {
            return None;
        }
        let active_owner_matches =
            self.frame_stack
                .active_delimiter_with_id()
                .is_some_and(|(owner, delimiter)| {
                    argument.owner == Some(owner) && delimiter.role.is_call_like()
                });
        if !active_owner_matches {
            let previous = self.output.last_non_empty_line()?;
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if !code.contains(')') || !previous_code.ends_with(',') || argument.owner.is_none() {
                return None;
            }
        }
        let anchor = argument.sibling_anchor_column?;
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let case_unindent = (self.line_adjuster.total_case_unindent_depth()
            * self.options.indent_width)
            .max(self.adjusted_line_indent_delta(previous));
        (self.token_input.token_source_line_indent == anchor).then_some(anchor + case_unindent)
    }

    pub(super) fn outer_call_argument_after_closed_inner_call_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if previous_code.ends_with("),") {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            for raw in self.output.iter().rev().skip(1).take(8) {
                let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                let trimmed = code.trim();
                if trimmed.ends_with(';') || trimmed == "{" || trimmed == "}" {
                    break;
                }
                if (code.contains(" new ") || code.contains("(new "))
                    && find_assignment_operator(code).is_some()
                    && let Some(open) = unmatched_open_paren_column(code)
                {
                    let base = leading_visual_width(raw, self.options.tab_width);
                    if open.saturating_sub(base) > self.options.max_continuation_indent {
                        return Some(previous_indent);
                    }
                }
            }
        }
        if previous_code.ends_with(',') {
            let previous_imbalance = line_paren_imbalance(previous_code);
            let spaces = self.outer_call_indent_after_closed_previous_line()?;
            let spaces =
                if self.current_inline_array_column().is_some() || self.in_initializer_brace() {
                    spaces.saturating_sub(1)
                } else {
                    spaces
                };
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if case_unindent > 0 {
                let spaces = if previous_imbalance.1.is_empty() {
                    spaces + case_unindent
                } else {
                    spaces
                };
                if leading_visual_width(line, self.options.tab_width) > 0
                    || self.token_input.token_source_line_indent > spaces
                    || leading_visual_width(previous, self.options.tab_width) == spaces
                    || previous_imbalance.0 > 0
                {
                    return Some(spaces);
                }
            } else if previous_imbalance.0 > 0 {
                return Some(spaces);
            }
        }
        None
    }

    pub(super) fn outer_call_indent_after_closed_previous_line(&self) -> Option<usize> {
        let mut close_pending = 0usize;
        for raw in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(12)
        {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let (closes, mut opens) = line_paren_imbalance(code);
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if let Some(open) = opens.last() {
                return Some(visual_width_from(
                    &code[..open + 1],
                    0,
                    self.options.tab_width,
                ));
            }
            let trimmed = code.trim();
            if trimmed.ends_with(';') || trimmed == "{" || trimmed == "}" {
                break;
            }
        }
        None
    }

    pub(super) fn completed_call_top_level_comma_sibling_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !previous_code.ends_with("),")
            || previous_trimmed.starts_with(':')
            || previous_code.contains('<')
            || current.contains('>')
            || current.starts_with(['#', '{', '}', ')'])
            || unmatched_open_paren_column(previous_code).is_some()
            || !previous_code
                .strip_suffix(',')
                .is_some_and(has_top_level_comma_text)
        {
            return None;
        }
        if line_paren_imbalance(previous_code).0 > 0
            && let Some(spaces) = self.outer_call_indent_after_closed_previous_line()
        {
            return Some(spaces);
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn closed_call_sibling_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') {
            return None;
        }
        if line_paren_imbalance(previous_code).0 > 0
            && let Some(spaces) = self.outer_call_indent_after_closed_previous_line()
        {
            return Some(spaces);
        }
        (previous_code.contains(").") && unmatched_open_paren_column(previous_code).is_none())
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn nested_call_argument_over_max_output_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',')
            || previous_code.contains(" new ")
            || previous_code.contains("(new ")
            || self.constructor_initializer_base_indent_spaces().is_some()
            || self.output_has_constructor_initializer_colon()
        {
            return None;
        }
        let columns = unmatched_open_paren_columns(previous_code);
        let base = leading_visual_width(previous, self.options.tab_width);
        let statement_base = self.continuation_base_indent() * self.options.indent_width;
        let inner = *columns.last()?;
        let over_statement_max = inner >= statement_base + self.options.max_continuation_indent;
        if inner < base + self.options.max_continuation_indent && !over_statement_max {
            return None;
        }
        if over_statement_max
            && let Some(spaces) = self
                .frame_stack
                .active_delimiter()
                .filter(|delimiter| delimiter.opener_output_column == inner)
                .and_then(|delimiter| delimiter.continuation_indent_column)
        {
            return Some(spaces);
        }
        if columns.len() < 2 {
            if previous_code.contains(" new ") || previous_code.contains("(new ") {
                return None;
            }
            if base >= self.options.indent_width * 2
                && (!self.preprocessor.branch_stack.is_empty()
                    || output_has_active_preprocessor_branch(self.output.as_slice()))
            {
                return Some(base + self.options.indent_width * 2);
            }
            if let Some((mut eq, mut operator)) = find_assignment_operator(previous_code)
                && eq < inner
            {
                let mut search_start = eq + operator.len();
                while search_start < inner {
                    let Some((next, next_operator)) =
                        find_assignment_operator(&previous_code[search_start..inner])
                    else {
                        break;
                    };
                    eq = search_start + next;
                    operator = next_operator;
                    search_start = eq + operator.len();
                }
                let value_start = previous_code[eq + operator.len()..]
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(previous_code.len(), |(offset, _)| {
                        eq + operator.len() + offset
                    });
                let spaces =
                    visual_width_from(&previous_code[..value_start], 0, self.options.tab_width);
                if spaces.saturating_sub(base) > self.options.max_continuation_indent {
                    return Some(base + self.options.indent_width * 2);
                }
                return Some(spaces);
            }
            if over_statement_max {
                return Some(base + self.options.indent_width * 2);
            }
            return None;
        }
        let outer = columns[columns.len() - 2] + 1;
        if columns.len() == 2
            && over_statement_max
            && starts_header_word(previous_code.trim_start(), "if")
        {
            return Some(base + self.options.indent_width * 2);
        }
        if over_statement_max && outer <= base {
            return Some(base + self.options.indent_width * 2);
        }
        let max_base = if over_statement_max {
            statement_base
        } else {
            base
        };
        if outer.saturating_sub(max_base) > self.options.max_continuation_indent {
            return Some(base + self.options.indent_width * 2);
        }
        Some(outer)
    }

    pub(super) fn over_max_inner_call_open_argument_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous.trim_end().ends_with('(') {
            return None;
        }
        let opens = line_paren_imbalance(previous_code).1;
        if opens.len() < 2 {
            return None;
        }
        if previous_code.contains(").") {
            return Some(opens[opens.len() - 2] + 1 + self.options.indent_width);
        }
        let inner = *opens.last()?;
        let base = leading_visual_width(previous, self.options.tab_width);
        let head = previous_code[..opens[0]].trim_start();
        let callee = previous_code[..inner]
            .rsplit(|ch: char| !(is_identifier_continue(ch) || matches!(ch, '.' | ':')))
            .next()
            .unwrap_or_default();
        if callee.contains('.') && is_macro_like_word(head) {
            return Some(base + self.options.indent_width * 3);
        }
        if callee.contains('.') && inner + 1 > base + self.options.max_continuation_indent {
            return Some(base + self.options.indent_width * 3);
        }
        None
    }

    pub(super) fn line_opens_attachable_lambda_block(&self, line: &str) -> bool {
        let trimmed = line.trim_end();
        if trimmed.ends_with('{') {
            return line_opens_lambda_block(line)
                || trimmed
                    .strip_suffix('{')
                    .is_some_and(|head| is_lambda_capture_header(head.trim_end()));
        }
        !trimmed.contains('{')
            && trimmed.trim_start().starts_with('[')
            && matches!(
                self.options.brace_style,
                BraceStyle::Attach
                    | BraceStyle::OneTrueBrace
                    | BraceStyle::WebKit
                    | BraceStyle::Ratliff
                    | BraceStyle::Lisp
            )
            && is_lambda_body_header(trimmed)
            && super::brace_classification::lambda_header_has_trailing_return(trimmed)
    }

    pub(super) fn lambda_call_argument_after_split_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !line.trim_start().starts_with('[') || !self.line_opens_attachable_lambda_block(line) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') || !line_paren_imbalance(previous_code).1.is_empty() {
            return None;
        }
        for raw in self.output.iter().rev().skip(1).take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            if !line_paren_imbalance(code).1.is_empty() {
                return Some(leading_visual_width(raw, self.options.tab_width));
            }
            let trimmed = code.trim_start();
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                break;
            }
        }
        None
    }

    pub(super) fn argument_after_lambda_call_argument_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '(', ')', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if previous_code.ends_with(',') && self.previous_output_line_closes_lambda_body() {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        None
    }

    fn previous_output_line_closes_lambda_body(&self) -> bool {
        let mut depth = 0usize;
        for raw in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            if depth == 0 && !code.trim_start().starts_with('}') {
                return false;
            }
            for (index, ch) in code.char_indices().rev() {
                match ch {
                    '}' => depth += 1,
                    '{' if depth > 0 => {
                        depth -= 1;
                        if depth == 0 {
                            let head = code[..=index].trim_end();
                            return line_opens_lambda_block(head)
                                || head
                                    .trim()
                                    .strip_suffix('{')
                                    .is_some_and(|head| is_lambda_capture_header(head.trim_end()));
                        }
                    }
                    _ => {}
                }
            }
        }
        false
    }
}
