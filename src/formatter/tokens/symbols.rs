use crate::config::{BraceStyle, FormatOptions, Mode, ObjCColonPad, PointerAlign};
use crate::formatter::braces::classification::{
    code_ends_definition_header, is_class_like_brace_type,
};
use crate::formatter::constructs::constructor_initializers::constructor_initializer_name_indent_from_line;
use crate::formatter::constructs::headers::is_header;
use crate::formatter::constructs::labels;
use crate::formatter::engine::{FormatEngine, TokenPushContext, closer_width_after_semicolon};
use crate::formatter::lexer::{CommentKind, Token};
use crate::formatter::state::frame::{
    ArgumentFrame, BraceSemanticKind, BracketFrame, BracketRole, CallFrame, ColonRole, CommaRole,
    DelimiterFrame, ParenRole, TernaryFrame, TernaryOwnerRole,
};
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::next_code_token;
use crate::formatter::syntax::language::{
    self, is_leading_continuation_operator, is_pointer_type_word, is_type_like_pointer_word,
    is_unpad_kept_type_word,
};
use crate::formatter::syntax::{
    assignment_declarator_offset, scoped_name_is_constructor, signature_ends_with_parameter_list,
};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::{is_comment_only_line, trailing_matching_parens};
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::operators::find_assignment_operator;
use crate::formatter::tokens::pointers::resolved_pointer_align;
use crate::source::lex::{is_identifier_continue, is_word_char, trailing_word};

fn should_keep_unpad_space_before_paren(word: &str, options: &FormatOptions) -> bool {
    options.unpad_parens
        && (matches!(word, language::RETURN | "and" | "or" | "in")
            || (options.pad_header
                && matches!(word, language::NEW | language::DELETE | language::THROW))
            || is_unpad_kept_type_word(word))
}

pub(crate) fn close_paren_out_suppressed(token: &Token) -> bool {
    match token {
        Token::Symbol(';' | ',' | ']' | '.') => true,
        Token::Operator(op) => {
            op == "&" || op == "^" || matches!(op.chars().next(), Some('+' | '-' | '.'))
        }
        _ => false,
    }
}

fn is_single_lvalue_assignment(line: &str) -> bool {
    if !line.contains('=') {
        return false;
    }
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'=' if depth == 0 => {
                let previous = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                if matches!(
                    previous,
                    b'=' | b'!'
                        | b'<'
                        | b'>'
                        | b'+'
                        | b'-'
                        | b'*'
                        | b'/'
                        | b'%'
                        | b'&'
                        | b'|'
                        | b'^'
                ) || next == b'='
                {
                    return false;
                }
                let head = line[..i].trimmed();
                if head.is_empty()
                    || head.contains_any_byte(b" \t,(){}")
                    || !is_word_char(head.chars().next().unwrap_or(' '))
                {
                    return false;
                }
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// What a `:` token is, decided from the current line and the token that follows.
#[derive(Clone, Copy)]
struct ColonKind {
    asm_operand: bool,
    class_initializer: bool,
    function_try_initializer: bool,
    objc_selector: bool,
    objc_interface: bool,
    enum_underlying_type: bool,
    class_base: bool,
    bit_field: bool,
    range_for: bool,
    aligned_continuation: bool,
    label: bool,
    ternary: bool,
    objc_method_definition: bool,
    role: ColonRole,
}

impl FormatEngine<'_> {
    pub(crate) fn push_symbol(&mut self, symbol: char, context: TokenPushContext<'_>) {
        let TokenPushContext {
            next,
            next_is_adjacent,
            token_index,
            starts_initializer_designator,
            inferred_definition_brace,
            following_closer_width,
            ..
        } = context;
        match symbol {
            '{' => self.push_open_brace(next, token_index, inferred_definition_brace),
            '}' => self.push_close_brace(next, next_is_adjacent),
            '(' => self.push_open_paren(next),
            ')' => self.push_close_paren(next, next_is_adjacent),
            '[' => self.push_open_bracket(next, starts_initializer_designator),
            ']' => self.push_close_bracket(),
            ';' => {
                let added_brace_follows = self.added_brace_follows(token_index);
                let following_while_suffix = self
                    .options
                    .attach_closing_while
                    .then(|| closer_width_after_semicolon(&self.tree.tokens, token_index, true).1)
                    .flatten();
                self.push_semicolon(
                    next,
                    following_closer_width,
                    following_while_suffix,
                    added_brace_follows,
                );
            }
            ',' => self.push_comma(next),
            ':' => self.push_colon(next, token_index),
            '?' => self.push_question(next),
            '.' => self.push_dot(next),
            '#' => {
                self.emit_source_space();
                self.current.push('#');
                self.emit_trailing_source_space();
                self.layout.command_state.observe_char('#');
                self.layout.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
            '@' => {
                let attached_closing_header = match next {
                    Some(Token::Word(word)) if matches!(word.as_str(), "catch" | "finally") => {
                        self.try_attach_leading_closing_header(&format!("@{word}"))
                    }
                    _ => false,
                };
                if !attached_closing_header {
                    if self.current.trimmed_end().ends_with('}')
                        || (self.options.pad_operators
                            && self.layout.previous == PreviousToken::Operator)
                    {
                        self.emit_source_space_or_ensure();
                    } else {
                        self.emit_source_space();
                    }
                }
                self.current.push('@');
                if matches!(next, Some(Token::Symbol('{'))) {
                    self.ensure_space();
                }
                self.layout.command_state.observe_char('@');
                self.layout.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
            '\\' => {
                if !self.current.ends_with_any(b" \t") {
                    self.emit_source_space();
                }
                self.current.push('\\');
                self.layout.command_state.observe_char('\\');
                self.layout.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
            _ => {
                self.current.push(symbol);
                self.layout.command_state.observe_char(symbol);
                self.layout.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
        }
    }

    fn push_dot(&mut self, next: Option<&Token>) {
        let keeps_padded_space = ((self.layout.previous == PreviousToken::Comma
            && (self.options.pad_commas || self.options.pad_operators))
            || (self.layout.previous == PreviousToken::Operator && self.options.pad_operators))
            && self.token_input.previous_input_whitespace.is_none()
            && self.current.ends_with(' ');
        if !self.current.ends_with('.')
            && self.layout.previous == PreviousToken::OpenParen
            && self.options.pad_parens_inside
        {
            self.pad_inside_paren_space();
        } else if !self.current.ends_with('.')
            && self.layout.previous == PreviousToken::Comma
            && self.options.pad_operators
        {
            self.emit_source_space_or_ensure();
        } else if !self.current.ends_with('.') && !keeps_padded_space {
            self.emit_source_space();
        }
        self.current.push('.');
        self.layout.command_state.observe_char('.');
        if self.current.ends_with("...") || !matches!(next, Some(Token::Symbol('.'))) {
            self.emit_trailing_source_space();
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    fn paren_role_for_open(
        &self,
        current_word: &str,
        opens_header_paren: bool,
        semicolonless_macro_call_indent: Option<usize>,
        handled_objc_return_paren: bool,
        handled_objc_param_paren: bool,
    ) -> ParenRole {
        if semicolonless_macro_call_indent.is_some() {
            ParenRole::SemicolonlessMacroCall
        } else if opens_header_paren {
            ParenRole::Header
        } else if handled_objc_return_paren || handled_objc_param_paren {
            ParenRole::ObjCTypeGroup
        } else if self.layout.previous == PreviousToken::Word && !current_word.is_empty() {
            ParenRole::Call
        } else {
            ParenRole::CastOrGroup
        }
    }

    fn open_paren_line_indent_spaces(&self, current_word: &str) -> usize {
        if let Some(base) = self.constructor_member_line_base_indent_spaces() {
            return base;
        }
        let base = self.current_line_indent_spaces();
        if self.token_input.token_source_line_indent <= base
            || current_word.is_empty()
            || !self.in_initializer_brace()
            || self.current.trimmed_start() != current_word
            || self.token_input.token_source_column
                != self.token_input.token_source_line_indent + current_word.chars().count()
        {
            return base;
        }
        let Some(previous) = self.output.last_non_empty_scoped() else {
            return base;
        };
        let previous_code = self.output.code_trimmed_of(previous);
        if previous_code.ends_with(',')
            && leading_visual_width(previous, self.options.tab_width)
                == self.token_input.token_source_line_indent
        {
            self.token_input.token_source_line_indent
        } else {
            base
        }
    }

    fn push_open_paren(&mut self, next: Option<&Token>) {
        let current_word = trailing_word(&self.current).to_string();
        let opens_header_paren = self.layout.previous == PreviousToken::Word
            && current_word != "case"
            && is_header(self.options, &current_word)
            // A `foreach` taken for no header names a function.
            && !(matches!(current_word.as_str(), "foreach" | "Q_FOREACH")
                && self.layout.command_state.current_header.as_deref()
                    != Some(current_word.as_str()));
        if opens_header_paren
            && self.token_input.token_begins_source_line
            && !self.current.trimmed().is_empty()
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        }
        let handled_objc_return_paren = self.layout.objc.post_prefix;
        if handled_objc_return_paren {
            self.layout.objc.post_prefix = false;
            self.layout.objc.return_paren_depth = Some(self.layout.nesting.paren_depth + 1);
        }
        let handled_objc_param_paren = self.layout.objc.post_method_colon;
        if handled_objc_param_paren {
            self.layout.objc.post_method_colon = false;
            self.layout.objc.param_paren_depth = Some(self.layout.nesting.paren_depth + 1);
            let colon_pads_after = matches!(
                self.options.pad_method_colon,
                ObjCColonPad::All | ObjCColonPad::After
            );
            if self.options.pad_param_type {
                self.ensure_space();
            } else if self.options.unpad_param_type && !colon_pads_after {
                self.trim_current_end();
            }
        }
        let next_is_close = matches!(next, Some(Token::Symbol(')')));
        let outside_pad = !next_is_close
            && (self.options.pad_parens_outside
                || (self.options.pad_first_paren_outside
                    && self.layout.previous != PreviousToken::OpenParen));
        if self.layout.previous == PreviousToken::Word {
            let word = trailing_word(&self.current);
            let opens_declarator = self
                .current
                .active_token()
                .and_then(|index| self.tree.groups.opened_at(index))
                .is_some_and(|group| self.tree.functions.is_declarator(group));
            let keep_source_space = !next_is_close
                && !self.options.unpad_parens
                && (opens_declarator
                    || (is_pointer_type_word(word) || is_type_like_pointer_word(word))
                        && matches!(next, Some(Token::Operator(op)) if matches!(op.as_str(), "*" | "&" | "^")));
            let keep_unpad_space =
                should_keep_unpad_space_before_paren(word, self.options) || keep_source_space;
            let force_space = (matches!(word, "and" | "or")
                && self.options.pad_operators
                && !self.layout.line_state.operator_padding_disabled)
                || (self.options.pad_header
                    && (is_header(self.options, word)
                        || matches!(word, "return" | "new" | "delete")
                        || word == "throw" && !self.throw_is_exception_specification()))
                || outside_pad;
            if force_space {
                self.pad_before_open_paren_space();
            } else if keep_unpad_space && self.options.unpad_parens {
                // Unpadding leaves the last whitespace character before the
                // paren.
                if let Some(last) = self
                    .token_input
                    .previous_input_whitespace
                    .as_deref()
                    .and_then(|whitespace| whitespace.chars().next_back())
                {
                    self.trim_current_end();
                    self.current.push(last);
                }
            } else if keep_unpad_space {
                self.emit_source_space();
            } else if self.options.unpad_parens {
                self.trim_current_end();
            } else {
                self.emit_source_space();
            }
        } else if !outside_pad
            && self.layout.previous == PreviousToken::Operator
            && self.options.pointer_align == PointerAlign::Name
            && self.current.trimmed_end().ends_with_any(b"*^")
            && self.looks_like_pointer_declaration_context()
            && !self.active_token_in_brackets()
        {
            if !self.function_pointer_parameter_keeps_space_before_name_group() {
                self.trim_current_end();
            }
        } else if outside_pad {
            self.pad_before_open_paren_space();
        } else if self.options.unpad_parens
            && self.current.ends_with_any(b" \t")
            && self.current.trimmed_end().ends_with('[')
        {
            // Unpadding takes the space out from after a bracket.
            self.trim_current_end();
        } else if self.options.unpad_parens
            && matches!(
                self.layout.previous,
                PreviousToken::Operator | PreviousToken::Comma
            )
            && self.current.ends_with_any(b" \t")
            && !self.current_is_blank()
        {
            // Unpadding leaves at most one space before the paren, none
            // after a negation.
            self.trim_current_end();
            if !self.current.ends_with_any(b"!~") {
                self.current.push(' ');
            }
        } else if self.pointer_run.spaces_declarator_group && self.current.ends_with_any(b" \t") {
            // The gap moved past a type's star stays before its group.
        } else if !handled_objc_return_paren
            && !handled_objc_param_paren
            && !self.options.unpad_parens
            && !((self.layout.previous == PreviousToken::Operator
                && self.options.pad_operators
                && self.token_input.previous_input_whitespace.is_none()
                && self.current.ends_with(' '))
                || (self.layout.previous == PreviousToken::Comma
                    && (self.options.pad_commas || self.options.pad_operators)
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' '))
                || (self.layout.previous == PreviousToken::OpenParen
                    && self.options.pad_parens_inside
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' '))
                || (self.layout.line_state.ternary_colon
                    && self.options.pad_operators
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' '))
                // A `for` header's clause starts a space past its `;`.
                || (self.layout.nesting.paren_depth > 0
                    && self.current.ends_with(' ')
                    && self.current.trimmed_end().ends_with(';')))
        {
            self.emit_source_space();
        }
        if self.current.trimmed().is_empty()
            && self.token_input.token_begins_source_line
            && let Some(previous) = self.layout.previous_pre_adjust_line.as_ref()
        {
            let code = self.output.code_trimmed_of(previous);
            if code.trimmed_start().starts_with("return new ") {
                let spaces =
                    leading_visual_width(previous, self.options.tab_width) + "return ".len();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
            } else if self.layout.nesting.paren_depth == 0
                && let Some(spaces) =
                    constructor_initializer_name_indent_from_line(self.options, previous)
            {
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
            }
        }
        let semicolonless_macro_call_indent =
            is_semicolonless_macro_call_name(self.current.trimmed())
                .then(|| self.current_line_indent_spaces());
        let inline_brace_call_indent = self.inline_brace_call_indent_spaces(&self.current);
        let paren_indent_spaces =
            // A line comment after the `(` ends its line as well.
            if matches!(
                next,
                None | Some(Token::Newline | Token::Comment(CommentKind::Line, _))
            ) || self.options.indent_after_parens
            {
                inline_brace_call_indent
                    .or_else(|| self.layout.nesting.current_continuation_indent_spaces())
                    .unwrap_or_else(|| {
                        let prefix_len = self.current.len() - self.current.trimmed_start().len();
                        if prefix_len > 0 && self.current.trimmed().is_empty() {
                            prefix_len
                        } else {
                            self.current_line_indent_spaces()
                        }
                    })
            } else {
                self.current_line_indent_spaces() + self.current_char_len()
            };
        let paren_role = self.paren_role_for_open(
            &current_word,
            opens_header_paren,
            semicolonless_macro_call_indent,
            handled_objc_return_paren,
            handled_objc_param_paren,
        );
        let opener_line_indent = self.open_paren_line_indent_spaces(&current_word);
        let opener_output_column = opener_line_indent + self.current_char_len();
        if std::mem::take(&mut self.pointer_run.spaces_declarator_group)
            && !self.current.ends_with_any(b" \t")
        {
            self.current.push(' ');
        }
        let opener_byte = self.current.len();
        self.current.push('(');
        if !matches!(next, Some(Token::Symbol(')'))) {
            if self.options.pad_parens_inside {
                self.ensure_space();
            } else if !self.options.unpad_parens {
                self.emit_trailing_source_space();
            }
        }
        let call_frame = paren_role.is_call_like().then(|| {
            let logical_chain_indent = self.layout.continuation_indent.logical_chain_indent_spaces;
            let logical_operand_indent_column = logical_chain_indent
                .or_else(|| self.return_continuation_indent_spaces())
                .or_else(|| self.assignment_continuation_indent_spaces())
                .or_else(|| self.layout.nesting.current_continuation_indent_spaces())
                .unwrap_or(opener_line_indent);
            CallFrame {
                first_argument_column: (!matches!(next, Some(Token::Symbol(')')))).then(|| {
                    let after_open_column = opener_output_column + 1;
                    after_open_column
                        + visual_width_from(
                            &self.current[opener_byte + 1..],
                            after_open_column,
                            self.options.tab_width,
                        )
                }),
                next_argument_index: 0,
                logical_operand_indent_column,
                logical_operand_indent_tracks_opener: logical_chain_indent.is_none(),
            }
        });
        self.layout.command_state.observe_char('(');
        self.layout.nesting.enter_paren(
            paren_indent_spaces,
            inline_brace_call_indent.is_some(),
            semicolonless_macro_call_indent,
        );
        let lambda_parameter_list = self.current_paren_is_lambda_parameter_list();
        self.layout.frame_stack.push_delimiter(DelimiterFrame {
            role: paren_role,
            lambda_parameter_list,
            opener_output_column,
            opener_output_line: self.output.len(),
            line_indent_spaces: opener_line_indent,
            continuation_indent_column: None,
            call: call_frame,
        });
        if opens_header_paren {
            self.header_paren.depth = Some(self.layout.nesting.paren_depth);
        }
        self.layout.indentation.enter_paren();
        self.register_current_continuation_indent(next);
        let continuation_indent = self.layout.nesting.current_continuation_indent_spaces();
        if let Some((_, delimiter)) = self.layout.frame_stack.active_delimiter_mut() {
            delimiter.continuation_indent_column = continuation_indent;
        }
        self.layout.previous = PreviousToken::OpenParen;
        self.previous_was_newline = false;
    }

    fn throw_is_exception_specification(&self) -> bool {
        let line = self.current.trimmed_end();
        let Some(prefix) = line.strip_suffix("throw").map(str::trim_end) else {
            return false;
        };
        let Some((open, close)) = trailing_matching_parens(prefix) else {
            return false;
        };
        if close + 1 != prefix.len() {
            return false;
        }
        let head = prefix[..open].trimmed_end();
        self.paren_head_is_declaration(head) || scoped_name_is_constructor(head)
    }

    fn push_close_paren(&mut self, next: Option<&Token>, next_is_adjacent: bool) {
        let is_objc_return_close =
            self.layout.objc.return_paren_depth == Some(self.layout.nesting.paren_depth);
        if is_objc_return_close {
            self.layout.objc.return_paren_depth = None;
        }
        let is_objc_param_close =
            self.layout.objc.param_paren_depth == Some(self.layout.nesting.paren_depth);
        if is_objc_param_close {
            self.layout.objc.param_paren_depth = None;
        }
        if self.current.trimmed().is_empty()
            && let Some(spaces) = self.layout.nesting.current_paren_indent_spaces()
        {
            let spaces = if self.header_paren.depth.is_some()
                || self.current_is_conditional_header_continuation()
            {
                let min_spaces = self.continuation_base_indent() * self.options.indent_width
                    + self.options.continuation_indent * self.options.indent_width;
                spaces.max(min_spaces)
            } else {
                spaces
            };
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        }
        let close_paren_out = self.options.pad_parens_outside
            && !self.options.unpad_parens
            && self.layout.previous == PreviousToken::CloseParen;
        let keeps_converted_pointer_gap = self.options.convert_tabs
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|gap| gap.contains('\t'))
            && self.current.ends_with(' ')
            && self.current.trimmed_end().ends_with_any(b"*&^");
        if self.options.pad_parens_inside && !self.current.ends_with('(') {
            self.pad_inside_paren_space();
        } else if close_paren_out {
            self.emit_source_space_or_ensure();
        } else if self.options.unpad_parens {
            self.trim_current_end();
        } else if !keeps_converted_pointer_gap {
            self.emit_source_space();
        }
        if matches!(self.options.pointer_align, PointerAlign::Name) && self.current.ends_with("* *")
        {
            let new_len = self.current.len() - "* *".len();
            self.current.truncate(new_len);
            self.current.push_str("**");
        }
        self.comments.block_comment_close_paren_ends_declaration = self.current_is_preindented
            && self.current.trimmed_start().starts_with('*')
            && self.current.trimmed_end().ends_with("*/")
            && self.current_paren_context_is_declaration();
        self.current.push(')');
        self.space_after_cast = self.current_ends_cast() && !next_is_adjacent;
        self.layout.command_state.observe_char(')');
        if self.header_paren.depth == Some(self.layout.nesting.paren_depth) {
            self.header_paren.depth = None;
            self.header_paren.just_closed = true;
        }
        let closes_semicolonless_macro_call_indent = self
            .layout
            .nesting
            .current_paren_semicolonless_macro_call_indent()
            .filter(|_| {
                !is_semicolonless_macro_call_name(
                    self.current
                        .trimmed_start()
                        .split_once('(')
                        .map_or("", |(name, _)| name.trimmed()),
                )
            });
        if self.layout.compound_literal.arg_paren_depth == Some(self.layout.nesting.paren_depth) {
            self.layout.compound_literal.arg_indent_spaces = None;
            self.layout.compound_literal.arg_paren_depth = None;
            self.layout.compound_literal.arg_brace_depth = None;
            self.layout.compound_literal.after_comma = false;
        }
        self.layout.nesting.exit_paren();
        self.layout.frame_stack.pop_delimiter(self.output.len());
        self.layout.indentation.exit_paren();
        if self.layout.nesting.paren_depth == 0
            && !matches!(next, Some(Token::Symbol(';')))
            && let Some(spaces) = closes_semicolonless_macro_call_indent
        {
            self.layout
                .continuation_indent
                .clear_continuation_after_line = Some(spaces);
        }
        self.layout.previous = PreviousToken::CloseParen;
        self.previous_was_newline = false;
        self.pad_close_paren_pending = self.options.pad_parens_outside;
        if is_objc_return_close {
            if self.options.pad_return_type {
                self.layout.objc.after_paren_pad = Some(true);
                self.space_after_cast = true;
            } else if self.options.unpad_return_type {
                self.layout.objc.after_paren_pad = Some(false);
                self.space_after_cast = false;
                self.pad_close_paren_pending = false;
            }
        }
        if is_objc_param_close {
            if self.options.pad_param_type {
                self.layout.objc.after_paren_pad = Some(true);
                self.space_after_cast = true;
            } else if self.options.unpad_param_type {
                self.layout.objc.after_paren_pad = Some(false);
                self.space_after_cast = false;
                self.pad_close_paren_pending = false;
            }
        }
    }

    fn push_open_bracket(&mut self, next: Option<&Token>, starts_initializer_designator: bool) {
        let opens_operator_name = self.current.trimmed_end().ends_with("operator");
        let opens_designator = self.inline_array.initializer_designator_bracket_depth > 0
            || (starts_initializer_designator && self.bracket_opens_initializer_designator());
        let opens_collection = !opens_designator && self.current.trimmed_end().ends_with('@');
        let opens_message = !opens_collection
            && !opens_designator
            && self.layout.frame_stack.bracket_depth() == 0
            && self.bracket_opens_objc_message();
        let bracket_role = if opens_collection {
            BracketRole::ObjCCollection
        } else if opens_message {
            BracketRole::ObjCMessage
        } else {
            BracketRole::Other
        };
        let current = self.current.trimmed_end();
        let keeps_padded_objc_selector_gap = self.layout.objc.message_active
            && current.ends_with(':')
            && matches!(
                self.options.pad_method_colon,
                ObjCColonPad::All | ObjCColonPad::After
            );
        let parent_objc_message_align = (self.layout.frame_stack.bracket_depth() > 0
            && self.layout.objc.message_active)
            .then_some(self.layout.objc.message_align)
            .flatten();
        let opens_after_selector = parent_objc_message_align.is_some()
            && current
                .trimmed_start()
                .strip_suffix(':')
                .is_some_and(|selector| {
                    !selector.is_empty()
                        && selector
                            .chars()
                            .all(|ch| ch == '_' || is_identifier_continue(ch))
                });
        let (declarator_operator, declarator_prefix) =
            if let Some(prefix) = current.strip_suffix("&&") {
                (Some("&"), prefix)
            } else if let Some(prefix) = current.strip_suffix('&') {
                (Some("&"), prefix)
            } else if let Some(prefix) = current.strip_suffix('*') {
                (Some("*"), prefix)
            } else if let Some(prefix) = current.strip_suffix('^') {
                (Some("^"), prefix)
            } else {
                (None, current)
            };
        let declarator_alignment =
            declarator_operator.map(|operator| resolved_pointer_align(self.options, operator));
        let opens_attribute = matches!(next, Some(Token::Symbol('[')));
        let opens_structured_binding =
            declarator_operator == Some("&") && trailing_word(declarator_prefix) == "auto";
        let attaches_to_name_side = declarator_alignment == Some(PointerAlign::Name)
            && (opens_attribute || opens_structured_binding);
        let keeps_aligned_declarator_gap = (opens_attribute || opens_structured_binding)
            && matches!(
                declarator_alignment,
                Some(PointerAlign::Type | PointerAlign::Middle)
            )
            && self.current.ends_with_any(b" \t");
        let keeps_padded_comma_gap = self.layout.previous == PreviousToken::Comma
            && (self.options.pad_commas || self.options.pad_operators)
            && self.token_input.previous_input_whitespace.is_none()
            && self.current.ends_with(' ');
        let keeps_padded_operator_gap = self.layout.previous == PreviousToken::Operator
            && self.options.pad_operators
            && self.token_input.previous_input_whitespace.is_none()
            && self.current.ends_with(' ');
        if attaches_to_name_side {
            self.trim_current_end();
        } else if keeps_padded_objc_selector_gap {
            self.ensure_space();
        } else if self.layout.previous == PreviousToken::OpenParen && self.options.pad_parens_inside
        {
            self.pad_inside_paren_space();
        } else if self.layout.previous == PreviousToken::OpenParen && self.options.unpad_parens {
            self.trim_current_end();
        } else if !keeps_aligned_declarator_gap
            && !keeps_padded_comma_gap
            && !keeps_padded_operator_gap
        {
            self.emit_source_space();
        }
        let opener_line_indent = self.current_line_indent_spaces();
        let opener_output_column = opener_line_indent + self.current_char_len();
        self.current.push('[');
        if matches!(next, Some(Token::Symbol('{'))) {
            self.current.push(' ');
        } else if !matches!(next, Some(Token::Symbol(']'))) {
            self.emit_trailing_source_space();
        }
        self.layout.command_state.observe_char('[');
        if opens_designator {
            self.inline_array.initializer_designator_bracket_depth += 1;
        } else if !opens_operator_name {
            self.layout.indentation.enter_bracket();
            self.layout.frame_stack.push_bracket(BracketFrame {
                opener_output_column,
                opener_output_line: self.output.len(),
                line_indent_spaces: opener_line_indent,
                role: bracket_role,
                parent_objc_message_align,
                opens_after_selector,
            });
        }
        if bracket_role != BracketRole::Other {
            self.layout.objc.message_active = true;
            self.layout.objc.message_pending_align = true;
        }
        self.layout.previous = PreviousToken::OpenBracket;
        self.previous_was_newline = false;
    }

    fn bracket_opens_initializer_designator(&self) -> bool {
        self.options.mode == Mode::C
            && self.token_input.token_begins_source_line
            && self.current.trimmed().is_empty()
    }

    fn bracket_opens_objc_message(&self) -> bool {
        if self.current.trimmed_end().ends_with("return") {
            return true;
        }
        match self.layout.command_state.previous_non_ws_char {
            None => true,
            Some(')') if self.options.mode == Mode::ObjC => true,
            Some(ch) => !is_word_char(ch) && ch != ']' && ch != ')',
        }
    }

    fn push_close_bracket(&mut self) {
        if self.token_input.token_begins_source_line
            && self.current.trimmed().is_empty()
            && self.inline_array.initializer_designator_bracket_depth == 0
            && let Some(frame) = self.layout.frame_stack.active_bracket()
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(match frame.role {
                BracketRole::ObjCCollection => frame.opener_output_column.saturating_sub(1),
                BracketRole::Other | BracketRole::ObjCMessage => frame.opener_output_column,
            });
        }
        if self.token_input.token_begins_source_line
            && !self.current.trimmed().is_empty()
            && self.current.trimmed_start().starts_with('>')
        {
            self.finish_line();
        }
        self.emit_source_space();
        self.current.push(']');
        self.layout.command_state.observe_char(']');
        if self.inline_array.initializer_designator_bracket_depth > 0 {
            self.inline_array.initializer_designator_bracket_depth -= 1;
            self.layout.previous = PreviousToken::CloseBracket;
            self.previous_was_newline = false;
            return;
        }
        let closes_operator_name = self.current.trimmed_end().ends_with("operator[]");
        let closing_nested_message_argument = self.layout.frame_stack.bracket_depth() > 1
            && self.layout.objc.message_active
            && self.current.contains(':')
            && !self.current.trimmed_start().starts_with('[');
        if !closes_operator_name {
            self.layout.indentation.exit_bracket();
            self.layout.frame_stack.pop_bracket();
        }
        if self.layout.frame_stack.bracket_depth() == 0 {
            self.layout.objc.message_active = false;
        } else if closing_nested_message_argument {
            self.layout.objc.message_align = None;
        }
        self.layout.previous = PreviousToken::CloseBracket;
        self.previous_was_newline = false;
    }

    /// Whether the token after `index` is a brace add-braces put there,
    /// which astyle counts in the statement line's length.
    #[inline(never)]
    fn added_brace_follows(&self, index: usize) -> bool {
        if self.added_closing_braces.is_empty() {
            return false;
        }
        (index.saturating_add(1)..self.tree.tokens.len())
            .find(|&next| {
                !matches!(
                    self.tree.tokens[next],
                    Token::Whitespace(_) | Token::Newline
                )
            })
            .is_some_and(|next| self.added_closing_braces.contains(&next))
    }

    /// Whether the `;` ends a statement of a block the source kept on its
    /// line and the layout broke: its statements part as a block's do.
    fn statement_in_broken_one_line_block(&self) -> bool {
        !self.token_input.replaying_removed_braces
            && self
                .current
                .active_token()
                .is_some_and(|index| self.in_broken_one_line_block(index))
    }

    /// Whether token `index` lies in a block the source kept on its line and
    /// the layout broke.
    pub(crate) fn in_broken_one_line_block(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        let Some(group) = groups.enclosing(index) else {
            return false;
        };
        let open = groups.get(group).open;
        let breaks_line = |token: &Token| match token {
            Token::Newline => true,
            Token::StringLiteral(text) | Token::Comment(_, text) => text.contains('\n'),
            _ => false,
        };
        matches!(self.tree.tokens[open], Token::Symbol('{'))
            && !self.current.contains('{')
            && !self.tree.tokens[open..index].iter().any(breaks_line)
            && groups
                .get(group)
                .close
                .is_some_and(|close| !self.tree.tokens[index..close].iter().any(breaks_line))
    }

    fn push_semicolon(
        &mut self,
        next: Option<&Token>,
        following_closer_width: usize,
        following_while_suffix: Option<String>,
        added_brace_follows: bool,
    ) {
        let run_in_closers = matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        );
        let suffix_width = if run_in_closers {
            following_closer_width
        } else {
            usize::from(added_brace_follows)
        };
        self.max_length_line.set_suffix_width(suffix_width);
        self.max_length_line
            .set_while_suffix(following_while_suffix.filter(|_| run_in_closers));
        let closed_lambda_header_indent = self
            .current
            .trimmed_end()
            .ends_with('}')
            .then(|| {
                self.layout
                    .frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
                    .map(|frame| frame.header_indent_column)
            })
            .flatten();
        self.layout.in_class_base_clause = false;
        self.unmatched_closing_brace_recovery = false;
        if self.one_line_block_mode {
            self.push_inline_semicolon(next);
            return;
        }
        if self.layout.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
            self.pad_inside_paren_space();
        } else if self.layout.previous == PreviousToken::OpenParen && self.options.unpad_parens {
            self.trim_current_end();
        } else {
            self.emit_source_space();
        }
        self.current.push(';');
        self.layout.command_state.observe_char(';');
        self.layout.line_state.passed_semicolon = true;
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        let closing_header_follows = matches!(
            (self.layout.command_state.current_header.as_deref(), next),
            (Some("if"), Some(Token::Word(word))) if word == "else"
        ) || matches!(
            (self.layout.command_state.current_header.as_deref(), next),
            (Some("try" | "catch"), Some(Token::Word(word))) if word == "catch"
        ) || matches!(
            (self.layout.command_state.current_header.as_deref(), next),
            (Some("do"), Some(Token::Word(word))) if word == "while"
        );
        let completed_header_in_outer_delimiter = self
            .layout
            .frame_stack
            .active_header()
            .is_some_and(|header| header.parent_delimiter.is_some())
            && self.layout.command_state.current_header.is_some()
            && self.header_paren.depth.is_none()
            && !closing_header_follows;
        if completed_header_in_outer_delimiter {
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.frame_stack.clear_header();
            self.layout.continuation_indent.clear_next_line();
            if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && self.layout.indentation.indent() == base + delta
            {
                self.layout.indentation.exit_braceless_block();
            }
        }
        if self.layout.indentation.statement_depth() == 0 {
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
            // A finished statement leaves no column for the line after it.
            self.layout
                .continuation_indent
                .clear_continuation_after_line = None;
            let closed_questions = self.layout.nesting.truncate_questions_to_brace_scope();
            for _ in 0..closed_questions {
                self.layout.frame_stack.pop_active_ternary();
            }
            self.layout.frame_stack.pop_completed_ternaries();
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
            self.multi_declarator_indent_spaces = None;
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.command_state.pending_block_word = None;
            self.pending_extern = false;
            self.header_paren.depth = None;
            self.layout.frame_stack.truncate_brackets(0);
            self.layout.objc.method_continuation = false;
            self.layout.objc.message_active = false;
            let following_header = matches!(
                next,
                Some(Token::Word(word))
                    if is_header(self.options, word) && !matches!(word.as_str(), "case" | "default")
            );
            let break_expanded_lisp_header = self.options.brace_style == BraceStyle::Lisp
                && self.layout.line_state.is_one_line_block
                && following_header;
            let keep_following_header = !break_expanded_lisp_header
                && !self.options.break_one_line_headers
                && self.options.keeps_multi_statement_line()
                && following_header;
            // Styles that break blocks from their statements still break
            // this one.
            let keep_following_block = self.options.keeps_multi_statement_line()
                && matches!(next, Some(Token::Symbol('{')))
                && !matches!(
                    self.options.brace_style,
                    BraceStyle::Allman
                        | BraceStyle::Gnu
                        | BraceStyle::Whitesmith
                        | BraceStyle::Vtk
                        | BraceStyle::Horstmann
                        | BraceStyle::Pico
                );
            if matches!(next, Some(Token::Comment(_, _))) {
                self.emit_trailing_source_space();
                self.schedule_block_spacing_semicolon();
            } else if matches!(next, Some(Token::Symbol(';'))) {
                self.trim_current_end();
            } else if matches!(next, Some(Token::Operator(operator)) if is_leading_continuation_operator(operator))
                || matches!(next, Some(Token::Symbol('.')))
            {
                self.emit_trailing_source_space();
            } else if (self.options.brace_style == BraceStyle::Pico
                && (self.token_input.token_line_opens_with_brace
                    || self.output.last().is_some_and(|line| line.trimmed() == "{")))
                || self.attached_statement_expression_closer_follows()
            {
                self.emit_trailing_source_space_or_ensure();
                self.schedule_block_spacing_semicolon();
            } else if break_expanded_lisp_header
                || self.statement_in_broken_one_line_block()
                || (!keep_following_header
                    && !keep_following_block
                    && (self.options.break_one_line_statements
                        || !self.layout.line_state.is_multi_statement_line
                        || (self.layout.line_state.is_one_line_block
                            && self.options.break_one_line_blocks)))
            {
                self.finish_line();
                self.observe_block_spacing_semicolon();
            } else if !matches!(next, Some(Token::Symbol(';' | ')' | '}'))) {
                self.emit_trailing_source_space_or_ensure();
                if matches!(next, Some(Token::Newline) | None) {
                    self.schedule_block_spacing_semicolon();
                }
            } else {
                self.trim_current_end();
            }
            self.unwind_else_if_break_depths_unless_else(next);
            self.layout.pending_braceless_block_bias = None;
            self.layout.inline_nested_header_braceless_bias = None;
            if self.preprocessor.split_else.body_braceless
                || self.preprocessor.split_else.trigger_output_len == Some(usize::MAX)
            {
                self.preprocessor.split_else.pending_body = false;
                self.preprocessor.split_else.body_braceless = false;
                self.preprocessor.split_else.extra_indent = false;
                self.preprocessor.split_else.extra_levels = 0;
                self.preprocessor.split_else.trigger_output_len = None;
                self.layout.continuation_indent.clear_next_line();
            }
            while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && self.layout.indentation.indent() == base + delta
                && !self.next_keeps_braceless_block(next, base)
            {
                self.layout.indentation.exit_braceless_block();
            }
            // Leaving braceless bodies can end an else-if chain too.
            self.unwind_else_if_break_depths_unless_else(next);
        } else {
            if self.current.trimmed_start().starts_with(':')
                && self.layout.nesting.paren_depth == 0
                && !self.layout.nesting.has_question_in_current_brace()
            {
                self.layout.indentation.clear_continuation_indents();
                self.layout.nesting.clear_continuation_indents();
            } else if let Some(spaces) = self.for_header_continuation_indent_spaces() {
                self.layout.nesting.clear_continuation_indents();
                self.layout
                    .nesting
                    .register_continuation_indent_spaces(spaces);
            } else if self.layout.nesting.paren_depth == 0
                && !self.layout.nesting.has_question_in_current_brace()
            {
                self.layout.indentation.clear_continuation_indents();
                self.layout.nesting.clear_continuation_indents();
                self.layout.continuation_indent.logical_chain_indent_spaces = None;
                self.layout.continuation_indent.next_line_indent_spaces = None;
            } else {
                self.layout.nesting.trim_to_current_statement_continuation();
            }
            if matches!(next, Some(Token::Symbol(';' | ')'))) {
                self.emit_trailing_source_space();
            } else {
                self.emit_trailing_source_space_or_ensure();
            }
            if self.layout.nesting.paren_depth == 0 {
                self.layout.pending_braceless_block_bias = None;
                self.layout.inline_nested_header_braceless_bias = None;
                while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                    && self.layout.indentation.indent() == base + delta
                    && !self.next_keeps_braceless_block(next, base)
                {
                    self.layout.indentation.exit_braceless_block();
                }
            }
        }
        if let Some(spaces) = closed_lambda_header_indent
            && self.current_is_blank()
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
    }

    fn push_comma(&mut self, next: Option<&Token>) {
        let after_compound_literal = std::mem::take(&mut self.layout.compound_literal.just_closed)
            && self.current.trimmed_end().ends_with('}');
        self.emit_source_space();
        // astyle drops the spaces before a comma that follows code.
        if !self.current_is_blank() {
            let kept = self.current.trim_end_matches(' ').len();
            self.current.truncate(kept);
        }
        self.current.push(',');
        self.layout.command_state.observe_char(',');
        if after_compound_literal {
            let indent = self.current_line_indent_spaces();
            self.layout.nesting.clear_continuation_indents();
            self.layout.compound_literal.after_comma = true;
            self.layout.compound_literal.arg_indent_spaces = Some(indent);
            self.layout.compound_literal.arg_paren_depth = Some(self.layout.nesting.paren_depth);
            self.layout.compound_literal.arg_brace_depth =
                Some(self.layout.nesting.brace_header_stack.len());
        } else if self.layout.compound_literal.arg_indent_spaces.is_some()
            && self.layout.compound_literal.arg_paren_depth == Some(self.layout.nesting.paren_depth)
            && self.layout.compound_literal.arg_brace_depth
                == Some(self.layout.nesting.brace_header_stack.len())
        {
            self.layout.nesting.clear_continuation_indents();
            self.layout.compound_literal.after_comma = true;
        } else if self.layout.indentation.statement_depth() > 0
            || self.layout.nesting.paren_depth > 0
        {
            self.layout.nesting.trim_to_current_statement_continuation();
        } else if self.innermost_brace_is_compound_literal() {
            self.layout.nesting.clear_continuation_indents();
        } else if !self.in_initializer_brace() && !self.in_aggregate_declaration_brace() {
            if self.multi_declarator_indent_spaces.is_none() && self.current.holds_equals() {
                let line = self.current.trimmed_end();
                let prefix = line.len() - line.trimmed_start().len();
                if let Some(offset) = assignment_declarator_offset(line.trimmed_start()) {
                    let base = if prefix == 0 {
                        self.current_line_indent_spaces()
                    } else {
                        prefix
                    };
                    self.multi_declarator_indent_spaces = Some(base + offset);
                    self.layout.nesting.clear_continuation_indents();
                } else if is_single_lvalue_assignment(line.trimmed_start()) {
                    self.multi_declarator_indent_spaces = Some(self.current_line_indent_spaces());
                    self.layout.nesting.clear_continuation_indents();
                }
            } else if self.multi_declarator_indent_spaces.is_some() {
                self.layout.nesting.clear_continuation_indents();
            }
        }
        let comma_role = self.comma_role_for_current_separator(after_compound_literal);
        self.update_argument_frame_after_comma(comma_role);
        if matches!(next, Some(Token::Symbol(','))) {
            self.trim_current_end();
        } else if !matches!(next, Some(Token::Comment(_, _)))
            && (self.options.pad_commas || self.options.pad_operators)
        {
            self.emit_trailing_source_space_or_ensure();
        } else {
            self.emit_trailing_source_space();
        }
        self.layout.previous = PreviousToken::Comma;
        self.previous_was_newline = false;
        let in_objc_dictionary_literal = self.output.recent_scoped_at_brace_line(64)
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.trimmed_end().ends_with(';'))
                .any(|line| line.contains_from_first_byte("@ {"));
        if in_objc_dictionary_literal {
            let spaces = self.current_line_indent_spaces();
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            if !matches!(next, Some(Token::Newline) | None) {
                self.finish_line();
                self.previous_was_newline = true;
            }
        }
    }

    fn comma_role_for_current_separator(&self, after_compound_literal: bool) -> CommaRole {
        if self
            .layout
            .frame_stack
            .active_delimiter()
            .is_some_and(|delimiter| delimiter.role.is_call_like())
        {
            return CommaRole::CallArgument;
        }
        if after_compound_literal
            || (self.layout.compound_literal.arg_indent_spaces.is_some()
                && self.layout.compound_literal.arg_paren_depth
                    == Some(self.layout.nesting.paren_depth)
                && self.layout.compound_literal.arg_brace_depth
                    == Some(self.layout.nesting.brace_header_stack.len()))
        {
            return CommaRole::CompoundLiteralArgument;
        }
        if self.in_initializer_brace()
            || self.in_aggregate_declaration_brace()
            || self.innermost_brace_is_compound_literal()
            || self.current_inline_array_column().is_some()
        {
            return CommaRole::InitializerSibling;
        }
        if self.multi_declarator_indent_spaces.is_some()
            || self.current.holds_equals() && {
                let body = self.current.trimmed();
                assignment_declarator_offset(body).is_some() || is_single_lvalue_assignment(body)
            }
        {
            return CommaRole::Declaration;
        }
        CommaRole::Other
    }

    fn update_argument_frame_after_comma(&mut self, role: CommaRole) {
        let mut frame = ArgumentFrame {
            role,
            owner: None,
            index: self
                .layout
                .frame_stack
                .last_argument()
                .filter(|argument| argument.owner.is_none() && argument.role == role)
                .map_or(0, |argument| argument.index + 1),
            sibling_anchor_column: self.argument_sibling_anchor_column(role),
        };
        if role == CommaRole::CallArgument
            && let Some((owner, delimiter)) = self.layout.frame_stack.active_delimiter_mut()
            && delimiter.role.is_call_like()
        {
            frame.owner = Some(owner);
            frame.sibling_anchor_column = delimiter
                .call
                .as_ref()
                .and_then(|call| call.first_argument_column)
                .or(Some(delimiter.opener_output_column + 1));
            if let Some(call) = delimiter.call.as_mut() {
                frame.index = call.next_argument_index;
                call.next_argument_index += 1;
            }
        }
        self.layout.frame_stack.set_last_argument(frame);
    }

    fn argument_sibling_anchor_column(&self, role: CommaRole) -> Option<usize> {
        match role {
            CommaRole::CallArgument => None,
            CommaRole::Declaration => self
                .multi_declarator_indent_spaces
                .or_else(|| Some(self.current_line_indent_spaces())),
            CommaRole::InitializerSibling | CommaRole::CompoundLiteralArgument => self
                .current_inline_array_column()
                .or(self.layout.compound_literal.arg_indent_spaces)
                .or_else(|| Some(self.current_line_indent_spaces())),
            CommaRole::Other => None,
        }
    }

    fn push_colon(&mut self, next: Option<&Token>, token_index: usize) {
        if self.current.trimmed_end().ends_with(':') && self.token_input.previous_input_was_adjacent
        {
            self.current.push(':');
            self.layout.command_state.observe_char(':');
            self.emit_trailing_source_space();
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        if self.current.trimmed().is_empty()
            && let Some(spaces) = self.previous_colon_continuation_indent_spaces()
        {
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        let ColonKind {
            asm_operand: is_asm_operand_colon,
            class_initializer: is_class_initializer,
            function_try_initializer,
            objc_selector: is_objc_colon,
            objc_interface: is_objc_interface_colon,
            enum_underlying_type: is_enum_underlying_type,
            class_base: is_class_base,
            bit_field: is_bit_field,
            range_for: is_range_for,
            aligned_continuation: aligned_continuation_colon,
            label: is_label,
            ternary: is_ternary,
            objc_method_definition: is_objc_method_def_colon,
            role: colon_role,
        } = self.classify_colon(next, token_index);
        let case_label_colon = matches!(
            self.layout.command_state.current_header.as_deref(),
            Some("case" | "default")
        ) && !self.layout.command_state.case_label_colon_emitted
            && !is_ternary
            && self.layout.nesting.paren_depth == 0
            && self.layout.frame_stack.bracket_depth() == 0
            && !matches!(next, Some(Token::Symbol(':')));
        self.layout.command_state.case_label_colon_emitted = case_label_colon;
        let pad_off =
            !self.options.pad_operators || self.layout.line_state.operator_padding_disabled;
        let colon_mode = self.options.pad_method_colon;
        let next_is_close_paren = matches!(next, Some(Token::Symbol(')')));
        if is_objc_interface_colon {
            self.ensure_space();
        } else if is_objc_colon {
            if colon_mode == ObjCColonPad::NoChange {
                self.emit_source_space();
            } else if !next_is_close_paren
                && matches!(colon_mode, ObjCColonPad::All | ObjCColonPad::Before)
            {
                self.ensure_space();
            } else {
                self.trim_current_end();
            }
        } else if is_range_for && !pad_off {
            self.ensure_space();
        } else if is_label
            || is_bit_field
            || is_class_initializer
            || is_class_base
            || (is_enum_underlying_type && pad_off)
            || (is_range_for && pad_off)
            || is_asm_operand_colon
            || aligned_continuation_colon
            || (is_ternary && pad_off)
        {
            self.emit_source_space();
        } else {
            self.trim_current_end();
        }
        if (is_ternary || is_enum_underlying_type || is_range_for) && !pad_off {
            self.emit_source_space_or_ensure();
        }
        if is_class_initializer {
            self.layout.line_state.in_class_initializer = true;
            self.current_line_has_class_initializer_colon = true;
        }
        if is_class_base {
            self.layout.in_class_base_clause = true;
            self.layout.split_class_export_pending_base = false;
        }
        let break_after_ternary_colon = is_ternary
            && !matches!(
                next,
                Some(Token::Newline) | Some(Token::Comment(_, _)) | None
            )
            && (self.current.last_question().is_some_and(|index| {
                self.current
                    .last_open_brace()
                    .is_some_and(|brace| brace > index)
            }) || (self.current.trimmed_end().ends_with('}')
                && self.layout.nesting.last_closed_brace_type.is_some()));
        let colon_output_column = self
            .current_visual_width()
            .max(self.token_input.token_source_line_indent);
        if colon_role == ColonRole::ClassInitializer {
            self.record_constructor_initializer_frame(function_try_initializer);
        }
        if colon_role == ColonRole::Other
            || (colon_role != ColonRole::Ternary && self.current.first_assignment().is_some())
        {
            self.layout.nesting.clear_continuation_indents();
        }
        self.current.push(':');
        self.layout.command_state.observe_char(':');
        if colon_role == ColonRole::Ternary {
            self.layout
                .frame_stack
                .close_active_ternary(colon_role, colon_output_column);
        }
        self.layout.line_state.passed_colon = true;
        self.layout.line_state.bit_field_colon = is_bit_field;
        if is_ternary {
            self.layout.line_state.ternary_colon = true;
        } else if is_objc_interface_colon {
            self.emit_source_space_or_ensure();
        } else if is_class_base && !pad_off && !matches!(next, Some(Token::Newline) | None) {
            self.emit_trailing_source_space_or_ensure();
        } else if is_class_initializer
            && !pad_off
            && !matches!(next, Some(Token::Newline) | None)
            && self
                .token_input
                .next_input_whitespace
                .as_deref()
                .unwrap_or_default()
                .is_empty()
        {
            self.ensure_space();
        } else if aligned_continuation_colon && !is_asm_operand_colon && !is_objc_colon {
            self.emit_trailing_source_space();
        }
        self.layout.objc.post_method_colon = is_objc_method_def_colon;
        if is_objc_colon && colon_mode != ObjCColonPad::NoChange && next_is_close_paren {
            self.layout.objc.after_paren_pad = Some(false);
        }
        if !is_label {
            self.layout.nesting.exit_question();
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        let in_objc_dictionary_literal = self.current.holds_dictionary_opener()
            || self.output.recent_scoped_at_brace_line(64)
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .take(64)
                    .take_while(|line| !line.trimmed_end().ends_with(';'))
                    .any(|line| line.contains_from_first_byte("@ {"));
        if in_objc_dictionary_literal
            && !self.one_line_block_mode
            && !is_objc_colon
            && !matches!(next, Some(Token::Newline) | None)
        {
            let spaces = if self.current.holds_dictionary_opener() {
                self.layout
                    .previous_pre_adjust_line
                    .as_ref()
                    .filter(|previous| previous.trimmed_end().ends_with('='))
                    .map(|previous| {
                        leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width * 2
                    })
                    .unwrap_or_else(|| {
                        self.current_line_indent_spaces() + self.options.indent_width
                    })
            } else {
                self.layout
                    .previous_pre_adjust_line
                    .as_ref()
                    .filter(|previous| previous.trimmed_end().ends_with(','))
                    .map(|previous| leading_visual_width(previous, self.options.tab_width))
                    .unwrap_or_else(|| self.current_line_indent_spaces())
            };
            self.finish_line();
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.previous_was_newline = true;
        } else if function_try_initializer
            && self.options.break_one_line_statements
            && !matches!(next, Some(Token::Newline | Token::Comment(_, _)) | None)
        {
            self.finish_line();
            self.layout.continuation_indent.next_line_indent = Some(self.statement_level() + 1);
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        } else if break_after_ternary_colon {
            let spaces = self.ternary_colon_break_indent_spaces();
            self.finish_line();
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = spaces;
            self.previous_was_newline = true;
        } else if is_label {
            if self.options.brace_style == BraceStyle::OneTrueBrace
                && matches!(next, Some(Token::Symbol('{')))
            {
                self.ensure_space();
            } else if self.options.break_one_line_statements
                && !self.one_line_block_mode
                && !matches!(next, Some(Token::Comment(_, _)))
            {
                self.finish_line();
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = None;
            } else if !matches!(
                next,
                Some(Token::Comment(_, _)) | Some(Token::Newline) | None
            ) {
                if self.options.pad_operators && !self.layout.line_state.operator_padding_disabled {
                    self.emit_trailing_source_space_or_ensure();
                } else {
                    self.emit_trailing_source_space();
                }
            }
        } else if is_asm_operand_colon
            || ((is_class_initializer
                || (is_enum_underlying_type && pad_off)
                || (is_class_base && pad_off))
                && !aligned_continuation_colon)
        {
            self.emit_trailing_source_space();
        } else if is_objc_colon {
            if colon_mode == ObjCColonPad::NoChange {
                self.emit_trailing_source_space();
            } else if !next_is_close_paren
                && matches!(colon_mode, ObjCColonPad::All | ObjCColonPad::After)
            {
                self.ensure_space();
            }
        } else if self.options.pad_operators && !self.layout.line_state.operator_padding_disabled {
            if is_ternary || is_bit_field || (is_enum_underlying_type && !is_class_base) {
                self.emit_trailing_source_space_or_ensure();
            } else {
                self.ensure_space();
            }
        } else if (is_ternary || is_bit_field || is_range_for) && pad_off {
            self.emit_trailing_source_space();
        }
    }

    fn classify_colon(&self, next: Option<&Token>, token_index: usize) -> ColonKind {
        let is_asm_operand_colon = self.is_asm_operand_colon();
        let is_class_initializer = !is_asm_operand_colon
            && (self.is_class_initializer_colon()
                || (self.current.trimmed().is_empty() && self.colon_leads_class_initializer()));
        let function_try_initializer =
            is_class_initializer && self.class_initializer_follows_function_try();
        let is_objc_colon = self.is_objc_selector_or_message_colon();
        let is_objc_interface_colon = self.current.trimmed_start().starts_with("@interface ");
        let is_enum_underlying_type = self.is_enum_underlying_type_colon();
        let is_class_base = !is_asm_operand_colon
            && !is_objc_colon
            && !is_objc_interface_colon
            && self.colon_leads_class_base_clause();
        let is_bit_field = !is_asm_operand_colon
            && !is_class_initializer
            && !is_enum_underlying_type
            && self.is_bit_field_colon(match next {
                // A member split at its colon has its width on the next line.
                None | Some(Token::Newline) => next_code_token(&self.tree.tokens, token_index + 1)
                    .map(|index| &self.tree.tokens[index]),
                _ => next,
            });
        let has_question = self.layout.nesting.has_question_in_current_brace();
        let is_range_for = !has_question && self.is_range_for_colon();
        // A label after the brace closing the case before stands on its own.
        let label_text = self.current[self.current.statement_start()..]
            .trimmed()
            .trim_start_matches('}')
            .trimmed();
        let label_candidate = labels::is_label_start(label_text, &self.options.access_labels);
        let access_label_candidate =
            labels::is_access_label_start(label_text, &self.options.access_labels);
        let aligned_continuation_colon = !has_question
            && !is_bit_field
            && !is_range_for
            && !access_label_candidate
            && (is_asm_operand_colon || self.current.first_assignment().is_none())
            // A comment line before a statement leaves its column pending,
            // which continues nothing.
            && (self
                .layout
                .continuation_indent
                .next_line_indent_spaces
                .is_some()
                && !self
                    .output
                    .last_non_empty_index()
                    .is_some_and(|index| is_comment_only_line(self.output[index].trimmed_start()))
                || self.in_initializer_brace()
                || self.current_inline_array_column().is_some());
        let is_label = !has_question
            && !is_objc_colon
            && !is_bit_field
            && !is_class_base
            && !is_enum_underlying_type
            && !is_range_for
            && !aligned_continuation_colon
            && label_candidate;
        let is_ternary = has_question
            && !is_label
            && !is_bit_field
            && !is_class_initializer
            && !is_class_base
            && !is_enum_underlying_type
            && !is_objc_colon
            && !is_objc_interface_colon
            && !aligned_continuation_colon;
        let in_objc_message = self.current.has_unclosed_bracket();
        let is_objc_method_def_colon = is_objc_colon
            && !in_objc_message
            && (self.is_objc_method_line() || self.layout.objc.method_continuation);
        let colon_role = if is_ternary {
            ColonRole::Ternary
        } else if is_label {
            ColonRole::Label
        } else if is_class_initializer {
            ColonRole::ClassInitializer
        } else if is_class_base {
            ColonRole::ClassBase
        } else if is_enum_underlying_type {
            ColonRole::EnumUnderlyingType
        } else if is_range_for {
            ColonRole::RangeFor
        } else if is_bit_field {
            ColonRole::BitField
        } else if is_objc_colon {
            ColonRole::ObjCSelector
        } else if is_objc_interface_colon {
            ColonRole::ObjCInterface
        } else if is_asm_operand_colon {
            ColonRole::AsmOperand
        } else if aligned_continuation_colon {
            ColonRole::AlignedContinuation
        } else {
            ColonRole::Other
        };
        ColonKind {
            asm_operand: is_asm_operand_colon,
            class_initializer: is_class_initializer,
            function_try_initializer,
            objc_selector: is_objc_colon,
            objc_interface: is_objc_interface_colon,
            enum_underlying_type: is_enum_underlying_type,
            class_base: is_class_base,
            bit_field: is_bit_field,
            range_for: is_range_for,
            aligned_continuation: aligned_continuation_colon,
            label: is_label,
            ternary: is_ternary,
            objc_method_definition: is_objc_method_def_colon,
            role: colon_role,
        }
    }

    fn ternary_colon_break_indent_spaces(&self) -> Option<usize> {
        let line = self.current.as_str();
        let trimmed = line.trimmed_start();
        let leading = self.current_line_indent_spaces();
        if trimmed.starts_with("return ") {
            return Some(leading + "return ".len());
        }
        let (operator_index, operator) = find_assignment_operator(line)?;
        let mut end = operator_index + operator.len();
        let bytes = line.as_bytes();
        while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
            end += 1;
        }
        Some(leading + visual_width_from(&line[..end], 0, self.options.tab_width))
    }

    fn is_asm_operand_colon(&self) -> bool {
        // The statement starts past a brace it shares its line with.
        let current = self.current[self.current.statement_start()..].trimmed_start();
        self.current.holds_asm_call()
            || current.starts_with("asm ")
            || current.starts_with("asm\t")
            || current.starts_with("_asm ")
            || current.starts_with("__asm ")
            || current.starts_with("__asm__ ")
            // The statement's lines back to its start, over any number of
            // template rows.
            || self.output.may_have_asm()
                && self
                .output
                .scoped()
                .iter()
                .rev()
                .take(256)
                .take_while(|line| !line.trimmed_end().ends_with_any(b";{}"))
                .any(|line| line.contains("asm"))
    }

    fn previous_colon_continuation_indent_spaces(&self) -> Option<usize> {
        let previous = self.output.last()?;
        let trimmed = previous.trimmed_start();
        if !trimmed.starts_with(':') {
            return None;
        }
        if self.open_paren_column_of(trimmed).is_some() {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    fn is_range_for_colon(&self) -> bool {
        self.layout.command_state.current_header.as_deref() == Some("for")
            && self.header_paren.depth == Some(self.layout.nesting.paren_depth)
    }

    fn is_enum_underlying_type_colon(&self) -> bool {
        if self.layout.nesting.has_question_in_current_brace() {
            return false;
        }
        let current = self.current.trimmed_end();
        let segment = current
            .rfind([';', '{', '}'])
            .map_or(current, |index| &current[index + 1..])
            .trimmed_start();
        segment == "enum" || segment.starts_with("enum ")
    }

    fn is_bit_field_colon(&self, next: Option<&Token>) -> bool {
        if !matches!(next, Some(Token::Number(_) | Token::Word(_))) {
            return false;
        }
        self.is_bit_field_segment(matches!(next, Some(Token::Number(_))))
    }

    fn is_bit_field_segment(&self, next_is_number: bool) -> bool {
        let in_class = self.layout.nesting.brace_type_stack.last() == Some(&BraceType::Class);
        if !(self.in_aggregate_declaration_brace() || in_class)
            || self.layout.nesting.has_question_in_current_brace()
        {
            return false;
        }
        let current = self.current.trimmed_end();
        let segment = current
            .rfind([';', '{', '}'])
            .map_or(current, |index| &current[index + 1..])
            .trimmed();
        if segment.is_empty()
            || segment.contains('?')
            || in_class && labels::is_access_label_start(segment, &self.options.access_labels)
        {
            return false;
        }
        let word_count = segment
            .split(|ch: char| !is_identifier_continue(ch))
            .filter(|word| !word.is_empty())
            .count();
        word_count >= 2 || (next_is_number && word_count >= 1)
    }

    fn is_class_initializer_colon(&self) -> bool {
        let code = &self.current[..self.current_trailing_comment_split_limit()];
        self.code_is_class_initializer_signature(code.trimmed_end())
    }

    pub(crate) fn class_initializer_follows_function_try(&self) -> bool {
        let code = self.current[..self.current_trailing_comment_split_limit()].trimmed_end();
        if trailing_word(code) == "try" {
            return true;
        }
        if !code.is_empty() {
            return false;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .find_map(|line| {
                let code = self.output.code_of(line).trimmed();
                (!code.is_empty() && !code.starts_with_any(b"#/*")).then_some(code)
            })
            .is_some_and(|code| trailing_word(code) == "try")
    }

    fn code_is_class_initializer_signature(&self, code: &str) -> bool {
        if trailing_word(code) == "try" {
            let before_try = code[..code.len() - "try".len()].trimmed_end();
            if !before_try.is_empty() {
                return self.code_is_class_initializer_signature(before_try);
            }
            return self
                .output
                .scoped()
                .iter()
                .rev()
                .find_map(|line| {
                    let code = self.output.code_of(line).trimmed();
                    (!code.is_empty() && code != "try" && !code.starts_with_any(b"#/*"))
                        .then_some(code)
                })
                .is_some_and(|code| self.code_is_class_initializer_signature(code));
        }
        let block_comment_close_paren_signature =
            self.comments.block_comment_close_paren_ends_declaration
                || (self.current.trimmed().is_empty()
                    && self
                        .comments
                        .previous_block_comment_close_paren_ended_declaration);
        !self.layout.nesting.has_question_in_current_brace()
            && (signature_ends_with_parameter_list(code) || block_comment_close_paren_signature)
            && (self
                .layout
                .nesting
                .brace_type_stack
                .iter()
                .any(|brace_type| is_class_like_brace_type(*brace_type))
                || code_ends_definition_header(code))
    }

    pub(crate) fn colon_leads_class_initializer(&self) -> bool {
        if self.is_class_initializer_colon() {
            return true;
        }
        if !self.current[..self.current_trailing_comment_split_limit()]
            .trimmed()
            .is_empty()
        {
            return false;
        }
        let Some(line) = self.output.scoped().iter().rev().find(|line| {
            let trimmed = line.trimmed_start();
            !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("/*")
                && (!trimmed.starts_with('*')
                    || trimmed
                        .split_once("*/")
                        .is_some_and(|(_, suffix)| !suffix.trimmed().is_empty()))
        }) else {
            return false;
        };
        let code = &self.output.code_of(line);
        self.code_is_class_initializer_signature(code.trimmed_end())
    }

    fn ternary_owner_role_for_question(&self) -> TernaryOwnerRole {
        let current = self.current[..self.current_trailing_comment_split_limit()].trimmed_end();
        if current.trimmed_start().starts_with("return ") || current.trimmed_start() == "return" {
            return TernaryOwnerRole::Return;
        }
        if find_assignment_operator(current).is_some() {
            return TernaryOwnerRole::Assignment;
        }
        for scan_index in self.output.scoped_range().rev().take(8) {
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("return ") || trimmed == "return" {
                return TernaryOwnerRole::Return;
            }
            if find_assignment_operator(code).is_some()
                || (code.ends_with('=')
                    && !code.ends_with("==")
                    && !code.ends_with("!=")
                    && !code.ends_with("<=")
                    && !code.ends_with(">="))
            {
                return TernaryOwnerRole::Assignment;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                break;
            }
        }
        TernaryOwnerRole::Other
    }

    fn ternary_condition_operand_anchor(&self, owner_role: TernaryOwnerRole) -> Option<usize> {
        let code = self.current[..self.current_trailing_comment_split_limit()].trimmed_end();
        let trimmed = code.trimmed_start();
        let lead = code.len() - trimmed.len();
        let operand_byte = match owner_role {
            TernaryOwnerRole::Return => {
                let rest = trimmed.strip_prefix("return")?;
                if !rest.starts_with(char::is_whitespace) {
                    return None;
                }
                if self.recent_base_trailing_return_function_header() {
                    lead
                } else {
                    lead + "return".len() + (rest.len() - rest.trimmed_start().len())
                }
            }
            TernaryOwnerRole::Assignment => {
                let (operator_index, operator) = find_assignment_operator(code)?;
                let after = &code[operator_index + operator.len()..];
                operator_index + operator.len() + (after.len() - after.trimmed_start().len())
            }
            TernaryOwnerRole::Other => return None,
        };
        Some(
            self.current_line_indent_spaces()
                + visual_width_from(&code[..operand_byte], 0, self.options.tab_width),
        )
    }

    fn push_question(&mut self, next: Option<&Token>) {
        let pad_off =
            !self.options.pad_operators || self.layout.line_state.operator_padding_disabled;
        let should_pad = !pad_off && !self.is_in_case_label_expression();
        if should_pad {
            self.emit_source_space_or_ensure();
        } else if pad_off {
            self.emit_source_space();
        } else {
            self.trim_current_end();
        }
        let bare_question_line = matches!(next, None | Some(Token::Newline));
        let parent_delimiter = self.layout.frame_stack.active_delimiter_with_id();
        let owner_role = self.ternary_owner_role_for_question();
        self.layout.frame_stack.push_ternary(TernaryFrame {
            owner_role,
            parent_delimiter: parent_delimiter.map(|(id, _)| id),
            question_indent_spaces: self.current_line_indent_spaces(),
            branch_anchor_column: parent_delimiter
                .map(|(_, delimiter)| delimiter.opener_output_column + 1)
                .or_else(|| self.ternary_condition_operand_anchor(owner_role)),
            colon_role: None,
            colon_output_column: None,
        });
        self.layout.nesting.enter_question();
        self.current.push('?');
        self.layout.command_state.observe_char('?');
        if should_pad {
            self.emit_trailing_source_space_or_ensure();
        } else if pad_off {
            self.emit_trailing_source_space();
        }
        self.layout.previous = if bare_question_line {
            PreviousToken::Other
        } else {
            PreviousToken::Operator
        };
        self.previous_was_newline = false;
    }
}

fn is_semicolonless_macro_call_name(name: &str) -> bool {
    let macro_part = name.strip_prefix("wx").unwrap_or(name);
    !macro_part.is_empty()
        && macro_part
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        && macro_part
            .chars()
            .any(|ch| ch.is_ascii_uppercase() || ch == '_')
}
