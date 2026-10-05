use crate::formatter::syntax::language;
use crate::formatter::text::line_scan::{has_top_level_comma_in_text, trailing_matching_parens};
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

pub(crate) fn line_ends_compound_literal_cast(line: &str) -> bool {
    let current = line.trim_ascii_end();
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

    let before_open = current[..open_pos].trim_ascii_end();
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
    // After `*`, `&` or `^` the parentheses may be a declarator's.
    let before = text[..open].trim_ascii_end();
    match before.chars().next_back() {
        Some(')' | ']' | '*' | '&' | '^') => false,
        Some(ch) if is_word_char(ch) => trailing_word(before) == language::RETURN,
        _ => true,
    }
}

fn ends_with_operator_overload_name(text: &str) -> bool {
    let stripped = text.trim_end_matches(|ch| "+-*/%^&|~!=<>".contains(ch));
    if stripped.len() == text.len() {
        return false;
    }
    trailing_word(stripped.trim_ascii_end()) == "operator"
}
