use crate::config::MinConditionalIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, next_non_whitespace};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::line_view::LineView;

pub(crate) fn source_indented_macro_row(
    tokens: &[Token],
    line_start: usize,
    line_end: usize,
    word_index: usize,
) -> bool {
    let Some(Token::Word(word)) = tokens.get(word_index) else {
        return false;
    };
    if !word
        .chars()
        .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        return false;
    }
    if !matches!(
        next_non_whitespace(tokens, word_index + 1, line_end).and_then(|index| tokens.get(index)),
        Some(Token::Symbol('('))
    ) {
        return false;
    }
    tokens[line_start..line_end]
        .iter()
        .any(|token| matches!(token, Token::Symbol(';')))
        && !tokens[line_start..line_end].iter().any(|token| {
            matches!(token, Token::Symbol('{' | '}'))
                || matches!(token, Token::Operator(operator) if operator == "=")
        })
}

impl FormatEngine<'_> {
    pub(crate) fn source_indent_override_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero
            || line_kind != LineKind::Normal
        {
            return None;
        }
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        self.previous_call_argument_sibling_indent(line)
    }
}
