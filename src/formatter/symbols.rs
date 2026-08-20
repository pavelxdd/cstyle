use super::brace_classification::is_class_like_brace_type;
use super::columns::{leading_visual_width, visual_width_from};
use super::frame::{
    ArgumentFrame, BraceSemanticKind, BracketFrame, BracketRole, CallFrame, ColonRole, CommaRole,
    DelimiterFrame, ParenRole, TernaryFrame, TernaryOwnerRole,
};
use super::labels;
use super::language::{self, is_leading_continuation_operator, is_numeric_variable_word};
use super::line_scan::{has_unclosed_delimiter_after, trailing_matching_parens};
use super::operators::find_assignment_operator;
use super::syntax::{
    assignment_declarator_offset, scoped_name_is_constructor, signature_ends_with_parameter_list,
};
use super::token::Token;
use super::{
    FormatEngine, PreviousToken, is_pointer_type_word, is_type_like_pointer_word,
    trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::config::{BraceStyle, FormatOptions, Mode, ObjCColonPad, PointerAlign};
use crate::source::lex::{is_identifier_continue, is_word_char, trailing_word};

fn should_keep_unpad_space_before_paren(word: &str, options: &FormatOptions) -> bool {
    options.unpad_parens
        && (matches!(word, language::RETURN | "and" | "or" | "in")
            || (options.pad_header
                && matches!(word, language::NEW | language::DELETE | language::THROW))
            || is_numeric_variable_word(word))
}

pub(super) fn close_paren_out_suppressed(token: &Token) -> bool {
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
                let prev = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                if matches!(
                    prev,
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
                let head = line[..i].trim();
                if head.is_empty()
                    || head.contains([' ', '\t', ',', '(', ')', '{', '}'])
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

impl FormatEngine<'_> {
    pub(super) fn push_symbol(
        &mut self,
        symbol: char,
        next: Option<&Token>,
        next_is_adjacent: bool,
        token_index: usize,
        starts_initializer_designator: bool,
        inferred_definition_brace: bool,
        following_closing_braces: usize,
    ) {
        match symbol {
            '{' => self.push_open_brace(next, token_index, inferred_definition_brace),
            '}' => self.push_close_brace(next, next_is_adjacent),
            '(' => self.push_open_paren(next),
            ')' => self.push_close_paren(next, next_is_adjacent),
            '[' => self.push_open_bracket(next, starts_initializer_designator),
            ']' => self.push_close_bracket(),
            ';' => self.push_semicolon(next, following_closing_braces),
            ',' => self.push_comma(next),
            ':' => self.push_colon(next),
            '?' => self.push_question(next),
            '.' => self.push_dot(next),
            '#' => {
                self.emit_source_space();
                self.current.push('#');
                self.emit_trailing_source_space();
                self.command_state.observe_char('#');
                self.previous = PreviousToken::Other;
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
                    if self.current.trim_end().ends_with('}')
                        || (self.options.pad_operators && self.previous == PreviousToken::Operator)
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
                self.command_state.observe_char('@');
                self.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
            '\\' => {
                if !self.current.ends_with([' ', '\t']) {
                    self.emit_source_space();
                }
                self.current.push('\\');
                self.command_state.observe_char('\\');
                self.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
            _ => {
                self.current.push(symbol);
                self.command_state.observe_char(symbol);
                self.previous = PreviousToken::Other;
                self.previous_was_newline = false;
            }
        }
    }

    fn push_dot(&mut self, next: Option<&Token>) {
        let keeps_padded_space = ((self.previous == PreviousToken::Comma
            && (self.options.pad_commas || self.options.pad_operators))
            || (self.previous == PreviousToken::Operator && self.options.pad_operators))
            && self.token_input.previous_input_whitespace.is_none()
            && self.current.ends_with(' ');
        if !self.current.ends_with('.')
            && self.previous == PreviousToken::OpenParen
            && self.options.pad_parens_inside
        {
            self.pad_inside_paren_space();
        } else if !self.current.ends_with('.')
            && self.previous == PreviousToken::Comma
            && self.options.pad_operators
        {
            self.emit_source_space_or_ensure();
        } else if !self.current.ends_with('.') && !keeps_padded_space {
            self.emit_source_space();
        }
        self.current.push('.');
        self.command_state.observe_char('.');
        if self.current.ends_with("...") || !matches!(next, Some(Token::Symbol('.'))) {
            self.emit_trailing_source_space();
        }
        self.previous = PreviousToken::Other;
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
        } else if self.previous == PreviousToken::Word && !current_word.is_empty() {
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
            || self.current.trim_start() != current_word
            || self.token_input.token_source_column
                != self.token_input.token_source_line_indent + current_word.chars().count()
        {
            return base;
        }
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        else {
            return base;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
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
        let opens_header_paren = self.previous == PreviousToken::Word
            && current_word != "case"
            && self.is_header(&current_word);
        if opens_header_paren
            && self.token_input.token_begins_source_line
            && !self.current.trim().is_empty()
        {
            let spaces = self.current_line_indent_spaces();
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        let handled_objc_return_paren = self.objc.post_prefix;
        if handled_objc_return_paren {
            self.objc.post_prefix = false;
            self.objc.return_paren_depth = Some(self.stack_state.paren_depth + 1);
        }
        let handled_objc_param_paren = self.objc.post_method_colon;
        if handled_objc_param_paren {
            self.objc.post_method_colon = false;
            self.objc.param_paren_depth = Some(self.stack_state.paren_depth + 1);
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
                    && self.previous != PreviousToken::OpenParen));
        if self.previous == PreviousToken::Word {
            let word = trailing_word(&self.current);
            let keep_source_space = !next_is_close
                && (is_pointer_type_word(word) || is_type_like_pointer_word(word))
                && matches!(next, Some(Token::Operator(op)) if matches!(op.as_str(), "*" | "&" | "^"));
            let keep_unpad_space =
                should_keep_unpad_space_before_paren(word, self.options) || keep_source_space;
            let force_space = (matches!(word, "and" | "or")
                && self.options.pad_operators
                && !self.line_state.operator_padding_disabled)
                || (self.options.pad_header
                    && (self.is_header(word)
                        || matches!(word, "return" | "new" | "delete")
                        || word == "throw" && !self.throw_is_exception_specification()))
                || outside_pad;
            if force_space {
                self.pad_before_open_paren_space();
            } else if keep_unpad_space {
                self.emit_source_space();
            } else if self.options.unpad_parens {
                self.trim_current_end();
            } else {
                self.emit_source_space();
            }
        } else if !outside_pad
            && self.previous == PreviousToken::Operator
            && self.options.pointer_align == PointerAlign::Name
            && self.current.trim_end().ends_with(['*', '&', '^'])
            && self.looks_like_pointer_declaration_context()
        {
            if !self.function_pointer_parameter_keeps_space_before_name_group() {
                self.trim_current_end();
            }
        } else if outside_pad {
            self.pad_before_open_paren_space();
        } else if !handled_objc_return_paren
            && !handled_objc_param_paren
            && !self.options.unpad_parens
            && !((self.previous == PreviousToken::Operator
                && self.options.pad_operators
                && self.token_input.previous_input_whitespace.is_none()
                && self.current.ends_with(' '))
                || (self.previous == PreviousToken::Comma
                    && (self.options.pad_commas || self.options.pad_operators)
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' '))
                || (self.previous == PreviousToken::OpenParen
                    && self.options.pad_parens_inside
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' '))
                || (self.line_state.ternary_colon
                    && self.options.pad_operators
                    && self.token_input.previous_input_whitespace.is_none()
                    && self.current.ends_with(' ')))
        {
            self.emit_source_space();
        }
        if self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && let Some(previous) = self.previous_pre_adjust_line.as_ref()
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if code.trim_start().starts_with("return new ") {
                let spaces =
                    leading_visual_width(previous, self.options.tab_width) + "return ".len();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
            } else if let Some(spaces) =
                self.constructor_initializer_name_indent_from_line(previous)
            {
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
            }
        }
        let semicolonless_macro_call_indent = is_semicolonless_macro_call_name(self.current.trim())
            .then(|| self.current_line_indent_spaces());
        let inline_brace_call_indent = self.inline_brace_call_indent_spaces(&self.current);
        let paren_indent_spaces =
            if matches!(next, None | Some(Token::Newline)) || self.options.indent_after_parens {
                inline_brace_call_indent
                    .or_else(|| self.stack_state.current_continuation_indent_spaces())
                    .unwrap_or_else(|| {
                        let prefix_len = self.current.len() - self.current.trim_start().len();
                        if prefix_len > 0 && self.current.trim().is_empty() {
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
            let logical_chain_indent = self.continuation_indent.logical_chain_indent_spaces;
            let logical_operand_indent_column = logical_chain_indent
                .or_else(|| self.return_continuation_indent_spaces())
                .or_else(|| self.assignment_continuation_indent_spaces())
                .or_else(|| self.stack_state.current_continuation_indent_spaces())
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
        self.command_state.observe_char('(');
        self.stack_state.enter_paren(
            paren_indent_spaces,
            inline_brace_call_indent.is_some(),
            semicolonless_macro_call_indent,
        );
        let lambda_parameter_list = self.current_paren_is_lambda_parameter_list();
        self.frame_stack.push_delimiter(DelimiterFrame {
            role: paren_role,
            lambda_parameter_list,
            opener_output_column,
            opener_output_line: self.output.len(),
            line_indent_spaces: opener_line_indent,
            continuation_indent_column: None,
            call: call_frame,
        });
        if opens_header_paren {
            self.header_paren.depth = Some(self.stack_state.paren_depth);
        }
        self.state.enter_paren();
        self.register_current_continuation_indent(next);
        let continuation_indent = self.stack_state.current_continuation_indent_spaces();
        if let Some((_, delimiter)) = self.frame_stack.active_delimiter_mut() {
            delimiter.continuation_indent_column = continuation_indent;
        }
        self.previous = PreviousToken::OpenParen;
        self.previous_was_newline = false;
    }

    fn throw_is_exception_specification(&self) -> bool {
        let line = self.current.trim_end();
        let Some(prefix) = line.strip_suffix("throw").map(str::trim_end) else {
            return false;
        };
        let Some((open, close)) = trailing_matching_parens(prefix) else {
            return false;
        };
        if close + 1 != prefix.len() {
            return false;
        }
        let head = prefix[..open].trim_end();
        self.paren_head_is_declaration(head) || scoped_name_is_constructor(head)
    }

    fn push_close_paren(&mut self, next: Option<&Token>, next_is_adjacent: bool) {
        let is_objc_return_close =
            self.objc.return_paren_depth == Some(self.stack_state.paren_depth);
        if is_objc_return_close {
            self.objc.return_paren_depth = None;
        }
        let is_objc_param_close = self.objc.param_paren_depth == Some(self.stack_state.paren_depth);
        if is_objc_param_close {
            self.objc.param_paren_depth = None;
        }
        if self.current.trim().is_empty()
            && let Some(spaces) = self.stack_state.current_paren_indent_spaces()
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
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        let close_paren_out = self.options.pad_parens_outside
            && !self.options.unpad_parens
            && self.previous == PreviousToken::CloseParen;
        let keeps_converted_pointer_gap = self.options.convert_tabs
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|gap| gap.contains('\t'))
            && self.current.ends_with(' ')
            && self.current.trim_end().ends_with(['*', '&', '^']);
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
        self.block_comment_close_paren_ends_declaration = self.current_is_preindented
            && self.current.trim_start().starts_with('*')
            && self.current.trim_end().ends_with("*/")
            && self.current_paren_context_is_declaration();
        self.current.push(')');
        self.space_after_cast = self.current_ends_cast() && !next_is_adjacent;
        self.command_state.observe_char(')');
        if self.header_paren.depth == Some(self.stack_state.paren_depth) {
            self.header_paren.depth = None;
            self.header_paren.just_closed = true;
        }
        let closes_semicolonless_macro_call_indent = self
            .stack_state
            .current_paren_semicolonless_macro_call_indent()
            .filter(|_| {
                !is_semicolonless_macro_call_name(
                    self.current
                        .trim_start()
                        .split_once('(')
                        .map_or("", |(name, _)| name.trim()),
                )
            });
        if self.compound_literal.arg_paren_depth == Some(self.stack_state.paren_depth) {
            self.compound_literal.arg_indent_spaces = None;
            self.compound_literal.arg_paren_depth = None;
            self.compound_literal.arg_brace_depth = None;
            self.compound_literal.after_comma = false;
        }
        self.stack_state.exit_paren();
        self.frame_stack.pop_delimiter(self.output.len());
        self.state.exit_paren();
        if self.stack_state.paren_depth == 0
            && !matches!(next, Some(Token::Symbol(';')))
            && let Some(spaces) = closes_semicolonless_macro_call_indent
        {
            self.continuation_indent.clear_continuation_after_line = Some(spaces);
        }
        self.previous = PreviousToken::CloseParen;
        self.previous_was_newline = false;
        self.pad_close_paren_pending = self.options.pad_parens_outside;
        if is_objc_return_close {
            if self.options.pad_return_type {
                self.objc.after_paren_pad = Some(true);
                self.space_after_cast = true;
            } else if self.options.unpad_return_type {
                self.objc.after_paren_pad = Some(false);
                self.space_after_cast = false;
                self.pad_close_paren_pending = false;
            }
        }
        if is_objc_param_close {
            if self.options.pad_param_type {
                self.objc.after_paren_pad = Some(true);
                self.space_after_cast = true;
            } else if self.options.unpad_param_type {
                self.objc.after_paren_pad = Some(false);
                self.space_after_cast = false;
                self.pad_close_paren_pending = false;
            }
        }
    }

    fn push_open_bracket(&mut self, next: Option<&Token>, starts_initializer_designator: bool) {
        let opens_operator_name = self.current.trim_end().ends_with("operator");
        let opens_designator = self.inline_array.initializer_designator_bracket_depth > 0
            || (starts_initializer_designator && self.bracket_opens_initializer_designator());
        let opens_collection = !opens_designator && self.current.trim_end().ends_with('@');
        let opens_message = !opens_collection
            && !opens_designator
            && self.frame_stack.bracket_depth() == 0
            && self.bracket_opens_objc_message();
        let bracket_role = if opens_collection {
            BracketRole::ObjectiveCCollection
        } else if opens_message {
            BracketRole::ObjectiveCMessage
        } else {
            BracketRole::Other
        };
        let current = self.current.trim_end();
        let keeps_padded_objc_selector_gap = self.objc.message_active
            && current.ends_with(':')
            && matches!(
                self.options.pad_method_colon,
                ObjCColonPad::All | ObjCColonPad::After
            );
        let parent_objc_message_align = (self.frame_stack.bracket_depth() > 0
            && self.objc.message_active)
            .then_some(self.objc.message_align)
            .flatten();
        let opens_after_selector = parent_objc_message_align.is_some()
            && current
                .trim_start()
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
            declarator_operator.map(|operator| self.resolved_pointer_align(operator));
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
            && self.current.ends_with([' ', '\t']);
        let keeps_padded_comma_gap = self.previous == PreviousToken::Comma
            && (self.options.pad_commas || self.options.pad_operators)
            && self.token_input.previous_input_whitespace.is_none()
            && self.current.ends_with(' ');
        if attaches_to_name_side {
            self.trim_current_end();
        } else if keeps_padded_objc_selector_gap {
            self.ensure_space();
        } else if self.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
            self.pad_inside_paren_space();
        } else if self.previous == PreviousToken::OpenParen && self.options.unpad_parens {
            self.trim_current_end();
        } else if !keeps_aligned_declarator_gap
            && !keeps_padded_comma_gap
            && !(self.previous == PreviousToken::Operator
                && self.options.pad_operators
                && self.token_input.previous_input_whitespace.is_none()
                && self.current.ends_with(' '))
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
        self.command_state.observe_char('[');
        if opens_designator {
            self.inline_array.initializer_designator_bracket_depth += 1;
        } else if !opens_operator_name {
            self.state.enter_bracket();
            self.frame_stack.push_bracket(BracketFrame {
                opener_output_column,
                opener_output_line: self.output.len(),
                line_indent_spaces: opener_line_indent,
                role: bracket_role,
                parent_objc_message_align,
                opens_after_selector,
            });
        }
        if bracket_role != BracketRole::Other {
            self.objc.message_active = true;
            self.objc.message_pending_align = true;
        }
        self.previous = PreviousToken::OpenBracket;
        self.previous_was_newline = false;
    }

    fn bracket_opens_initializer_designator(&self) -> bool {
        self.options.mode == Mode::C
            && self.token_input.token_begins_source_line
            && self.current.trim().is_empty()
    }

    fn bracket_opens_objc_message(&self) -> bool {
        if self.current.trim_end().ends_with("return") {
            return true;
        }
        match self.command_state.previous_non_ws_char {
            None => true,
            Some(')') if self.options.mode == Mode::ObjC => true,
            Some(ch) => !is_word_char(ch) && ch != ']' && ch != ')',
        }
    }

    fn push_close_bracket(&mut self) {
        if self.token_input.token_begins_source_line
            && self.current.trim().is_empty()
            && self.inline_array.initializer_designator_bracket_depth == 0
            && let Some(frame) = self.frame_stack.active_bracket()
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(match frame.role {
                BracketRole::ObjectiveCCollection => frame.opener_output_column.saturating_sub(1),
                BracketRole::Other | BracketRole::ObjectiveCMessage => frame.opener_output_column,
            });
        }
        if self.token_input.token_begins_source_line
            && !self.current.trim().is_empty()
            && self.current.trim_start().starts_with('>')
        {
            self.finish_line();
        }
        self.emit_source_space();
        self.current.push(']');
        self.command_state.observe_char(']');
        if self.inline_array.initializer_designator_bracket_depth > 0 {
            self.inline_array.initializer_designator_bracket_depth -= 1;
            self.previous = PreviousToken::CloseBracket;
            self.previous_was_newline = false;
            return;
        }
        let closes_operator_name = self.current.trim_end().ends_with("operator[]");
        let closing_nested_message_argument = self.frame_stack.bracket_depth() > 1
            && self.objc.message_active
            && self.current.contains(':')
            && !self.current.trim_start().starts_with('[');
        if !closes_operator_name {
            self.state.exit_bracket();
            self.frame_stack.pop_bracket();
        }
        if self.frame_stack.bracket_depth() == 0 {
            self.objc.message_active = false;
        } else if closing_nested_message_argument {
            self.objc.message_align = None;
        }
        self.previous = PreviousToken::CloseBracket;
        self.previous_was_newline = false;
    }

    fn push_semicolon(&mut self, next: Option<&Token>, following_closing_braces: usize) {
        let suffix_width = if matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) {
            following_closing_braces * 2
        } else {
            0
        };
        self.max_length_line.set_suffix_width(suffix_width);
        let closed_lambda_header_indent = self
            .current
            .trim_end()
            .ends_with('}')
            .then(|| {
                self.frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
                    .map(|frame| frame.header_indent_column)
            })
            .flatten();
        self.in_class_base_clause = false;
        self.unmatched_closing_brace_recovery = false;
        if self.one_line_block_mode {
            self.push_inline_semicolon(next);
            return;
        }
        if self.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
            self.pad_inside_paren_space();
        } else if self.previous == PreviousToken::OpenParen && self.options.unpad_parens {
            self.trim_current_end();
        } else {
            self.emit_source_space();
        }
        self.current.push(';');
        self.command_state.observe_char(';');
        self.line_state.passed_semicolon = true;
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        let closing_header_follows = matches!(
            (self.command_state.current_header.as_deref(), next),
            (Some("if"), Some(Token::Word(word))) if word == "else"
        ) || matches!(
            (self.command_state.current_header.as_deref(), next),
            (Some("try" | "catch"), Some(Token::Word(word))) if word == "catch"
        ) || matches!(
            (self.command_state.current_header.as_deref(), next),
            (Some("do"), Some(Token::Word(word))) if word == "while"
        );
        let completed_header_in_outer_delimiter = self
            .frame_stack
            .active_header()
            .is_some_and(|header| header.parent_delimiter.is_some())
            && self.command_state.current_header.is_some()
            && self.header_paren.depth.is_none()
            && !closing_header_follows;
        if completed_header_in_outer_delimiter {
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.frame_stack.clear_header();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            if let Some((base, delta)) = self.state.last_braceless_block()
                && self.state.indent() == base + delta
            {
                self.state.exit_braceless_block();
            }
        }
        if self.state.statement_depth() == 0 {
            self.state.clear_continuation_indents();
            self.stack_state.clear_continuation_indents();
            let closed_questions = self.stack_state.truncate_questions_to_brace_scope();
            for _ in 0..closed_questions {
                self.frame_stack.pop_active_ternary();
            }
            self.frame_stack.pop_completed_ternaries();
            self.continuation_indent.logical_chain_indent_spaces = None;
            self.multi_declarator_indent_spaces = None;
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.command_state.pending_block_word = None;
            self.pending_extern = false;
            self.header_paren.depth = None;
            self.frame_stack.truncate_brackets(0);
            self.objc.method_continuation = false;
            self.objc.message_active = false;
            let following_header = matches!(
                next,
                Some(Token::Word(word))
                    if self.is_header(word) && !matches!(word.as_str(), "case" | "default")
            );
            let break_expanded_lisp_header = self.options.brace_style == BraceStyle::Lisp
                && self.line_state.is_one_line_block
                && following_header;
            let keep_following_header = !break_expanded_lisp_header
                && !self.options.break_one_line_headers
                && self.options.keeps_multi_statement_line()
                && following_header;
            if matches!(next, Some(Token::Comment(_, _))) {
                self.emit_trailing_source_space();
                self.schedule_block_spacing_semicolon();
            } else if matches!(next, Some(Token::Symbol(';'))) {
                self.trim_current_end();
            } else if matches!(next, Some(Token::Operator(operator)) if is_leading_continuation_operator(operator))
                || matches!(next, Some(Token::Symbol('.')))
            {
                self.emit_trailing_source_space();
            } else if self.options.brace_style == BraceStyle::Pico
                && (self.token_input.token_line_opens_with_brace
                    || self.output.last().is_some_and(|line| line.trim() == "{"))
            {
                self.emit_trailing_source_space_or_ensure();
            } else if break_expanded_lisp_header
                || (!keep_following_header
                    && (self.options.break_one_line_statements
                        || !self.line_state.is_multi_statement_line
                        || (self.line_state.is_one_line_block
                            && self.options.break_one_line_blocks)))
            {
                self.finish_line();
                self.observe_block_spacing_semicolon();
            } else if !matches!(next, Some(Token::Symbol(';' | ')' | '}'))) {
                self.emit_trailing_source_space_or_ensure();
            } else {
                self.trim_current_end();
            }
            if !(matches!(next, Some(Token::Word(word)) if word == "else")
                && !self.else_if_break_depths.is_empty())
            {
                self.unwind_else_if_break_depths();
            }
            self.pending_braceless_block_bias = None;
            self.inline_nested_header_braceless_bias = None;
            if self.preprocessor.split_else.body_braceless
                || self.preprocessor.split_else.trigger_output_len == Some(usize::MAX)
            {
                self.preprocessor.split_else.pending_body = false;
                self.preprocessor.split_else.body_braceless = false;
                self.preprocessor.split_else.extra_indent = false;
                self.preprocessor.split_else.extra_levels = 0;
                self.preprocessor.split_else.trigger_output_len = None;
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            }
            while let Some((base, delta)) = self.state.last_braceless_block()
                && self.state.indent() == base + delta
                && !self.next_keeps_braceless_block(next, base)
            {
                self.state.exit_braceless_block();
            }
        } else {
            if self.current.trim_start().starts_with(':')
                && self.stack_state.paren_depth == 0
                && !self.stack_state.has_question_in_current_brace()
            {
                self.state.clear_continuation_indents();
                self.stack_state.clear_continuation_indents();
            } else if let Some(spaces) = self.for_header_continuation_indent_spaces() {
                self.stack_state.clear_continuation_indents();
                self.stack_state.register_continuation_indent_spaces(spaces);
            } else if self.stack_state.paren_depth == 0
                && !self.stack_state.has_question_in_current_brace()
            {
                self.state.clear_continuation_indents();
                self.stack_state.clear_continuation_indents();
                self.continuation_indent.logical_chain_indent_spaces = None;
                self.continuation_indent.next_line_indent_spaces = None;
            } else {
                self.stack_state.trim_to_current_statement_continuation();
            }
            if matches!(next, Some(Token::Symbol(';' | ')'))) {
                self.emit_trailing_source_space();
            } else {
                self.emit_trailing_source_space_or_ensure();
            }
            if self.stack_state.paren_depth == 0 {
                self.pending_braceless_block_bias = None;
                self.inline_nested_header_braceless_bias = None;
                while let Some((base, delta)) = self.state.last_braceless_block()
                    && self.state.indent() == base + delta
                    && !self.next_keeps_braceless_block(next, base)
                {
                    self.state.exit_braceless_block();
                }
            }
        }
        if let Some(spaces) = closed_lambda_header_indent
            && self.current_is_blank()
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
    }

    fn push_comma(&mut self, next: Option<&Token>) {
        let after_compound_literal = std::mem::take(&mut self.compound_literal.just_closed)
            && self.current.trim_end().ends_with('}');
        self.emit_source_space();
        self.current.push(',');
        self.command_state.observe_char(',');
        if after_compound_literal {
            let indent = self.current_line_indent_spaces();
            self.stack_state.clear_continuation_indents();
            self.compound_literal.after_comma = true;
            self.compound_literal.arg_indent_spaces = Some(indent);
            self.compound_literal.arg_paren_depth = Some(self.stack_state.paren_depth);
            self.compound_literal.arg_brace_depth = Some(self.stack_state.brace_header_stack.len());
        } else if self.compound_literal.arg_indent_spaces.is_some()
            && self.compound_literal.arg_paren_depth == Some(self.stack_state.paren_depth)
            && self.compound_literal.arg_brace_depth
                == Some(self.stack_state.brace_header_stack.len())
        {
            self.stack_state.clear_continuation_indents();
            self.compound_literal.after_comma = true;
        } else if self.state.statement_depth() > 0 || self.stack_state.paren_depth > 0 {
            self.stack_state.trim_to_current_statement_continuation();
        } else if self.innermost_brace_is_compound_literal() {
            self.stack_state.clear_continuation_indents();
        } else if !self.in_initializer_brace() && !self.in_aggregate_declaration_brace() {
            if self.multi_declarator_indent_spaces.is_none() {
                let line = self.current.trim_end();
                let prefix = line.len() - line.trim_start().len();
                if let Some(offset) = assignment_declarator_offset(line.trim_start()) {
                    let base = if prefix == 0 {
                        self.current_line_indent_spaces()
                    } else {
                        prefix
                    };
                    self.multi_declarator_indent_spaces = Some(base + offset);
                    self.stack_state.clear_continuation_indents();
                } else if is_single_lvalue_assignment(line.trim_start()) {
                    self.multi_declarator_indent_spaces = Some(self.current_line_indent_spaces());
                    self.stack_state.clear_continuation_indents();
                }
            } else {
                self.stack_state.clear_continuation_indents();
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
        self.previous = PreviousToken::Comma;
        self.previous_was_newline = false;
        let in_objc_dictionary_literal = self
            .output
            .iter()
            .rev()
            .take(64)
            .take_while(|line| !line.trim_end().ends_with(';'))
            .any(|line| line.contains("@ {"));
        if in_objc_dictionary_literal {
            let spaces = self.current_line_indent_spaces();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            if !matches!(next, Some(Token::Newline) | None) {
                self.finish_line();
                self.previous_was_newline = true;
            }
        }
    }

    fn comma_role_for_current_separator(&self, after_compound_literal: bool) -> CommaRole {
        if self
            .frame_stack
            .active_delimiter()
            .is_some_and(|delimiter| delimiter.role.is_call_like())
        {
            return CommaRole::CallArgument;
        }
        if after_compound_literal
            || (self.compound_literal.arg_indent_spaces.is_some()
                && self.compound_literal.arg_paren_depth == Some(self.stack_state.paren_depth)
                && self.compound_literal.arg_brace_depth
                    == Some(self.stack_state.brace_header_stack.len()))
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
        let line = self.current.trim_end();
        let body = line.trim_start();
        if self.multi_declarator_indent_spaces.is_some()
            || assignment_declarator_offset(body).is_some()
            || is_single_lvalue_assignment(body)
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
                .frame_stack
                .last_argument()
                .filter(|argument| argument.owner.is_none() && argument.role == role)
                .map_or(0, |argument| argument.index + 1),
            sibling_anchor_column: self.argument_sibling_anchor_column(role),
        };
        if role == CommaRole::CallArgument
            && let Some((owner, delimiter)) = self.frame_stack.active_delimiter_mut()
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
        self.frame_stack.set_last_argument(frame);
    }

    fn argument_sibling_anchor_column(&self, role: CommaRole) -> Option<usize> {
        match role {
            CommaRole::CallArgument => None,
            CommaRole::Declaration => self
                .multi_declarator_indent_spaces
                .or_else(|| Some(self.current_line_indent_spaces())),
            CommaRole::InitializerSibling | CommaRole::CompoundLiteralArgument => self
                .current_inline_array_column()
                .or(self.compound_literal.arg_indent_spaces)
                .or_else(|| Some(self.current_line_indent_spaces())),
            CommaRole::Other => None,
        }
    }

    fn push_colon(&mut self, next: Option<&Token>) {
        if self.current.trim_end().ends_with(':') && self.token_input.previous_input_was_adjacent {
            self.current.push(':');
            self.command_state.observe_char(':');
            self.emit_trailing_source_space();
            self.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        if self.current.trim().is_empty()
            && let Some(spaces) = self.previous_colon_continuation_indent_spaces()
        {
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        let is_asm_operand_colon = self.is_asm_operand_colon();
        let is_class_initializer = !is_asm_operand_colon
            && (self.is_class_initializer_colon()
                || (self.current.trim().is_empty() && self.colon_leads_class_initializer()));
        let function_try_initializer =
            is_class_initializer && self.class_initializer_follows_function_try();
        let is_objc_colon = self.is_objc_selector_or_message_colon();
        let is_objc_interface_colon = self.current.trim_start().starts_with("@interface ");
        let is_enum_underlying_type = self.is_enum_underlying_type_colon();
        let is_class_base = !is_asm_operand_colon
            && !is_objc_colon
            && !is_objc_interface_colon
            && self.colon_leads_class_base_clause();
        let is_bit_field = !is_asm_operand_colon
            && !is_class_initializer
            && !is_enum_underlying_type
            && self.is_bit_field_colon(next);
        let has_question = self.stack_state.has_question_in_current_brace();
        let is_range_for = !has_question && self.is_range_for_colon();
        let label_text = self
            .current
            .trim()
            .rsplit(['{', ';'])
            .next()
            .unwrap_or_default()
            .trim();
        let label_candidate = labels::is_label_start(label_text, &self.options.access_labels);
        let access_label_candidate =
            labels::is_access_label_start(label_text, &self.options.access_labels);
        let aligned_continuation_colon = !has_question
            && !is_range_for
            && !access_label_candidate
            && (is_asm_operand_colon || find_assignment_operator(&self.current).is_none())
            && (self.continuation_indent.next_line_indent_spaces.is_some()
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
        let case_label_colon = matches!(
            self.command_state.current_header.as_deref(),
            Some("case" | "default")
        ) && !self.command_state.case_label_colon_emitted
            && !is_ternary
            && self.stack_state.paren_depth == 0
            && self.frame_stack.bracket_depth() == 0
            && !matches!(next, Some(Token::Symbol(':')));
        self.command_state.case_label_colon_emitted = case_label_colon;
        let pad_off = !self.options.pad_operators || self.line_state.operator_padding_disabled;
        let in_objc_message = has_unclosed_delimiter_after(self.current.trim_end(), "[", "]");
        let is_objc_method_def_colon = is_objc_colon
            && !in_objc_message
            && (self.is_objc_method_line() || self.objc.method_continuation);
        let colon_mode = self.options.pad_method_colon;
        let next_is_close_paren = matches!(next, Some(Token::Symbol(')')));
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
            self.line_state.in_class_initializer = true;
            self.current_line_has_class_initializer_colon = true;
        }
        if is_class_base {
            self.in_class_base_clause = true;
            self.split_class_export_pending_base = false;
        }
        let break_after_ternary_colon = is_ternary
            && !matches!(
                next,
                Some(Token::Newline) | Some(Token::Comment(_, _)) | None
            )
            && (self
                .current
                .rfind('?')
                .is_some_and(|index| self.current[index + 1..].contains('{'))
                || (self.current.trim_end().ends_with('}')
                    && self.stack_state.last_closed_brace_type.is_some()));
        let colon_output_column = self
            .current_visual_width()
            .max(self.token_input.token_source_line_indent);
        if colon_role == ColonRole::ClassInitializer {
            self.record_constructor_initializer_frame(function_try_initializer);
        }
        if colon_role == ColonRole::Other
            || (colon_role != ColonRole::Ternary
                && find_assignment_operator(&self.current).is_some())
        {
            self.stack_state.clear_continuation_indents();
        }
        self.current.push(':');
        self.command_state.observe_char(':');
        if colon_role == ColonRole::Ternary
            && let Some(frame) = self.frame_stack.active_ternary_mut()
        {
            frame.colon_role = Some(colon_role);
            frame.colon_output_column = Some(colon_output_column);
        }
        self.line_state.passed_colon = true;
        if is_ternary {
            self.line_state.ternary_colon = true;
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
        self.objc.post_method_colon = is_objc_method_def_colon;
        if is_objc_colon && colon_mode != ObjCColonPad::NoChange && next_is_close_paren {
            self.objc.after_paren_pad = Some(false);
        }
        if !is_label {
            self.stack_state.exit_question();
        }
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        let in_objc_dictionary_literal = self.current.contains("@ {")
            || self
                .output
                .iter()
                .rev()
                .take(64)
                .take_while(|line| !line.trim_end().ends_with(';'))
                .any(|line| line.contains("@ {"));
        if in_objc_dictionary_literal
            && !self.one_line_block_mode
            && !is_objc_colon
            && !matches!(next, Some(Token::Newline) | None)
        {
            let spaces = if self.current.contains("@ {") {
                self.previous_pre_adjust_line
                    .as_ref()
                    .filter(|previous| previous.trim_end().ends_with('='))
                    .map(|previous| {
                        leading_visual_width(previous, self.options.tab_width)
                            + self.options.indent_width * 2
                    })
                    .unwrap_or_else(|| {
                        self.current_line_indent_spaces() + self.options.indent_width
                    })
            } else {
                self.previous_pre_adjust_line
                    .as_ref()
                    .filter(|previous| previous.trim_end().ends_with(','))
                    .map(|previous| leading_visual_width(previous, self.options.tab_width))
                    .unwrap_or_else(|| self.current_line_indent_spaces())
            };
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.previous_was_newline = true;
        } else if function_try_initializer
            && self.options.break_one_line_statements
            && !matches!(next, Some(Token::Newline | Token::Comment(_, _)) | None)
        {
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        } else if break_after_ternary_colon {
            let spaces = self.ternary_colon_break_indent_spaces();
            self.finish_line();
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = spaces;
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
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            } else if !matches!(
                next,
                Some(Token::Comment(_, _)) | Some(Token::Newline) | None
            ) {
                if self.options.pad_operators && !self.line_state.operator_padding_disabled {
                    self.emit_trailing_source_space_or_ensure();
                } else {
                    self.emit_trailing_source_space();
                }
            }
        } else if (is_class_initializer
            || (is_enum_underlying_type && pad_off)
            || (is_class_base && pad_off))
            && (!aligned_continuation_colon || is_asm_operand_colon)
        {
            self.emit_trailing_source_space();
        } else if is_asm_operand_colon {
            self.emit_trailing_source_space();
        } else if is_objc_colon {
            if colon_mode == ObjCColonPad::NoChange {
                self.emit_trailing_source_space();
            } else if !next_is_close_paren
                && matches!(colon_mode, ObjCColonPad::All | ObjCColonPad::After)
            {
                self.ensure_space();
            }
        } else if self.options.pad_operators && !self.line_state.operator_padding_disabled {
            if is_ternary || is_bit_field || (is_enum_underlying_type && !is_class_base) {
                self.emit_trailing_source_space_or_ensure();
            } else {
                self.ensure_space();
            }
        } else if (is_ternary || is_bit_field || is_range_for) && pad_off {
            self.emit_trailing_source_space();
        }
    }

    fn ternary_colon_break_indent_spaces(&self) -> Option<usize> {
        let line = self.current.as_str();
        let trimmed = line.trim_start();
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
        let current = self.current.trim_start();
        current.contains("asm(")
            || current.contains("__asm__(")
            || current.starts_with("asm ")
            || current.starts_with("asm\t")
            || current.starts_with("_asm ")
            || current.starts_with("__asm ")
            || current.starts_with("__asm__ ")
            || self
                .output
                .iter()
                .rev()
                .take(8)
                .take_while(|line| !line.trim_end().ends_with(';'))
                .any(|line| line.contains("asm"))
    }

    fn previous_colon_continuation_indent_spaces(&self) -> Option<usize> {
        let previous = self.output.last()?;
        let trimmed = previous.trim_start();
        if !trimmed.starts_with(':') {
            return None;
        }
        if unmatched_open_paren_column(trimmed).is_some() {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    fn is_range_for_colon(&self) -> bool {
        self.command_state.current_header.as_deref() == Some("for")
            && self.header_paren.depth == Some(self.stack_state.paren_depth)
    }

    fn is_enum_underlying_type_colon(&self) -> bool {
        if self.stack_state.has_question_in_current_brace() {
            return false;
        }
        let current = self.current.trim_end();
        let segment = current
            .rfind([';', '{', '}'])
            .map_or(current, |index| &current[index + 1..])
            .trim_start();
        segment == "enum" || segment.starts_with("enum ")
    }

    fn is_bit_field_colon(&self, next: Option<&Token>) -> bool {
        if !matches!(next, Some(Token::Number(_) | Token::Word(_))) {
            return false;
        }
        self.is_bit_field_segment(matches!(next, Some(Token::Number(_))))
    }

    fn is_bit_field_segment(&self, next_is_number: bool) -> bool {
        if !self.in_aggregate_declaration_brace()
            || self.stack_state.has_question_in_current_brace()
        {
            return false;
        }
        let current = self.current.trim_end();
        let segment = current
            .rfind([';', '{', '}'])
            .map_or(current, |index| &current[index + 1..])
            .trim();
        if segment.is_empty() || segment.contains('?') {
            return false;
        }
        let word_count = segment
            .split(|ch: char| !is_identifier_continue(ch))
            .filter(|word| !word.is_empty())
            .count();
        word_count >= 2 || (next_is_number && word_count >= 1)
    }

    pub(super) fn is_class_initializer_colon(&self) -> bool {
        let code = &self.current[..self.current_trailing_comment_split_limit()];
        self.code_is_class_initializer_signature(code.trim_end())
    }

    pub(super) fn class_initializer_follows_function_try(&self) -> bool {
        let code = self.current[..self.current_trailing_comment_split_limit()].trim_end();
        if trailing_word(code) == "try" {
            return true;
        }
        if !code.is_empty() {
            return false;
        }
        self.output
            .iter()
            .rev()
            .find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim();
                (!code.is_empty() && !code.starts_with(['#', '/', '*'])).then_some(code)
            })
            .is_some_and(|code| trailing_word(code) == "try")
    }

    fn code_is_class_initializer_signature(&self, code: &str) -> bool {
        if trailing_word(code) == "try" {
            let before_try = code[..code.len() - "try".len()].trim_end();
            if !before_try.is_empty() {
                return self.code_is_class_initializer_signature(before_try);
            }
            return self
                .output
                .iter()
                .rev()
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim();
                    (!code.is_empty() && code != "try" && !code.starts_with(['#', '/', '*']))
                        .then_some(code)
                })
                .is_some_and(|code| self.code_is_class_initializer_signature(code));
        }
        let block_comment_close_paren_signature = self.block_comment_close_paren_ends_declaration
            || (self.current.trim().is_empty()
                && self.previous_block_comment_close_paren_ended_declaration);
        !self.stack_state.has_question_in_current_brace()
            && (signature_ends_with_parameter_list(code) || block_comment_close_paren_signature)
            && (self
                .stack_state
                .brace_type_stack
                .iter()
                .any(|brace_type| is_class_like_brace_type(*brace_type))
                || self.code_ends_definition_header(code))
    }

    pub(super) fn colon_leads_class_initializer(&self) -> bool {
        if self.is_class_initializer_colon() {
            return true;
        }
        if !self.current[..self.current_trailing_comment_split_limit()]
            .trim()
            .is_empty()
        {
            return false;
        }
        let Some(line) = self.output.iter().rev().find(|line| {
            let trimmed = line.trim_start();
            !trimmed.is_empty()
                && !trimmed.starts_with('#')
                && !trimmed.starts_with("//")
                && !trimmed.starts_with("/*")
                && (!trimmed.starts_with('*')
                    || trimmed
                        .split_once("*/")
                        .is_some_and(|(_, suffix)| !suffix.trim().is_empty()))
        }) else {
            return false;
        };
        let code = &line[..trailing_comment_split_limit(line)];
        self.code_is_class_initializer_signature(code.trim_end())
    }

    fn ternary_owner_role_for_question(&self) -> TernaryOwnerRole {
        let current = self.current[..self.current_trailing_comment_split_limit()].trim_end();
        if current.trim_start().starts_with("return ") || current.trim_start() == "return" {
            return TernaryOwnerRole::Return;
        }
        if find_assignment_operator(current).is_some() {
            return TernaryOwnerRole::Assignment;
        }
        for raw in self.output.iter().rev().take(8) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
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
        let code = self.current[..self.current_trailing_comment_split_limit()].trim_end();
        let trimmed = code.trim_start();
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
                    lead + "return".len() + (rest.len() - rest.trim_start().len())
                }
            }
            TernaryOwnerRole::Assignment => {
                let (operator_index, operator) = find_assignment_operator(code)?;
                let after = &code[operator_index + operator.len()..];
                operator_index + operator.len() + (after.len() - after.trim_start().len())
            }
            TernaryOwnerRole::Other => return None,
        };
        Some(
            self.current_line_indent_spaces()
                + visual_width_from(&code[..operand_byte], 0, self.options.tab_width),
        )
    }

    fn push_question(&mut self, next: Option<&Token>) {
        let pad_off = !self.options.pad_operators || self.line_state.operator_padding_disabled;
        let should_pad = !pad_off && !self.is_in_case_label_expression();
        if should_pad {
            self.emit_source_space_or_ensure();
        } else if pad_off {
            self.emit_source_space();
        } else {
            self.trim_current_end();
        }
        let bare_question_line = matches!(next, None | Some(Token::Newline));
        let parent_delimiter = self.frame_stack.active_delimiter_with_id();
        let owner_role = self.ternary_owner_role_for_question();
        self.frame_stack.push_ternary(TernaryFrame {
            owner_role,
            parent_delimiter: parent_delimiter.map(|(id, _)| id),
            question_indent_spaces: self.current_line_indent_spaces(),
            branch_anchor_column: parent_delimiter
                .map(|(_, delimiter)| delimiter.opener_output_column + 1)
                .or_else(|| self.ternary_condition_operand_anchor(owner_role)),
            colon_role: None,
            colon_output_column: None,
        });
        self.stack_state.enter_question();
        self.current.push('?');
        self.command_state.observe_char('?');
        if should_pad {
            self.emit_trailing_source_space_or_ensure();
        } else if pad_off {
            self.emit_trailing_source_space();
        }
        self.previous = if bare_question_line {
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
