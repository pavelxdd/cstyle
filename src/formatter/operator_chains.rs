use super::FormatEngine;
use super::columns::{leading_visual_width, visual_width_from};
use super::compound_literals::line_ends_compound_literal_cast;

use super::frame::{ColonRole, FrameStack, ParenRole, TernaryOwnerRole};
use super::headers::is_braceless_header_line;
use super::headers::{is_conditional_header_line, line_is_control_body_header, starts_header_word};
use super::indentation::LineKind;

use super::line_scan::is_comment_line;
use super::line_scan::{
    line_paren_imbalance, trailing_comment_split_limit, unmatched_open_paren_column,
    unmatched_open_paren_columns,
};
use super::literals::{starts_string_literal_token, string_literal_token_end};
use super::operators::{
    find_assignment_operator, head_ends_binary_operator, starts_ternary_arm,
    starts_with_chain_operator,
};

use crate::config::{BraceStyle, MinConditionalIndent};
use crate::source::lex::is_identifier_start;

pub(super) enum ReadyOperatorChainLine {
    Single(String),
    SplitTernary { colon: String, tail: String },
}

pub(super) fn starts_operator_chain_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    starts_with_chain_operator(trimmed) || starts_ternary_arm(trimmed)
}

pub(super) fn clear_operator_chain_frames(frame_stack: &mut FrameStack) {
    frame_stack.clear_stream_frames();
    frame_stack.clear_logical_frames();
}

pub(super) fn clear_logical_chain_indent(logical_chain_indent_spaces: &mut Option<usize>) {
    *logical_chain_indent_spaces = None;
}

pub(super) fn clear_operator_chain_state(
    frame_stack: &mut FrameStack,
    logical_chain_indent_spaces: &mut Option<usize>,
) {
    clear_operator_chain_frames(frame_stack);
    clear_logical_chain_indent(logical_chain_indent_spaces);
}

pub(super) fn clear_stream_frames_and_logical_indent(
    frame_stack: &mut FrameStack,
    logical_chain_indent_spaces: &mut Option<usize>,
) {
    frame_stack.clear_stream_frames();
    clear_logical_chain_indent(logical_chain_indent_spaces);
}

impl FormatEngine<'_> {
    pub(super) fn ready_embedded_preprocessor_return_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.chars().next().is_some_and(is_identifier_start) || !self.output.may_have_hash()
        {
            return None;
        }
        let previous_index = (0..self.output.len())
            .rev()
            .find(|&index| !self.output.trimmed(index).is_empty())?;
        let previous_code = self.output.code(previous_index);
        if !self
            .output
            .code_trimmed(previous_index)
            .starts_with("return ")
            || !previous_code.contains('#')
            || previous_code.ends_with(';')
        {
            return None;
        }
        let spaces = self
            .output
            .lead_width(previous_index, self.options.tab_width)
            + "return ".len();
        (leading_visual_width(line, self.options.tab_width) < spaces).then_some(spaces)
    }

    pub(super) fn replayed_header_operator_indent_spaces(
        &self,
        line: &str,
        delimiter_owner: Option<usize>,
    ) -> Option<usize> {
        if self.options.max_code_length.is_none()
            || !(line.trim_start().starts_with("&&") || line.trim_start().starts_with("||"))
            || !self.output.last_non_empty_line().is_some_and(|previous| {
                is_conditional_header_line(
                    previous[..trailing_comment_split_limit(previous)].trim_start(),
                )
            })
        {
            return None;
        }
        let owner = delimiter_owner?;
        (owner == self.token_input.input_source_indent).then_some(owner)
    }

    pub(super) fn maximum_length_return_chain_indent_spaces(
        &self,
        line: &str,
        kind: LineKind,
    ) -> Option<usize> {
        let current = line.trim_start();
        if kind != LineKind::Normal
            || self.options.max_code_length.is_none()
            || self.options.indent_after_parens
            || !starts_with_chain_operator(current)
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !starts_header_word(previous_trimmed, "return") {
            return None;
        }
        let after_return = &previous_trimmed["return".len()..];
        let value_offset = "return".len()
            + after_return
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(after_return.len(), |(index, _)| index);
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + visual_width_from(&previous_trimmed[..value_offset], 0, self.options.tab_width),
        )
    }

    pub(super) fn first_ordinary_ternary_arm_indent_spaces(
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
            || !self.output.may_have_question()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.contains('?')
            && !previous_code.trim_start().starts_with('#')
            && !previous_code.contains(':')
            && !previous_code.ends_with(';'))
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn ordinary_ternary_colon_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        normal_indent: usize,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with(':') {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let mut spaces = previous.trim_start().starts_with('?').then(|| {
            leading_visual_width(previous, self.options.tab_width)
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        });
        if line_kind == LineKind::Normal
            && !current.starts_with("::")
            && self.stack_state.paren_depth == 0
            && previous_code.contains('?')
            && !previous_code.contains('<')
            && find_assignment_operator(previous_code).is_none()
            && self
                .assignment_rhs_first_line_indent(previous, true)
                .is_none()
            && !previous_code.trim_start().starts_with("return ")
            && unmatched_open_paren_column(previous_code).is_none()
        {
            spaces = Some(normal_indent * self.options.indent_width);
        }
        spaces
    }

    pub(super) fn completed_ternary_call_sibling_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['?', ':']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with("),")
            || !previous_code.contains('?')
            || !previous_code.contains(':')
        {
            return None;
        }
        let open = unmatched_open_paren_column(previous_code)?;
        let padding = previous_code
            .chars()
            .skip(open + 1)
            .take_while(|ch| ch.is_whitespace())
            .collect::<String>();
        Some(open + 1 + visual_width_from(&padding, open + 1, self.options.tab_width))
    }

    pub(super) fn operand_after_question_row_indent_spaces(
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
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        (previous_trimmed != "?"
            && previous_trimmed.starts_with('?')
            && previous_trimmed.ends_with('?'))
        .then(|| {
            leading_visual_width(previous, self.options.tab_width)
                + visual_width_from(previous_trimmed, 0, self.options.tab_width)
                + self.options.indent_width
                + 1
        })
    }

    pub(super) fn leading_operator_after_ternary_colon_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ])
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.contains('?') && previous_code.ends_with(':'))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn scoped_ternary_continuation_indent_spaces(&self, line: &str) -> Option<usize> {
        if line.trim_start().starts_with(['}', '#'])
            || !line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.contains('?')
            && previous_code.contains("::")
            && unmatched_open_paren_column(previous_code).is_none())
        .then(|| self.state.indent() * self.options.indent_width)
    }

    pub(super) fn allman_operator_or_preprocessor_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        normal_indent: usize,
        header_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        let current = line.trim_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::Allman
            || current.starts_with(['{', '}', '#'])
            || current.starts_with("//")
            || current.starts_with("/*")
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let current_starts_operator = current.starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]);
        let previous_starts_operator = previous_trimmed.starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]);
        if current_starts_operator && previous_starts_operator {
            if self.preprocessor.split_else.extra_indent
                && (current.starts_with("&&") || current.starts_with("||"))
                && let Some(spaces) = header_indent_spaces
            {
                return Some(spaces);
            }
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            return Some(match find_assignment_operator(previous_trimmed) {
                Some((0, operator)) => {
                    let after = &previous_trimmed[operator.len()..];
                    let operand = operator.len() + (after.len() - after.trim_start().len());
                    previous_indent
                        + visual_width_from(&previous_trimmed[..operand], 0, self.options.tab_width)
                }
                _ => previous_indent,
            });
        }
        if current_starts_operator && previous_code.trim() == "{" {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if previous_code.contains('#')
            && !previous_trimmed.starts_with(['#', '/'])
            && !previous_trimmed.starts_with("return ")
        {
            let mut spaces = normal_indent * self.options.indent_width;
            if self.state.indent() > 1
                && previous_code.contains("#else")
                && !current_starts_operator
            {
                spaces = spaces.saturating_sub(self.options.indent_width);
            } else if self.state.indent() > 1
                && previous_starts_operator
                && current.chars().next().is_some_and(is_identifier_start)
            {
                spaces = self.options.indent_width;
            }
            return Some(spaces);
        }
        None
    }

    pub(super) fn stream_after_closed_or_inline_row_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !(current.starts_with("<<") || current.starts_with(">>"))
            || line[..trailing_comment_split_limit(line)]
                .trim_end()
                .ends_with('{')
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_trimmed = previous.trim_end();
        if !previous_trimmed.ends_with('}')
            && !((previous_trimmed.contains(" << ") || previous_trimmed.contains(" >> "))
                && unmatched_open_paren_column(previous_trimmed).is_none())
        {
            return None;
        }
        self.previous_stream_chain_indent_spaces()
    }

    pub(super) fn logical_continuation_after_commented_noexcept_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if line.trim_start().starts_with("//") || line.trim_start().starts_with('{') {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous.contains("//")
            || !previous_code.ends_with("&&")
            || !(0..self.output.len()).rev().take(8).any(|index| {
                let code = self.output.code(index);
                previous.contains("//") && code.contains("noexcept(") && code.ends_with('(')
            })
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn braceless_ternary_comma_sibling_indent_spaces(
        &self,
        previous_code: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !previous_code.ends_with(',') || !self.previous_statement_is_braceless_ternary() {
            return None;
        }
        let target = unmatched_open_paren_column(previous_code)?
            + 1
            + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        (current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(super) fn nested_ternary_colon_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if !current.starts_with(':') {
            return None;
        }
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.trim_start().starts_with(':')
            && previous_code.contains('?')
            && unmatched_open_paren_column(previous_code).is_none())
        .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn observe_operator_chain_line_context(
        &mut self,
        output_line_index: usize,
        code: &str,
    ) {
        let paren_imbalance = line_paren_imbalance(code);
        let unmatched_open_paren = unmatched_open_paren_column(code);
        self.frame_stack.mark_stream_line_context(
            output_line_index,
            code.ends_with("<<") || code.ends_with(">>"),
            code.contains("{ {"),
            unmatched_open_paren.is_some(),
            code.ends_with(')'),
            paren_imbalance.0 > 0,
        );
        self.frame_stack.mark_logical_line_context(
            output_line_index,
            unmatched_open_paren,
            code.ends_with(')'),
            paren_imbalance.0 > 0,
        );
        if self.line_state.ternary_colon || code.trim_start().starts_with(':') {
            self.frame_stack
                .mark_last_ternary_colon_output_line(output_line_index);
        }
        if self.frame_stack.has_open_ternary() {
            self.frame_stack
                .mark_line_ended_open_ternary(output_line_index);
        }
    }

    pub(super) fn observe_operator_chain_output_line(
        &mut self,
        output_line_index: usize,
    ) -> Option<usize> {
        let output_line = self.output.get(output_line_index)?;
        let output_indent = leading_visual_width(output_line, self.options.tab_width);
        self.frame_stack
            .mark_stream_line_output_indent(output_line_index, output_indent);
        self.frame_stack
            .mark_logical_line_output_indent(output_line_index, output_indent);
        self.observe_ternary_colon_output_line(output_line_index);
        Some(output_indent)
    }

    pub(super) fn observe_ternary_colon_output_line(&mut self, output_line_index: usize) {
        let starts_with_colon = self
            .output
            .get(output_line_index)
            .is_some_and(|output_line| {
                output_line[..trailing_comment_split_limit(output_line)]
                    .trim_start()
                    .starts_with(':')
            });
        if starts_with_colon {
            self.frame_stack
                .mark_last_ternary_colon_output_line(output_line_index);
        }
    }

    pub(super) fn postprocess_ready_operator_chain_line(
        &mut self,
        output_line_index: usize,
        line: String,
    ) -> ReadyOperatorChainLine {
        if line.trim_start().starts_with(':') {
            self.frame_stack
                .mark_last_ternary_colon_output_line(output_line_index);
        }
        if let Some((colon, tail)) = self.split_ternary_colon_after_chained_true_arm(&line) {
            ReadyOperatorChainLine::SplitTernary { colon, tail }
        } else {
            ReadyOperatorChainLine::Single(line)
        }
    }

    pub(super) fn stream_chain_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let frame = self.frame_stack.active_stream()?;
        frame
            .after_multiline_braced_operand
            .then_some(frame.chain_anchor_column)
    }

    pub(super) fn parenthesized_after_trailing_stream_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !line.trim_start().starts_with('(') {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if !stream.operator_ends_output_line {
            return None;
        }
        Some(
            if stream
                .operator_output_column
                .saturating_sub(stream.line_indent_spaces)
                > self.options.max_continuation_indent
            {
                stream.line_indent_spaces + self.options.indent_width * 2
            } else {
                stream.operator_output_column
            },
        )
    }

    pub(super) fn comment_separated_stream_indent_spaces(&self, line: &str) -> Option<usize> {
        if !starts_with_chain_operator(line.trim_start()) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !is_comment_line(previous.trim_start()) {
            return None;
        }
        let before_comment = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        if !starts_with_chain_operator(before_comment.trim_start()) {
            return None;
        }
        let before_comment_line = self
            .output
            .iter()
            .position(|line| line.as_str() == before_comment.as_str())?;
        self.frame_stack
            .active_stream_on_output_line(before_comment_line)?;
        Some(leading_visual_width(before_comment, self.options.tab_width))
    }

    pub(super) fn comment_separated_leading_operator_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !starts_with_chain_operator(line.trim_start()) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !is_comment_line(previous.trim_start()) {
            return None;
        }
        let before_comment = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        starts_with_chain_operator(before_comment.trim_start())
            .then(|| leading_visual_width(before_comment, self.options.tab_width))
    }

    pub(super) fn comment_terminated_logical_chain_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['#', '}', ':']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let split = trailing_comment_split_limit(previous);
        let previous_code = previous[..split].trim_end();
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        (split < previous.len()
            && previous_code.ends_with("||")
            && current_spaces.unwrap_or(0) < previous_indent)
            .then_some(previous_indent)
    }

    pub(super) fn maximum_length_logical_header_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.max_code_length.is_none()
            || !self.options.indent_after_parens
            || !(line.trim_start().starts_with("&&") || line.trim_start().starts_with("||"))
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let configured = (self.continuation_base_indent() + self.options.continuation_indent)
            * self.options.indent_width;
        (self.token_input.token_source_line_indent == configured
            && unmatched_open_paren_column(previous_code).is_some()
            && ["if", "for", "while", "switch"]
                .iter()
                .any(|header| starts_header_word(previous_trimmed, header)))
        .then_some(configured)
    }

    pub(super) fn trailing_stream_top_level_indent_spaces(
        &self,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with("<<") && !previous_code.ends_with(">>") {
            return None;
        }
        self.line_after_trailing_stream_operator_indent_spaces()
            .or_else(|| self.previous_stream_chain_indent_spaces())
    }

    pub(super) fn set_next_line_indent_after_ternary_colon(
        &mut self,
        line: &str,
        line_kind: LineKind,
        current_spaces: usize,
    ) {
        if line_kind != LineKind::Normal
            || !line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ])
        {
            return;
        }
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())
        else {
            return;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if previous_code.contains('?') && previous_code.ends_with(':') {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(current_spaces);
        }
    }

    pub(super) fn gnu_leading_operator_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        normal_indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let current = line.trim_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::Gnu
            || current.starts_with("//")
            || current.starts_with("/*")
            || !current.starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
            ])
        {
            return None;
        }
        if let Some(stream) = self.frame_stack.active_stream() {
            return Some(if self.options.indent_after_parens {
                (normal_indent + self.options.continuation_indent) * self.options.indent_width
            } else {
                stream.chain_anchor_column
            });
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_starts_operator = previous_code.trim_start().starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]);
        if previous_code.trim() == "{" {
            return self.frame_stack.active_brace().map(|frame| {
                frame.body_indent_column.min(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                )
            });
        }
        if previous_starts_operator {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if previous_code.ends_with(',') {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if previous_code.trim_start().starts_with('}')
            || current.starts_with('.') && find_assignment_operator(previous_code).is_some()
            || starts_with_chain_operator(current)
                && starts_header_word(previous_code.trim_start(), "return")
        {
            return None;
        }
        let target =
            leading_visual_width(previous, self.options.tab_width).max(self.options.indent_width);
        Some(current_spaces.unwrap_or(target).max(target))
    }

    pub(super) fn pico_leading_operator_after_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Pico
            || !line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
            ])
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        previous_code
            .ends_with('}')
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn stream_after_string_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let string = self
            .frame_stack
            .string_continuation_on_output_line(previous_line)?;
        (string.has_stream_context
            && string.literal_start_column == string.line_indent_spaces
            && !string.has_opening_context
            && !string.inside_delimiter_context)
            .then_some(
                string.line_indent_spaces
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            )
    }

    pub(super) fn stream_after_closed_brace_frame_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let brace = self.frame_stack.last_closed_brace()?;
        if brace.close_output_line != Some(previous_line) || !brace.close_ends_output_line {
            return None;
        }
        Some(self.frame_stack.active_stream()?.chain_anchor_column)
    }

    pub(super) fn previous_leading_stream_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        (stream.operator_output_column == stream.line_indent_spaces)
            .then_some(stream.line_indent_spaces)
    }

    pub(super) fn stream_after_ternary_colon_frame_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        if self.frame_stack.last_ternary_colon_output_line() != Some(previous_line) {
            return None;
        }
        Some(self.frame_stack.active_stream()?.chain_anchor_column)
    }

    pub(super) fn line_start_stream_adjacent_string_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !starts_string_literal_token(line.trim_start()) {
            return None;
        }
        let frame = self
            .frame_stack
            .string_continuation_before_output_line(self.output.len())?;
        if !frame.line_starts_with_chain_operator || !frame.has_opening_context {
            return None;
        }
        let current = line.trim_start();
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if string_literal_token_end(current, 0).is_some_and(|end| {
            let rest = current[end..].trim_start();
            rest.starts_with("<<") || rest.starts_with(">>")
        }) {
            return Some(frame.line_indent_spaces + case_unindent);
        }
        Some(frame.literal_start_column + case_unindent)
    }

    pub(super) fn line_after_trailing_stream_operator_indent_spaces(&self) -> Option<usize> {
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if !stream.operator_ends_output_line {
            return None;
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if let Some(delimiter) = self.frame_stack.active_delimiter()
            && delimiter.opener_output_line < previous_line
            && stream.line_indent_spaces == delimiter.opener_output_column + 1
        {
            return Some(stream.line_indent_spaces + case_unindent);
        }
        if let Some(string) = self
            .frame_stack
            .string_continuation_before_output_line(self.output.len())
            && string.output_line == previous_line
            && string.literal_start_column == stream.line_indent_spaces
        {
            return Some(stream.line_indent_spaces + case_unindent);
        }
        if let Some(delimiter) = self.frame_stack.active_delimiter()
            && delimiter.opener_output_line == previous_line
            && delimiter.opener_output_column < stream.operator_output_column
        {
            return Some(delimiter.opener_output_column + 1 + case_unindent);
        }
        None
    }

    pub(super) fn string_after_stream_string_indent_spaces(&self) -> Option<usize> {
        if self.in_initializer_brace() || self.current_inline_array_column().is_some() {
            return None;
        }
        let string = self
            .frame_stack
            .string_continuation_before_output_line(self.output.len())?;
        let stream = self
            .frame_stack
            .first_stream_on_output_line(string.output_line)?;
        if stream.operator_output_column >= string.literal_start_column
            || string.has_open_brace_before_literal
        {
            return None;
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let delimiter_count = self
            .frame_stack
            .delimiter_count_after_output_column(string.output_line, stream.operator_output_column);
        if delimiter_count >= 2 {
            let max_column = string.line_indent_spaces + self.options.max_continuation_indent;
            let spaces = self
                .frame_stack
                .last_delimiter_column_after_output_column(
                    string.output_line,
                    stream.operator_output_column,
                )
                .filter(|column| *column <= max_column)
                .unwrap_or(string.line_indent_spaces + self.options.indent_width * 2);
            return Some(spaces + case_unindent);
        }
        let spaces = if stream
            .operator_output_column
            .saturating_sub(string.line_indent_spaces)
            > self.options.max_continuation_indent
        {
            string.line_indent_spaces + self.options.indent_width * 2
        } else {
            stream.operator_output_column
        };
        Some(spaces + case_unindent)
    }

    pub(super) fn ternary_operator_sibling_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '?', ':', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !head_ends_binary_operator(previous_code) {
            return None;
        }
        let mut question_indent = None;
        for raw in self.output.iter().rev().take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            if code.contains('?') {
                question_indent = Some(leading_visual_width(raw, self.options.tab_width));
            }
            if find_assignment_operator(code).is_some() {
                let base = self.continuation_base_indent() * self.options.indent_width;
                return question_indent.filter(|spaces| *spaces > base);
            }
            if code.ends_with([';', '{', '}']) {
                return None;
            }
        }
        None
    }

    pub(super) fn assignment_ternary_branch_after_colon_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '?', ':', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(':') {
            return None;
        }
        let mut branch_indent = Some(leading_visual_width(previous, self.options.tab_width));
        for raw in self.output.iter().rev().skip(1).take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if code.contains('?')
                && find_assignment_operator(code).is_some()
                && !code.ends_with(';')
            {
                return branch_indent;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
            branch_indent = Some(leading_visual_width(raw, self.options.tab_width));
        }
        None
    }

    pub(super) fn return_ternary_branch_after_colon_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '?', ':', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(':') {
            return None;
        }
        let mut branch_indent = Some(leading_visual_width(previous, self.options.tab_width));
        for raw in self.output.iter().rev().skip(1).take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("return ") && trimmed.contains('?') && !code.ends_with(';') {
                return branch_indent;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
            branch_indent = Some(leading_visual_width(raw, self.options.tab_width));
        }
        None
    }

    pub(super) fn return_ternary_call_argument_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '?', ':', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with('(') {
            return None;
        }
        for raw in self.output.iter().rev().skip(1).take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("return ") && trimmed.contains('?') {
                return Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
        }
        None
    }

    pub(super) fn ternary_call_clear_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || !line.trim_end().ends_with(");") {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let in_preprocessor_else_context = self.output.iter().rev().take(128).any(|line| {
            let trimmed = line[..trailing_comment_split_limit(line)]
                .trim_end()
                .trim_start();
            trimmed == "else" || trimmed.ends_with("} else")
        }) && self
            .output
            .iter()
            .rev()
            .take_while(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                !(leading_visual_width(line, self.options.tab_width) == 0
                    && code.ends_with('{')
                    && !code.trim_start().starts_with('#'))
            })
            .take(128)
            .any(|line| line.trim_start().starts_with('#'));
        (in_preprocessor_else_context
            && previous_code.ends_with(',')
            && unmatched_open_paren_column(previous_code).is_some())
        .then_some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn recent_ternary_argument_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let opens_compound_literal = current
            .rsplit_once('{')
            .is_some_and(|(prefix, _)| line_ends_compound_literal_cast(prefix.trim_end()));
        if !previous_code.ends_with(',')
            || current.starts_with(['#', '(', ')', '{', '}'])
            || opens_compound_literal
            || unmatched_open_paren_column(previous_code).is_some()
        {
            return None;
        }
        let mut saw_colon_argument = false;
        let has_recent_ternary_argument = (0..self.output.len()).rev().take(16).any(|index| {
            let code = self.output.code(index);
            let trimmed_code = self.output.code_trimmed(index);
            if trimmed_code.starts_with(':') && code.ends_with(',') {
                saw_colon_argument = true;
                false
            } else {
                saw_colon_argument && trimmed_code.contains('?')
            }
        });
        has_recent_ternary_argument.then_some(
            leading_visual_width(previous, self.options.tab_width)
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(super) fn contextual_ternary_argument_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if current.starts_with(['#', '(', ')', '{', '}'])
            || !previous_code.ends_with(',')
            || !current.ends_with(");")
            || unmatched_open_paren_column(previous_code).is_some()
        {
            return None;
        }
        let follows_ternary_argument = self
            .output
            .iter()
            .rev()
            .skip(1)
            .take_while(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                !(code.ends_with(';') || code == "{" || code == "}")
            })
            .any(|line| line[..trailing_comment_split_limit(line)].contains('?'));
        follows_ternary_argument.then_some(
            leading_visual_width(previous, self.options.tab_width)
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(super) fn assignment_rhs_first_line_indent(
        &self,
        previous: &str,
        require_question: bool,
    ) -> Option<usize> {
        let tab_width = self.options.tab_width;
        let mut candidate_indent = leading_visual_width(previous, tab_width);
        let mut saw_question = previous.contains('?');
        for line in self.output.iter().rev().skip(1) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            if code.ends_with('=')
                && !code.ends_with("==")
                && !code.ends_with("!=")
                && !code.ends_with("<=")
                && !code.ends_with(">=")
            {
                return (!require_question || saw_question).then_some(candidate_indent);
            }
            if code.ends_with(';') || code == "{" || code == "}" || trimmed.ends_with(':') {
                return None;
            }
            if trimmed.starts_with('?') {
                saw_question = true;
            }
            candidate_indent = leading_visual_width(line, tab_width);
        }
        None
    }

    pub(super) fn contextual_ternary_arm_indent_spaces(
        &self,
        line: &str,
        previous: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let tab_width = self.options.tab_width;
        let width = self.options.indent_width;
        if current.starts_with('?')
            && previous_trimmed.starts_with("return ")
            && self.recent_base_trailing_return_function_header()
        {
            return Some(leading_visual_width(previous, tab_width));
        }
        if current.starts_with('?')
            && (previous_trimmed.starts_with('(') || previous_trimmed.starts_with("return ("))
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            return Some(open + 1);
        }
        if current.starts_with('?')
            && previous_code.ends_with(')')
            && let Some(call_indent) = self
                .output
                .iter()
                .rev()
                .skip(1)
                .take_while(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    !(code.ends_with(';')
                        || code == "{"
                        || code == "}"
                        || (code.ends_with('=')
                            && !code.ends_with("==")
                            && !code.ends_with("!=")
                            && !code.ends_with("<=")
                            && !code.ends_with(">=")))
                })
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    code.ends_with('(')
                        .then(|| leading_visual_width(line, tab_width))
                })
        {
            return Some(call_indent);
        }
        if (current.starts_with(": ") || current == ":")
            && previous_code.contains('?')
            && !previous_code.trim_start().starts_with("return ")
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            return Some(
                open + 1
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        if let Some(spaces) =
            self.return_ternary_colon_after_multiline_template_declaration_indent_spaces(line)
        {
            return Some(spaces);
        }
        if (current.starts_with(": ") || current == ":")
            && previous_code.contains('?')
            && !previous_code.trim_start().starts_with("return ")
            && (self.stack_state.paren_depth > 0
                || self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .take_while(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        !(code.ends_with(';') || code == "{" || code == "}")
                    })
                    .any(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        code.ends_with('(') || unmatched_open_paren_column(code).is_some()
                    }))
        {
            return Some(
                leading_visual_width(previous, tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * width,
            );
        }
        if let Some(spaces) = self.ternary_arm_frame_indent_spaces(current) {
            return Some(spaces);
        }
        let ternary_arm = if current.starts_with('?') {
            Some(false)
        } else if current.starts_with(": ") || current == ":" {
            Some(true)
        } else {
            None
        };
        if let Some(require_question) = ternary_arm
            && let Some(indent) = self.assignment_rhs_first_line_indent(previous, require_question)
        {
            return Some(indent);
        }
        if !current.starts_with([')', '}', '?', ':'])
            && previous_code.ends_with(':')
            && !previous_code.contains('?')
            && self
                .output
                .iter()
                .rev()
                .skip(1)
                .take_while(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    !(code.ends_with(';') || code == "{" || code == "}")
                })
                .any(|line| line[..trailing_comment_split_limit(line)].contains('?'))
            && self
                .return_ternary_branch_after_colon_indent_spaces(line)
                .is_none()
        {
            return Some(
                leading_visual_width(previous, tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * width,
            );
        }
        if !current.starts_with([')', '}', '?', ':'])
            && previous_code.ends_with(':')
            && previous_code.contains('?')
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            return Some(
                open + 1
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        None
    }

    pub(super) fn return_chain_indent_spaces(
        &self,
        current: &str,
        previous: &str,
        natural: usize,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !starts_with_chain_operator(current) || !starts_header_word(previous_trimmed, "return") {
            return None;
        }
        if self.options.indent_after_parens {
            return Some(natural);
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if let Some(open) = unmatched_open_paren_column(previous_code) {
            return Some(open + 1 + case_unindent);
        }
        let after_return = &previous_trimmed["return".len()..];
        let value_offset = "return".len()
            + after_return
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(after_return.len(), |(index, _)| index);
        Some(leading_visual_width(previous, self.options.tab_width) + value_offset + case_unindent)
    }

    pub(super) fn inline_stream_opener_argument_indent_spaces(
        &self,
        current: &str,
        previous_code: &str,
    ) -> Option<usize> {
        if current.starts_with(['#', '(', ')', '{', '}']) || !previous_code.ends_with('(') {
            return None;
        }
        previous_code
            .find(" << ")
            .or_else(|| previous_code.find(" >> "))
            .map(|operator_start| operator_start + 5)
    }

    pub(super) fn contextual_ternary_colon_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if let Some(spaces) = self.nested_ternary_colon_sibling_indent_spaces(current, previous) {
            return Some(spaces);
        }
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        (current.starts_with(':') && previous_code.trim_start().starts_with('?')).then_some(
            previous_indent
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(super) fn contextual_stream_brace_indent_spaces(&self, current: &str) -> Option<usize> {
        let width = self.options.indent_width;
        if (current.starts_with("<<") || current.starts_with(">>"))
            && current.contains('{')
            && !current.contains('}')
            && self
                .output
                .len()
                .checked_sub(1)
                .is_some_and(|previous_line| {
                    self.frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_some_and(|stream| {
                            stream.operator_output_column != stream.line_indent_spaces
                        })
                })
        {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>"))
            && current.trim_end().ends_with('{')
            && !current.contains('}')
        {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>")) && current.contains("{ {") {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>"))
            && self
                .output
                .len()
                .checked_sub(1)
                .is_some_and(|previous_line| {
                    self.frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_some_and(|stream| stream.line_contains_nested_brace)
                })
        {
            return Some(self.continuation_base_indent() * width + width * 2);
        }
        None
    }

    pub(super) fn line_follows_logical_operator(&self) -> bool {
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.ends_with("||") || code.ends_with("&&")
            })
    }

    pub(super) fn operator_chain_owns_continuation(&self, line: &str) -> bool {
        let follows_stream_operator = self.output.last_non_empty_line().is_some_and(|previous| {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            code.ends_with("<<") || code.ends_with(">>")
        });
        self.frame_stack.active_ternary().is_some()
            || self.line_follows_logical_operator()
            || follows_stream_operator
            || starts_with_chain_operator(line.trim_start())
    }

    pub(super) fn line_follows_preprocessor_guarded_header_body(&self) -> bool {
        let mut saw_preprocessor = false;
        for line in self.output.iter().rev() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('#') {
                saw_preprocessor = true;
                continue;
            }
            return saw_preprocessor
                && is_braceless_header_line(trimmed)
                && !trimmed.ends_with('{');
        }
        false
    }

    pub(super) fn header_operator_continuation_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("&&") || trimmed.starts_with("||")) {
            return None;
        }
        if !self.split_else_body_indent_active()
            && self
                .frame_stack
                .active_delimiter()
                .is_some_and(|delimiter| {
                    delimiter.role == ParenRole::Header
                        && delimiter.opener_output_line + 1 == self.output.len()
                })
        {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let previous_header = previous_trimmed
            .strip_prefix('}')
            .map(str::trim_start)
            .unwrap_or(previous_trimmed);
        if self.split_else_body_indent_active()
            && (previous_trimmed.starts_with("&&") || previous_trimmed.starts_with("||"))
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let (closes, opens) = line_paren_imbalance(previous_code);
            if !opens.is_empty() {
                return Some(previous_indent + self.options.indent_width);
            }
            if closes > 0
                && let Some(sibling) = self.output.iter().rev().skip(1).take(16).find(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    (trimmed.starts_with("&&") || trimmed.starts_with("||"))
                        && leading_visual_width(line, self.options.tab_width) < previous_indent
                })
            {
                return Some(leading_visual_width(sibling, self.options.tab_width));
            }
            return Some(previous_indent);
        }
        if !(starts_header_word(previous_header, "if")
            || starts_header_word(previous_header, "while")
            || previous_header.starts_with("else if"))
        {
            return None;
        }
        unmatched_open_paren_column(previous_code).map(|column| {
            let base = leading_visual_width(previous, self.options.tab_width);
            let paren_indent = column
                + if previous_header.starts_with("else if(") {
                    2
                } else {
                    1
                };
            if self.split_else_body_indent_active() {
                let starts_nested_group = previous_header
                    .strip_prefix("else if")
                    .or_else(|| previous_header.strip_prefix("if"))
                    .or_else(|| previous_header.strip_prefix("while"))
                    .or_else(|| previous_header.strip_prefix("for"))
                    .or_else(|| previous_header.strip_prefix("switch"))
                    .is_some_and(|tail| tail.trim_start().starts_with("( ("));
                if previous_header.starts_with("else if") {
                    paren_indent
                } else if starts_nested_group {
                    base + self.min_conditional_indent_spaces()
                } else if line_paren_imbalance(previous_code).1.len() > 1 {
                    column + 1
                } else {
                    column
                }
            } else {
                paren_indent.max(base + self.min_conditional_indent_spaces())
            }
        })
    }

    pub(super) fn recent_output_has_open_ternary(&self) -> bool {
        (0..self.output.len()).rev().take(12).any(|index| {
            let code = self.output.code(index);
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return false;
            }
            code.contains('?')
        })
    }

    pub(super) fn previous_statement_is_braceless_ternary(&self) -> bool {
        for raw in self.output.iter().rev().skip(1).take(12) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() {
                continue;
            }
            if starts_ternary_arm(trimmed) && code.ends_with(';') {
                return true;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return false;
            }
        }
        false
    }

    pub(super) fn logical_condition_sibling_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', '{', '}']) {
            return None;
        }
        if !self.output.iter().rev().take(8).any(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with("if ") || trimmed.starts_with("while ")
        }) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if code.ends_with("||") {
            let base = self.continuation_base_indent() * self.options.indent_width;
            let standard = base + self.options.continuation_indent * self.options.indent_width;
            let conditional_floor = base + self.min_conditional_indent_spaces();
            if is_braceless_header_line(code.trim_start())
                && !line_paren_imbalance(code).1.is_empty()
            {
                return None;
            }
            if code.ends_with(") ||")
                && self.output.iter().rev().skip(1).take(8).any(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    trimmed.starts_with('(')
                        && code.ends_with(',')
                        && trimmed
                            .split_once(',')
                            .is_some_and(|(head, _)| head.contains(')'))
                })
            {
                return Some(standard);
            }
            if !trimmed.starts_with('(')
                && let Some(spaces) = self.parenthesized_logical_operand_indent_after_call_tail()
            {
                return Some(spaces);
            }
            if self.token_input.token_source_line_indent > standard.max(conditional_floor) {
                return Some(self.token_input.token_source_line_indent);
            }
            if trimmed.starts_with("(!") && code.trim_start().contains("((") {
                return None;
            }
            let paren_balance: isize = code
                .chars()
                .map(|ch| match ch {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                })
                .sum();
            if paren_balance < -1 {
                return None;
            }
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let current_indent = self.current_line_indent_spaces() + self.options.indent_width * 2;
            if previous_indent <= current_indent {
                return None;
            }
            if paren_balance == -1
                && self
                    .stack_state
                    .current_continuation_indent_spaces()
                    .is_none()
            {
                return None;
            }
            if paren_balance == -1 && !trimmed.starts_with("(!") {
                if self.options.min_conditional_indent == MinConditionalIndent::Zero {
                    return Some(
                        self.continuation_base_indent() * self.options.indent_width
                            + self.options.continuation_indent * self.options.indent_width,
                    );
                }
                return Some(previous_indent.saturating_sub(1));
            }
            if paren_balance == -1 && trimmed.starts_with("(!") {
                return Some(previous_indent.saturating_sub(1));
            }
            return Some(previous_indent);
        }
        None
    }

    fn parenthesized_logical_operand_indent_after_call_tail(&self) -> Option<usize> {
        for previous in self.output.iter().rev().skip(1).take(8) {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('(') && code.ends_with(',') {
                return Some(leading_visual_width(previous, self.options.tab_width) + 1);
            }
            if trimmed.starts_with("if ") || code.ends_with("&&") || code.ends_with(';') {
                return None;
            }
        }
        None
    }

    pub(super) fn return_ternary_tail_output_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with('?') {
            return None;
        }
        for previous in self.output.iter().rev().take(12) {
            let code = previous.trim_start();
            let code = code[..trailing_comment_split_limit(code)].trim_end();
            if code.is_empty() || code.starts_with('#') {
                continue;
            }
            if code.ends_with([';', '{', '}']) {
                return None;
            }
            if code.starts_with("return ") {
                let return_indent = leading_visual_width(previous, self.options.tab_width);
                let inside_switch = self
                    .stack_state
                    .brace_header_stack
                    .iter()
                    .any(|header| header.as_deref() == Some("switch"));
                if inside_switch {
                    if self.token_input.token_source_line_indent
                        <= return_indent + self.options.indent_width * 2
                    {
                        return Some(self.current_line_indent_spaces());
                    }
                    return Some(self.token_input.token_source_line_indent);
                }
                if self.recent_base_trailing_return_function_header() {
                    return Some(return_indent);
                }
                return Some(return_indent + "return ".len());
            }
        }
        None
    }

    pub(super) fn ternary_operator_tail_indent_spaces(&self, line: &str) -> Option<usize> {
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .and_then(|previous| {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                let previous_trimmed = previous_code.trim_start();
                if line.trim_start().starts_with('(')
                    && previous_trimmed.starts_with(": ")
                    && head_ends_binary_operator(previous_code)
                {
                    unmatched_open_paren_column(previous_code).map(|open| open + 1)
                } else {
                    None
                }
            })
    }

    fn split_ternary_colon_after_chained_true_arm(&self, line: &str) -> Option<(String, String)> {
        let current = line.trim_start();
        let tail = current.strip_prefix(": ")?;
        if tail.is_empty() {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        if !previous[..trailing_comment_split_limit(previous)]
            .trim_start()
            .starts_with('.')
        {
            return None;
        }
        let question_indent = self.output.iter().rev().skip(1).take(8).find_map(|line| {
            line[..trailing_comment_split_limit(line)]
                .trim_start()
                .starts_with('?')
                .then(|| leading_visual_width(line, self.options.tab_width))
        })?;
        let indent = leading_visual_width(line, self.options.tab_width);
        if indent != question_indent {
            return None;
        }
        let prefix = &line[..line.len() - current.len()];
        Some((format!("{prefix}:"), format!("{prefix}{tail}")))
    }

    pub(super) fn return_ternary_colon_after_multiline_template_declaration_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !(current.starts_with(": ") || current == ":") {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        if !previous[..trailing_comment_split_limit(previous)]
            .trim_start()
            .starts_with('?')
        {
            return None;
        }
        if !self.recent_trailing_return_function_after_multiline_template_declaration() {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.options.indent_width
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    fn capped_parenthesized_stream_indent(&self, spaces: usize, line_indent: usize) -> usize {
        let base = self.continuation_base_indent() * self.options.indent_width;
        if spaces.saturating_sub(base) > self.options.max_continuation_indent {
            line_indent + self.options.indent_width * 2
        } else {
            spaces
        }
    }

    pub(super) fn parenthesized_stream_chain_head_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if !current.starts_with("<<") && !current.starts_with(">>") && !current.starts_with("//") {
            return None;
        }
        let delimiter = self.frame_stack.active_delimiter()?;
        if delimiter.opener_output_line >= self.output.len() {
            return None;
        }
        self.parenthesized_stream_indent_for_line(delimiter.opener_output_line)
    }

    pub(super) fn nested_brace_after_stream_opener_indent_spaces(
        &self,
        current: &str,
        previous_code: &str,
    ) -> Option<usize> {
        if current.starts_with(['#', '(', ')', '{', '}'])
            || !current.contains("{ {")
            || !previous_code.ends_with('(')
        {
            return None;
        }
        let previous_trimmed = previous_code.trim_start();
        (previous_trimmed.starts_with("<<") || previous_trimmed.starts_with(">>"))
            .then(|| self.continuation_base_indent() * self.options.indent_width)
    }

    pub(super) fn previous_line_parenthesized_stream_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        self.parenthesized_stream_indent_for_line(previous_line)
    }

    pub(super) fn stream_after_closed_parenthesized_head_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with("<<") && !current.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if stream.operator_output_column != stream.line_indent_spaces
            || !stream.line_ends_with_close_paren
            || !stream.line_has_positive_paren_delta
            || stream.line_has_unmatched_open_paren
        {
            return None;
        }
        self.frame_stack
            .stream_before_output_line_with_unmatched_open_paren(previous_line)
            .map(|head| head.line_indent_spaces)
    }

    fn parenthesized_stream_indent_for_line(&self, line: usize) -> Option<usize> {
        let stream = self.frame_stack.first_stream_on_output_line(line)?;
        if !stream.line_has_unmatched_open_paren {
            return None;
        }
        let delimiter_column = self
            .frame_stack
            .first_delimiter_column_after_output_column(line, stream.operator_output_column)?;
        if stream.operator_output_column >= delimiter_column {
            return None;
        }
        let line_indent = stream.line_indent_spaces;
        let stream_spaces = stream.operator_output_column;
        let paren_spaces = delimiter_column + 1;
        let starts_with_operator = stream.operator_output_column == line_indent;
        let spaces = if starts_with_operator
            || paren_spaces.saturating_sub(line_indent) <= self.options.max_continuation_indent
        {
            paren_spaces
        } else {
            stream_spaces
        };
        Some(self.capped_parenthesized_stream_indent(spaces, line_indent))
    }

    pub(super) fn logical_after_previous_frame_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if !current.starts_with("&&") && !current.starts_with("||") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        if self
            .frame_stack
            .active_delimiter()
            .is_some_and(|delimiter| {
                delimiter.role == ParenRole::Header && delimiter.opener_output_line == previous_line
            })
        {
            return None;
        }
        let frame = self
            .frame_stack
            .active_logical_on_output_line(previous_line)?;
        if !frame.operator_starts_output_line {
            return None;
        }
        let current_operator = if current.starts_with("&&") {
            super::frame::LogicalOperator::And
        } else {
            super::frame::LogicalOperator::Or
        };
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if frame.line_has_positive_paren_delta {
            if frame.line_ends_with_close_paren {
                let candidate = frame
                    .line_indent_spaces
                    .saturating_sub(1)
                    .saturating_add(case_unindent);
                if self.token_input.token_source_line_indent > 0
                    && self.token_input.token_source_line_indent + self.options.indent_width
                        < candidate
                {
                    return Some(self.token_input.token_source_line_indent);
                }
                return Some(candidate);
            }
            if let Some(open) = self
                .frame_stack
                .logical_before_output_line_with_return(previous_line)
                .and_then(|frame| frame.line_unmatched_open_paren_column)
            {
                return Some(open + case_unindent);
            }
            return None;
        }
        if frame.operator != current_operator {
            return None;
        }
        Some(
            frame
                .line_unmatched_open_paren_column
                .map_or(frame.line_indent_spaces, |open| open + 1)
                + case_unindent,
        )
    }

    pub(super) fn ternary_colon_row_frame_indent_spaces(&self, current: &str) -> Option<usize> {
        if !(current.starts_with(": ") || current == ":") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with(':') || !previous_code.contains('?') {
            return None;
        }
        let frame = self.frame_stack.last_ternary_with_colon()?;
        if frame.colon_role != Some(ColonRole::Ternary) {
            return None;
        }
        frame.colon_output_column
    }

    pub(super) fn ternary_arm_frame_indent_spaces(&self, current: &str) -> Option<usize> {
        if current.starts_with('?') {
            let frame = self.frame_stack.active_ternary()?;
            if frame.colon_role.is_some()
                || (!matches!(
                    frame.owner_role,
                    TernaryOwnerRole::Assignment | TernaryOwnerRole::Return
                ) && frame.parent_delimiter.is_none())
            {
                return None;
            }
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if frame.owner_role == TernaryOwnerRole::Return {
                if self.recent_base_trailing_return_function_header() {
                    return Some(frame.question_indent_spaces + case_unindent);
                }
                if frame.parent_delimiter.is_some()
                    && let Some(return_line) = self.output.iter().rev().take(8).find(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        let trimmed = code.trim_start();
                        trimmed.starts_with("return ")
                            && !trimmed.contains('?')
                            && unmatched_open_paren_column(code).is_none()
                    })
                {
                    return Some(
                        leading_visual_width(return_line, self.options.tab_width)
                            + "return ".len()
                            + case_unindent,
                    );
                }
            }
            let spaces = frame
                .parent_delimiter
                .and_then(|id| self.frame_stack.delimiter_by_id(id))
                .map_or(frame.branch_anchor_column?, |delimiter| {
                    delimiter.opener_output_column + 1
                });
            return Some(spaces + case_unindent);
        }
        if current.starts_with(": ") || current == ":" {
            let frame = self.frame_stack.last_ternary_with_colon()?;
            if frame.colon_role != Some(ColonRole::Ternary)
                || (!matches!(
                    frame.owner_role,
                    TernaryOwnerRole::Assignment | TernaryOwnerRole::Return
                ) && frame.parent_delimiter.is_none())
            {
                return None;
            }
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if frame.owner_role == TernaryOwnerRole::Return {
                if self.recent_base_trailing_return_function_header() {
                    if self.recent_trailing_return_function_after_multiline_template_declaration() {
                        return Some(
                            frame.question_indent_spaces
                                + self.options.indent_width
                                + case_unindent,
                        );
                    }
                    return Some(frame.question_indent_spaces + case_unindent);
                }
                if frame.parent_delimiter.is_some()
                    && let Some(question_line) = self.output.iter().rev().take(8).find(|line| {
                        line[..trailing_comment_split_limit(line)]
                            .trim_start()
                            .starts_with('?')
                    })
                {
                    return Some(
                        leading_visual_width(question_line, self.options.tab_width) + case_unindent,
                    );
                }
            }
            let spaces = frame
                .parent_delimiter
                .and_then(|id| self.frame_stack.delimiter_by_id(id))
                .map_or(frame.branch_anchor_column?, |delimiter| {
                    delimiter.opener_output_column + 1
                });
            return Some(spaces + case_unindent);
        }
        None
    }

    pub(super) fn ternary_first_arm_indent_spaces(&self, current: &str) -> Option<usize> {
        if current.is_empty() || current.starts_with(['#', '?', ':', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !code.ends_with('?') {
            return None;
        }
        let indent_width = self.options.indent_width;
        if self.options.indent_after_parens {
            let frame = self.frame_stack.active_ternary()?;
            return Some(
                frame.question_indent_spaces
                    + self.options.continuation_indent * indent_width
                    + self.line_adjuster.total_case_unindent_depth() * indent_width,
            );
        }
        let condition = code[..code.len() - 1].trim_end();
        let anchor = if let Some(open) = unmatched_open_paren_column(condition) {
            visual_width_from(&condition[..open + 1], 0, self.options.tab_width)
        } else {
            let trimmed = condition.trim_start();
            let lead = condition.len() - trimmed.len();
            let operand_byte = if let Some(rest) = trimmed.strip_prefix("return") {
                if !rest.starts_with(char::is_whitespace) {
                    return None;
                }
                lead + "return".len() + (rest.len() - rest.trim_start().len())
            } else if let Some((operator_index, operator)) = find_assignment_operator(condition) {
                let after = &condition[operator_index + operator.len()..];
                operator_index + operator.len() + (after.len() - after.trim_start().len())
            } else {
                return Some(
                    leading_visual_width(condition, self.options.tab_width)
                        + self.line_adjuster.total_case_unindent_depth() * indent_width,
                );
            };
            visual_width_from(&condition[..operand_byte], 0, self.options.tab_width)
        };
        Some(anchor + self.line_adjuster.total_case_unindent_depth() * indent_width)
    }

    pub(super) fn ternary_colon_after_comment_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if !current.starts_with(':') || !previous.trim_start().starts_with("//") {
            return None;
        }
        self.output
            .iter()
            .rev()
            .skip(1)
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with("//")
            })
            .is_some_and(|line| line.trim_start().starts_with('?'))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn post_ternary_colon_comma_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with(':')
            || !previous_code.ends_with(',')
            || unmatched_open_paren_column(previous_code).is_some()
            || current.starts_with([':', ')', '}'])
        {
            return None;
        }
        self.output
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| line.contains('?'))
            .then(|| {
                leading_visual_width(previous, self.options.tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width
            })
    }

    pub(super) fn split_else_operator_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with([
            '<', '>', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~', '&', '|',
        ]) && !self.output.last_non_empty_index().is_some_and(|index| {
            let previous = self.output[index].trim_end();
            previous.ends_with("&&") || previous.ends_with("||") || previous.ends_with('(')
        }) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let split = trailing_comment_split_limit(previous);
        let previous_code = previous[..split].trim_end();
        let split_else_chain = self.recent_split_else_output_chain_active();
        let in_split_preprocessor_context = self.recent_split_else_operator_region_active();
        if split_else_chain
            && (current.starts_with("&&") || current.starts_with("||"))
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
        {
            return self.output.iter().rev().take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                assignment_value_column(code, self.options.tab_width)
            });
        }
        if in_split_preprocessor_context
            && current.starts_with([
                '<', '>', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ])
            && is_conditional_header_line(previous_code)
            && unmatched_open_paren_column(previous_code).is_some()
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.min_conditional_indent_spaces(),
            );
        }
        if (split < previous.len() || in_split_preprocessor_context)
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
            && !current.starts_with(['}', ')', ']'])
        {
            return Some(
                assignment_value_column(previous_code, self.options.tab_width).unwrap_or_else(
                    || {
                        if is_conditional_header_line(previous_code) {
                            leading_visual_width(previous, self.options.tab_width)
                                + self.min_conditional_indent_spaces()
                        } else {
                            unmatched_open_paren_column(previous_code)
                                .map(|column| column + 1)
                                .unwrap_or_else(|| {
                                    leading_visual_width(previous, self.options.tab_width)
                                })
                        }
                    },
                ),
            );
        }
        (in_split_preprocessor_context
            && previous_code.ends_with('(')
            && !current.starts_with(['}', ')', ']']))
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn split_else_ternary_sibling_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || !line.contains(" ? ") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.ends_with(':') && previous_code.contains(" ? "))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn split_else_ternary_comma_sibling_indent_floor(
        &self,
        line: &str,
        split_else_context: bool,
        current_spaces: usize,
    ) -> Option<usize> {
        if !split_else_context || !line.contains(" ? ") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') || !previous_code.contains(" ? ") {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces < target).then_some(target)
    }

    pub(super) fn split_else_completed_ternary_call_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(");") || !previous_code.contains(" ? ") {
            return None;
        }
        self.output.iter().rev().find_map(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            (code.ends_with(',') && unmatched_open_paren_column(code).is_some())
                .then_some(leading_visual_width(line, self.options.tab_width))
        })
    }

    pub(super) fn split_else_brace_logical_indent_spaces(
        &self,
        line: &str,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        let split_else_chain =
            structural_split_else_chain || self.recent_split_else_output_chain_active();
        if !split_else_chain {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let current = line.trim_start();
        let mut spaces = if (current.starts_with("&&") || current.starts_with("||"))
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
        {
            self.output.iter().rev().take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                assignment_value_column(code, self.options.tab_width)
            })
        } else if current.starts_with("||")
            && previous_code.trim_start().starts_with("||")
            && previous_code.ends_with(')')
        {
            self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start()
                    .starts_with("&& (")
                    .then_some(leading_visual_width(line, self.options.tab_width))
            })
        } else {
            None
        };
        if (current.starts_with("&&") || current.starts_with("||"))
            && line_is_control_body_header(previous_code.trim_start())
            && unmatched_open_paren_columns(previous_code).len() == 1
        {
            spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2,
            );
        }
        spaces
    }

    pub(super) fn none_style_split_else_logical_indent_spaces(
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
            || !line.trim_start().starts_with("||")
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (previous_code.trim_start().starts_with("&&")
            && unmatched_open_paren_column(previous_code).is_some())
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn split_else_completed_logical_statement_indent_spaces(&self) -> Option<usize> {
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if (previous_code.trim_start().starts_with("||")
            || previous_code.trim_start().starts_with("&&"))
            && previous_code.ends_with(';')
        {
            return self.output.iter().rev().skip(1).take(8).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                find_assignment_operator(code)
                    .is_some()
                    .then_some(leading_visual_width(line, self.options.tab_width))
            });
        }
        previous_code.ends_with(';').then(|| {
            self.output.iter().rev().skip(1).take(4).find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                ((code.ends_with("||") || code.ends_with("&&"))
                    && find_assignment_operator(code).is_some())
                .then_some(leading_visual_width(line, self.options.tab_width))
            })
        })?
    }

    pub(super) fn split_else_assignment_logical_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if !(current.starts_with("&&") || current.starts_with("||"))
            || !self.recent_split_else_output_chain_active()
        {
            return None;
        }
        self.output
            .iter()
            .rev()
            .take_while(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                !code.ends_with(';') && !code.ends_with('{') && !code.ends_with('}')
            })
            .take(8)
            .find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                assignment_value_column(code, self.options.tab_width)
            })
    }

    pub(super) fn observe_split_else_logical_statement_indent(
        &mut self,
        line: &str,
        line_kind: LineKind,
    ) {
        if line_kind != LineKind::Normal || !line.trim_end().ends_with(';') {
            return;
        }
        let current = line.trim_start();
        let statement = if current.starts_with("||") || current.starts_with("&&") {
            self.output.iter().rev().skip(1).take(8).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                find_assignment_operator(code).is_some()
            })
        } else if self.recent_split_else_logical_statement_region_active() {
            self.output.iter().rev().skip(1).take(4).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (code.ends_with("||") || code.ends_with("&&"))
                    && find_assignment_operator(code).is_some()
            })
        } else {
            None
        };
        if let Some(statement) = statement {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces =
                Some(leading_visual_width(statement, self.options.tab_width));
            self.stack_state.clear_continuation_indents();
        }
    }
}

fn assignment_value_column(code: &str, tab_width: usize) -> Option<usize> {
    let (assignment, operator) = find_assignment_operator(code)?;
    let after_operator = assignment + operator.len();
    let value_start = code[after_operator..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(code.len(), |(offset, _)| after_operator + offset);
    Some(visual_width_from(&code[..value_start], 0, tab_width))
}
