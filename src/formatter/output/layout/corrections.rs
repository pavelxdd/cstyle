use super::super::super::FormatEngine;
use super::super::super::columns::leading_visual_width;
use super::super::super::frame::BraceSemanticKind;
use super::super::super::headers::is_attachable_closing_header;
use super::super::super::headers::{same_line_nested_header_extra, starts_header_word};
use super::super::super::indentation::LineKind;
use super::super::super::labels;

use super::super::super::line_scan::{line_paren_imbalance, trailing_comment_split_limit};
use super::super::super::preprocessor::preprocessor_directive;
use super::super::model::{LineLayout, LineReplayLayout};
use crate::config::{BraceStyle, IndentStyle};
use crate::source::lex::is_identifier_continue;
use crate::source::lex::leading_identifier;

impl FormatEngine<'_> {
    pub(in super::super) fn apply_brace_header_case_and_initializer_correction_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        if self.options.indent_braces
            && matches!(
                self.options.brace_style,
                BraceStyle::None | BraceStyle::Allman
            )
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
        {
            let previous_brace_indent = self.output.last_non_empty_line().and_then(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.ends_with('{')
                    .then(|| leading_visual_width(previous, self.options.tab_width))
            });
            let target = previous_brace_indent
                .unwrap_or_else(|| indent.saturating_sub(1) * self.options.indent_width);
            if exact_indent_spaces.unwrap_or(indent * self.options.indent_width) > target {
                exact_indent_spaces = Some(target);
            }
        }
        if let Some(spaces) =
            self.active_case_control_closing_indent_override(line, indent, exact_indent_spaces)
        {
            exact_indent_spaces = Some(spaces);
        }
        if self.current_line_has_class_initializer_colon
            && line.trim_start().starts_with(':')
            && !line.trim_start().starts_with("::")
        {
            exact_indent_spaces = Some(indent * self.options.indent_width);
        }
        if let Some(spaces) = self.compound_case_label_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_switch_closing_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) && line.trim_start().starts_with(['.', '['])
            && let Some(frame) = self
                .frame_stack
                .active_brace()
                .filter(|frame| frame.semantic_kind == BraceSemanticKind::CompoundLiteral)
        {
            exact_indent_spaces = Some(frame.sibling_indent_column);
        }
        if self.options.brace_style == BraceStyle::Whitesmith
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.trim() == "{")
            && let Some(frame) = self.frame_stack.active_brace().filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Definition
                    && self.stack_state.brace_type_stack.last() == Some(&frame.formatter_type)
            })
        {
            exact_indent_spaces = Some(frame.sibling_indent_column);
        }
        let indented_command_body = self.indented_command_body_indent_spaces();
        let active_brace = self.frame_stack.active_brace();
        let current_header_owns_active_brace = self.is_header(leading_identifier(line))
            && active_brace
                .zip(self.frame_stack.active_header())
                .is_some_and(|(brace, header)| {
                    brace.header.as_deref() == Some(header.header.as_str())
                        && brace.header_indent_column == header.line_indent_spaces
                });
        let previous_output_brace = if current_header_owns_active_brace {
            self.frame_stack.enclosing_brace()
        } else {
            active_brace
        };
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim() == "{")
            && self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| preprocessor_directive(line.trim_start()) == Some("endif"))
            && let Some(frame) = previous_output_brace.filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command && frame.header.is_some()
            })
        {
            exact_indent_spaces = Some(
                frame.body_indent_column
                    + self.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if line.trim() == "{"
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        } else if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && !self.is_header(leading_identifier(line))
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.trim() == "{")
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && self.is_header(leading_identifier(line))
            && same_line_nested_header_extra(line.trim_start()) == 0
            && !(self.options.no_indent_if_after_else
                && starts_header_word(line.trim_start(), "if")
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|previous| matches!(previous.trim(), "else" | "} else")))
            && let Some(header) = self.frame_stack.active_header().filter(|header| {
                header
                    .line_indent_spaces
                    .is_multiple_of(self.options.indent_width)
            })
        {
            let current = indent * self.options.indent_width;
            if exact_indent_spaces.is_none() && current < header.line_indent_spaces {
                exact_indent_spaces = Some(header.line_indent_spaces);
            }
        }
        if line.trim_start().starts_with("else")
            && let Some(header) = self
                .frame_stack
                .active_header()
                .filter(|header| header.header == "else")
        {
            exact_indent_spaces = Some(header.line_indent_spaces);
        }
        if let Some(spaces) =
            self.vtk_or_ratliff_headerless_command_opening_brace_indent_spaces(line)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.gnu_command_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.gnu_command_closing_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.ratliff_command_closing_header_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && line.trim_start().starts_with(']')
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if ["for", "while", "switch"].iter().any(|header| {
                let trimmed = previous_code.trim_start();
                trimmed == *header
                    || trimmed
                        .strip_prefix(header)
                        .is_some_and(|rest| rest.starts_with([' ', '\t']))
            }) && !previous_code.contains('(')
            {
                exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some((column, true)) = self.inline_array.current_closed_body_column.take()
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with('}')
            && line.contains('}')
        {
            exact_indent_spaces = Some(column);
        }
        if let Some(spaces) = self.pending_split_else_braceless_body_indent_spaces(
            line,
            line_kind,
            indent * self.options.indent_width,
            exact_indent_spaces,
        ) {
            exact_indent_spaces = Some(spaces);
        }
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }

    pub(in super::super) fn apply_label_switch_case_and_opening_brace_correction_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let normal_indent = layout.normal_indent;
        let class_scope_label = layout.class_scope_label;
        let mut indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        let (line_closing_parens, line_opening_parens) = line_paren_imbalance(line);
        let line_closes_outer_delimiter = line_closing_parens > line_opening_parens.len();
        let line_has_owned_continuation = self.frame_stack.active_delimiter().is_some()
            || self.operator_chain_owns_continuation(line);
        if let Some(spaces) = self.post_block_case_body_indent_override(
            line,
            line_kind,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
            is_attachable_closing_header(leading_identifier(line)),
        ) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.nested_case_label_indent_override(line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.active_label_block_indent_spaces(
            line,
            line_kind,
            indent == normal_indent,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
        ) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.closed_label_block_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(case_layout) = self.active_case_block_body_layout(
            line,
            line_kind,
            indent == normal_indent,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
            exact_indent_spaces,
        ) {
            exact_indent_spaces = Some(case_layout.exact_indent_spaces);
            if let Some(minimum) = case_layout.minimum_indent_level {
                indent = indent.max(minimum);
            }
        }
        if let Some(spaces) =
            self.switch_case_frame_closing_indent_override(line, exact_indent_spaces)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(label_layout) = labels::default_line_layout(
            line_kind,
            class_scope_label,
            indent,
            self.case_body_indent_extra(LineKind::Normal),
            self.options,
        ) {
            exact_indent_spaces = Some(label_layout.indent_spaces);
            if let Some(level) = label_layout.indent_level {
                indent = level;
            }
        }
        if let Some(spaces) = self.lambda_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.initializer_or_array_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && (line.trim_start().starts_with("///") || line.trim_start().starts_with("//!"))
            && self
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Aggregate)
            && let Some(spaces) = self.active_body_comment_indent_spaces()
        {
            exact_indent_spaces = Some(spaces);
        }
        layout.indent = indent;
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }

    pub(in super::super) fn apply_final_recovery_floor_and_replay_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let mut indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && self
                .argument_after_lambda_call_argument_indent_spaces(line)
                .is_none()
            && !self.has_over_max_new_call_context()
            && let Some(spaces) = replay.closed_delimiter_continuation_indent
            && self.output.last_non_empty_line().is_some_and(|previous| {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                previous_code.ends_with(',')
                    && line_paren_imbalance(previous_code).0 > 0
                    && !(self.options.indent_after_parens
                        && previous_code
                            .split(|ch: char| !is_identifier_continue(ch))
                            .any(|word| word == "new"))
            })
        {
            exact_indent_spaces = Some(
                spaces + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        if let Some(spaces) = self.maximum_length_new_call_argument_indent_spaces() {
            exact_indent_spaces = Some(spaces);
        }
        if self.options.indent_after_parens
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with(',')
            && previous
                .split(|ch: char| !is_identifier_continue(ch))
                .any(|word| word == "new")
        {
            exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        if let Some(spaces) =
            self.maximum_length_capped_open_paren_argument_indent_spaces(line_kind)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.maximum_length_logical_header_indent_spaces(line, line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && self.output.last_non_empty_line().is_some()
            && let Some(spaces) = self.trailing_stream_top_level_indent_spaces(line_kind)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.maximum_length_return_chain_indent_spaces(line, line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.constructor_lambda_header_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.lambda_closing_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.lambda_body_indent_spaces_after_opening_brace(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.inline_body_owner_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.lisp_attached_suffix_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.header_operator_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.lambda_parameter_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(minimum) = self.split_else_exact_tab_indent_level(exact_indent_spaces) {
            indent = indent.max(minimum);
        }
        if self.options.indent_style != IndentStyle::Spaces
            && let Some(base) = self.constructor_initializer_base_indent_spaces()
            && exact_indent_spaces.is_some_and(|spaces| spaces >= base)
            && base.is_multiple_of(self.options.indent_width)
        {
            indent = indent.max(base / self.options.indent_width);
        }
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '}'])
            && !starts_header_word(line.trim_start(), "switch")
            && let Some(spaces) = self.direct_switch_body_indent_spaces()
        {
            let current = exact_indent_spaces.unwrap_or(indent * self.options.indent_width);
            if current < spaces {
                exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) = self.restored_preprocessor_branch_body_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.objc_line_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        layout.indent = indent;
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }
}
