//! Statements: which `if` each `else` belongs to.

use super::blocks::{BlockKind, Blocks, next_code_token};
use super::groups::Groups;
use crate::formatter::lexer::Token;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct Statements {
    /// `if` token of each `else` token, by token index.
    else_ifs: HashMap<usize, usize>,
    /// Keyword of the control statement whose braceless body starts at a
    /// token, by the body's first token.
    braceless_headers: HashMap<usize, usize>,
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
}

impl Statements {
    pub(crate) fn build(tokens: &[Token], groups: &Groups, blocks: &Blocks) -> Self {
        let mut parser = Parser {
            tokens,
            groups,
            blocks,
            else_ifs: HashMap::new(),
            braceless_headers: HashMap::new(),
            else_bodies: Vec::new(),
            unterminated: false,
        };
        parser.items(0, tokens.len());
        Self {
            else_ifs: parser.else_ifs,
            braceless_headers: parser.braceless_headers,
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

    /// Whether the token `index` is in the body of an `else` that a blank
    /// line separates from its body.
    pub(crate) fn in_split_else_body(&self, index: usize) -> bool {
        self.split_else_bodies
            .iter()
            .any(|body| (body.start..body.end).contains(&index))
    }
}

struct Parser<'a> {
    tokens: &'a [Token],
    groups: &'a Groups,
    blocks: &'a Blocks,
    else_ifs: HashMap<usize, usize>,
    braceless_headers: HashMap<usize, usize>,
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
        while let Some(at) = self.next(position, end) {
            position = self.statement(at, end).max(at + 1);
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
                if let Some(next) = self.next(at + 1, end)
                    && self.is_symbol(next, ':')
                {
                    return next + 1;
                }
            }
            None => {}
        }
        if self.is_symbol(at, '{') {
            return match self.close_of(at) {
                Some(close) => {
                    self.items(at + 1, close);
                    close + 1
                }
                None => at + 1,
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
                if !self.is_symbol(at, '{') {
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
            if !self.unterminated {
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
                        return index + 1;
                    };
                    self.nested(index, close);
                    if self.is_symbol(index, '{') && self.brace_ends_statement(index) {
                        return close + 1;
                    }
                    position = close + 1;
                }
                _ => position = index + 1,
            }
        }
        end
    }

    fn brace_kind(&self, open: usize) -> Option<BlockKind> {
        self.blocks.kind(self.groups.opened_at(open)?)
    }

    fn brace_ends_statement(&self, open: usize) -> bool {
        !matches!(
            self.brace_kind(open),
            Some(
                BlockKind::Aggregate
                    | BlockKind::Initializer
                    | BlockKind::CompoundLiteral
                    | BlockKind::Lambda
            )
        )
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

    #[test]
    fn else_inside_lambda_and_statement_expression() {
        let source = "int x = ({ if (a) 1; else 2; });\nauto f = [](int v) { if (v) return 1; else return 2; };\n";
        assert_eq!(else_lines(source), [Some(0), Some(1)]);
    }
}
