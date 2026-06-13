use super::columns::{leading_visual_width, visual_width_from};
use super::labels::is_label_start;
use super::language::{self, is_non_type_keyword, is_type_like_pointer_word};
use super::line_scan::line_paren_imbalance;
use super::line_scan::{
    reverse_scan_skips_block_comment, trailing_comment_split_limit, unmatched_open_paren_column,
};
use super::syntax::{first_operator_word, function_name_start, is_named_operator_word};
use super::{FormatEngine, switch_cases};
use crate::source::lex::{is_identifier_continue, leading_identifier};

impl FormatEngine<'_> {
    pub(super) fn split_return_type_pointer_name_indent_spaces(&self, line: &str) -> Option<usize> {
        if !is_pointer_prefixed_function_part(line.trim_start()) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        is_return_type_line(previous.trim())
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn split_trailing_return_arrow_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with("->") || current.starts_with("->*") {
            return None;
        }

        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(16)
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if reverse_scan_skips_block_comment(code, &mut in_block_comment) {
                continue;
            }
            if close_pending == 0 && !code.ends_with(')') {
                return None;
            }
            let (closes, mut opens) = line_paren_imbalance(code);
            if close_pending > 0
                && let Some(&column) = opens.last()
                && code[column..].starts_with('(')
            {
                let before = code[..column].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                if is_parameter_return_type_prefix(return_type)
                    && !name.is_empty()
                    && !self.is_header(name)
                {
                    return Some(leading_visual_width(previous, self.options.tab_width));
                }
            }
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if close_pending == 0
                && (code.ends_with(';') || code.ends_with('{') || code.ends_with('}'))
            {
                return None;
            }
        }
        None
    }

    fn recent_base_trailing_return_function_header_index(&self) -> Option<usize> {
        if self.state.indent() == 0 {
            return None;
        }
        let mut closed_blocks = 0usize;
        for (index, line) in self.output.iter().enumerate().rev().take(24) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('}') {
                closed_blocks += 1;
            }
            if code.ends_with('{') {
                if closed_blocks > 0 {
                    closed_blocks -= 1;
                    continue;
                }
                if code.contains(") ->") && !trimmed.starts_with("template ") {
                    return Some(index);
                }
                return None;
            }
        }
        None
    }

    pub(super) fn recent_base_trailing_return_function_header(&self) -> bool {
        self.recent_base_trailing_return_function_header_index()
            .is_some()
    }

    pub(super) fn recent_trailing_return_function_after_multiline_template_declaration(
        &self,
    ) -> bool {
        let Some(brace_index) = self.recent_base_trailing_return_function_header_index() else {
            return false;
        };
        let mut signature_start = brace_index;
        let mut index = brace_index;
        while index > 0 {
            index -= 1;
            let line = &self.output[index];
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("template ")
                || code.ends_with(';')
                || code.ends_with('{')
                || code.ends_with('}')
            {
                break;
            }
            signature_start = index;
            if code.contains('(') {
                break;
            }
        }
        self.output_closes_multiline_template_declaration_before(signature_start)
    }

    pub(super) fn trailing_return_function_parameter_tail_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.contains("= {}") || !current.contains(") ->") || !current.ends_with('{') {
            return None;
        }
        for (previous_index, previous) in self
            .output
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, line)| !line.trim().is_empty())
            .take(8)
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if let Some(open) = unmatched_open_paren_column(previous_code) {
                let before = previous_code[..open].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                let prefixed_return_type = return_type
                    .split_whitespace()
                    .any(language::is_macro_like_word)
                    && is_parameter_return_type_prefix(return_type);
                if leading_visual_width(previous, self.options.tab_width) == 0
                    && (prefixed_return_type
                        || self.output_closes_multiline_template_declaration_before(previous_index))
                    && !name.is_empty()
                    && !self.is_header(name)
                {
                    return Some(0);
                }
                return None;
            }
            if previous_code.ends_with([';', '{', '}']) {
                return None;
            }
        }
        None
    }

    pub(super) fn function_signature_parameter_continuation_indent_spaces(
        &self,
        current_line: &str,
        signature_line: &str,
        open_paren: usize,
    ) -> Option<usize> {
        let before = signature_line[..open_paren].trim_end();
        let name_start = function_name_start(before)?;
        let return_type = before[..name_start].trim_end();
        let name = before[name_start..].trim_start();
        if !is_parameter_return_type_prefix(return_type) || name.is_empty() || self.is_header(name)
        {
            return None;
        }
        let after_paren = &signature_line[open_paren + 1..];
        let after_paren_indent = after_paren.len() - after_paren.trim_start().len();
        let visual_open =
            visual_width_from(&signature_line[..open_paren], 0, self.options.tab_width);
        let visual_after = visual_width_from(
            &after_paren[..after_paren_indent],
            visual_open + 1,
            self.options.tab_width,
        );
        let spaces = if current_line.starts_with(')') {
            visual_open
        } else {
            visual_open + 1 + visual_after
        };
        (spaces <= self.options.max_continuation_indent).then_some(spaces)
    }

    pub(super) fn try_publish_attached_return_type(&mut self, line: &str) -> bool {
        let is_declaration = line.ends_with(';');
        let should_attach = if is_declaration {
            self.options.attach_return_type_decl
        } else {
            self.options.attach_return_type
        };
        if !should_attach || !is_function_part_line(line) {
            return false;
        }
        let Some(previous) = self.output.pop() else {
            return false;
        };
        let previous_trimmed = previous.trim();
        if switch_cases::find_case_colon(previous_trimmed).is_some()
            || (previous_trimmed.ends_with(':')
                && is_label_start(
                    previous_trimmed.trim_end_matches(':'),
                    &self.options.access_labels,
                ))
            || !is_return_type_line(previous_trimmed)
            || (previous_trimmed.starts_with("struct ") && previous_trimmed.ends_with('*'))
        {
            self.output.push(previous);
            return false;
        }
        let previous_prefix_len = previous.len() - previous.trim_start().len();
        let previous_prefix = &previous[..previous_prefix_len];
        let separator = if previous_trimmed.ends_with(['*', '&', '^']) {
            ""
        } else {
            " "
        };
        self.adjust_and_publish_line(format!(
            "{previous_prefix}{previous_trimmed}{separator}{}",
            line.trim_start()
        ));
        true
    }

    pub(super) fn try_publish_split_return_type(
        &mut self,
        line: &str,
        indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> bool {
        let is_declaration = line.ends_with(';');
        let should_split = if is_declaration {
            self.options.break_return_type_decl && !self.options.attach_return_type_decl
        } else {
            self.options.break_return_type && !self.options.attach_return_type
        };
        if !should_split || self.is_header(leading_identifier(line.trim_start())) {
            return false;
        }
        let Some((return_type, function_part)) = split_return_type_line(line) else {
            return false;
        };
        if let Some(spaces) = exact_indent_spaces {
            self.push_formatted_line_exact(&return_type, indent, spaces);
            self.push_formatted_line_exact(&function_part, indent, spaces);
        } else {
            self.push_formatted_line(&return_type, indent);
            self.push_formatted_line(&function_part, indent);
        }
        true
    }
}

fn split_return_type_line(line: &str) -> Option<(String, String)> {
    if line.starts_with("return ") || line.trim_start().starts_with('#') {
        return None;
    }
    let open_paren = line.find('(')?;
    if is_function_pointer_line(line, open_paren) {
        return None;
    }
    let before = line[..open_paren].trim_end();
    if before.is_empty() || language::is_header(before) || is_conversion_function_head(before) {
        return None;
    }
    let name_start = function_name_start(before)?;
    if before[..name_start].contains('=') {
        return None;
    }
    let return_type = before[..name_start].trim_end();
    let function_name = before[name_start..].trim_start();
    if return_type.is_empty() || function_name.is_empty() || !return_type_has_code(return_type) {
        return None;
    }
    if return_type.contains('.') || return_type.contains("->") || return_type.contains('}') {
        return None;
    }
    let function_part = format!("{}{}", function_name, &line[open_paren..]);
    Some((return_type.to_string(), function_part))
}

fn is_function_part_line(line: &str) -> bool {
    if line.starts_with("return ") || line.trim_start().starts_with('#') {
        return false;
    }
    let Some(open_paren) = line.find('(') else {
        return false;
    };
    if is_function_pointer_line(line, open_paren) {
        return false;
    }
    let before = line[..open_paren].trim_end();
    !before.is_empty()
        && !language::is_header(before)
        && function_name_start(before).is_some_and(|start| start == 0)
}

pub(super) fn is_return_type_line(line: &str) -> bool {
    if line.is_empty()
        || line.contains("//")
        || line.contains("/*")
        || line.contains("*/")
        || line.starts_with('#')
    {
        return false;
    }
    let mut angle_depth: i32 = 0;
    for ch in line.chars() {
        match ch {
            '{' | '}' | ';' => return false,
            '<' => angle_depth += 1,
            '>' => angle_depth = (angle_depth - 1).max(0),
            '(' | ')' | '=' | ',' if angle_depth == 0 => return false,
            _ => {}
        }
    }
    if angle_depth != 0 {
        return false;
    }
    line.split(|ch: char| !is_identifier_continue(ch))
        .any(|part| !part.is_empty() && is_type_like_pointer_word(part))
}

pub(super) fn is_parameter_return_type_prefix(line: &str) -> bool {
    let first_word = line
        .split(|ch: char| !is_identifier_continue(ch))
        .find(|word| !word.is_empty());
    if first_word.is_some_and(|word| {
        is_non_type_keyword(word)
            || matches!(
                word,
                "co_return" | "alignof" | "noexcept" | "typeid" | "requires" | "decltype"
            )
    }) {
        return false;
    }
    is_return_type_line(line)
        || (!line.trim().is_empty()
            && line.chars().all(|ch| {
                ch.is_whitespace()
                    || is_identifier_continue(ch)
                    || matches!(ch, ':' | '<' | '>' | '*' | '&')
            }))
}

fn return_type_has_code(mut text: &str) -> bool {
    loop {
        text = text.trim_start();
        if text.is_empty() || text.starts_with("//") {
            return false;
        }
        let Some(comment) = text.strip_prefix("/*") else {
            return true;
        };
        let Some(end) = comment.find("*/") else {
            return false;
        };
        text = &comment[end + 2..];
    }
}

fn is_conversion_function_head(before_open_paren: &str) -> bool {
    let Some(operator) = before_open_paren.rfind(language::OPERATOR) else {
        return false;
    };
    let after = before_open_paren[operator + language::OPERATOR.len()..].trim_start();
    first_operator_word(after).is_some_and(|word| !is_named_operator_word(word))
}

fn is_function_pointer_line(line: &str, open_paren: usize) -> bool {
    line[open_paren..].starts_with("(*") || line[..open_paren].trim_end().ends_with("(*")
}

fn is_pointer_prefixed_function_part(line: &str) -> bool {
    let rest =
        line.trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '*' | '&' | '^'));
    if rest == line {
        return false;
    }
    let Some(open_paren) = rest.find('(') else {
        return false;
    };
    let before = rest[..open_paren].trim_end();
    !before.is_empty()
        && !language::is_header(before)
        && function_name_start(before).is_some_and(|start| start == 0)
}
