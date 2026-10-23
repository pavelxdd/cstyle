use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{
    ContainsAnyByte, has_hash_outside_literals, is_comment_line,
};
use crate::formatter::text::line_scan::{
    after_leading_comments, trailing_comment_split_limit, unmatched_open_paren_column,
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
/// operator, as in `operator<<`, and those inside parentheses, where a `<`
/// may compare.
fn angle_chars(line: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let chars = line.chars().collect::<Vec<_>>();
    let mut operator_name_end = 0;
    let mut column = 0;
    let mut paren_depth = 0usize;
    std::iter::from_fn(move || {
        while column < chars.len() {
            let at = column;
            column += 1;
            match chars[at] {
                '(' => paren_depth += 1,
                ')' => paren_depth = paren_depth.saturating_sub(1),
                _ => {}
            }
            if paren_depth > 0 {
                continue;
            }
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

/// The indent of the line after a template head `line` leaves open: past
/// the innermost `<`, or, when parens indent, a step in for each open `<`.
pub(crate) fn template_continuation_indent_spaces(
    line: &str,
    angle_step: Option<usize>,
    indent_width: usize,
) -> Option<usize> {
    // Comments leading the line keep the columns of the head after them;
    // a step counts from the line's own indent.
    let code = after_leading_comments(line);
    if code.len() < line.trimmed_start().len() && code.starts_with("template") {
        if let Some(step) = angle_step {
            let code = &code[..trailing_comment_split_limit(code)];
            let depth = angle_chars(code).fold(0usize, |depth, (_, ch)| {
                if ch == '<' {
                    depth + 1
                } else {
                    depth.saturating_sub(1)
                }
            });
            return (depth > 0).then(|| line.len() - line.trimmed_start().len() + depth * step);
        }
        let masked = format!(
            "{}{code}",
            " ".repeat(line[..line.len() - code.len()].chars().count())
        );
        return template_continuation_indent_spaces(&masked, angle_step, indent_width);
    }
    let trimmed = line.trimmed_start();
    let code = line[..trailing_comment_split_limit(line)].trimmed_end();
    if !trimmed.starts_with("template") || angle_depth_delta(code) <= 0 {
        return None;
    }
    if (code.ends_with("||") || code.ends_with("&&"))
        && let Some(open) = unmatched_open_paren_column(code)
    {
        return Some(open + 1);
    }
    let mut stack = Vec::new();
    for (column, ch) in angle_chars(code) {
        if ch == '<' {
            stack.push(column);
        } else {
            stack.pop();
        }
    }
    if let Some(step) = angle_step
        && !stack.is_empty()
    {
        return Some(line.len() - trimmed.len() + stack.len() * step);
    }
    template_rows_continuation_indent_spaces(&[line], indent_width)
}

/// The indent of the row after `rows`, the rows of a template head that
/// leave angles open: past the innermost `<` when code follows it on its
/// row, else an indent past where the angle it stands in puts its rows.
fn template_rows_continuation_indent_spaces(rows: &[&str], indent_width: usize) -> Option<usize> {
    let mut open = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        let code = line[..trailing_comment_split_limit(line)].trimmed_end();
        for (column, ch) in angle_chars(code) {
            if ch == '<' {
                open.push((row, column));
            } else {
                open.pop();
            }
        }
    }
    open.iter()
        .fold(None, |outer: Option<usize>, &(row, column)| {
            let line = rows[row];
            let after = line.chars().skip(column + 1).collect::<String>();
            Some(if after.chars().any(|ch| !ch.is_whitespace()) {
                column + 1 + after.chars().take_while(|ch| ch.is_whitespace()).count()
            } else {
                outer.unwrap_or(line.len() - line.trimmed_start().len()) + indent_width
            })
        })
}

impl FormatEngine<'_> {
    /// The rows of the template head the last code rows in scope continue,
    /// from its `template` row on.
    fn open_template_head_rows(&self) -> Option<Vec<&str>> {
        let mut rows = Vec::new();
        for index in self.output.scoped_range().rev().take(32) {
            let trimmed = self.output.trimmed(index);
            if trimmed.is_empty()
                || is_comment_line(trimmed)
                || self.output.comment_start_index(index) != index
                || self.output.is_directive_line(index)
            {
                continue;
            }
            let line = self.output[index].as_str();
            rows.push(line);
            if trimmed.starts_with("template") {
                rows.reverse();
                return Some(rows);
            }
            if trimmed.ends_with_any(b";{}") {
                return None;
            }
        }
        None
    }

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
        let trimmed = self.output.code_body(index);
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
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            if is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = self.output.code_trimmed_of(line);
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
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with('>') {
            return false;
        }
        for (index, line) in lines.iter().enumerate().skip(1) {
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            if is_template_declaration_head_line(trimmed) {
                let depth: isize = lines[..=index]
                    .iter()
                    .rev()
                    .map(|line| {
                        let code = self.output.code_trimmed_of(line);
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
        // A row in parens the head opened continues them where parens
        // align their rows.
        if !self.layout.template_declaration.uses_source_indent
            || !self.token_input.token_begins_source_line
            || self.layout.nesting.paren_depth > 0 && !self.options.indent_after_parens
        {
            return;
        }
        // Comment lines take no part in the head's layout.
        let previous = self.output.last_code_line_in_scope();
        if previous.is_some_and(|line| has_hash_outside_literals(line)) {
            return;
        }
        let spaces = previous
            .and_then(|line| {
                template_continuation_indent_spaces(
                    line,
                    self.template_angle_step(),
                    self.options.indent_width,
                )
                .or_else(|| {
                    // The angles every row of the head leaves open
                    // hold the row after them, as a call's parens do.
                    self.template_angle_step()
                        .is_none()
                        .then(|| self.open_template_head_rows())
                        .flatten()
                        .and_then(|rows| {
                            template_rows_continuation_indent_spaces(
                                &rows,
                                self.options.indent_width,
                            )
                        })
                })
                .or_else(|| {
                    // Stepping after the angles, a row of the head goes
                    // on from the row before it.
                    let step = self.template_angle_step()?;
                    let code = self.output.code_trimmed_of(line);
                    let lead = leading_visual_width(line, self.options.tab_width);
                    let delta = angle_depth_delta(code);
                    Some(if delta >= 0 {
                        lead + delta.unsigned_abs() * step
                    } else {
                        lead.saturating_sub(delta.unsigned_abs() * step)
                    })
                })
                .or_else(|| {
                    (source_column == 0)
                        .then(|| leading_visual_width(line, self.options.tab_width))
                        .filter(|&spaces| spaces > 0)
                })
            })
            .unwrap_or(source_column);
        self.layout.continuation_indent.set_next_line_spaces(spaces);
        // A comment the line holds alone stands there too.
        self.layout
            .continuation_indent
            .input_line_continuation_indent
            .replace(ContinuationIndent::Spaces(spaces));
    }

    /// The step each open `<` of a template head adds when parens indent.
    pub(crate) fn template_angle_step(&self) -> Option<usize> {
        self.options
            .indent_after_parens
            .then(|| self.options.continuation_indent * self.options.indent_width)
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
        // Comments leading the line keep the columns of the head after them;
        // a comment after its code takes no part in it.
        let line = LineView::new(&line[..trailing_comment_split_limit(line)]);
        let code = after_leading_comments(&line);
        let masked;
        let line = if code.len() < line.trimmed_start().len() && !code.is_empty() {
            masked = format!(
                "{}{code}",
                " ".repeat(visual_width_from(
                    &line[..line.len() - code.len()],
                    0,
                    self.options.tab_width
                ))
            );
            LineView::new(&masked)
        } else {
            line.clone()
        };
        let line = &line;
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
            if let Some(spaces) = template_continuation_indent_spaces(
                line,
                self.template_angle_step(),
                self.options.indent_width,
            ) {
                self.layout.continuation_indent.set_next_line_spaces(spaces);
            }
        } else if self.layout.template_declaration.uses_source_indent {
            self.layout.template_declaration.angle_depth += angle_delta;
            if self.layout.template_declaration.angle_depth <= 0 {
                self.layout.template_declaration = TemplateDeclarationState::default();
                // Code after the head on its row continues on its own.
                if trimmed.ends_with('>') {
                    self.layout.nesting.clear_continuation_indents();
                    self.layout.continuation_indent.clear_next_line();
                }
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
