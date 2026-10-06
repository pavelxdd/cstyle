//! Engine state shared by several formatting concerns.

pub(crate) mod current_line;
pub(crate) mod frame;
pub(crate) mod indentation;
pub(crate) mod next_line;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct TokenInputState {
    pub(crate) previous_input_was_adjacent: bool,
    pub(crate) previous_input_whitespace: Option<std::borrow::Cow<'static, str>>,
    pub(crate) next_input_whitespace: Option<std::borrow::Cow<'static, str>>,
    pub(crate) token_begins_source_line: bool,
    pub(crate) token_source_column: usize,
    pub(crate) token_source_line_indent: usize,
    pub(crate) token_line_opens_with_brace: bool,
    pub(crate) token_followed_by_final_line_comment: bool,
    pub(crate) token_followed_by_line_comment_on_line: bool,
    pub(crate) input_source_indent: usize,
    pub(crate) has_next_meaningful_token: bool,
    pub(crate) next_token_is_line_comment: bool,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct CommandState {
    pub(crate) current_header: Option<String>,
    pub(crate) case_label_colon_emitted: bool,
    pub(crate) header_broken_before_comment: bool,
    pub(crate) preprocessor_after_header: bool,
    pub(crate) pending_block_word: Option<String>,
    pub(crate) pre_brace_header_stack: Vec<String>,
    pub(crate) previous_command_char: Option<char>,
    pub(crate) previous_non_ws_char: Option<char>,
}

impl CommandState {
    pub(crate) fn observe_text(&mut self, text: &str) {
        if let Some(ch) = text.chars().rev().find(|ch| !ch.is_whitespace()) {
            self.observe_char(ch);
        }
    }

    pub(crate) fn observe_char(&mut self, ch: char) {
        self.previous_non_ws_char = Some(ch);
        self.previous_command_char = Some(ch);
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BraceType {
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
    Initializer,
    Definition,
    DeferArray,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct NestingState {
    pub(crate) paren_depth: usize,
    paren_indent_spaces_stack: Vec<usize>,
    inline_brace_call_paren_stack: Vec<bool>,
    semicolonless_macro_call_paren_stack: Vec<Option<usize>>,
    continuation_indent_spaces_stack: Vec<usize>,
    continuation_indent_checkpoint_stack: Vec<usize>,
    brace_scope_depth_stack: Vec<ScopeDepth>,
    pub(crate) brace_header_stack: Vec<Option<String>>,
    pub(crate) brace_type_stack: Vec<BraceType>,
    brace_extra_indent_stack: Vec<usize>,
    brace_break_before_call_stack: Vec<bool>,
    pub(crate) question_depth: usize,
    pub(crate) last_closed_brace_header: Option<String>,
    pub(crate) last_closed_brace_type: Option<BraceType>,
    pub(crate) last_closed_brace_extra_indent: usize,
    pub(crate) last_closed_brace_breaks_before_call: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ScopeDepth {
    parens: usize,
    questions: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct ScopeRecovery {
    pub(crate) parens: usize,
    pub(crate) questions: usize,
}

impl NestingState {
    pub(crate) fn enter_paren(
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

    pub(crate) fn exit_paren(&mut self) {
        self.paren_depth = self.paren_depth.saturating_sub(1);
        self.paren_indent_spaces_stack.pop();
        self.inline_brace_call_paren_stack.pop();
        self.semicolonless_macro_call_paren_stack.pop();
        self.restore_continuation_checkpoint();
    }

    pub(crate) fn current_paren_indent_spaces(&self) -> Option<usize> {
        self.paren_indent_spaces_stack.last().copied()
    }

    pub(crate) fn current_paren_is_inline_brace_call(&self) -> bool {
        self.inline_brace_call_paren_stack
            .last()
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn current_paren_semicolonless_macro_call_indent(&self) -> Option<usize> {
        self.semicolonless_macro_call_paren_stack
            .last()
            .copied()
            .flatten()
    }

    pub(crate) fn register_continuation_indent_spaces(&mut self, spaces: usize) {
        let spaces = self
            .current_continuation_indent_spaces()
            .map_or(spaces, |previous| spaces.max(previous));
        self.continuation_indent_spaces_stack.push(spaces);
    }

    pub(crate) fn push_continuation_indent_spaces_raw(&mut self, spaces: usize) {
        self.continuation_indent_spaces_stack.push(spaces);
    }

    pub(crate) fn restore_continuation_checkpoint(&mut self) {
        let target_size = self.continuation_indent_checkpoint_stack.pop().unwrap_or(0);
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(crate) fn clear_continuation_indents(&mut self) {
        let target_size = self
            .continuation_indent_checkpoint_stack
            .last()
            .copied()
            .unwrap_or(0);
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(crate) fn trim_to_current_statement_continuation(&mut self) {
        let target_size = self
            .continuation_indent_checkpoint_stack
            .last()
            .map_or(0, |checkpoint| checkpoint + 1)
            .min(self.continuation_indent_spaces_stack.len());
        self.continuation_indent_spaces_stack.truncate(target_size);
    }

    pub(crate) fn current_continuation_indent_spaces(&self) -> Option<usize> {
        self.continuation_indent_spaces_stack.last().copied()
    }

    pub(crate) fn has_active_brace_scope(&self) -> bool {
        !self.brace_scope_depth_stack.is_empty()
    }

    pub(crate) fn current_brace_paren_depth(&self) -> Option<usize> {
        self.brace_scope_depth_stack
            .last()
            .map(|depth| depth.parens)
    }

    pub(crate) fn has_question_in_current_brace(&self) -> bool {
        let baseline = self
            .brace_scope_depth_stack
            .last()
            .map_or(0, |depth| depth.questions);
        self.question_depth > baseline
    }

    pub(crate) fn enter_brace(
        &mut self,
        header: Option<String>,
        brace_type: BraceType,
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

    pub(crate) fn mark_current_brace_break_before_call(&mut self) {
        if let Some(breaks_before_call) = self.brace_break_before_call_stack.last_mut() {
            *breaks_before_call = true;
        }
    }

    pub(crate) fn exit_brace(&mut self) -> ScopeRecovery {
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

    pub(crate) fn enter_question(&mut self) {
        self.question_depth += 1;
    }

    pub(crate) fn exit_question(&mut self) {
        self.question_depth = self.question_depth.saturating_sub(1);
    }

    pub(crate) fn truncate_questions_to_brace_scope(&mut self) -> usize {
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
pub(crate) struct LineState {
    pub(crate) passed_semicolon: bool,
    pub(crate) passed_colon: bool,
    pub(crate) is_multi_statement_line: bool,
    pub(crate) is_one_line_block: bool,
    pub(crate) column1_line_comment: bool,
    pub(crate) has_literal_quote: bool,
    pub(crate) indent_off_follows_code: bool,
    pub(crate) operator_padding_disabled: bool,
    pub(crate) in_class_initializer: bool,
    pub(crate) trailing_comment_columns: std::collections::VecDeque<usize>,
    pub(crate) has_nested_designated_init_brace: bool,
    pub(crate) ternary_colon: bool,
    pub(crate) template_angle_depth: usize,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct RunInState {
    pub(crate) current_run_in_indent: Option<usize>,
    pub(crate) adjuster_observed_line_count: usize,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PreviousToken {
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
    pub(crate) fn needs_space_before_word(self) -> bool {
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
        let mut state = NestingState::default();
        state.enter_brace(None, BraceType::Command, 0);
        state.enter_question();

        let recovery = state.exit_brace();

        assert_eq!(recovery.questions, 1);
        assert_eq!(state.question_depth, 0);
    }

    #[test]
    fn brace_exit_truncates_all_unclosed_paren_state() {
        let mut state = NestingState::default();
        state.enter_brace(None, BraceType::Command, 0);
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
