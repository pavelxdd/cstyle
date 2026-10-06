use crate::config::{BraceStyle, FormatOptions, IndentStyle};
use crate::formatter::braces::classification::ExternCGuard;
use crate::formatter::braces::initializers::InlineArrayState;
use crate::formatter::braces::rewrite::{
    add_marked_cross_line_statement_braces, following_operator_after_next_word,
    previous_non_whitespace, remove_cross_line_statement_braces,
};
use crate::formatter::braces::{compound_literals, initializers};
use crate::formatter::constructs::class_declarations::is_split_export_head;
use crate::formatter::constructs::objc::token_starts_objc_method_definition;
use crate::formatter::constructs::swig::SwigState;
use crate::formatter::constructs::switch_cases::SwitchCaseLayoutState;
use crate::formatter::constructs::template_declarations::TemplateDeclarationState;
use crate::formatter::constructs::{headers, labels, objc, switch_cases};
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::continuation::max_length::MaxLengthLineState;
use crate::formatter::index_hash::{IndexMap, IndexSet};
use crate::formatter::lexer::{
    CommentKind, Token, TokenLine, TokenLineCursor, matching_close_pairs,
    next_non_layout_token_index, next_non_whitespace, token_char_len, token_text,
};
use crate::formatter::output::block_spacing::BlockSpacingState;
use crate::formatter::output::member_spacing::MemberSpacingBoundary;
use crate::formatter::output::source_indent::source_indented_macro_row;
use crate::formatter::output::{buffer, line_adjust};
use crate::formatter::preprocessor::backslash_bodies::BackslashBodyState;
use crate::formatter::preprocessor::{macro_invocations, preprocessor_block_indentability};
use crate::formatter::state::current_line::CurrentLine;
use crate::formatter::state::frame::FrameStack;
use crate::formatter::state::indentation::{IndentationState, LineKind};
use crate::formatter::state::{
    CommandState, LineState, NestingState, PreviousToken, RunInState, TokenInputState, next_line,
};
use crate::formatter::structure::SourceTree;
use crate::formatter::structure::blocks::is_code_token;
use crate::formatter::syntax::language::{
    is_numeric_variable_word, is_type_like_pointer_word, is_unpad_kept_type_word,
};
use crate::formatter::syntax::{
    OperatorRole, SyntaxRoles, TemplateAngle, classify_syntax, known_template_angle_role,
    template_openers,
};
use crate::formatter::text::columns;
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::{
    line_ends_with_comment, line_paren_imbalance, unmatched_open_paren_column,
};
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::comments::{CommentState, trailing_comment_columns};
use crate::formatter::tokens::disabled_formatting::DisabledFormattingState;
use crate::formatter::tokens::{literals, operators, pointers, symbols};
use crate::formatter::{continuation, preprocessor, syntax};
use crate::source::lex::{is_identifier_continue, trailing_word};

/// The first token from `start` passing a test, read on as later ends ask:
/// the tokens' address, the start, the token read to, and the match.
#[derive(Clone, Copy)]
pub(crate) struct ForwardFind {
    address: usize,
    start: usize,
    scanned: usize,
    found: Option<usize>,
}

impl ForwardFind {
    /// The first token in `start..end` passing `test`, the last look kept
    /// in `cache`, which holds looks with this one test only.
    pub(crate) fn first_in(
        cache: &std::cell::Cell<Option<ForwardFind>>,
        tokens: &[Token],
        start: usize,
        end: usize,
        mut test: impl FnMut(usize) -> bool,
    ) -> Option<usize> {
        let address = tokens.as_ptr() as usize;
        let mut scan = cache
            .get()
            .filter(|scan| (scan.address, scan.start) == (address, start))
            .unwrap_or(ForwardFind {
                address,
                start,
                scanned: start,
                found: None,
            });
        if scan.found.is_none() && scan.scanned < end {
            scan.found = (scan.scanned..end).find(|&index| test(index));
            scan.scanned = end;
            cache.set(Some(scan));
        }
        scan.found.filter(|&found| found < end)
    }
}

/// A chain of subscripts read for an initializer designator: the tokens'
/// address and length and the end it was read to, its `[`s in order, and its
/// answer.
struct DesignatorChain {
    key: (usize, usize, usize),
    opens: Vec<usize>,
    answer: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct TokenPushContext<'a> {
    pub(crate) next: Option<&'a Token>,
    pub(crate) next_is_adjacent: bool,
    pub(crate) following_operator: Option<&'a str>,
    pub(crate) template_angle: TemplateAngle,
    pub(crate) token_index: usize,
    pub(crate) starts_initializer_designator: bool,
    pub(crate) inferred_definition_brace: bool,
    pub(crate) following_closer_width: usize,
}

#[derive(Default)]
struct LineSourceColumns {
    pub(crate) prefix: Vec<usize>,
    non_ws_prefix: Vec<usize>,
    first_non_ws: Option<usize>,
    first_non_ws_is_brace: bool,
    pub(crate) leading_indent: usize,
}

/// Layout state that a preprocessor branch or a disabled-formatting region
/// saves and restores as one unit.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct LayoutState {
    pub(crate) indentation: IndentationState,
    pub(crate) command_state: CommandState,
    pub(crate) nesting: NestingState,
    pub(crate) frame_stack: FrameStack,
    pub(crate) line_state: LineState,
    pub(crate) run_in_state: RunInState,
    pub(crate) line_adjuster: line_adjust::LineAdjuster,
    pub(crate) previous_pre_adjust_line: Option<String>,
    pub(crate) pending_member_spacing: Option<MemberSpacingBoundary>,
    pub(crate) previous: PreviousToken,
    /// The code token a run of trailing comments followed.
    pub(crate) previous_before_comment: Option<PreviousToken>,
    pub(crate) literal_line: literals::LiteralLineState,
    pub(crate) continuation_indent: continuation::ContinuationIndentState,
    pub(crate) objc: objc::ObjCLineState,
    pub(crate) switch_case_layout: SwitchCaseLayoutState,
    pub(crate) in_class_base_clause: bool,
    pub(crate) split_class_export_pending_base: bool,
    pub(crate) template_declaration: TemplateDeclarationState,
    pub(crate) else_if_break_depths: Vec<usize>,
    /// The else-if chain ends once the line that closed its block ends.
    pub(crate) unwind_else_if_after_line: bool,
    pub(crate) compound_literal: compound_literals::CompoundLiteralState,
    pub(crate) pending_braceless_block_bias: Option<usize>,
    pub(crate) inline_nested_header_braceless_bias: Option<usize>,
}

/// The output's line count and edit version, which identify what a look
/// back over it read.
pub(crate) type OutputKey = (usize, u64);

/// An output line leaving a paren open, and the column after that paren.
pub(crate) type OpenParenLine = (usize, usize);

/// The address and length of a token slice, the start of a line of it,
/// and the index of that line's first line comment.
pub(crate) type LineCommentCache = (usize, usize, usize, Option<usize>);

pub(crate) struct FormatEngine<'a> {
    pub(crate) options: &'a FormatOptions,
    /// Indent style of the finished output; the engine itself may lay out a
    /// tab-indented style in spaces.
    pub(crate) output_indent_style: IndentStyle,
    pub(crate) output: buffer::OutputBuffer,
    /// The last constructor initializer scan.
    pub(crate) constructor_scan_cache: std::cell::Cell<
        Option<crate::formatter::constructs::constructor_initializers::ConstructorScanCache>,
    >,
    /// The last reads past blanks and newlines after a line and after the
    /// token that follows it: the token each started at and the first
    /// token after them.
    layout_runs: [Option<(usize, Option<usize>)>; 2],
    /// The last look back for the open paren of a constructor initializer's
    /// argument.
    pub(crate) open_paren_arg_cache: std::cell::Cell<
        Option<crate::formatter::constructs::constructor_initializers::OpenParenArgLines>,
    >,
    /// The last look back for a line leaving a paren open, keyed by the
    /// output it read.
    pub(crate) open_paren_scan_cache: std::cell::Cell<Option<(OutputKey, Option<OpenParenLine>)>>,
    /// Whether the output last read holds a constructor initializer colon.
    pub(crate) constructor_colon_cache: std::cell::Cell<Option<(OutputKey, bool)>>,
    /// Whether the output last read stands inside a macro call's arguments.
    pub(crate) macro_call_context_cache: std::cell::Cell<Option<(OutputKey, bool)>>,
    /// The look back for a line ending a macro call's split opener.
    pub(crate) split_opener_look: crate::formatter::output::buffer::LineLook,
    /// The look back for a line ending with `;` or being `{` or `}`.
    pub(crate) statement_edge_look: crate::formatter::output::buffer::LineLook,
    /// The look back for a line ending with `,` in an over-long `new` call.
    pub(crate) over_max_new_call_look: crate::formatter::output::buffer::LineLook,
    /// The look back for a line ending a statement or a group before a
    /// `new` call's next line.
    pub(crate) new_call_edge_look: crate::formatter::output::buffer::LineLook,
    /// The look back for a line ending with the `{` of a named list.
    pub(crate) named_list_open_look: crate::formatter::output::buffer::LineLook,
    /// The first line comment of the last token line asked about: the
    /// address and length of its tokens, its start, and the comment's index.
    pub(crate) line_comment_cache: std::cell::Cell<Option<LineCommentCache>>,
    /// Which tokens of the tree open a template.
    template_openers: std::cell::OnceCell<Vec<bool>>,
    /// The last chain of subscripts read for an initializer designator.
    designator_chain_cache: std::cell::RefCell<Option<DesignatorChain>>,
    /// Each matched `[` of the tree with its `]`, in token order.
    bracket_closes: std::cell::OnceCell<Vec<(u32, u32)>>,
    /// Indices of the `case` and `default` words, in order.
    case_labels: std::cell::OnceCell<Vec<u32>>,
    /// The source columns of the line being formatted, kept to reuse their
    /// buffers.
    line_columns: LineSourceColumns,
    /// The text of the line being finished, kept to reuse its buffer.
    pub(crate) finished_line_buffer: String,
    /// The look backs for the active case label read so far.
    pub(crate) active_case_cache: std::cell::RefCell<
        buffer::LookBackAnswers<
            Option<crate::formatter::constructs::switch_cases::ActiveCaseLayout>,
        >,
    >,
    /// The look backs for an open lambda body read so far.
    pub(crate) open_lambda_cache: std::cell::RefCell<buffer::LookBackAnswers<Option<usize>>>,
    /// The look backs for the begin of the macro block a line lies in read
    /// so far.
    pub(crate) macro_block_cache: std::cell::RefCell<buffer::LookBackAnswers<Option<usize>>>,
    /// The tokens the last looks back for astyle's statement start passed.
    pub(crate) stack_start_cache:
        std::cell::RefCell<crate::formatter::output::layout::astyle_stack::StackStartPath>,
    /// The last brace or newline token looked for: the tokens' address,
    /// where the look started, and the token found.
    pub(crate) brace_or_newline_cache: std::cell::Cell<Option<(usize, usize, usize)>>,
    /// The last `return` an operand row looked back to: the tokens'
    /// address, the row's first token, and the `return`.
    pub(crate) operand_return_cache: std::cell::Cell<Option<(usize, usize, Option<usize>)>>,
    /// The braces of the last parens looked in: the tokens' address, the
    /// parens, and their first brace and last that is no compound literal's.
    pub(crate) paren_braces_cache:
        std::cell::Cell<Option<crate::formatter::output::layout::astyle_stack::ParenBraces>>,
    /// The last look back from a `:` for a `case`.
    pub(crate) case_label_cache:
        std::cell::Cell<Option<crate::formatter::output::layout::astyle_stack::CaseLabelLook>>,
    /// The last look back for the line an assignment's value starts after.
    pub(crate) assignment_rhs_cache:
        std::cell::Cell<Option<crate::formatter::continuation::operator_chains::AssignmentRhsWalk>>,
    /// The looks back for the line opening an initializer's rows.
    pub(crate) row_openers: std::cell::RefCell<crate::formatter::braces::initializers::RowOpeners>,
    /// The last look back from a leading operator for its statement's
    /// assignment: the tokens' address, the operator, its group, and the
    /// farthest `=` found, or `None` when the look gave up.
    pub(crate) operand_assign_cache:
        std::cell::Cell<Option<crate::formatter::output::layout::anchors::OperandAssignLook>>,
    /// The last look for an assignment stacked in a statement.
    pub(crate) stacked_assignment_cache: std::cell::Cell<Option<ForwardFind>>,
    /// The last look for a token registering an indent before a
    /// declarator's `,`.
    pub(crate) declarator_registers_cache: std::cell::Cell<Option<ForwardFind>>,
    /// The last statement start a declarator's `,` looked back to: the
    /// tokens' address, the `,`, and the start.
    pub(crate) declarator_start_cache: std::cell::Cell<Option<(usize, usize, usize)>>,
    /// The last replay of astyle's stack, for the next line of its
    /// statement.
    pub(crate) astyle_replay_cache:
        std::cell::Cell<Option<crate::formatter::output::layout::astyle_stack::ReplayCache>>,
    pub(crate) layout: LayoutState,
    pub(crate) current: CurrentLine,
    line_brace_match_start: usize,
    line_brace_matches: Vec<Option<usize>>,
    pub(crate) previous_was_newline: bool,
    pub(crate) previous_was_template_close: bool,
    pub(crate) newline_breaks_statement: bool,
    pub(crate) preserve_block_spacing_comment_blank: bool,
    pub(crate) next_line: next_line::NextLineState,
    pub(crate) multi_declarator_indent_spaces: Option<usize>,
    pub(crate) block_spacing: BlockSpacingState,
    pub(crate) comments: CommentState,
    pub(crate) source_run_in_brace_lines: Vec<usize>,
    pub(crate) disabled_formatting: Option<DisabledFormattingState<'a>>,
    pub(crate) current_is_preindented: bool,
    /// The current line is a comment row astyle writes as it stands.
    pub(crate) current_is_verbatim: bool,
    /// The line adjuster before it saw the lone `}` published at the
    /// index, restored when a closing header takes that line back.
    pub(crate) adjuster_before_lone_brace: Option<(usize, line_adjust::LineAdjuster)>,
    pub(crate) unmatched_closing_brace_recovery: bool,
    pub(crate) preserve_run_in_join_space: bool,
    pub(crate) one_line_block_mode: bool,
    /// A one-line block being formatted on the line after a directive.
    pub(crate) one_line_block_after_directive: bool,
    pub(crate) inline_array: InlineArrayState,
    pub(crate) max_length_line: MaxLengthLineState,
    pub(crate) backslash_body: BackslashBodyState,
    pub(crate) swig: SwigState,
    pub(crate) may_have_class_base_access: bool,
    /// Whether the source may spell `noexcept`.
    pub(crate) may_have_noexcept: bool,
    pub(crate) space_after_cast: bool,
    pub(crate) pad_close_paren_pending: bool,
    pub(crate) header_paren: headers::HeaderParenState,
    pub(crate) current_line_has_class_initializer_colon: bool,
    pub(crate) token_input: TokenInputState,
    pub(crate) pointer_run: pointers::PointerRunState,
    pub(crate) preprocessor: preprocessor::PreprocessorState,
    pub(crate) access_modified_braces: IndexSet<usize>,
    /// Closing braces add-braces put after statements.
    pub(crate) added_closing_braces: IndexSet<usize>,
    pub(crate) added_opener_overruns: IndexMap<usize, usize>,
    pub(crate) syntax_roles: SyntaxRoles,
    pub(crate) tree: SourceTree,
    pub(crate) pending_extern: bool,
    pub(crate) extern_c_guard: ExternCGuard,
}

impl<'a> FormatEngine<'a> {
    /// The options with the indent style of the finished output.
    pub(crate) fn output_options(&self) -> std::borrow::Cow<'a, FormatOptions> {
        if self.output_indent_style == self.options.indent_style {
            std::borrow::Cow::Borrowed(self.options)
        } else {
            let mut options = self.options.clone();
            options.indent_style = self.output_indent_style;
            std::borrow::Cow::Owned(options)
        }
    }

    pub(crate) fn new(options: &'a FormatOptions) -> Self {
        Self {
            options,
            output_indent_style: options.indent_style,
            output: buffer::OutputBuffer::default(),
            constructor_scan_cache: std::cell::Cell::new(None),
            layout_runs: [None; 2],
            open_paren_arg_cache: std::cell::Cell::new(None),
            open_paren_scan_cache: std::cell::Cell::new(None),
            macro_call_context_cache: std::cell::Cell::new(None),
            split_opener_look: Default::default(),
            statement_edge_look: Default::default(),
            over_max_new_call_look: Default::default(),
            new_call_edge_look: Default::default(),
            named_list_open_look: Default::default(),
            line_comment_cache: std::cell::Cell::new(None),
            line_columns: LineSourceColumns::default(),
            finished_line_buffer: String::new(),
            active_case_cache: std::cell::RefCell::default(),
            open_lambda_cache: std::cell::RefCell::default(),
            macro_block_cache: std::cell::RefCell::default(),
            stack_start_cache: std::cell::RefCell::default(),
            brace_or_newline_cache: std::cell::Cell::new(None),
            operand_return_cache: std::cell::Cell::new(None),
            paren_braces_cache: std::cell::Cell::new(None),
            case_label_cache: std::cell::Cell::new(None),
            assignment_rhs_cache: std::cell::Cell::new(None),
            row_openers: std::cell::RefCell::default(),
            operand_assign_cache: std::cell::Cell::new(None),
            stacked_assignment_cache: std::cell::Cell::new(None),
            declarator_registers_cache: std::cell::Cell::new(None),
            declarator_start_cache: std::cell::Cell::new(None),
            astyle_replay_cache: std::cell::Cell::new(None),
            template_openers: std::cell::OnceCell::new(),
            bracket_closes: std::cell::OnceCell::new(),
            designator_chain_cache: std::cell::RefCell::new(None),
            case_labels: std::cell::OnceCell::new(),
            constructor_colon_cache: std::cell::Cell::new(None),
            layout: LayoutState {
                indentation: IndentationState::default(),
                command_state: CommandState::default(),
                nesting: NestingState::default(),
                frame_stack: FrameStack::default(),
                line_state: LineState::default(),
                run_in_state: RunInState::default(),
                line_adjuster: line_adjust::LineAdjuster::new(options),
                previous_pre_adjust_line: None,
                pending_member_spacing: None,
                previous: PreviousToken::None,
                previous_before_comment: None,
                literal_line: literals::LiteralLineState::default(),
                continuation_indent: continuation::ContinuationIndentState::default(),
                objc: objc::ObjCLineState::default(),
                switch_case_layout: SwitchCaseLayoutState::default(),
                in_class_base_clause: false,
                split_class_export_pending_base: false,
                template_declaration: TemplateDeclarationState::default(),
                else_if_break_depths: Vec::new(),
                unwind_else_if_after_line: false,
                compound_literal: compound_literals::CompoundLiteralState::default(),
                pending_braceless_block_bias: None,
                inline_nested_header_braceless_bias: None,
            },
            current: CurrentLine::default(),
            line_brace_match_start: 0,
            line_brace_matches: Vec::new(),
            previous_was_newline: false,
            previous_was_template_close: false,
            newline_breaks_statement: false,
            preserve_block_spacing_comment_blank: false,
            next_line: next_line::NextLineState::default(),
            multi_declarator_indent_spaces: None,
            block_spacing: BlockSpacingState::default(),
            comments: CommentState::default(),
            source_run_in_brace_lines: Vec::new(),
            disabled_formatting: None,
            current_is_preindented: false,
            current_is_verbatim: false,
            adjuster_before_lone_brace: None,
            unmatched_closing_brace_recovery: false,
            preserve_run_in_join_space: false,
            one_line_block_mode: false,
            one_line_block_after_directive: false,
            inline_array: InlineArrayState::default(),
            max_length_line: MaxLengthLineState::default(),
            backslash_body: BackslashBodyState::default(),
            swig: SwigState::default(),
            may_have_class_base_access: true,
            may_have_noexcept: true,
            space_after_cast: false,
            pad_close_paren_pending: false,
            header_paren: headers::HeaderParenState::default(),
            current_line_has_class_initializer_colon: false,
            token_input: TokenInputState::default(),
            pointer_run: pointers::PointerRunState::default(),
            preprocessor: preprocessor::PreprocessorState::default(),
            access_modified_braces: IndexSet::default(),
            added_closing_braces: IndexSet::default(),
            added_opener_overruns: IndexMap::default(),
            syntax_roles: SyntaxRoles::new(0),
            tree: SourceTree::default(),
            pending_extern: false,
            extern_c_guard: ExternCGuard::Idle,
        }
    }

    pub(crate) fn current_char_len(&self) -> usize {
        self.current.char_len()
    }

    pub(crate) fn current_is_blank(&self) -> bool {
        self.current.is_blank()
    }

    pub(crate) fn current_visual_width(&self) -> usize {
        self.current.visual_width(self.options.tab_width)
    }

    pub(crate) fn current_visual_width_from(&self, start_column: usize) -> usize {
        self.current
            .visual_width_from(start_column, self.options.tab_width)
    }

    pub(crate) fn current_last_open_brace(&self) -> Option<usize> {
        self.current.last_open_brace()
    }

    pub(crate) fn current_trailing_comment_split_limit(&self) -> usize {
        self.current.trailing_comment_split_limit()
    }

    pub(crate) fn take_current(&mut self) -> String {
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

    pub(crate) fn matching_brace_on_current_line(&self, open_index: usize) -> Option<usize> {
        let offset = open_index.checked_sub(self.line_brace_match_start)?;
        self.line_brace_matches.get(offset).copied().flatten()
    }

    pub(crate) fn clear_current(&mut self) {
        self.current.clear();
    }

    pub(crate) fn reset_after_finished_line(&mut self) {
        self.clear_current();
        self.current_is_preindented = false;
        self.layout.literal_line.is_multiline_literal = false;
        self.layout.literal_line.multiline_literal_end = None;
        self.layout.literal_line.unterminated_raw_literal = false;
        self.current_line_has_class_initializer_colon = false;
        self.layout.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    #[cfg(test)]
    pub(crate) fn format_into(self, tokens: &[Token]) -> Self {
        self.format_owned(tokens.to_vec())
    }

    pub(crate) fn format_owned(mut self, tokens: Vec<Token>) -> Self {
        let tokens = if self.options.remove_braces {
            remove_cross_line_statement_braces(&tokens)
        } else {
            tokens
        };
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
            let added = add_marked_cross_line_statement_braces(
                tokens,
                attach_added_braces,
                matches!(
                    self.options.brace_style,
                    BraceStyle::Pico | BraceStyle::Lisp
                ),
                self.options.indent_width,
            );
            self.added_closing_braces = added.closers;
            self.added_opener_overruns = added.opener_overruns;
            added.tokens
        } else {
            tokens
        };
        let tokens = std::rc::Rc::new(tokens);
        self.tree = SourceTree::build_shared(std::rc::Rc::clone(&tokens));
        let tokens = tokens.as_slice();
        self.syntax_roles = classify_syntax(tokens, &self.tree);
        // Most output lines are source lines.
        self.output.reserve(
            tokens
                .iter()
                .filter(|token| matches!(token, Token::Newline))
                .count()
                + 1,
        );
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
        if !self.formatting_disabled()
            && self.try_push_case_line_marker(tokens, line.start, line.end)
        {
            return;
        }
        if !self.formatting_disabled()
            && self.try_push_generated_case_compact_action_line(tokens, line.start, line.end)
        {
            return;
        }
        if !self.formatting_disabled()
            && self.try_push_raw_standalone_macro_line(tokens, line.start, line.end)
        {
            return;
        }
        let mut index = line.start;
        let mut line_columns = std::mem::take(&mut self.line_columns);
        fill_line_source_columns(
            &mut line_columns,
            self.options,
            &tokens[line.start..line.end],
        );
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
                    self.layout.run_in_state.adjuster_observed_line_count + 1,
                    index - line.start,
                    line.end - line.start
                );
            }
            if self.formatting_disabled() {
                let next = next_non_whitespace(tokens, index + 1, line.end)
                    .and_then(|next_index| tokens.get(next_index));
                self.push_disabled(&tokens[index], next);
                index += 1;
                continue;
            }
            // Only the comment right after an added block follows it.
            if is_code_token(&tokens[index]) {
                self.comments.follows_added_one_line_block = false;
            }
            // Rewrites below push runs of tokens; their text belongs to the
            // token they start from.
            self.current
                .set_active_token(is_code_token(&tokens[index]).then_some(index));
            if let Some(next_index) =
                self.try_add_braces_to_statement(tokens, line.start, index, line.end)
            {
                self.current.set_active_token(None);
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.try_push_one_line_defer_block(tokens, index, line.end) {
                self.current.set_active_token(None);
                index = next_index;
                continue;
            }
            if self.try_break_one_line_header(tokens, line.start, index, line.end) {
                self.current.set_active_token(None);
                continue;
            }
            self.try_break_else_if(tokens, index);
            if let Some(next_index) = self.try_remove_braces_from_statement(tokens, index, line.end)
            {
                self.current.set_active_token(None);
                index = next_index;
                continue;
            }
            if let Some(next_index) =
                self.try_push_one_line_initializer_block(tokens, index, line.start, line.end)
            {
                self.current.set_active_token(None);
                index = next_index;
                continue;
            }
            if let Some(next_index) = self.try_push_kept_one_line_block(tokens, index, line.end) {
                self.current.set_active_token(None);
                index = next_index;
                continue;
            }
            self.current.set_active_token(None);
            if matches!(tokens[index], Token::Newline)
                && self.try_break_braceless_header_body(tokens, index)
            {
                index += 1;
                continue;
            }
            if matches!(tokens[index], Token::Newline) {
                self.observe_next_line_lead(tokens, index, line.start);
            }
            let context = self.token_push_context(tokens, index, line, &line_columns);
            self.current
                .set_active_token(is_code_token(&tokens[index]).then_some(index));
            let block_comment =
                matches!(tokens[index], Token::Comment(CommentKind::Block, _)).then_some(index);
            self.current.set_active_comment(block_comment);
            self.output.set_active_comment(block_comment);
            self.preprocessor.active_directive =
                matches!(tokens[index], Token::Preprocessor(_)).then_some(index);
            self.push_token(&tokens[index], context);
            self.current.set_active_token(None);
            self.current.set_active_comment(None);
            self.output.set_active_comment(None);
            if let Some((colon_index, has_action)) = multiline_case_colon
                && colon_index == index
            {
                self.finish_multiline_case_label_colon(has_action);
            }
            index += 1;
        }
        self.line_columns = line_columns;
    }

    /// Records how the next source line starts, before the newline token is pushed.
    fn observe_next_line_lead(&mut self, tokens: &[Token], index: usize, line_start: usize) {
        let following_index = self.next_non_layout_token_index(0, tokens, index + 1);
        let following = following_index.map(|i| &tokens[i]);
        let after_following = following_index
            .and_then(|i| self.next_non_layout_token_index(1, tokens, i + 1))
            .map(|i| &tokens[i]);
        // Only a removed brace leaves a gap at a line end; astyle drops the
        // source's own trailing whitespace.
        if self.options.remove_braces
            && matches!(
                self.options.brace_style,
                BraceStyle::Pico | BraceStyle::Lisp
            )
            && matches!(following, Some(Token::Symbol('}')))
            && previous_non_whitespace(tokens, index, line_start).is_some()
            && let Some(Token::Whitespace(whitespace)) = tokens.get(index.wrapping_sub(1))
        {
            if self.current_is_blank() {
                if let Some(previous) = self.output.last_mut()
                    && !line_ends_with_comment(previous)
                {
                    previous.truncate(previous.trimmed_end().len());
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
            matches!(following, Some(Token::Symbol(':'))) && self.colon_leads_class_initializer();
        self.next_line.leads_with_class_base = matches!(following, Some(Token::Symbol(':')))
            && !self.colon_leads_class_initializer()
            && self.colon_leads_class_base_clause();
        self.next_line.leads_with_comma = matches!(following, Some(Token::Symbol(',')));
        self.next_line.leads_with_open_brace = matches!(following, Some(Token::Symbol('{')));
        self.next_line.leads_with_close_brace = matches!(following, Some(Token::Symbol('}')));
        self.next_line.leads_with_else =
            matches!(following, Some(Token::Word(word)) if word == "else");
        self.next_line.leads_with_open_paren = matches!(following, Some(Token::Symbol('(')));
        self.next_line.word_followed_by_open_paren = matches!(
            (following, after_following),
            (Some(Token::Word(_)), Some(Token::Symbol('(')))
        );
        self.next_line.leads_with_noexcept = matches!(
            (following, after_following),
            (Some(Token::Word(word)), Some(Token::Symbol('('))) if word == "noexcept"
        );
    }

    /// `next_non_layout_token_index`, answered from the last read in `run`
    /// when it started inside the same run of blanks and newlines.
    fn next_non_layout_token_index(
        &mut self,
        run: usize,
        tokens: &[Token],
        start: usize,
    ) -> Option<usize> {
        if let Some((from, found)) = self.layout_runs[run]
            && (from..=found.unwrap_or(tokens.len())).contains(&start)
        {
            return found;
        }
        let found = next_non_layout_token_index(tokens, start);
        self.layout_runs[run] = Some((start, found));
        found
    }

    /// The `]` closing the `[` at `open` before `end`.
    fn bracket_close_before(&self, open: usize, end: usize) -> Option<usize> {
        let pairs = self
            .bracket_closes
            .get_or_init(|| matching_close_pairs(&self.tree.tokens, '[', ']'));
        let at = pairs
            .binary_search_by_key(&open, |&(opener, _)| opener as usize)
            .ok()?;
        Some(pairs[at].1 as usize).filter(|&close| close < end)
    }

    /// The `case` and `default` words after `start` and before `end`, last
    /// first.
    pub(crate) fn case_labels_between(
        &self,
        start: usize,
        end: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let labels = self.case_labels.get_or_init(|| {
            self.tree
                .tokens
                .iter()
                .enumerate()
                .filter(|(_, token)| matches!(token, Token::Word(word) if matches!(word.as_str(), "case" | "default")))
                .map(|(index, _)| index as u32)
                .collect()
        });
        let from = labels.partition_point(|&index| (index as usize) <= start);
        let to = labels.partition_point(|&index| (index as usize) < end);
        labels[from..to.max(from)]
            .iter()
            .rev()
            .map(|&index| index as usize)
    }

    /// Whether the `[` at `open` starts an initializer designator. Each `[`
    /// of a chain of subscripts has the chain's answer, kept for the next.
    fn bracket_chain_designator(&self, tokens: &[Token], open: usize, end: usize) -> bool {
        let key = (tokens.as_ptr() as usize, tokens.len(), end);
        let mut cache = self.designator_chain_cache.borrow_mut();
        if let Some(chain) = cache.as_ref()
            && chain.key == key
            && chain.opens.binary_search(&open).is_ok()
        {
            return chain.answer;
        }
        let mut opens = Vec::new();
        let answer = initializers::bracket_chain_starts_initializer_designator(
            tokens,
            open,
            end,
            |open| self.bracket_close_before(open, end),
            |open| opens.push(open),
        );
        *cache = Some(DesignatorChain { key, opens, answer });
        answer
    }

    fn template_opener_at(&self, index: usize) -> bool {
        self.template_openers
            .get_or_init(|| template_openers(&self.tree.tokens))
            .get(index)
            .copied()
            .unwrap_or(false)
    }

    fn token_push_context<'t>(
        &mut self,
        tokens: &'t [Token],
        index: usize,
        line: TokenLine,
        line_columns: &LineSourceColumns,
    ) -> TokenPushContext<'t> {
        let next_index = next_non_whitespace(tokens, index + 1, line.end);
        let next = next_index.and_then(|next_index| tokens.get(next_index));
        let next_is_adjacent = tokens
            .get(index + 1)
            .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
        let following_operator = following_operator_after_next_word(tokens, index + 1, line.end);
        self.set_input_whitespace(tokens, index, line.start);
        let offset = index - line.start;
        self.token_input.token_begins_source_line = line_columns
            .first_non_ws
            .is_none_or(|first| first >= offset);
        if self.options.align_method_colon
            && self.layout.objc.colon_align.is_none()
            && self.token_input.token_begins_source_line
            && token_starts_objc_method_definition(tokens, index, line.end)
        {
            self.layout.objc.colon_align = self.compute_objc_method_colon_align(tokens, index);
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
            && self.layout.nesting.paren_depth == 0
            && source_column
                > ContinuationIndent::Level(
                    self.layout.indentation.indent()
                        + self.case_body_indent_extra(LineKind::Normal),
                )
                .columns(self.options.indent_width)
            && source_indented_macro_row(tokens, line.start, line.end, index)
            && matches!(
                self.layout.nesting.brace_header_stack.last(),
                Some(Some(header)) if header == "switch"
            )
            && !matches!(
                self.layout.command_state.current_header.as_deref(),
                Some("case" | "default")
            )
            && !self
                .output
                .last()
                .is_some_and(|line| operators::head_ends_binary_operator(line.trimmed_end()))
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(source_column);
        }
        self.pointer_run.gap_before_column =
            Some(if self.token_input.previous_input_whitespace.is_some() {
                last_token_start
            } else {
                source_column
            });
        self.token_input.token_line_opens_with_brace = non_ws_count == 1 && first_non_ws_is_brace;
        let no_token_after_comment = |start| {
            next_non_whitespace(tokens, start, line.end)
                .is_none_or(|index| matches!(tokens.get(index), Some(Token::Newline)))
        };
        let token_followed_by_line_comment = next_index.is_some_and(|next_index| {
            let direct_line_comment = matches!(
                tokens.get(next_index),
                Some(Token::Comment(CommentKind::Line, _))
            ) && no_token_after_comment(next_index + 1);
            let after_block_comment = matches!(
                tokens.get(next_index),
                Some(Token::Comment(CommentKind::Block, comment)) if !comment.contains('\n')
            ) && next_non_whitespace(tokens, next_index + 1, line.end)
                .is_some_and(|after_block| {
                    matches!(
                        tokens.get(after_block),
                        Some(Token::Comment(CommentKind::Line, _))
                    ) && no_token_after_comment(after_block + 1)
                });
            direct_line_comment || after_block_comment
        });
        self.token_input.token_followed_by_line_comment_on_line = token_followed_by_line_comment;
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
        let mut template_angle = known_template_angle_role(
            tokens,
            index,
            self.layout.line_state.template_angle_depth,
            || self.template_opener_at(index),
        );
        if matches!(template_angle, TemplateAngle::None)
            && self.template_continuation_active()
            && matches!(tokens.get(index), Some(Token::Operator(operator)) if operator == "<")
            && next_index.is_some()
            && !matches!(next, Some(Token::Operator(operator)) if operator == "=")
            && previous_non_whitespace(tokens, index, line.start).is_some_and(|previous| {
                matches!(tokens.get(previous), Some(Token::Word(word)) if word != "operator")
            })
        {
            template_angle = TemplateAngle::Open;
        }
        self.comments.next_comment_ends_line = next_index.is_some_and(|comment_index| {
            matches!(tokens.get(comment_index), Some(Token::Comment(_, _)))
                && next_non_whitespace(tokens, comment_index + 1, line.end)
                    .is_none_or(|after| matches!(tokens.get(after), Some(Token::Newline)))
        });
        let starts_initializer_designator = matches!(tokens[index], Token::Symbol('['))
            && self.bracket_chain_designator(tokens, index, line.end);
        let inferred_definition_brace = matches!(tokens[index], Token::Symbol('{'))
            && self.inferred_definition_brace(tokens, index);
        let following_closer_width =
            closer_width_after_semicolon(tokens, index, self.options.attach_closing_while).0;
        TokenPushContext {
            next,
            next_is_adjacent,
            following_operator,
            template_angle,
            token_index: index,
            starts_initializer_designator,
            inferred_definition_brace,
            following_closer_width,
        }
    }

    fn finish_multiline_case_label_colon(&mut self, has_action: bool) {
        if let Some(byte_index) = self.current.rfind(':') {
            self.layout.line_adjuster.mark_case_label_colon(byte_index);
        }
        if has_action && self.options.break_one_line_statements {
            self.finish_line();
            let label_spaces = self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| {
                    line.trimmed_start()
                        .strip_prefix("case")
                        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace))
                })
                .map(|line| columns::leading_visual_width(line, self.options.tab_width))
                .unwrap_or_else(|| {
                    self.layout
                        .indentation
                        .line_indent(LineKind::SwitchLabel, self.options)
                        * self.options.indent_width
                });
            self.layout
                .continuation_indent
                .set_next_line_spaces(label_spaces + self.options.indent_width);
        }
    }

    pub(crate) fn operator_role_at(&self, token_index: usize) -> OperatorRole {
        self.syntax_roles.operator_role_at(token_index)
    }

    fn try_push_case_line_marker(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        line_end: usize,
    ) -> bool {
        if !switch_cases::tokens_may_start_label(&tokens[line_start..line_end]) {
            return false;
        }
        let line = tokens[line_start..line_end]
            .iter()
            .filter(|token| !matches!(token, Token::Newline))
            .map(token_text)
            .collect::<String>();
        let trimmed = line.trimmed();
        let Some(colon) = switch_cases::find_case_colon(trimmed) else {
            return false;
        };
        let marker = trimmed[colon + 1..].trimmed_start();
        if !marker.starts_with("#line") {
            return false;
        }
        self.finish_line();
        self.finish_line_text(&trimmed[..=colon]);
        self.adjust_and_publish_line(marker.to_string());
        self.preprocessor.last_output_was_preprocessor = true;
        self.layout.previous = PreviousToken::Other;
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
        let trimmed = line.trimmed();
        if !trimmed.starts_with("{{")
            || !trimmed.ends_with('}')
            || !trimmed.contains_from_first_byte("}{")
        {
            return false;
        }
        let mut lines = self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty());
        if !lines
            .next()
            .is_some_and(|line| line.trimmed_start().starts_with('#'))
        {
            return false;
        }
        let Some(case_line) = lines.next() else {
            return false;
        };
        let case_trimmed = case_line.trimmed_start();
        if !(case_trimmed.starts_with("case ") || case_trimmed.starts_with("default"))
            || !case_trimmed.ends_with(':')
        {
            return false;
        }
        let case_body_spaces = columns::leading_visual_width(case_line, self.options.tab_width)
            + self.options.indent_width * 2;
        self.finish_line();
        self.push_output_line_spaces(trimmed, self.layout.indentation.indent(), case_body_spaces);
        self.layout.previous = PreviousToken::None;
        self.previous_was_newline = true;
        true
    }

    fn observe_input_line(&mut self, tokens: &[Token]) {
        self.layout
            .continuation_indent
            .input_line_continuation_indent = self
            .layout
            .continuation_indent
            .next_input_line_continuation_indent
            .take();
        self.layout.line_state.passed_semicolon = false;
        self.layout.line_state.passed_colon = false;
        self.layout.line_state.ternary_colon = false;
        self.layout.line_state.is_multi_statement_line = false;
        self.layout.line_state.is_one_line_block = false;
        self.layout.line_state.column1_line_comment = {
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
        // The scans below find nothing on a line without the tokens they
        // look for.
        let (mut has_comment, mut has_literal, mut has_open_brace, mut has_semicolon) =
            (false, false, false, false);
        for token in tokens {
            match token {
                Token::Comment(..) => has_comment = true,
                Token::StringLiteral(_) | Token::CharLiteral(_) => has_literal = true,
                Token::Symbol('{') => has_open_brace = true,
                Token::Symbol(';') => has_semicolon = true,
                _ => {}
            }
        }
        self.layout.line_state.has_literal_quote = has_literal;
        self.layout.line_state.indent_off_follows_code =
            has_comment && preprocessor::indent_off_follows_code(tokens);
        self.layout.line_state.operator_padding_disabled = has_comment
            && tokens.iter().any(
                |token| matches!(token, Token::Comment(_, comment) if comment.contains("*NOPAD*")),
            );
        self.layout.line_state.in_class_initializer = false;
        let trailing_comment_columns = if has_comment {
            trailing_comment_columns(tokens)
        } else {
            Vec::new()
        };
        self.token_input.input_source_indent = 0;
        let tab_width = self.options.tab_width.max(1);
        let mut source_indent = 0;
        for token in tokens {
            match token {
                Token::Whitespace(value) if value.bytes().all(|byte| byte == b' ') => {
                    source_indent += value.len();
                }
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
        self.layout.line_state.trailing_comment_columns = trailing_comment_columns.into();
        self.layout.line_state.has_nested_designated_init_brace =
            has_open_brace && initializers::has_nested_designated_init_brace(tokens);

        if has_semicolon {
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
                self.layout.line_state.is_multi_statement_line = true;
            }
        }

        if !has_open_brace {
            return;
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
                    self.layout.line_state.is_one_line_block = saw_open;
                    depth -= 1;
                }
                _ => {}
            }
        }
    }

    pub(crate) fn push_token(&mut self, token: &Token, context: TokenPushContext<'_>) {
        let next = context.next;
        if self.formatting_disabled() {
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

        if self.comments.skip_next_attached_comment && matches!(token, Token::Comment(_, _)) {
            self.comments.skip_next_attached_comment = false;
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
                    let unpadded_close_paren =
                        self.options.unpad_parens && matches!(token, Token::Symbol(')'));
                    if !unpadded_close_paren
                        && !symbols::close_paren_out_suppressed(token)
                        && self
                            .token_input
                            .previous_input_whitespace
                            .as_ref()
                            .is_none_or(|ws| ws.is_empty())
                    {
                        self.token_input.previous_input_whitespace = Some(" ".to_string().into());
                    }
                    self.pad_close_paren_pending = false;
                }
            }
        }

        if let Some(pad) = self.layout.objc.after_paren_pad {
            match token {
                Token::Whitespace(_) => {}
                Token::Newline => self.layout.objc.after_paren_pad = None,
                _ => {
                    self.token_input.previous_input_whitespace =
                        Some(if pad { " ".into() } else { "".into() });
                    self.layout.objc.after_paren_pad = None;
                }
            }
        }

        if !matches!(token, Token::Whitespace(_) | Token::Newline) {
            self.pointer_run.template_close_before_current = self.previous_was_template_close;
            self.previous_was_template_close = false;
        }

        if !matches!(
            token,
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
        ) {
            self.header_paren.post_paren = self.header_paren.just_closed;
            self.header_paren.just_closed = false;
        }

        self.track_extern_c_guard(token);

        match token {
            Token::Word(word) => self.push_word(word, next),
            Token::Number(number) => self.push_literal(number, None),
            Token::StringLiteral(literal) => self.push_literal(literal, Some('"')),
            Token::CharLiteral(literal) => self.push_literal(literal, Some('\'')),
            Token::Comment(kind, comment) => {
                let before = self
                    .layout
                    .previous_before_comment
                    .unwrap_or(self.layout.previous);
                self.push_comment(*kind, comment);
                if self.layout.previous == PreviousToken::Other {
                    self.layout.previous_before_comment = Some(before);
                }
                return;
            }
            Token::Preprocessor(line) => {
                self.push_preprocessor(&line.text, &line.opaque_literal_line_ranges)
            }
            Token::RawLine(line) => self.push_raw_line(line),
            Token::Operator(operator) => self.push_operator(operator, context),
            Token::Symbol(symbol) => self.push_symbol(*symbol, context),
            Token::Whitespace(whitespace) => self.push_whitespace(whitespace),
            Token::Newline => self.push_newline(),
        }
        if !matches!(token, Token::Whitespace(_) | Token::Newline) {
            self.layout.previous_before_comment = None;
        }
    }

    fn push_raw_line(&mut self, line: &str) {
        if !self.current.trimmed().is_empty() {
            self.finish_line();
        }
        let published = self.output.len();
        self.adjust_and_publish_line(line.to_string());
        if self.output.len() > published {
            self.output.mark_last_verbatim();
        }
        self.layout.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    fn push_whitespace(&mut self, whitespace: &str) {
        if self.layout.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
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
        } else if whitespace.contains('\x0c') && self.current.trimmed().is_empty() {
            self.current.push('\x0c');
            self.current_is_preindented = true;
        }
    }

    pub(crate) fn set_input_whitespace(&mut self, tokens: &[Token], index: usize, lower: usize) {
        self.token_input.previous_input_was_adjacent = index > lower
            && tokens
                .get(index - 1)
                .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
        self.token_input.previous_input_whitespace = (index > lower)
            .then(|| tokens.get(index - 1))
            .flatten()
            .filter(|_| !matches!(tokens.get(index.wrapping_sub(2)), Some(Token::Newline)))
            .and_then(|token| match token {
                Token::Whitespace(ws) => Some(shared_whitespace(ws)),
                _ => None,
            });
        self.token_input.next_input_whitespace =
            tokens.get(index + 1).and_then(|token| match token {
                Token::Whitespace(ws) => Some(shared_whitespace(ws)),
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
            // Each operator of a run ends where the first did.
            let address = tokens.as_ptr() as usize;
            let last = match self.pointer_run.run_end {
                Some((cached_address, from, last))
                    if cached_address == address && (from..=last).contains(&index) =>
                {
                    last
                }
                _ => {
                    let mut last = index;
                    while let Some(Token::Operator(next_operator)) = tokens.get(last + 1)
                        && next_operator == operator
                    {
                        last += 1;
                    }
                    self.pointer_run.run_end = Some((address, index, last));
                    last
                }
            };
            self.pointer_run.star_count = (last - index + 1) * operator.chars().count();
            self.pointer_run.trailing_ws = match tokens.get(last + 1) {
                Some(Token::Whitespace(ws)) => Some(ws.to_string()),
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
            .layout
            .continuation_indent
            .pending_literal_continuation_indent_spaces
            .take()
        else {
            return;
        };
        if self.current.trimmed().is_empty() && self.layout.line_state.has_literal_quote {
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        }
    }

    /// `self.open_paren_column_of(text)`, read from the cache of the
    /// output line whose code `text` is.
    pub(crate) fn open_paren_column_of(&self, text: &str) -> Option<usize> {
        match self.output.code_line_of(text) {
            Some((index, offset)) => self
                .output
                .unmatched_open_paren_column(index)
                .map(|column| column - offset),
            None => unmatched_open_paren_column(text),
        }
    }

    /// `self.paren_imbalance_of(text)`, read from the cache of the output line
    /// whose code `text` is.
    pub(crate) fn paren_imbalance_of(&self, text: &str) -> (usize, Vec<usize>) {
        match self.output.code_line_of(text) {
            Some((index, offset)) => {
                let (closes, opens) = self.output.paren_imbalance(index);
                (closes, opens.iter().map(|column| column - offset).collect())
            }
            None => line_paren_imbalance(text),
        }
    }

    /// `paren_closes_of(text)` and `open_paren_column_of(text)`, scanning
    /// the text once.
    pub(crate) fn paren_closes_and_open_column_of(&self, text: &str) -> (usize, Option<usize>) {
        if self.output.code_line_of(text).is_some() {
            return (self.paren_closes_of(text), self.open_paren_column_of(text));
        }
        let (closes, opens) = line_paren_imbalance(text);
        let open = opens
            .into_iter()
            .rev()
            .find(|&column| text[column + 1..].chars().any(|ch| !ch.is_whitespace()));
        (closes, open)
    }

    /// `paren_imbalance_of(text).0`, without collecting the open parens.
    pub(crate) fn paren_closes_of(&self, text: &str) -> usize {
        match self.output.code_line_of(text) {
            Some((index, _)) => self.output.paren_imbalance(index).0,
            None => line_paren_imbalance(text).0,
        }
    }

    /// Whether `text` leaves a paren open, as `paren_imbalance_of` finds.
    pub(crate) fn leaves_paren_open(&self, text: &str) -> bool {
        match self.output.code_line_of(text) {
            Some((index, _)) => !self.output.paren_imbalance(index).1.is_empty(),
            None => !line_paren_imbalance(text).1.is_empty(),
        }
    }

    /// The current line's code before its trailing comment, trimmed.
    pub(crate) fn current_code_before_trailing_comment(&self) -> &str {
        let current = self.current.trimmed_end();
        if self.current.holds_comment_opener() {
            &current[..crate::formatter::text::line_scan::trailing_comment_split_limit(current)]
        } else {
            current
        }
        .trimmed_end()
    }

    /// `paren_imbalance_of` the code of output line `index` before its
    /// trailing comment, trimmed: the line's cached imbalance when the code
    /// is all of it or the line is recent.
    pub(crate) fn output_code_paren_imbalance(&self, index: usize) -> (usize, Vec<usize>) {
        let code = self.output.code_before_comment(index).trimmed_end();
        if index + 16 >= self.output.len()
            || code.len() == self.output.as_slice()[index].trimmed_end().len()
        {
            let (closes, opens) = self.output.paren_imbalance(index);
            (closes, opens.to_vec())
        } else {
            line_paren_imbalance(code)
        }
    }

    /// `leaves_paren_open` of the code of output line `index`.
    pub(crate) fn output_code_leaves_paren_open(&self, index: usize) -> bool {
        if index + 16 >= self.output.len() {
            !self.output.paren_imbalance(index).1.is_empty()
        } else {
            !line_paren_imbalance(self.output.code(index)).1.is_empty()
        }
    }

    pub(crate) fn current_ends_cast(&self) -> bool {
        self.current_cast_words()
            .is_some_and(|words| words.iter().any(|word| is_type_like_pointer_word(word)))
    }

    pub(crate) fn current_statement_contains_assignment(&self) -> bool {
        self.current.statement_tail().contains('=')
    }

    pub(crate) fn current_ends_numeric_cast(&self) -> bool {
        self.current_cast_words()
            .and_then(|words| words.last().copied())
            .is_some_and(|word| is_numeric_variable_word(word) || is_unpad_kept_type_word(word))
    }

    pub(crate) fn current_ends_pointer_cast(&self) -> bool {
        let current = self.current.trimmed_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = self.current.last_close_paren_match() else {
            return false;
        };
        current[open + 1..current.len() - 1]
            .trimmed_end()
            .ends_with_any(b"*&^")
    }

    pub(crate) fn current_ends_sizeof_pointer_expr(&self) -> bool {
        let current = self.current.trimmed_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = self.current.last_close_paren_match() else {
            return false;
        };
        trailing_word(current[..open].trimmed_end()) == "sizeof"
            && current[open + 1..current.len() - 1]
                .trimmed_end()
                .ends_with_any(b"*&^")
    }

    pub(crate) fn current_ends_size_operator_call(&self) -> bool {
        let current = self.current.trimmed_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = self.current.last_open_paren() else {
            return false;
        };
        matches!(
            trailing_word(current[..open].trimmed_end()),
            "sizeof" | "alignof" | "_Alignof"
        )
    }

    pub(crate) fn current_paren_started_by_expression_keyword(&self) -> bool {
        let Some(mut open) = self.current.last_open_paren() else {
            return false;
        };
        loop {
            let prefix = self.current[..open].trimmed_end();
            if !prefix.ends_with('(') {
                let word = trailing_word(prefix);
                return matches!(
                    word,
                    "if" | "while" | "for" | "switch" | "return" | "sizeof"
                );
            }
            open = prefix.len() - 1;
        }
    }

    pub(crate) fn current_paren_started_by_catch(&self) -> bool {
        let Some(mut open) = self.current.last_open_paren() else {
            return false;
        };
        loop {
            let prefix = self.current[..open].trimmed_end();
            if !prefix.ends_with('(') {
                return trailing_word(prefix) == "catch";
            }
            open = prefix.len() - 1;
        }
    }

    pub(crate) fn current_paren_is_expression_context(&self) -> bool {
        let Some(open) = self.current.last_open_paren() else {
            return false;
        };
        let prefix = self.current[..open].trimmed_end();
        self.current_paren_started_by_expression_keyword()
            || prefix.chars().any(|ch| {
                matches!(
                    ch,
                    '?' | '=' | '<' | '>' | '+' | '-' | '/' | '%' | '|' | '&' | '^'
                )
            })
    }

    fn current_cast_words(&self) -> Option<Vec<&str>> {
        let current = self.current.trimmed_end();
        if !current.ends_with(')') {
            return None;
        }
        let open = self.current.last_close_paren_match()?;
        if current[..open]
            .chars()
            .next_back()
            .is_some_and(is_identifier_continue)
        {
            return None;
        }
        if matches!(
            trailing_word(current[..open].trimmed_end()),
            "sizeof" | "alignof" | "_Alignof"
        ) {
            return None;
        }
        let inner = &current[open + 1..current.len() - 1];
        // No type starts with a parenthesis: `((int)x)` holds a cast and its
        // operand, `((x))` a group.
        if inner.trimmed_start().starts_with('(') {
            return None;
        }
        // `(f(x) y)` holds a call, no type.
        if inner.contains(')') && !inner.trimmed_end().ends_with(')') {
            return None;
        }
        if inner.chars().any(|ch| {
            matches!(
                ch,
                '+' | '-' | '/' | '%' | '|' | '&' | '^' | '=' | '<' | '>' | '?' | ':'
            )
        }) || (inner.contains('*') && !inner.trimmed_end().ends_with('*'))
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
        self.layout.objc.post_prefix = false;
        self.layout.objc.post_method_colon = false;
        self.layout.objc.return_paren_depth = None;
        self.layout.objc.param_paren_depth = None;
        self.end_incomplete_control_header_at_newline();
        if self.current_is_preindented && self.current.contains('\x0c') {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.current.trimmed().is_empty() {
            if self.next_line.leads_with_class_init || self.next_line.leads_with_class_base {
                self.layout.nesting.clear_continuation_indents();
                self.layout
                    .continuation_indent
                    .set_next_line_level(self.layout.indentation.indent() + 1);
                if self.next_line.leads_with_class_base {
                    self.layout.in_class_base_clause = true;
                }
            }
            if (self.previous_was_newline || self.output.is_empty())
                && self.should_preserve_input_empty_line()
            {
                self.push_empty_line();
            }
            self.previous_was_newline = true;
        } else if is_split_export_head(self.current.trimmed())
            && !self.next_line.leads_with_open_brace
        {
            self.finish_split_class_head_line();
        } else if self.previous_was_newline {
            self.finish_line();
            if self.should_preserve_input_empty_line() {
                self.push_empty_line();
            }
            self.previous_was_newline = true;
        } else if self.layout.literal_line.unterminated_literal_line {
            let next_literal_indent = self
                .current
                .rfind(['"', '\''])
                .map(|column| self.current_line_indent_spaces() + column);
            self.finish_line();
            self.layout
                .continuation_indent
                .pending_literal_continuation_indent_spaces = next_literal_indent;
            self.layout.literal_line.unterminated_literal_line = false;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_class_init {
            let header_indent = self.layout.indentation.indent();
            let follows_function_try = self.class_initializer_follows_function_try();
            self.finish_line();
            self.layout.nesting.clear_continuation_indents();
            self.layout
                .continuation_indent
                .set_next_line_level(header_indent + usize::from(!follows_function_try));
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_class_base {
            let header_indent = self.layout.indentation.indent();
            self.finish_line();
            self.layout.nesting.clear_continuation_indents();
            self.layout
                .continuation_indent
                .set_next_line_level(header_indent + 1);
            self.layout.in_class_base_clause = true;
            self.previous_was_newline = true;
        } else if self.current.trimmed_end().ends_with(',') && self.is_top_level_table_macro_row() {
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(1);
            self.previous_was_newline = true;
        } else if let Some(column) = self.current_inline_array_column()
            && self.layout.indentation.statement_depth() == 0
            && self.layout.nesting.paren_depth == 0
            && line_ends_with_comment(&self.current)
            && self.current[..self.current_trailing_comment_split_limit()]
                .trimmed_end()
                .ends_with(',')
        {
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(column);
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_comma
            && self.layout.indentation.statement_depth() > 0
            && self.current.trimmed_start().starts_with(',')
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.layout.nesting.clear_continuation_indents();
            self.layout.continuation_indent.set_next_line_spaces(spaces);
            self.previous_was_newline = true;
        } else if matches!(self.layout.previous, PreviousToken::Comma)
            && self.layout.indentation.statement_depth() == 0
            && (self.in_initializer_brace()
                || self.innermost_init_block_brace()
                || self.in_enum_declaration_brace()
                || self.current_inline_array_column().is_some())
        {
            let direct_list_sibling_column = if self.current.trimmed_end().ends_with("},")
                && !self.current.trimmed_start().starts_with('{')
            {
                let range = self.output.scoped_range();
                let start = range.start.max(range.end.saturating_sub(64));
                self.output
                    .last_line_looked(&self.named_list_open_look, start, range.end, |index| {
                        let code = self.output.code_before_comment_trimmed(index);
                        code.strip_suffix('{').is_some_and(|prefix| {
                            let prefix = prefix.trimmed();
                            !prefix.is_empty()
                                && !prefix.starts_with('{')
                                && !prefix.contains_any_byte(b"=(@")
                        })
                    })
                    .map(|index| {
                        columns::leading_visual_width(&self.output[index], self.options.tab_width)
                    })
            } else {
                None
            };
            let inline_column = self.current_inline_array_column();
            let clear_enum_continuation = self.in_enum_declaration_brace()
                && !self.current.holds_open_brace()
                && unmatched_open_paren_column(self.current.trimmed_end()).is_none();
            self.finish_line();
            if clear_enum_continuation {
                self.layout.continuation_indent.clear_next_line();
                self.layout.nesting.clear_continuation_indents();
            } else if let Some(column) = direct_list_sibling_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
                self.layout.nesting.clear_continuation_indents();
            } else if let Some(column) = inline_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
            self.previous_was_newline = true;
        } else if matches!(self.layout.previous, PreviousToken::Comma)
            && self.layout.indentation.statement_depth() == 0
            && self.multi_declarator_indent_spaces.is_some()
            && !self.in_initializer_brace()
            && !self.in_aggregate_declaration_brace()
        {
            let column = self.multi_declarator_indent_spaces;
            self.finish_line();
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = column;
            self.previous_was_newline = true;
        } else if self.is_complete_template_declaration_line() || self.is_objc_standalone_line() {
            self.finish_line();
            self.previous_was_newline = true;
        } else if (self.is_objc_method_line() || self.layout.objc.method_continuation)
            && !self.current[..self.current_trailing_comment_split_limit()]
                .trimmed_end()
                .ends_with(';')
        {
            self.finish_line();
            if self.newline_breaks_statement {
                self.layout
                    .continuation_indent
                    .set_next_line_level(self.layout.indentation.indent() + 1);
                self.layout.objc.method_continuation = true;
            } else {
                self.layout.objc.method_continuation = self.next_line.leads_with_open_brace;
            }
            self.previous_was_newline = true;
        } else if self.current.trimmed_end().ends_with('\\')
            && self.layout.nesting.paren_depth == 0
            && self.current_line_indent_spaces()
                > self.continuation_base_indent() * self.options.indent_width
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(spaces);
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()].trimmed() == ":"
            && self
                .layout
                .frame_stack
                .active_constructor_initializer()
                .is_some_and(|frame| frame.function_try)
        {
            self.finish_line();
            self.layout
                .continuation_indent
                .set_next_line_level(self.layout.indentation.indent() + 1);
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()]
            .trimmed_end()
            .ends_with(':')
            && !self.current.trimmed_start().starts_with("//")
            && !self.current[..self.current_trailing_comment_split_limit()]
                .trimmed_end()
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
            self.layout.continuation_indent.set_next_line_spaces(column);
            self.previous_was_newline = true;
        } else if self.in_enum_declaration_brace()
            && self.current.trimmed_end().ends_with(",")
            && !self.current.holds_open_brace()
            && unmatched_open_paren_column(self.current.trimmed_end()).is_none()
        {
            self.finish_line();
            self.layout.continuation_indent.clear_next_line();
            self.layout.nesting.clear_continuation_indents();
            self.previous_was_newline = true;
        } else if self.current_initializer_member_before_closing_brace() {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.unmatched_closing_brace_recovery {
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(0);
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
            self.layout.frame_stack.clear_stream_frames();
            self.layout.frame_stack.clear_logical_frames();
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
            self.previous_was_newline = true;
        } else if self.is_continuation_break()
            && !(self.current_is_preindented && self.current.trimmed_end().ends_with("*/"))
        {
            self.finish_continuation_line_at_newline();
        } else if self.current.trimmed_end().ends_with(';')
            || self.current.trimmed_end().ends_with("*/")
            || macro_invocations::is_standalone_macro_invocation_line(self.current.trimmed())
        {
            self.finish_line();
            self.layout.objc.method_continuation = false;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_open_brace && self.current.trimmed_end().ends_with('[')
        {
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
            && (self.current.trimmed_start().starts_with('}')
                || self
                    .current
                    .trimmed_start()
                    .starts_with_any(b"<>|&+-*/%=!?:,.~"))
        {
            self.finish_line();
            self.layout.continuation_indent.clear_next_line();
            self.layout.nesting.clear_continuation_indents();
            self.previous_was_newline = true;
        } else if self.current[..self.current_trailing_comment_split_limit()]
            .trimmed_end()
            .ends_with(':')
            && !self.current.trimmed_start().starts_with("//")
            && labels::is_label_start(
                self.current[..self.current_trailing_comment_split_limit()]
                    .trimmed()
                    .trim_end_matches(':'),
                &self.options.access_labels,
            )
        {
            self.finish_line();
            self.previous_was_newline = true;
        } else if self.newline_breaks_statement
            && self.header_allows_statement_break()
            && self.current.trimmed() != "else"
        {
            let bare_return = self.current.trimmed() == "return";
            let incomplete_control_header = self.incomplete_control_header();
            let header_indent = self.layout.indentation.indent();
            self.finish_line();
            if bare_return {
                self.layout
                    .continuation_indent
                    .set_next_line_level(header_indent + 1);
            } else if incomplete_control_header {
                self.layout.continuation_indent.clear_next_line();
                self.layout.pending_braceless_block_bias = None;
                self.layout.inline_nested_header_braceless_bias = None;
                self.layout.command_state.current_header = None;
                self.layout.command_state.preprocessor_after_header = false;
                self.layout.frame_stack.clear_header();
            }
            self.layout.objc.method_continuation = false;
            self.previous_was_newline = true;
        } else if self.next_line.leads_with_else {
            // A body without `;`, as a macro call, still ends before `else`.
            self.finish_line();
            self.previous_was_newline = true;
        } else {
            self.ensure_space();
            self.previous_was_newline = true;
        }
    }

    fn end_incomplete_control_header_at_newline(&mut self) {
        if self.incomplete_control_header() && !self.next_line.leads_with_open_paren {
            self.newline_breaks_statement = true;
            if !self.next_line.leads_with_close_brace {
                self.layout.continuation_indent.clear_next_line();
            }
            self.layout.pending_braceless_block_bias = None;
            self.layout.inline_nested_header_braceless_bias = None;
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.frame_stack.clear_header();
        }
    }

    fn finish_continuation_line_at_newline(&mut self) {
        let is_logical_continuation = self.logical_continuation_indent_spaces().is_some();
        let has_array_bound_operator_continuation = self
            .array_bound_operator_continuation_indent_spaces()
            .is_some();
        let after_compound_literal_comma =
            std::mem::take(&mut self.layout.compound_literal.after_comma)
                && self.layout.compound_literal.arg_paren_depth
                    == Some(self.layout.nesting.paren_depth)
                && self.layout.compound_literal.arg_brace_depth
                    == Some(self.layout.nesting.brace_header_stack.len());
        let has_macro_call_argument_continuation =
            matches!(self.layout.previous, PreviousToken::Comma)
                && !after_compound_literal_comma
                && self.macro_call_argument_indent_spaces().is_some();
        let saved_indent = if after_compound_literal_comma {
            Some(ContinuationIndent::Spaces(
                self.current_line_indent_spaces(),
            ))
        } else {
            self.layout
                .continuation_indent
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
            && unmatched_open_paren_column(self.current.trimmed_end()).is_none()
        {
            self.layout.continuation_indent.logical_chain_indent_spaces =
                Some(indent.columns(self.options.indent_width));
        }
        let previous_before_line = self.layout.previous;
        let clear_continuation_after_line = self
            .layout
            .continuation_indent
            .clear_continuation_after_line
            .is_some();
        let case_label_with_comment =
            switch_cases::case_label_with_trailing_comment(self.current.trimmed());
        self.finish_line();
        if matches!(
            previous_before_line,
            PreviousToken::Word
                | PreviousToken::Literal
                | PreviousToken::CloseParen
                | PreviousToken::CloseBracket
        ) {
            self.layout.previous = previous_before_line;
        }
        if !clear_continuation_after_line {
            if let Some(spaces) = one_shot_indent {
                self.layout
                    .continuation_indent
                    .after_one_shot_continuation_indent = Some(indent);
                self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
            } else {
                self.set_next_continuation_indent(indent);
            }
        }
        if case_label_with_comment && let Some(previous) = self.output.last() {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(
                columns::leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width,
            );
        }
        self.previous_was_newline = true;
    }

    pub(crate) fn current_initializer_member_before_closing_brace(&self) -> bool {
        if !self.next_line.leads_with_close_brace
            || !(self.in_initializer_brace()
                || self.innermost_init_block_brace()
                || self.current_inline_array_column().is_some())
        {
            return false;
        }
        let code = self.current[..self.current_trailing_comment_split_limit()].trimmed_end();
        let trimmed = code.trimmed_start();
        !trimmed.is_empty()
            && !trimmed.starts_with_any(b"#{}")
            && !code.ends_with_any(b",;\\")
            && !operators::head_ends_binary_operator(code)
            && self.open_paren_column_of(code).is_none()
    }

    fn incomplete_control_header(&self) -> bool {
        let Some(header @ ("if" | "for" | "while" | "switch")) =
            self.layout.command_state.current_header.as_deref()
        else {
            return false;
        };
        let current = self.current.trimmed();
        let code = self.current[..self.current_trailing_comment_split_limit()].trimmed();
        (trailing_word(code) == header || trailing_word(current) == header)
            && self.layout.command_state.previous_command_char != Some(')')
            && self.header_paren.depth.is_none()
    }

    fn header_allows_statement_break(&self) -> bool {
        match self.layout.command_state.current_header.as_deref() {
            None => true,
            Some("if" | "for" | "while" | "switch") => self.incomplete_control_header(),
            Some("case" | "default") => {
                let leading = self
                    .current
                    .trimmed_start()
                    .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .next()
                    .unwrap_or_default();
                !matches!(leading, "case" | "default")
            }
            Some(_) => false,
        }
    }

    fn formatting_disabled(&self) -> bool {
        self.disabled_formatting.is_some()
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
                    following_closer_width: 0,
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
                self.layout.previous_pre_adjust_line = self.output.last().cloned();
                self.reset_block_spacing();
                self.previous_was_newline = false;
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

/// Number of `}` tokens that directly follow the `;` at `index`, skipping layout.
/// The width the closing braces after a semicolon bring to its line where
/// they attach to it: a space and the brace each, then whatever follows the
/// last one on its line.
pub(crate) fn closer_width_after_semicolon(
    tokens: &[Token],
    index: usize,
    attach_while: bool,
) -> (usize, Option<String>) {
    if !matches!(tokens.get(index), Some(Token::Symbol(';'))) {
        return (0, None);
    }
    let mut cursor = index + 1;
    let mut width = 0;
    let mut after_last = cursor;
    loop {
        let mut newlines = 0;
        while matches!(
            tokens.get(cursor),
            Some(Token::Whitespace(_) | Token::Newline)
        ) {
            newlines += usize::from(matches!(tokens[cursor], Token::Newline));
            cursor += 1;
        }
        // A brace past an empty line stays on a line of its own.
        if newlines > 1 || !matches!(tokens.get(cursor), Some(Token::Symbol('}'))) {
            break;
        }
        width += 2;
        cursor += 1;
        after_last = cursor;
    }
    if width == 0 {
        return (0, None);
    }
    let mut spaced = false;
    let tail = &tokens[after_last.min(tokens.len())..];
    let header = tail
        .iter()
        .find(|token| !matches!(token, Token::Whitespace(_)))
        .and_then(|token| match token {
            Token::Word(word)
                if matches!(word.as_str(), "else" | "while" | "catch" | "finally") =>
            {
                Some(word.as_str())
            }
            _ => None,
        });
    // An attached `while` stays on the braces' line whole.
    if header == Some("while") && attach_while {
        let mut suffix = " }".repeat(width / 2);
        for token in tail {
            match token {
                Token::Whitespace(_) => spaced = true,
                Token::Newline | Token::Comment(..) => break,
                token => {
                    if spaced || suffix.ends_with('}') {
                        suffix.push(' ');
                    }
                    suffix.push_str(&token_text(token));
                    spaced = false;
                }
            }
        }
        return (suffix.len(), Some(suffix));
    }
    // A header after the braces breaks from them past the space astyle
    // pads it with.
    if header.is_some() {
        return (width + 1, None);
    }
    for token in tail {
        match token {
            Token::Whitespace(_) => spaced = true,
            Token::Newline | Token::Comment(..) | Token::Symbol('}' | '{') => break,
            token => {
                width += usize::from(spaced) + token_text(token).len();
                spaced = false;
                // The line may split after a comma of its own.
                if matches!(token, Token::Symbol(',')) {
                    break;
                }
            }
        }
    }
    (width, None)
}

fn fill_line_source_columns(
    columns: &mut LineSourceColumns,
    options: &FormatOptions,
    line_tokens: &[Token],
) {
    let tab_width = options.tab_width.max(1);
    let mut prefix = std::mem::take(&mut columns.prefix);
    let mut non_ws_prefix = std::mem::take(&mut columns.non_ws_prefix);
    prefix.clear();
    non_ws_prefix.clear();
    prefix.reserve(line_tokens.len() + 1);
    non_ws_prefix.reserve(line_tokens.len() + 1);
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
            Token::Whitespace(ws) if ws.bytes().all(|byte| byte == b' ') => column += ws.len(),
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
    *columns = LineSourceColumns {
        prefix,
        non_ws_prefix,
        first_non_ws,
        first_non_ws_is_brace,
        leading_indent,
    };
}

/// `whitespace` without an allocation when it is a run of spaces, as it
/// nearly always is.
fn shared_whitespace(whitespace: &str) -> std::borrow::Cow<'static, str> {
    const SPACES: &str = "                                                                ";
    if whitespace.len() <= SPACES.len() && whitespace.bytes().all(|byte| byte == b' ') {
        std::borrow::Cow::Borrowed(&SPACES[..whitespace.len()])
    } else {
        std::borrow::Cow::Owned(whitespace.to_owned())
    }
}
