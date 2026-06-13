use super::super::FormatEngine;
use super::super::closing_braces::starts_post_closing_declaration;
use super::super::columns::leading_visual_width;
use super::super::comments::line_comment_backslash_trailing_space;
use super::super::line_scan::is_comment_line;
use super::super::line_scan::{line_comment_split_limit, trailing_comment_split_limit};
use super::super::literals::first_string_literal_start;
use super::super::operator_chains;

use super::super::preprocessor::preprocessor_directive;
use super::super::state::PreviousToken;
use super::super::switch_cases::{case_label_with_trailing_comment, split_switch_label_statement};
use crate::source::lex::trailing_word;

impl FormatEngine<'_> {
    pub(in super::super) fn finish_disabled_line(&mut self) {
        let line = self.take_current();
        self.publish_unadjusted_line(line);
        self.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    pub(in super::super) fn finish_line(&mut self) {
        let block_comment_close_paren_ends_declaration =
            self.block_comment_close_paren_ends_declaration;
        self.block_comment_close_paren_ends_declaration = false;
        self.previous_block_comment_close_paren_ended_declaration = false;
        let preserve_raw_literal_line_end = self.literal_line.preserve_raw_literal_line_end;
        let preserve_run_in_join_space = self.preserve_run_in_join_space;
        if self.try_finish_multiline_literal_line() {
            return;
        }
        if self.current_is_preindented && self.current.contains('\x0c') {
            let line = self.take_current();
            self.adjust_and_publish_line(line);
            self.reset_after_finished_line();
            return;
        }
        let preserve_line_comment_trailing_space =
            line_comment_backslash_trailing_space(&self.current);
        if !preserve_line_comment_trailing_space
            && !self.literal_line.unterminated_raw_literal
            && !preserve_raw_literal_line_end
            && !preserve_run_in_join_space
        {
            self.trim_current_end();
        }
        if self.try_finish_preindented_comment_line(block_comment_close_paren_ends_declaration) {
            return;
        }
        if self.current_is_preindented {
            self.continuation_indent
                .input_line_continuation_indent
                .take();
            let line = self.take_current();
            let trimmed = line.trim_end().to_string();
            if trimmed
                .split_once("*/")
                .is_some_and(|(_, after)| after.trim_end().ends_with(';'))
            {
                self.continuation_indent.next_line_indent_spaces = None;
                self.stack_state.clear_continuation_indents();
            }
            if !trimmed.trim().is_empty() {
                self.push_raw_comment_output_line(trimmed);
                if block_comment_close_paren_ends_declaration {
                    self.previous_block_comment_close_paren_ended_declaration = true;
                }
            }
            self.reset_after_finished_line();
            return;
        }

        let line = if preserve_line_comment_trailing_space
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            self.current.trim_start().to_string()
        } else {
            self.current.trim().to_string()
        };
        self.finish_ordinary_line(&line);
    }

    fn finish_ordinary_line(&mut self, line: &str) {
        if !line.is_empty() {
            let output_line_index = self.output.len();
            let code = line[..trailing_comment_split_limit(&line)].trim_end();
            let code_ends_with_brace = code.ends_with('}');
            self.frame_stack
                .mark_last_closed_brace_line_end(output_line_index, code_ends_with_brace);
            self.observe_operator_chain_line_context(output_line_index, code);
            let clear_string_after_line = !code.trim_start().starts_with('#')
                && (code.ends_with(';') || first_string_literal_start(code).is_none());
            let clear_stream_after_line = code.ends_with(';');
            if self.line_state.column1_line_comment
                && !self.options.indent_col1_comments
                && line.starts_with("//")
            {
                if self.take_block_spacing_blank(&line) {
                    self.push_empty_line();
                }
                self.publish_unadjusted_line(line.to_string());
                if let Some(output_indent) =
                    self.observe_operator_chain_output_line(output_line_index)
                {
                    self.frame_stack
                        .mark_delimiter_line_output_indent(output_line_index, output_indent);
                }
                if clear_string_after_line {
                    self.frame_stack.clear_string_continuations();
                }
                self.run_in_state.current_run_in_indent = self.continuation_indent.next_line_indent;
                self.reset_after_finished_line();
                return;
            }

            let case_label_with_comment = case_label_with_trailing_comment(&line);
            if self.options.break_one_line_statements
                && let Some((label, statement)) = split_switch_label_statement(&line)
            {
                self.finish_line_text(&label);
                if statement.trim_start().starts_with('#') {
                    self.adjust_and_publish_line(statement.trim_start().to_string());
                    self.preprocessor.last_output_was_preprocessor = true;
                } else {
                    let label_spaces = self
                        .output
                        .last()
                        .map(|line| leading_visual_width(line, self.options.tab_width))
                        .unwrap_or(0);
                    let extra = if self.unmatched_closing_brace_recovery {
                        0
                    } else if statement.trim_start().starts_with([
                        '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
                        '~',
                    ]) {
                        self.options.indent_width * 2
                    } else {
                        self.options.indent_width
                    };
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(label_spaces + extra);
                    self.finish_line_text(&statement);
                }
            } else {
                self.finish_line_text(&line);
            }
            if let Some(output_indent) = self.observe_operator_chain_output_line(output_line_index)
                && let Some(output_line) = self.output.get(output_line_index)
            {
                self.frame_stack
                    .mark_delimiter_line_output_indent(output_line_index, output_indent);
                let output_code =
                    output_line[..trailing_comment_split_limit(output_line)].trim_end();
                let line_comment_limit = line_comment_split_limit(&line);
                let code_before_line_comment = line[..line_comment_limit].trim_end();
                let embedded_preprocessor = output_code.contains('#')
                    && !output_code.trim_start().starts_with('#')
                    || (line_comment_limit < line.len() || line.trim_end().ends_with(':'))
                        && code_before_line_comment.contains('#')
                        && !code_before_line_comment.trim_start().starts_with('#');
                if output_code.trim_start().starts_with("return ")
                    && output_code.contains('#')
                    && !output_code.ends_with(';')
                {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(output_indent + "return ".len());
                } else if embedded_preprocessor {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(if output_code.ends_with(':') {
                            output_indent + self.options.indent_width
                        } else {
                            output_indent
                        });
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_stream_frames_and_logical_indent(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                } else if starts_post_closing_declaration(output_code) {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(output_indent);
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_operator_chain_state(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                }
                if output_code.trim_start().starts_with("else,") {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(0);
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_operator_chain_state(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                }
                if output_code.trim_start().starts_with("#define") && !output_code.ends_with('\\') {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = None;
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_operator_chain_state(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                }
                if preprocessor_directive(output_code.trim_start()) == Some("endif")
                    && let Some(previous) = self.output[..output_line_index]
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                    && (is_comment_line(previous.trim_start())
                        || previous.trim_start().starts_with("/*"))
                {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(leading_visual_width(previous, self.options.tab_width));
                }
                if output_code.trim() == "?" {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(output_indent);
                    self.stack_state.clear_continuation_indents();
                }
                if output_code.trim() == "catch"
                    && self.output[..output_line_index]
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .is_none_or(|line| {
                            !line[..trailing_comment_split_limit(line)]
                                .trim_end()
                                .ends_with('}')
                        })
                {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(output_indent);
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_operator_chain_state(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                }
                if output_code.ends_with("; catch") {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(self.state.indent() * self.options.indent_width);
                    self.stack_state.clear_continuation_indents();
                    operator_chains::clear_operator_chain_state(
                        &mut self.frame_stack,
                        &mut self.continuation_indent.logical_chain_indent_spaces,
                    );
                }
            }
            for line_index in output_line_index..self.output.len() {
                self.observe_ternary_colon_output_line(line_index);
                let output_line = &self.output[line_index];
                let output_code =
                    output_line[..trailing_comment_split_limit(output_line)].trim_end();
                if output_code.trim_start().starts_with("return ")
                    && output_code.contains('#')
                    && !output_code.ends_with(';')
                {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(
                        leading_visual_width(output_line, self.options.tab_width) + "return ".len(),
                    );
                }
                if output_code.trim_start() == "#else"
                    && let Some(previous_line) = self.output[..line_index]
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                    && trailing_word(previous_line.trim_end()) == "do"
                {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(
                        leading_visual_width(previous_line, self.options.tab_width)
                            + self.options.indent_width * 2,
                    );
                }
            }
            if clear_stream_after_line {
                operator_chains::clear_operator_chain_frames(&mut self.frame_stack);
            }
            if clear_string_after_line {
                self.frame_stack.clear_string_continuations();
            }
            self.observe_finished_block_spacing_line();
            if let Some(spaces) = self
                .continuation_indent
                .clear_continuation_after_line
                .take()
            {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
                self.stack_state.clear_continuation_indents();
            }
            if case_label_with_comment && let Some(previous) = self.output.last() {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if self.unmatched_closing_brace_recovery {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(0);
            self.state.clear_continuation_indents();
            self.stack_state.clear_continuation_indents();
            operator_chains::clear_operator_chain_state(
                &mut self.frame_stack,
                &mut self.continuation_indent.logical_chain_indent_spaces,
            );
        }
        self.run_in_state.current_run_in_indent = self.continuation_indent.next_line_indent;
        self.max_length_line.reset();
        self.literal_line.preserve_raw_literal_line_end = false;
        self.preserve_run_in_join_space = false;
        self.reset_after_finished_line();
    }

    pub(in super::super) fn finish(mut self) -> String {
        self.flush_backslash_body_parts();
        self.merge_source_run_in_braces();
        self.merge_run_in_comment_braces();
        if self.output.is_empty() {
            String::new()
        } else {
            let mut output = self.output.join(self.options.line_break());
            output.push_str(self.options.line_break());
            output
        }
    }
}
