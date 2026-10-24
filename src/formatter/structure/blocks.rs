//! What each brace group is: a function body, a control block, a type
//! body, an initializer, and so on.

use crate::formatter::lexer::Token;
use crate::formatter::structure::groups::{Delimiter, GroupId, Groups};
use crate::formatter::syntax::language::{is_assignment_operator, is_non_type_keyword};

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
    /// GNU `({ ... })`: statements whose last value is the expression's.
    StatementExpression,
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
    /// First token of the statement each brace group belongs to, such as
    /// `typedef` in `typedef struct a {`.
    owners: Vec<Option<usize>>,
}

impl Blocks {
    pub(crate) fn build(tokens: &[Token], groups: &Groups) -> Self {
        Self::build_within(tokens, groups, None)
    }

    /// Classifies the brace groups of `tokens`, a brace group known to be of
    /// `root` kind standing first among them when `root` is given, as when
    /// the tokens are a block taken out of its context.
    pub(crate) fn build_within(
        tokens: &[Token],
        groups: &Groups,
        mut root: Option<BlockKind>,
    ) -> Self {
        let mut blocks = Self {
            kinds: vec![None; groups.len()],
            owners: vec![None; groups.len()],
        };
        // Group ids follow their opening tokens, so parents are classified
        // before their children.
        for id in groups.ids() {
            let group = groups.get(id);
            if group.delimiter == Delimiter::Brace {
                let kind = match root.take_if(|_| group.parent.is_none()) {
                    Some(kind) => kind,
                    None => classify(tokens, groups, &blocks, id),
                };
                blocks.kinds[id.index()] = Some(kind);
                let head = statement_head(tokens, groups, group.open);
                // The statement after a label owns its block; a block right
                // after a case label is the case's own.
                let first_word = head.first().and_then(|&first| word_at(tokens, first));
                let case_label = matches!(first_word, Some("case" | "default"));
                let label = first_word.is_some()
                    && head.get(1).is_some_and(|&colon| {
                        matches!(tokens[colon], Token::Symbol(':'))
                            && !head
                                .get(2)
                                .is_some_and(|&next| matches!(tokens[next], Token::Symbol(':')))
                    });
                let mut owner = head
                    .iter()
                    .position(|&index| matches!(tokens[index], Token::Symbol(':')))
                    .filter(|_| case_label || label)
                    .map(|colon| colon + 1);
                // Further case labels before the statement belong to it too.
                while let Some(position) = owner
                    && head
                        .get(position)
                        .and_then(|&index| word_at(tokens, index))
                        .is_some_and(|word| matches!(word, "case" | "default"))
                    && let Some(colon) = (position..head.len())
                        .find(|&next| matches!(tokens[head[next]], Token::Symbol(':')))
                    && colon + 1 < head.len()
                {
                    owner = Some(colon + 1);
                }
                let owner = owner
                    .and_then(|position| head.get(position))
                    .or(head.first());
                blocks.owners[id.index()] = Some(owner.copied().unwrap_or(group.open));
            }
        }
        blocks
    }

    /// First token of the statement the brace group `id` belongs to.
    pub(crate) fn owner(&self, id: GroupId) -> Option<usize> {
        self.owners.get(id.index()).copied().flatten()
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

/// Whether a token carries code, as opposed to layout (blanks and line
/// splices), comments, and directives.
pub(crate) fn is_code_token(token: &Token) -> bool {
    !matches!(
        token,
        Token::Whitespace(_)
            | Token::Newline
            | Token::Comment(_, _)
            | Token::Preprocessor(_)
            | Token::Symbol('\\')
    )
}

pub(crate) fn previous_code_token(tokens: &[Token], before: usize) -> Option<usize> {
    // Tokens a rewrite made have no index in the source.
    if before > tokens.len() {
        return None;
    }
    (0..before)
        .rev()
        .find(|&index| is_code_token(&tokens[index]))
}

pub(crate) fn next_code_token(tokens: &[Token], start: usize) -> Option<usize> {
    (start..tokens.len()).find(|&index| is_code_token(&tokens[index]))
}

/// A look forward for the first token from a start that a test admits,
/// kept to read on from: a start among the tokens it passed has the same
/// answer, and one before them reads on only to them. Each look holds to
/// one test over one token sequence.
#[derive(Default)]
pub(crate) struct TokenLook(std::cell::Cell<Option<(usize, Option<usize>)>>);

impl TokenLook {
    pub(crate) fn find(
        &self,
        tokens: &[Token],
        start: usize,
        matches: impl Fn(&Token) -> bool,
    ) -> Option<usize> {
        let found = match self.0.get() {
            Some((from, found)) if from <= start && start <= found.unwrap_or(tokens.len()) => {
                return found;
            }
            Some((from, found)) if start < from => (start..from)
                .find(|&index| matches(&tokens[index]))
                .or(found),
            _ => (start..tokens.len()).find(|&index| matches(&tokens[index])),
        };
        self.0.set(Some((start, found)));
        found
    }
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
    let previous = previous_code_token(tokens, group.open);
    if previous.is_some_and(|previous| opens_block_literal(tokens, groups, previous)) {
        return BlockKind::Lambda;
    }
    if matches!(
        parent_block,
        Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
    ) {
        // A lambda among a list's elements opens its own body.
        let head = statement_head(tokens, groups, group.open);
        return if previous.is_some() && has_lambda_capture(tokens, groups, &head) {
            BlockKind::Lambda
        } else {
            BlockKind::Initializer
        };
    }
    let Some(previous) = previous else {
        return BlockKind::Block;
    };
    // The body after a member's braced initializer ending a constructor's
    // initializers is the constructor's.
    if BlockKind::is_declaration_scope(parent_block)
        && let Some(value) = groups.closed_at(previous)
        && braces_member_initializer(tokens, groups, groups.get(value).open)
    {
        return BlockKind::FunctionBody;
    }
    let head = statement_head(tokens, groups, group.open);
    // Inside parentheses or brackets a brace belongs to an expression.
    if parent.is_some_and(|parent| groups.get(parent).delimiter != Delimiter::Brace) {
        return classify_in_expression(tokens, groups, &head, previous);
    }
    if let Some(close) = function_suffix_start(tokens, groups, &head)
        && let Some(paren) = groups.closed_at(close)
    {
        return classify_after_paren(tokens, groups, paren, parent_block);
    }
    if ends_with_value_capture(tokens, groups, &head) {
        return BlockKind::Lambda;
    }
    match &tokens[previous] {
        Token::Operator(operator) if operator == "=" => return BlockKind::Initializer,
        Token::Word(word) if word == "return" => return BlockKind::Initializer,
        Token::Word(word)
            if matches!(
                word.as_str(),
                "else" | "do" | "try" | "__try" | "defer" | "_Defer"
            ) =>
        {
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
    if braces_member_initializer(tokens, groups, group.open) {
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
        // A template type's value, as `std::vector<int>{ 1, 2 };`.
        Token::Operator(operator)
            if operator == ">"
                && (closing_is_followed_by_terminator(tokens, group.close)
                    || braces_value_in_expression(tokens, &head, group.close)) =>
        {
            BlockKind::Initializer
        }
        // A type's braces within an expression hold its value.
        Token::Word(_) if braces_value_in_expression(tokens, &head, group.close) => {
            BlockKind::Initializer
        }
        // A statement-like macro such as `SEH_TRY {`.
        Token::Word(_) if !BlockKind::is_declaration_scope(parent_block) => BlockKind::Control,
        _ => BlockKind::Unknown,
    }
}

/// Whether a brace after the token at `previous` opens a block literal's
/// body: `^{`, `^(int value) {` or `^Type *(int value) {`.
fn opens_block_literal(tokens: &[Token], groups: &Groups, previous: usize) -> bool {
    let mut index = previous;
    if matches!(tokens[index], Token::Symbol(')')) {
        let Some(paren) = groups.closed_at(index) else {
            return false;
        };
        let Some(before) = previous_code_token(tokens, groups.get(paren).open) else {
            return false;
        };
        index = before;
        while matches!(&tokens[index], Token::Word(_))
            || matches!(&tokens[index], Token::Operator(operator) if matches!(operator.as_str(), "*" | "&"))
        {
            let Some(before) = previous_code_token(tokens, index) else {
                return false;
            };
            index = before;
        }
    }
    if !matches!(&tokens[index], Token::Operator(operator) if operator == "^") {
        return false;
    }
    previous_code_token(tokens, index).is_none_or(|before| {
        matches!(
            &tokens[before],
            Token::Symbol('(' | ',' | '[' | ':' | '{' | ';' | '}')
        ) || matches!(&tokens[before], Token::Operator(operator) if operator == "=")
            || matches!(&tokens[before], Token::Word(word) if word == "return")
    })
}

fn is_lambda_specifier(token: &Token) -> bool {
    matches!(token, Token::Word(word) if matches!(
        word.as_str(),
        "mutable" | "constexpr" | "consteval" | "static" | "noexcept"
    ))
}

/// Whether `head` ends with a capture list, and its specifiers, where a value
/// begins: leading the head, or after an operator or a `return`. An array
/// declarator's bounds follow its name, an attribute's list holds a list.
fn ends_with_value_capture(tokens: &[Token], groups: &Groups, head: &[usize]) -> bool {
    let Some(capture) = head
        .iter()
        .rposition(|&index| matches!(tokens[index], Token::Symbol('[')))
    else {
        return false;
    };
    let open = head[capture];
    head[capture + 1..]
        .iter()
        .all(|&index| is_lambda_specifier(&tokens[index]))
        && groups
            .opened_at(open)
            .and_then(|id| groups.get(id).close)
            .is_some()
        && next_code_token(tokens, open + 1)
            .is_some_and(|inner| !matches!(tokens[inner], Token::Symbol('[')))
        && capture
            .checked_sub(1)
            .is_none_or(|before| match &tokens[head[before]] {
                Token::Operator(_) => true,
                Token::Word(word) => {
                    matches!(word.as_str(), "return" | "co_return" | "co_yield" | "throw")
                }
                _ => false,
            })
}

/// Whether `head` ends with a lambda's capture list, or a capture list and
/// its parameters.
fn has_lambda_capture(tokens: &[Token], groups: &Groups, head: &[usize]) -> bool {
    let specifier = |index: usize| is_lambda_specifier(&tokens[index]);
    head.iter().enumerate().any(|(position, &index)| {
        matches!(tokens[index], Token::Symbol('['))
            && !names_operator(tokens, index)
            && groups
                .opened_at(index)
                .and_then(|id| groups.get(id).close)
                .is_some()
            && after_template_parameters(tokens, &head[position + 1..])
                .first()
                .is_some_and(|&next| matches!(tokens[next], Token::Symbol('(')))
    }) || head
        .iter()
        .rposition(|&index| matches!(tokens[index], Token::Symbol('[')))
        .is_some_and(|capture| head[capture + 1..].iter().all(|&index| specifier(index)))
}

/// Whether the bracket at `open` names an operator, as `operator[]` or
/// `operator new[]` do.
fn names_operator(tokens: &[Token], open: usize) -> bool {
    let word = |index: Option<usize>, words: &[&str]| {
        index.is_some_and(
            |index| matches!(&tokens[index], Token::Word(word) if words.contains(&word.as_str())),
        )
    };
    let before = previous_code_token(tokens, open);
    word(before, &["operator"])
        || word(before, &["new", "delete"])
            && word(
                before.and_then(|before| previous_code_token(tokens, before)),
                &["operator"],
            )
}

/// The code token before the template parameter list that the `>` at
/// `close` ends.
fn before_template_parameters(tokens: &[Token], close: usize) -> Option<usize> {
    if !matches!(&tokens[close], Token::Operator(operator) if operator == ">") {
        return None;
    }
    let mut depth = 0usize;
    let mut index = close;
    loop {
        if let Token::Operator(operator) = &tokens[index] {
            match operator.as_str() {
                ">" => depth += 1,
                ">>" => depth += 2,
                "<" => depth = depth.saturating_sub(1),
                _ => {}
            }
            if depth == 0 {
                return previous_code_token(tokens, index);
            }
        }
        if matches!(tokens[index], Token::Symbol(';' | '{' | '}')) {
            return None;
        }
        index = previous_code_token(tokens, index)?;
    }
}

/// `head` past a template parameter list leading it, as a lambda's
/// `<typename T>` after its capture list.
fn after_template_parameters<'a>(tokens: &[Token], head: &'a [usize]) -> &'a [usize] {
    if !head.first().is_some_and(
        |&first| matches!(&tokens[first], Token::Operator(operator) if operator == "<"),
    ) {
        return head;
    }
    let mut depth = 0usize;
    for (position, &index) in head.iter().enumerate() {
        if let Token::Operator(operator) = &tokens[index] {
            match operator.as_str() {
                "<" => depth += 1,
                ">" => depth = depth.saturating_sub(1),
                ">>" => depth = depth.saturating_sub(2),
                _ => {}
            }
            if depth == 0 {
                return &head[position + 1..];
            }
        }
    }
    head
}

/// A brace inside an expression: a lambda body after a capture list, a
/// compound literal after a cast, or an initializer.
fn classify_in_expression(
    tokens: &[Token],
    groups: &Groups,
    head: &[usize],
    previous: usize,
) -> BlockKind {
    if has_lambda_capture(tokens, groups, head) {
        return BlockKind::Lambda;
    }
    match &tokens[previous] {
        Token::Symbol(')') => BlockKind::CompoundLiteral,
        Token::Symbol('(') if holds_statement(tokens, groups, previous) => {
            BlockKind::StatementExpression
        }
        _ => BlockKind::Initializer,
    }
}

/// Whether the brace right after the `(` at `paren`, on its line, ends a
/// statement with `;` at its own level: a brace on the next line is a
/// macro's block argument.
fn holds_statement(tokens: &[Token], groups: &Groups, paren: usize) -> bool {
    let Some(open) = next_code_token(tokens, paren + 1) else {
        return false;
    };
    if tokens[paren + 1..open]
        .iter()
        .any(|token| !matches!(token, Token::Whitespace(_)))
    {
        return false;
    }
    let Some(close) = groups.opened_at(open).and_then(|id| groups.get(id).close) else {
        return false;
    };
    let brace = groups.opened_at(open);
    (open + 1..close).any(|index| {
        matches!(tokens[index], Token::Symbol(';')) && groups.enclosing(index) == brace
    })
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
    // A lambda's template parameters stand between its captures and its
    // parameters, as in `[]<typename T>(T value) {`.
    if before_template_parameters(tokens, before)
        .is_some_and(|index| matches!(tokens[index], Token::Symbol(']')))
    {
        return BlockKind::Lambda;
    }
    match &tokens[before] {
        Token::Word(word) if is_control_keyword(word) && !follows_type(tokens, before) => {
            BlockKind::Control
        }
        Token::Word(word) if is_value_keyword(word) => BlockKind::CompoundLiteral,
        // The `=` of `operator=` names the function; any other assigns a
        // literal.
        Token::Word(_) | Token::Symbol('>') | Token::Operator(_)
            if BlockKind::is_declaration_scope(parent_block)
                && (!matches!(&tokens[before], Token::Operator(operator) if operator == "=")
                    || previous_code_token(tokens, before).is_some_and(
                        |name| matches!(&tokens[name], Token::Word(word) if word == "operator"),
                    )) =>
        {
            BlockKind::FunctionBody
        }
        Token::Symbol(']')
            if BlockKind::is_declaration_scope(parent_block)
                && groups
                    .closed_at(before)
                    .is_some_and(|brackets| names_operator(tokens, groups.get(brackets).open)) =>
        {
            BlockKind::FunctionBody
        }
        Token::Word(_) => BlockKind::Control,
        Token::Symbol(']') => BlockKind::Lambda,
        Token::Symbol(')') if groups.closed_at(before).is_some() => {
            // `f(a)(b) {` is a function returning a function pointer, or a
            // macro-generated head; `(*f)(a) {` the same. A cast cast again,
            // `(int *)(int[]) {`, opens a compound literal.
            let inner = groups.closed_at(before).map(|id| groups.get(id).open);
            if inner.is_some_and(|open| {
                previous_code_token(tokens, open).is_none_or(|index| match &tokens[index] {
                    Token::Word(word) => word == "return",
                    Token::Symbol(symbol) => !matches!(symbol, ')' | ']'),
                    // After `*`, `&` or `^` the parentheses may be a
                    // declarator's.
                    Token::Operator(operator) => !matches!(operator.as_str(), "*" | "&" | "^"),
                    _ => false,
                })
            }) {
                BlockKind::CompoundLiteral
            } else if BlockKind::is_declaration_scope(parent_block) {
                BlockKind::FunctionBody
            } else {
                BlockKind::Control
            }
        }
        _ => BlockKind::CompoundLiteral,
    }
}

/// Whether `word` takes an expression after it, so that `word (T) {` is a
/// cast to a compound literal rather than a statement-like macro head.
fn is_value_keyword(word: &str) -> bool {
    matches!(
        word,
        "return" | "throw" | "co_return" | "co_yield" | "co_await"
    )
}

/// Whether the word at `index` follows a type, as a function named like a
/// control keyword does (`static int foreach (lua_State *L)`).
fn follows_type(tokens: &[Token], index: usize) -> bool {
    previous_code_token(tokens, index).is_some_and(|previous| match &tokens[previous] {
        Token::Word(word) => !matches!(word.as_str(), "else" | "do"),
        Token::Operator(operator) => matches!(operator.as_str(), "*" | "&" | "&&"),
        _ => false,
    })
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
        // A control header leading its line after a macro word standing
        // without a semicolon starts a statement of its own.
        if head.last().is_some_and(|&first| {
            word_at(tokens, first).is_some_and(is_control_keyword) && begins_line(tokens, first)
        }) && word_at(tokens, previous).is_some_and(|word| !is_non_type_keyword(word))
        {
            break;
        }
        head.push(previous);
        index = previous;
    }
    head.reverse();
    head
}

/// Whether only whitespace and comments stand before `index` on its source
/// line.
fn begins_line(tokens: &[Token], index: usize) -> bool {
    tokens[..index]
        .iter()
        .rev()
        .take_while(|token| !matches!(token, Token::Newline))
        .all(|token| matches!(token, Token::Whitespace(_) | Token::Comment(..)))
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

/// Whether the brace opened at `open` holds the value of a member that a
/// constructor's initializers start with or reach after a `,`.
fn braces_member_initializer(tokens: &[Token], groups: &Groups, open: usize) -> bool {
    let Some(member) = previous_code_token(tokens, open) else {
        return false;
    };
    if !matches!(tokens[member], Token::Word(_)) {
        return false;
    }
    let level = groups.enclosing(open);
    let mut index = member;
    while let Some(before) = previous_code_token(tokens, index) {
        if groups.enclosing(before) != level {
            return false;
        }
        if let Some(group) = groups.closed_at(before) {
            let value = groups.get(group).open;
            // Only another member's braced value stands among the members.
            if groups.get(group).delimiter == Delimiter::Brace
                && !braces_member_initializer(tokens, groups, value)
            {
                return false;
            }
            index = value;
            continue;
        }
        match &tokens[before] {
            Token::Symbol(':') => {
                return previous_code_token(tokens, before).is_some_and(|params| {
                    matches!(tokens[params], Token::Symbol(')'))
                        && groups.closed_at(params).is_some()
                        || word_at(tokens, params).is_some_and(is_function_suffix_word)
                });
            }
            Token::Symbol(';' | '{' | '}') => return false,
            _ => index = before,
        }
    }
    false
}

/// Whether a brace after a word holds a value within an expression: its
/// statement returns or throws one, or assigns or chooses it, or the value
/// is used past its closing brace.
fn braces_value_in_expression(tokens: &[Token], head: &[usize], close: Option<usize>) -> bool {
    head.first()
        .and_then(|&first| word_at(tokens, first))
        .is_some_and(|word| matches!(word, "return" | "throw" | "co_return" | "co_yield"))
        || head.iter().any(|&index| match &tokens[index] {
            Token::Operator(operator) => is_assignment_operator(operator),
            token => matches!(token, Token::Symbol('?')),
        })
        || close.is_some_and(|close| {
            next_code_token(tokens, close + 1).is_some_and(|next| match &tokens[next] {
                Token::Operator(operator) => matches!(operator.as_str(), "." | "->" | "?"),
                // A call or subscript on the closer's line applies to the
                // value; a line after a block may open with a cast.
                Token::Symbol('(' | '[') => !tokens[close + 1..next]
                    .iter()
                    .any(|token| matches!(token, Token::Newline)),
                token => matches!(token, Token::Symbol('.' | '?')),
            })
        })
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
    fn classifies_braces_inside_call_arguments_as_expressions() {
        assert_eq!(
            kinds(
                "void f(void)\n{\n    call(child, &(const T) {\n        x, y\n    }, [](int v) {\n        return v;\n    }, (int[]){1});\n}\n"
            ),
            [FunctionBody, CompoundLiteral, Lambda, CompoundLiteral]
        );
    }

    #[test]
    fn classifies_an_assignment_operator_s_body_as_a_function_body() {
        assert_eq!(
            kinds(
                "void run() {\n  struct Item {\n    auto operator=(char) -> Item& {\n      return *this;\n    }\n    Item& operator=(int) {\n      return *this;\n    }\n  };\n}\n"
            ),
            [FunctionBody, Aggregate, FunctionBody, FunctionBody]
        );
    }

    #[test]
    fn classifies_a_type_s_braces_before_a_call_or_member_as_its_value() {
        assert_eq!(
            kinds(
                "void f()\n{\n    Item{}(v);\n    Item{1}.run();\n    Item{}->go();\n    Item{}[0];\n    std::hash<T>{}(v);\n    SEH_TRY {\n    }\n    (void)value;\n}\n"
            ),
            [
                FunctionBody,
                Initializer,
                Initializer,
                Initializer,
                Initializer,
                Initializer,
                Control
            ]
        );
    }

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
        assert_eq!(
            kinds("static int foreach (lua_State *L)\n{\n    foreach (x) {\n    }\n}\n"),
            [FunctionBody, Control]
        );
    }

    #[test]
    fn classifies_a_type_s_braces_within_an_expression_as_its_value() {
        assert_eq!(
            kinds(
                "void f() {\n    x = c ? Item{1} : g;\n    x = Item{1} + 1;\n    return Item{1}.value;\n    SEH_TRY {\n    }\n}\n"
            ),
            [FunctionBody, Initializer, Initializer, Initializer, Control]
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
        assert_eq!(
            kinds(
                "void f() {\n    auto g = [&] {\n    };\n    return [value] mutable {\n    };\n    int values[2] {1, 2};\n}\n"
            ),
            [FunctionBody, Lambda, Lambda, Initializer]
        );
        assert_eq!(
            kinds(
                "void f() {\n    auto add = []<typename T>(T a, T b) {\n    };\n    call([]<class T>(T a) {\n    });\n}\n"
            ),
            [FunctionBody, Lambda, Lambda]
        );
    }

    #[test]
    fn classifies_a_subscript_operator_body_as_a_function_body() {
        assert_eq!(
            kinds(
                "class C {\n    int& operator[](int pos) const {\n    }\n    void* operator new[](size_t size) {\n    }\n};\n"
            ),
            [Aggregate, FunctionBody, FunctionBody]
        );
    }
}
