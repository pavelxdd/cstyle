use super::FormatEngine;
use super::line_scan::trailing_comment_split_limit;
use super::state::FormatterBraceType;
use super::token::Token;
use crate::config::BraceStyle;

impl FormatEngine<'_> {
    pub(super) fn observe_blank_line_context(
        &mut self,
        tokens: &[Token],
        following_index: Option<usize>,
    ) {
        self.preserve_block_spacing_comment_blank =
            self.should_preserve_block_spacing_comment_blank(tokens, following_index);
    }

    pub(super) fn should_preserve_input_empty_line(&self) -> bool {
        !self.should_delete_input_empty_line()
            || self.should_keep_empty_line_before_attached_definition_brace()
    }

    pub(super) fn push_empty_line(&mut self) {
        self.flush_backslash_body_parts();
        self.clear_macro_interrupted_initializer_frames();
        self.reset_continuation_after_empty_line();
        self.clear_split_else_closing_state_on_empty_line();
        let line = if self.options.empty_line_fill {
            self.previous_output_indent_prefix()
        } else {
            String::new()
        };
        self.adjust_and_publish_line(line);
    }

    fn should_keep_empty_line_before_attached_definition_brace(&self) -> bool {
        if !self.options.delete_empty_lines
            || !matches!(
                self.options.brace_style,
                BraceStyle::Attach | BraceStyle::OneTrueBrace
            )
            || !self.next_line.leads_with_open_brace
        {
            return false;
        }
        let Some(previous) = self.output.last() else {
            return false;
        };
        let code = previous[..trailing_comment_split_limit(previous)].trim();
        if !code.ends_with(')') || code.starts_with('#') {
            return false;
        }
        let first = code
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
            .next()
            .unwrap_or_default();
        !self.is_header(first)
    }

    fn should_delete_input_empty_line(&self) -> bool {
        self.options.delete_empty_lines
            && !self.output.is_empty()
            && !self.stack_state.brace_type_stack.is_empty()
            && !self.preserve_block_spacing_comment_blank
            && !self.in_empty_line_protected_context()
    }

    fn in_empty_line_protected_context(&self) -> bool {
        !self.stack_state.brace_type_stack.is_empty()
            && self.stack_state.brace_type_stack.iter().all(|brace_type| {
                matches!(
                    brace_type,
                    FormatterBraceType::Extern
                        | FormatterBraceType::Namespace
                        | FormatterBraceType::Class
                        | FormatterBraceType::Interface
                        | FormatterBraceType::Struct
                        | FormatterBraceType::Union
                        | FormatterBraceType::Enum
                        | FormatterBraceType::Array
                        | FormatterBraceType::CompoundLiteral
                )
            })
    }
}
