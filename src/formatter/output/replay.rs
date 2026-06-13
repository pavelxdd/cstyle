use super::super::FormatEngine;
use super::model::LineReplayLayout;
use crate::config::BraceStyle;

impl FormatEngine<'_> {
    pub(super) fn take_line_replay_layout(&mut self, line: &str) -> LineReplayLayout {
        let input_continuation_indent = self
            .continuation_indent
            .input_line_continuation_indent
            .take();
        let closed_delimiter_continuation_indent = self
            .frame_stack
            .take_line_closed_delimiter_continuation_indent();
        let constructor_lambda_header_indent_spaces =
            self.replayed_constructor_lambda_header_indent_spaces(line);
        let inline_body_owner_indent_spaces =
            self.output.last_non_empty_line().and_then(|previous| {
                self.replayed_inline_case_body_indent_spaces(
                    previous,
                    closed_delimiter_continuation_indent.is_some(),
                )
                .or_else(|| {
                    self.replayed_inline_access_body_indent_spaces(
                        previous,
                        closed_delimiter_continuation_indent.is_some(),
                    )
                })
            });
        let lisp_attached_suffix_indent_spaces = self.replayed_lisp_attached_suffix_indent_spaces();
        let header_operator_indent_spaces =
            self.replayed_header_operator_indent_spaces(line, closed_delimiter_continuation_indent);
        let closed_lambda_parameter_list =
            self.frame_stack.take_line_closed_lambda_parameter_list();
        let break_lambda_parameters = matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Horstmann
        );
        let closed_split_lambda_parameter_list =
            closed_lambda_parameter_list && break_lambda_parameters;
        let lambda_parameter_indent_spaces = self.replayed_lambda_parameter_indent_spaces(
            closed_lambda_parameter_list,
            break_lambda_parameters,
        );
        LineReplayLayout {
            input_continuation_indent,
            closed_delimiter_continuation_indent,
            constructor_lambda_header_indent_spaces,
            inline_body_owner_indent_spaces,
            lisp_attached_suffix_indent_spaces,
            header_operator_indent_spaces,
            closed_lambda_parameter_list,
            closed_split_lambda_parameter_list,
            lambda_parameter_indent_spaces,
        }
    }
}
