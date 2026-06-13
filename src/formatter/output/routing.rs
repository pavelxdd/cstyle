use super::super::FormatEngine;
use super::super::indentation::LineKind;
use super::model::{AlignedLineLayout, ContextualLineLayout, LineRoute};

impl FormatEngine<'_> {
    fn route_line_before_layout(&mut self, line: &str) -> LineRoute<LineKind> {
        if self.try_emit_whitesmith_lambda_close(line)
            || self.try_split_lambda_body_header(line)
            || self.try_split_operator_body(line)
            || self.try_emit_swig_line(line)
            || self.try_emit_backslash_body(line)
            || self.try_join_class_base_line(line)
        {
            return LineRoute::Published;
        }
        let observed_line_kind = self.line_adjuster.observe_line(line);
        if observed_line_kind == LineKind::Normal
            && line.trim_start() == "&else"
            && let Some(previous) = self.output.last_mut()
            && previous.contains("#if")
            && !previous.trim_start().starts_with('#')
        {
            previous.push_str(" & else");
            return LineRoute::Published;
        }
        LineRoute::Layout(observed_line_kind)
    }

    pub(in super::super) fn finish_line_text(&mut self, line: &str) {
        let replay = self.take_line_replay_layout(line);
        let line_closed_brackets = self.frame_stack.take_line_closed_brackets();
        self.record_closed_objc_message_indent(line, &line_closed_brackets);
        self.preprocessor.last_output_was_preprocessor = false;
        let LineRoute::Layout(observed_line_kind) = self.route_line_before_layout(line) else {
            return;
        };
        let layout = self.initial_line_layout(line, observed_line_kind, &replay);
        let layout = self.apply_initial_syntax_layout(line, layout);
        let layout = self.apply_initial_operator_and_header_layout(line, layout);
        let layout = self.apply_separated_header_and_comment_layout(line, layout);
        let layout = self.apply_label_and_conditional_context_layout(line, layout);
        let layout = self.apply_top_level_and_initializer_prefix_layout(line, layout);
        let layout = self.apply_constructor_and_call_layout(line, layout);
        let layout = self.apply_ternary_template_and_source_layout(line, layout);
        let layout = self.apply_brace_array_and_objc_dictionary_layout(line, layout);
        let layout = self.apply_objc_pre_alignment_layout(line, layout);
        let LineRoute::Layout(aligned_layout) =
            self.align_objc_and_publish_return_type(line, &line_closed_brackets, layout)
        else {
            return;
        };
        let AlignedLineLayout {
            layout,
            restore_objc_message_align,
            case_unindent_closing_line,
        } = aligned_layout;
        let layout = self.apply_spacing_new_call_and_stream_layout(line, layout);
        let layout = self.apply_lambda_return_call_and_stream_layout(line, &replay, layout);
        let layout = self.apply_comment_brace_and_ternary_operand_layout(line, layout);
        let layout = self.apply_late_call_and_operator_layout(line, layout);
        let contextual_layout = self.begin_contextual_line_layout(line, layout);
        let contextual_layout =
            self.apply_previous_output_call_and_initializer_layout(line, contextual_layout);
        let contextual_layout = self.apply_source_indent_brace_and_style_operator_layout(
            line,
            case_unindent_closing_line,
            contextual_layout,
        );
        let contextual_layout =
            self.apply_previous_statement_and_operator_prefix_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_label_else_and_conditional_contextual_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_none_style_else_and_conditional_body_layout(line, contextual_layout);
        let contextual_layout = self.apply_normal_literal_comma_and_split_else_entry_layout(
            line,
            &replay,
            contextual_layout,
        );
        let contextual_layout =
            self.apply_none_style_split_else_body_and_closing_layout(line, contextual_layout);
        let contextual_layout = self
            .apply_emitted_split_else_call_initializer_and_ternary_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_header_label_and_switch_contextual_layout(line, &replay, contextual_layout);
        let contextual_layout =
            self.apply_structural_split_else_body_contextual_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_string_call_and_emitted_split_else_case_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_macro_case_brace_and_return_contextual_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_conditional_literal_paren_and_else_layout(line, &replay, contextual_layout);
        let contextual_layout =
            self.apply_call_initializer_and_case_control_contextual_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_final_sibling_and_directive_contextual_layout(line, contextual_layout);
        let contextual_layout =
            self.apply_preprocessor_and_split_else_recovery_layout(line, contextual_layout);
        let ContextualLineLayout {
            layout,
            output_spaces,
            next_sibling_statement_indent_spaces,
            ..
        } = contextual_layout;
        let layout = self.apply_brace_header_case_and_initializer_correction_layout(line, layout);
        let post_emission = self.deferred_post_emission_layout(
            line,
            &layout,
            restore_objc_message_align,
            next_sibling_statement_indent_spaces,
        );
        let layout = self.apply_label_switch_case_and_opening_brace_correction_layout(line, layout);
        let layout = self.apply_final_recovery_floor_and_replay_layout(line, &replay, layout);
        let emitted_indent_spaces = self.publish_formatted_line_layout(line, &layout);
        self.apply_post_emission_state(
            line,
            &layout,
            output_spaces,
            emitted_indent_spaces,
            post_emission,
        );
    }
}
