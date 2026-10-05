//! Token roles decided before layout, and the C-family vocabulary they use.

use crate::formatter::index_hash::IndexSet;
use crate::formatter::lexer::{
    Token, matching_close_paren_index, next_non_layout_token_index, next_non_whitespace,
    previous_non_layout_token_index,
};
use crate::formatter::structure::SourceTree;
use crate::formatter::syntax::language::{
    is_macro_like_word, is_non_type_keyword, is_pointer_type_word, is_type_like_pointer_word,
};
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{
    is_identifier_continue, is_identifier_start, is_word_char, trailing_word,
};

pub(crate) mod language;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OperatorRole {
    Unknown,
    PointerDeclarator,
    BinaryOperator,
    UnaryOperator,
}

pub(crate) fn signature_ends_with_parameter_list(line: &str) -> bool {
    let mut rest = line.trimmed_end();
    loop {
        if rest.ends_with(')') {
            return true;
        }
        if let Some(stripped) = rest.strip_suffix("&&").or_else(|| rest.strip_suffix('&')) {
            rest = stripped.trimmed_end();
            continue;
        }
        let word = trailing_word(rest);
        if matches!(
            word,
            "const" | "volatile" | "noexcept" | "override" | "final" | "mutable" | "try"
        ) {
            rest = rest[..rest.len() - word.len()].trimmed_end();
            continue;
        }
        return false;
    }
}

pub(crate) fn function_name_start(before_open_paren: &str) -> Option<usize> {
    let end = before_open_paren.trimmed_end().len();
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
            // `Type<Args>::name`
            if start >= 3
                && bytes[start - 3] == b'>'
                && let Some(open) = template_arguments_open(&bytes[..start - 2])
                && let Some(index) = identifier_segment_start(open)
            {
                start = index;
                continue;
            }
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

/// Index of the `<` matching the `>` that ends `bytes`.
fn template_arguments_open(bytes: &[u8]) -> Option<usize> {
    let mut depth = 0usize;
    for (index, &byte) in bytes.iter().enumerate().rev() {
        match byte {
            b'>' => depth += 1,
            b'<' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            b'(' | b')' | b';' | b'{' | b'}' => return None,
            _ => {}
        }
    }
    None
}

pub(crate) fn function_head_has_assignment(before: &str) -> bool {
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
    let after = before_open_paren[start + language::OPERATOR.len()..].trimmed_start();
    (!after.is_empty()
        && (!after.chars().next().is_some_and(is_identifier_start)
            || first_operator_word(after).is_some_and(is_named_operator_word)))
    .then_some(start)
}

pub(crate) fn first_operator_word(after_operator: &str) -> Option<&str> {
    after_operator
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .filter(|word| !word.is_empty())
}

pub(crate) fn is_named_operator_word(word: &str) -> bool {
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
pub(crate) enum SyntaxRole {
    Unknown,
    FunctionDeclarator,
    StandaloneMacroInvocation,
    Operator(OperatorRole),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SyntaxRoles {
    token_roles: Vec<SyntaxRole>,
    inside_parenthesized_expression: Vec<bool>,
}

pub(crate) fn template_angle_role(
    tokens: &[Token],
    index: usize,
    end: usize,
    template_depth: usize,
) -> TemplateAngle {
    known_template_angle_role(tokens, index, template_depth, || {
        looks_like_template_opener(tokens, index, end)
    })
}

/// Which tokens are `<`s that open a template, as
/// `looks_like_template_opener` finds each one reading on to the end, found
/// in one pass: each `<` still open is resolved by the token that ends its
/// own scan.
pub(crate) fn template_openers(tokens: &[Token]) -> Vec<bool> {
    let end = tokens.len();
    let mut opens = vec![false; end];
    let closing_follows = |cursor: usize| {
        next_non_whitespace(tokens, cursor + 1, end)
            .and_then(|next| tokens.get(next))
            .is_none_or(|token| !matches!(token, Token::Number(_)))
    };
    // The `<`s still open, innermost last, with the paren depth each stands
    // at and whether what follows it lets it open at all.
    let mut open: Vec<(usize, isize, bool)> = Vec::new();
    let mut paren_depth = 0isize;
    for (cursor, token) in tokens.iter().enumerate() {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => {}
            Token::Word(_) | Token::Number(_) => {}
            Token::Operator(operator) if operator == "<" => {
                let first_after_open =
                    next_non_whitespace(tokens, cursor + 1, end).and_then(|next| tokens.get(next));
                let viable = first_after_open.is_some_and(
                    |token| !matches!(token, Token::Operator(operator) if operator == "="),
                );
                open.push((cursor, paren_depth, viable));
            }
            Token::Operator(operator) if operator == ">" || operator == ">>" => {
                for _ in 0..operator.len() {
                    let Some((opener, opener_depth, viable)) = open.pop() else {
                        break;
                    };
                    opens[opener] =
                        viable && opener_depth == paren_depth && closing_follows(cursor);
                }
            }
            Token::Operator(operator)
                if matches!(
                    operator.as_str(),
                    "::" | "*" | "&" | "&&" | "^" | "=" | "!" | "!="
                ) => {}
            Token::Symbol('(') => paren_depth += 1,
            Token::Symbol(')') => {
                while open
                    .last()
                    .is_some_and(|&(_, opener_depth, _)| opener_depth == paren_depth)
                {
                    open.pop();
                }
                paren_depth -= 1;
            }
            Token::Symbol(',' | ':' | '[' | ']') => {}
            _ => open.clear(),
        }
    }
    opens
}

/// The role of the angle at `index` when `opens` tells whether a `<` there
/// would open a template outside one.
pub(crate) fn known_template_angle_role(
    tokens: &[Token],
    index: usize,
    template_depth: usize,
    opens: impl FnOnce() -> bool,
) -> TemplateAngle {
    match tokens.get(index) {
        Some(Token::Operator(operator)) if operator == "<" => {
            if template_depth > 0 || opens() {
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

pub(crate) fn scoped_name_is_constructor(name: &str) -> bool {
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

pub(crate) fn assignment_declarator_offset(line: &str) -> Option<usize> {
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
                let previous = if i > 0 { bytes[i - 1] } else { b' ' };
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                if !matches!(
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
    let head = line[..eq].trimmed_end();
    if head.contains_any_byte(b"(),{}") {
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

pub(crate) fn access_modified_brace_indices(tokens: &[Token]) -> IndexSet<usize> {
    let mut indices = IndexSet::default();
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

pub(crate) fn nested_brace_array_indices(tokens: &[Token]) -> IndexSet<usize> {
    let mut indices = IndexSet::default();
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

pub(crate) fn classify_syntax(tokens: &[Token], tree: &SourceTree) -> SyntaxRoles {
    let mut roles = SyntaxRoles::new(tokens.len());
    classify_paren_ranges(tokens, &mut roles);
    classify_word_roles(tokens, &mut roles);
    for (index, token) in tokens.iter().enumerate() {
        let role = match token {
            Token::Operator(operator)
                if matches!(operator.as_str(), "*" | "&")
                    && is_tree_declarator_operator(tokens, tree, index) =>
            {
                OperatorRole::PointerDeclarator
            }
            Token::Operator(operator) if operator == "*" && in_declared_star_run(tokens, index) => {
                OperatorRole::PointerDeclarator
            }
            Token::Operator(operator)
                if operator == "*" && is_qualified_function_pointer_star(tokens, tree, index) =>
            {
                OperatorRole::PointerDeclarator
            }
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

/// Whether the operator at `index` declares a pointer or reference by the
/// structure tree: it sits in the return type of a function head, or
/// directly in a parameter list before any default argument of its
/// parameter.
fn is_tree_declarator_operator(tokens: &[Token], tree: &SourceTree, index: usize) -> bool {
    if tree.functions.is_return_type_pointer(index) {
        return true;
    }
    let Some(group) = tree.groups.enclosing(index) else {
        return false;
    };
    if !tree.functions.is_parameter_list(group) {
        return false;
    }
    let open = tree.groups.get(group).open;
    !tokens[open + 1..index]
        .iter()
        .enumerate()
        .rev()
        .filter(|&(offset, _)| tree.groups.enclosing(open + 1 + offset) == Some(group))
        .map(|(_, token)| token)
        .take_while(|token| !matches!(token, Token::Symbol(',')))
        .any(|token| matches!(token, Token::Operator(operator) if operator == "="))
}

fn classify_paren_ranges(tokens: &[Token], roles: &mut SyntaxRoles) {
    let mut stack = Vec::new();
    let mut depth_changes = vec![0isize; tokens.len()];
    // Assignments before each token, so a range counts its own at once.
    let mut assignments_before = Vec::with_capacity(tokens.len() + 1);
    assignments_before.push(0usize);
    for token in tokens {
        let assigns = matches!(token, Token::Operator(operator) if language::ASSIGNMENT_OPERATORS.contains(&operator.as_str()));
        assignments_before
            .push(assignments_before.last().copied().unwrap_or(0) + usize::from(assigns));
    }
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Symbol('(') => stack.push(index),
            Token::Symbol(')') => {
                let Some(open) = stack.pop() else {
                    continue;
                };
                let assigns = assignments_before[index] > assignments_before[open + 1];
                if paren_range_is_expression(tokens, open, index, assigns) && open + 1 < index {
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
        let previous = previous_non_layout_token_index(tokens, index);
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
    while let Some(previous) = previous_non_layout_token_index(tokens, cursor) {
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

/// Whether the `*` at `index` is in a run of two or more stars that
/// declares a name at the start of a statement or a `for` header, as in
/// `for (const char **p = argv;`.
/// Whether the `*` at `index` declares the name of a function pointer
/// after a calling convention, as in `int (WINAPI *name)(int)`.
fn is_qualified_function_pointer_star(tokens: &[Token], tree: &SourceTree, index: usize) -> bool {
    let Some(qualifier) = previous_non_layout_token_index(tokens, index) else {
        return false;
    };
    let Some(open) = previous_non_layout_token_index(tokens, qualifier) else {
        return false;
    };
    let name = next_non_layout_token_index(tokens, index + 1);
    let close = name.and_then(|name| next_non_layout_token_index(tokens, name + 1));
    let parameters = close.and_then(|close| next_non_layout_token_index(tokens, close + 1));
    if !matches!(tokens[qualifier], Token::Word(_))
        || !matches!(tokens[open], Token::Symbol('('))
        || !name.is_some_and(|name| matches!(tokens[name], Token::Word(_)))
        || !close.is_some_and(|close| matches!(tokens[close], Token::Symbol(')')))
        || !parameters.is_some_and(|open| matches!(tokens[open], Token::Symbol('(')))
    {
        return false;
    }
    // The declaration's type words lead the statement up to the group.
    let mut words = 0;
    let mut cursor = open;
    let mut typedef = false;
    while let Some(previous) = previous_non_layout_token_index(tokens, cursor) {
        match &tokens[previous] {
            Token::Word(word) if !is_non_type_keyword(word) => {
                typedef |= word == "typedef";
                words += 1;
                cursor = previous;
            }
            Token::Symbol(';' | '{' | '}') | Token::Preprocessor(_) => break,
            _ => return false,
        }
    }
    // At file scope a single type word declares too.
    words >= 2 || typedef || words == 1 && tree.groups.enclosing(open).is_none()
}

fn in_declared_star_run(tokens: &[Token], index: usize) -> bool {
    let is_star =
        |index: usize| matches!(&tokens[index], Token::Operator(operator) if operator == "*");
    let mut first = index;
    while let Some(previous) = previous_non_layout_token_index(tokens, first)
        && is_star(previous)
    {
        first = previous;
    }
    let mut last = index;
    while let Some(next) = next_non_layout_token_index(tokens, last + 1)
        && is_star(next)
    {
        last = next;
    }
    if first == last {
        return false;
    }
    let declares_name = next_non_layout_token_index(tokens, last + 1)
        .filter(|&name| matches!(tokens[name], Token::Word(_)))
        .and_then(|name| next_non_layout_token_index(tokens, name + 1))
        .is_some_and(|after| {
            matches!(tokens[after], Token::Symbol(';' | ',' | '['))
                || matches!(&tokens[after], Token::Operator(operator) if operator == "=")
        });
    if !declares_name {
        return false;
    }
    let mut words = 0;
    let mut cursor = first;
    while let Some(previous) = previous_non_layout_token_index(tokens, cursor) {
        match &tokens[previous] {
            Token::Word(word) if !is_non_type_keyword(word) => {
                words += 1;
                cursor = previous;
            }
            Token::Symbol('(' | ';' | '{' | '}') => return words > 0,
            _ => return false,
        }
    }
    words > 0
}

fn classify_star_operator(tokens: &[Token], index: usize, roles: &SyntaxRoles) -> OperatorRole {
    let previous = previous_non_layout_token_index(tokens, index);
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
    let previous = previous_non_layout_token_index(tokens, index);
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
        .and_then(|index| previous_non_layout_token_index(tokens, index))
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
            .and_then(|open_index| previous_non_layout_token_index(tokens, open_index))
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

/// Whether the token at `close` is the `)` ending the condition of an
/// `if`, `while`, `for`, or `switch`.
fn closes_control_header(tokens: &[Token], close: usize) -> bool {
    if !matches!(tokens.get(close), Some(Token::Symbol(')'))) {
        return false;
    }
    let mut depth = 0usize;
    let mut index = close;
    loop {
        match tokens[index] {
            Token::Symbol(')') => depth += 1,
            Token::Symbol('(') => {
                depth -= 1;
                if depth == 0 {
                    return previous_non_layout_token_index(tokens, index).is_some_and(|keyword| {
                        matches!(&tokens[keyword], Token::Word(word)
                            if matches!(word.as_str(), "if" | "while" | "for" | "switch"))
                    });
                }
            }
            Token::Symbol(';' | '{' | '}')
                if depth <= 1 && !matches!(tokens[index], Token::Symbol(';')) =>
            {
                return false;
            }
            _ => {}
        }
        if index == 0 {
            return false;
        }
        index -= 1;
    }
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
    // After the condition of a control header a statement starts.
    if closes_control_header(tokens, previous) {
        return false;
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

fn paren_range_is_expression(tokens: &[Token], open: usize, close: usize, assigns: bool) -> bool {
    if previous_non_layout_token_index(tokens, open)
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
    if syntax_token_is_type_word(&tokens[first]) || assigns {
        return false;
    }
    syntax_token_can_start_expression(&tokens[first])
        && syntax_token_can_end_expression(&tokens[last])
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

impl SyntaxRoles {
    pub(crate) fn new(token_count: usize) -> Self {
        Self {
            token_roles: vec![SyntaxRole::Unknown; token_count],
            inside_parenthesized_expression: vec![false; token_count],
        }
    }

    pub(crate) fn role_at(&self, index: usize) -> SyntaxRole {
        self.token_roles
            .get(index)
            .copied()
            .unwrap_or(SyntaxRole::Unknown)
    }

    pub(crate) fn operator_role_at(&self, index: usize) -> OperatorRole {
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum TemplateAngle {
    None,
    Open,
    Close(usize),
}

#[cfg(test)]
mod tests {
    use super::{
        OperatorRole, SyntaxRole, classify_syntax, looks_like_template_opener, template_openers,
    };
    use crate::formatter::lexer::{Token, tokenize};
    use crate::formatter::structure::SourceTree;

    fn operator_roles(source: &str, operator: &str) -> Vec<OperatorRole> {
        let tokens = tokenize(source);
        let roles = classify_syntax(&tokens, &SourceTree::build(&tokens));
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
        let roles = classify_syntax(&tokens, &SourceTree::build(&tokens));
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

    #[test]
    fn template_openers_match_the_scan_from_each_angle() {
        const PIECES: [&str; 18] = [
            "<", ">", ">>", "(", ")", "a", "1", " ", ",", ";", "=", "::", "*", "[", "]", "+", "\n",
            "<<",
        ];
        let mut state = 0x2545_f491_4f6c_dd1du64;
        for _ in 0..4000 {
            let mut source = String::new();
            for _ in 0..(state % 24) {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                source.push_str(PIECES[(state % PIECES.len() as u64) as usize]);
                source.push(' ');
            }
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let tokens = tokenize(&source);
            let openers = template_openers(&tokens);
            for (index, token) in tokens.iter().enumerate() {
                if matches!(token, Token::Operator(operator) if operator == "<") {
                    assert_eq!(
                        openers[index],
                        looks_like_template_opener(&tokens, index, tokens.len()),
                        "{source:?} at {index}"
                    );
                }
            }
        }
    }
}
