//! State carried across a *INDENT-OFF* ... *INDENT-ON* region.

use crate::formatter::braces::compound_literals::CompoundLiteralState;
use crate::formatter::constructs::objective_c::ObjectiveCLineState;
use crate::formatter::constructs::switch_cases::SwitchCaseLayoutState;
use crate::formatter::constructs::template_declarations::TemplateDeclarationState;
use crate::formatter::continuation::ContinuationIndentState;
use crate::formatter::engine::{FormatEngine, TokenPushContext};
use crate::formatter::lexer::Token;
use crate::formatter::output::line_adjust::LineAdjuster;
use crate::formatter::output::member_spacing::MemberSpacingBoundary;
use crate::formatter::preprocessor::{PreprocessorBranchState, PreprocessorSplitElseState};
use crate::formatter::state::frame::FrameStack;
use crate::formatter::state::indentation::IndentationState;
use crate::formatter::state::{CommandState, LineState, NestingState, PreviousToken, RunInState};
use crate::formatter::syntax::SyntaxRoles;
use crate::formatter::tokens::literals::LiteralLineState;
use std::collections::VecDeque;

/// The subset of engine state that formatting after a disabled region depends on.
#[derive(Debug, Clone)]
struct DisabledFormattingSnapshot {
    indentation: IndentationState,
    command_state: CommandState,
    nesting: NestingState,
    frame_stack: FrameStack,
    line_state: LineState,
    run_in_state: RunInState,
    branch_stack: Vec<PreprocessorBranchState>,
    indented_block_stack: Vec<bool>,
    indentable_blocks: VecDeque<bool>,
    syntax_roles: SyntaxRoles,
    input_source_indent: usize,
    has_next_meaningful_token: bool,
    next_token_is_line_comment: bool,
    previous_pre_adjust_line: Option<String>,
    pending_member_spacing: Option<MemberSpacingBoundary>,
    previous: PreviousToken,
    previous_was_newline: bool,
    literal_line: LiteralLineState,
    continuation_indent: ContinuationIndentState,
    objc: ObjectiveCLineState,
    switch_case_layout: SwitchCaseLayoutState,
    in_class_base_clause: bool,
    split_class_export_pending_base: bool,
    split_else: PreprocessorSplitElseState,
    template_declaration: TemplateDeclarationState,
    else_if_break_depths: Vec<usize>,
    compound_literal: CompoundLiteralState,
    pending_braceless_block_bias: Option<usize>,
    inline_nested_header_braceless_bias: Option<usize>,
    line_adjuster: LineAdjuster,
}

impl DisabledFormattingSnapshot {
    fn capture(engine: &FormatEngine<'_>) -> Self {
        Self {
            indentation: engine.indentation.clone(),
            command_state: engine.command_state.clone(),
            nesting: engine.nesting.clone(),
            frame_stack: engine.frame_stack.clone(),
            line_state: engine.line_state.clone(),
            run_in_state: engine.run_in_state.clone(),
            branch_stack: engine.preprocessor.branch_stack.clone(),
            indented_block_stack: engine.preprocessor.indented_block_stack.clone(),
            indentable_blocks: engine.preprocessor.indentable_blocks.clone(),
            syntax_roles: engine.syntax_roles.clone(),
            input_source_indent: engine.token_input.input_source_indent,
            has_next_meaningful_token: engine.token_input.has_next_meaningful_token,
            next_token_is_line_comment: engine.token_input.next_token_is_line_comment,
            previous_pre_adjust_line: engine.previous_pre_adjust_line.clone(),
            pending_member_spacing: engine.pending_member_spacing,
            previous: engine.previous,
            previous_was_newline: engine.previous_was_newline,
            literal_line: engine.literal_line.clone(),
            continuation_indent: engine.continuation_indent.clone(),
            objc: engine.objc.clone(),
            switch_case_layout: engine.switch_case_layout.clone(),
            in_class_base_clause: engine.in_class_base_clause,
            split_class_export_pending_base: engine.split_class_export_pending_base,
            split_else: engine.preprocessor.split_else,
            template_declaration: engine.template_declaration,
            else_if_break_depths: engine.else_if_break_depths.clone(),
            compound_literal: engine.compound_literal.clone(),
            pending_braceless_block_bias: engine.pending_braceless_block_bias,
            inline_nested_header_braceless_bias: engine.inline_nested_header_braceless_bias,
            line_adjuster: engine.line_adjuster.clone(),
        }
    }

    fn apply_to(&self, engine: &mut FormatEngine<'_>) {
        engine.indentation = self.indentation.clone();
        engine.command_state = self.command_state.clone();
        engine.nesting = self.nesting.clone();
        engine.frame_stack = self.frame_stack.clone();
        engine.line_state = self.line_state.clone();
        engine.run_in_state = self.run_in_state.clone();
        engine.preprocessor.branch_stack = self.branch_stack.clone();
        engine.preprocessor.indented_block_stack = self.indented_block_stack.clone();
        engine.preprocessor.indentable_blocks = self.indentable_blocks.clone();
        engine.syntax_roles = self.syntax_roles.clone();
        engine.token_input.input_source_indent = self.input_source_indent;
        engine.token_input.has_next_meaningful_token = self.has_next_meaningful_token;
        engine.token_input.next_token_is_line_comment = self.next_token_is_line_comment;
        engine.previous_pre_adjust_line = self.previous_pre_adjust_line.clone();
        engine.pending_member_spacing = self.pending_member_spacing;
        engine.previous = self.previous;
        engine.previous_was_newline = self.previous_was_newline;
        engine.literal_line = self.literal_line.clone();
        engine.continuation_indent = self.continuation_indent.clone();
        engine.objc = self.objc.clone();
        engine.switch_case_layout = self.switch_case_layout.clone();
        engine.in_class_base_clause = self.in_class_base_clause;
        engine.split_class_export_pending_base = self.split_class_export_pending_base;
        engine.preprocessor.split_else = self.split_else;
        engine.template_declaration = self.template_declaration;
        engine.else_if_break_depths = self.else_if_break_depths.clone();
        engine.compound_literal = self.compound_literal.clone();
        engine.pending_braceless_block_bias = self.pending_braceless_block_bias;
        engine.inline_nested_header_braceless_bias = self.inline_nested_header_braceless_bias;
        engine.line_adjuster = self.line_adjuster.clone();
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
