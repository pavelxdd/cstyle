//! What each brace group is: a function body, a control block, a type
//! body, an initializer, and so on.

use crate::formatter::lexer::Token;
use crate::formatter::structure::groups::{Delimiter, GroupId, Groups};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BlockKind {
    /// Body of a function definition at declaration scope.
    FunctionBody,
    /// Body of `if`/`for`/`while`/`switch`/`catch`, of `else`/`do`/`try`,
    /// or of a statement-like macro such as `FOREACH(x) {`.
    Control,
    /// Body of a `struct`, `union`, `enum`, or `class`.
    Aggregate,
    /// `= { ... }`, `return { ... }`, brace initialization, and braces
    /// nested in them.
    Initializer,
    /// `(Type){ ... }`.
    CompoundLiteral,
    /// Body of a C++ lambda.
    Lambda,
    Namespace,
    /// `extern "C" { ... }`.
    ExternC,
    /// A nested compound statement: `{` that starts a statement.
    Block,
    Unknown,
}

impl BlockKind {
    /// Whether declarations directly inside a block of this kind (or at file
    /// scope) are declarations rather than statements.
    pub(crate) fn is_declaration_scope(kind: Option<Self>) -> bool {
        matches!(
            kind,
            None | Some(Self::Namespace | Self::ExternC | Self::Aggregate)
        )
    }
}

/// Kind of every brace group, indexed by group.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct Blocks {
    kinds: Vec<Option<BlockKind>>,
}

impl Blocks {
    pub(crate) fn build(tokens: &[Token], groups: &Groups) -> Self {
        let mut blocks = Self {
            kinds: vec![None; groups.len()],
        };
        // Group ids follow their opening tokens, so parents are classified
        // before their children.
        for id in groups.ids() {
            if groups.get(id).delimiter == Delimiter::Brace {
                let kind = classify(tokens, groups, &blocks, id);
                blocks.kinds[id.index()] = Some(kind);
            }
        }
        blocks
    }

    /// Marks the bodies of function heads found after block classification,
    /// such as K&R definitions whose body follows parameter declarations.
    pub(crate) fn mark_function_bodies(&mut self, bodies: impl IntoIterator<Item = GroupId>) {
        for body in bodies {
            if let Some(kind @ Some(BlockKind::Unknown)) = self.kinds.get_mut(body.index()) {
                *kind = Some(BlockKind::FunctionBody);
            }
        }
    }

    /// Kind of the brace group `id`; `None` for parentheses and brackets.
    pub(crate) fn kind(&self, id: GroupId) -> Option<BlockKind> {
        self.kinds.get(id.index()).copied().flatten()
    }
}

/// Whether a token carries code, as opposed to layout, comments, and
/// directives.
pub(crate) fn is_code_token(token: &Token) -> bool {
    !matches!(
        token,
        Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) | Token::Preprocessor(_)
    )
}

pub(crate) fn previous_code_token(tokens: &[Token], before: usize) -> Option<usize> {
    (0..before)
        .rev()
        .find(|&index| is_code_token(&tokens[index]))
}

pub(crate) fn next_code_token(tokens: &[Token], start: usize) -> Option<usize> {
    (start..tokens.len()).find(|&index| is_code_token(&tokens[index]))
}

fn word_at(tokens: &[Token], index: usize) -> Option<&str> {
    match tokens.get(index) {
        Some(Token::Word(word)) => Some(word),
        _ => None,
    }
}

fn is_control_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "for" | "while" | "switch" | "catch" | "foreach" | "__except" | "synchronized"
    )
}

fn classify(tokens: &[Token], groups: &Groups, blocks: &Blocks, id: GroupId) -> BlockKind {
    let group = groups.get(id);
    let parent = group.parent;
    let parent_block = parent.and_then(|parent| blocks.kind(parent));
    if matches!(
        parent_block,
        Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
    ) {
        return BlockKind::Initializer;
    }
    let Some(previous) = previous_code_token(tokens, group.open) else {
        return BlockKind::Block;
    };
    let head = statement_head(tokens, groups, group.open);
    if let Some(close) = function_suffix_start(tokens, groups, &head)
        && let Some(paren) = groups.closed_at(close)
    {
        return classify_after_paren(tokens, groups, paren, parent_block);
    }
    match &tokens[previous] {
        Token::Operator(operator) if operator == "=" => return BlockKind::Initializer,
        Token::Word(word) if word == "return" => return BlockKind::Initializer,
        Token::Word(word) if matches!(word.as_str(), "else" | "do" | "try" | "__try") => {
            return BlockKind::Control;
        }
        Token::Symbol(')') => {
            if let Some(paren) = groups.closed_at(previous) {
                return classify_after_paren(tokens, groups, paren, parent_block);
            }
        }
        _ => {}
    }
    if let Some(kind) = classify_head(tokens, &head) {
        return kind;
    }
    if parent.is_some_and(|parent| groups.get(parent).delimiter != Delimiter::Brace) {
        return BlockKind::Initializer;
    }
    match &tokens[previous] {
        Token::Symbol(';' | '{' | '}') | Token::Operator(_) if !head_has_code(&head) => {
            if BlockKind::is_declaration_scope(parent_block) {
                BlockKind::Unknown
            } else {
                BlockKind::Block
            }
        }
        Token::Symbol(':') => BlockKind::Block,
        Token::Word(_) | Token::Symbol(']' | '>')
            if closing_is_followed_by_terminator(tokens, group.close) =>
        {
            BlockKind::Initializer
        }
        // A statement-like macro such as `SEH_TRY {`.
        Token::Word(_) if !BlockKind::is_declaration_scope(parent_block) => BlockKind::Control,
        _ => BlockKind::Unknown,
    }
}

fn classify_after_paren(
    tokens: &[Token],
    groups: &Groups,
    paren: GroupId,
    parent_block: Option<BlockKind>,
) -> BlockKind {
    let open = groups.get(paren).open;
    let Some(before) = previous_code_token(tokens, open) else {
        return BlockKind::CompoundLiteral;
    };
    match &tokens[before] {
        Token::Word(word) if is_control_keyword(word) => BlockKind::Control,
        Token::Word(_) | Token::Symbol('>') | Token::Operator(_)
            if BlockKind::is_declaration_scope(parent_block)
                && !matches!(&tokens[before], Token::Operator(operator) if operator == "=") =>
        {
            BlockKind::FunctionBody
        }
        Token::Word(_) => BlockKind::Control,
        Token::Symbol(']') => BlockKind::Lambda,
        Token::Symbol(')') if groups.closed_at(before).is_some() => {
            // `f(a)(b) {` is a function returning a function pointer, or a
            // macro-generated head; `(*f)(a) {` the same.
            if BlockKind::is_declaration_scope(parent_block) {
                BlockKind::FunctionBody
            } else {
                BlockKind::Control
            }
        }
        _ => BlockKind::CompoundLiteral,
    }
}

/// Code tokens of the statement that a brace at `open` ends, from the
/// previous statement boundary at the same nesting level.
fn statement_head(tokens: &[Token], groups: &Groups, open: usize) -> Vec<usize> {
    let level = groups.enclosing(open);
    let mut head = Vec::new();
    let mut index = open;
    while let Some(previous) = previous_code_token(tokens, index) {
        if groups.enclosing(previous) != level
            || matches!(tokens[previous], Token::Symbol(';' | '{' | '}'))
        {
            break;
        }
        if let Some(id) = groups.closed_at(previous) {
            // A nested group stands in the head as its opening delimiter.
            index = groups.get(id).open;
            head.push(index);
            continue;
        }
        if matches!(tokens[previous], Token::Symbol(';' | '{' | '}')) {
            break;
        }
        head.push(previous);
        index = previous;
    }
    head.reverse();
    head
}

/// The `)` of a parameter list followed only by function suffixes such as
/// `const`, `noexcept(...)`, `override`, ref-qualifiers, attributes, or a
/// trailing return type, at the end of `head`.
fn function_suffix_start(tokens: &[Token], groups: &Groups, head: &[usize]) -> Option<usize> {
    let mut end = head.len();
    if let Some(arrow) = head
        .iter()
        .rposition(|&index| matches!(&tokens[index], Token::Operator(operator) if operator == "->"))
    {
        end = arrow;
    }
    let mut suffix_seen = end < head.len();
    while end > 0 {
        let index = head[end - 1];
        match &tokens[index] {
            Token::Word(word) if is_function_suffix_word(word) => {}
            Token::Operator(operator) if matches!(operator.as_str(), "&" | "&&") => {}
            // A head keeps nested groups as their opening delimiter.
            Token::Symbol('(')
                if end >= 2
                    && word_at(tokens, head[end - 2]).is_some_and(|word| {
                        matches!(word, "noexcept" | "throw" | "__attribute__" | "__declspec")
                    }) =>
            {
                end -= 1;
            }
            Token::Symbol('(') if suffix_seen => {
                return groups.opened_at(index).and_then(|id| groups.get(id).close);
            }
            _ => return None,
        }
        suffix_seen = true;
        end -= 1;
    }
    None
}

fn is_function_suffix_word(word: &str) -> bool {
    matches!(
        word,
        "const" | "volatile" | "noexcept" | "override" | "final" | "mutable" | "throw"
    )
}

fn head_has_code(head: &[usize]) -> bool {
    !head.is_empty()
}

fn classify_head(tokens: &[Token], head: &[usize]) -> Option<BlockKind> {
    for (position, &index) in head.iter().enumerate() {
        match word_at(tokens, index) {
            Some("namespace") => return Some(BlockKind::Namespace),
            Some("extern")
                if head
                    .get(position + 1)
                    .is_some_and(|&next| matches!(tokens[next], Token::StringLiteral(_))) =>
            {
                return Some(BlockKind::ExternC);
            }
            Some("struct" | "union" | "enum" | "class" | "interface") => {
                return Some(BlockKind::Aggregate);
            }
            _ => {}
        }
    }
    None
}

fn closing_is_followed_by_terminator(tokens: &[Token], close: Option<usize>) -> bool {
    close
        .and_then(|close| next_code_token(tokens, close + 1))
        .is_some_and(|next| matches!(tokens[next], Token::Symbol(';' | ',' | ')')))
}

#[cfg(test)]
mod tests {
    use super::{BlockKind, Blocks};
    use crate::formatter::lexer::{Token, tokenize};
    use crate::formatter::structure::groups::Groups;

    /// Kinds of all brace groups in source order.
    fn kinds(source: &str) -> Vec<BlockKind> {
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let blocks = Blocks::build(&tokens, &groups);
        (0..tokens.len())
            .filter(|&index| matches!(tokens[index], Token::Symbol('{')))
            .filter_map(|index| groups.opened_at(index))
            .map(|id| blocks.kind(id).expect("brace group"))
            .collect()
    }

    use BlockKind::*;

    #[test]
    fn classifies_function_bodies_and_control_blocks() {
        assert_eq!(
            kinds(
                "static int\nf(int a)\n{\n    if (a) {\n    } else {\n    }\n    do {\n    } while (0);\n    {\n    }\n}\n"
            ),
            [FunctionBody, Control, Control, Control, Block]
        );
        assert_eq!(
            kinds(
                "void g(void) {\n    FOREACH(x) {\n    }\n    switch (a) {\n    case 1: {\n    }\n    }\n}\n"
            ),
            [FunctionBody, Control, Control, Block]
        );
    }

    #[test]
    fn classifies_types_and_initializers() {
        assert_eq!(
            kinds(
                "typedef struct a {\n    int d;\n} a;\nstatic struct p b = {\n    .m = { 1 },\n    { 2 },\n};\n"
            ),
            [Aggregate, Initializer, Initializer, Initializer]
        );
        assert_eq!(
            kinds(
                "void f(void) {\n    x = (struct p){ 1 };\n    g((int[]){ 1, 2 });\n    return;\n}\n"
            ),
            [FunctionBody, CompoundLiteral, CompoundLiteral]
        );
        assert_eq!(
            kinds("enum e : int { A };\nstruct s *make(void) {\n}\n"),
            [Aggregate, FunctionBody]
        );
    }

    #[test]
    fn classifies_scopes_and_lambdas() {
        assert_eq!(
            kinds(
                "#ifdef __cplusplus\nextern \"C\" {\n#endif\nnamespace n {\nclass C : public B {\n    int f() const {\n    }\n};\n}\n"
            ),
            [ExternC, Namespace, Aggregate, FunctionBody]
        );
        assert_eq!(
            kinds(
                "void f() {\n    auto g = [&](int x) {\n    };\n    std::vector<int> v{1, 2};\n}\n"
            ),
            [FunctionBody, Lambda, Initializer]
        );
    }
}
