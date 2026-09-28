//! Function heads at declaration scope: return type, name, parameter list,
//! and body.

use std::collections::HashMap;

use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::{BlockKind, Blocks, next_code_token};
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
}

impl Functions {
    pub(crate) fn build(tokens: &[Token], groups: &Groups, blocks: &Blocks) -> Self {
        let mut functions = Self::default();
        for params in groups.ids() {
            if let Some(head) = function_head(tokens, groups, blocks, params) {
                let index = functions.heads.len();
                functions.by_params.insert(params, index);
                functions.by_start.insert(head.start, index);
                functions.by_name_start.insert(head.name_start, index);
                functions.heads.push(head);
            }
        }
        functions
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

fn function_head(
    tokens: &[Token],
    groups: &Groups,
    blocks: &Blocks,
    params: GroupId,
) -> Option<FunctionHead> {
    let group = groups.get(params);
    // `(*handler)` is a declarator, not a parameter list.
    if group.delimiter != Delimiter::Paren
        || next_code_token(tokens, group.open + 1)
            .is_some_and(|first| is_operator(tokens, first, "*") || is_operator(tokens, first, "^"))
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
    let before_params = previous_head_token(tokens, group.open)?;
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
    if name_word != "operator"
        && (is_non_type_keyword(name_word)
            || is_builtin_type_word(name_word)
            || is_header(name_word)
            || is_attribute_word(name_word))
    {
        return None;
    }
    let name_start = parenthesized_name.unwrap_or_else(|| qualified_name_start(tokens, name));
    let start = match declarator {
        Some((outer, _)) => return_type_start(tokens, groups, groups.get(outer).open)?,
        None => return_type_start(tokens, groups, name_start)?,
    };
    let class_scope = scope.and_then(|scope| blocks.kind(scope)) == Some(BlockKind::Aggregate);
    if start == name_start && !class_scope && name_start == name {
        // `MACRO(args)` at file scope: no return type and no qualifier.
        return None;
    }
    let last_params = declarator.map_or(params, |(_, returned)| returned);
    let body = head_end(tokens, groups, last_params, params)?;
    if body.is_some_and(|body| {
        !matches!(
            blocks.kind(body),
            Some(BlockKind::FunctionBody | BlockKind::Unknown)
        )
    }) {
        return None;
    }
    Some(FunctionHead {
        start,
        name_start,
        name,
        params,
        body,
    })
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
            && (is_word(tokens, qualifier) || is_operator(tokens, qualifier, ">"))
        {
            start = qualifier;
            continue;
        }
        break;
    }
    start
}

/// First token of the specifiers and return type before `name_start`, or
/// `None` when the tokens before the name cannot be a return type (`x = f(`,
/// `return f(`, `a, f(`).
fn return_type_start(tokens: &[Token], groups: &Groups, name_start: usize) -> Option<usize> {
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
                let open = groups.get(id).open;
                start = match previous_head_token(tokens, open) {
                    Some(word) if matches!(&tokens[word], Token::Word(word) if is_attribute_word(word) || is_macro_word(word)) => {
                        word
                    }
                    _ if tokens[previous] == Token::Symbol(']') => open,
                    // A macro invocation on its own line before the head,
                    // such as `rb_gen(...)` without a semicolon.
                    _ if line_break_between(tokens, previous, start) => break,
                    _ => return None,
                };
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

/// The only code token inside the group `id`, when it is a word.
fn sole_word_in_group(tokens: &[Token], groups: &Groups, id: GroupId) -> Option<usize> {
    let group = groups.get(id);
    let word = next_code_token(tokens, group.open + 1)?;
    (is_word(tokens, word) && next_code_token(tokens, word + 1) == group.close).then_some(word)
}

fn line_break_between(tokens: &[Token], from: usize, to: usize) -> bool {
    tokens[from..to]
        .iter()
        .any(|token| matches!(token, Token::Newline))
}

/// Opening `<` of the template argument list closed at `close`.
fn template_arguments_start(tokens: &[Token], close: usize) -> Option<usize> {
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

/// Body of the head whose parameter list is `params` and whose declarator
/// ends with the paren group `last_params`: `Some(Some(body))` for a
/// definition, `Some(None)` for a declaration, and `None` when the tokens
/// after the declarator do not end a function head.
fn head_end(
    tokens: &[Token],
    groups: &Groups,
    last_params: GroupId,
    params: GroupId,
) -> Option<Option<GroupId>> {
    let group = groups.get(last_params);
    let level = group.parent;
    let mut index = group.close? + 1;
    let knr_parameters = is_identifier_list(tokens, groups, params);
    while let Some(next) = next_code_token(tokens, index) {
        if groups.enclosing(next) != level {
            return None;
        }
        match &tokens[next] {
            Token::Symbol('{') => return Some(groups.opened_at(next)),
            Token::Symbol(';') => {
                // K&R parameter declarations run up to the body.
                if knr_parameters && let Some(after) = next_code_token(tokens, next + 1) {
                    if is_word(tokens, after) {
                        index = next + 1;
                        continue;
                    }
                    if tokens[after] == Token::Symbol('{') {
                        return Some(groups.opened_at(after));
                    }
                }
                return Some(None);
            }
            Token::Symbol('}') => return None,
            Token::Symbol(',') if !knr_parameters => return Some(None),
            _ => {}
        }
        index = match groups.opened_at(next) {
            Some(id) => groups.get(id).close? + 1,
            None => next + 1,
        };
    }
    None
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
            Token::Word(word) if expect_word && !matches!(word.as_str(), "void") => {
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
                "int f(a, b)\nint a;\nchar *b;\n{\n}\nvoid g(void) __attribute__((noreturn));\nLUA_API int (lua_gettop) (lua_State *L) {\n}\nstatic void (*f(int k))(void) {\n}\n__attribute__((cold)) void h(void) {}\n"
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

    #[test]
    fn handles_cpp_heads() {
        assert_eq!(
            heads(
                "namespace n {\nclass C {\npublic:\n    C(int x);\n    ~C();\n    virtual int get() const = 0;\n    int (*handler)(int);\n    run(int);\n};\nstd::vector<int> C::values() const {\n}\n}\nT operator+(T a, T b) {\n}\nbool Cls::operator==(int a);\nvoid *Type::operator new(unsigned long size) {\n}\n"
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
            ]
        );
    }
}
