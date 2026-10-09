//! State carried across a *INDENT-OFF* ... *INDENT-ON* region.

use crate::formatter::engine::{FormatEngine, LayoutState, TokenPushContext};
use crate::formatter::lexer::Token;
use crate::formatter::preprocessor::{PreprocessorBranchState, PreprocessorSplitElseState};
use crate::formatter::syntax::SyntaxRoles;
use std::collections::VecDeque;

/// The subset of engine state that formatting after a disabled region depends on.
#[derive(Debug, Clone)]
struct DisabledFormattingSnapshot {
    layout: LayoutState,
    branch_stack: Vec<PreprocessorBranchState>,
    indented_block_stack: Vec<bool>,
    indentable_blocks: VecDeque<bool>,
    syntax_roles: SyntaxRoles,
    input_source_indent: usize,
    has_next_meaningful_token: bool,
    next_token_is_line_comment: bool,
    previous_was_newline: bool,
    split_else: PreprocessorSplitElseState,
}

impl DisabledFormattingSnapshot {
    fn capture(engine: &FormatEngine<'_>) -> Self {
        Self {
            layout: engine.layout.clone(),
            branch_stack: engine.preprocessor.branch_stack.clone(),
            indented_block_stack: engine.preprocessor.indented_block_stack.clone(),
            indentable_blocks: engine.preprocessor.indentable_blocks.clone(),
            syntax_roles: engine.syntax_roles.clone(),
            input_source_indent: engine.token_input.input_source_indent,
            has_next_meaningful_token: engine.token_input.has_next_meaningful_token,
            next_token_is_line_comment: engine.token_input.next_token_is_line_comment,
            previous_was_newline: engine.previous_was_newline,
            split_else: engine.preprocessor.split_else,
        }
    }

    fn apply_to(&self, engine: &mut FormatEngine<'_>) {
        engine.layout = self.layout.clone();
        engine.preprocessor.branch_stack = self.branch_stack.clone();
        engine.preprocessor.indented_block_stack = self.indented_block_stack.clone();
        engine.preprocessor.indentable_blocks = self.indentable_blocks.clone();
        engine.syntax_roles = self.syntax_roles.clone();
        engine.token_input.input_source_indent = self.input_source_indent;
        engine.token_input.has_next_meaningful_token = self.has_next_meaningful_token;
        engine.token_input.next_token_is_line_comment = self.next_token_is_line_comment;
        engine.previous_was_newline = self.previous_was_newline;
        engine.preprocessor.split_else = self.split_else;
    }
}

/// Runs the tokens of a disabled region through a shadow engine so the state
/// after the region reflects its contents; the captured snapshot is what the
/// main engine resumes from.
pub(crate) struct DisabledFormattingState<'a> {
    shadow: Box<FormatEngine<'a>>,
}

impl<'a> DisabledFormattingState<'a> {
    pub(crate) fn capture(engine: &FormatEngine<'a>) -> Self {
        let mut shadow = FormatEngine::new(engine.options);
        DisabledFormattingSnapshot::capture(engine).apply_to(&mut shadow);
        Self {
            shadow: Box::new(shadow),
        }
    }

    pub(crate) fn restore(self, engine: &mut FormatEngine<'a>) {
        DisabledFormattingSnapshot::capture(&self.shadow).apply_to(engine);
    }

    pub(crate) fn push_token(&mut self, token: &Token, context: TokenPushContext<'_>) {
        self.shadow.push_token(token, context);
    }
}
