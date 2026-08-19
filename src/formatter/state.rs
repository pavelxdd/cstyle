#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ContinuationIndent {
    Level(usize),
    Spaces(usize),
}

impl ContinuationIndent {
    pub(super) fn columns(self, indent_width: usize) -> usize {
        match self {
            Self::Level(level) => level * indent_width,
            Self::Spaces(columns) => columns,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct InlineArrayFrame {
    pub(super) depth: usize,
    pub(super) body_column: usize,
    pub(super) brace_column: usize,
    pub(super) output_line: usize,
    pub(super) aggregate_assignment: bool,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct InlineArrayState {
    pub(super) initializer_designator_bracket_depth: usize,
    pub(super) frames: Vec<InlineArrayFrame>,
    pub(super) current_closed_body_column: Option<(usize, bool)>,
    pub(super) aggregate_braces: Vec<bool>,
    pub(super) nested_brace_arrays: std::collections::HashSet<usize>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct TokenInputState {
    pub(super) previous_input_was_adjacent: bool,
    pub(super) previous_input_whitespace: Option<String>,
    pub(super) next_input_whitespace: Option<String>,
    pub(super) token_begins_source_line: bool,
    pub(super) token_source_column: usize,
    pub(super) token_source_line_indent: usize,
    pub(super) token_line_opens_with_brace: bool,
    pub(super) token_followed_by_final_line_comment: bool,
    pub(super) token_followed_by_line_comment_on_line: bool,
    pub(super) input_source_indent: usize,
    pub(super) has_next_meaningful_token: bool,
    pub(super) next_token_is_line_comment: bool,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct CommandState {
    pub(super) current_header: Option<String>,
    pub(super) case_label_colon_emitted: bool,
    pub(super) header_broken_before_comment: bool,
    pub(super) preprocessor_after_header: bool,
    pub(super) pending_block_word: Option<String>,
    pub(super) pre_brace_header_stack: Vec<String>,
    pub(super) previous_command_char: Option<char>,
    pub(super) previous_non_ws_char: Option<char>,
}

impl CommandState {
    pub(super) fn observe_text(&mut self, text: &str) {
        if let Some(ch) = text.chars().rev().find(|ch| !ch.is_whitespace()) {
            self.observe_char(ch);
        }
    }

    pub(super) fn observe_char(&mut self, ch: char) {
        self.previous_non_ws_char = Some(ch);
        self.previous_command_char = Some(ch);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum FormatterBraceType {
    Command,
    NonStatement,
    Extern,
    Namespace,
    Class,
    Interface,
    Struct,
    Union,
    Enum,
    Array,
    CompoundLiteral,
    Init,
    Definition,
    DeferArray,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct FormatterStackState {
    pub(super) paren_depth: usize,
    pub(super) paren_indent_spaces_stack: Vec<usize>,
    pub(super) inline_brace_call_paren_stack: Vec<bool>,
    pub(super) semicolonless_macro_call_paren_stack: Vec<Option<usize>>,
    pub(super) continuation_indent_spaces_stack: Vec<usize>,
    pub(super) continuation_indent_checkpoint_stack: Vec<usize>,
    brace_scope_depth_stack: Vec<ScopeDepth>,
    pub(super) brace_header_stack: Vec<Option<String>>,
    pub(super) brace_type_stack: Vec<FormatterBraceType>,
    pub(super) brace_extra_indent_stack: Vec<usize>,
    pub(super) brace_break_before_call_stack: Vec<bool>,
    pub(super) question_depth: usize,
    pub(super) last_closed_brace_header: Option<String>,
    pub(super) last_closed_brace_type: Option<FormatterBraceType>,
    pub(super) last_closed_brace_extra_indent: usize,
    pub(super) last_closed_brace_breaks_before_call: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ScopeDepth {
    parens: usize,
    questions: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ScopeRecovery {
    pub(super) parens: usize,
    pub(super) questions: usize,
}

impl FormatterStackState {
    pub(super) fn enter_paren(
        &mut self,
        indent_spaces: usize,
        inline_brace_call: bool,
        semicolonless_macro_call_indent: Option<usize>,
    ) {
        self.paren_depth += 1;
        self.paren_indent_spaces_stack.push(indent_spaces);
        self.inline_brace_call_paren_stack.push(inline_brace_call);
        self.semicolonless_macro_call_paren_stack
            .push(semicolonless_macro_call_indent);
        self.continuation_indent_checkpoint_stack
            .push(self.continuation_indent_spaces_stack.len());
    }

    pub(super) fn exit_paren(&mut self) {
        self.paren_depth = self.paren_depth.saturating_sub(1);
        self.paren_indent_spaces_stack.pop();
        self.inline_brace_call_paren_stack.pop();
        self.semicolonless_macro_call_paren_stack.pop();
        self.restore_continuation_checkpoint();
    }

    pub(super) fn current_paren_indent_spaces(&self) -> Option<usize> {
        self.paren_indent_spaces_stack.last().copied()
    }

    pub(super) fn current_paren_is_inline_brace_call(&self) -> bool {
        self.inline_brace_call_paren_stack
            .last()
            .copied()
            .unwrap_or(false)
    }

    pub(super) fn current_paren_semicolonless_macro_call_indent(&self) -> Option<usize> {
        self.semicolonless_macro_call_paren_stack
            .last()
            .copied()
            .flatten()
    }

    pub(super) fn register_continuation_indent_spaces(&mut self, spaces: usize) {
        let spaces = self
            .current_continuation_indent_spaces()
            .map_or(spaces, |previous| spaces.max(previous));
        self.continuation_indent_spaces_stack.push(spaces);
    }

    pub(super) fn push_continuation_indent_spaces_raw(&mut self, spaces: usize) {
        self.continuation_indent_spaces_stack.push(spaces);
    }

    pub(super) fn restore_continuation_checkpoint(&mut self) {
        let target_size = self.continuation_indent_checkpoint_stack.pop().unwrap_or(0);
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(super) fn clear_continuation_indents(&mut self) {
        let target_size = self
            .continuation_indent_checkpoint_stack
            .last()
            .copied()
            .unwrap_or(0);
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(super) fn trim_to_current_statement_continuation(&mut self) {
        let target_size = self
            .continuation_indent_checkpoint_stack
            .last()
            .map_or(0, |checkpoint| checkpoint + 1)
            .min(self.continuation_indent_spaces_stack.len());
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(super) fn current_continuation_indent_spaces(&self) -> Option<usize> {
        self.continuation_indent_spaces_stack.last().copied()
    }

    pub(super) fn has_active_brace_scope(&self) -> bool {
        !self.brace_scope_depth_stack.is_empty()
    }

    pub(super) fn current_brace_paren_depth(&self) -> Option<usize> {
        self.brace_scope_depth_stack
            .last()
            .map(|depth| depth.parens)
    }

    pub(super) fn has_question_in_current_brace(&self) -> bool {
        let baseline = self
            .brace_scope_depth_stack
            .last()
            .map_or(0, |depth| depth.questions);
        self.question_depth > baseline
    }

    pub(super) fn enter_brace(
        &mut self,
        header: Option<String>,
        brace_type: FormatterBraceType,
        extra_indent: usize,
    ) {
        self.brace_scope_depth_stack.push(ScopeDepth {
            parens: self.paren_depth,
            questions: self.question_depth,
        });
        self.brace_header_stack.push(header);
        self.brace_type_stack.push(brace_type);
        self.brace_extra_indent_stack.push(extra_indent);
        self.brace_break_before_call_stack.push(false);
    }

    pub(super) fn mark_current_brace_break_before_call(&mut self) {
        if let Some(breaks_before_call) = self.brace_break_before_call_stack.last_mut() {
            *breaks_before_call = true;
        }
    }

    pub(super) fn exit_brace(&mut self) -> ScopeRecovery {
        self.last_closed_brace_header = self.brace_header_stack.pop().flatten();
        self.last_closed_brace_type = self.brace_type_stack.pop();
        self.last_closed_brace_extra_indent = self.brace_extra_indent_stack.pop().unwrap_or(0);
        self.last_closed_brace_breaks_before_call =
            self.brace_break_before_call_stack.pop().unwrap_or(false);
        let mut recovery = ScopeRecovery {
            parens: 0,
            questions: 0,
        };
        let depth = self.brace_scope_depth_stack.pop().unwrap_or(ScopeDepth {
            parens: 0,
            questions: 0,
        });
        if self.paren_depth > depth.parens {
            recovery.parens = self.paren_depth - depth.parens;
            let target_cont = self
                .continuation_indent_checkpoint_stack
                .get(depth.parens)
                .copied()
                .unwrap_or(0);
            self.paren_depth = depth.parens;
            self.paren_indent_spaces_stack.truncate(depth.parens);
            self.inline_brace_call_paren_stack.truncate(depth.parens);
            self.semicolonless_macro_call_paren_stack
                .truncate(depth.parens);
            self.continuation_indent_checkpoint_stack
                .truncate(depth.parens);
            self.continuation_indent_spaces_stack.truncate(target_cont);
        }
        if self.question_depth > depth.questions {
            recovery.questions = self.question_depth - depth.questions;
            self.question_depth = depth.questions;
        }
        recovery
    }

    pub(super) fn enter_question(&mut self) {
        self.question_depth += 1;
    }

    pub(super) fn exit_question(&mut self) {
        self.question_depth = self.question_depth.saturating_sub(1);
    }

    pub(super) fn truncate_questions_to_brace_scope(&mut self) -> usize {
        let target = self
            .brace_scope_depth_stack
            .last()
            .map_or(0, |depth| depth.questions);
        let removed = self.question_depth.saturating_sub(target);
        self.question_depth = self.question_depth.min(target);
        removed
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct FormatterLineState {
    pub(super) passed_semicolon: bool,
    pub(super) passed_colon: bool,
    pub(super) is_multi_statement_line: bool,
    pub(super) is_one_line_block: bool,
    pub(super) column1_line_comment: bool,
    pub(super) has_literal_quote: bool,
    pub(super) indent_off_follows_code: bool,
    pub(super) operator_padding_disabled: bool,
    pub(super) in_class_initializer: bool,
    pub(super) trailing_comment_columns: Vec<usize>,
    pub(super) has_nested_designated_init_brace: bool,
    pub(super) ternary_colon: bool,
    pub(super) template_angle_depth: usize,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct RunInState {
    pub(super) current_run_in_indent: Option<usize>,
    pub(super) adjuster_observed_line_count: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum TemplateAngle {
    None,
    Open,
    Close(usize),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PreviousToken {
    None,
    Word,
    Literal,
    Operator,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
    Other,
}

impl PreviousToken {
    pub(super) fn needs_space_before_word(self) -> bool {
        matches!(
            self,
            Self::Word | Self::Literal | Self::CloseParen | Self::CloseBracket
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brace_exit_truncates_unclosed_question_state() {
        let mut state = FormatterStackState::default();
        state.enter_brace(None, FormatterBraceType::Command, 0);
        state.enter_question();

        let recovery = state.exit_brace();

        assert_eq!(recovery.questions, 1);
        assert_eq!(state.question_depth, 0);
    }

    #[test]
    fn brace_exit_truncates_all_unclosed_paren_state() {
        let mut state = FormatterStackState::default();
        state.enter_brace(None, FormatterBraceType::Command, 0);
        state.enter_paren(8, true, Some(4));

        let recovery = state.exit_brace();

        assert_eq!(recovery.parens, 1);
        assert_eq!(state.paren_depth, 0);
        assert!(state.paren_indent_spaces_stack.is_empty());
        assert!(state.inline_brace_call_paren_stack.is_empty());
        assert!(state.semicolonless_macro_call_paren_stack.is_empty());
        assert!(state.continuation_indent_checkpoint_stack.is_empty());
        assert!(!state.current_paren_is_inline_brace_call());
        assert_eq!(state.current_paren_semicolonless_macro_call_indent(), None);
    }
}
