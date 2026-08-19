use super::super::super::FormatEngine;
use super::super::super::brace_classification::line_opens_lambda_block;
use super::super::super::call_arguments::{
    assignment_call_value_column, casted_assignment_value_column,
};
use super::super::super::closing_braces::starts_post_closing_declaration;
use super::super::super::columns::{leading_visual_width, visual_width_from};
use super::super::super::frame::BraceSemanticKind;
use super::super::super::headers::{
    is_braceless_header_line, line_is_control_body_header, same_line_nested_header_extra,
    starts_header_word,
};
use super::super::super::indentation::LineKind;
use super::super::super::labels;

use super::super::super::language::is_macro_like_word;
use super::super::super::line_scan::is_comment_line;
use super::super::super::line_scan::{
    has_unmatched_open_brace, line_paren_imbalance, trailing_comment_split_limit,
    unmatched_open_paren_column, unmatched_open_paren_columns,
};
use super::super::super::literals::{first_string_literal_start, starts_string_literal_token};
use super::super::super::objective_c::objc_message_following_keyword_column;
use super::super::super::operators::{
    head_ends_binary_operator, is_prefix_increment_statement, starts_prefix_increment,
    starts_with_chain_operator,
};
use super::super::super::preprocessor::preprocessor_directive;
use super::super::super::state::FormatterBraceType;
use super::super::super::switch_cases::case_label_with_trailing_comment;
use super::super::super::template_declarations::{
    template_continuation_indent_spaces, template_declaration_line_complete,
};
use super::super::model::{ContextualLineLayout, LineLayout, LineReplayLayout};
use crate::config::{BraceStyle, IndentStyle};
use crate::source::lex::leading_identifier;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

impl FormatEngine<'_> {
    pub(in super::super) fn begin_contextual_line_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = layout
            .exact_indent_spaces
            .unwrap_or(layout.indent * self.options.indent_width);
        let split_else_state_active = self.split_else_line_layout_active();
        if layout.line_kind == LineKind::Normal
            && let Some(spaces) = self.else_split_header_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && let Some(spaces) = self.split_return_type_pointer_name_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
            && {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                previous_code.ends_with(',')
                    && previous_code.split_once(" : ").is_some_and(|(_, tail)| {
                        tail.split(',')
                            .next()
                            .is_some_and(|first| first.contains('('))
                    })
                    && unmatched_open_paren_column(previous_code).is_none()
            }
            && let Some(base) = self.same_line_constructor_initializer_base_indent_spaces()
        {
            layout.exact_indent_spaces = Some(base);
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .strip_prefix(['+', '-'])
                .is_some_and(|rest| rest.trim_start().starts_with('('))
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with('+') {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if previous_code.trim_start().starts_with("*/")
                && self
                    .output
                    .iter()
                    .rev()
                    .take_while(|line| !line.trim_start().starts_with("@end"))
                    .any(|line| line.trim_start().starts_with("@interface"))
            {
                layout.exact_indent_spaces = Some(0);
            }
        }
        if starts_prefix_increment(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
            && is_prefix_increment_statement(previous.trim_start())
            && previous.trim_end().ends_with(';')
            && let Some(header) = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty())
            && is_braceless_header_line(header.trim_start())
            && !header.trim_end().ends_with('{')
        {
            layout.exact_indent_spaces = Some(leading_visual_width(header, self.options.tab_width));
        }
        if let Some(spaces) = self.commented_class_head_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = layout.exact_indent_spaces.as_mut()
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let current_trimmed = line.trim_start();
            let starts_new = |text: &str| {
                text.strip_prefix("new")
                    .is_some_and(|rest| rest.starts_with(char::is_whitespace))
            };
            if let Some(column) = self.current_inline_array_column()
                && self.frame_stack.active_constructor_initializer().is_some()
                && previous_code.ends_with(',')
                && previous_code.contains('{')
                && !current_trimmed.starts_with('}')
            {
                *spaces = column;
            } else if previous_code.ends_with(',')
                && starts_new(previous_code.trim_start())
                && starts_new(current_trimmed)
                && *spaces < previous_indent
            {
                *spaces = previous_indent;
            } else if let Some(column) = self.current_inline_array_column()
                && self.constructor_initializer_base_indent_spaces().is_none()
                && *spaces > column
                && previous_code.ends_with(',')
                && previous_code.contains('(')
                && !previous_code.contains('{')
            {
                *spaces = column;
            } else if *spaces > previous_indent
                && previous_code.ends_with(',')
                && previous_code.contains('(')
                && unmatched_open_paren_column(previous_code).is_none()
                && self
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
                        code.ends_with("({")
                    })
            {
                *spaces = previous_indent;
            }
        }
        if !line.trim_start().starts_with(['.', '{', '}']) {
            let spaces = layout
                .exact_indent_spaces
                .unwrap_or(layout.indent * self.options.indent_width);
            let base = self.continuation_base_indent() * self.options.indent_width;
            let stream_indent = (0..self.output.len()).rev().take(8).find_map(|index| {
                let code = self.output.code(index);
                if has_unmatched_open_brace(code) {
                    code.find(" << ")
                        .or_else(|| code.find(" >> "))
                        .map(|index| index + 1)
                } else {
                    None
                }
            });
            if spaces.saturating_sub(base) > self.options.max_continuation_indent
                && let Some(stream_indent) = stream_indent
            {
                layout.exact_indent_spaces = Some(stream_indent);
            }
        }
        if self.stream_chain_frame_indent_spaces(line).is_none()
            && let Some(spaces) = self.initializer_member_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.designated_initializer_source_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.closed_initializer_or_array_indent_spaces(
            line,
            layout.indent,
            layout.normal_indent,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.compound_closing_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("};")
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.contains(['{', '}']))
        {
            layout.exact_indent_spaces = Some(layout.indent * self.options.indent_width);
        }
        if let Some(spaces) = self.over_max_inner_call_open_argument_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.outer_call_argument_after_closed_inner_call_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        ContextualLineLayout {
            layout,
            output_spaces,
            split_else_state_active,
        }
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_previous_output_call_and_initializer_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let previous_trimmed = previous.trim_start();
            let current = line.trim_start();
            if starts_with_chain_operator(current) && previous_trimmed.starts_with("//") {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if let Some(base) = self.constructor_initializer_base_indent_spaces()
                && previous_code.ends_with(',')
                && !current.starts_with(['#', ':', ',', '{', '}'])
                && current
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_uppercase())
                && unmatched_open_paren_column(previous_code.trim_start()).is_none()
                && !has_unmatched_open_brace(previous_code)
                && !has_unmatched_open_brace(current)
                && leading_visual_width(previous, self.options.tab_width) <= base
                && !(previous_code.contains('{')
                    && current.contains('{')
                    && !previous_trimmed.starts_with(':')
                    && leading_visual_width(previous, self.options.tab_width) != base)
            {
                layout.exact_indent_spaces = Some(base);
            }
            if previous_trimmed.starts_with(':')
                && previous_code.ends_with(',')
                && has_unmatched_open_brace(previous_code)
                && !current.starts_with(['#', '(', ')', '{', '}'])
                && let Some(open) = previous_code.rfind('{')
            {
                layout.exact_indent_spaces = Some(visual_width_from(
                    &previous_code[..open + 1],
                    0,
                    self.options.tab_width,
                ));
            }
            if previous_trimmed.starts_with(':')
                && previous_code.ends_with(',')
                && !has_unmatched_open_brace(previous_code)
                && self.constructor_initializer_base_indent_spaces().is_none()
                && !current.starts_with(['#', '(', ')', '{', '}'])
            {
                for raw in self.output.iter().rev().skip(1).take(16) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed_code = code.trim();
                    if trimmed_code == "(" {
                        layout.exact_indent_spaces = Some(
                            leading_visual_width(raw, self.options.tab_width)
                                + self.options.indent_width,
                        );
                        break;
                    }
                    if let Some(open) = unmatched_open_paren_column(code) {
                        layout.exact_indent_spaces = Some(open + 1);
                        break;
                    }
                    if trimmed_code.ends_with(';') || trimmed_code == "{" || trimmed_code == "}" {
                        break;
                    }
                }
            }
            if previous_code.ends_with(',')
                && previous_trimmed.starts_with('(')
                && current.chars().next().is_some_and(|ch| ch.is_ascii_digit())
            {
                let previous_indent = leading_visual_width(previous, self.options.tab_width);
                if self.token_input.token_source_line_indent >= previous_indent
                    && previous_indent > self.options.indent_width
                {
                    layout.exact_indent_spaces = Some(previous_indent);
                }
            }
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if case_unindent > 0
                && self.token_input.token_source_line_indent > 0
                && previous_code.ends_with(',')
                && line_paren_imbalance(previous_code).0 > 0
                && !current.starts_with(['#', '(', ')', '{', '}'])
            {
                let target = self.token_input.token_source_line_indent + case_unindent;
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
            if case_unindent > 0
                && self.token_input.token_source_line_indent
                    > layout.exact_indent_spaces.unwrap_or(0)
                && previous_code.ends_with(',')
                && !line_paren_imbalance(previous_code).1.is_empty()
                && !current.starts_with(['#', '(', ')', '{', '}'])
                && let Some(spaces) = layout.exact_indent_spaces.as_mut()
            {
                *spaces += case_unindent;
            }
            if let Some(spaces) =
                self.recent_ternary_argument_sibling_indent_spaces(current, previous)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if self.token_input.token_source_line_indent
                > layout
                    .exact_indent_spaces
                    .unwrap_or(layout.indent * self.options.indent_width)
                && previous_trimmed.starts_with('(')
                && previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with(',')
                && unmatched_open_paren_column(previous_code).is_none()
                && layout.exact_indent_spaces
                    != Some(leading_visual_width(previous, self.options.tab_width))
                && !current.starts_with(['#', '(', ')', '{', '}'])
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
            if let Some(spaces) = labels::access_label_body_indent_spaces(
                line,
                previous,
                self.stack_state.brace_type_stack.last().copied(),
                self.options,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) = self.stream_chain_frame_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.range_designator_source_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = layout.exact_indent_spaces.as_mut()
            && *spaces == self.token_input.token_source_line_indent
            && self.token_input.token_source_line_indent > 0
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let case_unindent = self.adjusted_line_indent_delta(previous);
            if case_unindent > 0 && previous_code.ends_with(',') {
                *spaces += case_unindent;
            }
        }
        if line.trim_start().starts_with(',')
            && self
                .output
                .iter()
                .rev()
                .take(8)
                .any(|line| line.trim() == "?")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if line.trim_start().starts_with(':')
            && self
                .output
                .iter()
                .rev()
                .take(8)
                .any(|line| line.trim_start().starts_with(":#if"))
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim() == "=" {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        contextual
    }

    pub(in super::super) fn apply_source_indent_brace_and_style_operator_layout(
        &mut self,
        line: &str,
        case_unindent_closing_line: bool,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        let current_spaces = layout
            .exact_indent_spaces
            .unwrap_or(layout.indent * self.options.indent_width);
        if let Some(spaces) =
            self.source_indent_override_spaces(line, layout.line_kind, current_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && (self.previous_output_is_complete_template_declaration()
                || self.previous_output_closes_multiline_template_declaration())
        {
            layout.indent = layout.normal_indent;
            layout.exact_indent_spaces = None;
        }
        if (line.trim_start().starts_with("} else if") || line.trim_start().starts_with("}else if"))
            && let Some(previous) = self.output.last_non_empty_line()
            && preprocessor_directive(previous.trim_start()).is_some()
            && let Some(header) = self.output.iter().rev().skip(1).take(32).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                code.ends_with('{')
                    && (starts_header_word(trimmed, "if")
                        || trimmed.starts_with("} else")
                        || trimmed.starts_with("}else"))
            })
        {
            layout.exact_indent_spaces = Some(leading_visual_width(header, self.options.tab_width));
        }
        if let Some(spaces) =
            self.immediate_case_brace_indent_spaces(line, case_unindent_closing_line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.unmatched_closing_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        } else if line.trim() == "}"
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with("];")
            && self
                .output
                .iter()
                .rev()
                .take(16)
                .any(|line| line.trim_start().starts_with("- ("))
        {
            layout.exact_indent_spaces = Some(0);
        } else if let Some(spaces) =
            self.isolated_closing_brace_indent_spaces(line, case_unindent_closing_line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.split_else_closing_indent_floor(line, layout.indent, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim() == "}"
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with(',')
                && previous_code.ends_with(");")
                && let Some((open_spaces, _, _)) = self
                    .output
                    .current_closing_brace_open(self.options.tab_width)
            {
                layout.exact_indent_spaces = Some(open_spaces);
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '}', '#'])
            && self.frame_stack.active_brace().is_none_or(|frame| {
                frame.header.as_deref().is_none_or(|header| {
                    starts_header_word(header, "if")
                        || starts_header_word(header, "while")
                        || starts_header_word(header, "for")
                        || starts_header_word(header, "do")
                        || header.starts_with("else")
                        || header.starts_with("case ")
                        || header.starts_with("default:")
                })
            })
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if code.ends_with('{')
                && (starts_header_word(trimmed, "if")
                    || starts_header_word(trimmed, "while")
                    || starts_header_word(trimmed, "for")
                    || starts_header_word(trimmed, "do")
                    || trimmed.starts_with("else")
                    || trimmed.starts_with("case ")
                    || trimmed.starts_with("default:"))
            {
                let nested_header_extra = same_line_nested_header_extra(trimmed);
                let target = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * (1 + nested_header_extra)
                    + usize::from(
                        !trimmed.starts_with("case ") && !trimmed.starts_with("default:"),
                    ) * self.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
        }
        if let Some(base) = self.initializer_brace_continuation_anchor(line) {
            layout.exact_indent_spaces = Some(base);
            self.update_current_brace_indent_columns(base + self.options.indent_width, base);
        }
        if let Some(spaces) =
            self.whitesmith_identifier_opening_brace_indent_spaces(line, layout.normal_indent)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.gnu_continuation_opening_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
            self.update_current_brace_indent_columns(spaces + self.options.indent_width, spaces);
        }
        if let Some(spaces) = self.gnu_leading_operator_indent_spaces(
            line,
            layout.line_kind,
            layout.normal_indent,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.allman_operator_or_preprocessor_indent_spaces(
            line,
            layout.line_kind,
            layout.normal_indent,
            self.header_operator_continuation_indent_spaces(line),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line
            .trim_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains("#endif") && !previous_code.trim_start().starts_with('#') {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if line.trim_start().starts_with("{ ~")
            && self
                .output
                .iter()
                .rev()
                .take(3)
                .any(|line| line.contains("#else") || line.contains("#define"))
        {
            layout.exact_indent_spaces = Some(self.options.indent_width);
        }
        if line
            .trim_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with("#else")
            && previous.trim_start().ends_with(']')
        {
            layout.exact_indent_spaces = Some(0);
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_previous_statement_and_operator_prefix_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if !line.trim_start().starts_with(['}', '#'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous.contains("\\ //")
                && previous_code.contains('{')
                && !previous_code.trim_start().starts_with('#')
                && let Some(frame) = self.frame_stack.active_brace()
            {
                layout.exact_indent_spaces = Some(frame.body_indent_column);
            }
            if line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ]) && previous.trim_end().ends_with(':')
                && previous.contains('#')
                && !previous.trim_start().starts_with('#')
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width * 2,
                );
            }
            if previous.contains("; catch")
                && line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(is_identifier_start)
            {
                layout.exact_indent_spaces = Some(self.state.indent() * self.options.indent_width);
            }
            if previous_code.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
            ]) && previous_code.ends_with('{')
                && self.current_inline_array_column().is_none()
                && line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(is_identifier_start)
            {
                layout.exact_indent_spaces = Some(self.state.indent() * self.options.indent_width);
            }
            if let Some(spaces) = self.scoped_ternary_continuation_indent_spaces(line) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if previous_code.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
            ]) && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
                && self.output.iter().rev().skip(1).take(3).any(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    code.trim_start().starts_with('#') && code.contains('(') && !code.contains(')')
                })
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
            if starts_post_closing_declaration(previous_code) {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if line.trim() == "::"
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with('}')
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if let Some(spaces) = self.whitesmith_operator_opening_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("::") && line.contains('}') {
            layout.exact_indent_spaces = Some(self.options.indent_width);
        }
        if line.trim_start().starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]) && let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with('#')
                && previous_code.contains('(')
                && !previous_code.contains(')')
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if line.trim_start().starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]) && (0..self.output.len()).rev().take(8).any(|index| {
            let previous_code = self.output.code(index);
            starts_post_closing_declaration(previous_code)
        }) {
            layout.exact_indent_spaces = Some(self.options.indent_width);
        }
        if line.trim_start().starts_with([
            '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]) && let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with('#')
                && previous_code.contains('(')
                && !previous_code.contains(')')
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if let Some(spaces) = self.pico_leading_operator_after_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with([
            '<', '>', '|', '&', '*', '/', '%', '=', '!', '?', ':', ',', '.',
        ]) && !line.trim_start().starts_with("//")
            && !line.trim_start().starts_with("/*")
            && self
                .output
                .last()
                .is_some_and(|line| line.trim().is_empty())
            && self.output.iter().any(|line| !line.trim().is_empty())
        {
            layout.exact_indent_spaces = Some(self.options.indent_width);
        }
        contextual
    }

    pub(in super::super) fn apply_label_else_and_conditional_contextual_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if let Some(spaces) = labels::candidate_line_indent_spaces(
            line,
            self.options,
            self.frame_stack.active_ternary().is_some()
                || self.in_initializer_brace()
                || self.current_inline_array_column().is_some(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("else")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            if let Some(spaces) =
                self.else_after_candidate_label_indent_spaces(layout.line_kind, previous)
            {
                layout.exact_indent_spaces = Some(spaces);
            } else if let Some(spaces) = self.else_after_closed_nested_header_indent_spaces(line) {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '}', '#'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains('#')
                && !previous_code.trim_start().starts_with('#')
                && head_ends_binary_operator(previous_code)
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
            if self.in_enum_declaration_brace()
                && !previous_code.trim_start().starts_with(['/', '*'])
                && !previous_code.trim_end().ends_with("*/")
                && head_ends_binary_operator(previous_code)
                && line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_digit())
            {
                let previous_indent = leading_visual_width(previous, self.options.tab_width);
                let base = layout.normal_indent * self.options.indent_width;
                let target = if previous_indent > base {
                    previous_indent
                } else {
                    previous_indent + self.options.indent_width
                };
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
            if self.token_input.token_source_line_indent > 0
                && line.contains('#')
                && !line.trim_start().starts_with('#')
                && previous_code.ends_with('~')
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
        }
        if let Some(spaces) =
            self.whitesmith_definition_or_command_opening_brace_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ])
        {
            let mut anchor = None;
            for index in (0..self.output.len()).rev().take(4) {
                let previous = &self.output[index];
                let code = self.output.code(index);
                let trimmed = self.output.code_trimmed(index);
                if !trimmed.starts_with('#')
                    && (code.contains("#if") || code.contains("#else") || code.contains("#elif"))
                {
                    anchor = Some(previous);
                    break;
                }
                if !(trimmed.starts_with([
                    '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
                    '{', '}',
                ]) || trimmed.starts_with("//")
                    || trimmed.starts_with("/*"))
                {
                    break;
                }
            }
            if let Some(anchor) = anchor {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(anchor, self.options.tab_width));
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('}')
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with("&&")
                && (0..self.output.len()).rev().take(4).any(|index| {
                    let code = self.output.code(index);
                    code.contains('#') && !self.output.code_trimmed(index).starts_with('#')
                })
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + visual_width_from(previous_code.trim_start(), 0, self.options.tab_width)
                        + self.options.indent_width * 4,
                );
            }
        }
        if let Some(spaces) = self.designated_initializer_source_indent_floor(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && let Some(previous_index) = self.output.last_non_empty_index()
            && self.output[previous_index].trim_end().ends_with('>')
        {
            let previous = &self.output[previous_index];
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if self.is_template_declaration_head_line(previous_trimmed)
                && !line.trim_start().starts_with(['#', '{', '}', ':', ','])
            {
                if template_declaration_line_complete(previous_trimmed) {
                    layout.exact_indent_spaces =
                        Some(leading_visual_width(previous, self.options.tab_width));
                } else if let Some(spaces) = template_continuation_indent_spaces(previous) {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && let Some(spaces) = self.split_else_operator_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.class_base_logical_operand_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && (self.previous_output_is_complete_template_declaration()
                || self.previous_output_closes_multiline_template_declaration())
        {
            layout.indent = layout.normal_indent;
            layout.exact_indent_spaces = None;
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with("} else")
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|line| is_comment_line(line.trim_start()))
        {
            let anchor = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.trim().is_empty() || is_comment_line(line.trim_start()))
                .find(|line| !line.trim().is_empty());
            if let Some(anchor) = anchor {
                if anchor.trim_end().contains("/*") && !anchor.trim_start().starts_with("/*") {
                    layout.exact_indent_spaces = Some(
                        leading_visual_width(anchor, self.options.tab_width)
                            .saturating_sub(self.options.indent_width),
                    );
                } else if let Some(header) = self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.trim().is_empty() || is_comment_line(line.trim_start()))
                    .find(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        code.ends_with('{') && code.trim() != "{"
                    })
                {
                    layout.exact_indent_spaces =
                        Some(leading_visual_width(header, self.options.tab_width));
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('*')
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if previous_trimmed.starts_with("case ") && previous_trimmed.ends_with(':') {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with("/*")
            && self.output.last_non_empty_line().is_some_and(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                (code.ends_with(';') || code.ends_with('{') || code.ends_with('}'))
                    && unmatched_open_paren_column(code).is_none()
            })
        {
            layout.indent = layout.normal_indent;
            layout.exact_indent_spaces = Some(layout.normal_indent * self.options.indent_width);
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_none_style_else_and_conditional_body_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let layout = &mut contextual.layout;
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && line.trim_start().starts_with("} else")
        {
            let mut previous_lines = self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty());
            if let Some(previous) = previous_lines.next()
                && !previous.trim_start().starts_with(['}', '#'])
                && !previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with('{')
                && previous_lines
                    .take(8)
                    .any(|line| line.trim_start().starts_with('}'))
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if layout.exact_indent_spaces.unwrap_or(output_spaces) > spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && !line.trim_start().starts_with(['#', '{', '}'])
            && let Some(previous_index) = self.output.last_non_empty_index()
        {
            let previous = &self.output[previous_index];
            let previous_trimmed = self.output.code_trimmed(previous_index);
            let after_blank = self.output.last().is_some_and(|line| line.is_empty())
                || self
                    .token_input
                    .previous_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.matches('\n').count() > 1);
            if after_blank && (previous_trimmed == "else" || previous_trimmed.ends_with("} else")) {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
            if after_blank
                && preprocessor_directive(previous_trimmed) == Some("endif")
                && self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    })
                    .is_some_and(|line| {
                        let trimmed = line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .trim_start();
                        trimmed == "else" || trimmed.ends_with("} else")
                    })
            {
                let spaces = self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    })
                    .map(|line| {
                        let trimmed = line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .trim_start();
                        let extra = usize::from(trimmed == "else") * self.options.indent_width;
                        leading_visual_width(line, self.options.tab_width) + extra
                    })
                    .unwrap_or(layout.normal_indent * self.options.indent_width);
                layout.exact_indent_spaces = Some(spaces);
            }
            if !after_blank
                && preprocessor_directive(previous_trimmed)
                    .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
                && let Some(header) = self.output.iter().rev().skip(1).find(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
            {
                let header_code = header[..trailing_comment_split_limit(header)].trim_end();
                let header_trimmed = header_code.trim_start();
                if header_trimmed == "else" || header_trimmed.ends_with("} else") {
                    let spaces = leading_visual_width(header, self.options.tab_width)
                        + self.options.indent_width;
                    if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                } else if is_comment_line(header.trim_start()) {
                    let spaces = leading_visual_width(header, self.options.tab_width);
                    if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                } else if header_code.ends_with('{') {
                    let spaces = leading_visual_width(header, self.options.tab_width)
                        + self.options.indent_width;
                    if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                }
            } else if let Some(directive) = preprocessor_directive(previous_trimmed)
                && (directive == "else" || directive == "endif" || directive.starts_with("elif"))
                && let Some(sibling) = self.output.iter().rev().skip(1).find(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
            {
                let sibling_code = sibling[..trailing_comment_split_limit(sibling)].trim_end();
                if sibling_code.ends_with(';') && !is_comment_line(sibling.trim_start()) {
                    let sibling_spaces = leading_visual_width(sibling, self.options.tab_width);
                    let spaces = (directive != "endif")
                        .then(|| {
                            self.preprocessor
                                .branch_stack
                                .last()
                                .and_then(|branch| branch.first_body_indent_spaces)
                        })
                        .flatten()
                        .unwrap_or(sibling_spaces);
                    let has_block_header = directive != "endif"
                        || self.output.iter().rev().skip(1).take(16).any(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            code.ends_with('{')
                                && leading_visual_width(line, self.options.tab_width)
                                    + self.options.indent_width
                                    == spaces
                        });
                    if has_block_header
                        && layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces
                    {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && self.frame_stack.active_brace().is_none_or(|frame| {
                frame.header.as_deref().is_none_or(|header| {
                    starts_header_word(header, "if")
                        || starts_header_word(header, "while")
                        || starts_header_word(header, "for")
                        || header.starts_with("else if")
                })
            })
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_trimmed = previous.trim_start();
            if previous_trimmed.ends_with('{')
                && (starts_header_word(previous_trimmed, "if")
                    || starts_header_word(previous_trimmed, "while")
                    || starts_header_word(previous_trimmed, "for")
                    || previous_trimmed.starts_with("else if"))
                && let Some(branch) = self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                && preprocessor_directive(
                    branch[..trailing_comment_split_limit(branch)]
                        .trim_end()
                        .trim_start(),
                )
                .is_some_and(|directive| {
                    self.output
                        .iter()
                        .rev()
                        .skip_while(|line| line.as_str() != branch.as_str())
                        .skip(1)
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            let trimmed = code.trim_start();
                            match directive {
                                "if" | "ifdef" | "ifndef" => {
                                    trimmed == "else" || trimmed.ends_with("} else")
                                }
                                "endif" => code.ends_with(';'),
                                _ => false,
                            }
                        })
                })
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && line.trim_start().starts_with("} else")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let mut seen_header = false;
            let mut seen_comment = false;
            let after_braceless_else_comment = self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty())
                .skip(1)
                .take(12)
                .any(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    if !seen_header {
                        seen_header = code.ends_with('{')
                            && (starts_header_word(trimmed, "if")
                                || starts_header_word(trimmed, "while")
                                || starts_header_word(trimmed, "for")
                                || trimmed.starts_with("else if"));
                        return false;
                    }
                    if is_comment_line(line.trim_start()) {
                        seen_comment = true;
                        return false;
                    }
                    seen_comment && (trimmed == "else" || trimmed.ends_with("} else"))
                });
            if previous_code.ends_with(';')
                && !is_comment_line(previous.trim_start())
                && after_braceless_else_comment
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && line.trim() == "}"
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let mut seen_header = false;
            let mut seen_comment = false;
            let mut blocked = false;
            let after_braceless_else_comment = self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty())
                .skip(1)
                .take(12)
                .any(|line| {
                    if blocked {
                        return false;
                    }
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    if !seen_header {
                        if trimmed.starts_with('}') {
                            blocked = true;
                            return false;
                        }
                        seen_header = code.ends_with('{')
                            && (starts_header_word(trimmed, "if")
                                || starts_header_word(trimmed, "while")
                                || starts_header_word(trimmed, "for")
                                || trimmed.starts_with("else if"));
                        return false;
                    }
                    if is_comment_line(line.trim_start()) {
                        seen_comment = true;
                        return false;
                    }
                    seen_comment && (trimmed == "else" || trimmed.ends_with("} else"))
                });
            let previous_is_closing_brace = previous_code.trim() == "}";
            let recent_preprocessor = self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty())
                .skip(1)
                .take(12)
                .any(|line| line.trim_start().starts_with('#'));
            if ((previous_code.ends_with(';') && !previous_code.ends_with("};"))
                || (previous_is_closing_brace && recent_preprocessor))
                && !is_comment_line(previous.trim_start())
            {
                let mut spaces = leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if previous_code.ends_with(';')
                    && let Some(braceless_else) = self
                        .output
                        .iter()
                        .rev()
                        .skip(1)
                        .find(|line| !line.trim().is_empty())
                    && {
                        let trimmed = braceless_else
                            [..trailing_comment_split_limit(braceless_else)]
                            .trim_end()
                            .trim_start();
                        trimmed == "else" || trimmed.ends_with("} else")
                    }
                {
                    spaces = leading_visual_width(braceless_else, self.options.tab_width)
                        .saturating_sub(self.options.indent_width);
                }
                let recent_matching_header = self
                    .output
                    .iter()
                    .rev()
                    .filter(|line| !line.trim().is_empty())
                    .skip(1)
                    .take(16)
                    .take_while(|line| {
                        !line.trim_start().starts_with('}')
                            || leading_visual_width(line, self.options.tab_width) > spaces
                    })
                    .any(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        code.ends_with('{')
                            && leading_visual_width(line, self.options.tab_width) == spaces
                    });
                if (after_braceless_else_comment || recent_matching_header)
                    && layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces
                {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_normal_literal_comma_and_split_else_entry_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let split_else_state_active = contextual.split_else_state_active;
        let layout = &mut contextual.layout;
        if layout.line_kind == LineKind::Normal
            && self.line_adjuster.total_case_unindent_depth() > 0
            && line
                .trim_start()
                .strip_prefix(['+', '-'])
                .is_some_and(|tail| tail.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
            && self.output.last_non_empty_line().is_some_and(|previous| {
                previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with(',')
            })
            && let Some(spaces) = layout.exact_indent_spaces.as_mut()
        {
            *spaces = spaces.saturating_sub(1);
        }
        if layout.exact_indent_spaces == Some(0)
            && replay.input_continuation_indent.is_none()
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with('(')
                && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous_index) = self.output.last_non_empty_index()
            && self.output[previous_index].trim_end().ends_with("\";")
        {
            let normal_spaces = layout.normal_indent * self.options.indent_width;
            if layout.exact_indent_spaces.unwrap_or(normal_spaces) < normal_spaces {
                layout.exact_indent_spaces = Some(normal_spaces);
            }
        }
        if layout.line_kind == LineKind::Normal
            && starts_string_literal_token(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if starts_with_chain_operator(previous_trimmed)
                && let Some(string_start) = first_string_literal_start(previous_code)
            {
                let before_string = &previous_code[..string_start];
                if let Some(open) = before_string.rfind('(')
                    && before_string[open + 1..].contains(',')
                {
                    layout.exact_indent_spaces = Some(
                        open + 1
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && let Some(previous_index) = self.output.last_non_empty_index()
            && self.output[previous_index].trim_end().ends_with(',')
        {
            let previous = &self.output[previous_index];
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if !previous_code.contains(" new ")
                && !previous_code.contains("(new ")
                && !previous_code.contains('{')
                && let Some(string_start) = first_string_literal_start(previous_code)
                && previous_code[..string_start].contains(',')
                && let Some(open) = unmatched_open_paren_columns(previous_code).last().copied()
                && previous_code[open + 1..].contains(',')
            {
                let spaces = open
                    + 1
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
            if self.options.brace_style == BraceStyle::None
                && !previous_code.contains(" new ")
                && !previous_code.contains("(new ")
                && let Some(open) = unmatched_open_paren_column(previous_code)
                && self.commented_split_else_preprocessor_region_active()
            {
                let spaces = (open + 1).max(leading_visual_width(previous, self.options.tab_width));
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        let none_style_split_else_comma_spaces = self.none_style_split_else_comma_indent_spaces(
            line,
            layout.line_kind,
            split_else_state_active,
        );
        if let Some(spaces) = none_style_split_else_comma_spaces {
            layout.exact_indent_spaces = Some(spaces);
        }
        if none_style_split_else_comma_spaces.is_none()
            && let Some(spaces) = self.none_style_split_else_logical_indent_spaces(
                line,
                layout.line_kind,
                split_else_state_active,
            )
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && !line.trim_start().starts_with(['#', '{', '}'])
            && !is_comment_line(line.trim_start())
            && split_else_state_active
            && self.output.last_non_empty_line().is_some()
            && self.commented_split_else_preprocessor_region_active()
            && line.trim_end().ends_with(");")
        {
            self.continuation_indent.clear_continuation_after_line =
                Some(layout.normal_indent * self.options.indent_width);
        }
        if let Some(spaces) = self.none_style_split_else_blank_gap_sibling_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.none_style_post_comment_sibling_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        contextual
    }

    pub(in super::super) fn apply_none_style_split_else_body_and_closing_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let split_else_state_active = contextual.split_else_state_active;
        let layout = &mut contextual.layout;
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && !line.trim_start().starts_with(['#', '{', '}'])
            && let Some(previous_index) = self.output.last_non_empty_index()
        {
            let previous_trimmed = self.output[previous_index].trim_start();
            if previous_trimmed.ends_with('{')
                && (starts_header_word(previous_trimmed, "if")
                    || starts_header_word(previous_trimmed, "while")
                    || starts_header_word(previous_trimmed, "for")
                    || previous_trimmed.starts_with("else if"))
            {
                let tab_width = if self.options.indent_style == IndentStyle::Tabs {
                    self.options.indent_width
                } else {
                    self.options.tab_width
                };
                let spaces =
                    self.output.lead_width(previous_index, tab_width) + self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces
                    || self.output[..previous_index]
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| is_comment_line(line.trim_start()))
                {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::None
            && line.trim() == "}"
            && self.token_input.token_source_line_indent == 0
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.trim() == "}")
            && self
                .output
                .iter()
                .rev()
                .take(16)
                .any(|line| preprocessor_directive(line.trim_start()) == Some("endif"))
        {
            layout.exact_indent_spaces = Some(0);
        }
        if let Some(spaces) = self.split_else_immediate_post_comment_indent_floor(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_none_style_closing_indent_spaces(
            line,
            layout.line_kind,
            split_else_state_active,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.none_style_split_else_closing_header_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.none_style_split_else_body_indent_floor(
            line,
            layout.line_kind,
            split_else_state_active,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.none_style_conditional_closing_brace_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        let none_style_split_else_comma_spaces = self.none_style_split_else_comma_indent_spaces(
            line,
            layout.line_kind,
            split_else_state_active,
        );
        if let Some(spaces) = none_style_split_else_comma_spaces {
            layout.exact_indent_spaces = Some(spaces);
        }
        if none_style_split_else_comma_spaces.is_none()
            && let Some(spaces) = self.none_style_split_else_logical_indent_spaces(
                line,
                layout.line_kind,
                split_else_state_active,
            )
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if self.options.brace_style == BraceStyle::None
            && !line.trim_start().starts_with(['#', '{', '}'])
            && !is_comment_line(line.trim_start())
            && split_else_state_active
            && self.output.last_non_empty_line().is_some()
            && self.commented_split_else_preprocessor_region_active()
            && line.trim_end().ends_with(");")
        {
            self.continuation_indent.clear_continuation_after_line =
                Some(layout.normal_indent * self.options.indent_width);
        }
        contextual
    }

    pub(in super::super) fn apply_emitted_split_else_call_initializer_and_ternary_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let split_else_state_active = contextual.split_else_state_active;
        let layout = &mut contextual.layout;
        let current_may_need_preprocessor_else_context = {
            let trimmed = line.trim_start();
            split_else_state_active
                || starts_string_literal_token(trimmed)
                || trimmed.starts_with([
                    ')', ',', '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', '.',
                ])
        };
        if current_may_need_preprocessor_else_context
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let in_preprocessor_else_context = self.recent_split_else_preprocessor_region_active();
            if let Some(spaces) = self.split_else_adjacent_string_argument_indent_spaces(
                line,
                in_preprocessor_else_context,
                false,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(call_layout) =
                self.split_else_string_comma_argument_layout(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(call_layout.indent_spaces);
                if let Some(spaces) = call_layout.clear_continuation_after_line {
                    self.continuation_indent.clear_continuation_after_line = Some(spaces);
                }
            }
            if let Some(call_layout) =
                self.split_else_call_closing_layout(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(call_layout.indent_spaces);
                if let Some(spaces) = call_layout.clear_continuation_after_line {
                    self.continuation_indent.clear_continuation_after_line = Some(spaces);
                }
            }
            if let Some(spaces) = self.split_else_following_assignment_call_indent_spaces(
                line,
                in_preprocessor_else_context,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) =
                self.split_else_local_type_body_indent_spaces(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(call_layout) =
                self.split_else_comma_argument_layout(line, in_preprocessor_else_context)
            {
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < call_layout.indent_spaces {
                    layout.exact_indent_spaces = Some(call_layout.indent_spaces);
                }
                if let Some(spaces) = call_layout.clear_continuation_after_line {
                    self.continuation_indent.clear_continuation_after_line = Some(spaces);
                }
            }
            if let Some(spaces) =
                self.split_else_following_call_indent_spaces(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if in_preprocessor_else_context
                && previous_code.ends_with(';')
                && !starts_string_literal_token(previous_code.trim_start())
                && !line.trim_start().starts_with(['#', '{', '}', ')'])
                && !is_comment_line(line.trim_start())
                && !(previous_code.trim_start().starts_with(");")
                    && (self.output.iter().rev().skip(1).take(8).any(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        assignment_call_value_column(code, self.options.tab_width).is_some()
                    }) || (self
                        .output
                        .iter()
                        .rev()
                        .skip(1)
                        .take(8)
                        .any(|line| starts_string_literal_token(line.trim_start()))
                        && self.output.iter().rev().skip(1).take(8).any(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            code.ends_with(',') && unmatched_open_paren_column(code).is_some()
                        }))))
            {
                let previous_spaces = leading_visual_width(previous, self.options.tab_width);
                if let Some(spaces) =
                    self.split_else_post_local_type_statement_indent_spaces(previous_spaces)
                {
                    layout.exact_indent_spaces = Some(spaces);
                } else if previous_spaces
                    > layout.exact_indent_spaces.unwrap_or(output_spaces)
                        + self.options.indent_width
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
                    layout.exact_indent_spaces = Some(previous_spaces);
                }
            }
            if in_preprocessor_else_context
                && previous_code.ends_with(',')
                && previous_code.trim_start().starts_with('{')
                && line.trim_start().starts_with('{')
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
            if let Some(spaces) = self.preprocessor_interrupted_closing_brace_indent_spaces(
                line,
                in_preprocessor_else_context,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if in_preprocessor_else_context
                && previous_code.trim() == "}"
                && !line.trim_start().starts_with(['#', '{', '}'])
                && !is_comment_line(line.trim_start())
            {
                let spaces = leading_visual_width(previous, self.options.tab_width);
                if layout.exact_indent_spaces.unwrap_or(output_spaces) < spaces {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
            if let Some(spaces) = self.preprocessor_interrupted_closing_header_indent_spaces(
                line,
                in_preprocessor_else_context,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) =
                self.split_else_ternary_sibling_indent_spaces(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self
                .split_else_completed_ternary_call_indent_spaces(line, in_preprocessor_else_context)
            {
                layout.exact_indent_spaces = Some(spaces);
                self.continuation_indent.clear_continuation_after_line = Some(spaces);
            }
            let in_split_preprocessor_else_context =
                self.commented_split_else_preprocessor_region_active();
            if let Some(spaces) = self.split_else_ternary_comma_sibling_indent_floor(
                line,
                in_split_preprocessor_else_context,
                layout.exact_indent_spaces.unwrap_or(output_spaces),
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.split_else_completed_ternary_call_indent_spaces(
                line,
                in_split_preprocessor_else_context,
            ) {
                layout.exact_indent_spaces = Some(spaces);
                self.continuation_indent.clear_continuation_after_line = Some(spaces);
            }
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_header_label_and_switch_contextual_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if let Some(spaces) = self.preprocessor_interrupted_else_if_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if is_comment_line(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with('{') && previous_code.trim_start().starts_with("} else") {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if (line.trim_start().starts_with("} while") || line.trim_start().starts_with("}while"))
            && let Some((spaces, _, trimmed)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && starts_header_word(trimmed, "do")
        {
            layout.exact_indent_spaces = Some(
                spaces
                    + self.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if let Some(spaces) = self.emitted_case_body_indent_spaces(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if !line.trim_start().starts_with(['{', '}', '#'])
                && code.ends_with('{')
                && (starts_header_word(trimmed, "if")
                    || starts_header_word(trimmed, "while")
                    || starts_header_word(trimmed, "for")
                    || starts_header_word(trimmed, "do")
                    || trimmed.starts_with("else if")
                    || trimmed.starts_with("} else"))
            {
                layout.exact_indent_spaces = Some(layout.exact_indent_spaces.unwrap_or(0).max(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                ));
            } else if !line.trim_start().starts_with(['{', '}', '#'])
                && !code.ends_with([';', '{', '}'])
                && unmatched_open_paren_column(code).is_none()
                && line_is_control_body_header(trimmed)
            {
                layout.exact_indent_spaces = Some(layout.exact_indent_spaces.unwrap_or(0).max(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                ));
            } else if !line.trim_start().starts_with(['{', '}', '#', ':'])
                && !line.trim_start().starts_with("->")
                && code.ends_with(')')
                && !trimmed.starts_with('#')
                && let Some(header) = self.output.iter().rev().skip(1).take(8).find(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    line_is_control_body_header(trimmed)
                        || starts_header_word(trimmed, "for")
                        || starts_header_word(trimmed, "while")
                        || starts_header_word(trimmed, "if")
                })
            {
                layout.exact_indent_spaces = Some(layout.exact_indent_spaces.unwrap_or(0).max(
                    leading_visual_width(header, self.options.tab_width)
                        + self.options.indent_width
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                ));
            } else if !line.trim_start().starts_with(['{', '}', '#'])
                && code.ends_with("{")
                && trimmed.contains(')')
                && let Some(header) = self.output.iter().rev().skip(1).take(3).find(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    line_is_control_body_header(trimmed)
                        || starts_header_word(trimmed, "for")
                        || starts_header_word(trimmed, "while")
                        || starts_header_word(trimmed, "if")
                })
            {
                let header_indent = leading_visual_width(header, self.options.tab_width);
                if header_indent < leading_visual_width(previous, self.options.tab_width) {
                    layout.exact_indent_spaces = Some(layout.exact_indent_spaces.unwrap_or(0).max(
                        header_indent
                            + self.options.indent_width
                            + self.line_adjuster.next_line_case_unindent_depth()
                                * self.options.indent_width,
                    ));
                }
            }
        }
        if let Some(spaces) = labels::current_line_indent_spaces(
            layout.line_kind,
            line,
            self.stack_state.brace_type_stack.last().copied(),
            self.options,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.following_label_body_indent_spaces(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.split_else_matching_if_indent_spaces(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.label_block_indent_spaces(line, layout.exact_indent_spaces) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.same_line_nested_header_closing_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim() == "}"
            && let Some((open_spaces, _, open_trimmed)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && open_trimmed.starts_with("switch")
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim() == "}")
        {
            layout.exact_indent_spaces = Some(open_spaces);
        }
        if line.trim() == "}"
            && let Some((open_spaces, _, open_trimmed)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
            && open_trimmed.contains(')')
            && !line_is_control_body_header(open_trimmed)
            && !starts_header_word(open_trimmed, "if")
            && !starts_header_word(open_trimmed, "for")
            && !starts_header_word(open_trimmed, "while")
            && !starts_header_word(open_trimmed, "switch")
            && !starts_header_word(open_trimmed, "do")
            && !open_trimmed.starts_with("case ")
            && !open_trimmed.starts_with("default:")
            && !matches!(
                self.stack_state.last_closed_brace_header.as_deref(),
                Some("case" | "default")
            )
            && layout
                .exact_indent_spaces
                .unwrap_or(layout.indent * self.options.indent_width)
                > open_spaces
        {
            layout.exact_indent_spaces = Some(open_spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line.trim() != "break;"
            && !line.trim_start().starts_with(['#', '{', '}'])
            && self.stack_state.last_closed_brace_header.as_deref() == Some("switch")
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim() == "}"
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if layout
                .exact_indent_spaces
                .unwrap_or(layout.indent * self.options.indent_width)
                > previous_indent
            {
                layout.exact_indent_spaces = Some(previous_indent);
            }
        }
        if self.options.brace_style == BraceStyle::None
            && !line.trim_start().starts_with(['#', '}'])
            && self.line_adjuster.next_line_case_unindent_depth() > 0
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if !previous_code.ends_with('{')
                && previous_code.trim() != "}"
                && let Some(header) = self.output.iter().rev().take(8).find(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    (line_is_control_body_header(code.trim_start())
                        || starts_header_word(code.trim_start(), "for")
                        || starts_header_word(code.trim_start(), "while")
                        || starts_header_word(code.trim_start(), "if"))
                        && code.contains('(')
                        && !code.contains(')')
                })
            {
                layout.exact_indent_spaces = Some(layout.exact_indent_spaces.unwrap_or(0).max(
                    leading_visual_width(header, self.options.tab_width)
                        + self.options.indent_width * 2
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                ));
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && case_label_with_trailing_comment(previous.trim())
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width
                    + self.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if replay.input_continuation_indent.is_none()
            && !line.trim_start().starts_with(['#', '{', '}'])
            && (is_comment_line(line.trim_start())
                || self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|previous| {
                        is_comment_line(previous.trim_start())
                            && !previous.trim_end().ends_with(';')
                    }))
            && let Some(spaces) = self.recent_paren_continuation_indent_spaces()
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.preprocessor_else_comment_sibling_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && preprocessor_directive(previous.trim_start()) == Some("endif")
        {
            let mut before_lines = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .filter(|line| !line.trim().is_empty());
            if let Some(before) = before_lines.next() {
                let before_code = before[..trailing_comment_split_limit(before)].trim_end();
                let before_trimmed = before_code.trim_start();
                if before_trimmed == "} else" || before_trimmed == "}else" {
                    layout.exact_indent_spaces =
                        Some(leading_visual_width(before, self.options.tab_width));
                }
            }
        }
        let header_operator_spaces = self.header_operator_continuation_indent_spaces(line);
        if let Some(spaces) = self.split_else_header_operator_case_compensation_indent_spaces(
            line,
            layout.exact_indent_spaces,
            header_operator_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = layout.exact_indent_spaces.as_mut()
            && line.trim() == "}"
            && self.line_adjuster.total_case_unindent_depth() > 0
            && self.current_closes_same_line_else_block()
        {
            *spaces += self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        }
        contextual
    }

    pub(in super::super) fn apply_structural_split_else_body_contextual_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let layout = &mut contextual.layout;
        if let Some(body_context) = self.structural_split_else_body_context(line, layout.line_kind)
        {
            let structural_split_else_chain = body_context.structural_chain();
            let current_spaces = layout.exact_indent_spaces.unwrap_or(output_spaces);
            if let Some(spaces) =
                self.split_else_brace_logical_indent_spaces(line, structural_split_else_chain)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.split_else_adjacent_string_argument_indent_spaces(
                line,
                structural_split_else_chain,
                true,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self
                .structural_split_else_string_comma_indent_spaces(line, structural_split_else_chain)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.structural_split_else_following_string_call_indent_spaces(
                line,
                structural_split_else_chain,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.structural_split_else_string_call_close_indent_spaces(
                line,
                structural_split_else_chain,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.structural_split_else_closing_brace_indent_spaces(
                line,
                current_spaces,
                structural_split_else_chain,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            } else if let Some(spaces) = self.structural_split_else_closing_header_indent_spaces(
                line,
                current_spaces,
                structural_split_else_chain,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            } else if !line.trim_start().starts_with(['{', '}']) {
                if let Some(spaces) = self.split_else_completed_logical_statement_indent_spaces() {
                    layout.exact_indent_spaces = Some(spaces);
                } else if let Some(spaces) =
                    self.split_else_branch_body_indent_override(current_spaces)
                {
                    layout.exact_indent_spaces = Some(spaces);
                } else if let Some(spaces) = self.structural_split_else_ordinary_row_indent_spaces(
                    line,
                    current_spaces,
                    &body_context,
                ) {
                    layout.exact_indent_spaces = Some(spaces);
                } else if let Some(spaces) = self.structural_split_else_post_comment_indent_spaces(
                    line,
                    current_spaces,
                    body_context.body_indent_spaces(),
                    structural_split_else_chain,
                ) {
                    layout.exact_indent_spaces = Some(spaces);
                } else if let Some(spaces) = self.structural_split_else_trailing_body_indent_spaces(
                    current_spaces,
                    &body_context,
                ) {
                    layout.exact_indent_spaces = Some(spaces);
                }
            }
        }
        if let Some(spaces) = self.over_max_new_call_adjacent_string_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_condition_closing_paren_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_adjacent_string_call_close_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_assignment_logical_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.preprocessor_else_comment_sibling_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_string_call_and_emitted_split_else_case_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let split_else_state_active = contextual.split_else_state_active;
        let layout = &mut contextual.layout;
        if self.output.iter().rev().any(|line| !line.trim().is_empty()) {
            if let Some(call_layout) = self.string_call_continuation_layout(line) {
                layout.exact_indent_spaces = Some(call_layout.indent_spaces);
                if let Some(spaces) = call_layout.clear_continuation_after_line {
                    self.continuation_indent.clear_continuation_after_line = Some(spaces);
                }
            } else if let Some(spaces) =
                self.multiline_control_header_body_indent_spaces(line, layout.line_kind)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if line.trim() == "{"
            && let Some(spaces) = layout.exact_indent_spaces
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| preprocessor_directive(previous.trim_start()).is_some())
            && let Some(before_preprocessor) = self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
        {
            let code = before_preprocessor[..trailing_comment_split_limit(before_preprocessor)]
                .trim_end()
                .trim_start();
            if starts_string_literal_token(code) && code.ends_with(';') {
                layout.exact_indent_spaces = Some(spaces.saturating_sub(self.options.indent_width));
            }
        }
        let split_preprocessor_else_context =
            self.split_else_preprocessor_context(split_else_state_active);
        if split_preprocessor_else_context.layout_active()
            && !line.trim_start().starts_with(['{', '}', '#'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            let previous_raw_trimmed = previous.trim_start();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let current_spaces = layout
                .exact_indent_spaces
                .unwrap_or(layout.indent * self.options.indent_width);
            if let Some(spaces) = self.split_else_case_call_sibling_indent_spaces(
                line,
                split_preprocessor_else_context.layout_active(),
            ) {
                layout.exact_indent_spaces = Some(spaces);
            } else if previous_code.ends_with(';')
                && self.line_adjuster.total_case_unindent_depth() > 0
                && current_spaces <= previous_indent
                && self.output.iter().rev().skip(1).take(16).any(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    code.ends_with('{')
                        && leading_visual_width(line, self.options.tab_width)
                            + self.options.indent_width
                            == previous_indent
                        && (starts_header_word(trimmed, "if")
                            || starts_header_word(trimmed, "for")
                            || starts_header_word(trimmed, "while")
                            || trimmed.starts_with("else"))
                })
            {
                layout.exact_indent_spaces = Some(
                    previous_indent
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            } else if previous_code.ends_with(';')
                && starts_header_word(previous_trimmed, "if")
                && current_spaces <= previous_indent
            {
                layout.exact_indent_spaces = Some(
                    previous_indent
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            } else if previous_code.ends_with(';')
                && self.line_adjuster.total_case_unindent_depth() > 0
                && current_spaces <= previous_indent
                && (line_is_control_body_header(line.trim_start())
                    || starts_header_word(line.trim_start(), "if")
                    || starts_header_word(line.trim_start(), "while")
                    || starts_header_word(line.trim_start(), "for")
                    || starts_header_word(line.trim_start(), "switch")
                    || is_comment_line(line.trim_start()))
            {
                layout.exact_indent_spaces = Some(
                    previous_indent
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            } else if (is_comment_line(previous_raw_trimmed)
                || previous_raw_trimmed.starts_with("/*"))
                && current_spaces != previous_indent
                && current_spaces.abs_diff(previous_indent) <= self.options.indent_width
            {
                layout.exact_indent_spaces = Some(previous_indent);
            }
        }
        if let Some(spaces) = self.split_else_endif_sibling_indent_spaces(
            line,
            split_preprocessor_else_context.emitted_region_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if split_preprocessor_else_context.emitted_region_active()
            && line.contains(" ? ")
            && !line.trim_start().starts_with(['#', '{', '}', '?', ':'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',') && unmatched_open_paren_column(previous_code).is_none()
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some(spaces) = self.split_else_switch_comment_indent_spaces(
            line,
            split_preprocessor_else_context.emitted_region_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_case_closing_indent_floor(
            line,
            split_preprocessor_else_context.layout_active(),
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if let Some(spaces) = self.split_else_adjusted_case_indent_floor(
            line,
            layout.line_kind,
            split_preprocessor_else_context.layout_active(),
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) = self.split_else_initializer_closing_indent_spaces(
            line,
            split_preprocessor_else_context.layout_active(),
            case_unindent,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) = self.split_else_local_type_line_indent_spaces(
            line,
            layout.line_kind,
            split_preprocessor_else_context.layout_active(),
            case_unindent,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_switch_label_indent_spaces(
            layout.line_kind,
            split_preprocessor_else_context.layout_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_case_body_indent_spaces(
            line,
            layout.line_kind,
            split_preprocessor_else_context.layout_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && let Some(spaces) = self.split_else_comma_sibling_indent_spaces(
                line,
                split_preprocessor_else_context.layout_active(),
            )
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.string_call_closing_indent_spaces(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self
            .string_argument_after_comma_indent_floor(line, layout.exact_indent_spaces.unwrap_or(0))
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_case_closed_block_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_commented_aggregate_member_indent_spaces(
            line,
            layout.line_kind,
            self.split_else_body_indent_active(),
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_braced_member_body_indent_spaces(
            line,
            layout.line_kind,
            layout.normal_indent,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_case_completed_call_indent_spaces(
            line,
            layout.line_kind,
            layout.normal_indent,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.split_else_closing_indent_ceiling(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.active_split_else_comma_and_string_indent_floor(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        contextual
    }

    pub(in super::super) fn apply_macro_case_brace_and_return_contextual_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let output_spaces = contextual.output_spaces;
        let layout = &mut contextual.layout;
        if let Some(spaces) = layout.exact_indent_spaces.as_mut()
            && is_macro_like_word(leading_identifier(line))
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if previous_code.ends_with(';')
                && is_macro_like_word(leading_identifier(previous_trimmed))
                && *spaces > previous_indent
            {
                *spaces = previous_indent;
            }
        }
        if let Some(spaces) = layout.exact_indent_spaces.as_mut() {
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if case_unindent > 0 {
                if let Some(adjusted) =
                    self.logical_case_unindent_adjusted_spaces(line, *spaces, layout.normal_indent)
                {
                    *spaces = adjusted;
                } else if let Some(adjusted) =
                    self.case_parenthesized_block_indent_spaces(line, *spaces)
                {
                    *spaces = adjusted;
                } else if let Some(adjusted) = self.aggregate_member_case_indent_spaces(
                    *spaces,
                    layout.normal_indent,
                    case_unindent,
                ) {
                    *spaces = adjusted;
                } else if let Some(adjusted) =
                    self.case_control_indent_floor(line, layout.normal_indent, *spaces)
                {
                    *spaces = adjusted;
                }
            }
        }
        if let Some(spaces) = self.root_preprocessor_closing_brace_indent_reset(
            line,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.preprocessor_closing_header_indent_spaces(
            line,
            layout.normal_indent,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.closing_header_body_brace_indent_spaces(
            line,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.nested_if_closing_brace_indent_reset(
            line,
            layout.exact_indent_spaces,
            output_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("return ")
            && layout
                .exact_indent_spaces
                .is_some_and(|spaces| spaces > layout.normal_indent * self.options.indent_width)
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.trim() == "}")
        {
            layout.exact_indent_spaces = Some(layout.normal_indent * self.options.indent_width);
        }
        if let Some(spaces) =
            self.nested_closing_brace_indent_reset(line, layout.exact_indent_spaces, output_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.case_post_comment_sibling_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.contains("*INDENT-ON*")
        {
            layout.exact_indent_spaces = Some(layout.normal_indent * self.options.indent_width);
        }
        if line.trim() == "{"
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|line| preprocessor_directive(line.trim_start()) == Some("endif"))
        {
            layout.exact_indent_spaces = Some(
                self.frame_stack
                    .active_brace()
                    .filter(|frame| {
                        frame.semantic_kind == BraceSemanticKind::Command && frame.header.is_some()
                    })
                    .map_or(layout.normal_indent * self.options.indent_width, |frame| {
                        frame.sibling_indent_column
                    }),
            );
        }
        if let Some(spaces) = self.ratliff_closing_brace_indent_spaces(line, layout.normal_indent) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous.trim_end();
            let previous_trimmed = previous_code.trim_start();
            if previous_code.ends_with('}')
                && previous_code.contains('{')
                && (line_is_control_body_header(previous_trimmed)
                    || starts_header_word(previous_trimmed, "if")
                    || starts_header_word(previous_trimmed, "for")
                    || starts_header_word(previous_trimmed, "while"))
                && !matches!(leading_identifier(line), "else" | "catch" | "while")
            {
                layout.exact_indent_spaces = Some(layout.normal_indent * self.options.indent_width);
            }
        }
        if let Some(spaces) = self
            .same_line_nested_header_closing_brace_indent_floor(line, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(floor) = self.call_shaped_brace_body_indent_floor(line, layout.normal_indent)
            && layout.exact_indent_spaces.unwrap_or(0) < floor
        {
            layout.exact_indent_spaces = Some(floor);
        }
        contextual
    }

    pub(in super::super) fn apply_conditional_literal_paren_and_else_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if preprocessor_directive(previous_trimmed)
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
                && let Some(branch) = self.output.iter().rev().skip(1).find(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
            {
                let branch_code = branch[..trailing_comment_split_limit(branch)].trim_end();
                let branch_trimmed = branch_code.trim_start();
                if branch_trimmed == "else"
                    || branch_trimmed.ends_with("} else")
                    || branch_trimmed.ends_with(" else")
                {
                    layout.exact_indent_spaces = Some(
                        leading_visual_width(branch, self.options.tab_width)
                            + self.options.indent_width,
                    );
                }
            } else if previous_code.contains("#endif")
                && !previous_trimmed.starts_with('#')
                && !previous_code.ends_with(';')
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', ')'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if !previous_code.ends_with([';', '{', '}'])
                && unmatched_open_paren_column(previous_code).is_none()
                && (is_braceless_header_line(previous_trimmed)
                    || starts_header_word(previous_trimmed, "foreach"))
            {
                let target = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            } else if matches!(
                self.stack_state.last_closed_brace_type,
                Some(FormatterBraceType::CompoundLiteral)
            ) && previous_trimmed.starts_with('}')
                && previous_code.ends_with(')')
                && line
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(is_identifier_start)
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if layout.normal_indent == 0
            && !line.trim_start().starts_with(['#', '{', '}', ')'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                let open = visual_width_from(&previous_code[..open], 0, self.options.tab_width);
                if open > self.options.max_continuation_indent {
                    layout.exact_indent_spaces = Some(self.options.indent_width * 2);
                }
            }
        }
        if starts_string_literal_token(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
            && starts_string_literal_token(previous.trim_start())
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        if self.options.indent_after_parens
            && layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', ')'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',')
                && !previous_code
                    .split(|ch: char| !is_identifier_continue(ch))
                    .any(|word| word == "new")
                && (replay.closed_delimiter_continuation_indent.is_some()
                    || self
                        .frame_stack
                        .active_delimiter()
                        .is_some_and(|delimiter| delimiter.opener_output_line < self.output.len()))
                && let Some(spaces) = if self
                    .frame_stack
                    .active_brace()
                    .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
                {
                    replay
                        .input_continuation_indent
                        .map(|indent| indent.columns(self.options.indent_width))
                        .or(replay.closed_delimiter_continuation_indent)
                        .or_else(|| self.stack_state.current_continuation_indent_spaces())
                } else {
                    replay
                        .closed_delimiter_continuation_indent
                        .or_else(|| {
                            replay
                                .input_continuation_indent
                                .map(|indent| indent.columns(self.options.indent_width))
                        })
                        .or_else(|| self.stack_state.current_continuation_indent_spaces())
                }
            {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) = self.else_after_braceless_body_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("else")
            && let Some(spaces) = layout.exact_indent_spaces.as_mut()
        {
            let case_unindent =
                self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            let normal_spaces = layout.normal_indent * self.options.indent_width;
            if case_unindent > 0 && *spaces < normal_spaces {
                *spaces += case_unindent;
            }
        }
        if self.options.no_indent_if_after_else
            && starts_header_word(line.trim_start(), "if")
            && let Some(previous) = self.output.last_non_empty_line()
            && matches!(previous.trim(), "else" | "} else")
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        contextual
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_call_initializer_and_case_control_contextual_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .split_once('(')
                .is_some_and(|(word, _)| is_macro_like_word(word.trim()))
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim() == "}"
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
            );
        }
        if !self.options.indent_after_parens
            && layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',') {
                let base = leading_visual_width(previous, self.options.tab_width);
                let opens = unmatched_open_paren_columns(previous_code);
                if previous_code.contains(" new ")
                    && let Some(new_index) = previous_code.rfind(" new ")
                {
                    if let Some(inner) = opens.iter().copied().find(|open| *open > new_index)
                        && inner.saturating_sub(base) >= self.options.max_continuation_indent
                    {
                        let spaces = opens
                            .iter()
                            .rev()
                            .copied()
                            .find(|open| *open < new_index)
                            .map_or(base + self.options.indent_width * 2, |_| new_index + 1);
                        layout.exact_indent_spaces = Some(spaces);
                    }
                } else if opens.len() >= 2
                    && let Some(inner) = opens.last().copied()
                    && inner.saturating_sub(base) >= self.options.max_continuation_indent
                {
                    let outer = opens[opens.len() - 2];
                    layout.exact_indent_spaces = Some(
                        if outer.saturating_sub(base) >= self.options.max_continuation_indent {
                            base + self.options.indent_width * 2
                        } else {
                            outer + 1
                        },
                    );
                }
                let previous_trimmed = previous_code.trim_start();
                if base > self.options.indent_width * 2
                    && !opens.is_empty()
                    && previous_trimmed
                        .split_once('(')
                        .is_some_and(|(callee, _)| is_macro_like_word(callee.trim()))
                    && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
                {
                    let spaces = base + self.options.indent_width * 2;
                    if layout.exact_indent_spaces.unwrap_or(0) < spaces {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                }
            }
        }
        if let Some(brace_layout) = self.compound_literal_opening_layout(
            line,
            layout.normal_indent,
            layout.indent,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(brace_layout.line_indent_spaces);
            self.update_current_brace_indent_columns(
                brace_layout.brace_indent_spaces + self.options.indent_width,
                brace_layout.brace_indent_spaces,
            );
        }
        if self.options.indent_style == IndentStyle::Tabs
            && layout.indent > layout.normal_indent
            && self.token_input.token_source_line_indent
                > layout.normal_indent * self.options.indent_width
            && let Some(spaces) = layout.exact_indent_spaces
            && spaces > self.token_input.token_source_line_indent
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with(',')
            && !line.trim_start().starts_with(['#', '{', '}', ')'])
        {
            layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
        }
        if layout.line_kind == LineKind::Normal
            && !self.options.indent_after_parens
            && line.trim_start().starts_with('*')
            && let Some(spaces) = self.function_parameter_continuation_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        let line_in_case_control_block = if line.trim() == "}" {
            self.output
                .current_closing_brace_open(self.options.tab_width)
                .is_some_and(|(_, _, open_trimmed)| {
                    starts_header_word(open_trimmed, "if")
                        || starts_header_word(open_trimmed, "for")
                        || starts_header_word(open_trimmed, "while")
                        || starts_header_word(open_trimmed, "do")
                        || open_trimmed.starts_with("else")
                })
        } else {
            self.frame_stack
                .active_brace()
                .and_then(|frame| frame.header.as_deref())
                .is_some_and(|header| {
                    starts_header_word(header, "if")
                        || starts_header_word(header, "for")
                        || starts_header_word(header, "while")
                        || starts_header_word(header, "do")
                        || header.starts_with("else")
                })
        };
        let case_unindent =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if line.trim() == "}"
            && self.options.brace_style == BraceStyle::Horstmann
            && case_unindent > 0
            && let Some((open_spaces, _, _)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
        {
            layout.exact_indent_spaces = Some(open_spaces + case_unindent);
        }
        if layout.line_kind == LineKind::Normal
            && case_unindent > 0
            && line_in_case_control_block
            && self.stack_state.paren_depth == 0
            && !line.trim_start().starts_with("else")
            && !line_is_control_body_header(line.trim_start())
            && (line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
                || line.trim() == "}")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(';')
                && !previous_code.ends_with("};")
                && line_paren_imbalance(previous_code).0 == 0
            {
                let previous_indent = leading_visual_width(previous, self.options.tab_width);
                let target = if line.trim() == "}" {
                    previous_indent.saturating_sub(self.options.indent_width) + case_unindent
                } else {
                    previous_indent + case_unindent
                };
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
        }
        if let Some(spaces) = self.top_level_closing_brace_indent_spaces(line, layout.normal_indent)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.initializer_or_array_closing_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with(')')
            && let Some(open_line) = self.output.iter().rev().take(16).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim() == "("
            })
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(open_line, self.options.tab_width));
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('(')
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if matches!(
                previous_code.trim_start(),
                "if" | "for" | "while" | "switch"
            ) {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some(spaces) = self.else_indent_from_previous_if(line, layout.line_kind) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', ')'])
            && let Some(spaces) = self.after_lambda_condition_indent_spaces()
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with([
                '#', '{', '}', '/', '<', '>', '|', '&', '+', '-', '*', '%', '=', '!', '?', ':',
                ',', '.', '~',
            ])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if preprocessor_directive(previous_code.trim_start()) == Some("endif")
                && let Some(branch_line) = self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
            {
                let branch_code =
                    branch_line[..trailing_comment_split_limit(branch_line)].trim_end();
                let branch_trimmed = branch_code.trim_start();
                if branch_code.ends_with(';')
                    && !branch_code.ends_with("};")
                    && (starts_header_word(branch_trimmed, "if")
                        || starts_header_word(branch_trimmed, "for")
                        || starts_header_word(branch_trimmed, "while")
                        || starts_header_word(branch_trimmed, "else"))
                {
                    let spaces = leading_visual_width(branch_line, self.options.tab_width);
                    if layout
                        .exact_indent_spaces
                        .unwrap_or(layout.indent * self.options.indent_width)
                        > spaces
                    {
                        layout.exact_indent_spaces = Some(spaces);
                    }
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && line_opens_lambda_block(line)
            && let Some(previous) = self.output.last_non_empty_line()
            && is_comment_line(previous.trim_start())
            && let Some(before_comment) = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty() && !is_comment_line(line.trim_start()))
        {
            let code = before_comment[..trailing_comment_split_limit(before_comment)].trim_end();
            if code.ends_with(',') && unmatched_open_paren_column(code).is_some() {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(before_comment, self.options.tab_width));
            }
        }
        if !self.options.indent_after_parens
            && let Some(spaces) = self.nested_call_argument_over_max_output_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && starts_string_literal_token(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',')
                && unmatched_open_paren_column(previous_code).is_none()
                && has_unmatched_open_brace(previous_code)
                && let Some(open) = previous_code.rfind('{')
                && previous_code[..open]
                    .chars()
                    .last()
                    .is_none_or(|ch| !is_identifier_continue(ch))
                && previous_code[open + 1..].starts_with(char::is_whitespace)
            {
                layout.exact_indent_spaces = Some(
                    visual_width_from(&previous_code[..open + 1], 0, self.options.tab_width) + 1,
                );
            }
        }
        if let Some(spaces) = self.constructor_initializer_header_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_closing_paren_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',')
                && !self.in_initializer_brace()
                && !self.output_has_open_initializer_brace()
                && !self.in_aggregate_declaration_brace()
                && has_unmatched_open_brace(previous_code)
                && let Some(open) = previous_code.rfind('{')
                && previous_code[open + 1..].starts_with(char::is_whitespace)
            {
                layout.exact_indent_spaces = Some(
                    visual_width_from(&previous_code[..open + 1], 0, self.options.tab_width) + 1,
                );
            }
            if let Some(spaces) = self.braceless_ternary_comma_sibling_indent_spaces(
                previous_code,
                layout.exact_indent_spaces,
            ) {
                layout.exact_indent_spaces = Some(spaces);
            }
            if previous_code.ends_with(',')
                && previous_code.contains('[')
                && !line.trim_start().starts_with('[')
                && let Some(spaces) = objc_message_following_keyword_column(previous_code)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) =
                casted_assignment_value_column(previous_code, self.options.tab_width)
            {
                layout.exact_indent_spaces = Some(
                    spaces
                        + self.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            }
            if let Some(spaces) =
                self.active_split_else_comma_argument_indent_spaces(line, layout.line_kind)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
            if previous.len() != previous_code.len()
                && previous_code.ends_with(',')
                && starts_string_literal_token(line.trim_start())
                && !line.trim_start().starts_with(['#', '{', '}'])
                && let Some(open) = previous_code.rfind('{')
            {
                layout.exact_indent_spaces = Some(
                    visual_width_from(&previous_code[..open + 1], 0, self.options.tab_width) + 1,
                );
            }
            if let Some(spaces) =
                self.nested_ternary_colon_sibling_indent_spaces(line.trim_start(), previous)
            {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) = self.comment_separated_leading_operator_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with(");")
            && !self.options.indent_cases
            && self
                .stack_state
                .brace_header_stack
                .iter()
                .any(|header| header.as_deref() == Some("case"))
            && self.output.iter().rev().take(16).any(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.contains(" new ") || code.trim_start().starts_with("new ")
            })
            && let Some(spaces) = self.split_call_closing_paren_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.asm_colon_line_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && matches!(line.trim_start().chars().next(), Some('.' | '['))
            && self.innermost_brace_is_compound_literal()
            && let Some(spaces) = self.active_initializer_brace_indent_spaces(line, false)
        {
            layout.exact_indent_spaces =
                Some(spaces.max(self.state.indent() * self.options.indent_width));
        }
        contextual
    }

    pub(in super::super) fn apply_final_sibling_and_directive_contextual_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let layout = &mut contextual.layout;
        if layout.exact_indent_spaces.is_none()
            && let Some(spaces) =
                self.preprocessor_directive_closing_indent_spaces(line, layout.indent)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        contextual
    }

    pub(in super::super) fn apply_preprocessor_and_split_else_recovery_layout(
        &mut self,
        line: &str,
        mut contextual: ContextualLineLayout,
    ) -> ContextualLineLayout {
        let split_else_state_active = contextual.split_else_state_active;
        let layout = &mut contextual.layout;
        let recent_split_else_context =
            self.recent_split_else_chain_context(split_else_state_active);
        if let Some(spaces) = self.recent_split_else_command_closing_indent_spaces(
            line,
            layout.indent,
            layout.exact_indent_spaces,
            recent_split_else_context.chain_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.comment_terminated_logical_chain_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.exact_indent_spaces.is_none()
            && layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '}', ':'])
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let natural = layout.indent * self.options.indent_width;
            if previous_code == "{" && previous_indent + self.options.indent_width > natural {
                layout.exact_indent_spaces = Some(previous_indent + self.options.indent_width);
            }
        }
        if let Some(spaces) =
            self.opening_conditional_directive_body_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.preprocessor_interrupted_header_body_indent_spaces(
            line,
            layout.line_kind,
            layout.indent,
            layout.exact_indent_spaces,
            recent_split_else_context.interrupted_header_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.recent_split_else_if_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
            recent_split_else_context.chain_active(),
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if self.options.brace_style == BraceStyle::Allman
            && layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains('[') && !previous_code.contains(']') {
                let previous_indent = leading_visual_width(previous, self.options.tab_width);
                let target = if previous_code.trim_start().starts_with("}[") {
                    self.options.indent_width
                } else {
                    previous_indent + 1
                };
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
        }
        if layout.exact_indent_spaces.is_none()
            && self.options.brace_style == BraceStyle::Allman
            && layout.line_kind == LineKind::Normal
            && self.state.indent() == 0
            && layout.indent > layout.normal_indent
            && self.token_input.token_source_line_indent == 0
            && self
                .output
                .last()
                .is_some_and(|line| line.trim().is_empty())
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            layout.exact_indent_spaces = Some(0);
        }
        if let Some(spaces) =
            self.case_label_block_indent_override(line, layout.indent, layout.exact_indent_spaces)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.active_split_else_header_continuation_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) =
            self.active_split_else_call_argument_fallback_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.active_split_else_open_header_brace_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        contextual
    }
}
