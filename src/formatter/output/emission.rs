use super::super::FormatEngine;
use super::super::brace_classification::line_opens_lambda_block;
use super::super::buffer;

use super::super::columns::leading_visual_width;
use super::super::comments::line_comment_backslash_trailing_space;
use super::super::indentation::LineKind;

use super::super::line_adjust::macro_call_starts_with;
use super::super::operator_chains::ReadyOperatorChainLine;

use super::super::state::ContinuationIndent;
use super::model::{LineLayout, PostEmissionLayout};
use crate::config::IndentStyle;

impl FormatEngine<'_> {
    pub(super) fn publish_formatted_line_layout(
        &mut self,
        line: &str,
        layout: &LineLayout,
    ) -> usize {
        let emitted_indent_spaces = layout
            .exact_indent_spaces
            .unwrap_or(layout.indent * self.options.indent_width);
        if let Some(spaces) = layout.exact_indent_spaces {
            let structural_level = self.exact_brace_indent_level(line, layout.indent, spaces);
            self.push_formatted_line_exact(line, structural_level, spaces);
        } else {
            self.push_formatted_line(line, layout.indent);
        }
        emitted_indent_spaces
    }

    pub(in super::super) fn push_formatted_line(&mut self, line: &str, indent: usize) {
        self.push_formatted_line_with_indent(
            line,
            indent,
            ContinuationIndent::Level(indent),
            ContinuationIndent::Spaces(
                (indent + self.options.continuation_indent) * self.options.indent_width,
            ),
        );
    }

    pub(in super::super) fn push_formatted_line_exact(
        &mut self,
        line: &str,
        structural_level: usize,
        spaces: usize,
    ) {
        self.push_formatted_line_with_indent(
            line,
            structural_level,
            ContinuationIndent::Spaces(spaces),
            ContinuationIndent::Spaces(
                spaces + self.options.continuation_indent * self.options.indent_width,
            ),
        );
    }

    pub(in super::super) fn push_output_line_with_indent(
        &mut self,
        line: &str,
        structural_level: usize,
        indent: ContinuationIndent,
    ) {
        match indent {
            ContinuationIndent::Level(indent) => self.push_output_line(line, indent),
            ContinuationIndent::Spaces(spaces) => {
                self.push_output_line_spaces(line, structural_level, spaces)
            }
        }
    }

    pub(in super::super) fn push_output_line(&mut self, line: &str, indent: usize) {
        if line.is_empty() {
            self.push_empty_line();
            return;
        }
        if let Some(spaces) = self.ternary_operator_tail_indent_spaces(line) {
            self.push_output_line_spaces(line, indent, spaces);
            return;
        }
        if let Some(spaces) = self.case_comment_following_indent_spaces(line)
            && spaces > indent * self.options.indent_width
        {
            self.push_output_line_spaces(line, indent, spaces);
            return;
        }
        let preserve_raw_literal_line_end =
            std::mem::take(&mut self.literal_line.preserve_raw_literal_line_end);
        let preserve_run_in_join_space = std::mem::take(&mut self.preserve_run_in_join_space);
        let body = if line_comment_backslash_trailing_space(line)
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            line
        } else {
            line.trim_end()
        };
        let mut output = self.options.indent_prefix(indent);
        output.reserve(body.len());
        output.push_str(body);
        self.adjust_and_publish_line(output);
    }

    pub(in super::super) fn push_output_line_spaces(
        &mut self,
        line: &str,
        structural_level: usize,
        spaces: usize,
    ) {
        if line.is_empty() {
            self.push_empty_line();
            return;
        }
        let spaces = self
            .return_ternary_colon_after_multiline_template_declaration_indent_spaces(line)
            .map_or(spaces, |target| spaces.max(target));
        let spaces = self
            .ternary_operator_tail_indent_spaces(line)
            .unwrap_or(spaces);
        let spaces = self
            .case_comment_following_indent_spaces(line)
            .map_or(spaces, |target| spaces.max(target));
        let preserve_raw_literal_line_end =
            std::mem::take(&mut self.literal_line.preserve_raw_literal_line_end);
        let preserve_run_in_join_space = std::mem::take(&mut self.preserve_run_in_join_space);
        let body = if line_comment_backslash_trailing_space(line)
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            line
        } else {
            line.trim_end()
        };
        let structural_level = self.constructor_initializer_prefix_level(structural_level);
        let mut output = self
            .options
            .continuation_indent_prefix(structural_level, spaces);
        output.reserve(body.len());
        output.push_str(body);
        self.adjust_and_publish_line(output);
    }

    pub(in super::super) fn previous_output_indent_prefix(&self) -> String {
        self.previous_pre_adjust_line
            .as_ref()
            .map(|line| {
                line.chars()
                    .take_while(|ch| matches!(ch, ' ' | '\t'))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(in super::super) fn adjust_and_publish_line(&mut self, line: String) {
        let line = self.align_adjacent_block_comments_before_adjustment(line);
        let line = self.macro_block_body_line_before_adjustment(line);
        self.observe_raw_output_comment_frame(&line);
        let brace_indent_before_adjustment =
            (line.trim() == "{").then(|| leading_visual_width(&line, self.options.tab_width));
        self.previous_pre_adjust_line = Some(line.clone());
        let line = self.line_adjuster.adjust_line(line);
        let line =
            self.align_else_opening_brace_after_adjustment(line, brace_indent_before_adjustment);
        self.publish_ready_line(line);
    }

    pub(in super::super) fn adjust_and_publish_raw_literal_line(
        &mut self,
        line: String,
        structural_start: usize,
    ) {
        self.previous_pre_adjust_line = Some(line.clone());
        let line = self.line_adjuster.adjust_raw_literal_line(line);
        self.output.push_raw_literal(line, structural_start);
    }

    fn macro_block_body_line_before_adjustment(&self, line: String) -> String {
        if !self.line_adjuster.is_in_macro_block() || self.options.macro_blocks.is_empty() {
            return line;
        }
        let trimmed = line.trim_start();
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || self
                .options
                .macro_blocks
                .iter()
                .any(|(_, end)| macro_call_starts_with(trimmed, end))
        {
            return line;
        }
        let Some(begin_indent) = self.current_macro_block_begin_indent_spaces() else {
            return line;
        };
        let current_indent = leading_visual_width(&line, self.options.tab_width);
        if current_indent <= begin_indent {
            return line;
        }
        let prefix = self.options.continuation_indent_prefix(
            0,
            current_indent.saturating_sub(self.options.indent_width),
        );
        format!("{prefix}{}", line.trim_start_matches([' ', '\t']))
    }

    pub(super) fn publish_unadjusted_line(&mut self, line: String) {
        self.previous_pre_adjust_line = Some(line.clone());
        self.publish_ready_line(line);
    }

    pub(in super::super) fn publish_ready_line(&mut self, line: String) {
        let line = self.normalize_ready_preprocessor_line(line);
        let line = if let Some(spaces) = self.ready_objc_method_closing_brace_indent_spaces(&line) {
            format!("{}{}", " ".repeat(spaces), line.trim_start())
        } else {
            self.align_isolated_closing_brace_line(line)
        };
        let line = if let Some(spaces) = self.ready_non_paren_header_indent_spaces(&line) {
            format!("{}{}", " ".repeat(spaces), line.trim_start())
        } else {
            line
        };
        let line =
            if let Some(spaces) = self.ready_embedded_preprocessor_return_indent_spaces(&line) {
                format!("{}{}", " ".repeat(spaces), line.trim_start())
            } else {
                line
            };
        let output_line_index = self.output.len();
        let line_start = line.trim_start();
        let output_line_hints = buffer::output_line_hints(line_start);
        self.finish_define_line(&line);
        let line = if let Some(spaces) = self
            .ternary_operator_tail_indent_spaces(&line)
            .or_else(|| self.maximum_length_using_alias_rhs_indent_spaces(&line))
            .or_else(|| self.using_alias_rhs_indent_spaces(&line))
            .or_else(|| self.split_assignment_rhs_indent_spaces(&line))
            .or_else(|| self.trailing_return_function_parameter_tail_indent_spaces(&line))
        {
            if leading_visual_width(&line, self.options.tab_width) != spaces {
                format!("{}{}", " ".repeat(spaces), line.trim_start())
            } else {
                line
            }
        } else {
            line
        };
        match self.postprocess_ready_operator_chain_line(output_line_index, line) {
            ReadyOperatorChainLine::Single(line) => {
                self.output.push_with_hints(line, output_line_hints);
            }
            ReadyOperatorChainLine::SplitTernary { colon, tail } => {
                self.output.push(colon);
                self.output.push(tail);
            }
        }
    }

    pub(super) fn deferred_post_emission_layout(
        &self,
        line: &str,
        layout: &LineLayout,
        restore_objc_message_align: Option<usize>,
        next_sibling_statement_indent_spaces: Option<usize>,
    ) -> PostEmissionLayout {
        PostEmissionLayout {
            restore_objc_message_align,
            next_sibling_statement_indent_spaces,
            split_condition_body_indent_spaces: self
                .split_else_condition_body_indent_spaces(line, layout.line_kind),
            ternary_call_clear_indent_spaces: self
                .ternary_call_clear_indent_spaces(line, layout.line_kind),
            else_while_brace: layout.else_while_brace,
        }
    }

    pub(super) fn apply_post_emission_state(
        &mut self,
        line: &str,
        layout: &LineLayout,
        output_spaces: usize,
        emitted_indent_spaces: usize,
        post_emission: PostEmissionLayout,
    ) {
        let line_kind = layout.line_kind;
        self.restore_objc_message_alignment(post_emission.restore_objc_message_align);
        self.record_preprocessor_branch_body_indent(line, emitted_indent_spaces);
        if let Some(spaces) = post_emission.next_sibling_statement_indent_spaces
            && line.trim_end().ends_with(';')
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && self.options.indent_style == IndentStyle::Tabs
            && line.trim_end().ends_with('{')
            && line_opens_lambda_block(line)
            && let Some(output_line) = self.output.last()
        {
            self.continuation_indent.next_line_indent_spaces = Some(
                leading_visual_width(output_line, self.options.tab_width)
                    + self.options.indent_width,
            );
        }
        if let Some(spaces) = post_emission.split_condition_body_indent_spaces {
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = post_emission.ternary_call_clear_indent_spaces {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.stack_state.clear_continuation_indents();
        }
        self.observe_split_else_logical_statement_indent(line, line_kind);
        self.restore_split_else_call_argument_indent_after_emission(line, line_kind);
        self.observe_emitted_label_body_indent(
            line,
            line_kind,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        );
        self.update_typedef_function_pointer_frame(line);
        self.observe_formatted_output_comment_frame(line, output_spaces);
        if matches!(
            line.trim_start(),
            text if text.starts_with("//")
                || text.starts_with("/*")
                || text.starts_with("*/")
                || text == "*"
                || text.starts_with("* ")
                || text.starts_with("*\t")
        ) && self
            .output
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| previous.trim() == "else")
        {
            let level = output_spaces / self.options.indent_width;
            self.continuation_indent.next_line_indent = Some(level);
            self.continuation_indent.next_line_indent_spaces = None;
            self.pending_braceless_block_bias = Some(level);
        }
        if post_emission.else_while_brace {
            self.continuation_indent.next_line_indent = Some(layout.indent + 1);
            self.continuation_indent.next_line_indent_spaces = None;
        }
        self.observe_template_declaration_line(line);
        self.observe_member_spacing_boundary(line);
        self.observe_split_else_body_closing(line, output_spaces);
        self.set_next_line_indent_after_ternary_colon(
            line,
            line_kind,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        );
    }
}
