use crate::formatter::engine::FormatEngine;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{ContainsAnyByte, has_hash_outside_literals};
use crate::formatter::text::line_scan::{
    trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TemplateDeclarationState {
    uses_source_indent: bool,
    angle_depth: isize,
}

pub(crate) fn template_declaration_line_complete(line: &str) -> bool {
    line.ends_with('>') && angle_depth(line) <= 0
}

fn angle_depth_delta(line: &str) -> isize {
    if !line.contains_any_byte(b"<>") {
        return 0;
    }
    angle_chars(line).fold(0, |depth, (_, ch)| match ch {
        '<' => depth + 1,
        _ => depth - 1,
    })
}

/// The angle brackets of `line` by char column, leaving out those naming an
/// operator, as in `operator<<`.
fn angle_chars(line: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let chars = line.chars().collect::<Vec<_>>();
    let mut operator_name_end = 0;
    let mut column = 0;
    std::iter::from_fn(move || {
        while column < chars.len() {
            let at = column;
            column += 1;
            if chars[at..].starts_with(&['o', 'p', 'e', 'r', 'a', 't', 'o', 'r'])
                && !at
                    .checked_sub(1)
                    .is_some_and(|before| chars[before].is_alphanumeric() || chars[before] == '_')
            {
                let mut end = at + "operator".len();
                if chars
                    .get(end)
                    .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_')
                {
                    continue;
                }
                while chars.get(end).is_some_and(|ch| ch.is_whitespace()) {
                    end += 1;
                }
                while chars
                    .get(end)
                    .is_some_and(|ch| "<>=!+-*/%^&|~".contains(*ch))
                {
                    end += 1;
                }
                operator_name_end = end;
            }
            if at < operator_name_end {
                continue;
            }
            if matches!(chars[at], '<' | '>') {
                return Some((at, chars[at]));
            }
        }
        None
    })
}

pub(crate) fn template_continuation_indent_spaces(line: &str) -> Option<usize> {
    let trimmed = line.trimmed_start();
    if !trimmed.starts_with("template") || angle_depth_delta(trimmed) <= 0 {
        return None;
    }
    let code = line[..trailing_comment_split_limit(line)].trimmed_end();
    if (code.ends_with("||") || code.ends_with("&&"))
        && let Some(open) = unmatched_open_paren_column(code)
    {
        return Some(open + 1);
    }
    let mut stack = Vec::new();
    for (column, ch) in angle_chars(line) {
        if ch == '<' {
            stack.push(column);
        } else {
            stack.pop();
        }
    }
    stack
        .into_iter()
        .rev()
        .find_map(|column| {
            let after = line.chars().skip(column + 1).collect::<String>();
            after
                .chars()
                .any(|ch| !ch.is_whitespace())
                .then(|| column + 1 + after.chars().take_while(|ch| ch.is_whitespace()).count())
        })
        .or_else(|| Some(line.len() - trimmed.len() + 4))
}

impl FormatEngine<'_> {
    pub(crate) fn is_template_declaration_line(&self) -> bool {
        self.current.trimmed_start().starts_with("template")
    }

    pub(crate) fn is_complete_template_declaration_line(&self) -> bool {
        let current = self.current.trimmed();
        current.starts_with("template")
            && current.ends_with('>')
            && template_declaration_line_complete(current)
    }

    pub(crate) fn previous_output_is_complete_template_declaration(&self) -> bool {
        let Some(index) = self.output.last_non_empty_index() else {
            return false;
        };
        let line = &self.output[index];
        if !line.trimmed_end().ends_with('>') {
            return false;
        }
        let code = self.output.code_before_comment(index).trimmed_end();
        let trimmed = code.trimmed_start();
        is_template_declaration_head_line(trimmed)
            && template_declaration_line_complete(trimmed)
            && !trimmed.ends_with(';')
    }

    pub(crate) fn previous_output_closes_multiline_template_declaration(&self) -> bool {
        let Some(index) = self.output.last_non_empty_index() else {
            return false;
        };
        let previous = &self.output[index];
        if !previous.trimmed_end().ends_with('>') {
            return false;
        }
        let lines: Vec<&str> = self.output[..=index]
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(16)
            .map(String::as_str)
            .collect();
        for (index, line) in lines.iter().enumerate().skip(1) {
            let code = self.output.code_of(line).trimmed_end();
            let trimmed = code.trimmed_start();
            if is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = self.output.code_of(line).trimmed_end();
                        angle_depth_delta(code)
                    })
                    .sum();
                return depth <= 0 && !template_declaration_line_complete(trimmed);
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
        }
        false
    }

    pub(super) fn output_closes_multiline_template_declaration_before(&self, end: usize) -> bool {
        let lines: Vec<&str> = self.output[..end]
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(16)
            .map(String::as_str)
            .collect();
        let Some(previous) = lines.first() else {
            return false;
        };
        let previous_code = self.output.code_of(previous).trimmed_end();
        if !previous_code.ends_with('>') {
            return false;
        }
        for (index, line) in lines.iter().enumerate().skip(1) {
            let code = self.output.code_of(line).trimmed_end();
            let trimmed = code.trimmed_start();
            if is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = self.output.code_of(line).trimmed_end();
                        angle_depth_delta(code)
                    })
                    .sum();
                return depth <= 0 && !template_declaration_line_complete(trimmed);
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
        }
        false
    }

    pub(crate) fn prepare_template_continuation_token_indent(&mut self, source_column: usize) {
        if !self.layout.template_declaration.uses_source_indent
            || !self.token_input.token_begins_source_line
        {
            return;
        }
        let previous = self.output.last_non_empty_scoped();
        if previous.is_some_and(|line| has_hash_outside_literals(line)) {
            return;
        }
        let spaces = previous
            .and_then(|line| {
                template_continuation_indent_spaces(line).or_else(|| {
                    (source_column == 0)
                        .then(|| leading_visual_width(line, self.options.tab_width))
                        .filter(|&spaces| spaces > 0)
                })
            })
            .unwrap_or(source_column);
        self.layout.continuation_indent.set_next_line_spaces(spaces);
    }

    pub(crate) fn template_continuation_active(&self) -> bool {
        self.layout.template_declaration.uses_source_indent
    }

    pub(crate) fn template_continuation_closes_on_line(&self, line: &str) -> bool {
        self.layout.template_declaration.uses_source_indent
            && line.ends_with('>')
            && self.layout.template_declaration.angle_depth + angle_depth_delta(line) <= 0
    }

    pub(crate) fn template_continuation_line_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_trimmed = line.trimmed();
        if !self.layout.template_declaration.uses_source_indent {
            return None;
        }
        let mut spaces = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| {
                self.output
                    .code_of(line)
                    .trimmed_start()
                    .starts_with("template <")
            })
            .filter(|line| self.output.code_of(line).trimmed() == "template <")
            .map(|line| {
                leading_visual_width(line, self.options.tab_width) + self.options.indent_width
            });
        if self.template_continuation_closes_on_line(line_trimmed)
            && line_trimmed == ">"
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        spaces
    }

    pub(crate) fn observe_template_declaration_line(&mut self, line: &LineView<'_>) {
        let trimmed = line.trimmed();
        let starts_template = trimmed.starts_with("template");
        if !starts_template && !self.layout.template_declaration.uses_source_indent {
            return;
        }
        let angle_delta = angle_depth_delta(trimmed);
        if starts_template
            && angle_delta > 0
            && !trimmed.ends_with(';')
            && !template_declaration_line_complete(trimmed)
        {
            self.layout.template_declaration.uses_source_indent = true;
            self.layout.template_declaration.angle_depth = angle_delta;
            if let Some(spaces) = template_continuation_indent_spaces(line) {
                self.layout.continuation_indent.set_next_line_spaces(spaces);
            }
        } else if self.layout.template_declaration.uses_source_indent {
            self.layout.template_declaration.angle_depth += angle_delta;
            if self.layout.template_declaration.angle_depth <= 0 && trimmed.ends_with('>') {
                self.layout.template_declaration = TemplateDeclarationState::default();
                self.layout.nesting.clear_continuation_indents();
                self.layout.continuation_indent.clear_next_line();
            }
        }
    }
}

fn angle_depth(line: &str) -> isize {
    let mut depth = 0isize;
    for (_, ch) in angle_chars(line) {
        if ch == '<' {
            depth += 1;
        } else {
            depth = depth.saturating_sub(1);
        }
    }
    depth
}

pub(crate) fn is_template_declaration_head_line(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    let Some(rest) = trimmed.strip_prefix("template") else {
        return false;
    };
    let Some(open_offset) = rest.find('<') else {
        return false;
    };
    let start = "template".len() + open_offset;
    let mut depth = 0isize;
    let mut paren_depth = 0usize;
    let mut saw_open = false;
    for (offset, ch) in trimmed[start..].char_indices() {
        match ch {
            '(' | '[' => paren_depth += 1,
            ')' | ']' => paren_depth = paren_depth.saturating_sub(1),
            '<' if paren_depth == 0 => {
                depth += 1;
                saw_open = true;
            }
            '>' if paren_depth == 0 => {
                depth -= 1;
                if saw_open && depth <= 0 {
                    let end = start + offset + ch.len_utf8();
                    return trimmed[end..].trimmed().is_empty();
                }
            }
            _ => {}
        }
    }
    saw_open && depth > 0
}
