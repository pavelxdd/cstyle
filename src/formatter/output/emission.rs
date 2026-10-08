use crate::config::BraceStyle;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::output::line_adjust::macro_call_starts_with;
use crate::formatter::output::model::{LineLayout, PostEmissionLayout};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{ContainsAnyByte, line_ends_with_comment};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::comments::line_comment_backslash_trailing_space;

impl FormatEngine<'_> {
    pub(super) fn publish_formatted_line_layout(
        &mut self,
        line: &LineView<'_>,
        layout: &LineLayout,
    ) -> usize {
        let emitted_indent_spaces = layout
            .exact_indent_spaces
            .unwrap_or(layout.indent * self.options.indent_width);
        if let Some(spaces) = layout.exact_indent_spaces {
            self.push_formatted_line_exact(line, layout.indent, spaces);
        } else {
            self.push_formatted_line(line, layout.indent);
        }
        emitted_indent_spaces
    }

    pub(crate) fn push_formatted_line(&mut self, line: &str, indent: usize) {
        self.push_formatted_line_with_indent(
            line,
            indent,
            ContinuationIndent::Level(indent),
            ContinuationIndent::Spaces(
                (indent + self.options.continuation_indent) * self.options.indent_width,
            ),
        );
    }

    pub(crate) fn push_formatted_line_exact(
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

    pub(crate) fn push_output_line_with_indent(
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

    pub(crate) fn push_output_line(&mut self, line: &str, indent: usize) {
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
            std::mem::take(&mut self.layout.literal_line.preserve_raw_literal_line_end);
        let preserve_run_in_join_space = std::mem::take(&mut self.preserve_run_in_join_space);
        let body = if line_comment_backslash_trailing_space(line)
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            line
        } else {
            line.trimmed_end()
        };
        let mut output = String::with_capacity(indent * self.options.indent_width + body.len());
        self.options.push_indent_prefix(&mut output, indent);
        output.push_str(body);
        self.adjust_and_publish_line(output);
    }

    pub(crate) fn push_output_line_spaces(
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
            std::mem::take(&mut self.layout.literal_line.preserve_raw_literal_line_end);
        let preserve_run_in_join_space = std::mem::take(&mut self.preserve_run_in_join_space);
        let body = if line_comment_backslash_trailing_space(line)
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            line
        } else {
            line.trimmed_end()
        };
        let structural_level = self.constructor_initializer_prefix_level(structural_level);
        let mut output = String::with_capacity(spaces + body.len());
        self.options
            .push_continuation_indent_prefix(&mut output, structural_level, spaces);
        output.push_str(body);
        self.adjust_and_publish_line(output);
    }

    pub(crate) fn previous_output_indent_prefix(&self) -> String {
        self.layout
            .previous_pre_adjust_line
            .as_ref()
            .map(|line| {
                line.chars()
                    .take_while(|ch| matches!(ch, ' ' | '\t'))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn adjust_and_publish_line(&mut self, line: String) {
        // A line the tree anchors keeps its place.
        let line = match self.comment_led_statement_line(&line) {
            Some(anchored) => anchored,
            None => self.align_adjacent_block_comments_before_adjustment(line),
        };
        let line = self.macro_block_body_line_before_adjustment(line);
        self.observe_raw_output_comment_frame(&line);
        // The line before keeps its buffer.
        match &mut self.layout.previous_pre_adjust_line {
            Some(previous) => previous.clone_from(&line),
            previous => *previous = Some(line.clone()),
        }
        let observed = self
            .adjuster_before_observed_lone_brace
            .take()
            .filter(|(index, _)| *index == self.output.len());
        self.adjuster_before_lone_brace = (line.trimmed() == "}").then(|| {
            // The brace's own count of the switch it may close goes back too.
            observed.unwrap_or_else(|| (self.output.len(), self.layout.line_adjuster.clone()))
        });
        let line = self.layout.line_adjuster.adjust_line(line);
        self.shift_run_in_brace_for_spared_level(&line);
        let line = self.align_allman_control_brace_to_header(line);
        let line = self.align_else_opening_brace_after_adjustment(line);
        self.publish_ready_line(line);
    }

    /// astyle unindents a case block's lines as joined, where a comment
    /// after its closer keeps the joined line a level of its unindent.
    fn shift_run_in_brace_for_spared_level(&mut self, line: &str) {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) || !self.layout.line_adjuster.last_line_comment_spared_level()
        {
            return;
        }
        let lone_brace = |index: usize| {
            self.output
                .get(index)
                .is_some_and(|row| row.trimmed() == "{")
        };
        let len = self.output.len();
        let target = if line.trimmed_start().starts_with('}') {
            // The closer joins the row before, which joins a lone brace
            // before it.
            if self.output.last().is_none_or(|previous| {
                previous.trimmed().is_empty()
                    || previous.trimmed_start().starts_with('#')
                    || line_ends_with_comment(previous)
            }) {
                return;
            }
            if len >= 2 && lone_brace(len - 2) {
                len - 2
            } else {
                len - 1
            }
        } else if !line.trimmed_start().starts_with_any(b"#/") && len >= 1 && lone_brace(len - 1) {
            len - 1
        } else {
            return;
        };
        let indent = self.options.indent_prefix(1);
        let row = format!("{indent}{}", self.output[target]);
        self.output.set(target, row);
    }

    pub(crate) fn adjust_and_publish_raw_literal_line(
        &mut self,
        line: String,
        structural_start: usize,
    ) {
        self.layout.previous_pre_adjust_line = Some(line.clone());
        let line = self.layout.line_adjuster.adjust_raw_literal_line(line);
        self.output.push_raw_literal(line, structural_start);
    }

    fn macro_block_body_line_before_adjustment(&self, line: String) -> String {
        if !self.layout.line_adjuster.is_in_macro_block() || self.options.macro_blocks.is_empty() {
            return line;
        }
        let trimmed = line.trimmed_start();
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
        self.layout.previous_pre_adjust_line = Some(line.clone());
        self.publish_ready_line(line);
    }

    pub(crate) fn publish_ready_line(&mut self, line: String) {
        // Each step that moves the line gives a new line, viewed anew.
        let view = LineView::new(&line);
        let normalized = self.normalize_ready_preprocessor_line(&view);
        let view = normalized.as_deref().map_or(view, LineView::new);
        let closing = match self.ready_objc_method_closing_brace_indent_spaces(&view) {
            Some(spaces) => Some(reindented(spaces, &view)),
            None => self.align_isolated_closing_brace_line(&view),
        };
        let view = closing.as_deref().map_or(view, LineView::new);
        let header = self
            .ready_non_paren_header_indent_spaces(&view)
            .map(|spaces| reindented(spaces, &view));
        let view = header.as_deref().map_or(view, LineView::new);
        let embedded = self
            .ready_embedded_preprocessor_return_indent_spaces(&view)
            .map(|spaces| reindented(spaces, &view));
        let view = embedded.as_deref().map_or(view, LineView::new);
        let output_line_index = self.output.len();
        let output_line_hints = self.output.line_hints(view.trimmed_start());
        self.finish_define_line(&view);
        let anchored_part = self.max_length_line.take_anchored_part();
        let spaces = if anchored_part {
            None
        } else {
            self.ternary_operator_tail_indent_spaces(&view)
                .or_else(|| self.assignment_rhs_indent_spaces(&view))
                .or_else(|| self.trailing_return_function_parameter_tail_indent_spaces(&view))
        };
        let tail = spaces
            .filter(|&spaces| leading_visual_width(&view, self.options.tab_width) != spaces)
            .map(|spaces| reindented(spaces, &view));
        let view = tail.as_deref().map_or(view, LineView::new);
        let split = self.postprocess_ready_operator_chain_line(output_line_index, &view);
        drop(view);
        if let Some((colon, tail)) = split {
            self.output.push(colon);
            self.output.push(tail);
        } else {
            let line = tail
                .or(embedded)
                .or(header)
                .or(closing)
                .or(normalized)
                .unwrap_or(line);
            self.output.push_with_hints(line, output_line_hints);
        }
    }

    pub(super) fn deferred_post_emission_layout(
        &self,
        line: &LineView<'_>,
        layout: &LineLayout,
        restore_objc_message_align: Option<usize>,
    ) -> PostEmissionLayout {
        PostEmissionLayout {
            restore_objc_message_align,
            split_condition_body_indent_spaces: self
                .split_else_condition_body_indent_spaces(line, layout.line_kind),
            ternary_call_clear_indent_spaces: self
                .ternary_call_clear_indent_spaces(line, layout.line_kind),
            else_while_brace: layout.else_while_brace,
        }
    }

    pub(super) fn apply_post_emission_state(
        &mut self,
        line: &LineView<'_>,
        layout: &LineLayout,
        output_spaces: usize,
        emitted_indent_spaces: usize,
        post_emission: PostEmissionLayout,
    ) {
        let line_kind = layout.line_kind;
        self.restore_objc_message_alignment(post_emission.restore_objc_message_align);
        self.record_preprocessor_branch_body_indent(line, emitted_indent_spaces);
        if let Some(spaces) = post_emission.split_condition_body_indent_spaces {
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = post_emission.ternary_call_clear_indent_spaces {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.layout.nesting.clear_continuation_indents();
        }
        self.observe_split_else_logical_statement_indent(line, line_kind);
        self.restore_split_else_call_argument_indent_after_emission(line, line_kind);
        self.observe_emitted_label_body_indent(
            line,
            line_kind,
            layout.exact_indent_spaces.unwrap_or(output_spaces),
        );
        if std::mem::take(&mut self.brace_frame_awaits_line) {
            self.rebase_brace_frame_on_emitted_line();
        }
        self.update_typedef_function_pointer_frame(line);
        self.observe_formatted_output_comment_frame(line, output_spaces);
        // A row of code led by a dereference is no comment.
        let comment_row = self
            .output
            .len()
            .checked_sub(1)
            .and_then(|last| self.output.line_tokens(last))
            .is_none_or(|span| {
                (span.first..=span.last)
                    .map(|index| &self.tree.tokens[index])
                    .find(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
                    .is_none_or(|token| matches!(token, Token::Comment(..)))
            });
        if comment_row
            && matches!(
                line.trimmed_start(),
                text if text.starts_with("//")
                    || text.starts_with("/*")
                    || text.starts_with("*/")
                    || text == "*"
                    || text.starts_with("* ")
                    || text.starts_with("*\t")
            )
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
                .is_some_and(|previous| {
                    let previous = previous.trimmed();
                    previous.strip_prefix('}').map_or(previous, str::trim_start) == "else"
                })
        {
            let level = output_spaces / self.options.indent_width;
            self.layout.continuation_indent.set_next_line_level(level);
            self.layout.pending_braceless_block_bias = Some(level);
        }
        if post_emission.else_while_brace {
            self.layout
                .continuation_indent
                .set_next_line_level(layout.indent + 1);
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

/// `line` without its indent, after `spaces` spaces.
fn reindented(spaces: usize, line: &LineView<'_>) -> String {
    let text = line.trimmed_start();
    let mut reindented = String::with_capacity(spaces + text.len());
    reindented.extend(std::iter::repeat_n(' ', spaces));
    reindented.push_str(text);
    reindented
}
