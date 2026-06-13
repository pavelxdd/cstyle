use super::FormatEngine;
use super::columns::leading_visual_width;
use super::line_scan::{trailing_comment_split_limit, unmatched_open_paren_column};

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub(super) struct TemplateDeclarationState {
    uses_source_indent: bool,
    angle_depth: isize,
}

pub(super) fn template_declaration_line_complete(line: &str) -> bool {
    line.ends_with('>') && angle_depth(line) <= 0
}

pub(super) fn angle_depth_delta(line: &str) -> isize {
    line.chars().fold(0, |depth, ch| match ch {
        '<' => depth + 1,
        '>' => depth - 1,
        _ => depth,
    })
}

pub(super) fn template_continuation_indent_spaces(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("template") || angle_depth_delta(trimmed) <= 0 {
        return None;
    }
    let code = line[..trailing_comment_split_limit(line)].trim_end();
    if (code.ends_with("||") || code.ends_with("&&"))
        && let Some(open) = unmatched_open_paren_column(code)
    {
        return Some(open + 1);
    }
    let mut stack = Vec::new();
    for (column, ch) in line.chars().enumerate() {
        match ch {
            '<' => stack.push(column),
            '>' => {
                stack.pop();
            }
            _ => {}
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
    pub(super) fn is_template_declaration_line(&self) -> bool {
        self.current.trim_start().starts_with("template")
    }

    pub(super) fn is_complete_template_declaration_line(&self) -> bool {
        let current = self.current.trim();
        current.starts_with("template")
            && current.ends_with('>')
            && template_declaration_line_complete(current)
    }

    pub(super) fn is_template_declaration_head_line(&self, line: &str) -> bool {
        let trimmed = line.trim_start();
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
                        return trimmed[end..].trim().is_empty();
                    }
                }
                _ => {}
            }
        }
        saw_open && depth > 0
    }

    pub(super) fn previous_output_is_complete_template_declaration(&self) -> bool {
        let Some(index) = self.output.last_non_empty_index() else {
            return false;
        };
        let line = &self.output[index];
        if !line.trim_end().ends_with('>') {
            return false;
        }
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        let trimmed = code.trim_start();
        self.is_template_declaration_head_line(trimmed)
            && template_declaration_line_complete(trimmed)
            && !trimmed.ends_with(';')
    }

    pub(super) fn previous_output_closes_multiline_template_declaration(&self) -> bool {
        let Some(index) = self.output.last_non_empty_index() else {
            return false;
        };
        let previous = &self.output[index];
        if !previous.trim_end().ends_with('>') {
            return false;
        }
        let lines: Vec<&str> = self.output[..=index]
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(16)
            .map(String::as_str)
            .collect();
        for (index, line) in lines.iter().enumerate().skip(1) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if self.is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
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
            .filter(|line| !line.trim().is_empty())
            .take(16)
            .map(String::as_str)
            .collect();
        let Some(previous) = lines.first() else {
            return false;
        };
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with('>') {
            return false;
        }
        for (index, line) in lines.iter().enumerate().skip(1) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if self.is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
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

    pub(super) fn prepare_template_continuation_token_indent(&mut self, source_column: usize) {
        if !self.template_declaration.uses_source_indent
            || !self.token_input.token_begins_source_line
        {
            return;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty());
        if previous.is_some_and(|line| line.contains('#')) {
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
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = Some(spaces);
    }

    pub(super) fn template_continuation_active(&self) -> bool {
        self.template_declaration.uses_source_indent
    }

    pub(super) fn template_continuation_closes_on_line(&self, line: &str) -> bool {
        self.template_declaration.uses_source_indent
            && line.ends_with('>')
            && self.template_declaration.angle_depth + angle_depth_delta(line) <= 0
    }

    pub(super) fn template_continuation_line_indent_spaces(&self, line: &str) -> Option<usize> {
        if !self.template_declaration.uses_source_indent {
            return None;
        }
        let mut spaces = self
            .output
            .iter()
            .rev()
            .find(|line| {
                line[..trailing_comment_split_limit(line)]
                    .trim_start()
                    .starts_with("template <")
            })
            .filter(|line| line[..trailing_comment_split_limit(line)].trim() == "template <")
            .map(|line| {
                leading_visual_width(line, self.options.tab_width) + self.options.indent_width
            });
        if self.template_continuation_closes_on_line(line.trim())
            && line.trim() == ">"
            && let Some(previous) = self.output.last_non_empty_line()
        {
            spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        spaces
    }

    pub(super) fn observe_template_declaration_line(&mut self, line: &str) {
        let trimmed = line.trim();
        let angle_delta = angle_depth_delta(trimmed);
        if trimmed.starts_with("template")
            && angle_delta > 0
            && !trimmed.ends_with(';')
            && !template_declaration_line_complete(trimmed)
        {
            self.template_declaration.uses_source_indent = true;
            self.template_declaration.angle_depth = angle_delta;
            if let Some(spaces) = template_continuation_indent_spaces(line) {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        } else if self.template_declaration.uses_source_indent {
            self.template_declaration.angle_depth += angle_delta;
            if self.template_declaration.angle_depth <= 0 && trimmed.ends_with('>') {
                self.template_declaration = TemplateDeclarationState::default();
                self.stack_state.clear_continuation_indents();
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            }
        }
    }
}

fn angle_depth(line: &str) -> isize {
    let mut depth = 0isize;
    for ch in line.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth
}
