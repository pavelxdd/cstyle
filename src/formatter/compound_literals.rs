use super::language;
use super::line_scan::{has_top_level_comma_in_text, trailing_matching_parens};
use crate::source::lex::{is_word_char, trailing_word};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct CompoundLiteralState {
    pub(super) forced_break_depths: Vec<usize>,
    pub(super) just_closed: bool,
    pub(super) after_comma: bool,
    pub(super) arg_indent_spaces: Option<usize>,
    pub(super) arg_paren_depth: Option<usize>,
    pub(super) arg_brace_depth: Option<usize>,
}

pub(super) fn line_ends_compound_literal_cast(line: &str) -> bool {
    let current = line.trim_end();
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

    let before_open = current[..open_pos].trim_end();
    if ends_with_operator_overload_name(before_open) {
        return false;
    }
    match before_open.chars().next_back() {
        Some(ch) if is_word_char(ch) => trailing_word(before_open) == language::RETURN,
        Some(')' | ']') => false,
        _ => true,
    }
}

fn ends_with_operator_overload_name(text: &str) -> bool {
    let stripped = text.trim_end_matches(|ch| "+-*/%^&|~!=<>".contains(ch));
    if stripped.len() == text.len() {
        return false;
    }
    trailing_word(stripped.trim_end()) == "operator"
}
