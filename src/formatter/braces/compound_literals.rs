use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::language;
use crate::formatter::text::line_scan::{has_top_level_comma_in_text, trailing_matching_parens};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_word_char, trailing_word};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct CompoundLiteralState {
    pub(crate) forced_break_depths: Vec<usize>,
    pub(crate) just_closed: bool,
    pub(crate) after_comma: bool,
    pub(crate) arg_indent_spaces: Option<usize>,
    pub(crate) arg_paren_depth: Option<usize>,
    pub(crate) arg_brace_depth: Option<usize>,
}

impl FormatEngine<'_> {
    /// Whether the parens opened at `open` hold, as an argument of their
    /// own, a compound literal whose braces span lines.
    pub(crate) fn parens_hold_multiline_compound_literal(&self, open: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let Some(parens) = groups.opened_at(open) else {
            return false;
        };
        groups.get(parens).delimiter == Delimiter::Paren
            && groups.members(parens).any(|brace| {
                groups.opened_at(brace).is_some_and(|literal| {
                    self.tree.blocks.kind(literal) == Some(BlockKind::CompoundLiteral)
                        && groups.get(literal).close.is_some_and(|close| {
                            tokens[brace..close]
                                .iter()
                                .any(|token| matches!(token, Token::Newline))
                        })
                })
            })
    }

    /// The `(` that ends its line and opens the parens holding the token at
    /// `index`, when they hold a compound literal spanning lines. The
    /// arguments of such a call stand a level past the line of its `(`,
    /// where the literal's rows have room.
    pub(crate) fn literal_argument_parens(&self, index: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let open = groups.get(groups.enclosing(index)?).open;
        let ends_line = tokens[open + 1..]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_)))
            .is_some_and(|token| matches!(token, Token::Newline));
        (ends_line && self.parens_hold_multiline_compound_literal(open)).then_some(open)
    }
}

pub(crate) fn line_ends_compound_literal_cast(line: &str) -> bool {
    let current = line.trimmed_end();
    if !current.ends_with(')') {
        return false;
    }
    let Some((open_pos, close_pos)) = trailing_matching_parens(current) else {
        return false;
    };
    if close_pos == open_pos + 1 {
        return false;
    }
    if has_top_level_comma_in_text(&current[open_pos + 1..close_pos]) {
        return false;
    }

    let before_open = current[..open_pos].trimmed_end();
    if ends_with_operator_overload_name(before_open) {
        return false;
    }
    match before_open.chars().next_back() {
        Some(ch) if is_word_char(ch) => trailing_word(before_open) == language::RETURN,
        Some(')') => ends_with_value_cast(before_open),
        Some(']') => false,
        _ => true,
    }
}

/// Whether `text` ends with a cast that starts a value, which the cast of a
/// compound literal may follow: `(int *)` in `p = (int *)(int[]){ … }`.
fn ends_with_value_cast(text: &str) -> bool {
    let Some((open, close)) = trailing_matching_parens(text) else {
        return false;
    };
    if close == open + 1 || has_top_level_comma_in_text(&text[open + 1..close]) {
        return false;
    }
    // After `*`, `&` or `^` the parentheses may be a declarator's, unless
    // the operator joins two values of an expression.
    let before = text[..open].trimmed_end();
    match before.chars().next_back() {
        Some('*' | '&' | '^') => operator_in_expression(&before[..before.len() - 1]),
        Some(')' | ']') => false,
        Some(ch) if is_word_char(ch) => trailing_word(before) == language::RETURN,
        _ => true,
    }
}

/// Whether an operator after `text` stands in an expression: as a unary
/// operator, or joining a value that an assignment, a `return` or an open
/// parenthesis starts, where a declaration's star would follow its type.
fn operator_in_expression(text: &str) -> bool {
    let text = text.trimmed_end();
    if !text
        .chars()
        .next_back()
        .is_some_and(|ch| is_word_char(ch) || matches!(ch, ')' | ']'))
    {
        return true;
    }
    let bytes = text.as_bytes();
    let mut depth = 0isize;
    for (index, &byte) in bytes.iter().enumerate().rev() {
        match byte {
            b')' | b']' => depth += 1,
            b'(' | b'[' if depth == 0 => return true,
            b'(' | b'[' => depth -= 1,
            b'=' if depth == 0
                && bytes.get(index + 1) != Some(&b'=')
                && !index
                    .checked_sub(1)
                    .is_some_and(|before| matches!(bytes[before], b'=' | b'!' | b'<' | b'>')) =>
            {
                return true;
            }
            b';' | b'{' | b'}' if depth == 0 => return false,
            _ => {}
        }
    }
    let words = text.split(|ch: char| !is_word_char(ch));
    words.into_iter().any(|word| word == language::RETURN)
}

fn ends_with_operator_overload_name(text: &str) -> bool {
    let stripped = text.trim_end_matches(|ch| "+-*/%^&|~!=<>".contains(ch));
    if stripped.len() == text.len() {
        return false;
    }
    trailing_word(stripped.trimmed_end()) == "operator"
}
