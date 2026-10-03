use crate::config::{PointerAlign, ReferenceAlign};
use crate::formatter::constructs::headers::is_header;
use crate::formatter::engine::{FormatEngine, TokenPushContext};
use crate::formatter::lexer::Token;
use crate::formatter::state::frame::{LogicalFrame, LogicalOperator, StreamFrame};
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::structure::blocks::next_code_token;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::language::{
    self, is_leading_continuation_operator, is_macro_like_word, is_pointer_type_word,
};
use crate::formatter::syntax::{OperatorRole, TemplateAngle, function_name_start};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    has_unclosed_delimiter_after, last_unmatched_open_delimiter, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::formatter::tokens::pointers::{is_pointer_declaration_segment, resolved_pointer_align};
use crate::source::lex::{is_identifier_continue, is_word_char, trailing_word};

/// Whether `text` ends with a name alone in parentheses that no call
/// opens, as a cast to a type name does.
fn ends_parenthesized_name(text: &str) -> bool {
    let Some(inner) = text.trim_end().strip_suffix(')') else {
        return false;
    };
    let Some(open) = inner.rfind('(') else {
        return false;
    };
    let name = inner[open + 1..].trim();
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
        && !name.starts_with(|ch: char| ch.is_ascii_digit())
        && matches!(trailing_word(&inner[..open]), "" | "return")
}

pub(crate) fn starts_ternary_arm(line: &str) -> bool {
    line.starts_with('?') || (line.starts_with(':') && !line.starts_with("::"))
}

pub(crate) fn starts_with_chain_operator(line: &str) -> bool {
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

pub(crate) fn find_assignment_operator(line: &str) -> Option<(usize, &'static str)> {
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

pub(crate) fn trailing_binary_operator_column(head: &str) -> Option<usize> {
    let head = head.trim_end();
    ["<<", ">>", "+", "-", "*", "/", "%", "|", "&", "^"]
        .iter()
        .find_map(|operator| {
            head.ends_with(operator)
                .then(|| head.len() - operator.len())
        })
        .filter(|_| !head.ends_with("++") && !head.ends_with("--") && !head.ends_with("->"))
}

/// Column where a row continuing an array bound after `head` starts:
/// astyle aligns it under the trailing operator, or past the open `[`
/// once a bracket closed earlier on the line.
pub(crate) fn array_bound_operator_column(head: &str) -> Option<usize> {
    let open = crate::formatter::text::line_scan::unmatched_open_bracket_column(head)?;
    if head[..open].contains(']') {
        let after = &head[open + 1..];
        return Some(open + 1 + after.len() - after.trim_start().len());
    }
    trailing_binary_operator_column(head)
}

pub(crate) fn head_ends_binary_operator(head: &str) -> bool {
    let head = head.trim_end();
    ["<<", ">>", "+", "-", "*", "/", "%", "|", "&", "^"]
        .iter()
        .any(|operator| head.ends_with(operator))
        && !head.ends_with("++")
        && !head.ends_with("--")
        && !head.ends_with("->")
}

pub(crate) fn head_ends_assignment_operator(head: &str) -> bool {
    let head = head.trim_end();
    let Some((start, operator)) = find_assignment_operator(head) else {
        return false;
    };
    start + operator.len() == head.len()
}

pub(crate) fn head_starts_binary_operator(head: &str) -> bool {
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

pub(crate) fn starts_prefix_increment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("++") || trimmed.starts_with("--")
}

pub(crate) fn is_prefix_increment_statement(line: &str) -> bool {
    starts_prefix_increment(line) && line.trim_end().ends_with(';')
}

impl FormatEngine<'_> {
    fn has_continuable_previous_statement(&self) -> bool {
        self.output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| {
                let trimmed = line.trim_end();
                !matches!(
                    trimmed.trim_start(),
                    "break" | "continue" | "throw" | "goto" | "co_return" | "co_yield" | "co_await"
                ) && !has_hash_outside_literals(trimmed)
                    && !trimmed.ends_with(';')
                    && !trimmed.ends_with('{')
                    && !trimmed.ends_with('}')
            })
    }

    pub(crate) fn push_operator(&mut self, operator: &str, context: TokenPushContext<'_>) {
        let TokenPushContext {
            next,
            next_is_adjacent,
            following_operator,
            template_angle,
            token_index,
            ..
        } = context;
        let statement = self
            .current
            .rsplit([';', '{', '}'])
            .next()
            .unwrap_or(&self.current)
            .trim_start();
        if matches!(operator, "*" | "&" | "&&" | "^")
            && statement.starts_with("using ")
            && statement.contains('=')
            && self.layout.line_state.template_angle_depth == 0
        {
            self.emit_source_space();
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Operator;
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
                .scoped()
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
        self.set_leading_operator_continuation(operator, split_rvalue_reference);
        if self.try_push_operator_special_case(operator, next, template_angle) {
            return;
        }
        self.reset_stale_leading_operator_continuation(operator);

        // After a binary operator a `*` or `&` is unary, whatever spacing the
        // text around it suggests.
        if matches!(operator, "*" | "&")
            && operator_role == OperatorRole::UnaryOperator
            && self.follows_expression_operator(token_index)
        {
            self.push_unary_prefix(operator);
            return;
        }
        // `&&label` takes a label's address where an operand starts.
        if operator == "&&"
            && token_index < self.tree.tokens.len()
            && self
                .tree
                .previous_code_token(token_index)
                .is_some_and(|previous| match &self.tree.tokens[previous] {
                    Token::Symbol('{' | ',' | '(' | '[' | ':' | '?') => true,
                    Token::Operator(operator) if operator == "*" => self
                        .tree
                        .previous_code_token(previous)
                        .is_some_and(|keyword| matches!(&self.tree.tokens[keyword], Token::Word(word) if word == "goto")),
                    Token::Operator(operator) => matches!(operator.as_str(), "=" | "?" | ":"),
                    Token::Word(word) => word == "return",
                    _ => false,
                })
            && next_code_token(&self.tree.tokens, token_index + 1)
                .is_some_and(|next| matches!(self.tree.tokens[next], Token::Word(_)))
        {
            self.push_unary_prefix(operator);
            self.layout.command_state.observe_text(operator);
            self.previous_was_newline = false;
            return;
        }
        // After `)` a `&` before a name may take an address after a cast or
        // join two operands; its spacing stays as written.
        if operator == "&"
            && self.layout.previous == PreviousToken::CloseParen
            && token_index < self.tree.tokens.len()
            // At file scope astyle pads no paren before it.
            && (!self.options.pad_parens_outside
                || self.tree.groups.enclosing(token_index).is_some())
            && !self
                .tree
                .groups
                .enclosing(token_index)
                .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer))
            && self
                .tree
                .previous_code_token(token_index)
                .is_some_and(|previous| matches!(self.tree.tokens[previous], Token::Symbol(')')))
            && next_code_token(&self.tree.tokens, token_index + 1).is_some_and(|next| {
                match &self.tree.tokens[next] {
                    Token::Word(word) => word.starts_with(|ch: char| !ch.is_ascii_digit()),
                    Token::StringLiteral(_) => true,
                    Token::Operator(operator) => operator == "*",
                    _ => false,
                }
            })
        {
            // Padding parens outside puts a space after the `)` anyway.
            if self.options.pad_parens_outside {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return;
        }
        if operator == "*"
            && self.pointer_run.star_count == 2
            && next_is_adjacent
            && self.double_pointer_after_comma_in_function_body(token_index)
        {
            self.pointer_run.skip_adjacent_pointer_operators = 1;
            self.push_unary_prefix("**");
            return;
        }

        if operator == "*" && self.star_run_closes_type_argument(token_index) {
            self.push_pointer_run(operator, next, next_is_adjacent);
            return;
        }

        self.push_operator_by_kind(
            operator,
            next,
            next_is_adjacent,
            following_operator,
            operator_role,
            split_rvalue_reference,
        );
    }

    /// Whether the `*` at `token_index` is in a run of stars closing a type
    /// name before a `,` in parentheses, as in a macro's type argument.
    fn star_run_closes_type_argument(&self, token_index: usize) -> bool {
        let tokens = &self.tree.tokens;
        let is_star = |index: usize| matches!(&tokens[index], Token::Operator(op) if op == "*");
        if self.layout.nesting.paren_depth == 0
            || self.active_token_in_brackets()
            || token_index >= tokens.len()
        {
            return false;
        }
        let mut stars = 1;
        let mut after = next_code_token(tokens, token_index + 1);
        while let Some(index) = after.filter(|&index| is_star(index)) {
            stars += 1;
            after = next_code_token(tokens, index + 1);
        }
        let mut before = self.tree.previous_code_token(token_index);
        while let Some(index) = before.filter(|&index| is_star(index)) {
            stars += 1;
            before = self.tree.previous_code_token(index);
        }
        stars > 1
            && after.is_some_and(|index| matches!(tokens[index], Token::Symbol(',')))
            && before.is_some_and(|index| matches!(tokens[index], Token::Word(_)))
    }

    /// Whether the `**` at `token_index` follows a comma in a function body:
    /// astyle reads a `*` or `**` there as a dereference and leaves it alone,
    /// while it aligns a longer run and declarators outside functions.
    fn double_pointer_after_comma_in_function_body(&self, token_index: usize) -> bool {
        let groups = &self.tree.groups;
        self.tree
            .previous_code_token(token_index)
            .is_some_and(|previous| matches!(self.tree.tokens[previous], Token::Symbol(',')))
            && groups.enclosing(token_index).is_some_and(|group| {
                groups
                    .ancestors(group)
                    .any(|id| self.tree.blocks.kind(id) == Some(BlockKind::FunctionBody))
            })
    }

    /// Whether the code token before `token_index` is a binary operator, so
    /// what follows starts an operand.
    fn follows_expression_operator(&self, token_index: usize) -> bool {
        self.tree
            .previous_code_token(token_index)
            .is_some_and(|previous| match &self.tree.tokens[previous] {
                Token::Operator(operator) => matches!(
                    operator.as_str(),
                    "&&" | "||"
                        | "=="
                        | "!="
                        | "<="
                        | ">="
                        | "="
                        | "+="
                        | "-="
                        | "*="
                        | "/="
                        | "%="
                        | "&="
                        | "|="
                        | "^="
                        | "<<="
                        | ">>="
                        | "+"
                        | "-"
                        | "/"
                        | "%"
                        | "!"
                        | "?"
                        | "|"
                        | "^"
                        | "<<"
                        | ">>"
                ),
                _ => false,
            })
    }

    fn set_leading_operator_continuation(&mut self, operator: &str, split_rvalue_reference: bool) {
        if split_rvalue_reference {
            let indent_spaces = self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map_or(0, |line| leading_visual_width(line, self.options.tab_width));
            self.layout
                .continuation_indent
                .set_next_line_spaces(indent_spaces);
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
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
                .and_then(|line| self.layout.frame_stack.active_logical_on_output_line(line))
                .is_some_and(|frame| frame.operator != current_operator);
            let persisted_chain = if previous_opens_nested_logical_group {
                None
            } else {
                self.layout.continuation_indent.logical_chain_indent_spaces
            };
            let chain_spaces = persisted_chain
                .or_else(|| self.previous_logical_continuation_indent_spaces(operator))
                .or_else(|| {
                    self.layout
                        .command_state
                        .current_header
                        .is_none()
                        .then(|| {
                            self.output
                                .scoped()
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
                let spaces = if persisted_chain.is_some() && self.layout.nesting.paren_depth == 0 {
                    spaces
                } else {
                    match self.layout.nesting.current_continuation_indent_spaces() {
                        Some(paren) if paren < spaces => paren,
                        _ => spaces,
                    }
                };
                self.layout.continuation_indent.set_next_line_spaces(spaces);
                self.layout.continuation_indent.logical_chain_indent_spaces = Some(spaces);
            } else if self.layout.nesting.paren_depth == 0
                && let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces
            {
                self.layout.continuation_indent.logical_chain_indent_spaces = Some(spaces);
            }
        }
        if matches!(operator, "<<" | ">>")
            && self.current.trim().is_empty()
            && self.stream_line_follows_multiline_braced_operand()
            && let Some(stream) = self.layout.frame_stack.active_stream()
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(stream.chain_anchor_column);
        }
        if matches!(operator, "<<" | ">>")
            && self.current.trim().is_empty()
            && self.layout.continuation_indent.next_line_indent.is_none()
            && self
                .layout
                .continuation_indent
                .next_line_indent_spaces
                .is_none()
            && self.layout.indentation.statement_depth() == 0
            && !self.in_initializer_brace()
            && self.has_continuable_previous_statement()
            && self
                .output
                .len()
                .checked_sub(1)
                .is_none_or(|previous_line| {
                    self.layout
                        .frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_none()
                })
        {
            let spaces = self.continuation_base_indent() * self.options.indent_width
                + 2 * self.options.indent_width;
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        }
    }

    fn try_push_operator_special_case(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        template_angle: TemplateAngle,
    ) -> bool {
        if self.pointer_run.skip_adjacent_pointer_operators > 0
            && matches!(operator, "*" | "&" | "^")
        {
            self.pointer_run.skip_adjacent_pointer_operators -= 1;
            if resolved_pointer_align(self.options, operator) == PointerAlign::None
                && self.pointer_run.skip_adjacent_pointer_operators == 0
            {
                self.emit_trailing_source_space();
            }
            return true;
        }
        match template_angle {
            TemplateAngle::Open => {
                self.emit_source_space();
                self.current.push('<');
                self.layout.line_state.template_angle_depth += 1;
                self.emit_trailing_source_space();
                self.layout.command_state.observe_text(operator);
                self.layout.previous = PreviousToken::Operator;
                self.previous_was_newline = false;
                return true;
            }
            TemplateAngle::Close(count) => {
                if self.options.close_templates && self.current.trim_end().ends_with('>') {
                    self.trim_current_end();
                } else {
                    self.emit_source_space();
                }
                self.current.push_str(operator);
                self.layout.line_state.template_angle_depth = self
                    .layout
                    .line_state
                    .template_angle_depth
                    .saturating_sub(count);
                self.emit_trailing_source_space();
                self.layout.command_state.observe_text(operator);
                self.layout.previous = PreviousToken::Operator;
                self.previous_was_newline = false;
                if self.layout.line_state.template_angle_depth == 0 {
                    self.previous_was_template_close = true;
                }
                return true;
            }
            TemplateAngle::None => {}
        }
        if self.layout.line_state.operator_padding_disabled {
            let keeps_non_operator_padding = self.current.ends_with([' ', '\t'])
                && ((self.layout.previous == PreviousToken::OpenParen
                    && self.options.pad_parens_inside)
                    || (self.layout.previous == PreviousToken::Comma
                        && (self.options.pad_commas || self.options.pad_operators)));
            if !keeps_non_operator_padding {
                self.emit_source_space();
            }
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return true;
        }

        if operator == "<?"
            && matches!(next, Some(Token::Operator(next_operator)) if next_operator == ">")
        {
            self.trim_current_end();
            self.current.push_str(operator);
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return true;
        }
        if operator == ">" && self.current.trim_end().ends_with('?') {
            self.current.push('>');
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return true;
        }
        if self.is_in_asm_operator_context() {
            if matches!(operator, "*" | "&" | "^") {
                self.emit_source_space();
            } else {
                self.trim_current_end();
            }
            self.current.push_str(operator);
            self.layout.command_state.observe_text(operator);
            self.layout.previous = PreviousToken::Operator;
            self.previous_was_newline = false;
            return true;
        }
        false
    }

    fn reset_stale_leading_operator_continuation(&mut self, operator: &str) {
        if self.current.trim().is_empty()
            && is_leading_continuation_operator(operator)
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line[..trailing_comment_split_limit(line)].trim() == "}")
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces =
                Some(self.layout.indentation.indent() * self.options.indent_width);
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
        } else if self.current.trim().is_empty()
            && is_leading_continuation_operator(operator)
            && self
                .layout
                .continuation_indent
                .next_line_indent_spaces
                .is_none()
            && !self.preprocessor.last_output_was_preprocessor
            && self.has_continuable_previous_statement()
        {
            let stale_level = self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(|line| {
                    leading_visual_width(line, self.options.tab_width) / self.options.indent_width
                        + 1
                })
                .unwrap_or_else(|| self.layout.indentation.indent() + 1);
            if self
                .layout
                .continuation_indent
                .next_line_indent
                .is_some_and(|level| level > stale_level)
            {
                self.layout.continuation_indent.next_line_indent = Some(stale_level);
            } else if self.layout.continuation_indent.next_line_indent.is_none() {
                self.layout.continuation_indent.next_line_indent_spaces = Some(
                    self.continuation_base_indent() * self.options.indent_width
                        + self.options.continuation_indent * self.options.indent_width,
                );
            }
        }
    }

    fn push_operator_by_kind(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        next_is_adjacent: bool,
        following_operator: Option<&str>,
        operator_role: OperatorRole,
        split_rvalue_reference: bool,
    ) {
        match operator {
            "::" => {
                if self.layout.previous == PreviousToken::OpenParen
                    && self.options.pad_parens_inside
                {
                    self.pad_inside_paren_space();
                } else if self.layout.previous == PreviousToken::OpenParen
                    && self.options.unpad_parens
                {
                    self.trim_current_end_horizontal_space();
                } else if !(self.layout.previous == PreviousToken::Comma
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
                self.layout.objc.post_prefix = true;
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
                    && self.layout.indentation.statement_depth() > 0
                    && self.options.pad_operators
                    && self.line_start_sign_is_unary(next) =>
            {
                self.push_unary_prefix(operator);
            }
            "+" | "-"
                if self.current.trim().is_empty()
                    && self.layout.indentation.statement_depth() > 0
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
            // A `*` closing a type name before a `,` in parentheses, as in a
            // macro's type argument, is a pointer.
            "*" if matches!(next, Some(Token::Symbol(',')))
                && self.layout.previous == PreviousToken::Word
                && self.layout.nesting.paren_depth > 0
                && !self.active_token_in_brackets() =>
            {
                self.push_pointer_run(operator, next, next_is_adjacent);
            }
            // Between brackets a `*` after a name or value multiplies; after
            // a `)` it may follow a cast.
            "*" if matches!(
                self.layout.previous,
                PreviousToken::Word | PreviousToken::Literal | PreviousToken::CloseBracket
            ) && self.active_token_in_brackets() =>
            {
                self.push_binary_operator(operator);
            }
            // A `*` or `&` run after `return` dereferences or takes an
            // address.
            "*" | "&"
                if trailing_word(self.current.trim_end().trim_end_matches(['*', '&']))
                    == "return" =>
            {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            // A `*` after `sizeof`, or attached to its operand after a
            // parenthesized name within parentheses, dereferences.
            "*" if next_is_adjacent
                && (trailing_word(&self.current) == "sizeof"
                    || self.layout.nesting.paren_depth > 0
                        && ends_parenthesized_name(&self.current))
                && matches!(next, Some(Token::Word(_) | Token::Symbol('(')))
                    | matches!(next, Some(Token::Operator(next)) if next == "*") =>
            {
                self.emit_source_space();
                self.current.push_str(operator);
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
                // Inside parentheses astyle keeps the spacing of the source,
                // as in brackets before any assignment.
                if self.layout.nesting.paren_depth > 0
                    || self.active_token_in_brackets()
                        && find_assignment_operator(&self.current).is_none()
                {
                    self.emit_source_space();
                    self.current.push_str(operator);
                    self.emit_trailing_source_space();
                } else {
                    self.push_binary_operator(operator);
                }
            }
            "*" if operator_role != OperatorRole::PointerDeclarator
                && self.layout.nesting.paren_depth > 0
                && matches!(next, Some(Token::Word(_)))
                && (self.current_paren_is_expression_context()
                    && !self.current_paren_context_is_declaration()
                    || self.layout.previous == PreviousToken::Word
                        && self.continues_expression_parens())
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
                && self.tree_declaration_context() != Some(true)
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
                && (self.layout.previous == PreviousToken::Comma
                    && (self.current_paren_is_expression_context()
                        || self.tree_declaration_context().is_none())
                    // After a binary operator spaced off its left operand.
                    || self.layout.previous == PreviousToken::Operator
                        && self.current.ends_with(' ')
                        && self.current.trim_end().ends_with(['&', '*'])
                    // No declarator puts a `*` right after a `&`.
                    || self.layout.previous == PreviousToken::Operator
                        && self.current.ends_with('&')
                        && !self.current.ends_with("&&")
                    || !self.is_pointer_like(
                        operator,
                        next,
                        next_is_adjacent,
                        following_operator,
                    )
                    || (self.current.trim_end().ends_with('*')
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
                && self.layout.previous == PreviousToken::Word
                && trailing_word(&self.current) == "auto" =>
            {
                if resolved_pointer_align(self.options, operator) == PointerAlign::None {
                    self.push_unary_prefix(operator);
                } else {
                    self.push_pointer_or_reference(operator, next, next_is_adjacent);
                }
            }
            // A reference to a parenthesized declarator keeps its spacing.
            "&" if operator_role == OperatorRole::PointerDeclarator
                && matches!(next, Some(Token::Symbol('(')))
                && self.layout.previous == PreviousToken::Word =>
            {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && self.current.trim_start().starts_with("return ")
                && self.layout.previous != PreviousToken::OpenParen
                && !is_pointer_type_word(trailing_word(&self.current)) =>
            {
                self.push_binary_operator(operator);
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && self.layout.previous == PreviousToken::OpenParen =>
            {
                self.push_unary_prefix(operator);
            }
            "&" if operator_role == OperatorRole::PointerDeclarator
                && !self.current_ends_cast()
                && !self.current_ends_pointer_cast()
                && (self.layout.nesting.paren_depth == 0
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
                    // An argument of a call in a statement block.
                    && !(self.layout.previous == PreviousToken::Comma
                        && self.layout.nesting.brace_type_stack.last()
                            == Some(&BraceType::Command))
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
                    self.layout.previous,
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
            "&" if self.layout.nesting.paren_depth > 0
                && matches!(
                    self.layout.previous,
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
            "&" if self.layout.nesting.paren_depth > 0
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
                && self.layout.nesting.paren_depth == 0
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    Some(BraceType::Array | BraceType::Initializer | BraceType::DeferArray)
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
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
            "&" | "*"
                if self.current_ends_cast()
                    && self.options.pad_operators
                    && self.layout.nesting.paren_depth > 0 =>
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
            "&&" if !self
                .current
                .active_token()
                .is_some_and(|index| self.operand_member_is_accessed(index))
                && (split_rvalue_reference
                    || (!self.current_paren_is_expression_context()
                        || trailing_word(&self.current) == language::AUTO
                        || self.current.trim_end().ends_with('*')
                        || self.current_in_cast_type_group()
                        || self.current_in_parenthesized_type_operand()
                        || self.current_paren_context_is_declaration()
                        || self.is_function_declaration_parameter_continuation())
                        && self.is_rvalue_reference_like(next)) =>
            {
                self.push_pointer_or_reference(operator, next, next_is_adjacent);
            }
            "&" | "*" | "^"
                if self.current.trim().is_empty()
                    && self.token_input.token_begins_source_line
                    && self
                        .layout
                        .continuation_indent
                        .next_line_indent_spaces
                        .is_some()
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
            // No pointer precedes a number.
            "&" | "*" | "^"
                if !matches!(next, Some(Token::Number(_)))
                    && self.is_pointer_like(
                        operator,
                        next,
                        next_is_adjacent,
                        following_operator,
                    ) =>
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
            let rhs_next = if self.comments.next_comment_ends_line {
                None
            } else {
                next
            };
            self.register_current_continuation_indent(rhs_next);
        }
        self.layout.command_state.observe_text(operator);
        self.layout.previous = PreviousToken::Operator;
        self.previous_was_newline = false;
    }

    fn should_attach_sizeof_after_standalone_call_argument(&self, next: Option<&Token>) -> bool {
        if !matches!(next, Some(Token::Word(word)) if word == "sizeof")
            || !self.current.trim_end().ends_with(')')
            || !self.current.trim_start().starts_with('(')
        {
            return false;
        }
        for line in self.output.scoped().iter().rev().take(8) {
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
                && !is_header(self.options, before)
                && matches!(function_name_start(before), Some(0))
            {
                return true;
            }
        }
        false
    }

    fn is_trailing_return_arrow(&self) -> bool {
        let current = self.current.trim_end();
        current.starts_with("auto ") && current.ends_with(')')
    }

    fn is_prefix_increment_or_decrement(&self) -> bool {
        !matches!(
            self.layout.previous,
            PreviousToken::Word
                | PreviousToken::Literal
                | PreviousToken::CloseParen
                | PreviousToken::CloseBracket
        ) || trailing_word(&self.current) == "return"
    }

    /// Whether the token being pushed stands between brackets, where only
    /// expressions go.
    pub(crate) fn active_token_in_brackets(&self) -> bool {
        self.current.active_token().is_some_and(|index| {
            self.tree
                .groups
                .enclosing(index)
                .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Bracket)
        })
    }

    /// Whether the operand after the operator at `index` is a name whose
    /// member is accessed, which no declarator is.
    fn operand_member_is_accessed(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        index < tokens.len()
            && next_code_token(tokens, index + 1)
                .filter(|&name| matches!(tokens[name], Token::Word(_)))
                .and_then(|name| next_code_token(tokens, name + 1))
                .is_some_and(|after| match &tokens[after] {
                    Token::Operator(operator) => operator == "->",
                    Token::Symbol('.') => true,
                    _ => false,
                })
    }

    fn is_cast_unary_sign(&self, next: Option<&Token>) -> bool {
        matches!(next, Some(Token::Number(_)))
            && (self.current_ends_numeric_cast() || self.current_ends_builtin_pointer_cast())
    }

    /// Whether the current line ends with a cast to a pointer to a builtin
    /// type, as `(void *)`.
    fn current_ends_builtin_pointer_cast(&self) -> bool {
        let current = self.current.trim_end();
        self.current_ends_pointer_cast()
            && crate::formatter::engine::matching_open_paren_offset(current).is_some_and(|open| {
                current[open + 1..]
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .find(|word| !word.is_empty() && *word != "const")
                    .is_some_and(|word| {
                        matches!(
                            word,
                            "void"
                                | "char"
                                | "short"
                                | "int"
                                | "long"
                                | "float"
                                | "double"
                                | "signed"
                                | "unsigned"
                        )
                    })
            })
    }

    fn is_sizeof_typedef_unary_sign(&self, next: Option<&Token>) -> bool {
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

    fn line_start_sign_is_unary(&self, next: Option<&Token>) -> bool {
        if !matches!(
            next,
            Some(Token::Word(_) | Token::Number(_) | Token::Symbol('('))
        ) {
            return false;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.ends_with(['(', '[', '{', ',', '=', '?', ':'])
                    || head_ends_binary_operator(code)
                    || code.trim_start().starts_with("return ")
            })
    }

    fn is_unary_sign(&self) -> bool {
        let previous = match self.layout.previous {
            PreviousToken::Other => self
                .layout
                .previous_before_comment
                .unwrap_or(PreviousToken::Other),
            previous => previous,
        };
        matches!(
            previous,
            PreviousToken::None
                | PreviousToken::Operator
                | PreviousToken::OpenParen
                | PreviousToken::OpenBracket
                | PreviousToken::Comma
        ) || self.current.trim_end().ends_with([':', '{'])
            || matches!(trailing_word(&self.current), "return" | "case")
            // A sign leading the line after an opening brace starts an
            // element or a statement.
            || self.current.trim().is_empty()
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| {
                        line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .ends_with('{')
                    })
    }

    fn current_ends_prefix_increment_or_decrement(&self) -> bool {
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

    fn current_ends_postfix_increment_or_decrement(&self) -> bool {
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

    fn push_unary_prefix(&mut self, operator: &str) {
        let word = trailing_word(&self.current);
        let after_return = word == "return";
        let after_return_or_case = after_return || matches!(word, "case" | "do");
        if after_return_or_case {
            if self.options.pad_operators && after_return && matches!(operator, "+" | "-") {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.pads_after_close_paren(operator) {
            self.emit_source_space_or_ensure();
        } else if self.layout.previous == PreviousToken::Comma {
            if self.options.pad_commas || self.options.pad_operators {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.layout.previous == PreviousToken::Word
            || self.layout.previous == PreviousToken::CloseParen
        {
            self.emit_source_space();
        }
        self.current.push_str(operator);
        self.emit_trailing_source_space();
        self.layout.previous = PreviousToken::Operator;
    }

    fn stream_line_follows_multiline_braced_operand(&self) -> bool {
        if !self.current.trim().is_empty() {
            return false;
        }
        let Some(previous_line) = self.output.len().checked_sub(1) else {
            return false;
        };
        self.layout
            .frame_stack
            .last_closed_brace()
            .is_some_and(|brace| {
                brace.close_output_line == Some(previous_line)
                    && brace.close_ends_output_line
                    && self
                        .layout
                        .frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_none()
            })
    }

    fn record_logical_operator_frame(&mut self, operator: &str) {
        if self.current.trim() == ")"
            && let Some(spaces) = self
                .layout
                .frame_stack
                .take_line_closed_call_logical_operand_indent(self.output.len())
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.layout.continuation_indent.logical_chain_indent_spaces = Some(spaces);
        }
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
        self.layout.frame_stack.push_logical(LogicalFrame {
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
            .layout
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
        self.layout.frame_stack.push_stream(StreamFrame {
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
            && self.layout.frame_stack.active_stream().is_some()
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
            if self.pads_after_close_paren(operator) {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
            self.record_stream_operator_frame(operator);
            self.current.push_str(operator);
            self.emit_trailing_source_space();
        }
    }

    /// Padding parens outside spaces a `)` from any operator but those
    /// starting with `+`, `-` or `.`, within a block.
    fn pads_after_close_paren(&self, operator: &str) -> bool {
        // At file scope astyle pads no paren before an operator outside
        // parens.
        self.options.pad_parens_outside
            && (!self.layout.nesting.brace_type_stack.is_empty()
                || self.layout.nesting.paren_depth > 0)
            && self.layout.previous == PreviousToken::CloseParen
            && !operator.starts_with(['+', '-', '.'])
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
