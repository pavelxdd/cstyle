use self::indentation::{IndentationState, LineKind};
use self::language::{
    is_macro_like_word, is_numeric_variable_word, is_pointer_type_word, is_type_like_pointer_word,
};
use crate::config::{BraceStyle, FormatOptions, PointerAlign, ReferenceAlign};
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};
use std::collections::HashSet;
mod assembly;
mod backslash_bodies;
mod blank_lines;
mod block_spacing;
mod brace_classification;
mod brace_frames;
mod brace_postprocess;
mod buffer;
mod call_arguments;
mod class_declarations;
mod closing_braces;
mod columns;
mod comments;
mod compound_literals;
mod constructor_initializers;
mod continuation;
mod current_line;
mod disabled_formatting;
mod entry;
mod frame;
mod headers;
mod indentation;
mod initializer_braces;
mod labels;
mod language;
mod line_adjust;
mod line_scan;
mod literals;
mod macro_definitions;
mod macro_invocations;
mod max_length;
mod member_spacing;
mod next_line;
mod objective_c;
mod opening_braces;
mod operator_chains;
mod operators;
mod output;
mod pointers;
mod preprocessor;
mod raw_strings;
mod return_types;
mod rewrite;
mod source_indent;
mod state;
mod swig;
mod switch_cases;
mod symbols;
mod syntax;
mod tabs;
mod template_declarations;
mod token;
mod typedefs;
mod words;

use backslash_bodies::BackslashBodyState;
use block_spacing::BlockSpacingState;

use class_declarations::is_split_export_head;

use comments::trailing_comment_columns;
use current_line::CurrentLine;
use disabled_formatting::DisabledFormattingState;
use frame::FrameStack;

use line_scan::{
    line_ends_with_comment, trailing_comment_split_limit, unmatched_open_paren_column,
};

use max_length::MaxLengthLineState;
use member_spacing::MemberSpacingBoundary;

use preprocessor::preprocessor_block_indentability;
use rewrite::{
    add_cross_line_statement_braces, following_operator_after_next_word, previous_non_whitespace,
    remove_cross_line_statement_braces,
};
use source_indent::source_indented_macro_row;
use state::{
    CommandState, ContinuationIndent, FormatterLineState, FormatterStackState, InlineArrayFrame,
    InlineArrayState, PreviousToken, RunInState, TemplateAngle, TokenInputState,
};
use swig::SwigState;
use switch_cases::SwitchCaseLayoutState;

use syntax::{OperatorRole, SyntaxRoles, classify_syntax, template_angle_role};

use template_declarations::TemplateDeclarationState;
use token::{
    CommentKind, Token, TokenLine, TokenLineCursor, next_non_layout_token_index,
    next_non_whitespace, token_char_len, token_text,
};

pub(crate) use entry::format_c;

#[derive(Clone, Copy)]
struct TokenPushContext<'a> {
    next: Option<&'a Token>,
    next_is_adjacent: bool,
    following_operator: Option<&'a str>,
    template_angle: TemplateAngle,
    token_index: usize,
    starts_initializer_designator: bool,
    inferred_definition_brace: bool,
    following_closing_braces: usize,
}

struct LineSourceColumns {
    prefix: Vec<usize>,
    non_ws_prefix: Vec<usize>,
    first_non_ws: Option<usize>,
    first_non_ws_is_brace: bool,
    leading_indent: usize,
}

struct FormatEngine<'a> {
    options: &'a FormatOptions,
    output: buffer::OutputBuffer,
    previous_pre_adjust_line: Option<String>,
    pending_member_spacing: Option<MemberSpacingBoundary>,
    current: CurrentLine,
    line_brace_match_start: usize,
    line_brace_matches: Vec<Option<usize>>,
    state: IndentationState,
    previous: PreviousToken,
    previous_was_newline: bool,
    previous_was_template_close: bool,
    template_close_before_current: bool,
    newline_breaks_statement: bool,
    preserve_block_spacing_comment_blank: bool,
    next_line: next_line::NextLineState,
    template_declaration: TemplateDeclarationState,
    multi_declarator_indent_spaces: Option<usize>,
    block_spacing: BlockSpacingState,
    run_in_comment_brace_lines: Vec<usize>,
    source_run_in_brace_lines: Vec<usize>,
    formatting_disabled: bool,
    disabled_formatting: Option<DisabledFormattingState<'a>>,
    current_is_preindented: bool,
    literal_line: literals::LiteralLineState,
    unmatched_closing_brace_recovery: bool,
    preserve_run_in_join_space: bool,
    one_line_block_mode: bool,
    inline_array: InlineArrayState,
    continuation_indent: continuation::ContinuationIndentState,
    max_length_line: MaxLengthLineState,
    objc: objective_c::ObjectiveCLineState,
    switch_case_layout: SwitchCaseLayoutState,
    in_class_base_clause: bool,
    split_class_export_pending_base: bool,
    else_if_break_depths: Vec<usize>,
    compound_literal: compound_literals::CompoundLiteralState,
    pending_braceless_block_bias: Option<usize>,
    inline_nested_header_braceless_bias: Option<usize>,
    skip_adjacent_pointer_operators: usize,
    skip_next_attached_comment: bool,
    line_comment_starts_reordered_brace_body: bool,
    reordered_brace_line_comment_gap: Option<String>,
    backslash_body: BackslashBodyState,
    swig: SwigState,
    may_have_class_base_access: bool,
    next_comment_ends_line: bool,
    space_after_cast: bool,
    pad_close_paren_pending: bool,
    header_paren: headers::HeaderParenState,
    block_comment_close_paren_ends_declaration: bool,
    previous_block_comment_close_paren_ended_declaration: bool,
    current_line_has_class_initializer_colon: bool,
    token_input: TokenInputState,
    pointer_run: pointers::PointerRunState,
    command_state: CommandState,
    stack_state: FormatterStackState,
    frame_stack: FrameStack,
    line_state: FormatterLineState,
    run_in_state: RunInState,
    preprocessor: preprocessor::PreprocessorState,
    access_modified_braces: HashSet<usize>,
    syntax_roles: SyntaxRoles,
    pending_extern: bool,
    cpp_extern_c_brace: u8,
    line_adjuster: line_adjust::LineAdjuster,
}

impl<'a> FormatEngine<'a> {
    fn new(options: &'a FormatOptions) -> Self {
        Self {
            options,
            output: buffer::OutputBuffer::default(),
            previous_pre_adjust_line: None,
            pending_member_spacing: None,
            current: CurrentLine::default(),
            line_brace_match_start: 0,
            line_brace_matches: Vec::new(),
            state: IndentationState::default(),
            previous: PreviousToken::None,
            previous_was_newline: false,
            previous_was_template_close: false,
            template_close_before_current: false,
            newline_breaks_statement: false,
            preserve_block_spacing_comment_blank: false,
            next_line: next_line::NextLineState::default(),
            template_declaration: TemplateDeclarationState::default(),
            multi_declarator_indent_spaces: None,
            block_spacing: BlockSpacingState::default(),
            run_in_comment_brace_lines: Vec::new(),
            source_run_in_brace_lines: Vec::new(),
            formatting_disabled: false,
            disabled_formatting: None,
            current_is_preindented: false,
            literal_line: literals::LiteralLineState::default(),
            unmatched_closing_brace_recovery: false,
            preserve_run_in_join_space: false,
            one_line_block_mode: false,
            inline_array: InlineArrayState::default(),
            continuation_indent: continuation::ContinuationIndentState::default(),
            max_length_line: MaxLengthLineState::default(),
            objc: objective_c::ObjectiveCLineState::default(),
            switch_case_layout: SwitchCaseLayoutState::default(),
            in_class_base_clause: false,
            split_class_export_pending_base: false,
            else_if_break_depths: Vec::new(),
            compound_literal: compound_literals::CompoundLiteralState::default(),
            pending_braceless_block_bias: None,
            inline_nested_header_braceless_bias: None,
            skip_adjacent_pointer_operators: 0,
            skip_next_attached_comment: false,
            line_comment_starts_reordered_brace_body: false,
            reordered_brace_line_comment_gap: None,
            backslash_body: BackslashBodyState::default(),
            swig: SwigState::default(),
            may_have_class_base_access: true,
            next_comment_ends_line: false,
            space_after_cast: false,
            pad_close_paren_pending: false,
            header_paren: headers::HeaderParenState::default(),
            block_comment_close_paren_ends_declaration: false,
            previous_block_comment_close_paren_ended_declaration: false,
            current_line_has_class_initializer_colon: false,
            token_input: TokenInputState::default(),
            pointer_run: pointers::PointerRunState::default(),
            command_state: CommandState::default(),
            stack_state: FormatterStackState::default(),
            frame_stack: FrameStack::default(),
            line_state: FormatterLineState::default(),
            run_in_state: RunInState::default(),
            preprocessor: preprocessor::PreprocessorState::default(),
            access_modified_braces: HashSet::new(),
            syntax_roles: SyntaxRoles::new(0),
            pending_extern: false,
            cpp_extern_c_brace: 0,
            line_adjuster: line_adjust::LineAdjuster::new(options),
        }
    }

    fn current_char_len(&self) -> usize {
        self.current.char_len()
    }

    fn current_is_blank(&self) -> bool {
        self.current.is_blank()
    }

    fn current_visual_width(&self) -> usize {
        self.current.visual_width(self.options.tab_width)
    }

    fn current_visual_width_from(&self, start_column: usize) -> usize {
        self.current
            .visual_width_from(start_column, self.options.tab_width)
    }

    fn current_last_open_brace(&self) -> Option<usize> {
        self.current.last_open_brace()
    }

    fn current_trailing_comment_split_limit(&self) -> usize {
        self.current.trailing_comment_split_limit()
    }

    fn take_current(&mut self) -> String {
        self.current.take()
    }

    fn fill_line_brace_matches(&mut self, tokens: &[Token], line_start: usize, line_end: usize) {
        self.line_brace_match_start = line_start;
        self.line_brace_matches.clear();
        self.line_brace_matches.resize(line_end - line_start, None);
        let mut open_stack: Vec<usize> = Vec::new();
        for (offset, token) in tokens[line_start..line_end].iter().enumerate() {
            match token {
                Token::Symbol('{') => open_stack.push(offset),
                Token::Symbol('}') => {
                    if let Some(open) = open_stack.pop() {
                        self.line_brace_matches[open] = Some(line_start + offset);
                    }
                }
                Token::Newline => open_stack.clear(),
                _ => {}
            }
            if matches!(
                token,
                Token::StringLiteral(text)
                    | Token::CharLiteral(text)
                    | Token::Comment(_, text)
                    | Token::RawLine(text)
                    if text.contains('\n')
            ) || matches!(token, Token::Preprocessor(preprocessor) if preprocessor.text.contains('\n'))
            {
                open_stack.clear();
            }
        }
    }

    fn matching_brace_on_current_line(&self, open_index: usize) -> Option<usize> {
        let offset = open_index.checked_sub(self.line_brace_match_start)?;
        self.line_brace_matches.get(offset).copied().flatten()
    }

    fn clear_current(&mut self) {
        self.current.clear();
    }

    fn reset_after_finished_line(&mut self) {
        self.clear_current();
        self.current_is_preindented = false;
        self.literal_line.is_multiline_literal = false;
        self.literal_line.multiline_literal_end = None;
        self.literal_line.unterminated_raw_literal = false;
        self.current_line_has_class_initializer_colon = false;
        self.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    fn line_source_columns(&self, line_tokens: &[Token]) -> LineSourceColumns {
        let tab_width = self.options.tab_width.max(1);
        let mut prefix = Vec::with_capacity(line_tokens.len() + 1);
        let mut non_ws_prefix = Vec::with_capacity(line_tokens.len() + 1);
        let mut column = 0usize;
        let mut non_ws = 0usize;
        let mut first_non_ws = None;
        let mut first_non_ws_is_brace = false;
        let mut leading_indent = 0usize;
        prefix.push(0);
        non_ws_prefix.push(0);
        for (offset, token) in line_tokens.iter().enumerate() {
            match token {
                Token::Newline => {}
                Token::Whitespace(ws) => {
                    for ch in ws.chars() {
                        if ch == '\t' {
                            column += tab_width - (column % tab_width);
                        } else {
                            column += 1;
                        }
                    }
                }
                other => {
                    if first_non_ws.is_none() {
                        first_non_ws = Some(offset);
                        first_non_ws_is_brace = matches!(other, Token::Symbol('{'));
                        leading_indent = column;
                    }
                    non_ws += 1;
                    column += token_char_len(other);
                }
            }
            prefix.push(column);
            non_ws_prefix.push(non_ws);
        }
        LineSourceColumns {
            prefix,
            non_ws_prefix,
            first_non_ws,
            first_non_ws_is_brace,
            leading_indent,
        }
    }

    fn format_into(mut self, tokens: &[Token]) -> Self {
        let rewritten_tokens;
        let tokens = if self.options.remove_braces {
            rewritten_tokens = remove_cross_line_statement_braces(tokens);
            rewritten_tokens.as_slice()
        } else {
            tokens
        };
        let added_brace_tokens;
        let tokens = if self.options.add_braces && !self.options.add_one_line_braces {
            let attach_added_braces = matches!(
                self.options.brace_style,
                BraceStyle::None
                    | BraceStyle::Attach
                    | BraceStyle::OneTrueBrace
                    | BraceStyle::WebKit
                    | BraceStyle::Ratliff
                    | BraceStyle::Lisp
            );
            added_brace_tokens = add_cross_line_statement_braces(tokens, attach_added_braces);
            added_brace_tokens.as_slice()
        } else {
            tokens
        };
        self.syntax_roles = classify_syntax(tokens);
        self.preprocessor.indentable_blocks = preprocessor_block_indentability(tokens);
        self.access_modified_braces = syntax::access_modified_brace_indices(tokens);
        self.inline_array.nested_brace_arrays = syntax::nested_brace_array_indices(tokens);
        let mut cursor = TokenLineCursor::new(tokens);
        while let Some(line) = cursor.next_line() {
            self.format_line(tokens, line);
        }
        self.finish_line();
        self
    }

    fn newline_following_token_breaks(next: Option<&Token>) -> bool {
        match next {
            None | Some(Token::Symbol('{') | Token::Symbol('}')) => false,
            Some(Token::Word(word)) if word == "else" => false,
            Some(_) => true,
        }
    }

    fn format_line(&mut self, tokens: &[Token], line: TokenLine) {
        self.observe_input_line(&tokens[line.start..line.end]);
        if !self.formatting_disabled && self.try_push_case_line_marker(tokens, line.start, line.end)
        {
            return;
        }
        if !self.formatting_disabled
            && self.try_push_generated_case_compact_action_line(tokens, line.start, line.end)
        {
            return;
        }
        if !self.formatting_disabled
            && self.try_push_raw_standalone_macro_line(tokens, line.start, line.end)
        {
            return;
        }
        let mut index = line.start;
        let line_columns = self.line_source_columns(&tokens[line.start..line.end]);
        self.fill_line_brace_matches(tokens, line.start, line.end);
        let multiline_case_colon =
            switch_cases::multiline_switch_label_colon(tokens, line.start, line.end);
        let iteration_limit = (line.end - line.start) * 64 + 1024;
        let mut iterations = 0usize;
        while index < line.end {
            iterations += 1;
            if iterations > iteration_limit {
                panic!(
                    "internal formatter error: formatting did not make progress at line {} \
                         (token {} of {})",
                    self.run_in_state.adjuster_observed_line_count + 1,
                    index - line.start,
                    line.end - line.start
                );
            }
            if self.formatting_disabled {
                let next = next_non_whitespace(tokens, index + 1, line.end)
                    .and_then(|next_index| tokens.get(next_index));
                self.push_disabled(&tokens[index], next);
                index += 1;
                continue;
            }
            if let Some(next_index) =
                self.try_add_braces_to_statement(tokens, line.start, index, line.end)
            {
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.try_push_one_line_defer_block(tokens, index, line.end) {
                index = next_index;
                continue;
            }
            if self.try_break_one_line_header(tokens, line.start, index, line.end) {
                continue;
            }
            self.try_break_else_if(tokens, index);
            if let Some(next_index) = self.try_remove_braces_from_statement(tokens, index, line.end)
            {
                index = next_index;
                continue;
            }
            if let Some(next_index) =
                self.try_push_one_line_initializer_block(tokens, index, line.start, line.end)
            {
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.try_push_kept_one_line_block(tokens, index, line.end) {
                index = next_index;
                continue;
            }
            if matches!(tokens[index], Token::Newline)
                && self.try_break_braceless_header_body(tokens, index)
            {
                index += 1;
                continue;
            }
            if matches!(tokens[index], Token::Newline) {
                let following_index = next_non_layout_token_index(tokens, index + 1);
                let following = following_index.map(|i| &tokens[i]);
                let after_following = following_index
                    .and_then(|i| next_non_layout_token_index(tokens, i + 1))
                    .map(|i| &tokens[i]);
                if matches!(
                    self.options.brace_style,
                    BraceStyle::Pico | BraceStyle::Lisp
                ) && matches!(following, Some(Token::Symbol('}')))
                    && previous_non_whitespace(tokens, index, line.start).is_some()
                    && let Some(Token::Whitespace(whitespace)) = tokens.get(index.wrapping_sub(1))
                {
                    if self.current_is_blank() {
                        if let Some(previous) = self.output.last_mut()
                            && !line_ends_with_comment(previous)
                        {
                            previous.truncate(previous.trim_end().len());
                            previous.push_str(whitespace);
                        }
                    } else {
                        self.trim_current_end_horizontal_space();
                        self.current.push_str(whitespace);
                        self.preserve_run_in_join_space = true;
                    }
                }
                self.observe_blank_line_context(tokens, following_index);
                self.newline_breaks_statement = Self::newline_following_token_breaks(following);
                self.next_line.leads_with_assignment =
                    matches!(following, Some(Token::Operator(operator)) if operator == "=");
                self.next_line.leads_with_class_init =
                    matches!(following, Some(Token::Symbol(':')))
                        && self.colon_leads_class_initializer();
                self.next_line.leads_with_class_base =
                    matches!(following, Some(Token::Symbol(':')))
                        && !self.colon_leads_class_initializer()
                        && self.colon_leads_class_base_clause();
                self.next_line.leads_with_comma = matches!(following, Some(Token::Symbol(',')));
                self.next_line.leads_with_open_brace =
                    matches!(following, Some(Token::Symbol('{')));
                self.next_line.leads_with_close_brace =
                    matches!(following, Some(Token::Symbol('}')));
                self.next_line.leads_with_open_paren =
                    matches!(following, Some(Token::Symbol('(')));
                self.next_line.word_followed_by_open_paren = matches!(
                    (following, after_following),
                    (Some(Token::Word(_)), Some(Token::Symbol('(')))
                );
                self.next_line.leads_with_noexcept = matches!(
                    (following, after_following),
                    (Some(Token::Word(word)), Some(Token::Symbol('('))) if word == "noexcept"
                );
            }
            let next_index = next_non_whitespace(tokens, index + 1, line.end);
            let next = next_index.and_then(|next_index| tokens.get(next_index));
            let next_is_adjacent = tokens
                .get(index + 1)
                .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
            let following_operator =
                following_operator_after_next_word(tokens, index + 1, line.end);
            self.set_input_whitespace(tokens, index, line.start);
            let offset = index - line.start;
            self.token_input.token_begins_source_line = line_columns
                .first_non_ws
                .is_none_or(|first| first >= offset);
            if self.options.align_method_colon
                && self.objc.colon_align.is_none()
                && self.token_input.token_begins_source_line
                && self.token_starts_objc_method_definition(tokens, index, line.end)
            {
                self.objc.colon_align = self.compute_objc_method_colon_align(tokens, index);
            }
            let source_column = line_columns.prefix[offset];
            let last_token_start = offset.checked_sub(1).map_or(0, |i| line_columns.prefix[i]);
            let non_ws_count = line_columns.non_ws_prefix[offset];
            let first_non_ws_is_brace = line_columns.first_non_ws_is_brace;
            self.token_input.token_source_column = source_column;
            self.token_input.token_source_line_indent = if non_ws_count == 0 {
                source_column
            } else {
                line_columns.leading_indent
            };
            self.prepare_template_continuation_token_indent(source_column);
            self.prepare_split_class_head_continuation();
            if self.token_input.token_begins_source_line
                && self.current.is_empty()
                && self.stack_state.paren_depth == 0
                && source_column
                    > ContinuationIndent::Level(
                        self.state.indent() + self.case_body_indent_extra(LineKind::Normal),
                    )
                    .columns(self.options.indent_width)
                && source_indented_macro_row(tokens, line.start, line.end, index)
                && matches!(
                    self.stack_state.brace_header_stack.last(),
                    Some(Some(header)) if header == "switch"
                )
                && !matches!(
                    self.command_state.current_header.as_deref(),
                    Some("case" | "default")
                )
                && !self
                    .output
                    .last()
                    .is_some_and(|line| operators::head_ends_binary_operator(line.trim_end()))
            {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(source_column);
            }
            self.pointer_run.gap_before_column =
                Some(if self.token_input.previous_input_whitespace.is_some() {
                    last_token_start
                } else {
                    source_column
                });
            self.token_input.token_line_opens_with_brace =
                non_ws_count == 1 && first_non_ws_is_brace;
            let no_token_after_comment = |start| {
                next_non_whitespace(tokens, start, line.end)
                    .is_none_or(|index| matches!(tokens.get(index), Some(Token::Newline)))
            };
            let token_followed_by_line_comment = next_index.is_some_and(|next_index| {
                let direct_line_comment = matches!(
                    tokens.get(next_index),
                    Some(Token::Comment(CommentKind::Line, _))
                ) && no_token_after_comment(next_index + 1);
                let after_block_comment =
                    matches!(
                        tokens.get(next_index),
                        Some(Token::Comment(CommentKind::Block, comment)) if !comment.contains('\n')
                    ) && next_non_whitespace(tokens, next_index + 1, line.end).is_some_and(
                        |after_block| {
                            matches!(
                                tokens.get(after_block),
                                Some(Token::Comment(CommentKind::Line, _))
                            ) && no_token_after_comment(after_block + 1)
                        },
                    );
                direct_line_comment || after_block_comment
            });
            self.token_input.token_followed_by_line_comment_on_line =
                token_followed_by_line_comment;
            self.token_input.token_followed_by_final_line_comment = token_followed_by_line_comment
                && !matches!(tokens.get(line.end.saturating_sub(1)), Some(Token::Newline));
            self.token_input.has_next_meaningful_token =
                next.is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
            self.token_input.next_token_is_line_comment =
                matches!(next, Some(Token::Comment(CommentKind::Line, _)));
            if self.token_input.token_begins_source_line
                && matches!(tokens[index], Token::Comment(_, _))
            {
                self.observe_block_spacing_comment(tokens, index);
            }
            let mut template_angle = template_angle_role(
                tokens,
                index,
                tokens.len(),
                self.line_state.template_angle_depth,
            );
            if matches!(template_angle, TemplateAngle::None)
                && self.template_continuation_active()
                && matches!(tokens.get(index), Some(Token::Operator(operator)) if operator == "<")
                && next_index.is_some()
                && !matches!(next, Some(Token::Operator(operator)) if operator == "=")
                && previous_non_whitespace(tokens, index, line.start)
                    .is_some_and(|previous| matches!(tokens.get(previous), Some(Token::Word(_))))
            {
                template_angle = TemplateAngle::Open;
            }
            self.next_comment_ends_line = next_index.is_some_and(|comment_index| {
                matches!(tokens.get(comment_index), Some(Token::Comment(_, _)))
                    && next_non_whitespace(tokens, comment_index + 1, line.end)
                        .is_none_or(|after| matches!(tokens.get(after), Some(Token::Newline)))
            });
            let starts_initializer_designator =
                initializer_braces::bracket_starts_initializer_designator(tokens, index, line.end);
            let inferred_definition_brace = matches!(tokens[index], Token::Symbol('{'))
                && self.inferred_definition_brace(tokens, index);
            let following_closing_braces = if matches!(tokens[index], Token::Symbol(';')) {
                let mut cursor = index + 1;
                let mut count = 0;
                loop {
                    while matches!(
                        tokens.get(cursor),
                        Some(Token::Whitespace(_) | Token::Newline)
                    ) {
                        cursor += 1;
                    }
                    if !matches!(tokens.get(cursor), Some(Token::Symbol('}'))) {
                        break;
                    }
                    count += 1;
                    cursor += 1;
                }
                count
            } else {
                0
            };
            self.push_token(
                &tokens[index],
                TokenPushContext {
                    next,
                    next_is_adjacent,
                    following_operator,
                    template_angle,
                    token_index: index,
                    starts_initializer_designator,
                    inferred_definition_brace,
                    following_closing_braces,
                },
            );
            if let Some((colon_index, has_action)) = multiline_case_colon
                && colon_index == index
            {
                if let Some(byte_index) = self.current.rfind(':') {
                    self.line_adjuster.mark_case_label_colon(byte_index);
                }
                if has_action && self.options.break_one_line_statements {
                    self.finish_line();
                    let label_spaces = self
                        .output
                        .iter()
                        .rev()
                        .find(|line| {
                            line.trim_start().strip_prefix("case").is_some_and(|rest| {
                                rest.chars().next().is_some_and(char::is_whitespace)
                            })
                        })
                        .map(|line| columns::leading_visual_width(line, self.options.tab_width))
                        .unwrap_or_else(|| {
                            self.state.line_indent(LineKind::SwitchLabel, self.options)
                                * self.options.indent_width
                        });
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(label_spaces + self.options.indent_width);
                }
            }
            index += 1;
        }
    }

    fn operator_role_at(&self, token_index: usize) -> OperatorRole {
        self.syntax_roles.operator_role_at(token_index)
    }

    fn try_push_case_line_marker(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        line_end: usize,
    ) -> bool {
        let line = tokens[line_start..line_end]
            .iter()
            .filter(|token| !matches!(token, Token::Newline))
            .map(token_text)
            .collect::<String>();
        let trimmed = line.trim();
        let Some(colon) = switch_cases::find_case_colon(trimmed) else {
            return false;
        };
        let marker = trimmed[colon + 1..].trim_start();
        if !marker.starts_with("#line") {
            return false;
        }
        self.finish_line();
        self.finish_line_text(&trimmed[..=colon]);
        self.adjust_and_publish_line(marker.to_string());
        self.preprocessor.last_output_was_preprocessor = true;
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        true
    }

    fn try_push_generated_case_compact_action_line(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        line_end: usize,
    ) -> bool {
        if !self.preprocessor.last_output_was_preprocessor {
            return false;
        }
        let line = tokens[line_start..line_end]
            .iter()
            .filter(|token| !matches!(token, Token::Newline))
            .map(token_text)
            .collect::<String>();
        let trimmed = line.trim();
        if !trimmed.starts_with("{{") || !trimmed.ends_with('}') || !trimmed.contains("}{") {
            return false;
        }
        let mut lines = self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty());
        if !lines
            .next()
            .is_some_and(|line| line.trim_start().starts_with('#'))
        {
            return false;
        }
        let Some(case_line) = lines.next() else {
            return false;
        };
        let case_trimmed = case_line.trim_start();
        if !(case_trimmed.starts_with("case ") || case_trimmed.starts_with("default"))
            || !case_trimmed.ends_with(':')
        {
            return false;
        }
        let case_body_spaces = columns::leading_visual_width(case_line, self.options.tab_width)
            + self.options.indent_width * 2;
        self.finish_line();
        self.push_output_line_spaces(trimmed, self.state.indent(), case_body_spaces);
        self.previous = PreviousToken::None;
        self.previous_was_newline = true;
        true
    }

    fn observe_input_line(&mut self, tokens: &[Token]) {
        self.continuation_indent.input_line_continuation_indent = self
            .continuation_indent
            .next_input_line_continuation_indent
            .take();
        self.line_state.passed_semicolon = false;
        self.line_state.passed_colon = false;
        self.line_state.ternary_colon = false;
        self.line_state.is_multi_statement_line = false;
        self.line_state.is_one_line_block = false;
        self.line_state.column1_line_comment = {
            let mut iter = tokens.iter();
            match iter.next() {
                Some(Token::Comment(CommentKind::Line, comment)) => comment.starts_with("//"),
                Some(Token::Whitespace(ws)) if ws == " " => matches!(
                    iter.next(),
                    Some(Token::Comment(CommentKind::Line, comment)) if comment.starts_with("//")
                ),
                _ => false,
            }
        };
        self.line_state.has_literal_quote = tokens
            .iter()
            .any(|token| matches!(token, Token::StringLiteral(_) | Token::CharLiteral(_)));
        self.line_state.indent_off_follows_code = preprocessor::indent_off_follows_code(tokens);
        self.line_state.operator_padding_disabled = tokens.iter().any(
            |token| matches!(token, Token::Comment(_, comment) if comment.contains("*NOPAD*")),
        );
        self.line_state.in_class_initializer = false;
        let trailing_comment_columns = trailing_comment_columns(tokens);
        self.token_input.input_source_indent = 0;
        let tab_width = self.options.tab_width.max(1);
        let mut source_indent = 0;
        for token in tokens {
            match token {
                Token::Whitespace(value) => {
                    for ch in value.chars() {
                        if ch == '\t' {
                            source_indent += tab_width - (source_indent % tab_width);
                        } else {
                            source_indent += ch.len_utf8();
                        }
                    }
                }
                Token::Newline => {}
                _ => {
                    self.token_input.input_source_indent = source_indent;
                    break;
                }
            }
        }
        self.line_state.trailing_comment_columns = trailing_comment_columns;
        self.line_state.has_nested_designated_init_brace =
            initializer_braces::has_nested_designated_init_brace(tokens);

        let mut statement_count = 0usize;
        let mut paren_depth = 0i32;
        for token in tokens {
            match token {
                Token::Symbol('(') => paren_depth += 1,
                Token::Symbol(')') => paren_depth -= 1,
                Token::Symbol(';') if paren_depth <= 0 => statement_count += 1,
                _ => {}
            }
        }
        if statement_count > 1 {
            self.line_state.is_multi_statement_line = true;
        }

        let mut depth = 0usize;
        let mut saw_open = false;
        for token in tokens {
            match token {
                Token::Symbol('{') => {
                    depth += 1;
                    saw_open = true;
                }
                Token::Symbol('}') if depth > 0 => {
                    self.line_state.is_one_line_block = saw_open;
                    depth -= 1;
                }
                _ => {}
            }
        }
    }

    fn push_token(&mut self, token: &Token, context: TokenPushContext<'_>) {
        let TokenPushContext {
            next,
            next_is_adjacent,
            following_operator,
            template_angle,
            token_index,
            starts_initializer_designator,
            inferred_definition_brace,
            following_closing_braces,
        } = context;
        if self.formatting_disabled {
            self.push_disabled(token, next);
            return;
        }
        if self.one_line_block_mode {
            match token {
                Token::Comment(_, comment) => {
                    self.push_inline_comment(comment);
                    return;
                }
                Token::Preprocessor(_)
                | Token::RawLine(_)
                | Token::Whitespace(_)
                | Token::Newline => {
                    return;
                }
                _ => {}
            }
        }

        if self.skip_next_attached_comment && matches!(token, Token::Comment(_, _)) {
            self.skip_next_attached_comment = false;
            return;
        }
        if !matches!(token, Token::Whitespace(_) | Token::Newline) {
            self.apply_pending_literal_continuation_indent();
        }

        if self.pad_close_paren_pending {
            match token {
                Token::Whitespace(_) => {}
                Token::Newline => self.pad_close_paren_pending = false,
                _ => {
                    if !(self.options.unpad_parens && matches!(token, Token::Symbol(')')))
                        && !symbols::close_paren_out_suppressed(token)
                        && self
                            .token_input
                            .previous_input_whitespace
                            .as_ref()
                            .is_none_or(|ws| ws.is_empty())
                    {
                        self.token_input.previous_input_whitespace = Some(" ".to_string());
                    }
                    self.pad_close_paren_pending = false;
                }
            }
        }

        if let Some(pad) = self.objc.after_paren_pad {
            match token {
                Token::Whitespace(_) => {}
                Token::Newline => self.objc.after_paren_pad = None,
                _ => {
                    self.token_input.previous_input_whitespace =
                        Some(if pad { " ".to_string() } else { String::new() });
                    self.objc.after_paren_pad = None;
                }
            }
        }

        if !matches!(token, Token::Whitespace(_) | Token::Newline) {
            self.template_close_before_current = self.previous_was_template_close;
            self.previous_was_template_close = false;
        }

        if !matches!(
            token,
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
        ) {
            self.header_paren.post_paren = self.header_paren.just_closed;
            self.header_paren.just_closed = false;
        }

        self.track_cpp_extern_c_brace(token);

        match token {
            Token::Word(word) => self.push_word(word, next),
            Token::Number(number) => self.push_literal(number, None),
            Token::StringLiteral(literal) => self.push_literal(literal, Some('"')),
            Token::CharLiteral(literal) => self.push_literal(literal, Some('\'')),
            Token::Comment(kind, comment) => self.push_comment(*kind, comment),
            Token::Preprocessor(line) => {
                self.push_preprocessor(&line.text, &line.opaque_literal_line_ranges)
            }
            Token::RawLine(line) => self.push_raw_line(line),
            Token::Operator(operator) => {
                self.push_operator(
                    operator,
                    next,
                    next_is_adjacent,
                    following_operator,
                    template_angle,
                    token_index,
                );
            }
            Token::Symbol(symbol) => self.push_symbol(
                *symbol,
                next,
                next_is_adjacent,
                token_index,
                starts_initializer_designator,
                inferred_definition_brace,
                following_closing_braces,
            ),
            Token::Whitespace(whitespace) => self.push_whitespace(whitespace),
            Token::Newline => self.push_newline(),
        }
    }

    fn push_raw_line(&mut self, line: &str) {
        if !self.current.trim().is_empty() {
            self.finish_line();
        }
        self.adjust_and_publish_line(line.to_string());
        self.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    fn push_whitespace(&mut self, whitespace: &str) {
        if self.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
            self.trim_current_end_horizontal_space();
            if self.options.unpad_parens {
                self.current.push(if whitespace.ends_with('\t') {
                    '\t'
                } else {
                    ' '
                });
            } else {
                self.current.push_str(whitespace);
            }
        } else if whitespace.contains('\x0c') && self.current.trim().is_empty() {
            self.current.push('\x0c');
            self.current_is_preindented = true;
        }
    }

    fn set_input_whitespace(&mut self, tokens: &[Token], index: usize, lower: usize) {
        self.token_input.previous_input_was_adjacent = index > lower
            && tokens
                .get(index - 1)
                .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
        self.token_input.previous_input_whitespace = (index > lower)
            .then(|| tokens.get(index - 1))
            .flatten()
            .and_then(|token| match token {
                Token::Whitespace(ws) => Some(ws.clone()),
                _ => None,
            })
            .filter(|_| !matches!(tokens.get(index.wrapping_sub(2)), Some(Token::Newline)));
        self.token_input.next_input_whitespace =
            tokens.get(index + 1).and_then(|token| match token {
                Token::Whitespace(ws) => Some(ws.clone()),
                _ => None,
            });
        self.pointer_run.trailing_ws = None;
        self.pointer_run.next_is_name_like = false;
        self.pointer_run.followed_by_reference = false;
        self.pointer_run.reference_has_name = false;
        self.pointer_run.followed_by_comment = false;
        self.pointer_run.star_count = 0;
        self.pointer_run.gap_before_column = None;
        if let Some(Token::Operator(operator)) = tokens.get(index)
            && matches!(operator.as_str(), "*" | "&" | "&&" | "^")
        {
            let mut last = index;
            while let Some(Token::Operator(next_operator)) = tokens.get(last + 1)
                && next_operator == operator
            {
                last += 1;
            }
            self.pointer_run.star_count = (last - index + 1) * operator.chars().count();
            self.pointer_run.trailing_ws = match tokens.get(last + 1) {
                Some(Token::Whitespace(ws)) => Some(ws.clone()),
                _ => None,
            };
            let after_run_index = next_non_whitespace(tokens, last + 1, tokens.len());
            let after_run = after_run_index.and_then(|index| tokens.get(index));
            self.pointer_run.next_is_name_like = pointers::pointer_next_is_name_like(after_run);
            self.pointer_run.followed_by_reference = matches!(
                after_run,
                Some(Token::Operator(operator)) if matches!(operator.as_str(), "&" | "&&")
            );
            self.pointer_run.reference_has_name = after_run_index.is_some_and(|reference| {
                self.pointer_run.followed_by_reference
                    && next_non_whitespace(tokens, reference + 1, tokens.len())
                        .and_then(|index| tokens.get(index))
                        .is_some_and(|token| matches!(token, Token::Word(_) | Token::Symbol('[')))
            });
            self.pointer_run.followed_by_comment = matches!(after_run, Some(Token::Comment(_, _)));
        }
    }

    fn apply_pending_literal_continuation_indent(&mut self) {
        let Some(spaces) = self
            .continuation_indent
            .pending_literal_continuation_indent_spaces
            .take()
        else {
            return;
        };
        if self.current.trim().is_empty() && self.line_state.has_literal_quote {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
    }

    fn current_ends_cast(&self) -> bool {
        self.current_cast_words()
            .is_some_and(|words| words.iter().any(|word| is_type_like_pointer_word(word)))
    }

    fn current_statement_contains_assignment(&self) -> bool {
        self.current
            .rsplit([';', '{', '}'])
            .next()
            .unwrap_or(&self.current)
            .contains('=')
    }

    fn current_ends_numeric_cast(&self) -> bool {
        self.current_cast_words()
            .and_then(|words| words.last().copied())
            .is_some_and(is_numeric_variable_word)
    }

    fn current_ends_pointer_cast(&self) -> bool {
        let current = self.current.trim_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = current.rfind('(') else {
            return false;
        };
        current[open + 1..current.len() - 1]
            .trim_end()
            .ends_with(['*', '&', '^'])
    }

    fn current_ends_sizeof_pointer_expr(&self) -> bool {
        let current = self.current.trim_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = current.rfind('(') else {
            return false;
        };
        trailing_word(current[..open].trim_end()) == "sizeof"
            && current[open + 1..current.len() - 1]
                .trim_end()
                .ends_with(['*', '&', '^'])
    }

    fn current_ends_size_operator_call(&self) -> bool {
        let current = self.current.trim_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = current.rfind('(') else {
            return false;
        };
        matches!(
            trailing_word(current[..open].trim_end()),
            "sizeof" | "alignof" | "_Alignof"
        )
    }

    fn current_paren_started_by_expression_keyword(&self) -> bool {
        let mut before = self.current.trim_end();
        while let Some(open) = before.rfind('(') {
            let prefix = before[..open].trim_end();
            if !prefix.ends_with('(') {
                let word = trailing_word(prefix);
                return matches!(
                    word,
                    "if" | "while" | "for" | "switch" | "return" | "sizeof"
                );
            }
            before = prefix;
        }
        false
    }

    fn current_paren_started_by_catch(&self) -> bool {
        let mut before = self.current.trim_end();
        while let Some(open) = before.rfind('(') {
            let prefix = before[..open].trim_end();
            if !prefix.ends_with('(') {
                return trailing_word(prefix) == "catch";
            }
            before = prefix;
        }
        false
    }

    fn current_paren_is_expression_context(&self) -> bool {
        let Some(open) = self.current.rfind('(') else {
            return false;
        };
        let prefix = self.current[..open].trim_end();
        self.current_paren_started_by_expression_keyword()
            || prefix.chars().any(|ch| {
                matches!(
                    ch,
                    '?' | '=' | '<' | '>' | '+' | '-' | '/' | '%' | '|' | '&' | '^'
                )
            })
    }

    fn current_cast_words(&self) -> Option<Vec<&str>> {
        let current = self.current.trim_end();
        if !current.ends_with(')') {
            return None;
        }
        let open = current.rfind('(')?;
        if current[..open]
            .chars()
            .next_back()
            .is_some_and(is_identifier_continue)
        {
            return None;
        }
        if matches!(
            trailing_word(current[..open].trim_end()),
            "sizeof" | "alignof" | "_Alignof"
        ) {
            return None;
        }
        let inner = &current[open + 1..current.len() - 1];
        if inner.chars().any(|ch| {
            matches!(
                ch,
                '+' | '-' | '/' | '%' | '|' | '&' | '^' | '=' | '<' | '>' | '?' | ':'
            )
        }) || (inner.contains('*') && !inner.trim_end().ends_with('*'))
        {
            return None;
        }
        Some(
            inner
                .split(|ch: char| !is_identifier_continue(ch))
                .filter(|part| !part.is_empty())
                .collect(),
        )
    }

    fn push_newline(&mut self) {
        self.objc.post_prefix = false;
        self.objc.post_method_colon = false;
        self.objc.return_paren_depth = None;
        self.objc.param_paren_depth = None;
        if self.incomplete_control_header() && !self.next_line.leads_with_open_paren {
            self.newline_breaks_statement = true;
            if !self.next_line.leads_with_close_brace {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            }
            self.pending_braceless_block_bias = None;
            self.inline_nested_header_braceless_bias = None;
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.frame_stack.clear_header();
        }
        if self.current_is_preindented && self.current.contains('\x0c') {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.current.trim().is_empty() {
            if self.next_line.leads_with_class_init || self.next_line.leads_with_class_base {
                self.stack_state.clear_continuation_indents();
                self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
                self.continuation_indent.next_line_indent_spaces = None;
                if self.next_line.leads_with_class_base {
                    self.in_class_base_clause = true;
                }
            }
            if (self.previous_was_newline || self.output.is_empty())
                && self.should_preserve_input_empty_line()
            {
                self.push_empty_line();
            }
            self.previous_was_newline = true;
        } else if is_split_export_head(self.current.trim()) && !self.next_line.leads_with_open_brace
        {
            self.finish_split_class_head_line();
        } else if self.previous_was_newline {
            self.finish_line();
            if self.should_preserve_input_empty_line() {
                self.push_empty_line();
            }
            self.previous_was_newline = true;
        } else if self.literal_line.unterminated_literal_line {
            let next_literal_indent = self
                .current
                .rfind(['"', '\''])
                .map(|column| self.current_line_indent_spaces() + column);
            self.finish_line();
            self.continuation_indent
                .pending_literal_continuation_indent_spaces = next_literal_indent;
            self.literal_line.unterminated_literal_line = false;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_class_init {
            let header_indent = self.state.indent();
            let follows_function_try = self.class_initializer_follows_function_try();
            self.finish_line();
            self.stack_state.clear_continuation_indents();
            self.continuation_indent.next_line_indent =
                Some(header_indent + usize::from(!follows_function_try));
            self.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_class_base {
            let header_indent = self.state.indent();
            self.finish_line();
            self.stack_state.clear_continuation_indents();
            self.continuation_indent.next_line_indent = Some(header_indent + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.in_class_base_clause = true;
            self.previous_was_newline = true;
        } else if self.current.trim_end().ends_with(',') && self.is_top_level_table_macro_row() {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(1);
            self.previous_was_newline = true;
        } else if let Some(column) = self.current_inline_array_column()
            && self.state.statement_depth() == 0
            && self.stack_state.paren_depth == 0
            && line_ends_with_comment(&self.current)
            && self.current[..self.current_trailing_comment_split_limit()]
                .trim_end()
                .ends_with(',')
        {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(column);
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_comma
            && self.state.statement_depth() > 0
            && self.current.trim_start().starts_with(',')
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.stack_state.clear_continuation_indents();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.previous_was_newline = true;
        } else if matches!(self.previous, PreviousToken::Comma)
            && self.state.statement_depth() == 0
            && (self.in_initializer_brace()
                || self.innermost_init_block_brace()
                || self.in_enum_declaration_brace()
                || self.current_inline_array_column().is_some())
        {
            let direct_list_sibling_column = if self.current.trim_end().ends_with("},")
                && !self.current.trim_start().starts_with('{')
            {
                self.output.iter().rev().take(64).find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let prefix = code.strip_suffix('{')?.trim_end();
                    let prefix = prefix.trim_start();
                    (!prefix.is_empty()
                        && !prefix.starts_with('{')
                        && !prefix.contains(['=', '(', '@']))
                    .then(|| columns::leading_visual_width(line, self.options.tab_width))
                })
            } else {
                None
            };
            let inline_column = self.current_inline_array_column();
            let clear_enum_continuation = self.in_enum_declaration_brace()
                && !self.current.contains('{')
                && unmatched_open_paren_column(self.current.trim_end()).is_none();
            self.finish_line();
            if clear_enum_continuation {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
                self.stack_state.clear_continuation_indents();
            } else if let Some(column) = direct_list_sibling_column {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(column);
                self.stack_state.clear_continuation_indents();
            } else if let Some(column) = inline_column {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(column);
            }
            self.previous_was_newline = true;
        } else if matches!(self.previous, PreviousToken::Comma)
            && self.state.statement_depth() == 0
            && self.multi_declarator_indent_spaces.is_some()
            && !self.in_initializer_brace()
            && !self.in_aggregate_declaration_brace()
        {
            let column = self.multi_declarator_indent_spaces;
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = column;
            self.previous_was_newline = true;
        } else if self.is_complete_template_declaration_line() || self.is_objc_standalone_line() {
            self.finish_line();
            self.previous_was_newline = true;
        } else if (self.is_objc_method_line() || self.objc.method_continuation)
            && !self.current.trim_end().ends_with(';')
        {
            self.finish_line();
            if self.newline_breaks_statement {
                self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
                self.continuation_indent.next_line_indent_spaces = None;
                self.objc.method_continuation = true;
            } else {
                self.objc.method_continuation = self.next_line.leads_with_open_brace;
            }
            self.previous_was_newline = true;
        } else if self.current.trim_end().ends_with('\\')
            && self.stack_state.paren_depth == 0
            && self.current_line_indent_spaces()
                > self.continuation_base_indent() * self.options.indent_width
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()].trim() == ":"
            && self
                .frame_stack
                .active_constructor_initializer()
                .is_some_and(|frame| frame.function_try)
        {
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()]
            .trim_end()
            .ends_with(':')
            && !self.current.trim_start().starts_with("//")
            && !self.current[..self.current_trailing_comment_split_limit()]
                .trim_end()
                .contains('?')
            && !self.current_ends_base_clause_colon()
            && (self.in_initializer_brace()
                || self.current_inline_array_column().is_some()
                || self.current_line_indent_spaces()
                    > self.continuation_base_indent() * self.options.indent_width)
        {
            let column = self
                .current_inline_array_column()
                .unwrap_or_else(|| self.current_line_indent_spaces());
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(column);
            self.previous_was_newline = true;
        } else if self.in_enum_declaration_brace()
            && self.current.trim_end().ends_with(",")
            && !self.current.contains('{')
            && unmatched_open_paren_column(self.current.trim_end()).is_none()
        {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            self.stack_state.clear_continuation_indents();
            self.previous_was_newline = true;
        } else if self.current_initializer_member_before_closing_brace() {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.unmatched_closing_brace_recovery {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(0);
            self.state.clear_continuation_indents();
            self.stack_state.clear_continuation_indents();
            self.frame_stack.clear_stream_frames();
            self.frame_stack.clear_logical_frames();
            self.continuation_indent.logical_chain_indent_spaces = None;
            self.previous_was_newline = true;
        } else if self.is_continuation_break()
            && !(self.current_is_preindented && self.current.trim_end().ends_with("*/"))
        {
            let is_logical_continuation = self.logical_continuation_indent_spaces().is_some();
            let has_array_bound_operator_continuation = self
                .array_bound_operator_continuation_indent_spaces()
                .is_some();
            let after_compound_literal_comma =
                std::mem::take(&mut self.compound_literal.after_comma)
                    && self.compound_literal.arg_paren_depth == Some(self.stack_state.paren_depth)
                    && self.compound_literal.arg_brace_depth
                        == Some(self.stack_state.brace_header_stack.len());
            let has_macro_call_argument_continuation =
                matches!(self.previous, PreviousToken::Comma)
                    && !after_compound_literal_comma
                    && self.macro_call_argument_indent_spaces().is_some();
            let saved_indent = if after_compound_literal_comma {
                Some(ContinuationIndent::Spaces(
                    self.current_line_indent_spaces(),
                ))
            } else {
                self.continuation_indent
                    .after_one_shot_continuation_indent
                    .take()
            };
            let saved_indent =
                if has_array_bound_operator_continuation || has_macro_call_argument_continuation {
                    None
                } else {
                    saved_indent
                };
            let indent = saved_indent.unwrap_or_else(|| self.next_continuation_indent());
            let one_shot_indent = saved_indent
                .is_none()
                .then(|| {
                    (!has_array_bound_operator_continuation)
                        .then(|| self.trailing_open_bracket_indent_spaces())
                        .flatten()
                })
                .flatten();
            if is_logical_continuation
                && unmatched_open_paren_column(self.current.trim_end()).is_none()
            {
                self.continuation_indent.logical_chain_indent_spaces =
                    Some(indent.columns(self.options.indent_width));
            }
            let previous_before_line = self.previous;
            let clear_continuation_after_line = self
                .continuation_indent
                .clear_continuation_after_line
                .is_some();
            let case_label_with_comment =
                switch_cases::case_label_with_trailing_comment(self.current.trim());
            self.finish_line();
            if matches!(
                previous_before_line,
                PreviousToken::Word
                    | PreviousToken::Literal
                    | PreviousToken::CloseParen
                    | PreviousToken::CloseBracket
            ) {
                self.previous = previous_before_line;
            }
            if !clear_continuation_after_line {
                if let Some(spaces) = one_shot_indent {
                    self.continuation_indent.after_one_shot_continuation_indent = Some(indent);
                    self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
                } else {
                    self.set_next_continuation_indent(indent);
                }
            }
            if case_label_with_comment && let Some(previous) = self.output.last() {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(
                    columns::leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            self.previous_was_newline = true;
        } else if self.current.trim_end().ends_with(';')
            || self.current.trim_end().ends_with("*/")
            || macro_invocations::is_standalone_macro_invocation_line(self.current.trim())
        {
            self.finish_line();
            self.objc.method_continuation = false;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_open_brace && self.current.trim_end().ends_with('[') {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_open_brace
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
            && (self.current.trim_start().starts_with('}')
                || self.current.trim_start().starts_with([
                    '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.', '~',
                ]))
        {
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            self.stack_state.clear_continuation_indents();
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()]
            .trim_end()
            .ends_with(':')
            && !self.current.trim_start().starts_with("//")
            && labels::is_label_start(
                self.current[..self.current_trailing_comment_split_limit()]
                    .trim()
                    .trim_end_matches(':'),
                &self.options.access_labels,
            )
        {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.newline_breaks_statement
            && self.header_allows_statement_break()
            && self.current.trim() != "else"
        {
            let bare_return = self.current.trim() == "return";
            let incomplete_control_header = self.incomplete_control_header();
            let header_indent = self.state.indent();
            self.finish_line();
            if bare_return {
                self.continuation_indent.next_line_indent = Some(header_indent + 1);
                self.continuation_indent.next_line_indent_spaces = None;
            } else if incomplete_control_header {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
                self.pending_braceless_block_bias = None;
                self.inline_nested_header_braceless_bias = None;
                self.command_state.current_header = None;
                self.command_state.preprocessor_after_header = false;
                self.frame_stack.clear_header();
            }
            self.objc.method_continuation = false;
            self.previous_was_newline = true;
        } else {
            self.ensure_space();
            self.previous_was_newline = true;
        }
    }

    fn current_initializer_member_before_closing_brace(&self) -> bool {
        if !self.next_line.leads_with_close_brace
            || !(self.in_initializer_brace()
                || self.innermost_init_block_brace()
                || self.current_inline_array_column().is_some())
        {
            return false;
        }
        let code = self.current[..self.current_trailing_comment_split_limit()].trim_end();
        let trimmed = code.trim_start();
        !trimmed.is_empty()
            && !trimmed.starts_with(['#', '{', '}'])
            && !code.ends_with([',', ';', '\\'])
            && !operators::head_ends_binary_operator(code)
            && unmatched_open_paren_column(code).is_none()
    }

    fn incomplete_control_header(&self) -> bool {
        let Some(header @ ("if" | "for" | "while" | "switch")) =
            self.command_state.current_header.as_deref()
        else {
            return false;
        };
        let current = self.current.trim();
        let code = self.current[..self.current_trailing_comment_split_limit()].trim();
        (trailing_word(code) == header || trailing_word(current) == header)
            && self.command_state.previous_command_char != Some(')')
            && self.header_paren.depth.is_none()
    }

    fn header_allows_statement_break(&self) -> bool {
        match self.command_state.current_header.as_deref() {
            None => true,
            Some("if" | "for" | "while" | "switch") => self.incomplete_control_header(),
            Some("case" | "default") => {
                let leading = self
                    .current
                    .trim_start()
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .next()
                    .unwrap_or_default();
                !matches!(leading, "case" | "default")
            }
            Some(_) => false,
        }
    }

    fn push_disabled(&mut self, token: &Token, next: Option<&Token>) {
        let is_indent_on =
            matches!(token, Token::Comment(_, comment) if comment.contains("*INDENT-ON*"));
        if !is_indent_on && let Some(disabled) = self.disabled_formatting.as_mut() {
            disabled.push_token(
                token,
                TokenPushContext {
                    next,
                    next_is_adjacent: false,
                    following_operator: None,
                    template_angle: TemplateAngle::None,
                    token_index: usize::MAX,
                    starts_initializer_designator: false,
                    inferred_definition_brace: false,
                    following_closing_braces: 0,
                },
            );
        }

        match token {
            Token::Newline => self.finish_disabled_line(),
            Token::Comment(_, comment) if comment.contains("*INDENT-ON*") => {
                self.push_disabled_raw_text(comment);
                self.finish_disabled_line();
                if let Some(disabled) = self.disabled_formatting.take() {
                    disabled.restore(self);
                }
                self.previous_pre_adjust_line = self.output.last().cloned();
                self.reset_block_spacing();
                self.previous_was_newline = false;
                self.formatting_disabled = false;
            }
            _ => self.push_disabled_raw_text(&token_text(token)),
        }
    }

    fn push_disabled_raw_text(&mut self, text: &str) {
        for part in text.split_inclusive('\n') {
            if let Some(line) = part.strip_suffix('\n') {
                self.current.push_str(line);
                self.finish_disabled_line();
            } else {
                self.current.push_str(part);
            }
        }
    }
}

#[cfg(test)]
mod tests;
