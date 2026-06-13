use super::columns::{leading_visual_width, visual_width_from};
use super::frame::{LogicalFrame, LogicalOperator, StreamFrame};
use super::language::{self, is_leading_continuation_operator};
use super::line_scan::has_unclosed_delimiter_after;
use super::line_scan::last_unmatched_open_delimiter;
use super::pointers::is_pointer_declaration_segment;
use super::state::FormatterBraceType;
use super::syntax::function_name_start;
use super::token::Token;
use super::{
    FormatEngine, OperatorRole, PreviousToken, TemplateAngle, is_macro_like_word,
    is_pointer_type_word, trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::config::{PointerAlign, ReferenceAlign};
use crate::source::lex::{is_identifier_continue, is_word_char, trailing_word};

pub(super) fn starts_ternary_arm(line: &str) -> bool {
    line.starts_with('?') || (line.starts_with(':') && !line.starts_with("::"))
}

pub(super) fn starts_with_chain_operator(line: &str) -> bool {
    if ["and", "or"].into_iter().any(|operator| {
        line.strip_prefix(operator).is_some_and(|tail| {
            tail.chars()
                .next()
                .is_none_or(|ch| !is_identifier_continue(ch))
        })
    }) {
        return true;
    }
    let bytes = line.as_bytes();
    match bytes.first() {
        Some(b'|' | b'^' | b'%') => true,
        Some(b'/') => !matches!(bytes.get(1), Some(b'/' | b'*')),
        Some(b'<') => matches!(bytes.get(1), Some(b'<' | b'=')),
        Some(b'>') => matches!(bytes.get(1), Some(b'>' | b'=')),
        Some(b'&') => matches!(bytes.get(1), Some(b'&')),
        Some(b'=' | b'!') => matches!(bytes.get(1), Some(b'=')),
        _ => false,
    }
}

pub(super) fn find_assignment_operator(line: &str) -> Option<(usize, &'static str)> {
    if !line.contains('=') {
        return None;
    }
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    while index < bytes.len() {
        let ch = bytes[index];
        let next = bytes.get(index + 1).copied();
        if in_block_comment {
            if ch == b'*' && next == Some(b'/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == b'/' && next == Some(b'/') {
            break;
        }
        if ch == b'/' && next == Some(b'*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == b'"' || ch == b'\'' {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
        if paren_depth == 0
            && bracket_depth == 0
            && matches!(
                ch,
                b'=' | b'+' | b'-' | b'*' | b'/' | b'%' | b'&' | b'|' | b'^' | b'<' | b'>'
            )
        {
            for &operator in language::ASSIGNMENT_OPERATORS {
                if line[index..].starts_with(operator)
                    && is_assignment_operator_boundary(line, index, operator)
                    && !operator_overload_token_precedes(line, index)
                {
                    return Some((index, operator));
                }
            }
        }
        index += 1;
    }
    None
}

fn operator_overload_token_precedes(line: &str, index: usize) -> bool {
    let before = line[..index].trim_end();
    before.ends_with("operator")
        && before[..before.len() - "operator".len()]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_identifier_continue(ch))
}

fn is_assignment_operator_boundary(line: &str, index: usize, operator: &str) -> bool {
    if operator != "=" {
        return true;
    }
    let previous = line[..index].chars().next_back();
    let next = line[index + operator.len()..].chars().next();
    !matches!(
        previous,
        Some('=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^')
    ) && !matches!(next, Some('=' | '>'))
}

pub(super) fn trailing_binary_operator_column(head: &str) -> Option<usize> {
    let head = head.trim_end();
    ["<<", ">>", "+", "-", "*", "/", "%", "|", "&", "^"]
        .iter()
        .find_map(|operator| {
            head.ends_with(operator)
                .then(|| head.len() - operator.len())
        })
        .filter(|_| !head.ends_with("++") && !head.ends_with("--") && !head.ends_with("->"))
}

pub(super) fn head_ends_binary_operator(head: &str) -> bool {
    let head = head.trim_end();
    ["<<", ">>", "+", "-", "*", "/", "%", "|", "&", "^"]
        .iter()
        .any(|operator| head.ends_with(operator))
        && !head.ends_with("++")
        && !head.ends_with("--")
        && !head.ends_with("->")
}

pub(super) fn head_ends_assignment_operator(head: &str) -> bool {
    let head = head.trim_end();
    let Some((start, operator)) = find_assignment_operator(head) else {
        return false;
    };
    start + operator.len() == head.len()
}

pub(super) fn head_starts_binary_operator(head: &str) -> bool {
    let head = head.trim_start();
    if ["and", "or"].into_iter().any(|operator| {
        head.strip_prefix(operator).is_some_and(|tail| {
            tail.chars()
                .next()
                .is_none_or(|ch| !is_identifier_continue(ch))
        })
    }) {
        return true;
    }
    if head.starts_with("++") || head.starts_with("--") {
        return false;
    }
    [
        "<<", ">>", "||", "&&", "+", "-", "*", "/", "%", "|", "&", "^",
    ]
    .iter()
    .any(|operator| head.starts_with(operator))
}

pub(super) fn starts_prefix_increment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("++") || trimmed.starts_with("--")
}

pub(super) fn is_prefix_increment_statement(line: &str) -> bool {
    starts_prefix_increment(line) && line.trim_end().ends_with(';')
}

impl FormatEngine<'_> {
    fn has_continuable_previous_statement(&self) -> bool {
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| {
                let trimmed = line.trim_end();
                !matches!(
                    trimmed.trim_start(),
                    "break" | "continue" | "throw" | "goto" | "co_return" | "co_yield" | "co_await"
                ) && !trimmed.contains('#')
                    && !trimmed.ends_with(';')
                    && !trimmed.ends_with('{')
                    && !trimmed.ends_with('}')
            })
    }

    pub(super) fn push_operator(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        next_is_adjacent: bool,
        following_operator: Option<&str>,
        template_angle: TemplateAngle,
        token_index: usize,
    ) {
        let statement = self
            .current
            .rsplit([';', '{', '}'])
            .next()
            .unwrap_or(&self.current)
            .trim_start();
        if matches!(operator, "*" | "&" | "&&" | "^")
            && statement.starts_with("using ")
            && statement.contains('=')
            && self.line_state.template_angle_depth == 0
        {
            self.emit_source_space();
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            self.command_state.observe_text(operator);
            self.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return;
        }
        let operator_role = self.operator_role_at(token_index);
        let split_rvalue_reference = operator == "&&"
            && self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && matches!(next, Some(Token::Word(_)))
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim();
                    !code.starts_with('#')
                        && !starts_with_chain_operator(code)
                        && last_unmatched_open_delimiter(code).is_none()
                        && is_pointer_declaration_segment(code)
                });
        if split_rvalue_reference {
            let indent_spaces = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map_or(0, |line| leading_visual_width(line, self.options.tab_width));
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(indent_spaces);
            self.continuation_indent.logical_chain_indent_spaces = None;
        }
        if matches!(operator, "&&" | "||") && !split_rvalue_reference {
            self.record_logical_operator_frame(operator);
        }
        if matches!(operator, "&&" | "||")
            && self.current.trim().is_empty()
            && !split_rvalue_reference
        {
            let current_operator = if operator == "&&" {
                LogicalOperator::And
            } else {
                LogicalOperator::Or
            };
            let previous_opens_nested_logical_group = self
                .output
                .len()
                .checked_sub(1)
                .and_then(|line| self.frame_stack.active_logical_on_output_line(line))
                .is_some_and(|frame| frame.operator != current_operator);
            let persisted_chain = if previous_opens_nested_logical_group {
                None
            } else {
                self.continuation_indent.logical_chain_indent_spaces
            };
            let chain_spaces = persisted_chain
                .or_else(|| self.previous_logical_continuation_indent_spaces(operator))
                .or_else(|| {
                    self.command_state.current_header.is_none().then(|| {
                        self.output
                            .iter()
                            .rev()
                            .find(|line| !line.trim().is_empty())
                            .and_then(|line| {
                                let line = line.trim_end();
                                let column = unmatched_open_paren_column(line)?;
                                let after = line[column + 1..].len()
                                    - line[column + 1..].trim_start().len();
                                Some(column + 1 + after)
                            })
                            .filter(|spaces| *spaces <= self.options.max_continuation_indent)
                    })?
                });
            if let Some(spaces) = chain_spaces {
                let spaces = if persisted_chain.is_some() && self.stack_state.paren_depth == 0 {
                    spaces
                } else {
                    match self.stack_state.current_continuation_indent_spaces() {
                        Some(paren) if paren < spaces => paren,
                        _ => spaces,
                    }
                };
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
                self.continuation_indent.logical_chain_indent_spaces = Some(spaces);
            } else if self.stack_state.paren_depth == 0
                && let Some(spaces) = self.continuation_indent.next_line_indent_spaces
            {
                self.continuation_indent.logical_chain_indent_spaces = Some(spaces);
            }
        }
        if matches!(operator, "<<" | ">>")
            && self.current.trim().is_empty()
            && self.stream_line_follows_multiline_braced_operand()
            && let Some(stream) = self.frame_stack.active_stream()
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(stream.chain_anchor_column);
        }
        if matches!(operator, "<<" | ">>")
            && self.current.trim().is_empty()
            && self.continuation_indent.next_line_indent.is_none()
            && self.continuation_indent.next_line_indent_spaces.is_none()
            && self.state.statement_depth() == 0
            && !self.in_initializer_brace()
            && self.has_continuable_previous_statement()
            && self
                .output
                .len()
                .checked_sub(1)
                .is_none_or(|previous_line| {
                    self.frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_none()
                })
        {
            let spaces = self.continuation_base_indent() * self.options.indent_width
                + 2 * self.options.indent_width;
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if self.skip_adjacent_pointer_operators > 0 && matches!(operator, "*" | "&" | "^") {
            self.skip_adjacent_pointer_operators -= 1;
            if self.resolved_pointer_align(operator) == PointerAlign::None
                && self.skip_adjacent_pointer_operators == 0
            {
                self.emit_trailing_source_space();
            }
            return;
        }
        match template_angle {
            TemplateAngle::Open => {
                self.emit_source_space();
                self.current.push('<');
                self.line_state.template_angle_depth += 1;
                self.emit_trailing_source_space();
                self.command_state.observe_text(operator);
                self.previous = PreviousToken::Operator;
                self.previous_was_newline = false;
                return;
            }
            TemplateAngle::Close(count) => {
                if self.options.close_templates && self.current.trim_end().ends_with('>') {
                    self.trim_current_end();
                } else {
                    self.emit_source_space();
                }
                self.current.push_str(operator);
                self.line_state.template_angle_depth =
                    self.line_state.template_angle_depth.saturating_sub(count);
                self.emit_trailing_source_space();
                self.command_state.observe_text(operator);
                self.previous = PreviousToken::Operator;
                self.previous_was_newline = false;
                if self.line_state.template_angle_depth == 0 {
                    self.previous_was_template_close = true;
                }
                return;
            }
            TemplateAngle::None => {}
        }
        if self.line_state.operator_padding_disabled {
            let keeps_non_operator_padding = self.current.ends_with([' ', '\t'])
                && ((self.previous == PreviousToken::OpenParen && self.options.pad_parens_inside)
                    || (self.previous == PreviousToken::Comma
                        && (self.options.pad_commas || self.options.pad_operators)));
            if !keeps_non_operator_padding {
                self.emit_source_space();
            }
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            self.command_state.observe_text(operator);
            self.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return;
        }

        if operator == "<?"
            && matches!(next, Some(Token::Operator(next_operator)) if next_operator == ">")
        {
            self.trim_current_end();
            self.current.push_str(operator);
            self.command_state.observe_text(operator);
            self.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return;
        }
        if operator == ">" && self.current.trim_end().ends_with('?') {
            self.current.push('>');
            self.command_state.observe_text(operator);
            self.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        if self.is_in_asm_operator_context() {
            if matches!(operator, "*" | "&" | "^") {
                self.emit_source_space();
            } else {
                self.trim_current_end();
            }
            self.current.push_str(operator);
            self.command_state.observe_text(operator);
            self.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return;
        }
        if self.current.trim().is_empty()
            && is_leading_continuation_operator(operator)
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line[..trailing_comment_split_limit(line)].trim() == "}")
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces =
                Some(self.state.indent() * self.options.indent_width);
            self.continuation_indent.logical_chain_indent_spaces = None;
        } else if self.current.trim().is_empty()
            && is_leading_continuation_operator(operator)
            && self.continuation_indent.next_line_indent_spaces.is_none()
            && !self.preprocessor.last_output_was_preprocessor
            && self.has_continuable_previous_statement()
        {
            let stale_level = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(|line| {
                    leading_visual_width(line, self.options.tab_width) / self.options.indent_width
                        + 1
                })
                .unwrap_or_else(|| self.state.indent() + 1);
            if self
                .continuation_indent
                .next_line_indent
                .is_some_and(|level| level > stale_level)
            {
                self.continuation_indent.next_line_indent = Some(stale_level);
            } else if self.continuation_indent.next_line_indent.is_none() {
                self.continuation_indent.next_line_indent_spaces = Some(
                    self.continuation_base_indent() * self.options.indent_width
                        + self.options.continuation_indent * self.options.indent_width,
                );
            }
        }

        match operator {
            "::" => {
                if self.previous == PreviousToken::OpenParen && self.options.pad_parens_inside {
                    self.pad_inside_paren_space();
                } else if self.previous == PreviousToken::OpenParen && self.options.unpad_parens {
                    self.trim_current_end_horizontal_space();
                } else if !(self.previous == PreviousToken::Comma
                    && (self.options.pad_commas || self.options.pad_operators)
                    && self.current.ends_with([' ', '\t']))
                {
                    self.emit_source_space();
                }
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "->" if self.is_trailing_return_arrow() => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "+" | "-" if self.is_objc_method_prefix(next) => {
                self.trim_current_end();
                self.current.push_str(operator);
                self.objc.post_prefix = true;
                if self.options.pad_method_prefix {
                    self.emit_trailing_source_space_or_ensure();
                } else if !self.options.unpad_method_prefix {
                    self.emit_trailing_source_space();
                }
            }
            "<" if self.is_template_declaration_line() => {
                self.emit_source_space();
                self.current.push('<');
                self.emit_trailing_source_space();
            }
            ">" if self.is_template_declaration_line() => {
                self.emit_source_space();
                self.current.push('>');
                self.emit_trailing_source_space();
            }
            "++" | "--" if self.is_prefix_increment_or_decrement() => {
                self.push_unary_prefix(operator);
            }
            "++" | "--" => {
                self.emit_source_space();
                self.current.push_str(operator);
            }
            "!" | "~" => self.push_unary_prefix(operator),
            "+" | "-"
                if self.current.trim().is_empty()
                    && self.state.statement_depth() > 0
                    && self.options.pad_operators
                    && self.line_start_sign_is_unary(next) =>
            {
                self.push_unary_prefix(operator);
            }
            "+" | "-"
                if self.current.trim().is_empty()
                    && self.state.statement_depth() > 0
                    && self.options.pad_operators =>
            {
                self.current.push_str(operator);
                self.ensure_space();
            }
            "+" | "-"
                if self.is_cast_unary_sign(next) || self.is_sizeof_typedef_unary_sign(next) =>
            {
                if self.options.pad_operators {
                    self.ensure_space();
                } else {
                    self.emit_source_space();
                }
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "+" | "-" if self.current_ends_size_operator_call() => {
                self.push_binary_operator(operator);
            }
            "+" | "-" if self.is_unary_sign() => {
                if self.current_ends_postfix_increment_or_decrement() {
                    if self.options.pad_operators {
                        self.ensure_space();
                    } else {
                        self.emit_source_space();
                    }
                }
                self.push_unary_prefix(operator);
            }
            "->" => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "..." => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            _ if trailing_word(&self.current) == language::OPERATOR => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "*" if self.should_attach_sizeof_after_standalone_call_argument(next) => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "*" if self.current_ends_numeric_cast()
                && !self.current_ends_pointer_cast()
                && self.current.chars().filter(|&ch| ch == '(').count()
                    <= self.current.chars().filter(|&ch| ch == ')').count()
                && matches!(next, Some(Token::Word(_))) =>
            {
                self.push_binary_operator(operator);
            }
            "*" if operator_role != OperatorRole::PointerDeclarator
                && self.stack_state.paren_depth > 0
                && matches!(next, Some(Token::Word(_)))
                && self.current_paren_is_expression_context()
                && !self.current_paren_context_is_declaration()
                && !self.current_ends_cast()
                && !self.is_unary_pointer_operator()
                && !matches!(following_operator, Some("=" | ":")) =>
            {
                self.push_binary_operator(operator);
            }
            "*" if self.current_ends_prefix_increment_or_decrement() => {
                self.push_unary_prefix(operator);
            }
            "*" if operator_role == OperatorRole::PointerDeclarator
                && self.current.trim_start().starts_with('(')
                && is_macro_like_word(trailing_word(&self.current))
                && matches!(next, Some(Token::Word(word)) if !is_macro_like_word(word)) =>
            {
                self.push_binary_operator(operator);
            }
            "*" if operator_role == OperatorRole::PointerDeclarator => {
                self.push_pointer_run(operator, next, next_is_adjacent);
            }
            "*" if operator_role == OperatorRole::UnaryOperator
                && (!self.is_pointer_like(
                    operator,
                    next,
                    next_is_adjacent,
                    following_operator,
                ) || (self.current.trim_end().ends_with('*')
                    && !self.looks_like_pointer_declaration_context())) =>
            {
                self.push_unary_prefix(operator);
            }
            "*" if operator_role == OperatorRole::BinaryOperator
                && !(self.current_ends_cast()
                    && self.current.chars().filter(|&ch| ch == '(').count()
                        > self.current.chars().filter(|&ch| ch == ')').count())
                && !self.is_pointer_like(operator, next, next_is_adjacent, following_operator) =>
            {
                self.push_binary_operator(operator);
            }
            "&" if matches!(next, Some(Token::Symbol('[')))
                && self.previous == PreviousToken::Word
                && trailing_word(&self.current) == "auto" =>
            {
                if self.resolved_pointer_align(operator) == PointerAlign::None {
                    self.push_unary_prefix(operator);
                } else {
                    self.push_pointer_or_reference(operator, next, next_is_adjacent);
                }
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && self.current.trim_start().starts_with("return ")
                && self.previous != PreviousToken::OpenParen
                && !is_pointer_type_word(trailing_word(&self.current)) =>
            {
                self.push_binary_operator(operator);
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && self.previous == PreviousToken::OpenParen =>
            {
                self.push_unary_prefix(operator);
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast()
                && (self.stack_state.paren_depth == 0
                    || !self.current_paren_started_by_expression_keyword()) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "&" if operator_role == OperatorRole::UnaryOperator
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast() =>
            {
                self.push_unary_prefix(operator);
            }
            "&" if operator_role == OperatorRole::BinaryOperator
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast()
                && !self.is_pointer_like(operator, next, next_is_adjacent, following_operator) =>
            {
                self.push_binary_operator(operator);
            }
            "&" | "*"
                if operator_role == OperatorRole::Unknown
                    && self.current_paren_context_is_declaration()
                    && self.looks_like_pointer_declaration_context()
                    && matches!(next, Some(Token::Word(_)) | Some(Token::Symbol(')' | ','))) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "&" if self.current_statement_contains_assignment()
                && !self.current_paren_is_lambda_parameter_list()
                && !self.is_pointer_like(operator, next, next_is_adjacent, following_operator)
                && matches!(
                    self.previous,
                    PreviousToken::Word
                        | PreviousToken::Literal
                        | PreviousToken::CloseParen
                        | PreviousToken::CloseBracket
                )
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast() =>
            {
                self.push_binary_operator(operator);
            }
            "&" if self.stack_state.paren_depth > 0
                && matches!(
                    self.previous,
                    PreviousToken::Word
                        | PreviousToken::Literal
                        | PreviousToken::CloseParen
                        | PreviousToken::CloseBracket
                )
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast()
                && !self.current_paren_context_is_declaration()
                && !self.is_pointer_like(operator, next, next_is_adjacent, following_operator) =>
            {
                self.push_binary_operator(operator);
            }
            "&" | "*" if self.current_ends_prefix_increment_or_decrement() => {
                self.push_unary_prefix(operator);
            }
            "&" | "*"
                if self.current_ends_postfix_increment_or_decrement()
                    && self.options.pad_operators =>
            {
                self.ensure_space();
                self.current.push_str(operator);
                self.ensure_space();
            }
            "*" if self.current_ends_sizeof_pointer_expr() => {
                self.push_binary_operator(operator);
            }
            "&" if self.stack_state.paren_depth > 0
                && self.current_paren_started_by_expression_keyword()
                && !self.is_unary_pointer_operator()
                && !self.is_pointer_like(operator, next, next_is_adjacent, following_operator) =>
            {
                self.push_binary_operator(operator);
            }
            "&" | "*"
                if self.current_ends_pointer_cast()
                    && self.options.pad_operators
                    && matches!(next, Some(Token::Symbol('('))) =>
            {
                self.ensure_space();
                self.current.push_str(operator);
                self.ensure_space();
            }
            "&" | "*" if self.current_ends_pointer_cast() => self.push_unary_prefix(operator),
            "&" if self.current_ends_cast()
                && !matches!(next, Some(Token::Symbol('(')))
                && self.stack_state.paren_depth == 0
                && matches!(
                    self.stack_state.brace_type_stack.last(),
                    Some(
                        FormatterBraceType::Array
                            | FormatterBraceType::Init
                            | FormatterBraceType::DeferArray
                    )
                ) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "&" if self.current_ends_cast() && !matches!(next, Some(Token::Symbol('('))) => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "&" | "*"
                if self.current_ends_cast()
                    && self.options.pad_operators
                    && self.current.chars().filter(|&ch| ch == '(').count()
                        > self.current.chars().filter(|&ch| ch == ')').count() =>
            {
                self.trim_current_end();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "&" | "*"
                if self.current_ends_cast()
                    && self.options.pad_operators
                    && self.stack_state.paren_depth > 0 =>
            {
                self.push_unary_prefix(operator);
            }
            "&" | "*"
                if self.current_ends_cast()
                    && self.options.pad_operators
                    && self.current.chars().filter(|&ch| ch == '(').count()
                        <= self.current.chars().filter(|&ch| ch == ')').count() =>
            {
                self.ensure_space();
                self.current.push_str(operator);
                self.ensure_space();
            }
            "&" | "*" if matches!(trailing_word(&self.current), "else" | "delete") => {
                self.ensure_space();
                self.current.push_str(operator);
            }
            "&&" if split_rvalue_reference
                || (!self.current_paren_is_expression_context()
                    || trailing_word(&self.current) == language::AUTO
                    || self.current.trim_end().ends_with('*')
                    || self.current_in_cast_type_group()
                    || self.current_in_parenthesized_type_operand()
                    || self.current_paren_context_is_declaration()
                    || self.current_paren_context_has_attached_return_type()
                    || self.is_function_declaration_parameter_continuation())
                    && self.is_rvalue_reference_like(next) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "&" | "*" | "^"
                if self.current.trim().is_empty()
                    && self.token_input.token_begins_source_line
                    && self.continuation_indent.next_line_indent_spaces.is_some()
                    && self.is_function_declaration_parameter_continuation()
                    && matches!(next, Some(Token::Word(_)) | Some(Token::Symbol(')' | ','))) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "*" if self.current.trim_end().ends_with('^') => self.push_unary_prefix(operator),
            "^" if self.current.trim_end().ends_with("++")
                || self.current.trim_end().ends_with("--") =>
            {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "&" if self.token_input.previous_input_was_adjacent
                && self.options.pad_operators
                && self.current.ends_with([' ', '\t'])
                && self.current.trim_end().ends_with('*')
                && self
                    .current
                    .trim_end()
                    .strip_suffix('*')
                    .is_some_and(|before| !before.trim_end().ends_with(['*', '&', '^']))
                && !self.looks_like_pointer_declaration_context() =>
            {
                self.push_unary_prefix(operator);
            }
            "&" | "*" | "^"
                if self.is_pointer_like(operator, next, next_is_adjacent, following_operator) =>
            {
                self.push_pointer_run(operator, next, next_is_adjacent);
            }
            "&" | "*" | "^" if self.is_unary_pointer_operator() => self.push_unary_prefix(operator),
            "&" | "*" if self.header_paren.post_paren => {
                self.emit_source_space();
                self.push_unary_prefix(operator);
            }
            "<<" | ">>" => self.push_binary_operator(operator),
            _ if language::ASSIGNMENT_OPERATORS.contains(&operator)
                && (self.current.ends_with(' ') || self.current.ends_with('\t'))
                && self.current.trim_end().ends_with(['*', '&', '^'])
                && (self.options.pointer_align != PointerAlign::None
                    || !matches!(
                        self.options.reference_align,
                        ReferenceAlign::None | ReferenceAlign::SameAsPointer
                    )) =>
            {
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            _ if self.options.pad_operators && self.is_in_case_label_expression() => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            _ if self.options.pad_operators => {
                self.emit_source_space_or_ensure();
                self.current.push_str(operator);
                self.emit_trailing_source_space_or_ensure();
            }
            _ => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
        }
        if language::ASSIGNMENT_OPERATORS.contains(&operator)
            && (!self.in_initializer_brace() || self.innermost_brace_is_compound_literal())
            && !self.in_aggregate_declaration_brace()
        {
            let rhs_next = if self.next_comment_ends_line {
                None
            } else {
                next
            };
            self.register_current_continuation_indent(rhs_next);
        }
        self.command_state.observe_text(operator);
        self.previous = PreviousToken::Operator;
        self.previous_was_newline = false;
    }

    pub(super) fn should_attach_sizeof_after_standalone_call_argument(
        &self,
        next: Option<&Token>,
    ) -> bool {
        if !matches!(next, Some(Token::Word(word)) if word == "sizeof")
            || !self.current.trim_end().ends_with(')')
            || !self.current.trim_start().starts_with('(')
        {
            return false;
        }
        for line in self.output.iter().rev().take(8) {
            let trimmed = line.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                break;
            }
            if !has_unclosed_delimiter_after(trimmed, "(", ")") {
                continue;
            }
            let Some(open) = trimmed.find('(') else {
                continue;
            };
            let before = trimmed[..open].trim();
            if !before.is_empty()
                && !before.contains('=')
                && !self.is_header(before)
                && matches!(function_name_start(before), Some(0))
            {
                return true;
            }
        }
        false
    }

    pub(super) fn is_trailing_return_arrow(&self) -> bool {
        let current = self.current.trim_end();
        current.starts_with("auto ") && current.ends_with(')')
    }

    pub(super) fn is_prefix_increment_or_decrement(&self) -> bool {
        !matches!(
            self.previous,
            PreviousToken::Word
                | PreviousToken::Literal
                | PreviousToken::CloseParen
                | PreviousToken::CloseBracket
        ) || trailing_word(&self.current) == "return"
    }

    pub(super) fn is_cast_unary_sign(&self, next: Option<&Token>) -> bool {
        matches!(next, Some(Token::Number(_))) && self.current_ends_numeric_cast()
    }

    pub(super) fn is_sizeof_typedef_unary_sign(&self, next: Option<&Token>) -> bool {
        if !matches!(next, Some(Token::Number(_))) {
            return false;
        }
        let current = self.current.trim_end();
        if !current.ends_with(')') {
            return false;
        }
        let Some(open) = current.rfind('(') else {
            return false;
        };
        if trailing_word(current[..open].trim_end()) != "sizeof" {
            return false;
        }
        is_pointer_type_word(trailing_word(&current[open + 1..current.len() - 1]))
    }

    pub(super) fn line_start_sign_is_unary(&self, next: Option<&Token>) -> bool {
        if !matches!(
            next,
            Some(Token::Word(_) | Token::Number(_) | Token::Symbol('('))
        ) {
            return false;
        }
        self.output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.ends_with(['(', ',', '=', '?', ':'])
                    || head_ends_binary_operator(code)
                    || code.trim_start().starts_with("return ")
            })
    }

    pub(super) fn is_unary_sign(&self) -> bool {
        matches!(
            self.previous,
            PreviousToken::None
                | PreviousToken::Operator
                | PreviousToken::OpenParen
                | PreviousToken::OpenBracket
                | PreviousToken::Comma
        ) || self.current.trim_end().ends_with([':', '{'])
            || matches!(trailing_word(&self.current), "return" | "case")
    }

    pub(super) fn current_ends_prefix_increment_or_decrement(&self) -> bool {
        let current = self.current.trim_end();
        let Some(before) = current
            .strip_suffix("++")
            .or_else(|| current.strip_suffix("--"))
        else {
            return false;
        };
        let before = before.trim_end();
        before.is_empty()
            || before.ends_with(['(', '[', '{', ',', '=', '?', ':'])
            || trailing_word(before) == "return"
            || head_ends_binary_operator(before)
    }

    pub(super) fn current_ends_postfix_increment_or_decrement(&self) -> bool {
        let current = self.current.trim_end();
        current.ends_with("++") || current.ends_with("--")
    }

    pub(super) fn is_in_case_label_expression(&self) -> bool {
        let current = self.current.trim_start();
        current
            .strip_prefix("case")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| !is_word_char(ch))
            && !current.contains(':')
    }

    pub(super) fn push_unary_prefix(&mut self, operator: &str) {
        let word = trailing_word(&self.current);
        let after_return = word == "return";
        let after_return_or_case = after_return || matches!(word, "case" | "do");
        if after_return_or_case {
            if self.options.pad_operators && after_return && matches!(operator, "+" | "-") {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.options.pad_parens_outside
            && self.previous == PreviousToken::CloseParen
            && self.current_ends_cast()
            && self.stack_state.paren_depth > 0
        {
            self.emit_source_space_or_ensure();
        } else if self.previous == PreviousToken::Comma {
            if self.options.pad_commas || self.options.pad_operators {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.previous == PreviousToken::Word || self.previous == PreviousToken::CloseParen
        {
            self.emit_source_space();
        }
        self.current.push_str(operator);
        self.emit_trailing_source_space();
    }

    fn stream_line_follows_multiline_braced_operand(&self) -> bool {
        if !self.current.trim().is_empty() {
            return false;
        }
        let Some(previous_line) = self.output.len().checked_sub(1) else {
            return false;
        };
        self.frame_stack.last_closed_brace().is_some_and(|brace| {
            brace.close_output_line == Some(previous_line)
                && brace.close_ends_output_line
                && self
                    .frame_stack
                    .active_stream_on_output_line(previous_line)
                    .is_none()
        })
    }

    fn record_logical_operator_frame(&mut self, operator: &str) {
        let logical_operator = match operator {
            "&&" => LogicalOperator::And,
            "||" => LogicalOperator::Or,
            _ => return,
        };
        let line_indent_spaces = self.current_line_indent_spaces();
        let operator_output_column = line_indent_spaces + self.current_visual_width();
        let operator_starts_output_line = self.current.trim().is_empty();
        let return_value_column = {
            let current = self.current.trim_start();
            current.starts_with("return ").then(|| {
                let prefix_len = self.current.len() - current.len();
                let after_return = &current["return".len()..];
                let value_offset = "return".len()
                    + after_return
                        .char_indices()
                        .find(|(_, ch)| !ch.is_whitespace())
                        .map_or(after_return.len(), |(index, _)| index);
                line_indent_spaces
                    + visual_width_from(
                        &self.current[..prefix_len + value_offset],
                        0,
                        self.options.tab_width,
                    )
            })
        };
        self.frame_stack.push_logical(LogicalFrame {
            operator: logical_operator,
            operator_output_column,
            operator_output_line: self.output.len(),
            line_indent_spaces,
            operator_starts_output_line,
            line_has_positive_paren_delta: false,
            line_ends_with_close_paren: false,
            line_unmatched_open_paren_column: None,
            return_value_column,
        });
    }

    fn record_stream_operator_frame(&mut self, operator: &str) {
        if !matches!(operator, "<<" | ">>") {
            return;
        }
        let line_indent_spaces = self.current_line_indent_spaces();
        let operator_output_column = line_indent_spaces + self.current_visual_width();
        let chain_anchor_column = self
            .frame_stack
            .active_stream()
            .map(|frame| frame.chain_anchor_column)
            .unwrap_or(operator_output_column);
        let assignment_value_start_column = find_assignment_operator(&self.current)
            .map(|(assignment, assignment_operator)| {
                self.current[assignment + assignment_operator.len()..]
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(self.current.len(), |(offset, _)| {
                        assignment + assignment_operator.len() + offset
                    })
            })
            .map(|value_start| {
                line_indent_spaces
                    + visual_width_from(&self.current[..value_start], 0, self.options.tab_width)
            });
        let after_multiline_braced_operand = self.stream_line_follows_multiline_braced_operand();
        self.frame_stack.push_stream(StreamFrame {
            operator_output_column,
            operator_output_line: self.output.len(),
            line_indent_spaces,
            operator_ends_output_line: false,
            line_contains_nested_brace: false,
            line_has_unmatched_open_paren: false,
            line_ends_with_close_paren: false,
            line_has_positive_paren_delta: false,
            chain_anchor_column,
            assignment_value_start_column,
            after_multiline_braced_operand,
        });
    }

    fn push_binary_operator(&mut self, operator: &str) {
        if matches!(operator, "<<" | ">>")
            && self.current.trim().is_empty()
            && self.stream_line_follows_multiline_braced_operand()
            && self.frame_stack.active_stream().is_some()
        {
            self.clear_current();
            self.record_stream_operator_frame(operator);
            self.current.push_str(operator);
            if self.options.pad_operators {
                self.emit_trailing_source_space_or_ensure();
            } else {
                self.emit_trailing_source_space();
            }
        } else if self.options.pad_operators && self.is_in_case_label_expression() {
            self.emit_source_space();
            self.record_stream_operator_frame(operator);
            self.current.push_str(operator);
            self.emit_trailing_source_space();
        } else if self.options.pad_operators {
            self.emit_source_space_or_ensure();
            self.record_stream_operator_frame(operator);
            self.current.push_str(operator);
            self.emit_trailing_source_space_or_ensure();
        } else {
            self.emit_source_space();
            self.record_stream_operator_frame(operator);
            self.current.push_str(operator);
            self.emit_trailing_source_space();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::find_assignment_operator;

    #[test]
    fn finds_top_level_assignment_operators() {
        assert_eq!(find_assignment_operator("alpha = beta"), Some((6, "=")));
        assert_eq!(find_assignment_operator("alpha >>= beta"), Some((6, ">>=")));
        assert_eq!(find_assignment_operator("alpha == beta"), None);
        assert_eq!(find_assignment_operator("call(alpha = beta)"), None);
    }
}
