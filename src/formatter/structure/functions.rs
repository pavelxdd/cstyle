//! Function heads at declaration scope: return type, name, parameter list,
//! and body.

use std::collections::{HashMap, HashSet};

use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::{
    BlockKind, Blocks, next_code_token, previous_code_token,
};
use crate::formatter::structure::groups::{Delimiter, GroupId, Groups};
use crate::formatter::syntax::language::{is_header, is_non_type_keyword};

/// A function declaration or definition at file, namespace, `extern "C"`,
/// or class scope.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct FunctionHead {
    /// First token of the specifiers and return type; equals `name_start`
    /// when the head has no return type (constructors, destructors).
    pub(crate) start: usize,
    /// First token of the possibly qualified name (`ns::Type::f`).
    pub(crate) name_start: usize,
    /// The unqualified name token.
    pub(crate) name: usize,
    pub(crate) params: GroupId,
    /// Body of a definition; `None` for a declaration.
    pub(crate) body: Option<GroupId>,
}

impl FunctionHead {
    pub(crate) fn has_return_type(&self) -> bool {
        self.start < self.name_start
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct Functions {
    heads: Vec<FunctionHead>,
    by_params: HashMap<GroupId, usize>,
    by_start: HashMap<usize, usize>,
    by_name_start: HashMap<usize, usize>,
    /// Parameter lists outside `heads`: of function pointer declarators
    /// (`(*name)(...)`) and of functions whose body a macro supplies.
    other_parameter_lists: HashSet<GroupId>,
    /// Declarator groups such as `(*name)` in `int (*name)(void)` or
    /// `(name)` in `int (name)(void)`.
    declarators: HashSet<GroupId>,
    /// `*`, `&`, and `^` tokens in the return types of `heads`.
    return_type_pointers: HashSet<usize>,
}

impl Functions {
    pub(crate) fn build(tokens: &[Token], groups: &Groups, blocks: &Blocks) -> Self {
        let mut functions = Self::default();
        for params in groups.ids() {
            match function_head(tokens, groups, blocks, &functions.by_params, params) {
                Some(HeadMatch::Head(head)) => {
                    if let Some((outer, returned)) =
                        function_pointer_declarator(tokens, groups, params)
                    {
                        functions.declarators.insert(outer);
                        functions.other_parameter_lists.insert(returned);
                    }
                    // `int (lua_gettop) (lua_State *L)`
                    if let Some(name_group) = groups.opened_at(head.name_start) {
                        functions.declarators.insert(name_group);
                    }
                    functions.return_type_pointers.extend(
                        (head.start..head.name_start).filter(|&index| {
                            matches!(&tokens[index], Token::Operator(operator) if matches!(operator.as_str(), "*" | "&" | "&&" | "^"))
                        }),
                    );
                    let index = functions.heads.len();
                    functions.by_params.insert(params, index);
                    functions.by_start.insert(head.start, index);
                    functions.by_name_start.insert(head.name_start, index);
                    functions.heads.push(head);
                }
                Some(HeadMatch::ParameterList) => {
                    functions.other_parameter_lists.insert(params);
                }
                None => {
                    if let Some(declarator) = pointer_declarator_before(tokens, groups, params) {
                        functions.declarators.insert(declarator);
                        functions.other_parameter_lists.insert(params);
                    }
                }
            }
        }
        functions
    }

    /// Whether the paren group `id` lists parameters: of a function head or
    /// of a function pointer declarator.
    pub(crate) fn is_parameter_list(&self, id: GroupId) -> bool {
        self.by_params.contains_key(&id) || self.other_parameter_lists.contains(&id)
    }

    /// Whether the token at `index` is a `*`, `&`, or `^` in the return type
    /// of a function head.
    pub(crate) fn is_return_type_pointer(&self, index: usize) -> bool {
        self.return_type_pointers.contains(&index)
    }

    /// Whether the paren group `id` is a declarator such as `(*name)` in
    /// `int (*name)(void)` or `(name)` in `int (name)(void)`.
    pub(crate) fn is_declarator(&self, id: GroupId) -> bool {
        self.declarators.contains(&id)
    }

    /// The function head whose specifiers and return type start at `token`.
    pub(crate) fn starting_at(&self, token: usize) -> Option<&FunctionHead> {
        self.by_start.get(&token).map(|&index| &self.heads[index])
    }

    /// The function head whose name starts at `token`.
    pub(crate) fn named_at(&self, token: usize) -> Option<&FunctionHead> {
        self.by_name_start
            .get(&token)
            .map(|&index| &self.heads[index])
    }

    pub(crate) fn heads(&self) -> &[FunctionHead] {
        &self.heads
    }

    /// The function whose parameter list is the paren group `params`.
    pub(crate) fn by_params(&self, params: GroupId) -> Option<&FunctionHead> {
        self.by_params.get(&params).map(|&index| &self.heads[index])
    }
}

/// Previous token that carries code; a directive ends the search, since a
/// head never continues across one.
fn previous_head_token(tokens: &[Token], before: usize) -> Option<usize> {
    for index in (0..before).rev() {
        match tokens[index] {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => {}
            Token::Preprocessor(_) => return None,
            // An escaped newline continues the head.
            Token::Symbol('\\') => {}
            _ => return Some(index),
        }
    }
    None
}

fn is_builtin_type_word(word: &str) -> bool {
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
            | "bool"
            | "_Bool"
    )
}

/// A word that takes parentheses after a parameter list: `noexcept(...)`,
/// `throw()`, `__attribute__((...))`, `__nonnull((1))`, `LOCKS_EXCLUDED(mu)`.
fn is_suffix_call_word(word: &str) -> bool {
    matches!(word, "noexcept" | "throw")
        || is_attribute_word(word)
        || is_macro_word(word)
        || word.starts_with("__")
}

fn is_attribute_word(word: &str) -> bool {
    matches!(
        word,
        "__attribute__" | "__attribute" | "__declspec" | "alignas" | "_Alignas" | "__asm__" | "asm"
    )
}

/// An all-caps macro name such as `EXPORT` in `EXPORT(int) f(void)`.
fn is_macro_word(word: &str) -> bool {
    word.chars().any(|ch| ch.is_ascii_uppercase())
        && word
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn is_word(tokens: &[Token], index: usize) -> bool {
    matches!(tokens.get(index), Some(Token::Word(_)))
}

fn is_operator(tokens: &[Token], index: usize, expected: &str) -> bool {
    matches!(tokens.get(index), Some(Token::Operator(operator)) if operator == expected)
}

/// What a paren group at declaration scope turned out to be.
enum HeadMatch {
    Head(FunctionHead),
    /// The parameter list of a function whose head does not end in a body
    /// or `;` the tree can see.
    ParameterList,
}

/// `head_params` holds the parameter lists of heads found so far, which
/// lie to the left of `params`.
fn function_head(
    tokens: &[Token],
    groups: &Groups,
    blocks: &Blocks,
    head_params: &HashMap<GroupId, usize>,
    params: GroupId,
) -> Option<HeadMatch> {
    let group = groups.get(params);
    // `(*handler)` is a declarator, not a parameter list.
    if group.delimiter != Delimiter::Paren
        || next_code_token(tokens, group.open + 1)
            .is_some_and(|first| is_operator(tokens, first, "*") || is_operator(tokens, first, "^"))
        || !holds_declarations(tokens, groups, params)
    {
        return None;
    }
    let declarator = function_pointer_declarator(tokens, groups, params);
    let scope = match declarator {
        Some((outer, _)) => groups.get(outer).parent,
        None => group.parent,
    };
    if scope.is_some_and(|scope| groups.get(scope).delimiter != Delimiter::Brace)
        || !BlockKind::is_declaration_scope(scope.and_then(|scope| blocks.kind(scope)))
    {
        return None;
    }
    // `int init\n#endif\n(sqlite3 *db)`: a directive may separate the name
    // from its parameter list.
    let mut before_params = previous_code_token(tokens, group.open)?;
    // An escaped newline may part the name from its parameter list.
    if matches!(tokens[before_params], Token::Symbol('\\')) {
        before_params = previous_code_token(tokens, before_params)?;
    }
    // `int (lua_gettop) (lua_State *L)` keeps the name in parentheses.
    let (name, parenthesized_name) =
        if let Some(operator) = operator_function_name(tokens, groups, before_params) {
            (operator, None)
        } else if let Some(id) = groups.closed_at(before_params) {
            (
                sole_word_in_group(tokens, groups, id)?,
                Some(groups.get(id).open),
            )
        } else {
            (before_params, None)
        };
    let Token::Word(name_word) = &tokens[name] else {
        return None;
    };
    // C knows no `foreach`, so a function may take the name.
    if name_word != "operator"
        && (is_non_type_keyword(name_word)
            || is_builtin_type_word(name_word)
            || is_header(name_word) && !matches!(name_word.as_str(), "foreach" | "Q_FOREACH")
            || is_attribute_word(name_word))
    {
        return None;
    }
    let name_start = parenthesized_name.unwrap_or_else(|| qualified_name_start(tokens, name));
    let start = match declarator {
        Some((outer, _)) => return_type_start(tokens, groups, head_params, groups.get(outer).open)?,
        None => return_type_start(tokens, groups, head_params, name_start)?,
    };
    let class_scope = scope.and_then(|scope| blocks.kind(scope)) == Some(BlockKind::Aggregate);
    if start == name_start && !class_scope && name_start == name {
        // `MACRO(args)` at file scope: no return type and no qualifier.
        return None;
    }
    let last_params = declarator.map_or(params, |(_, returned)| returned);
    let body = match head_end(tokens, groups, last_params, params)? {
        HeadEnd::Found(body) => body,
        HeadEnd::Unterminated => return Some(HeadMatch::ParameterList),
    };
    if body.is_some_and(|body| {
        !matches!(
            blocks.kind(body),
            Some(BlockKind::FunctionBody | BlockKind::Unknown)
        )
    }) {
        return None;
    }
    Some(HeadMatch::Head(FunctionHead {
        start,
        name_start,
        name,
        params,
        body,
    }))
}

/// The `operator` word of an overloaded operator name that ends at `last`:
/// `operator+`, `operator==`, `operator()`, `operator[]`, `operator new[]`.
/// Conversion functions such as `operator bool` are not matched; they have
/// no return type.
fn operator_function_name(tokens: &[Token], groups: &Groups, last: usize) -> Option<usize> {
    let mut index = last;
    for _ in 0..3 {
        let previous = previous_head_token(tokens, index)?;
        if matches!(&tokens[previous], Token::Word(word) if word == "operator") {
            let symbol = next_code_token(tokens, previous + 1)?;
            let named = matches!(&tokens[symbol], Token::Word(word) if matches!(word.as_str(), "new" | "delete"));
            return (named || !is_word(tokens, symbol)).then_some(previous);
        }
        index = match groups.closed_at(previous) {
            Some(id) => groups.get(id).open,
            None => previous,
        };
    }
    None
}

/// First token of `Outer::Inner::name` (or `~name`) ending at `name`.
fn qualified_name_start(tokens: &[Token], name: usize) -> usize {
    let mut start = name;
    while let Some(previous) = previous_head_token(tokens, start) {
        if matches!(tokens[previous], Token::Operator(ref operator) if operator == "~") {
            start = previous;
            continue;
        }
        if is_operator(tokens, previous, "::")
            && let Some(qualifier) = previous_head_token(tokens, previous)
        {
            if is_word(tokens, qualifier) {
                start = qualifier;
                continue;
            }
            // `Outer<T1, T2>::name`
            if is_operator(tokens, qualifier, ">")
                && let Some(open) = template_arguments_start(tokens, qualifier)
                && let Some(template) = previous_head_token(tokens, open)
                && is_word(tokens, template)
            {
                start = template;
                continue;
            }
        }
        break;
    }
    start
}

/// Whether `tokens` hold an empty line: two line breaks with only
/// whitespace between them.
pub(crate) fn holds_empty_line(tokens: &[Token]) -> bool {
    let mut after_break = false;
    for token in tokens {
        match token {
            Token::Newline if after_break => return true,
            Token::Newline => after_break = true,
            Token::Whitespace(_) => {}
            _ => after_break = false,
        }
    }
    false
}

/// First token of the specifiers and return type before `name_start`, or
/// `None` when the tokens before the name cannot be a return type (`x = f(`,
/// `return f(`, `a, f(`).
fn return_type_start(
    tokens: &[Token],
    groups: &Groups,
    head_params: &HashMap<GroupId, usize>,
    name_start: usize,
) -> Option<usize> {
    let level = groups.enclosing(name_start);
    let mut start = name_start;
    while let Some(previous) = previous_head_token(tokens, start) {
        if groups.enclosing(previous) != level {
            // The opening brace of the enclosing scope.
            break;
        }
        match &tokens[previous] {
            Token::Symbol(';' | '{' | '}' | ':') => break,
            Token::Word(word) if is_non_type_keyword(word) || is_header(word) => return None,
            // A word above an empty line, such as a macro left without a
            // semicolon, ends what came before.
            Token::Word(_) if holds_empty_line(&tokens[previous..start]) => break,
            Token::Word(_) => start = previous,
            Token::Operator(operator)
                if matches!(operator.as_str(), "*" | "&" | "&&" | "::" | "^") =>
            {
                start = previous;
            }
            Token::Operator(operator) if matches!(operator.as_str(), ">" | ">>") => {
                let open = template_arguments_start(tokens, previous)?;
                // `template<class T>` introduces the head; it is not part of
                // the return type.
                if previous_head_token(tokens, open).is_some_and(
                    |word| matches!(&tokens[word], Token::Word(word) if word == "template"),
                ) {
                    break;
                }
                start = open;
            }
            Token::Symbol(')' | ']') => {
                // `__attribute__((...))`, `[[...]]`, `EXPORT(int)`.
                let id = groups.closed_at(previous)?;
                // `f(void) ATTRIBUTE(1)`: the group is the parameter list of
                // the head found before, and this is its suffix.
                if head_params.contains_key(&id) {
                    return None;
                }
                let open = groups.get(id).open;
                // On the line before the name, a macro group is part of the
                // return type only when the name starts the next line:
                // `API DEPRECATED(7.1, "x")` / `f(...)`, unlike `MACRO(a, b)` /
                // `static void f(...)`.
                let same_line_as_name = !tokens[previous..name_start]
                    .iter()
                    .any(|token| matches!(token, Token::Newline))
                    || first_code_token_after_line_break(tokens, previous, name_start)
                        == Some(name_start);
                start = match previous_head_token(tokens, open) {
                    Some(word)
                        if matches!(&tokens[word], Token::Word(word)
                            if is_attribute_word(word) || is_macro_word(word) && same_line_as_name) =>
                    {
                        word
                    }
                    _ if tokens[previous] == Token::Symbol(']') => open,
                    // `void printflike(1, 2) f(...)`: a call followed by the
                    // rest of the head on the same line is an attribute.
                    Some(word) if is_word(tokens, word) && same_line_as_name => word,
                    // The end of a previous line without a semicolon, such as
                    // a macro invocation `rb_gen(...)` or a declaration whose
                    // body a macro supplies: `f(void) NOT_REACHED`. The head
                    // starts on the next line.
                    _ => {
                        return first_code_token_after_line_break(tokens, previous, name_start);
                    }
                };
            }
            // `extern "C" int f(void)`
            Token::StringLiteral(_)
                if previous_head_token(tokens, previous).is_some_and(
                    |word| matches!(&tokens[word], Token::Word(word) if word == "extern"),
                ) =>
            {
                start = previous;
            }
            _ => return None,
        }
    }
    Some(start)
}

/// For `void (*f(args))(void)`: the group `(*f(args))` and the parameter
/// list `(void)` of the returned function pointer.
fn function_pointer_declarator(
    tokens: &[Token],
    groups: &Groups,
    params: GroupId,
) -> Option<(GroupId, GroupId)> {
    let outer = groups.get(params).parent?;
    let outer_group = groups.get(outer);
    if outer_group.delimiter != Delimiter::Paren
        || !next_code_token(tokens, outer_group.open + 1)
            .is_some_and(|star| is_operator(tokens, star, "*"))
        || next_code_token(tokens, groups.get(params).close? + 1) != outer_group.close
    {
        return None;
    }
    let returned = groups.opened_at(next_code_token(tokens, outer_group.close? + 1)?)?;
    (groups.get(returned).delimiter == Delimiter::Paren).then_some((outer, returned))
}

/// The declarator group before the paren group `params`, such as
/// `(*handler)`, `(*)`, `(^block)`, `(&handler)`, `(Owner::*member)`,
/// `(*const table[])`, or
/// `(WINAPI *const callback)`, when that group itself follows a type, as in
/// `int (*handler)(char *s)`. A call through a pointer, `x = (*fp)(a * b)`,
/// has no type before the declarator group.
fn pointer_declarator_before(
    tokens: &[Token],
    groups: &Groups,
    params: GroupId,
) -> Option<GroupId> {
    let group = groups.get(params);
    if group.delimiter != Delimiter::Paren {
        return None;
    }
    let declarator_id = groups.closed_at(previous_head_token(tokens, group.open)?)?;
    let declarator = groups.get(declarator_id);
    let close = declarator.close?;
    let mut saw_pointer = false;
    let mut index = declarator.open + 1;
    while let Some(next) = next_code_token(tokens, index).filter(|&next| next < close) {
        if groups.enclosing(next) == Some(declarator_id) {
            match &tokens[next] {
                Token::Operator(operator)
                    if matches!(operator.as_str(), "*" | "^" | "&" | "&&") =>
                {
                    saw_pointer = true;
                }
                // Before the pointer only a calling convention or a member
                // pointer's class may appear, which tells `(WINAPI *f)` and
                // `(Owner::*f)` from a cast such as `*(void **)(p)`.
                Token::Word(word)
                    if saw_pointer
                        || is_macro_word(word)
                        || word.starts_with("__")
                        || next_code_token(tokens, next + 1)
                            .is_some_and(|after| is_operator(tokens, after, "::")) => {}
                Token::Operator(operator) if operator == "::" => {}
                Token::Symbol('[' | ']') if saw_pointer => {}
                _ => return None,
            }
        }
        index = next + 1;
    }
    let after_type =
        previous_head_token(tokens, declarator.open).is_some_and(|before| match &tokens[before] {
            Token::Word(word) => !is_non_type_keyword(word),
            Token::Operator(operator) => matches!(operator.as_str(), "*" | "&"),
            _ => false,
        });
    (saw_pointer && after_type).then_some(declarator_id)
}

/// Whether the paren group `params` can hold parameter declarations: outside
/// default arguments it has no literals and no operators other than those of
/// declarators, as opposed to call arguments such as `(1)` or `(a + b)`.
fn holds_declarations(tokens: &[Token], groups: &Groups, params: GroupId) -> bool {
    let group = groups.get(params);
    let Some(close) = group.close else {
        return true;
    };
    let mut default_argument = false;
    for (index, token) in tokens.iter().enumerate().take(close).skip(group.open + 1) {
        if groups.enclosing(index) != Some(params) {
            continue;
        }
        match token {
            Token::Symbol(',') => default_argument = false,
            _ if default_argument => {}
            Token::Operator(operator) if operator == "=" => default_argument = true,
            Token::Number(_) | Token::StringLiteral(_) | Token::CharLiteral(_) => return false,
            Token::Operator(operator)
                if !matches!(
                    operator.as_str(),
                    "*" | "&" | "&&" | "^" | "::" | "<" | ">" | ">>"
                ) =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// The only code token inside the group `id`, when it is a word.
fn sole_word_in_group(tokens: &[Token], groups: &Groups, id: GroupId) -> Option<usize> {
    let group = groups.get(id);
    let word = next_code_token(tokens, group.open + 1)?;
    (is_word(tokens, word) && next_code_token(tokens, word + 1) == group.close).then_some(word)
}

/// First code token of the first line that starts after `from`, if that
/// line starts no later than `limit`.
fn first_code_token_after_line_break(tokens: &[Token], from: usize, limit: usize) -> Option<usize> {
    let line_break = (from..limit).find(|&index| matches!(tokens[index], Token::Newline))?;
    next_code_token(tokens, line_break + 1).filter(|&first| first <= limit)
}

/// Opening `<` of the template argument list closed at `close`.
pub(crate) fn template_arguments_start(tokens: &[Token], close: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..=close).rev() {
        match &tokens[index] {
            Token::Operator(operator) if operator == ">" => depth += 1,
            Token::Operator(operator) if operator == ">>" => depth += 2,
            Token::Operator(operator) if operator == "<" => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            Token::Symbol(';' | '{' | '}') => return None,
            _ => {}
        }
    }
    None
}

/// How the tokens after a function declarator end the head.
enum HeadEnd {
    /// A body (`Some`) or `;` (`None`).
    Found(Option<GroupId>),
    /// The head ends without a body or `;` the tree can see, as when a macro
    /// supplies the body: `bool f(void) NOT_REACHED`.
    Unterminated,
}

/// How the head whose parameter list is `params` and whose declarator ends
/// with the paren group `last_params` ends; `None` when the tokens after the
/// declarator show that it is not a function head.
fn head_end(
    tokens: &[Token],
    groups: &Groups,
    last_params: GroupId,
    params: GroupId,
) -> Option<HeadEnd> {
    let group = groups.get(last_params);
    let level = group.parent;
    let mut index = group.close? + 1;
    let knr_parameters = is_identifier_list(tokens, groups, params);
    let mut constructor_initializers = false;
    // The first `;` after an identifier list may end a declaration or start
    // K&R parameter declarations; when the tokens after it are no K&R
    // declarations, the head was a declaration ending there.
    let mut knr_semicolon = None;
    let unterminated = |knr_semicolon: Option<usize>| {
        Some(knr_semicolon.map_or(HeadEnd::Unterminated, |_| HeadEnd::Found(None)))
    };
    while let Some(next) = next_code_token(tokens, index) {
        // A directive ends what the tree can see of the head.
        if tokens[index..next]
            .iter()
            .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return unterminated(knr_semicolon);
        }
        if groups.enclosing(next) != level {
            return None;
        }
        // Past the parameter list, parentheses belong to attributes,
        // exception specifications, or constructor initializers; any other
        // call-like group starts the next declaration. K&R parameter
        // declarations hold parentheses only as function pointer
        // declarators, `int (*cmp)();`.
        let knr_declarator = knr_semicolon.is_some()
            && (next_code_token(tokens, next + 1)
                .is_some_and(|first| is_operator(tokens, first, "*"))
                || previous_head_token(tokens, next)
                    .and_then(|before| groups.closed_at(before))
                    .is_some());
        if tokens[next] == Token::Symbol('(')
            && !knr_declarator
            && !constructor_initializers
            && !previous_head_token(tokens, next).is_some_and(
                |before| matches!(&tokens[before], Token::Word(word) if is_suffix_call_word(word)),
            )
        {
            if knr_semicolon.is_some() {
                return unterminated(knr_semicolon);
            }
            // On the same line the group was an attribute macro such as
            // `printflike(1, 2)`, not a parameter list; on a later line the
            // head ended without a visible body or `;`.
            let same_line = !tokens[group.close? + 1..next]
                .iter()
                .any(|token| matches!(token, Token::Newline));
            return (!same_line).then_some(HeadEnd::Unterminated);
        }
        match &tokens[next] {
            Token::Symbol(':') => constructor_initializers = true,
            Token::Symbol('{') => return Some(HeadEnd::Found(groups.opened_at(next))),
            Token::Symbol(';') => {
                // `f(a) int a; char *b; {`: declarations between the
                // identifier list and the body.
                let knr_declarations = knr_parameters
                    && (knr_semicolon.is_some()
                        || next_code_token(tokens, group.close? + 1) != Some(next));
                if knr_declarations && let Some(after) = next_code_token(tokens, next + 1) {
                    if tokens[after] == Token::Symbol('{') {
                        return Some(HeadEnd::Found(groups.opened_at(after)));
                    }
                    if is_word(tokens, after) {
                        knr_semicolon.get_or_insert(next);
                        index = next + 1;
                        continue;
                    }
                }
                return Some(HeadEnd::Found(None));
            }
            Token::Symbol('}') => return unterminated(knr_semicolon),
            Token::Operator(operator) if operator == "=" && knr_semicolon.is_some() => {
                return unterminated(knr_semicolon);
            }
            Token::Symbol(',') if knr_semicolon.is_none() => return Some(HeadEnd::Found(None)),
            _ => {}
        }
        index = match groups.opened_at(next) {
            Some(id) => groups.get(id).close? + 1,
            None => next + 1,
        };
    }
    unterminated(knr_semicolon)
}

/// Whether the parameter list holds only identifiers separated by commas,
/// as in a K&R definition `f(a, b) int a; char *b; { ... }`.
fn is_identifier_list(tokens: &[Token], groups: &Groups, params: GroupId) -> bool {
    let group = groups.get(params);
    let Some(close) = group.close else {
        return false;
    };
    let mut expect_word = true;
    let mut words = 0;
    for token in &tokens[group.open + 1..close] {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => {}
            Token::Word(word) if expect_word && !is_builtin_type_word(word) => {
                words += 1;
                expect_word = false;
            }
            Token::Symbol(',') if !expect_word => expect_word = true,
            _ => return false,
        }
    }
    words > 0 && !expect_word
}

#[cfg(test)]
mod tests {
    use super::Functions;
    use crate::formatter::lexer::{Token, token_text, tokenize};
    use crate::formatter::structure::blocks::Blocks;
    use crate::formatter::structure::groups::Groups;

    /// `(return type, name, is definition)` of every function head.
    fn heads(source: &str) -> Vec<(String, String, bool)> {
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let blocks = Blocks::build(&tokens, &groups);
        let text = |range: std::ops::Range<usize>| {
            tokens[range]
                .iter()
                .filter(|token| !matches!(token, Token::Newline | Token::Whitespace(_)))
                .map(token_text)
                .collect::<Vec<_>>()
                .join(" ")
        };
        Functions::build(&tokens, &groups, &blocks)
            .heads()
            .iter()
            .map(|head| {
                (
                    text(head.start..head.name_start),
                    text(head.name_start..head.name + 1),
                    head.body.is_some(),
                )
            })
            .collect()
    }

    fn head(return_type: &str, name: &str, definition: bool) -> (String, String, bool) {
        (return_type.to_owned(), name.to_owned(), definition)
    }

    #[test]
    fn finds_definitions_and_declarations_with_split_return_types() {
        assert_eq!(
            heads(
                "static int\nf(\n    T *a)\n{\n    g(a * b);\n}\nconst char *name(void);\nstruct s *make(int n) { return 0; }\n"
            ),
            [
                head("static int", "f", true),
                head("const char *", "name", false),
                head("struct s *", "make", true),
            ]
        );
    }

    #[test]
    fn skips_calls_macros_and_initializers() {
        assert_eq!(
            heads(
                "MODULE_INIT(x)\nint v = f(1);\nstatic T t = { g(2) };\nvoid h(void)\n{\n    int k(int);\n    return k(3);\n}\n"
            ),
            [head("void", "h", true)]
        );
    }

    #[test]
    fn handles_suffixes_attributes_and_knr_definitions() {
        assert_eq!(
            heads(
                "int f(a, b)\nint a;\nchar *b;\n{\n}\nvoid g(void) __attribute__((noreturn));\nLUA_API int (lua_gettop) (lua_State *L) {\n}\nbool create(void) NOT_REACHED\nbool enable(tsd_t *tsd) NOT_REACHED\nstatic void (*f(int k))(void) {\n}\n__attribute__((cold)) void h(void) {}\n"
            ),
            [
                head("int", "f", true),
                head("void", "g", false),
                head("LUA_API int", "( lua_gettop", true),
                head("static void ( *", "f", true),
                head("__attribute__ ( ( cold ) ) void", "h", true),
            ]
        );
    }

    /// Text of every paren group that lists parameters, heads included.
    fn parameter_lists(source: &str) -> Vec<String> {
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let blocks = Blocks::build(&tokens, &groups);
        let functions = Functions::build(&tokens, &groups, &blocks);
        groups
            .ids()
            .filter(|&id| functions.is_parameter_list(id))
            .map(|id| {
                let group = groups.get(id);
                tokens[group.open..=group.close.expect("closed")]
                    .iter()
                    .map(token_text)
                    .collect()
            })
            .collect()
    }

    #[test]
    fn finds_parameter_lists_outside_heads() {
        assert_eq!(
            parameter_lists(
                "bool enable(tsd_t *tsd) NOT_REACHED\ntypedef int (*cb_t)(char *s);\nstatic const void (WINAPI *const tls)(void *h);\nstatic int (*const table[])(sqlite3 *db);\nvoid printflike(1, 2) add(const char *fmt, ...);\nstatic int (*hash(int k))(const void *p, int n) {\n}\nvoid f(void)\n{\n    g((int (*)(void *))h);\n    x = (*fp)(a * b);\n    y = *(void **)(p);\n}\nint init\n#endif\n(sqlite3 *db) {\n}\n"
            ),
            [
                "(tsd_t *tsd)",
                "(char *s)",
                "(void *h)",
                "(sqlite3 *db)",
                "(const char *fmt, ...)",
                "(int k)",
                "(const void *p, int n)",
                "(void)",
                "(void *)",
                "(sqlite3 *db)",
            ]
        );
    }

    #[test]
    fn tells_knr_definitions_from_following_declarations() {
        assert_eq!(
            heads(
                "int f(a)\nint a;\n{\n}\nint g(size_t);\nint h(int);\nx = foo(a);\nint k(n) __THROW;\nint m(void);\n"
            ),
            [
                head("int", "f", true),
                head("int", "g", false),
                head("int", "h", false),
                head("int", "k", false),
                head("int", "m", false),
            ]
        );
    }

    #[test]
    fn handles_cpp_heads() {
        assert_eq!(
            heads(
                "namespace n {\nclass C {\npublic:\n    C(int x);\n    ~C();\n    virtual int get() const = 0;\n    int (*handler)(int);\n    run(int);\n};\nstd::vector<int> C::values() const {\n}\n}\nT operator+(T a, T b) {\n}\nbool Cls::operator==(int a);\nvoid *Type::operator new(unsigned long size) {\n}\nPair<T1, T2>::Pair(int *p) {\n}\n"
            ),
            [
                head("", "C", false),
                head("", "~ C", false),
                head("virtual int", "get", false),
                head("", "run", false),
                head("std :: vector < int >", "C :: values", true),
                head("T", "operator", true),
                head("bool", "Cls :: operator", false),
                head("void *", "Type :: operator", true),
                head("", "Pair < T1 , T2 > :: Pair", true),
            ]
        );
    }
}
