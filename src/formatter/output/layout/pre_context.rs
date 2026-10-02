use crate::formatter::continuation::call_arguments::{
    closing_braced_call_argument_indent_spaces, line_opens_attachable_lambda_block,
    plain_call_opener_indent_for_closing_line,
};
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{self, CommentKind, Token};
use crate::formatter::output::model::{LineLayout, LineReplayLayout};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    has_unmatched_open_brace, line_paren_imbalance, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::formatter::tokens::literals::starts_string_literal_token;
use crate::formatter::tokens::operators::starts_with_chain_operator;
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

impl FormatEngine<'_> {
    pub(crate) fn apply_spacing_new_call_and_stream_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        self.insert_member_spacing_before_line(line);
        if self.take_block_spacing_blank(line) {
            self.push_empty_line();
        }
        if line.trim_start().starts_with('|')
            && !line.trim_start().starts_with("||")
            && let Some(previous) = self.output.last_line_outside_comment()
            && previous.contains("CHECK(")
        {
            let base = layout.indent * self.options.indent_width;
            if visual_width_from(previous, 0, self.options.tab_width).saturating_sub(base)
                > self.options.max_continuation_indent
            {
                layout.exact_indent_spaces = Some(base + self.options.indent_width * 2);
            }
        }
        if line.trim() == "("
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains(" : ")
                && !previous_code.trim_start().starts_with(':')
                && previous_code
                    .chars()
                    .last()
                    .is_some_and(is_identifier_continue)
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if let Some(spaces) = self.split_or_empty_new_call_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.over_max_new_call_default_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.template_continuation_line_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_preprocessor_branch_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_branch_opening_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_comment_row_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.exact_indent_spaces.is_some()
            && !self.in_aggregate_declaration_brace()
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let split = trailing_comment_split_limit(previous);
            let previous_code = previous[..split].trim_end();
            if (split < previous.len()
                || lexer::tokenize(previous_code)
                    .iter()
                    .any(|token| matches!(token, Token::Comment(CommentKind::Block, _))))
                && previous_code.ends_with(';')
                && unmatched_open_paren_column(previous_code).is_none()
            {
                layout.indent = layout.normal_indent;
                layout.exact_indent_spaces = None;
            }
        }
        if let Some(spaces) = self.parenthesized_after_trailing_stream_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.stream_chain_frame_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.comment_separated_stream_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) = self.stream_after_closed_or_inline_row_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        layout
    }

    pub(crate) fn apply_lambda_return_call_and_stream_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut layout: LineLayout,
    ) -> LineLayout {
        if !replay.closed_split_lambda_parameter_list
            && layout.line_kind != LineKind::SwitchLabel
            && let Some(spaces) = self.contextual_line_indent_spaces(
                line,
                layout.indent,
                layout.normal_indent,
                layout.exact_indent_spaces,
            )
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_trailing_return_arrow_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.exact_indent_spaces.is_none()
            && !self.options.indent_after_parens
            && let Some(spaces) = self.nested_call_argument_over_max_output_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.completed_ternary_call_sibling_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        // A comment's body line continues no assignment.
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('*')
            && self.output.pending_tokens().is_some()
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with('=')
                && !previous_code.ends_with("==")
                && !previous_code.ends_with("!=")
                && !previous_code.ends_with("<=")
                && !previous_code.ends_with(">=")
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('*')
            && let Some(previous) = self.output.last_line_outside_comment()
            && (previous.trim_end().ends_with(';') || previous.trim() == "*/")
            && unmatched_open_paren_column(previous).is_none()
        {
            layout.exact_indent_spaces = None;
        }
        if let Some(spaces) = self.take_split_else_comment_body_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_body_closing_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_else_reduced_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with(");")
            && let Some(spaces) = plain_call_opener_indent_for_closing_line(
                self.output.as_slice(),
                self.options.tab_width,
            )
        {
            layout.exact_indent_spaces = Some(
                spaces
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if line_opens_attachable_lambda_block(self.options, line) {
            if let Some(spaces) = self.lambda_call_argument_after_split_indent_spaces(line) {
                layout.exact_indent_spaces = Some(spaces);
            } else if !line.trim_start().starts_with("/*") {
                layout.indent = layout.normal_indent;
                layout.exact_indent_spaces = None;
            }
        }
        if let Some(spaces) = self.argument_after_lambda_call_argument_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.call_argument_sibling_frame_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) =
            self.outer_call_argument_after_closed_inner_call_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.embedded_capture_lambda_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = closing_braced_call_argument_indent_spaces(
            line,
            self.output.as_slice(),
            self.options.tab_width,
        ) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.line_start_stream_adjacent_string_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = layout.exact_indent_spaces.as_mut()
            && self.enclosing_macro_call_output_context()
            && !starts_string_literal_token(line.trim_start())
            && !line.trim_start().starts_with([')', '}'])
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_trimmed = previous.trim_start();
            let current_trimmed = line.trim_start();
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let uses_outer_call_column = previous_code.ends_with(',')
                && previous_code.contains(").")
                && self
                    .outer_call_indent_after_closed_previous_line()
                    .is_some_and(|target| *spaces == target);
            let closed_inner_logical_tail = starts_with_chain_operator(current_trimmed)
                && starts_with_chain_operator(previous_trimmed)
                && previous_trimmed.ends_with(')')
                && line_paren_imbalance(previous_code).0 > 0;
            let previous_closes_inner_call = line_paren_imbalance(previous_code).0 > 0;
            let wanted = leading_visual_width(previous, self.options.tab_width)
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if !uses_outer_call_column
                && !closed_inner_logical_tail
                && !previous_closes_inner_call
                && *spaces < wanted
            {
                *spaces = wanted;
            }
        }
        layout
    }
}

impl FormatEngine<'_> {
    pub(crate) fn apply_comment_brace_and_ternary_operand_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if line.trim_start().starts_with("/*")
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with("#if") {
                for (index, candidate) in self.output.iter().enumerate().rev() {
                    let candidate_code =
                        candidate[..trailing_comment_split_limit(candidate)].trim_end();
                    if !(candidate_code.trim_start().starts_with('#')
                        && candidate_code.ends_with(']'))
                    {
                        continue;
                    }
                    if let Some(body) = self.output[index + 1..]
                        .iter()
                        .find(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
                    {
                        layout.exact_indent_spaces =
                            Some(leading_visual_width(body, self.options.tab_width));
                    }
                    break;
                }
            }
        }
        if line.trim() == "{"
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim() == "{" {
                // Output lines already carry the case-block unindent.
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + self.case_unindent_spaces(),
                );
            }
        }
        if line.trim() == "{"
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let mut paren_depth = 0isize;
            for ch in previous_code.chars() {
                match ch {
                    '(' => paren_depth += 1,
                    ')' => paren_depth -= 1,
                    _ => {}
                }
            }
            if paren_depth < 0
                && !self.pending_line_starts_initializer_element()
                && !previous_code.ends_with(')')
                && previous_code.chars().any(is_identifier_start)
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if line.trim() == "{"
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if trailing_word(previous_code) == "return"
                && !previous_code.trim_start().starts_with("return")
                && let Some(open) = self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| line.trim() == "{")
            {
                let spaces = leading_visual_width(open, self.options.tab_width);
                layout.exact_indent_spaces = Some(spaces);
                self.update_current_brace_indent_columns(
                    spaces + self.options.indent_width,
                    spaces,
                );
            }
        }
        if let Some(spaces) = self.operand_after_question_row_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        } else if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if previous_trimmed
                .chars()
                .next()
                .is_some_and(is_identifier_start)
                && (0..self.output.len()).rev().take(5).any(|index| {
                    let code = self.output.code(index);
                    self.output.code_trimmed(index).starts_with('#') && code.ends_with(']')
                })
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        layout
    }

    pub(crate) fn apply_late_call_and_operator_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
            ])
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if line.contains("#endif")
                && previous_trimmed
                    .chars()
                    .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
                && (0..self.output.len()).rev().take(4).any(|index| {
                    let code = self.output.code(index);
                    code.contains("#endif") && !self.output.code_trimmed(index).starts_with('#')
                })
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width / 2,
                );
            } else if has_hash_outside_literals(previous_code)
                && !previous_code.trim_start().starts_with('#')
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if previous_code.trim_start().starts_with('#')
                && previous_code.ends_with(']')
                && let Some(before_preprocessor) =
                    self.output.scoped().iter().rev().skip(1).find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !trimmed.starts_with('#')
                    })
            {
                layout.exact_indent_spaces = Some(leading_visual_width(
                    before_preprocessor,
                    self.options.tab_width,
                ));
            }
        }
        if let Some(spaces) =
            self.leading_operator_after_ternary_colon_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && (0..self.output.len()).rev().take(12).any(|index| {
                let code = self.output.code(index);
                code.contains("#if")
            })
            && !(0..self.output.len()).rev().take(4).any(|index| {
                let code = self.output.code(index);
                let trimmed = self.output.code_trimmed(index);
                !trimmed.is_empty()
                    && (code.ends_with('{')
                        || code.ends_with(';')
                        || code.ends_with('}')
                        || (trimmed.starts_with('#')
                            && !trimmed.starts_with("#if")
                            && !trimmed.starts_with("#elif")))
            })
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            let current_trimmed = line.trim_start();
            if current_trimmed.starts_with([';', '!', ','])
                && previous_trimmed.starts_with([
                    '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '?', ':', '.', '~',
                ])
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width / 2,
                );
            } else if !previous_trimmed.starts_with(['#', '{', '}'])
                && !self.pending_line_starts_call_argument()
                && !self
                    .output
                    .last_non_empty_index()
                    .is_some_and(|index| self.output.is_directive_line(index))
                && (0..self.output.len()).rev().take(4).any(|index| {
                    self.output.code_trimmed(index).starts_with([';', '!', ','])
                        && !self.output.is_directive_line(index)
                })
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if layout.line_kind == LineKind::Normal && !line.trim_start().starts_with(['#', '{', '}']) {
            for index in (0..self.output.len()).rev().take(4) {
                let previous = &self.output[index];
                let previous_code = self.output.code(index);
                let previous_trimmed = self.output.code_trimmed(index);
                if previous_code.contains("#define") {
                    break;
                }
                if previous_trimmed == "enum" {
                    layout.exact_indent_spaces = Some(
                        leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width,
                    );
                    break;
                }
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && (0..self.output.len())
                .rev()
                .take(4)
                .any(|index| self.output.code_trimmed(index).starts_with("else,"))
        {
            layout.exact_indent_spaces = Some(0);
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_line_outside_comment()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .trim()
                == "("
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with(';')
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with("#if") && previous_code.contains("->") {
                layout.exact_indent_spaces =
                    Some(self.layout.indentation.indent() * self.options.indent_width);
            }
        }
        if layout.line_kind == LineKind::Normal
            && ["for ", "while ", "switch "]
                .iter()
                .any(|header| line.trim_start().starts_with(header))
            && !line.contains('(')
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_spaces = leading_visual_width(previous, self.options.tab_width);
            if previous_spaces
                > layout
                    .exact_indent_spaces
                    .unwrap_or(layout.indent * self.options.indent_width)
            {
                layout.exact_indent_spaces = Some(previous_spaces);
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.output.last_line_outside_comment().is_some_and(|line| {
                line[..trailing_comment_split_limit(line)].trim_end().trim() == "catch"
            })
        {
            layout.exact_indent_spaces = self
                .output
                .last_line_outside_comment()
                .map(|line| leading_visual_width(line, self.options.tab_width));
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with(']')
            && self.output.last_line_outside_comment().is_some_and(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with("#define") && !code.ends_with('\\')
            })
        {
            layout.exact_indent_spaces = Some(0);
        }
        if layout.line_kind == LineKind::Normal
            && let Some(spaces) = self.preprocessor_branch_initializer_member_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && let Some(base_spaces) = self.embedded_preprocessor_branch_body_base_spaces()
        {
            layout.exact_indent_spaces = Some(base_spaces);
        }
        if (self.current_inline_array_column().is_some() || self.in_initializer_brace())
            && !line.trim_start().starts_with(['.', '{', '}'])
            && !self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                    has_hash_outside_literals(code) && !code.trim_start().starts_with('#')
                })
            && self.stream_chain_frame_indent_spaces(line).is_none()
            && let Some(spaces) = layout.exact_indent_spaces
        {
            let base = self.continuation_base_indent() * self.options.indent_width;
            let source_indent = self
                .token_input
                .token_source_line_indent
                .max(leading_visual_width(line, self.options.tab_width));
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
            if spaces.saturating_sub(base) > self.options.max_continuation_indent {
                layout.exact_indent_spaces =
                    stream_indent.or((source_indent > 0).then_some(source_indent));
            }
        }
        layout
    }
}
