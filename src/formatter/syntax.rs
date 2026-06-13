use super::language::{
    self, is_macro_like_word, is_non_type_keyword, is_pointer_type_word, is_type_like_pointer_word,
};
use super::state::TemplateAngle;
use super::token::{
    Token, matching_close_paren_index, next_non_layout_token_index, next_non_whitespace,
};
use crate::source::lex::{
    is_identifier_continue, is_identifier_start, is_word_char, trailing_word,
};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum OperatorRole {
    Unknown,
    PointerDeclarator,
    BinaryOperator,
    UnaryOperator,
}

pub(super) fn signature_ends_with_parameter_list(line: &str) -> bool {
    let mut rest = line.trim_end();
    loop {
        if rest.ends_with(')') {
            return true;
        }
        if let Some(stripped) = rest.strip_suffix("&&").or_else(|| rest.strip_suffix('&')) {
            rest = stripped.trim_end();
            continue;
        }
        let word = trailing_word(rest);
        if matches!(
            word,
            "const" | "volatile" | "noexcept" | "override" | "final" | "mutable" | "try"
        ) {
            rest = rest[..rest.len() - word.len()].trim_end();
            continue;
        }
        return false;
    }
}

pub(super) fn function_name_start(before_open_paren: &str) -> Option<usize> {
    let end = before_open_paren.trim_end().len();
    let head = &before_open_paren[..end];
    let bytes = head.as_bytes();
    let identifier_segment_start = |limit: usize| {
        head[..limit]
            .char_indices()
            .rev()
            .take_while(|(_, ch)| is_identifier_continue(*ch))
            .last()
            .map(|(index, _)| index)
    };
    let mut start = match operator_function_name_start(head) {
        Some(op_start) => op_start,
        None => identifier_segment_start(end)?,
    };
    loop {
        if start >= 1 && bytes[start - 1] == b'~' {
            start -= 1;
        }
        if start >= 2 && bytes[start - 1] == b':' && bytes[start - 2] == b':' {
            match identifier_segment_start(start - 2) {
                Some(index) => {
                    start = index;
                    continue;
                }
                None => start -= 2,
            }
        }
        break;
    }
    (start < end).then_some(start)
}

pub(super) fn function_head_has_assignment(before: &str) -> bool {
    let limit = operator_function_name_start(before).unwrap_or(before.len());
    before[..limit].contains('=')
}

fn operator_function_name_start(before_open_paren: &str) -> Option<usize> {
    let start = before_open_paren.rfind(language::OPERATOR)?;
    if start > 0
        && before_open_paren[..start]
            .chars()
            .last()
            .is_some_and(is_identifier_continue)
    {
        return None;
    }
    let after = before_open_paren[start + language::OPERATOR.len()..].trim_start();
    (!after.is_empty()
        && (!after.chars().next().is_some_and(is_identifier_start)
            || first_operator_word(after).is_some_and(is_named_operator_word)))
    .then_some(start)
}

pub(super) fn first_operator_word(after_operator: &str) -> Option<&str> {
    after_operator
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|word| !word.is_empty())
}

pub(super) fn is_named_operator_word(word: &str) -> bool {
    matches!(
        word,
        "new"
            | "delete"
            | "co_await"
            | "and"
            | "or"
            | "xor"
            | "not"
            | "bitand"
            | "bitor"
            | "compl"
            | "and_eq"
            | "or_eq"
            | "xor_eq"
            | "not_eq"
    )
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum SyntaxRole {
    Unknown,
    FunctionDeclarator,
    StandaloneMacroInvocation,
    Operator(OperatorRole),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct SyntaxRoles {
    token_roles: Vec<SyntaxRole>,
    inside_parenthesized_expression: Vec<bool>,
}

pub(super) fn template_angle_role(
    tokens: &[Token],
    index: usize,
    end: usize,
    template_depth: usize,
) -> TemplateAngle {
    match tokens.get(index) {
        Some(Token::Operator(operator)) if operator == "<" => {
            if template_depth > 0 || looks_like_template_opener(tokens, index, end) {
                TemplateAngle::Open
            } else {
                TemplateAngle::None
            }
        }
        Some(Token::Operator(operator)) if operator == ">" && template_depth > 0 => {
            TemplateAngle::Close(1)
        }
        Some(Token::Operator(operator)) if operator == ">>" && template_depth > 0 => {
            TemplateAngle::Close(template_depth.min(2))
        }
        _ => TemplateAngle::None,
    }
}

fn looks_like_template_opener(tokens: &[Token], index: usize, end: usize) -> bool {
    if !matches!(tokens.get(index), Some(Token::Operator(operator)) if operator == "<") {
        return false;
    }
    let first_after_open =
        next_non_whitespace(tokens, index + 1, end).and_then(|next| tokens.get(next));
    if first_after_open.is_none()
        || matches!(first_after_open, Some(Token::Operator(operator)) if operator == "=")
    {
        return false;
    }
    let mut depth = 0usize;
    let mut paren_depth = 0usize;
    for (cursor, token) in tokens.iter().enumerate().take(end).skip(index) {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => {}
            Token::Word(_) | Token::Number(_) => {}
            Token::Operator(operator) if operator == "<" => depth += 1,
            Token::Operator(operator) if operator == ">" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return paren_depth == 0
                        && next_non_whitespace(tokens, cursor + 1, end)
                            .and_then(|next| tokens.get(next))
                            .is_none_or(|token| !matches!(token, Token::Number(_)));
                }
            }
            Token::Operator(operator) if operator == ">>" => {
                depth = depth.saturating_sub(2);
                if depth == 0 {
                    return paren_depth == 0
                        && next_non_whitespace(tokens, cursor + 1, end)
                            .and_then(|next| tokens.get(next))
                            .is_none_or(|token| !matches!(token, Token::Number(_)));
                }
            }
            Token::Operator(operator)
                if matches!(
                    operator.as_str(),
                    "::" | "*" | "&" | "&&" | "^" | "=" | "!" | "!="
                ) => {}
            Token::Symbol('(') => paren_depth += 1,
            Token::Symbol(')') => {
                if paren_depth == 0 {
                    return false;
                }
                paren_depth -= 1;
            }
            Token::Symbol(',' | ':' | '[' | ']') => {}
            _ => return false,
        }
    }
    false
}

pub(super) fn scoped_name_is_constructor(name: &str) -> bool {
    let mut parts = name
        .rsplit("::")
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let Some(last) = parts.next() else {
        return false;
    };
    let Some(parent) = parts.next() else {
        return false;
    };
    let last = last.trim_start_matches('~');
    last == parent
}

pub(super) fn assignment_declarator_offset(line: &str) -> Option<usize> {
    if !line.contains('=') {
        return None;
    }
    let bytes = line.as_bytes();
    let mut depth = 0usize;
    let mut eq = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'=' if depth == 0 => {
                let prev = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                if !matches!(
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
                ) && next != b'='
                {
                    eq = Some(i);
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let eq = eq?;
    let head = line[..eq].trim_end();
    if head.contains(['(', ')', ',', '{', '}']) {
        return None;
    }
    let head_bytes = head.as_bytes();
    let mut word_starts: Vec<usize> = Vec::new();
    let mut in_word = false;
    for (offset, byte) in head_bytes.iter().enumerate() {
        let is_space = matches!(byte, b' ' | b'\t');
        if !is_space && !in_word {
            word_starts.push(offset);
        }
        in_word = !is_space;
    }
    if word_starts.len() < 2 {
        return None;
    }
    let first_end = head[word_starts[0]..]
        .find([' ', '\t'])
        .map_or(head.len(), |p| word_starts[0] + p);
    let first = &head[word_starts[0]..first_end];
    if matches!(
        first,
        "return" | "case" | "goto" | "if" | "while" | "for" | "switch" | "else" | "do" | "sizeof"
    ) {
        return None;
    }
    let mut declarator = *word_starts.last()?;
    while declarator < eq && matches!(head_bytes[declarator], b'*' | b'&') {
        declarator += 1;
    }
    if declarator >= head.len() || !is_word_char(head[declarator..].chars().next()?) {
        return None;
    }
    Some(declarator)
}

pub(super) fn access_modified_brace_indices(tokens: &[Token]) -> HashSet<usize> {
    let mut indices = HashSet::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    let mut modifier_count = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Symbol('{') => stack.push((index, modifier_count)),
            Token::Symbol('}') => {
                if let Some((open_index, count_at_open)) = stack.pop()
                    && modifier_count > count_at_open
                {
                    indices.insert(open_index);
                }
            }
            Token::Word(word) if matches!(word.as_str(), "public" | "private" | "protected") => {
                modifier_count += 1;
            }
            _ => {}
        }
    }
    indices
}

pub(super) fn nested_brace_array_indices(tokens: &[Token]) -> HashSet<usize> {
    let mut indices = HashSet::new();
    let mut stack: Vec<usize> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Symbol('{') => {
                if let Some(&parent) = stack.last() {
                    indices.insert(parent);
                }
                stack.push(index);
            }
            Token::Symbol('}') => {
                stack.pop();
            }
            _ => {}
        }
    }
    indices
}

pub(super) fn classify_syntax(tokens: &[Token]) -> SyntaxRoles {
    let mut roles = SyntaxRoles::new(tokens.len());
    classify_paren_ranges(tokens, &mut roles);
    classify_word_roles(tokens, &mut roles);
    for (index, token) in tokens.iter().enumerate() {
        let role = match token {
            Token::Operator(operator) if operator == "*" => {
                classify_star_operator(tokens, index, &roles)
            }
            Token::Operator(operator) if operator == "&" => {
                classify_ampersand_operator(tokens, index)
            }
            _ => OperatorRole::Unknown,
        };
        if role != OperatorRole::Unknown {
            roles.set_role(index, SyntaxRole::Operator(role));
        }
    }
    roles
}

fn classify_paren_ranges(tokens: &[Token], roles: &mut SyntaxRoles) {
    let mut stack = Vec::new();
    let mut depth_changes = vec![0isize; tokens.len()];
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Symbol('(') => stack.push(index),
            Token::Symbol(')') => {
                let Some(open) = stack.pop() else {
                    continue;
                };
                if paren_range_is_expression(tokens, open, index) && open + 1 < index {
                    depth_changes[open + 1] += 1;
                    depth_changes[index] -= 1;
                }
            }
            _ => {}
        }
    }
    let mut depth = 0isize;
    for (index, change) in depth_changes.into_iter().enumerate() {
        depth += change;
        roles.inside_parenthesized_expression[index] = depth > 0;
    }
}

fn classify_word_roles(tokens: &[Token], roles: &mut SyntaxRoles) {
    for (index, token) in tokens.iter().enumerate() {
        let Token::Word(word) = token else {
            continue;
        };
        if is_non_type_keyword(word) {
            continue;
        }
        let Some(open) = next_non_layout_token_index(tokens, index + 1) else {
            continue;
        };
        if !matches!(tokens.get(open), Some(Token::Symbol('('))) {
            continue;
        }
        let Some(close) = matching_close_paren_index(tokens, open) else {
            continue;
        };
        let previous = previous_token_skipping_layout(tokens, index);
        let after = next_non_layout_token_index(tokens, close + 1);
        if function_declarator_word(tokens, previous, after) {
            roles.set_role(index, SyntaxRole::FunctionDeclarator);
        } else if standalone_macro_invocation_word(tokens, word, close, after) {
            roles.set_role(index, SyntaxRole::StandaloneMacroInvocation);
        }
    }
}

fn function_declarator_word(
    tokens: &[Token],
    previous: Option<usize>,
    after: Option<usize>,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let has_return_type = match tokens.get(previous) {
        Some(token @ Token::Word(_)) => syntax_token_is_type_word(token),
        Some(Token::Operator(operator)) if matches!(operator.as_str(), "*" | "&") => {
            operator_preceded_by_return_type(tokens, previous)
        }
        Some(Token::Symbol(')')) => true,
        _ => false,
    };
    has_return_type
        && after
            .and_then(|index| tokens.get(index))
            .is_some_and(|token| match token {
                Token::Symbol(';' | '{') => true,
                Token::Operator(operator) => operator == "=",
                _ => false,
            })
}

fn operator_preceded_by_return_type(tokens: &[Token], operator: usize) -> bool {
    let mut cursor = operator;
    while let Some(previous) = previous_token_skipping_layout(tokens, cursor) {
        match tokens.get(previous) {
            Some(Token::Operator(operator)) if matches!(operator.as_str(), "*" | "&" | "::") => {
                cursor = previous;
            }
            Some(Token::Symbol(')')) => return true,
            Some(token) => return syntax_token_is_type_word(token),
            None => return false,
        }
    }
    false
}

fn standalone_macro_invocation_word(
    tokens: &[Token],
    word: &str,
    close: usize,
    after: Option<usize>,
) -> bool {
    is_macro_like_word(word)
        && word.contains('_')
        && (after
            .and_then(|index| tokens.get(index))
            .is_some_and(|token| matches!(token, Token::Symbol(';')))
            || line_ends_after_token(tokens, close))
}

fn line_ends_after_token(tokens: &[Token], index: usize) -> bool {
    let mut cursor = index + 1;
    while matches!(tokens.get(cursor), Some(Token::Whitespace(_))) {
        cursor += 1;
    }
    matches!(tokens.get(cursor), None | Some(Token::Newline))
}

fn classify_star_operator(tokens: &[Token], index: usize, roles: &SyntaxRoles) -> OperatorRole {
    let previous = previous_token_skipping_layout(tokens, index);
    let next = next_non_layout_token_index(tokens, index + 1);
    if star_is_binary_operator(tokens, previous, next, index, roles) {
        OperatorRole::BinaryOperator
    } else if star_is_pointer_declarator(tokens, previous, next) {
        OperatorRole::PointerDeclarator
    } else if operator_is_unary_prefix(tokens, previous, next) {
        OperatorRole::UnaryOperator
    } else {
        OperatorRole::Unknown
    }
}

fn classify_ampersand_operator(tokens: &[Token], index: usize) -> OperatorRole {
    let previous = previous_token_skipping_layout(tokens, index);
    let next = next_non_layout_token_index(tokens, index + 1);
    if previous
        .and_then(|index| tokens.get(index))
        .is_some_and(|token| matches!(token, Token::Operator(operator) if matches!(operator.as_str(), "*" | "&" | "^")))
    {
        return OperatorRole::Unknown;
    }
    if suffix_type_word_in_expression(tokens, previous, next, index) {
        return OperatorRole::BinaryOperator;
    }
    if star_is_pointer_declarator(tokens, previous, next) {
        OperatorRole::PointerDeclarator
    } else if ampersand_is_binary_operator(tokens, previous, next) {
        OperatorRole::BinaryOperator
    } else if operator_is_unary_prefix(tokens, previous, next) {
        OperatorRole::UnaryOperator
    } else {
        OperatorRole::Unknown
    }
}

fn star_is_pointer_declarator(
    tokens: &[Token],
    previous: Option<usize>,
    next: Option<usize>,
) -> bool {
    let Some(mut next) = next else {
        return false;
    };
    let followed_by_attribute = if matches!(tokens.get(next), Some(Token::Symbol('['))) {
        let Some(after_attribute) = token_after_attribute(tokens, next) else {
            return false;
        };
        next = after_attribute;
        true
    } else {
        false
    };
    if followed_by_attribute
        && matches!(tokens.get(next), Some(Token::Word(_)))
        && previous
            .and_then(|index| tokens.get(index))
            .is_some_and(|token| {
                matches!(token, Token::Word(_)) || syntax_token_is_type_word(token)
            })
    {
        return true;
    }
    let is_nested_function_pointer = matches!(tokens.get(next), Some(Token::Operator(operator)) if matches!(operator.as_str(), "*" | "&" | "&&"))
        && previous
            .filter(|open| matches!(tokens.get(*open), Some(Token::Symbol('('))))
            .and_then(|open| matching_close_paren_index(tokens, open))
            .and_then(|close| next_non_layout_token_index(tokens, close + 1))
            .is_some_and(|after| matches!(tokens.get(after), Some(Token::Symbol('(' | '['))));
    if is_nested_function_pointer {
        return true;
    }
    let is_trailing_return_pointer = matches!(tokens.get(next), Some(Token::Symbol(';' | '{')))
        && preceding_statement_has_trailing_return_arrow(tokens, previous);
    if !is_trailing_return_pointer
        && !matches!(tokens.get(next), Some(Token::Word(_) | Token::Symbol(')')))
    {
        return false;
    }
    if matches!(tokens.get(next), Some(Token::Word(word)) if is_non_type_keyword(word)) {
        return false;
    }
    if is_trailing_return_pointer {
        return true;
    }
    if previous
        .and_then(|index| previous_token_skipping_layout(tokens, index))
        .and_then(|index| tokens.get(index))
        .is_some_and(|token| {
            matches!(token, Token::Operator(operator) if operator == "->")
                || matches!(token, Token::Symbol('.'))
        })
    {
        return false;
    }
    if matches!(tokens.get(next), Some(Token::Symbol(')')))
        && !previous
            .and_then(|index| tokens.get(index))
            .is_some_and(syntax_token_is_type_word)
    {
        return false;
    }
    match previous.and_then(|index| tokens.get(index)) {
        Some(token) if syntax_token_is_type_word(token) => true,
        Some(Token::Symbol('(')) => previous
            .and_then(|open_index| previous_token_skipping_layout(tokens, open_index))
            .and_then(|before_open| tokens.get(before_open))
            .is_some_and(syntax_token_is_type_word),
        _ => false,
    }
}

fn token_after_attribute(tokens: &[Token], first_open: usize) -> Option<usize> {
    let second_open = next_non_layout_token_index(tokens, first_open + 1)?;
    if !matches!(tokens.get(second_open), Some(Token::Symbol('['))) {
        return None;
    }
    let mut depth = 2usize;
    let mut cursor = second_open + 1;
    while cursor < tokens.len() {
        match tokens.get(cursor) {
            Some(Token::Symbol('[')) => depth += 1,
            Some(Token::Symbol(']')) => {
                depth -= 1;
                if depth == 0 {
                    return next_non_layout_token_index(tokens, cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn preceding_statement_has_trailing_return_arrow(tokens: &[Token], before: Option<usize>) -> bool {
    let Some(before) = before else {
        return false;
    };
    for token in tokens[..=before].iter().rev() {
        match token {
            Token::Operator(operator) if operator == "->" => return true,
            Token::Symbol(';' | '{' | '}') => return false,
            _ => {}
        }
    }
    false
}

fn star_is_binary_operator(
    tokens: &[Token],
    previous: Option<usize>,
    next: Option<usize>,
    index: usize,
    roles: &SyntaxRoles,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let Some(next) = next else {
        return false;
    };
    if matches!(tokens.get(next), Some(Token::Symbol('[')))
        && token_after_attribute(tokens, next).is_some_and(|after_attribute| {
            matches!(tokens.get(after_attribute), Some(Token::Word(_)))
        })
    {
        return false;
    }
    if matches!(tokens.get(previous), Some(Token::Word(word)) if is_macro_like_word(word))
        && matches!(tokens.get(next), Some(Token::Word(word)) if is_macro_like_word(word))
    {
        return true;
    }
    let suffix_type_word_in_expression =
        suffix_type_word_in_expression(tokens, Some(previous), Some(next), index);
    if (syntax_token_is_type_word(&tokens[previous])
        && !following_token_is_call_open(tokens, next)
        && !suffix_type_word_in_expression)
        || !syntax_token_can_end_expression(&tokens[previous])
        || !syntax_token_can_start_expression(&tokens[next])
    {
        return false;
    }
    if roles.token_inside_parenthesized_expression(index)
        && !following_token_is_assignment(tokens, next)
    {
        return true;
    }
    if suffix_type_word_in_expression && matches!(tokens.get(next), Some(Token::Word(_))) {
        return true;
    }
    if roles.role_at(next) == SyntaxRole::FunctionDeclarator {
        return false;
    }
    if matches!(tokens.get(next), Some(Token::Word(_)))
        && following_token_is_call_open(tokens, next)
    {
        return true;
    }
    if matches!(tokens.get(next), Some(Token::Word(_)))
        && following_token_is_non_assignment_operator(tokens, next)
    {
        return true;
    }
    !matches!(
        tokens.get(previous),
        Some(Token::Word(_) | Token::Symbol(')'))
    ) || !matches!(tokens.get(next), Some(Token::Word(_)))
}

fn suffix_type_word_in_expression(
    tokens: &[Token],
    previous: Option<usize>,
    next: Option<usize>,
    index: usize,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let Some(next) = next else {
        return false;
    };
    matches!(
        tokens.get(previous),
        Some(Token::Word(word)) if word.ends_with("_t") && !is_pointer_type_word(word)
    ) && syntax_token_can_start_expression(&tokens[next])
        && !following_token_is_call_open(tokens, next)
        && token_follows_expression_intro(tokens, index)
}

fn token_follows_expression_intro(tokens: &[Token], index: usize) -> bool {
    for token in tokens[..index].iter().rev() {
        match token {
            Token::Whitespace(_) => {}
            Token::Newline | Token::Symbol(';' | '{' | '}') => return false,
            Token::Operator(operator)
                if language::ASSIGNMENT_OPERATORS.contains(&operator.as_str()) =>
            {
                return true;
            }
            Token::Word(word) if matches!(word.as_str(), "return" | "case" | "throw") => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn ampersand_is_binary_operator(
    tokens: &[Token],
    previous: Option<usize>,
    next: Option<usize>,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let Some(next) = next else {
        return false;
    };
    if !syntax_token_can_end_expression(&tokens[previous])
        || !syntax_token_can_start_expression(&tokens[next])
    {
        return false;
    }
    if matches!(
        (tokens.get(previous), tokens.get(next)),
        (Some(Token::Word(_)), Some(Token::Word(_)))
    ) {
        return following_token_is_symbol(tokens, next, ';')
            || following_token_is_non_assignment_operator(tokens, next);
    }
    true
}

fn operator_is_unary_prefix(
    tokens: &[Token],
    previous: Option<usize>,
    next: Option<usize>,
) -> bool {
    let next_starts_expression = next
        .and_then(|index| tokens.get(index))
        .is_some_and(syntax_token_can_start_expression);
    let previous_allows_unary =
        previous.is_none_or(|index| syntax_token_allows_unary_after(&tokens[index]));
    next_starts_expression && previous_allows_unary
}

fn following_token_is_call_open(tokens: &[Token], index: usize) -> bool {
    next_non_layout_token_index(tokens, index + 1)
        .and_then(|next| tokens.get(next))
        .is_some_and(|token| matches!(token, Token::Symbol('(')))
}

fn following_token_is_assignment(tokens: &[Token], index: usize) -> bool {
    next_non_layout_token_index(tokens, index + 1)
        .and_then(|next| tokens.get(next))
        .is_some_and(|token| matches!(token, Token::Operator(operator) if language::ASSIGNMENT_OPERATORS.contains(&operator.as_str())))
}

fn following_token_is_non_assignment_operator(tokens: &[Token], index: usize) -> bool {
    next_non_layout_token_index(tokens, index + 1)
        .and_then(|next| tokens.get(next))
        .is_some_and(|token| {
            matches!(token, Token::Operator(operator) if !language::ASSIGNMENT_OPERATORS.contains(&operator.as_str()))
        })
}

fn following_token_is_symbol(tokens: &[Token], index: usize, symbol: char) -> bool {
    next_non_layout_token_index(tokens, index + 1)
        .and_then(|next| tokens.get(next))
        .is_some_and(|token| matches!(token, Token::Symbol(found) if *found == symbol))
}

fn paren_range_is_expression(tokens: &[Token], open: usize, close: usize) -> bool {
    if previous_token_skipping_layout(tokens, open)
        .and_then(|previous| tokens.get(previous))
        .is_some_and(|token| matches!(token, Token::Word(_)))
    {
        return false;
    }
    let Some(first) = first_token_in_range(tokens, open + 1, close) else {
        return false;
    };
    let Some(last) = last_token_in_range(tokens, open + 1, close) else {
        return false;
    };
    if syntax_token_is_type_word(&tokens[first])
        || range_contains_assignment(tokens, open + 1, close)
    {
        return false;
    }
    syntax_token_can_start_expression(&tokens[first])
        && syntax_token_can_end_expression(&tokens[last])
}

fn range_contains_assignment(tokens: &[Token], start: usize, end: usize) -> bool {
    tokens[start..end].iter().any(|token| {
        matches!(token, Token::Operator(operator) if language::ASSIGNMENT_OPERATORS.contains(&operator.as_str()))
    })
}

fn first_token_in_range(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end).find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

fn last_token_in_range(tokens: &[Token], start: usize, end: usize) -> Option<usize> {
    (start..end)
        .rev()
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

fn syntax_token_is_type_word(token: &Token) -> bool {
    match token {
        Token::Word(word) => is_type_like_pointer_word(word) && !is_non_type_keyword(word),
        _ => false,
    }
}

fn syntax_token_can_end_expression(token: &Token) -> bool {
    match token {
        Token::Word(word) => !is_non_type_keyword(word),
        Token::Operator(operator) if matches!(operator.as_str(), "++" | "--") => true,
        Token::Number(_) | Token::StringLiteral(_) | Token::CharLiteral(_) => true,
        Token::Symbol(')' | ']') => true,
        _ => false,
    }
}

fn syntax_token_can_start_expression(token: &Token) -> bool {
    matches!(
        token,
        Token::Word(_)
            | Token::Number(_)
            | Token::StringLiteral(_)
            | Token::CharLiteral(_)
            | Token::Symbol('(' | '[')
    )
}

fn syntax_token_allows_unary_after(token: &Token) -> bool {
    match token {
        Token::Operator(operator) => !matches!(operator.as_str(), ">" | ">>"),
        Token::Symbol('(' | '[' | '{' | ',' | ':' | '?' | ';') => true,
        Token::Word(word) => is_non_type_keyword(word),
        _ => false,
    }
}

fn previous_token_skipping_layout(tokens: &[Token], before: usize) -> Option<usize> {
    (0..before)
        .rev()
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

impl SyntaxRoles {
    pub(super) fn new(token_count: usize) -> Self {
        Self {
            token_roles: vec![SyntaxRole::Unknown; token_count],
            inside_parenthesized_expression: vec![false; token_count],
        }
    }

    pub(super) fn role_at(&self, index: usize) -> SyntaxRole {
        self.token_roles
            .get(index)
            .copied()
            .unwrap_or(SyntaxRole::Unknown)
    }

    pub(super) fn operator_role_at(&self, index: usize) -> OperatorRole {
        match self.role_at(index) {
            SyntaxRole::Operator(role) => role,
            _ => OperatorRole::Unknown,
        }
    }

    fn set_role(&mut self, index: usize, role: SyntaxRole) {
        if let Some(slot) = self.token_roles.get_mut(index) {
            *slot = role;
        }
    }

    fn token_inside_parenthesized_expression(&self, index: usize) -> bool {
        self.inside_parenthesized_expression
            .get(index)
            .copied()
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::{OperatorRole, SyntaxRole, classify_syntax};
    use crate::formatter::token::{Token, tokenize};

    fn operator_roles(source: &str, operator: &str) -> Vec<OperatorRole> {
        let tokens = tokenize(source);
        let roles = classify_syntax(&tokens);
        tokens
            .iter()
            .enumerate()
            .filter_map(|(index, token)| match token {
                Token::Operator(value) if value == operator => Some(roles.operator_role_at(index)),
                _ => None,
            })
            .collect()
    }

    fn word_roles(source: &str, target: &str) -> Vec<SyntaxRole> {
        let tokens = tokenize(source);
        let roles = classify_syntax(&tokens);
        tokens
            .iter()
            .enumerate()
            .filter_map(|(index, token)| match token {
                Token::Word(word) if word == target => Some(roles.role_at(index)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn classifies_trailing_return_pointer_declarator_star() {
        assert_eq!(
            operator_roles("auto function()->int*;\n", "*"),
            [OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_return_pointer_declarator_star() {
        assert_eq!(
            operator_roles("int *f(char a);\n", "*"),
            [OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_function_pointer_declarator_group_star() {
        assert_eq!(
            operator_roles("int (*fp)(int);\n", "*"),
            [OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_expression_star_as_binary_multiplication() {
        assert_eq!(
            operator_roles("x * f(1);\n", "*"),
            [OperatorRole::BinaryOperator]
        );
    }

    #[test]
    fn classifies_pointer_cast_range_and_star() {
        assert_eq!(
            operator_roles("call((int *)x);\n", "*"),
            [OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_parenthesized_expression_star_as_binary() {
        assert_eq!(
            operator_roles("call((a * b));\n", "*"),
            [OperatorRole::BinaryOperator]
        );
        assert_eq!(
            operator_roles("size_t size = (MIN_PAGES *page_size);\n", "*"),
            [OperatorRole::BinaryOperator]
        );
    }

    #[test]
    fn classifies_dereference_before_pointer_cast() {
        assert_eq!(
            operator_roles("value = *(int *)p;\n", "*"),
            [OperatorRole::UnaryOperator, OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_reference_declarator_ampersand() {
        assert_eq!(
            operator_roles("int &ref;\n", "&"),
            [OperatorRole::PointerDeclarator]
        );
    }

    #[test]
    fn classifies_address_of_and_bitwise_and() {
        assert_eq!(
            operator_roles("value = &item;\n", "&"),
            [OperatorRole::UnaryOperator]
        );
        assert_eq!(
            operator_roles("left & right;\n", "&"),
            [OperatorRole::BinaryOperator]
        );
    }

    #[test]
    fn leaves_unproven_macro_operator_unknown() {
        assert_eq!(operator_roles("MACRO(*);\n", "*"), [OperatorRole::Unknown]);
    }

    #[test]
    fn classifies_function_declarator() {
        assert_eq!(
            word_roles("int *f(char a);\n", "f"),
            [SyntaxRole::FunctionDeclarator]
        );
    }

    #[test]
    fn classifies_standalone_macro_invocation() {
        assert_eq!(
            word_roles("void f() { DO_MACRO(value); }\n", "DO_MACRO"),
            [SyntaxRole::StandaloneMacroInvocation]
        );
        assert_eq!(
            word_roles("ITEM_CASE(value)\n", "ITEM_CASE"),
            [SyntaxRole::StandaloneMacroInvocation]
        );
        assert_eq!(
            word_roles("ITEM_CASE(value)   \n", "ITEM_CASE"),
            [SyntaxRole::StandaloneMacroInvocation]
        );
    }

    #[test]
    fn leaves_uncertain_macro_typedef_shape_unknown() {
        assert_eq!(
            word_roles("MAYBE(value) name;\n", "MAYBE"),
            [SyntaxRole::Unknown]
        );
    }
}
