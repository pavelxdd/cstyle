//! Statements: which `if` each `else` belongs to.

use super::blocks::{BlockKind, Blocks, is_code_token, next_code_token};
use super::groups::Groups;
use crate::formatter::lexer::Token;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::text::line_scan::preprocessor_directive;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct Statements {
    /// `if` token of each `else` token, by token index.
    else_ifs: HashMap<usize, usize>,
    /// Keyword of the control statement whose braceless body starts at a
    /// token, by the body's first token.
    braceless_headers: HashMap<usize, usize>,
    /// First token of the previous statement in the same block, by a
    /// statement's first token: labels are skipped, and a statement after
    /// one without its `;` (a macro call) has none.
    previous_siblings: HashMap<usize, usize>,
    /// The `{` of the block that a statement starts, by the statement's
    /// first token.
    block_openings: HashMap<usize, usize>,
    /// First tokens of the statements and labels in blocks.
    block_statements: HashSet<usize>,
    /// Bodies of `else` keywords separated from them by a blank line.
    split_else_bodies: Vec<ElseBody>,
}

/// The body of an `else`, as token indices.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ElseBody {
    /// The `else` keyword.
    keyword: usize,
    /// First code token of the body.
    start: usize,
    /// Index after the body.
    end: usize,
    /// Whether an empty line, not just a directive line, splits it off.
    after_blank_line: bool,
}

impl Statements {
    pub(crate) fn build(tokens: &[Token], groups: &Groups, blocks: &Blocks) -> Self {
        let mut parser = Parser {
            tokens,
            groups,
            blocks,
            else_ifs: HashMap::new(),
            braceless_headers: HashMap::new(),
            previous_siblings: HashMap::new(),
            block_openings: HashMap::new(),
            block_statements: HashSet::new(),
            else_bodies: Vec::new(),
            unterminated: false,
        };
        parser.file_items(tokens.len());
        Self {
            else_ifs: parser.else_ifs,
            braceless_headers: parser.braceless_headers,
            previous_siblings: parser.previous_siblings,
            block_openings: parser.block_openings,
            block_statements: parser.block_statements,
            split_else_bodies: parser
                .else_bodies
                .into_iter()
                .filter(|body| {
                    tokens[body.keyword..body.start]
                        .iter()
                        .filter(|token| matches!(token, Token::Newline))
                        .count()
                        > 1
                })
                .map(|body| ElseBody {
                    after_blank_line: tokens[body.keyword..body.start]
                        .iter()
                        .filter(|token| !matches!(token, Token::Whitespace(_)))
                        .collect::<Vec<_>>()
                        .windows(2)
                        .any(|pair| {
                            matches!(pair[0], Token::Newline) && matches!(pair[1], Token::Newline)
                        }),
                    ..body
                })
                .collect(),
        }
    }

    /// The `if` token that the `else` token at `index` belongs to.
    pub(crate) fn if_of_else(&self, index: usize) -> Option<usize> {
        self.else_ifs.get(&index).copied()
    }

    /// Keyword of the control statement whose braceless body starts at the
    /// token `index`.
    pub(crate) fn braceless_header(&self, index: usize) -> Option<usize> {
        self.braceless_headers.get(&index).copied()
    }

    /// First token of the statement before the one starting at `index`, in
    /// the same block.
    pub(crate) fn previous_sibling(&self, index: usize) -> Option<usize> {
        self.previous_siblings.get(&index).copied()
    }

    /// The `{` of the block whose first statement starts at `index`.
    pub(crate) fn block_opening(&self, index: usize) -> Option<usize> {
        self.block_openings.get(&index).copied()
    }

    /// Whether a statement or label in a block starts at the token `index`.
    pub(crate) fn starts_block_statement(&self, index: usize) -> bool {
        self.block_statements.contains(&index)
    }

    /// Whether the token `index` is in the body of an `else` that a blank
    /// line separates from its body.
    pub(crate) fn in_split_else_body(&self, index: usize) -> bool {
        self.split_else_bodies
            .iter()
            .any(|body| (body.start..body.end).contains(&index))
    }

    /// Whether the token `index` is in the body of an `else` that an empty
    /// line separates from its body; a directive line alone does not.
    pub(crate) fn in_else_body_after_blank_line(&self, index: usize) -> bool {
        self.split_else_bodies
            .iter()
            .any(|body| body.after_blank_line && (body.start..body.end).contains(&index))
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    groups: &'a Groups,
    blocks: &'a Blocks,
    else_ifs: HashMap<usize, usize>,
    braceless_headers: HashMap<usize, usize>,
    previous_siblings: HashMap<usize, usize>,
    block_openings: HashMap<usize, usize>,
    block_statements: HashSet<usize>,
    else_bodies: Vec<ElseBody>,
    /// Whether the last expression statement ended at a keyword, not `;`.
    unterminated: bool,
}

impl Parser<'_> {
    fn next(&self, from: usize, end: usize) -> Option<usize> {
        next_code_token(self.tokens, from).filter(|&index| index < end)
    }

    fn word(&self, index: usize) -> Option<&str> {
        match &self.tokens[index] {
            Token::Word(word) => Some(word),
            _ => None,
        }
    }

    fn is_symbol(&self, index: usize, symbol: char) -> bool {
        matches!(self.tokens[index], Token::Symbol(ch) if ch == symbol)
    }

    fn close_of(&self, open: usize) -> Option<usize> {
        let id = self.groups.opened_at(open)?;
        self.groups.get(id).close
    }

    /// Parses the statements in `start..end`.
    fn items(&mut self, start: usize, end: usize) {
        let mut position = start;
        // The last statement of the block that is no label and ended with
        // its `;` or block, as its first token and the index after it.
        let mut sibling: Option<(usize, usize)> = None;
        // The sibling before each open conditional group, and whether the
        // group has an alternative branch: each branch follows the code
        // before the group, and so does the code after a group with
        // alternatives; after a lone branch, its code comes last.
        let mut branch_siblings: Vec<(Option<(usize, usize)>, bool)> = Vec::new();
        let mut first = true;
        while let Some(at) = self.next(position, end) {
            for index in position..at {
                let Token::Preprocessor(directive) = &self.tokens[index] else {
                    continue;
                };
                match preprocessor_directive(&directive.text) {
                    Some("if" | "ifdef" | "ifndef") => branch_siblings.push((sibling, false)),
                    Some("else" | "elif" | "elifdef" | "elifndef") => {
                        if let Some((before, alternatives)) = branch_siblings.last_mut() {
                            *alternatives = true;
                            sibling = *before;
                        } else {
                            sibling = None;
                        }
                    }
                    Some("endif") => {
                        if let Some((before, alternatives)) = branch_siblings.pop()
                            && alternatives
                        {
                            sibling = before;
                        }
                    }
                    _ => {}
                }
            }
            // astyle lays out statements after a label afresh. A branch of
            // a conditional group may close a block that the tree matched
            // elsewhere; the code after that stray `}` starts afresh too.
            let fresh = self.is_label(at, end) || self.is_symbol(at, '}');
            self.block_statements.insert(at);
            if first
                && !fresh
                && let Some(open) = start.checked_sub(1)
                && self.is_symbol(open, '{')
                && !self.crosses_branch(open, at)
            {
                self.block_openings.insert(at, open);
            }
            first = false;
            // A statement that a directive splits parses one branch after
            // another; it anchors nothing.
            if !fresh
                && let Some((previous, previous_end)) = sibling
                && !self.crosses_branch(previous, previous_end)
            {
                self.previous_siblings.insert(at, previous);
            }
            self.unterminated = false;
            position = self.statement(at, end).max(at + 1);
            sibling = (!fresh && !self.unterminated).then_some((at, position));
        }
    }

    /// File scope: declarations, not statements of a block.
    fn file_items(&mut self, end: usize) {
        let mut position = 0;
        while let Some(at) = self.next(position, end) {
            position = self.statement(at, end).max(at + 1);
        }
    }

    /// Whether an alternative branch of a conditional group starts between
    /// the tokens `from` and `to`: code on both sides of it never meets.
    /// Branches holding only directives, such as alternative `#define`s,
    /// split no code.
    fn crosses_branch(&self, from: usize, to: usize) -> bool {
        let mut branch_has_code = false;
        for token in &self.tokens[from..to] {
            if let Token::Preprocessor(directive) = token {
                match preprocessor_directive(&directive.text) {
                    Some("else" | "elif" | "elifdef" | "elifndef") if branch_has_code => {
                        return true;
                    }
                    Some(name) if is_conditional_preprocessor(name) => branch_has_code = false,
                    _ => {}
                }
            } else if is_code_token(token) {
                branch_has_code = true;
            }
        }
        false
    }

    fn is_label(&self, at: usize, end: usize) -> bool {
        match self.word(at) {
            Some("case" | "default") => true,
            Some(_) => self
                .next(at + 1, end)
                .is_some_and(|next| self.is_symbol(next, ':')),
            None => false,
        }
    }

    /// Parses the statement starting at the code token `at`; returns the
    /// index after it.
    fn statement(&mut self, at: usize, end: usize) -> usize {
        match self.word(at) {
            Some("if") => return self.if_statement(at, end),
            Some("for" | "while" | "switch" | "foreach") => {
                return match self.after_parens(at + 1, end) {
                    Some(after) => self.body(at, after, end),
                    None => self.simple(at, end),
                };
            }
            Some("do") => return self.do_statement(at, end),
            Some("else" | "try" | "__try" | "__finally") => return self.body(at, at + 1, end),
            Some("case" | "default") => {
                if let Some(colon) = (at + 1..end).find(|&index| self.is_symbol(index, ':')) {
                    return colon + 1;
                }
            }
            Some(_) => {
                // A labeled statement: the label and the statement after it.
                if let Some(colon) = self.next(at + 1, end)
                    && self.is_symbol(colon, ':')
                {
                    return match self.next(colon + 1, end) {
                        Some(next) if !self.is_symbol(next, '}') => {
                            self.block_statements.insert(next);
                            self.statement(next, end)
                        }
                        _ => colon + 1,
                    };
                }
            }
            None => {}
        }
        if self.is_symbol(at, '{') {
            return match self.close_of(at) {
                Some(close) => {
                    self.items(at + 1, close);
                    self.unterminated = false;
                    close + 1
                }
                None => {
                    self.unterminated = true;
                    at + 1
                }
            };
        }
        if self.is_symbol(at, ';') {
            return at + 1;
        }
        self.simple(at, end)
    }

    /// Parses the body of the control statement whose keyword is `header`.
    fn body(&mut self, header: usize, from: usize, end: usize) -> usize {
        match self.next(from, end) {
            Some(at) => {
                if !self.is_symbol(at, '{') && !self.crosses_branch(header, at) {
                    self.braceless_headers.insert(at, header);
                }
                self.statement(at, end)
            }
            None => end,
        }
    }

    /// Index after the parenthesized group starting at the first code token
    /// from `from`.
    fn after_parens(&self, from: usize, end: usize) -> Option<usize> {
        let open = self.next(from, end)?;
        if !self.is_symbol(open, '(') {
            return None;
        }
        Some(self.close_of(open)? + 1)
    }

    /// Index after a condition: a parenthesized group, or a macro call that
    /// stands for one (`if EQ("x") return 0;`).
    fn after_condition(&self, from: usize, end: usize) -> Option<usize> {
        self.after_parens(from, end).or_else(|| {
            let name = self.next(from, end)?;
            self.word(name)?;
            self.after_parens(name + 1, end)
        })
    }

    fn if_statement(&mut self, mut at: usize, end: usize) -> usize {
        // An `else if` chain is a loop, not recursion: chains can be long.
        // Each `else` body runs to the end of the chain.
        let mut else_bodies = Vec::new();
        let chain_end = loop {
            let mut condition = at + 1;
            if let Some(next) = self.next(condition, end)
                && matches!(self.word(next), Some("constexpr" | "consteval"))
            {
                condition = next + 1;
            }
            let Some(after) = self.after_condition(condition, end) else {
                break self.simple(at, end);
            };
            // astyle reads no header in `if MACRO(x)`, and pairs no `else`.
            let parenthesized = self.after_parens(condition, end).is_some();
            self.unterminated = false;
            let after_body = self.body(at, after, end);
            let Some(next) = self.next(after_body, end) else {
                break after_body;
            };
            if self.word(next) != Some("else") {
                break after_body;
            }
            // Without its `;` (a macro call) astyle does not see the body
            // end, and neither pairs the `else`.
            if !self.unterminated && parenthesized {
                self.else_ifs.insert(next, at);
            }
            let Some(following) = self.next(next + 1, end) else {
                break end;
            };
            else_bodies.push((next, following));
            if self.word(following) == Some("if") {
                at = following;
            } else {
                break self.body(next, next + 1, end);
            }
        };
        for (keyword, start) in else_bodies {
            self.else_bodies.push(ElseBody {
                keyword,
                start,
                end: chain_end,
                after_blank_line: false,
            });
        }
        chain_end
    }

    fn do_statement(&mut self, at: usize, end: usize) -> usize {
        let after = self.body(at, at + 1, end);
        let Some(keyword) = self.next(after, end) else {
            return after;
        };
        if self.word(keyword) != Some("while") {
            return after;
        }
        let Some(after_condition) = self.after_parens(keyword + 1, end) else {
            return after;
        };
        match self.next(after_condition, end) {
            Some(semicolon) if self.is_symbol(semicolon, ';') => semicolon + 1,
            _ => after_condition,
        }
    }

    /// An expression or declaration statement: runs to its `;`, to a block
    /// that ends it (a function or namespace body), or to a keyword that
    /// cannot occur inside an expression (a macro call without `;`).
    fn simple(&mut self, at: usize, end: usize) -> usize {
        let mut position = at;
        // Whether the statement so far is words and parenthesized groups,
        // like the head of a statement macro (`FOREACH(x) {`, `SEH_TRY {`).
        let mut plain_head = true;
        while let Some(index) = self.next(position, end) {
            if index != at
                && self.word(index).is_some_and(|word| {
                    matches!(
                        word,
                        "if" | "else" | "for" | "while" | "do" | "switch" | "return" | "case"
                    )
                })
            {
                self.unterminated = true;
                return index;
            }
            match self.tokens[index] {
                Token::Symbol(';') => return index + 1,
                Token::Symbol('}') => return index,
                Token::Symbol('(' | '[' | '{') => {
                    // Preprocessor branches can leave a group unclosed, such
                    // as a function head per branch; what follows it still
                    // parses as statements.
                    let Some(close) = self.close_of(index) else {
                        if self.is_symbol(index, '{') {
                            // What follows is inside the block, no sibling.
                            self.unterminated = true;
                            return index + 1;
                        }
                        position = index + 1;
                        continue;
                    };
                    self.nested(index, close);
                    self.unterminated = false;
                    if self.is_symbol(index, '{') && self.brace_ends_statement(index, plain_head) {
                        return close + 1;
                    }
                    position = close + 1;
                }
                Token::Operator(ref operator) if operator != "::" => {
                    plain_head = false;
                    position = index + 1;
                }
                Token::Symbol(',') => {
                    plain_head = false;
                    position = index + 1;
                }
                _ => position = index + 1,
            }
        }
        end
    }

    fn brace_kind(&self, open: usize) -> Option<BlockKind> {
        self.blocks.kind(self.groups.opened_at(open)?)
    }

    /// Whether a block at `open` inside a statement ends it: a function or
    /// namespace body does, a statement macro's block only after a plain
    /// head; an aggregate, initializer, or lambda does not.
    fn brace_ends_statement(&self, open: usize, plain_head: bool) -> bool {
        match self.brace_kind(open) {
            Some(BlockKind::FunctionBody | BlockKind::Namespace | BlockKind::ExternC) => true,
            Some(
                BlockKind::Aggregate
                | BlockKind::Initializer
                | BlockKind::CompoundLiteral
                | BlockKind::Lambda,
            ) => false,
            _ => plain_head,
        }
    }

    /// Parses statements inside a group of an expression: lambda bodies and
    /// statement expressions.
    fn nested(&mut self, open: usize, close: usize) {
        if self.is_symbol(open, '{')
            && !matches!(
                self.brace_kind(open),
                Some(BlockKind::Aggregate | BlockKind::Initializer | BlockKind::CompoundLiteral)
            )
        {
            self.items(open + 1, close);
            return;
        }
        let mut position = open + 1;
        while let Some(index) = self.next(position, close) {
            position = index + 1;
            if matches!(self.tokens[index], Token::Symbol('(' | '[' | '{'))
                && let Some(inner) = self.close_of(index)
            {
                // A statement expression: `({ ... })`.
                if self.is_symbol(open, '(')
                    && self.is_symbol(index, '{')
                    && self.next(open + 1, close) == Some(index)
                {
                    self.items(index + 1, inner);
                } else {
                    self.nested(index, inner);
                }
                position = inner + 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Statements;
    use crate::formatter::lexer::{Token, tokenize};
    use crate::formatter::structure::blocks::Blocks;
    use crate::formatter::structure::groups::Groups;

    /// For each `else` in source order, the source line of its `if`.
    fn else_lines(source: &str) -> Vec<Option<usize>> {
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let blocks = Blocks::build(&tokens, &groups);
        let statements = Statements::build(&tokens, &groups, &blocks);
        let line_of = |index: usize| {
            tokens[..index]
                .iter()
                .filter(|token| matches!(token, Token::Newline))
                .count()
        };
        (0..tokens.len())
            .filter(|&index| matches!(&tokens[index], Token::Word(word) if word == "else"))
            .map(|index| statements.if_of_else(index).map(line_of))
            .collect()
    }

    #[test]
    fn else_chain_in_braceless_loop_body() {
        let source = "void f(void)\n{\n    for (i = 0; i < 8; i++)\n        if (a)\n            return 0;\n        else if (b)\n        {\n            c();\n        }\n        else if (d)\n            return 0;\n    return 1;\n}\n";
        assert_eq!(else_lines(source), [Some(3), Some(5)]);
    }

    #[test]
    fn nested_braceless_if_binds_else_to_inner_if() {
        let source = "void f(void)\n{\n    if (a)\n        if (b)\n            x();\n        else\n            y();\n    else\n        z();\n}\n";
        assert_eq!(else_lines(source), [Some(3), Some(2)]);
    }

    #[test]
    fn else_after_do_while_and_macro_call_without_semicolon() {
        let source = "void f(void)\n{\n    if (a)\n        do\n            x();\n        while (b);\n    else\n        FOO(c)\n    if (d) {\n        g();\n    } else {\n        h();\n    }\n}\n";
        assert_eq!(else_lines(source), [Some(2), Some(8)]);
    }

    #[test]
    fn else_after_semicolonless_macro_body_stays_unpaired() {
        let source = "void f()\n{\n    if ( a )\n        MACRO(a)\n    else if ( b )\n        MACRO(b)\n    return;\n}\n";
        assert_eq!(else_lines(source), [None]);
    }

    /// For each statement starting a line, the line of its previous
    /// sibling.
    fn sibling_lines(source: &str) -> Vec<(usize, usize)> {
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let blocks = Blocks::build(&tokens, &groups);
        let statements = Statements::build(&tokens, &groups, &blocks);
        let line_of = |index: usize| {
            tokens[..index]
                .iter()
                .filter(|token| matches!(token, Token::Newline))
                .count()
        };
        let mut pairs: Vec<_> = (0..tokens.len())
            .filter_map(|index| {
                statements
                    .previous_sibling(index)
                    .map(|previous| (line_of(index), line_of(previous)))
            })
            .collect();
        pairs.sort_unstable();
        pairs
    }

    #[test]
    fn statements_link_to_previous_siblings_past_comments_and_bodies() {
        let source = "void f(void)\n{\n    a();\n    /* c */\n    if (x)\n        b();\n    switch (y) {\n    case 1:\n        c();\n        break;\n    }\n    FOO(z)\n    d();\n}\n";
        assert_eq!(sibling_lines(source), [(4, 2), (6, 4), (9, 8), (11, 6)]);
    }

    #[test]
    fn conditional_branches_and_the_code_after_them_follow_the_code_before() {
        let source = "void f(void)\n{\n    a();\n#if X\n    b();\n#else\n    c();\n#endif\n    d();\n    if (x\n#if Y\n        && y\n#endif\n       )\n        e();\n    g();\n}\n";
        assert_eq!(
            sibling_lines(source),
            [(4, 2), (6, 2), (8, 2), (9, 8), (15, 9)]
        );
    }

    #[test]
    fn else_after_macro_condition_stays_unpaired() {
        let source = "void f(void)\n{\n    if EQ(\"\") return 0;\n    else if EQ(\"x\") {\n        g();\n    }\n}\n";
        assert_eq!(else_lines(source), [None]);
    }

    #[test]
    fn else_inside_lambda_and_statement_expression() {
        let source = "int x = ({ if (a) 1; else 2; });\nauto f = [](int v) { if (v) return 1; else return 2; };\n";
        assert_eq!(else_lines(source), [Some(0), Some(1)]);
    }
}
