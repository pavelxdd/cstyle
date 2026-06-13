use super::super::super::FormatEngine;
use super::super::super::brace_classification::line_opens_lambda_block;
use super::super::super::columns::{leading_visual_width, visual_width_from};
use super::super::super::frame::BracketFrame;
use super::super::super::headers::{
    is_braceless_header_line, line_is_control_body_header, starts_header_word,
};
use super::super::super::indentation::LineKind;
use super::super::super::labels;

use super::super::super::line_scan::{find_outside_quotes, is_comment_line};
use super::super::super::line_scan::{
    has_unmatched_open_brace, trailing_comment_split_limit, unmatched_open_paren_column,
};
use super::super::super::literals::starts_string_literal_token;
use super::super::super::operator_chains;

use super::super::super::operators::{
    find_assignment_operator, head_ends_binary_operator, is_prefix_increment_statement,
    starts_prefix_increment,
};
use super::super::super::preprocessor::{
    is_conditional_preprocessor, is_known_preprocessor_directive, preprocessor_directive,
};
use super::super::super::state::{ContinuationIndent, FormatterBraceType};
use super::super::super::switch_cases::case_label_with_trailing_comment;
use super::super::super::template_declarations::{
    template_continuation_indent_spaces, template_declaration_line_complete,
};
use super::super::model::{AlignedLineLayout, LineLayout, LineReplayLayout, LineRoute};
use crate::config::BraceStyle;
use crate::source::lex::{is_identifier_start, trailing_word};

impl FormatEngine<'_> {
    pub(in super::super) fn initial_line_layout(
        &mut self,
        line: &str,
        observed_line_kind: LineKind,
        replay: &LineReplayLayout,
    ) -> LineLayout {
        let line_kind = labels::reconcile_line_kind(
            observed_line_kind,
            line,
            &self.options.access_labels,
            labels::ClassificationContext {
                enclosing_brace: self.stack_state.brace_type_stack.last().copied(),
                in_initializer: self.in_initializer_brace()
                    || self.current_inline_array_column().is_some(),
                in_ternary: self.line_state.ternary_colon
                    || self
                        .return_ternary_colon_after_multiline_template_declaration_indent_spaces(
                            line,
                        )
                        .is_some(),
                previous_line: self.output.last_non_empty_line().map(String::as_str),
            },
        );
        self.run_in_state.adjuster_observed_line_count += 1;
        self.clear_case_body_indent_if_past_switch();
        let split_else_line_start = self.prepare_split_else_line_start(line, line_kind);
        let split_else_extra = split_else_line_start.extra_levels();
        let else_if_break_extra =
            if line_kind == LineKind::Normal && !self.options.no_indent_if_after_else {
                self.else_if_break_depths.len()
            } else {
                0
            };
        let normal_indent = self.state.line_indent(line_kind, self.options)
            + self.case_body_indent_extra(line_kind)
            + self.case_preproc_body_indent_extra(line_kind, line)
            + split_else_extra
            + else_if_break_extra
            + self.member_init_continuation_extra(line_kind, line);
        let else_while_brace = line == "{"
            && self
                .output
                .last()
                .is_some_and(|previous| previous.trim_start().starts_with("else while"));
        let forced_brace_indent = if else_while_brace {
            let previous = self.output.last().map(String::as_str).unwrap_or("");
            Some(
                (leading_visual_width(previous, self.options.tab_width)
                    / self.options.indent_width)
                    + usize::from(self.options.indent_blocks)
                    + 1,
            )
        } else {
            (line == "{")
                .then(|| {
                    self.continuation_indent
                        .next_line_indent
                        .take()
                        .map(|level| split_else_line_start.adjust_brace_level(level))
                })
                .flatten()
        };
        let class_label_indent = labels::class_scope_indent(
            line_kind,
            line,
            self.stack_state.brace_type_stack.last().copied(),
            self.state.indent(),
            self.options,
        );
        let class_scope_label = class_label_indent.is_some();
        if let Some(level) = forced_brace_indent {
            self.continuation_indent.next_line_indent_spaces.take();
            LineLayout {
                line_kind,
                normal_indent,
                indent: level,
                exact_indent_spaces: None,
                class_scope_label,
                else_while_brace,
            }
        } else if let Some(class_label_indent) = class_label_indent {
            self.continuation_indent.next_line_indent.take();
            self.continuation_indent.next_line_indent_spaces.take();
            match class_label_indent {
                ContinuationIndent::Level(indent) => LineLayout {
                    line_kind,
                    normal_indent,
                    indent,
                    exact_indent_spaces: None,
                    class_scope_label,
                    else_while_brace,
                },
                ContinuationIndent::Spaces(spaces) => LineLayout {
                    line_kind,
                    normal_indent,
                    indent: normal_indent,
                    exact_indent_spaces: Some(spaces),
                    class_scope_label,
                    else_while_brace,
                },
            }
        } else if line_kind == LineKind::Normal {
            let pending_level = self.continuation_indent.next_line_indent.take();
            let pending_spaces = self.continuation_indent.next_line_indent_spaces.take();
            let delimiter_owns_snapshot = replay.closed_lambda_parameter_list
                && replay.closed_delimiter_continuation_indent.is_some()
                || self
                    .frame_stack
                    .active_delimiter()
                    .is_some_and(|frame| frame.opener_output_line < self.output.len());
            let snapshot_level = match replay.input_continuation_indent {
                Some(ContinuationIndent::Level(level))
                    if delimiter_owns_snapshot
                        && !line.trim_start().starts_with(['{', '}', ')']) =>
                {
                    Some(level)
                }
                _ => None,
            };
            let next_line_indent = pending_level.or(snapshot_level);
            let snapshot_spaces = match replay.input_continuation_indent {
                Some(ContinuationIndent::Spaces(spaces))
                    if delimiter_owns_snapshot
                        && !line.trim_start().starts_with(['{', '}', ')']) =>
                {
                    Some(spaces)
                }
                _ => None,
            };
            let level = next_line_indent
                .map(|level| {
                    let included_base_indent = if line.trim_start().starts_with("else") {
                        self.state.indent()
                    } else {
                        self.state.indent() + 1
                    };
                    split_else_line_start.adjust_pending_level(level, included_base_indent)
                        + else_if_break_extra
                })
                .unwrap_or(normal_indent);
            let mut spaces = pending_spaces.or(snapshot_spaces);
            if spaces.is_some()
                && let Some(previous) = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
            {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                if self.in_enum_declaration_brace()
                    && previous_code.ends_with(',')
                    && previous.len() != previous_code.len()
                {
                    spaces = None;
                } else if next_line_indent.is_some()
                    && is_braceless_header_line(previous_code.trim_start())
                    && !line.trim_start().starts_with(['#', '{', '}'])
                    && !operator_chains::starts_operator_chain_continuation(line)
                {
                    spaces = None;
                } else if previous_code.ends_with("},")
                    && let Some(open) = unmatched_open_paren_column(previous_code)
                {
                    spaces = Some(open + 1);
                } else if self.state.indent() == 0
                    && self.token_input.token_source_line_indent == 0
                    && previous_code.trim_start().starts_with('#')
                    && line
                        .trim_start()
                        .chars()
                        .next()
                        .is_some_and(is_identifier_start)
                {
                    spaces = None;
                }
            }
            let opens_lambda_block =
                line.trim_end().ends_with('{') && line_opens_lambda_block(line);
            if spaces.is_some() && opens_lambda_block {
                spaces = None;
            }
            let mut level = level.max(self.pending_braceless_block_bias.unwrap_or(0));
            if opens_lambda_block {
                level = normal_indent;
            }
            if line.trim_start().starts_with("else")
                && let Some(else_level) = self.braceless_else_output_level()
            {
                level = else_level;
                spaces = None;
            }
            if self.options.no_indent_if_after_else
                && starts_header_word(line.trim_start(), "if")
                && let Some(previous) = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                && matches!(previous.trim(), "else" | "} else")
            {
                let previous_indent = leading_visual_width(previous, self.options.tab_width);
                if previous_indent.is_multiple_of(self.options.indent_width) {
                    level = previous_indent / self.options.indent_width;
                    spaces = None;
                } else {
                    spaces = Some(previous_indent);
                }
            }
            LineLayout {
                line_kind,
                normal_indent,
                indent: level,
                exact_indent_spaces: spaces,
                class_scope_label,
                else_while_brace,
            }
        } else {
            if !(line_kind == LineKind::Label && self.pending_braceless_block_bias.is_some()) {
                self.continuation_indent.next_line_indent.take();
            }
            self.continuation_indent.next_line_indent_spaces.take();
            LineLayout {
                line_kind,
                normal_indent,
                indent: normal_indent,
                exact_indent_spaces: None,
                class_scope_label,
                else_while_brace,
            }
        }
    }

    pub(in super::super) fn apply_initial_syntax_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::Normal
            && let Some(previous) = self.output.last_non_empty_line()
        {
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
            if starts_string_literal_token(line.trim_start())
                && previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                let target = open + 1 + self.adjusted_line_indent_delta(previous);
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
        }
        if starts_string_literal_token(line.trim_start())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',')
                && let Some(open) = unmatched_open_paren_column(previous_code)
            {
                let target = open + 1 + self.adjusted_line_indent_delta(previous);
                if layout.exact_indent_spaces.unwrap_or(0) < target {
                    layout.exact_indent_spaces = Some(target);
                }
            }
        }
        layout.exact_indent_spaces = self.initial_switch_case_indent_spaces(
            line,
            layout.line_kind,
            layout.exact_indent_spaces,
        );
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('#')
            && line.contains(['{', '}'])
            && preprocessor_directive(line)
                .is_some_and(|directive| !is_known_preprocessor_directive(directive))
        {
            layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
        }
        if let Some(spaces) = self.continuation_adjacent_opening_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) = self.continuation_adjacent_closing_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::Gnu
            && line.trim_start().starts_with('{')
            && line.trim() != "{"
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains("#define") && !previous_code.trim_start().starts_with('#') {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if let Some(level) =
            self.else_structural_indent_after_braced_statement_level(line, layout.line_kind)
        {
            layout.indent = level;
            layout.exact_indent_spaces = None;
        }
        layout
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_initial_operator_and_header_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
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
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with("else")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if self.options.brace_style == BraceStyle::Whitesmith && previous_code.ends_with("::") {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            } else if previous_code.trim_start().starts_with([
                '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
            ]) && !previous_code.ends_with(';')
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + visual_width_from(previous_code.trim_start(), 0, self.options.tab_width)
                        + 2,
                );
            }
        }
        if layout.line_kind == LineKind::Normal
            && self.options.brace_style == BraceStyle::Whitesmith
            && !line.trim_start().starts_with("else")
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_full = previous.trim_end();
            if previous_full.ends_with("::")
                && let Some(open) = unmatched_open_paren_column(previous_full)
            {
                layout.exact_indent_spaces = Some(open + 1);
            } else if previous_code.ends_with("::") {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some(spaces) = self.else_indent_from_previous_if(line, layout.line_kind) {
            layout.indent = spaces / self.options.indent_width;
            layout.exact_indent_spaces = Some(spaces);
            while let Some((base, delta)) = self.state.last_braceless_block() {
                if self.state.indent() <= layout.indent + 1 || self.state.indent() != base + delta {
                    break;
                }
                self.state.exit_braceless_block();
            }
        }
        if let Some(spaces) = self.detached_else_nested_header_indent_spaces(line, layout.line_kind)
        {
            layout.indent = spaces / self.options.indent_width;
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.multiline_else_header_continuation_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(else_layout) = self.plain_else_body_layout(line, layout.line_kind) {
            if let Some(level) = else_layout.indent_level {
                layout.indent = level;
            }
            layout.exact_indent_spaces = Some(else_layout.indent_spaces);
        }
        if let Some(spaces) =
            self.active_split_else_multiline_header_body_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.multiline_control_header_body_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.else_while_body_indent_spaces(line, layout.line_kind) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.isolated_opening_brace_body_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line
            .trim_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && self.output.may_have_hash()
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains("#endif") && !previous_code.trim_start().starts_with('#') {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if self
                .output
                .last()
                .is_some_and(|line| line.trim().is_empty())
                && previous_code.contains("#if")
                && !previous_code.trim_start().starts_with('#')
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            } else if self
                .output
                .last()
                .is_some_and(|line| line.trim().is_empty())
                && self.next_line.leads_with_open_brace
                && previous_code.ends_with('>')
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some(spaces) = self.first_ordinary_ternary_arm_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        layout
    }

    pub(in super::super) fn apply_separated_header_and_comment_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '#'])
            && (self.output.may_have_else()
                || self.output.may_have_hash()
                || self.output.may_have_comment())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_trimmed = previous.trim_start();
            if previous_trimmed.starts_with("} else if") && previous_trimmed.ends_with('{') {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                );
            } else if previous_trimmed.ends_with("} else") {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + self.line_adjuster.next_line_case_unindent_depth()
                            * self.options.indent_width,
                );
            } else if let Some(spaces) =
                self.separated_else_header_body_indent_floor(line, layout.line_kind)
                && layout.exact_indent_spaces.unwrap_or(0) < spaces
            {
                layout.exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) =
            self.block_comment_separated_header_body_indent_spaces(line, layout.line_kind)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '#'])
            && self.output.may_have_comment()
        {
            let mut comment_indent = None;
            for previous in self
                .output
                .iter()
                .rev()
                .filter(|line| !line.trim().is_empty())
            {
                if matches!(
                    previous.trim_start(),
                    text if text.starts_with("//")
                        || text.starts_with("/*")
                        || text.starts_with("*/")
                        || text == "*"
                        || text.starts_with("* ")
                        || text.starts_with("*\t")
                        || text.starts_with("**")
                ) {
                    let trimmed = previous.trim_start();
                    if !trimmed.starts_with("//") || comment_indent.is_none() {
                        comment_indent =
                            Some(leading_visual_width(previous, self.options.tab_width));
                    }
                    continue;
                }
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                let previous_code_trimmed = previous_code.trim_start();
                if previous_code.ends_with('{')
                    && (previous_code_trimmed.starts_with("case ")
                        || previous_code_trimmed.starts_with("default:"))
                    && let Some(spaces) = comment_indent
                {
                    layout.exact_indent_spaces = Some(
                        spaces
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                } else if previous_code == "{"
                    && let Some(spaces) = comment_indent
                    && self
                        .output
                        .iter()
                        .rev()
                        .skip_while(|line| line.as_str() != previous.as_str())
                        .skip(1)
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|before| {
                            let code = before[..trailing_comment_split_limit(before)].trim_end();
                            let trimmed = code.trim_start();
                            trimmed.starts_with("case ") || trimmed.starts_with("default:")
                        })
                {
                    layout.exact_indent_spaces = Some(
                        spaces
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
                break;
            }
        }
        if let Some(spaces) = self.else_body_after_comments_indent_spaces(line, layout.line_kind) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '}', '#'])
            && self.output.may_have_else()
            && self.output.may_have_comment()
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
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
                && layout.exact_indent_spaces.unwrap_or(0) < previous_indent
            {
                layout.exact_indent_spaces = Some(previous_indent);
            }
        }
        layout
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_label_and_conditional_context_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '#'])
            && (self.output.may_have_hash()
                || self.output.may_have_label_open()
                || line.trim_start().starts_with(['}', ':', '?'])
                || self.output.may_have_else())
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if let Some(spaces) = self.candidate_label_body_indent_spaces(previous) {
                layout.exact_indent_spaces = Some(spaces);
            } else if previous_code.trim() == "?" {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if let Some(spaces) =
                self.none_style_conditional_else_indent_spaces(line, layout.line_kind)
            {
                layout.exact_indent_spaces = Some(spaces);
            } else if line.trim_start().starts_with(':')
                && line.contains('#')
                && previous_code.trim() == "}"
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if previous_code.trim_start() == "#else"
                && let Some(header_line) = self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                && trailing_word(header_line.trim_end()) == "do"
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(header_line, self.options.tab_width)
                        + self.options.indent_width * 2,
                );
            } else if (line.trim() == "?"
                && self.output.iter().rev().take(8).any(|line| {
                    let trimmed = line.trim_start();
                    trimmed == "}" || trimmed.starts_with("} ")
                }))
                || (previous_code.trim_start().starts_with("} ")
                    && !previous_code.ends_with('{')
                    && !previous_code.trim_start().starts_with("} while")
                    && !previous_code.trim_start().starts_with("} else")
                    && !previous_code.trim_start().starts_with("} catch")
                    && !previous_code.trim_start().starts_with("} __finally")
                    && !previous_code.trim_start().starts_with("} __except")
                    && !previous_code.trim_start().starts_with("} *"))
            {
                layout.exact_indent_spaces = Some(0);
            } else if previous_code.trim_start().starts_with("}[") {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width / 2,
                );
            } else if line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
                && previous_code.trim_start().starts_with('[')
                && leading_visual_width(previous, self.options.tab_width)
                    <= self.options.indent_width / 2
                && self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| {
                        let trimmed = line.trim_start();
                        trimmed == "}" || trimmed.starts_with("} ")
                    })
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width / 4,
                );
            } else if line
                .trim_start()
                .chars()
                .next()
                .is_some_and(|ch| is_identifier_start(ch) || ch == ',')
                && previous_code
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|ch| is_identifier_start(ch) || ch.is_ascii_digit())
                && !previous_code.ends_with('{')
                && !line_is_control_body_header(previous_code.trim_start())
                && leading_visual_width(previous, self.options.tab_width)
                    <= self.options.indent_width / 2
                && self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| {
                        let trimmed = line.trim_start();
                        trimmed == "}" || trimmed.starts_with("} ")
                    })
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if line.trim_start().starts_with(['>', ']'])
                && (previous_code.trim_start().starts_with("}~")
                    || previous_code.trim_start().starts_with(">]"))
            {
                layout.exact_indent_spaces = Some(0);
            } else if line.trim_start().starts_with('~') && previous_code.contains("||:") {
                layout.exact_indent_spaces = Some(self.options.indent_width);
            }
            if previous_code.trim_start().starts_with("return ")
                && previous_code.contains('#')
                && !previous_code.ends_with(';')
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width) + "return ".len());
            } else if self.token_input.token_source_line_indent > 0
                && previous_code.trim_start().starts_with("#endif")
                && line.contains("#define")
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '#'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_end().ends_with(')')
            && (previous.trim_start().starts_with("while ")
                || previous.trim_start().starts_with("while(")
                || previous.trim_start().starts_with("for ")
                || previous.trim_start().starts_with("for("))
            && self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line.trim() == "else")
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with("[]")
            && let Some(open) = unmatched_open_paren_column(
                previous[..trailing_comment_split_limit(previous)].trim_end(),
            )
        {
            layout.exact_indent_spaces = Some(open + 1);
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with("[]")
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)].trim() == "},"
            && let Some(first_lambda) = self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| line.trim_start().starts_with("[]"))
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(first_lambda, self.options.tab_width));
        }
        layout
    }

    pub(in super::super) fn apply_top_level_and_initializer_prefix_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::Normal
            && line
                .trim_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            && self.stack_state.brace_type_stack.is_empty()
            && let Some(previous) = self.output.last_non_empty_line()
            && previous[..trailing_comment_split_limit(previous)].trim() == "};"
        {
            layout.exact_indent_spaces = Some(0);
        }
        if matches!(line.trim_start(), "@private" | "@public" | "@protected")
            && self
                .output
                .iter()
                .rev()
                .any(|previous| previous.trim_start().starts_with("@interface "))
        {
            layout.exact_indent_spaces = Some(0);
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with("//")
            && let Some(previous) = self.output.last()
            && case_label_with_trailing_comment(previous.trim())
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if let Some(spaces) = self.control_header_line_comment_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        } else if line.trim_start().starts_with("//")
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_trimmed = previous.trim_start();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if previous_trimmed.starts_with("//")
                && previous_indent > layout.normal_indent * self.options.indent_width
            {
                layout.exact_indent_spaces = Some(previous_indent);
            }
        }
        if layout.line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['/', '#', '{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with("//")
        {
            let comment = previous.trim_start().trim_start_matches("//").trim_start();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let before_comment = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .filter(|line| !line.trim().is_empty())
                .collect::<Vec<_>>();
            if comment.contains('(')
                && comment.ends_with('.')
                && before_comment
                    .first()
                    .is_some_and(|line| line.trim_end().ends_with('.'))
            {
                layout.exact_indent_spaces = Some(previous_indent);
            }
            for line in before_comment {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with("//") {
                    if trimmed.contains("{{{") {
                        layout.exact_indent_spaces = Some(previous_indent);
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with('*')
            && let Some(previous) = self.output.last_non_empty_line()
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
            } else if previous_code.ends_with(',') && previous_code.trim_start().starts_with('}') {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if previous_code.ends_with(',')
                && previous_code.contains("= new ")
                && unmatched_open_paren_column(previous_code).is_none()
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            } else if previous_code.trim_start().starts_with('*')
                && previous_code.ends_with(',')
                && find_assignment_operator(previous_code).is_some()
                && !previous_code.contains("= new ")
                && unmatched_open_paren_column(previous_code).is_none()
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width) + 1);
            }
        }
        if layout.line_kind == LineKind::Normal
            && line.trim_start().starts_with(',')
            && let Some(previous) = self.output.last_non_empty_line()
            && matches!(previous.trim_start().chars().next(), Some(':' | ','))
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if layout.line_kind == LineKind::Normal
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_trimmed = previous.trim_start();
            if previous_trimmed.starts_with(':')
                && previous_trimmed.ends_with(',')
                && !line.trim_start().starts_with([',', '{', '}'])
                && unmatched_open_paren_column(previous_trimmed).is_none()
                && !has_unmatched_open_brace(previous_trimmed)
                && self.current_inline_array_column().is_none()
            {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width) + 2);
            }
        }
        layout
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_constructor_and_call_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if let Some(spaces) = self.constructor_initializer_header_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if self.stream_chain_frame_indent_spaces(line).is_none()
            && let Some(spaces) = self.initializer_member_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.designated_initializer_source_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_continuation_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.exact_indent_spaces.is_some()
            && !self.options.indent_after_parens
            && let Some(spaces) = self.array_bound_operator_output_indent_spaces()
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !self.options.indent_after_parens
            && !line.trim_start().starts_with('{')
            && let Some(spaces) = self.macro_call_continuation_output_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !self.options.indent_after_parens
            && let Some(spaces) = self.split_call_opening_paren_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !self.options.indent_after_parens
            && let Some(spaces) = self.split_call_closing_paren_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if layout.exact_indent_spaces.is_some()
            && !self.options.indent_after_parens
            && !line.trim_start().starts_with('{')
            && self.current_inline_array_column().is_none()
            && !self.in_initializer_brace()
            && !self.output_has_open_initializer_brace()
            && !self.enclosing_macro_call_output_context()
            && !self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| head_ends_binary_operator(previous.trim_end()))
            && let Some(spaces) = self.function_parameter_continuation_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !self.options.indent_after_parens
            && let Some(spaces) = self.nested_call_argument_over_max_output_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_argument_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_preprocessor_branch_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        let macro_call_previous_ends_comma = self.enclosing_macro_call_output_context()
            && self.output.last_non_empty_line().is_some_and(|previous| {
                previous[..trailing_comment_split_limit(previous)]
                    .trim_end()
                    .ends_with(',')
            });
        if !macro_call_previous_ends_comma
            && let Some(spaces) = self.string_literal_continuation_after_layout_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_open_paren_arg_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.constructor_initializer_ternary_arm_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        layout
    }

    pub(in super::super) fn apply_ternary_template_and_source_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if line.trim_start().starts_with('{')
            && self.has_pending_case_label_brace()
            && self.output.last_non_empty_line().is_some_and(|previous| {
                let previous = previous.trim_start();
                previous.starts_with('#')
                    && !preprocessor_directive(previous).is_some_and(is_conditional_preprocessor)
            })
        {
            layout.exact_indent_spaces =
                Some(self.current_line_indent_spaces() + self.options.indent_width);
        }
        if let Some(spaces) = self.assignment_ternary_branch_after_colon_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.return_ternary_branch_after_colon_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.return_ternary_call_argument_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !self.options.indent_after_parens
            && let Some(spaces) = self.return_ternary_tail_output_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.header_operator_continuation_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.logical_condition_sibling_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.ternary_operator_sibling_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_condition_closing_paren_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !line.trim_start().starts_with(['{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_end().ends_with(',')
            && self
                .output
                .iter()
                .rev()
                .take(4)
                .any(|line| line.contains("@["))
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if !line.trim_start().starts_with(['{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_end().ends_with(',')
            && self.output.iter().rev().take(4).any(|line| {
                line.contains("std::conditional <") || line.contains("std::conditional<")
            })
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if !line.trim_start().starts_with(['{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_end().ends_with(',')
            && previous.trim_start().starts_with("void_t<")
        {
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if !line.trim_start().starts_with(['{', '}'])
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_end().ends_with(',')
            && let Some(using_pos) = find_outside_quotes(previous, " using ")
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            layout.exact_indent_spaces = unmatched_open_paren_column(previous_code)
                .map(|open| open + 1)
                .or(Some(using_pos + " using ".len()));
        }
        if let Some(spaces) = self.simple_template_base_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !line.trim_start().starts_with(['{', '}'])
            && self.constructor_initializer_base_indent_spaces().is_none()
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with(':')
            && previous.trim_end().ends_with(',')
            && let Some(open) = previous.rfind('{')
        {
            layout.exact_indent_spaces = Some(open + 1);
        }
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let previous_trimmed = previous.trim_start();
            let macro_before_previous = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous)
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty()
                        && trimmed
                            .chars()
                            .all(|ch| ch == '_' || ch.is_ascii_uppercase())
                });
            if self.token_input.token_source_line_indent
                >= layout.normal_indent * self.options.indent_width
                && self.token_input.token_source_line_indent > 0
                && ((previous_trimmed == "#endif" && line.trim_start().starts_with("auto "))
                    || ((line.trim_start().starts_with("noexcept(")
                        || line.trim_start().starts_with("->"))
                        && previous_trimmed.contains("noexcept(")
                        && self
                            .output
                            .iter()
                            .rev()
                            .take(4)
                            .any(|line| line.trim_start() == "#endif")))
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous.contains("//")
                && (previous_code.contains("noexcept (")
                    || (macro_before_previous && previous_code.contains("noexcept(")))
                && previous_code.ends_with('(')
                && self.token_input.token_source_line_indent > 0
                && !line.trim_start().starts_with("//")
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
        }
        if let Some(spaces) = self.logical_continuation_after_commented_noexcept_indent_spaces(line)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        if !line.trim_start().starts_with("//")
            && !line.trim_start().starts_with('{')
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.contains('#') && head_ends_binary_operator(previous_code) {
                let previous_trimmed = previous_code.trim_start();
                layout.exact_indent_spaces =
                    Some(if self.token_input.token_source_line_indent > 0 {
                        self.token_input.token_source_line_indent
                    } else if previous_trimmed.starts_with('#')
                        && preprocessor_directive(previous_trimmed).is_some_and(|directive| {
                            directive.starts_with("else") || directive.starts_with("elif")
                        })
                    {
                        leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width
                    } else if !previous_trimmed.starts_with('#') {
                        leading_visual_width(previous, self.options.tab_width)
                    } else {
                        layout
                            .exact_indent_spaces
                            .unwrap_or(layout.indent * self.options.indent_width)
                    });
            }
        }
        if self.token_input.token_source_line_indent > 0
            && !line.trim_start().starts_with("//")
            && !line.trim_start().starts_with('{')
            && (0..self.output.len()).rev().take(8).any(|index| {
                let previous = &self.output[index];
                let code = self.output.code(index);
                let macro_before_previous = (0..index)
                    .rev()
                    .map(|before| self.output.trimmed(before))
                    .find(|trimmed| !trimmed.is_empty())
                    .is_some_and(|trimmed| {
                        trimmed
                            .chars()
                            .all(|ch| ch == '_' || ch.is_ascii_uppercase())
                    });
                previous.contains("//")
                    && (code.contains("noexcept (")
                        || (macro_before_previous && code.contains("noexcept(")))
                    && code.ends_with('(')
            })
        {
            layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
        }
        layout
    }
}

impl FormatEngine<'_> {
    pub(in super::super) fn apply_brace_array_and_objc_dictionary_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if line.trim() == "{"
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with('{') && previous_code.trim() != "{" {
                let namespace_parent = matches!(
                    self.stack_state.brace_type_stack.last(),
                    Some(FormatterBraceType::Namespace)
                ) && !self.options.indent_namespaces;
                let extra = if namespace_parent {
                    0
                } else {
                    self.options.indent_width
                };
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width) + extra);
            } else if previous_code.ends_with('/')
                && self
                    .output
                    .iter()
                    .rev()
                    .take(4)
                    .any(|line| line.contains('#'))
            {
                layout.exact_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width * 3,
                );
            }
        }
        if let Some(spaces) = self.recent_double_brace_indent_spaces(line) {
            layout.exact_indent_spaces = Some(spaces);
        }
        if line.trim_start().starts_with("//")
            && self.token_input.token_source_line_indent > 0
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with("//")
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if previous_indent > 0 {
                layout.exact_indent_spaces = Some(previous_indent);
            }
        }
        if self.current_inline_array_column().is_some()
            && !self.innermost_brace_is_compound_literal()
            && !self.enclosed_in_compound_literal()
            && !line.trim_start().starts_with(['.', '{', '}'])
            && !self
                .output
                .last_non_empty_line()
                .is_some_and(|previous| previous.trim_start().starts_with(':'))
        {
            if let Some(column) = self.current_inline_array_column()
                && let Some(previous) = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
            {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                if self.stack_state.paren_depth == 0
                    && previous_code.ends_with(',')
                    && previous_code.contains('{')
                    && !line.trim_start().starts_with('}')
                    && let Some(open) = previous_code.find('{')
                {
                    let brace_column =
                        visual_width_from(&previous_code[..open], 0, self.options.tab_width);
                    let previous_body = previous_code[open + 1..].trim_start();
                    let current_body = line.trim_start();
                    let starts_new = |text: &str| {
                        text.strip_prefix("new")
                            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
                    };
                    let spaces = if starts_new(previous_body) && starts_new(current_body) {
                        brace_column + self.options.indent_width
                    } else {
                        column.max(brace_column + 1)
                    };
                    let base = self.continuation_base_indent() * self.options.indent_width;
                    let source_indent = self
                        .token_input
                        .token_source_line_indent
                        .max(leading_visual_width(line, self.options.tab_width));
                    let stream_indent = previous_code
                        .find(" << ")
                        .or_else(|| previous_code.find(" >> "))
                        .map(|index| index + 1);
                    layout.exact_indent_spaces = Some(
                        if spaces.saturating_sub(base) > self.options.max_continuation_indent {
                            stream_indent
                                .or((source_indent > 0).then_some(source_indent))
                                .unwrap_or(spaces)
                        } else {
                            spaces
                        },
                    );
                } else if self.stack_state.paren_depth == 0
                    && (previous_code.ends_with(',')
                        || (previous_code.contains('{') && previous.len() != previous_code.len()))
                {
                    layout.exact_indent_spaces = Some(column);
                }
            }
            let current_spaces = layout
                .exact_indent_spaces
                .unwrap_or(layout.indent * self.options.indent_width);
            let block_comment_after_statement = line.trim_start().starts_with("/*")
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|previous| {
                        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                        code.ends_with(';') && unmatched_open_paren_column(code).is_none()
                    });
            if (self.state.statement_depth() > 0 || self.stack_state.paren_depth > 0)
                && self.token_input.token_source_line_indent > current_spaces
                && !block_comment_after_statement
            {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            } else if let Some(previous) = self.output.last_non_empty_line()
                && previous.trim_end().ends_with(',')
                && !previous.trim_start().starts_with([':', ','])
            {
                let previous_spaces = leading_visual_width(previous, self.options.tab_width);
                if previous_spaces > current_spaces {
                    layout.exact_indent_spaces = Some(previous_spaces);
                }
            }
        }
        if !line.trim_start().starts_with('}')
            && let Some(previous) = self.output.last_non_empty_line()
            && is_comment_line(previous.trim_start())
            && let Some(before_comment) = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| !line.trim().is_empty())
        {
            let code = before_comment[..trailing_comment_split_limit(before_comment)].trim_end();
            if code.ends_with(',') {
                layout.exact_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
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
        if let Some(spaces) =
            self.ordinary_ternary_colon_indent_spaces(line, layout.line_kind, layout.normal_indent)
        {
            layout.exact_indent_spaces = Some(spaces);
        }
        layout.exact_indent_spaces =
            self.objc_dictionary_indent_spaces(line, layout.exact_indent_spaces);
        layout
    }

    pub(in super::super) fn apply_objc_pre_alignment_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if line.trim_start().starts_with("/**")
            && (self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim_start().starts_with("@interface"))
                || self
                    .previous_pre_adjust_line
                    .as_ref()
                    .is_some_and(|line| line.trim_start().starts_with("@interface")))
        {
            layout.indent = 1;
            layout.exact_indent_spaces = Some(self.options.indent_width);
        } else if matches!(line.trim_start().chars().next(), Some('*'))
            && self
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|previous| {
                    let previous_text = previous.trim_start();
                    (previous_text.starts_with("/**") || previous_text.starts_with('*'))
                        && !previous_text.trim_end().ends_with("*/")
                })
        {
            layout.exact_indent_spaces = self
                .previous_pre_adjust_line
                .as_ref()
                .map(|previous| leading_visual_width(previous, self.options.tab_width));
        }
        if self.token_input.token_source_line_indent > 0
            && let Some(previous) = self.output.last_non_empty_line()
        {
            let previous_trimmed = previous.trim();
            if previous_trimmed.starts_with(')') && previous_trimmed.ends_with("\";") {
                layout.exact_indent_spaces = Some(self.token_input.token_source_line_indent);
            }
        }
        layout
    }

    pub(in super::super) fn align_objc_and_publish_return_type(
        &mut self,
        line: &str,
        line_closed_brackets: &[BracketFrame],
        mut layout: LineLayout,
    ) -> LineRoute<AlignedLineLayout> {
        let objc_alignment = self.apply_objc_message_alignment(
            line,
            line_closed_brackets,
            layout.indent,
            layout.exact_indent_spaces,
        );
        layout.indent = objc_alignment.indent_level;
        layout.exact_indent_spaces = objc_alignment.exact_indent_spaces;
        let restore_objc_message_align = objc_alignment.restore_message_align;
        self.update_case_body_indent(layout.line_kind);
        let case_unindent_closing_line = self.case_closing_line_needs_unindent();
        self.update_case_brace_unindent(layout.line_kind, line);
        if self.try_publish_attached_return_type(line)
            || self.try_publish_split_return_type(line, layout.indent, layout.exact_indent_spaces)
        {
            return LineRoute::Published;
        }
        if self.stack_state.paren_depth == 0
            && !self.in_initializer_brace()
            && line.trim_start().starts_with('"')
            && let Some(previous) = self.output.last_non_empty_line()
            && previous.trim_start().starts_with('"')
        {
            let case_unindent = (self.line_adjuster.total_case_unindent_depth()
                * self.options.indent_width)
                .max(self.adjusted_line_indent_delta(previous));
            layout.exact_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width) + case_unindent);
        }
        LineRoute::Layout(AlignedLineLayout {
            layout,
            restore_objc_message_align,
            case_unindent_closing_line,
        })
    }
}
