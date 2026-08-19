use super::FormatEngine;
use super::indentation::LineKind;
use super::initializer_braces::initializer_sibling_uses_previous_indent;
use super::labels::is_attached_user_label;
use super::line_scan::{trailing_comment_split_limit, unmatched_open_paren_column};
use super::literals::starts_string_literal_token;
use super::operators::{head_ends_binary_operator, starts_ternary_arm, starts_with_chain_operator};
use super::token::{Token, next_non_whitespace};
use crate::config::MinConditionalIndent;

pub(super) fn source_indented_macro_row(
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
    pub(super) fn source_indent_override_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: usize,
    ) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero
            || line_kind != LineKind::Normal
        {
            return None;
        }
        let source = self.token_input.input_source_indent;
        let output_source = self.source_indent_for_output(source);
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }
        if starts_string_literal_token(trimmed)
            && self.output.last_non_empty_line().is_some_and(|previous| {
                let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
                previous_code.ends_with(',') && unmatched_open_paren_column(previous_code).is_some()
            })
        {
            return None;
        }
        if self
            .constructor_initializer_header_indent_spaces(line)
            .is_some()
        {
            return None;
        }
        if let Some(spaces) = self.previous_initializer_comma_indent()
            && initializer_sibling_uses_previous_indent(trimmed)
        {
            if current_spaces < spaces {
                return Some(spaces);
            }
            if current_spaces == spaces {
                return None;
            }
        }
        if let Some(spaces) = self.compound_initializer_value_indent(trimmed) {
            if current_spaces < spaces {
                return Some(spaces);
            }
            if current_spaces == spaces {
                return None;
            }
        }
        if let Some(spaces) = self.previous_call_argument_sibling_indent(line) {
            return Some(spaces);
        }
        if let Some(spaces) =
            self.call_argument_source_indent(trimmed, current_spaces, output_source, source)
        {
            return Some(spaces);
        }
        if self.initializer_current_indent_matches_previous_row(trimmed, current_spaces, source) {
            return None;
        }
        if self.options.break_after_logical
            && source > 0
            && output_source > current_spaces
            && self.source_owned_continuation_line(trimmed)
        {
            return Some(output_source);
        }
        if source == 0 || output_source == current_spaces {
            return None;
        }
        if self.options.break_after_logical && self.line_follows_preprocessor_guarded_header_body()
        {
            return Some(output_source);
        }
        if self.in_initializer_brace()
            || self.innermost_init_block_brace()
            || self.in_aggregate_declaration_brace()
            || self.current_inline_array_column().is_some()
            || self.output_has_open_initializer_brace()
            || self.previous_comma_inside_open_brace()
        {
            if current_spaces > output_source
                && trimmed.starts_with('.')
                && (self.current_initializer_member_before_closing_brace()
                    || source >= current_spaces)
            {
                return None;
            }
            if self.initializer_line_keeps_source_indent(trimmed) {
                return Some(output_source);
            }
        }
        if self.options.break_after_logical
            && starts_ternary_arm(trimmed)
            && (trimmed.starts_with('?') || self.recent_output_has_open_ternary())
        {
            return Some(output_source);
        }
        if self.options.break_after_logical
            && output_source > current_spaces
            && self.line_follows_logical_operator()
        {
            if self
                .logical_condition_sibling_indent_spaces(line)
                .is_some_and(|spaces| spaces == current_spaces)
                || self
                    .header_operator_continuation_indent_spaces(line)
                    .is_some_and(|spaces| spaces == current_spaces)
            {
                return None;
            }
            return Some(output_source);
        }
        None
    }

    fn source_indent_for_output(&self, source: usize) -> usize {
        source + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width
    }

    fn source_owned_continuation_line(&self, trimmed: &str) -> bool {
        if trimmed.starts_with("case ")
            || trimmed.starts_with("default:")
            || is_attached_user_label(trimmed)
        {
            return false;
        }
        if starts_ternary_arm(trimmed) || starts_with_chain_operator(trimmed) {
            return true;
        }
        let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        else {
            return false;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if previous_trimmed.starts_with('#') || previous_code.ends_with(';') {
            return false;
        }
        previous_code.ends_with(['(', '[', '=', '?', '\\'])
            || head_ends_binary_operator(previous_code)
            || self.line_follows_logical_operator()
            || self.stack_state.paren_depth > 0
    }
}
