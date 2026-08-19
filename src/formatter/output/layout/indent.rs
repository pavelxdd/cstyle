use super::super::super::FormatEngine;
use super::super::super::brace_classification::{
    is_lambda_body_header, line_opens_lambda_block, line_opens_lambda_or_capture_only_block,
};
use super::super::super::call_arguments::callee_name_start_before_open;
use super::super::super::columns::{leading_visual_width, visual_width_from};
use super::super::super::compound_literals::line_ends_compound_literal_cast;
use super::super::super::constructor_initializers::has_inline_constructor_initializer_colon;
use super::super::super::headers::{
    line_is_control_body_header, same_line_nested_header_extra, starts_header_word,
};
use super::super::super::indentation::LineKind;
use super::super::super::labels;

use super::super::super::language::is_macro_like_word;
use super::super::super::line_adjust::macro_call_starts_with;
use super::super::super::line_scan::is_comment_only_line;
use super::super::super::line_scan::{
    has_unmatched_open_brace, line_paren_imbalance, reverse_scan_skips_block_comment,
    trailing_comment_split_limit, unmatched_open_bracket_column, unmatched_open_paren_column,
    unmatched_open_paren_columns,
};
use super::super::super::literals::{
    first_string_literal_start, last_string_literal_start, single_string_literal_comma_line,
    starts_string_literal_token, string_literal_has_opening_context,
};
use super::super::super::objective_c::objc_message_following_keyword_column;
use super::super::super::operators::{
    find_assignment_operator, head_ends_binary_operator, starts_with_chain_operator,
    trailing_binary_operator_column,
};

use super::super::super::{language, switch_cases};
use crate::config::BraceStyle;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

impl FormatEngine<'_> {
    pub(in super::super) fn member_init_continuation_extra(
        &self,
        line_kind: LineKind,
        line: &str,
    ) -> usize {
        if line_kind != LineKind::Normal {
            return 0;
        }
        if self.continuation_indent.next_line_indent_spaces.is_some() {
            return 0;
        }
        let trimmed = line.trim_start();
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line[..trailing_comment_split_limit(line)].trim())
        else {
            return 0;
        };
        if trimmed.starts_with(':') && !trimmed.starts_with("::") {
            let previous_constructor = previous.ends_with(')')
                || self
                    .output
                    .iter()
                    .rev()
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !(trimmed.starts_with('#') || trimmed.is_empty())
                    })
                    .is_some_and(|line| line.trim_end().ends_with(')'));
            return usize::from(previous_constructor);
        }
        if previous.ends_with(':')
            && previous.contains(')')
            && !previous.starts_with("//")
            && switch_cases::find_case_colon(previous).is_none()
            && !previous.starts_with("default")
            && !trimmed.starts_with('{')
        {
            return 1;
        }
        if trimmed.starts_with(',') {
            let previous_member_init = previous.starts_with(':')
                || previous.starts_with(',')
                || self.output.iter().rev().take(8).any(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.starts_with('#')
                        && (trimmed.starts_with(':') || trimmed.starts_with(','))
                });
            return usize::from(previous_member_init);
        }
        0
    }

    pub(in super::super) fn string_literal_continuation_after_layout_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !starts_string_literal_token(trimmed) {
            return None;
        }
        if let Some(spaces) = self.line_after_trailing_stream_operator_indent_spaces() {
            return Some(spaces);
        }
        if let Some(spaces) = self.string_after_stream_string_indent_spaces() {
            return Some(spaces);
        }
        let mut skipped_layout_line = false;
        let mut skipped_comment_indent = None;
        for raw in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let previous = raw.trim_start();
            if previous.starts_with('#') {
                skipped_layout_line = true;
                continue;
            }
            if is_comment_only_line(previous) {
                skipped_layout_line = true;
                skipped_comment_indent = Some(leading_visual_width(raw, self.options.tab_width));
                continue;
            }
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed_code = code.trim_start();
            if trimmed_code.ends_with(',') {
                if single_string_literal_comma_line(code) {
                    return Some(
                        leading_visual_width(raw, self.options.tab_width)
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
                return None;
            }
            if let Some((assignment, operator)) = find_assignment_operator(code) {
                let after_operator = assignment + operator.len();
                if code[after_operator..].trim().is_empty() {
                    return Some(
                        leading_visual_width(raw, self.options.tab_width)
                            + self.options.indent_width
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
            }
            if trimmed_code.ends_with([';', '{', '}']) {
                return None;
            }
            if trimmed_code.ends_with("},") && unmatched_open_paren_column(code).is_some() {
                return None;
            }
            if let (Some(first), Some(last)) = (
                first_string_literal_start(code),
                last_string_literal_start(code),
            ) && first != last
                && code[..first].trim().is_empty()
            {
                return Some(
                    leading_visual_width(raw, self.options.tab_width)
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            }
            return last_string_literal_start(code)
                .filter(|start| string_literal_has_opening_context(code, *start))
                .and_then(|start| {
                    let before_string = &code[..start];
                    let leading = leading_visual_width(raw, self.options.tab_width);
                    let case_unindent = (self.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width)
                        .max(self.adjusted_line_indent_delta(raw));
                    if let Some(spaces) = skipped_comment_indent {
                        return Some(spaces + case_unindent);
                    }
                    if before_string.trim().is_empty()
                        || (before_string.contains(',')
                            && unmatched_open_paren_column(before_string).is_none())
                    {
                        return Some(leading + case_unindent);
                    }
                    let previous_trimmed = raw.trim_start();
                    if previous_trimmed.starts_with("+ ") || previous_trimmed.starts_with("- ") {
                        return Some(
                            leading
                                + self.options.indent_width * 2
                                + self.line_adjuster.total_case_unindent_depth()
                                    * self.options.indent_width,
                        );
                    }
                    if starts_with_chain_operator(previous_trimmed)
                        && let Some(open) = before_string.rfind('(')
                        && before_string[open + 1..].contains(',')
                    {
                        return Some(
                            open + 1
                                + self.line_adjuster.total_case_unindent_depth()
                                    * self.options.indent_width,
                        );
                    }
                    let call_opens = unmatched_open_paren_columns(before_string);
                    if let Some(&open) = call_opens.last()
                        && let Some(relative_brace) = before_string[open + 1..].rfind('{')
                    {
                        let brace = open + 1 + relative_brace;
                        if before_string[..brace]
                            .chars()
                            .last()
                            .is_none_or(|ch| !is_identifier_continue(ch))
                        {
                            return None;
                        }
                    }
                    if let Some(&open) = call_opens.last() {
                        let after_open = before_string[open + 1..].to_string();
                        if after_open.contains(',') {
                            let padding = after_open
                                .chars()
                                .take_while(|ch| ch.is_whitespace())
                                .collect::<String>();
                            let padding_width =
                                visual_width_from(&padding, open + 1, self.options.tab_width);
                            return Some(
                                open + 1
                                    + padding_width
                                    + self.line_adjuster.total_case_unindent_depth()
                                        * self.options.indent_width,
                            );
                        }
                    }
                    let string_start = visual_width_from(before_string, 0, self.options.tab_width);
                    if (previous_trimmed.starts_with(['+', '-'])
                        || starts_with_chain_operator(previous_trimmed))
                        && before_string.contains('(')
                        && string_start.saturating_sub(leading)
                            > self.options.max_continuation_indent
                    {
                        return Some(
                            leading
                                + self.options.indent_width * 2
                                + self.line_adjuster.total_case_unindent_depth()
                                    * self.options.indent_width,
                        );
                    }
                    let spaces = if string_start.saturating_sub(leading)
                        <= self.options.max_continuation_indent
                    {
                        string_start
                    } else if let Some((eq, _)) = find_assignment_operator(before_string) {
                        if !skipped_layout_line && !before_string[eq + 1..].contains('(') {
                            return None;
                        }
                        let max_column = leading + self.options.max_continuation_indent;
                        let opens = unmatched_open_paren_columns(before_string);
                        if opens.len() >= 2 {
                            let outer_open = opens[opens.len() - 2];
                            let inner_open = *opens.last()?;
                            if outer_open < max_column {
                                let spaces =
                                    callee_name_start_before_open(before_string, inner_open)
                                        .filter(|callee_start| *callee_start <= max_column)
                                        .unwrap_or(outer_open + 1);
                                return Some(
                                    spaces
                                        + self.line_adjuster.total_case_unindent_depth()
                                            * self.options.indent_width,
                                );
                            }
                        }
                        let value_start = before_string[eq + 1..]
                            .char_indices()
                            .find(|(_, ch)| !ch.is_whitespace())
                            .map_or(before_string.len(), |(offset, _)| eq + 1 + offset);
                        visual_width_from(&code[..value_start], 0, self.options.tab_width)
                    } else {
                        let opens = unmatched_open_paren_columns(code);
                        if opens.len() >= 2 {
                            opens[opens.len() - 2] + 1
                        } else {
                            leading + self.options.indent_width * 2
                        }
                    };
                    Some(
                        spaces
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    )
                });
        }
        None
    }

    pub(in super::super) fn array_bound_operator_output_indent_spaces(&self) -> Option<usize> {
        let previous = self.output.last()?;
        let trimmed = previous.trim_end();
        if !head_ends_binary_operator(trimmed) || unmatched_open_bracket_column(trimmed).is_none() {
            return None;
        }
        trailing_binary_operator_column(trimmed)
    }

    pub(in super::super::super) fn adjusted_line_indent_delta(&self, adjusted: &str) -> usize {
        self.previous_pre_adjust_line
            .as_deref()
            .filter(|raw| raw.trim() == adjusted.trim())
            .map_or(0, |raw| {
                leading_visual_width(raw, self.options.tab_width)
                    .saturating_sub(leading_visual_width(adjusted, self.options.tab_width))
            })
    }

    pub(in super::super) fn function_parameter_continuation_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed_line = line.trim_start();
        if trimmed_line.is_empty()
            || trimmed_line.starts_with(['#', '{', '}'])
            || starts_with_chain_operator(trimmed_line)
        {
            return None;
        }
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            && previous.trim() == "("
            && !self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line.contains(" new ") || line.contains("(new "))
        {
            let leading = leading_visual_width(previous, self.options.tab_width);
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if trimmed_line.starts_with(')') {
                return Some(leading + case_unindent);
            }
            return Some(leading + self.options.indent_width + case_unindent);
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
            let Some(column) = unmatched_open_paren_column(trimmed) else {
                continue;
            };
            if opens.last() != Some(&column) {
                continue;
            }
            if !trimmed[column..].starts_with('(') {
                continue;
            }
            if let Some(first_open) = trimmed.find('(') {
                let macro_name = trimmed[..first_open].trim();
                if is_macro_like_word(macro_name) {
                    let case_unindent =
                        self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
                    if trimmed_line.starts_with(')') {
                        return Some(
                            column + self.adjusted_line_indent_delta(previous).max(case_unindent),
                        );
                    }
                    let padding = trimmed
                        .chars()
                        .skip(column + 1)
                        .take_while(|ch| ch.is_whitespace())
                        .collect::<String>();
                    let padding_width =
                        visual_width_from(&padding, column + 1, self.options.tab_width);
                    let spaces = column
                        + 1
                        + padding_width
                        + self.adjusted_line_indent_delta(previous).max(case_unindent);
                    if spaces <= self.options.max_continuation_indent {
                        return Some(spaces);
                    }
                    return None;
                }
            }
            return self.function_signature_parameter_continuation_indent_spaces(
                trimmed_line,
                trimmed,
                column,
            );
        }
        None
    }

    pub(in super::super::super) fn open_lambda_body_indent_spaces(&self) -> Option<usize> {
        let mut closed_blocks = 0usize;
        for raw in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            if line_opens_lambda_or_capture_only_block(code.trim_start()) && closed_blocks == 0 {
                return Some(
                    leading_visual_width(raw, self.options.tab_width) + self.options.indent_width,
                );
            }
            closed_blocks += code.chars().filter(|ch| *ch == '}').count();
            closed_blocks =
                closed_blocks.saturating_sub(code.chars().filter(|ch| *ch == '{').count());
        }
        None
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn using_alias_rhs_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.is_empty() || current.starts_with(['#', '{', '}']) {
            return None;
        }
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .and_then(|previous| {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                let previous_trimmed = previous_code.trim_start();
                if previous_trimmed.starts_with("using ") && previous_code.ends_with('=') {
                    let previous_indent = leading_visual_width(previous, self.options.tab_width);
                    if self.recent_base_trailing_return_function_header() {
                        Some(previous_indent)
                    } else {
                        Some(previous_indent + self.options.indent_width)
                    }
                } else {
                    None
                }
            })
    }

    pub(in super::super) fn split_assignment_rhs_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if current.is_empty() || current.starts_with(['#', '{', '}']) {
            return None;
        }
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .and_then(|previous| {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                let (operator_start, operator) = find_assignment_operator(previous_code)?;
                let before = previous_code[..operator_start].trim_end();
                if operator_start + operator.len() == previous_code.len()
                    && (before.contains("* ")
                        || before.contains("& ")
                        || before.ends_with('*')
                        || before.ends_with('&')
                        || self.recent_base_trailing_return_function_header())
                {
                    Some(
                        leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width,
                    )
                } else {
                    None
                }
            })
    }

    pub(in super::super) fn current_macro_block_begin_indent_spaces(&self) -> Option<usize> {
        if self.options.macro_blocks.is_empty() {
            return None;
        }
        let mut closed_blocks = 0usize;
        for index in (0..self.output.len()).rev() {
            let line = &self.output[index];
            let trimmed = self.output.trimmed(index);
            if self
                .options
                .macro_blocks
                .iter()
                .any(|(_, end)| macro_call_starts_with(trimmed, end))
            {
                closed_blocks += 1;
                continue;
            }
            if self
                .options
                .macro_blocks
                .iter()
                .any(|(begin, _)| macro_call_starts_with(trimmed, begin))
            {
                if closed_blocks == 0 {
                    return Some(leading_visual_width(line, self.options.tab_width));
                }
                closed_blocks -= 1;
            }
        }
        None
    }

    pub(in super::super) fn after_lambda_condition_indent_spaces(&self) -> Option<usize> {
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with("})") {
            return None;
        }
        let mut saw_lambda = false;
        for raw in self.output.iter().rev().take(32) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            if line_opens_lambda_block(code) {
                saw_lambda = true;
            }
            if starts_header_word(trimmed, "if")
                || starts_header_word(trimmed, "while")
                || starts_header_word(trimmed, "for")
                || trimmed.starts_with("else if")
            {
                return (saw_lambda && unmatched_open_paren_column(code).is_some())
                    .then_some(leading_visual_width(raw, self.options.tab_width));
            }
            if saw_lambda && (code.ends_with(';') || trimmed == "{") {
                break;
            }
        }
        None
    }

    pub(in super::super) fn asm_colon_line_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with(':') {
            return None;
        }
        let mut colon_spaces = None;
        let mut saw_asm = line.contains("asm") || line.contains("__asm__");
        for previous in self
            .output
            .iter()
            .rev()
            .take(8)
            .take_while(|line| !line.trim_end().ends_with(';'))
        {
            if colon_spaces.is_none() && previous.trim_start().starts_with(':') {
                colon_spaces = Some(leading_visual_width(previous, self.options.tab_width));
            }
            if previous.contains("asm") || previous.contains("__asm__") {
                saw_asm = true;
            }
        }
        saw_asm.then_some(colon_spaces).flatten()
    }

    pub(in super::super) fn contextual_line_indent_spaces(
        &self,
        line: &str,
        indent: usize,
        normal_indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        let current = line.trim_start();
        if current.is_empty() || is_lambda_body_header(current) {
            return None;
        }
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let natural = exact_indent_spaces.unwrap_or(indent * width);
        let previous_index = (0..self.output.len())
            .rev()
            .find(|&index| !self.output.trimmed(index).is_empty());
        let previous_is_preprocessor =
            previous_index.is_some_and(|index| self.output.trimmed(index).starts_with('#'));
        if let Some(spaces) =
            self.nonconditional_directive_sibling_indent_spaces(line, normal_indent)
        {
            return Some(spaces);
        }
        if exact_indent_spaces.is_some() && current.starts_with("else") {
            return None;
        }
        if exact_indent_spaces.is_some()
            && (current.starts_with("&&") || current.starts_with("||"))
            && self
                .header_operator_continuation_indent_spaces(line)
                .is_some()
        {
            return None;
        }
        if let Some(previous_index) = previous_index {
            let previous = &self.output[previous_index];
            let previous_trimmed = self.output.trimmed(previous_index);
            let previous_full_code = previous.trim_end();
            let previous_code = self.output.code(previous_index);
            if self.frame_stack.active_constructor_initializer().is_some()
                && is_comment_only_line(previous_trimmed)
                && current.chars().next().is_some_and(is_identifier_start)
                && self.output[..previous_index]
                    .iter()
                    .rev()
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !is_comment_only_line(trimmed)
                    })
                    .is_some_and(|line| {
                        line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .ends_with(',')
                    })
                && let Some(base) = self.constructor_initializer_base_indent_spaces()
            {
                return Some(base);
            }
            if previous_code.ends_with("&&")
                && previous_trimmed.starts_with("return ")
                && self.recent_base_trailing_return_function_header()
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if let Some(spaces) = self.template_base_colon_indent_spaces(current, previous) {
                return Some(spaces);
            }
            if let Some(spaces) = self.ternary_colon_row_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.ternary_first_arm_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.stream_chain_frame_indent_spaces(line) {
                return Some(spaces);
            }
            if previous_full_code.ends_with(';')
                && previous_full_code.contains("/*")
                && unmatched_open_paren_column(previous_full_code).is_none()
                && current
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_uppercase())
            {
                return Some(normal_indent * width);
            }
            if self.constructor_initializer_base_indent_spaces().is_some()
                && previous_code.ends_with(',')
                && has_unmatched_open_brace(previous_code)
                && current.contains('{')
                && !current.starts_with(['#', ':', ',', '{', '}'])
                && let Some(open) = previous_code.find('{')
            {
                return Some(
                    leading_visual_width(previous, tab_width).max(visual_width_from(
                        &previous_code[..open + 1],
                        0,
                        width,
                    )),
                );
            }
            if let Some(base) = self.constructor_initializer_base_indent_spaces()
                && previous_code.ends_with(',')
                && !current.starts_with(['#', ':', ',', '{', '}'])
                && current
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_uppercase())
            {
                if previous_code.ends_with("),") && line_paren_imbalance(previous_code).0 > 0 {
                    return Some(base);
                }
                if unmatched_open_paren_column(previous_code.trim_start()).is_none()
                    && !has_unmatched_open_brace(previous_code)
                    && !has_unmatched_open_brace(current)
                    && leading_visual_width(previous, tab_width) <= base
                    && !(previous_code.contains('{')
                        && current.contains('{')
                        && !previous_trimmed.starts_with(':')
                        && leading_visual_width(previous, tab_width) != base)
                {
                    return Some(base);
                }
            }
            if self.has_case_body_indent()
                && exact_indent_spaces.is_none_or(|spaces| spaces <= normal_indent * width)
                && previous_code.ends_with(';')
                && !current.starts_with(['#', '{', '}', ')'])
                && current
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_uppercase())
            {
                let normal_spaces = normal_indent * width;
                let previous_indent = leading_visual_width(previous, tab_width);
                if previous_indent > normal_spaces
                    && self.frame_stack.active_brace().is_some_and(|frame| {
                        frame.body_indent_column == previous_indent
                            && frame.header.as_deref().is_some_and(|header| {
                                starts_header_word(header, "if")
                                    || starts_header_word(header, "for")
                                    || starts_header_word(header, "while")
                                    || header.starts_with("else")
                            })
                    })
                {
                    return Some(previous_indent);
                }
                return Some(normal_spaces);
            }
            if let Some(spaces) = self.completed_call_top_level_comma_sibling_indent_spaces(line) {
                return Some(spaces);
            }
            if current.starts_with("{},")
                && previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                return Some(open + 1);
            }
            if (current.starts_with("+ ") || current.starts_with("- "))
                && let Some(open) = previous_code.rfind('{')
                && !previous_code[open + 1..].contains('}')
            {
                return Some(open + 1);
            }
            if previous_code.ends_with(',')
                && previous_code.contains('[')
                && !current.starts_with(['#', '(', ')', '{', '}'])
                && let Some(spaces) = objc_message_following_keyword_column(previous_code)
            {
                return Some(spaces);
            }
            if !self.options.indent_after_parens
                && previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                let mut saw_blank = false;
                for raw in self.output.iter().rev().skip(1) {
                    if raw.trim().is_empty() {
                        saw_blank = true;
                        continue;
                    }
                    if saw_blank && raw.trim_start().starts_with(':') {
                        return Some(open + 1);
                    }
                    break;
                }
            }
            if previous_code.ends_with(',')
                && previous_trimmed.starts_with('(')
                && !current.starts_with(['#', '(', ')', '{', '}'])
            {
                let fallback = leading_visual_width(previous, tab_width)
                    + usize::from(!previous_code.contains('?'));
                return Some(
                    unmatched_open_paren_column(previous_code).map_or(fallback, |open| open + 1)
                        + self.line_adjuster.total_case_unindent_depth() * width,
                );
            }
            if !self.options.indent_after_parens
                && previous_code.ends_with(',')
                && let Some(spaces) = self.nested_call_argument_over_max_output_indent_spaces(line)
            {
                return Some(spaces);
            }
            let previous_indent = leading_visual_width(previous, tab_width);
            if !current.starts_with(['#', '{', '}', ')'])
                && previous_indent > natural
                && (0..self.output.len()).rev().take(16).any(|index| {
                    let trimmed = self.output.code(index);
                    trimmed.contains("operator[]") && trimmed.ends_with('{')
                })
            {
                return Some(previous_indent);
            }
            if current.starts_with(':') && previous_trimmed.starts_with("//") {
                if let Some(spaces) =
                    self.ternary_colon_after_comment_indent_spaces(current, previous)
                {
                    return Some(spaces);
                }
                for raw in self.output.iter().rev().skip(1).take(16) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed = code.trim_start();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if code.ends_with(')') && !code.ends_with(';') {
                        let first_word = trimmed
                            .split(|ch: char| !is_identifier_continue(ch))
                            .find(|word| !word.is_empty())
                            .unwrap_or_default();
                        if !language::is_header(first_word) {
                            return Some(leading_visual_width(raw, tab_width) + width);
                        }
                    }
                    break;
                }
            }
            if let Some(spaces) = labels::access_label_body_indent_spaces(
                line,
                previous,
                self.stack_state.brace_type_stack.last().copied(),
                self.options,
            ) {
                return Some(spaces);
            }
            if current.starts_with('|')
                && !current.starts_with("||")
                && self.constructor_initializer_base_indent_spaces().is_some()
                && (previous_trimmed.starts_with('|') || previous_code.contains('|'))
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if current.starts_with("if")
                && (previous_trimmed.starts_with("foreach ")
                    || previous_trimmed.starts_with("foreach("))
            {
                return Some(leading_visual_width(previous, tab_width) + width);
            }
            if current.starts_with("else") && previous_code.ends_with([';', '}']) {
                let previous_indent = leading_visual_width(previous, tab_width);
                if previous_trimmed.starts_with("if") {
                    return Some(
                        previous_indent + same_line_nested_header_extra(previous_trimmed) * width,
                    );
                }
                if previous_code.ends_with(';') && previous_trimmed.starts_with("else") {
                    if previous_trimmed.starts_with("else if") {
                        return Some(previous_indent);
                    }
                    if let Some(previous_index) = self
                        .output
                        .iter()
                        .rposition(|line| std::ptr::eq(line, previous))
                    {
                        for if_index in (0..previous_index).rev() {
                            let line = &self.output[if_index];
                            if line.trim().is_empty() {
                                continue;
                            }
                            let indent = leading_visual_width(line, tab_width);
                            let trimmed = line[..trailing_comment_split_limit(line)].trim_start();
                            if indent == previous_indent
                                && trimmed.starts_with("if")
                                && trimmed.ends_with(';')
                            {
                                let level = previous_indent / width;
                                return Some(
                                    self.enclosing_if_level(
                                        if_index,
                                        level,
                                        level.saturating_sub(1),
                                    ) * width,
                                );
                            }
                            if indent < previous_indent {
                                break;
                            }
                        }
                    }
                    return Some(previous_indent.saturating_sub(width));
                }
            }
            if current.starts_with('=') && previous_trimmed.starts_with("#if") {
                for raw in self.output.iter().rev().skip(1) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed = code.trim_start();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    return Some(leading_visual_width(raw, tab_width) + width);
                }
            }
            if starts_string_literal_token(current)
                && (previous_trimmed.starts_with("#else") || previous_trimmed.starts_with("#elif"))
            {
                for raw in self.output.iter().rev().skip(1) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed = code.trim_start();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.starts_with("#if")
                        || trimmed.starts_with("#ifdef")
                        || trimmed.starts_with("#ifndef")
                    {
                        break;
                    }
                    if !trimmed.starts_with('#') {
                        return Some(leading_visual_width(raw, tab_width));
                    }
                }
            }
            if let Some(spaces) = self.split_declaration_assignment_indent_spaces(current, previous)
            {
                return Some(spaces);
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && previous_trimmed.starts_with('}')
            {
                return Some(
                    self.token_input
                        .token_source_line_indent
                        .min(leading_visual_width(previous, tab_width)),
                );
            }
            if previous_code.ends_with("},")
                && previous_code.contains(") }")
                && current.ends_with('{')
                && !current.starts_with(['{', '}', '.', '['])
                && find_assignment_operator(current).is_none()
                && unmatched_open_paren_column(previous_code).is_none()
            {
                return Some(normal_indent * width);
            }
            if previous_code.ends_with("},")
                && current.contains('{')
                && !current.starts_with(['{', '}', '.', '['])
                && find_assignment_operator(current).is_none()
                && unmatched_open_paren_column(previous_code).is_none()
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && previous_trimmed.starts_with('.')
                && find_assignment_operator(current).is_none()
                && find_assignment_operator(previous_trimmed).is_none()
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if previous_code.trim() == "}"
                && (current.starts_with("Q_FALLTHROUGH") || current.starts_with("[[fallthrough]]"))
            {
                return Some(
                    leading_visual_width(previous, tab_width)
                        + self.line_adjuster.total_case_unindent_depth() * width,
                );
            }
            if let Some(spaces) = self.argument_after_lambda_call_argument_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.closed_call_sibling_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.stream_after_closed_parenthesized_head_indent_spaces(current)
            {
                return Some(spaces);
            }
            if let Some(spaces) = self.stream_after_ternary_colon_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.previous_line_parenthesized_stream_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.parenthesized_stream_chain_head_indent_spaces(current) {
                return Some(spaces);
            }
            if previous_code.ends_with("},")
                && let Some(prefix) = current.strip_suffix('{')
                && !prefix.trim().is_empty()
                && !prefix.trim_start().starts_with('{')
                && !prefix.contains(['=', '(', '@'])
            {
                for line in self.output.iter().rev().skip(1).take(64) {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    if let Some(prefix) = code.strip_suffix('{') {
                        let prefix = prefix.trim_start();
                        if !prefix.is_empty()
                            && !prefix.starts_with('{')
                            && !prefix.contains(['=', '(', '@'])
                        {
                            return Some(leading_visual_width(line, tab_width));
                        }
                    }
                    if code.ends_with(';') || code.ends_with('}') {
                        break;
                    }
                }
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && previous_code.ends_with(')')
                && line_is_control_body_header(previous_trimmed)
            {
                return Some(
                    leading_visual_width(previous, tab_width)
                        + width * 2
                        + self.line_adjuster.total_case_unindent_depth() * width,
                );
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && !previous_code.ends_with('{')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                return Some(open + 1 + self.line_adjuster.total_case_unindent_depth() * width);
            }
            if previous_code.ends_with("qPrintable(")
                && current.starts_with("QString")
                && let Some(callee) = previous_code.find("qPrintable")
            {
                let callee_column = visual_width_from(&previous_code[..callee], 0, tab_width);
                let outer_column = unmatched_open_paren_columns(previous_code)
                    .first()
                    .map_or(callee_column, |open| open + 1 + width);
                return Some(callee_column.min(outer_column));
            }
            if current.starts_with('&')
                && previous_code.ends_with('(')
                && previous_code.contains("= std::get_if<")
                && let Some((eq, op)) = find_assignment_operator(previous_code)
            {
                let after_operator = eq + op.len();
                let value_start = previous_code[after_operator..]
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(previous_code.len(), |(offset, _)| after_operator + offset);
                return Some(
                    visual_width_from(&previous_code[..value_start], 0, tab_width) + width,
                );
            }
            if let Some(spaces) =
                self.nested_brace_after_stream_opener_indent_spaces(current, previous_code)
            {
                return Some(spaces);
            }
            if !current.starts_with(['#', '(', ')', '{', '}'])
                && previous_code.ends_with('(')
                && self.open_lambda_body_indent_spaces().is_some()
            {
                return Some(
                    leading_visual_width(previous, tab_width)
                        + width
                        + self.line_adjuster.total_case_unindent_depth() * width,
                );
            }
            if let Some(spaces) = self.return_chain_indent_spaces(current, previous, natural) {
                return Some(spaces);
            }
            if let Some(spaces) = self.logical_after_previous_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) =
                self.inline_stream_opener_argument_indent_spaces(current, previous_code)
            {
                return Some(spaces);
            }
            if !current.starts_with(['#', '(', ')', '{', '}', '.', '?', ':'])
                && previous_trimmed.starts_with("qDebug(")
                && previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                return Some(open + 1 + self.line_adjuster.total_case_unindent_depth() * width);
            }
            if current.starts_with(',') && previous_trimmed.starts_with('#') {
                for line in self.output.iter().rev().skip(1) {
                    let trimmed = line.trim_start();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    if trimmed.starts_with(',') {
                        return Some(leading_visual_width(line, tab_width));
                    }
                    break;
                }
            }
            if !current.starts_with(['/', '#', '{', '}']) && previous_trimmed.starts_with("//") {
                let previous_indent = leading_visual_width(previous, tab_width);
                for line in self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.starts_with("//") {
                        if trimmed.contains("{{{") {
                            return Some(previous_indent);
                        }
                        continue;
                    }
                    break;
                }
            }
            if !current.starts_with(['#', '(', ')', '{', '}'])
                && previous_code.ends_with(',')
                && let Some(capture_end) = previous_code.find("](")
                && previous_code[..capture_end].contains('=')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                let aligned = open + 1;
                if matches!(
                    self.options.brace_style,
                    BraceStyle::Allman
                        | BraceStyle::Whitesmith
                        | BraceStyle::Vtk
                        | BraceStyle::Gnu
                        | BraceStyle::Horstmann
                        | BraceStyle::Pico
                ) || self.token_input.token_source_line_indent == aligned
                {
                    return Some(aligned);
                }
                return Some(leading_visual_width(previous, tab_width));
            }
            if previous_trimmed.starts_with(':')
                && previous_code.ends_with(',')
                && self.constructor_initializer_base_indent_spaces().is_none()
                && !current.starts_with(['#', '(', ')', '{', '}'])
            {
                for raw in self.output.iter().rev().skip(1).take(16) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed_code = code.trim();
                    if trimmed_code == "(" {
                        return Some(leading_visual_width(raw, tab_width) + width);
                    }
                    if let Some(open) = unmatched_open_paren_column(code) {
                        return Some(open + 1);
                    }
                    if trimmed_code.ends_with(';') || trimmed_code == "{" || trimmed_code == "}" {
                        break;
                    }
                }
            }
            if let Some(spaces) =
                self.contextual_ternary_colon_sibling_indent_spaces(current, previous)
            {
                return Some(spaces);
            }
            if starts_string_literal_token(current)
                && previous_code.contains("QStringList{")
                && has_unmatched_open_brace(previous_code)
                && let Some(open) = line_paren_imbalance(previous_code).1.last()
            {
                return Some(open + 1);
            }
            if starts_string_literal_token(current)
                && (previous_code.ends_with('+')
                    || (previous_code.ends_with('"')
                        && unmatched_open_paren_column(previous_code).is_some()
                        && (previous_code.contains("\" + ") || previous_code.contains("+ \""))))
                && let Some(string_start) = first_string_literal_start(previous_code)
            {
                return Some(string_start);
            }
            if let Some(spaces) = self.stream_after_string_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.stream_after_closed_brace_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.contextual_stream_brace_indent_spaces(current) {
                return Some(spaces);
            }
            if let Some(spaces) = self.previous_leading_stream_frame_indent_spaces(current) {
                return Some(spaces);
            }
            if previous_code.ends_with('=')
                && !previous_code.ends_with("==")
                && !previous_code.ends_with("!=")
                && !previous_code.ends_with("<=")
                && !previous_code.ends_with(">=")
                && !previous_code.contains('(')
            {
                let in_parameter_list = self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .take_while(|line| {
                        let trimmed = line.trim_end();
                        !trimmed.ends_with(';') && trimmed != "{" && trimmed != "}"
                    })
                    .any(|line| unmatched_open_paren_column(line.trim_end()).is_some());
                if in_parameter_list && !current.starts_with(['#', '{']) {
                    return Some(leading_visual_width(previous, tab_width) + width);
                }
            }
            if let Some(spaces) = self.argument_after_split_new_call_opener_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.immediate_macro_call_opener_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.over_max_new_call_argument_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.closed_over_max_new_call_sibling_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.preprocessor_branch_new_call_fallback_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) = self.split_new_call_owner_indent_spaces(line) {
                return Some(spaces);
            }
            if !current.starts_with(['#', '(', ')', '{', '}'])
                && previous_code.ends_with(',')
                && previous_code.contains('{')
                && current.contains('{')
                && let Some(member_base) = self.constructor_initializer_base_indent_spaces()
            {
                let previous_indent = leading_visual_width(previous, tab_width);
                if !has_unmatched_open_brace(previous_code) {
                    let inside_member_brace =
                        self.output.iter().rev().skip(1).take(16).any(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            !code.ends_with(';') && has_unmatched_open_brace(code)
                        });
                    let target = if inside_member_brace {
                        previous_indent
                    } else {
                        member_base
                    };
                    return Some(target + self.line_adjuster.total_case_unindent_depth() * width);
                }
                let open_indent = previous_code.find('{').map_or(0, |open| open + 1);
                let target = if previous_indent > member_base {
                    previous_indent
                } else {
                    previous_indent.max(open_indent)
                };
                return Some(target + self.line_adjuster.total_case_unindent_depth() * width);
            }
            if let Some(spaces) = self.split_new_call_sibling_indent_spaces(line) {
                return Some(spaces);
            }
            if let Some(spaces) =
                self.contextual_ternary_argument_sibling_indent_spaces(current, previous)
            {
                return Some(spaces);
            }
            if let Some(spaces) = self.macro_call_sibling_fallback_indent_spaces(line) {
                return Some(spaces);
            }
            if current.starts_with("==")
                && previous_trimmed.contains("ASSERT")
                && let Some(open) = unmatched_open_paren_column(previous.trim_end())
            {
                return Some(open + 1);
            }
            if let Some(spaces) = self.contextual_ternary_arm_indent_spaces(line, previous) {
                return Some(spaces);
            }
            if previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
                && let Some(previous_index) = self
                    .output
                    .iter()
                    .rposition(|line| std::ptr::eq(line, previous))
                && let Some(before_previous) = self.output[..previous_index]
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                && (before_previous.trim_start().starts_with("while ")
                    || before_previous.trim_start().starts_with("while(")
                    || before_previous.trim_start().starts_with("for ")
                    || before_previous.trim_start().starts_with("for("))
                && self.output[..previous_index]
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != before_previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| line.trim() == "else")
            {
                return Some(open + 1);
            }
            if !current.starts_with([')', '}'])
                && previous_code.ends_with(',')
                && previous_trimmed.starts_with(':')
                && let Some(open) = unmatched_open_paren_column(previous_code)
                && open + 1 > self.options.max_continuation_indent
            {
                return Some(leading_visual_width(previous, tab_width) + width * 2);
            }
            if !current.starts_with([')', '}'])
                && previous_code.ends_with(',')
                && (self.in_initializer_brace()
                    || self.innermost_init_block_brace()
                    || self.current_inline_array_column().is_some()
                    || self.output_has_open_initializer_brace())
                && let Some(open) = unmatched_open_paren_column(previous_code)
                && open + 1 > self.options.max_continuation_indent
            {
                return Some(leading_visual_width(previous, tab_width) + width * 2);
            }
            if !current.starts_with([')', '}'])
                && let Some(open) = unmatched_open_paren_column(previous_code)
                && open > self.options.max_continuation_indent
            {
                let previous_indent = leading_visual_width(previous, tab_width);
                let head = previous_code[..open].trim_start();
                if previous_indent > self.state.indent() * width && is_macro_like_word(head) {
                    return Some(previous_indent + width * 2);
                }
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && previous_trimmed.starts_with("return ")
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                return Some(open + 1);
            }
            if current.starts_with('.')
                && !current.starts_with("...")
                && previous_code.contains('=')
                && previous_code.trim_end().ends_with(')')
                && let Some(eq) = previous_code.find('=')
            {
                if self.options.indent_after_parens {
                    return Some(natural);
                }
                let value_start = previous_code[eq + 1..]
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(previous_code.len(), |(offset, _)| eq + 1 + offset);
                return Some(visual_width_from(
                    &previous_code[..value_start],
                    0,
                    tab_width,
                ));
            }
            if (current.starts_with('.') || current.starts_with("->"))
                && !current.starts_with("...")
                && !previous_is_preprocessor
                && !previous_trimmed.starts_with("return ")
                && !previous_trimmed.starts_with('}')
                && previous_code.trim_end().ends_with(')')
                && unmatched_open_paren_column(previous_code).is_none()
            {
                for index in (0..self.output.len()).rev().take(8) {
                    let line = &self.output[index];
                    let trimmed = self.output.code_trimmed(index);
                    if trimmed.starts_with("return ") {
                        return Some(leading_visual_width(line, tab_width) + "return ".len());
                    }
                    if trimmed.ends_with(';') || trimmed == "{" || trimmed == "}" {
                        break;
                    }
                }
            }
            if current.starts_with("return ")
                && previous_trimmed.starts_with("inline ")
                && previous_trimmed.ends_with('{')
                && leading_visual_width(previous, tab_width) == 0
            {
                return Some(width);
            }
            if let Some(spaces) =
                self.post_ternary_colon_comma_sibling_indent_spaces(current, previous)
            {
                return Some(spaces);
            }
            if previous_code.ends_with(',')
                && previous_code.contains('=')
                && current.contains('=')
                && !previous_code.trim_start().starts_with('.')
                && !current.starts_with(['.', '[', '*', '&'])
                && current.starts_with("m_")
                && !current.contains(')')
                && let Some(eq) = previous_code.find('=')
            {
                let before_value = previous_code[..eq].trim_end();
                let name_start = before_value
                    .rfind(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
                    .map_or(0, |index| index + 1);
                return Some(visual_width_from(
                    &previous_code[..name_start],
                    0,
                    tab_width,
                ));
            }
            if previous_code.ends_with(',')
                && !current.starts_with(['.', '[', '*', '&'])
                && let Some(previous_eq) = previous_code.find('=')
                && let Some(current_eq) = current.find('=')
                && let Some(arrow) = previous_code[..previous_eq].find("->")
                && current[..current_eq].contains("->")
                && unmatched_open_paren_column(previous_code).is_none()
            {
                return Some(visual_width_from(&previous_code[..arrow + 2], 0, tab_width));
            }
            if current.starts_with('(')
                && (previous_trimmed.starts_with("static_cast<")
                    || previous_trimmed.starts_with("const_cast<")
                    || previous_trimmed.starts_with("dynamic_cast<")
                    || previous_trimmed.starts_with("reinterpret_cast<"))
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            let previous_name = previous_code.split(':').next().unwrap_or_default().trim();
            let previous_previous_ends_question = self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line.trim_end().ends_with('?'));
            let current_code = current[..trailing_comment_split_limit(current)].trim_end();
            if previous_code.contains(':')
                && !previous_code.contains("::")
                && !previous_name.contains(char::is_whitespace)
                && previous_code.ends_with(',')
                && !previous_code.contains('?')
                && !previous_previous_ends_question
                && current_code.contains(':')
                && !current_code.contains('?')
                && !current_code.starts_with(['"', '\''])
                && !current_code.contains("::")
                && (current_code.ends_with(',') || current_code.ends_with(';'))
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if current.starts_with(':')
                && previous_trimmed.starts_with("//")
                && let Some(signature) = self.output.iter().rev().skip(1).find(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with("//")
                })
            {
                let signature_code =
                    signature[..trailing_comment_split_limit(signature)].trim_end();
                if signature_code.ends_with(')')
                    && signature_code.contains('(')
                    && !signature_code.trim_start().starts_with('?')
                    && !signature_code.contains('?')
                {
                    return Some(leading_visual_width(signature, tab_width) + width);
                }
            }
            if previous_trimmed.starts_with("//") {
                let previous_indent = leading_visual_width(previous, tab_width);
                let after_initializer_comment = current.contains('(')
                    && self
                        .output
                        .iter()
                        .rev()
                        .skip(1)
                        .find(|line| {
                            let trimmed = line.trim_start();
                            !trimmed.is_empty() && !trimmed.starts_with("//")
                        })
                        .is_some_and(|line| line.trim_end().ends_with(':'));
                let after_comma_comment = self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !trimmed.starts_with("//")
                    })
                    .is_some_and(|line| {
                        line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .ends_with(',')
                    });
                if after_comma_comment
                    && previous_indent > natural
                    && !current.starts_with([')', '}', ';'])
                    && !current.starts_with("//")
                    && !is_lambda_body_header(current)
                {
                    return Some(previous_indent);
                }
                if natural < previous_indent
                    && !current.starts_with(['(', ')', '{', '}', ';'])
                    && !is_lambda_body_header(current)
                    && (current.starts_with(['*', '&', '+', '-'])
                        || starts_with_chain_operator(current)
                        || self.stack_state.paren_depth > 0
                        || after_initializer_comment)
                    && !(starts_with_chain_operator(current) && current.trim_end().ends_with('{'))
                {
                    return Some(previous_indent);
                }
            }
            if current.starts_with("//")
                && let Some(previous_indent) = self.line_comment_continuation_anchor_column()
                && natural < previous_indent
            {
                return Some(previous_indent);
            }
            if current.starts_with(',') && has_inline_constructor_initializer_colon(previous_code) {
                return Some(leading_visual_width(previous, tab_width) + width);
            }
            if current.starts_with('*')
                && previous_code.trim_start().starts_with('*')
                && previous_code.ends_with(',')
                && previous_code.contains('=')
                && !previous_code.contains("= new ")
                && unmatched_open_paren_column(previous_code).is_none()
            {
                return Some(leading_visual_width(previous, tab_width) + 1);
            }
            if !self.options.indent_after_parens
                && previous_code.ends_with(',')
                && !current.starts_with(['.', '{', '}', ')', '#', '?', ':'])
            {
                let mut open_info = None;
                for (idx, line) in self.output.iter().enumerate().rev() {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    if code.ends_with(';') || code.contains('{') || code.contains('}') {
                        break;
                    }
                    if let Some(open) = unmatched_open_paren_column(code) {
                        open_info = Some((idx, open));
                        break;
                    }
                }
                if let Some((idx, open)) = open_info {
                    let target = open + 1;
                    let before_open = self.output[..idx].iter().rev();
                    let mut saw_blank = false;
                    let mut after_ternary = false;
                    for line in before_open {
                        if line.trim().is_empty() {
                            saw_blank = true;
                            continue;
                        }
                        after_ternary = saw_blank && line.trim_start().starts_with(':');
                        break;
                    }
                    if after_ternary && natural < target {
                        return Some(target);
                    }
                }
            }
            if has_inline_constructor_initializer_colon(previous_code)
                && previous_code.ends_with('(')
            {
                return Some(leading_visual_width(previous, tab_width) + width * 2);
            }
            if has_inline_constructor_initializer_colon(previous_code)
                && previous_code.ends_with(',')
                && previous[trailing_comment_split_limit(previous)..].contains("//")
            {
                return Some(natural + width);
            }
            if previous[trailing_comment_split_limit(previous)..].contains("//")
                && previous_code.ends_with(',')
                && self.output.iter().rev().take(8).skip(1).any(|line| {
                    let limit = trailing_comment_split_limit(line);
                    let code = line[..limit].trim_end();
                    line[limit..].contains("//")
                        && has_inline_constructor_initializer_colon(code)
                        && code.ends_with(',')
                })
            {
                return Some(leading_visual_width(previous, tab_width));
            }
            if current.chars().next().is_some_and(is_identifier_start)
                && current.contains('=')
                && let Some(spaces) = self.enum_member_missing_comma_indent_spaces(previous)
            {
                return Some(spaces);
            }
            if let Some(spaces) = self.immediate_typedef_template_indent_spaces(current, previous) {
                return Some(spaces);
            }
        }
        if let Some(spaces) = self.split_class_head_indent_spaces(current) {
            return Some(spaces);
        }
        if previous_is_preprocessor {
            return None;
        }
        if let Some(spaces) = self.typedef_template_context_indent_spaces(current) {
            return Some(spaces);
        }
        if let Some(spaces) = self.typedef_function_pointer_frame_indent_spaces(current) {
            return Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_context_indent(current, natural) {
            return Some(spaces);
        }
        if let Some(spaces) = self.split_constructor_member_call_indent(current) {
            return Some(spaces);
        }
        None
    }
}
