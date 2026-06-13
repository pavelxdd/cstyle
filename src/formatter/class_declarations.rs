use super::FormatEngine;
use super::columns::{leading_visual_width, visual_width_from};
use super::indentation::LineKind;
use super::line_scan::trailing_comment_split_limit;
use super::syntax::signature_ends_with_parameter_list;
use super::token::Token;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(super) fn has_base_access_token(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token,
            Token::Word(word) if matches!(word.as_str(), "public" | "protected" | "private")
        )
    })
}

pub(super) fn is_split_export_head(line: &str) -> bool {
    if line
        .chars()
        .any(|ch| !is_identifier_continue(ch) && !ch.is_whitespace())
    {
        return false;
    }
    let mut words = line
        .split(|ch: char| !is_identifier_continue(ch))
        .filter(|word| !word.is_empty());
    let Some(kind) = words.next() else {
        return false;
    };
    if !matches!(kind, "class" | "struct" | "union" | "interface") {
        return false;
    }
    let Some(export) = words.next() else {
        return false;
    };
    words.next().is_none()
        && export.len() > 1
        && export
            .chars()
            .all(|ch| ch == '_' || ch.is_ascii_uppercase() || ch.is_ascii_digit())
}

impl FormatEngine<'_> {
    pub(super) fn in_open_class_head(&self) -> bool {
        for index in (0..self.output.len()).rev() {
            let trimmed = self.output.trimmed(index);
            if trimmed.is_empty() {
                continue;
            }
            if trimmed == "{"
                || trimmed.starts_with('}')
                || trimmed.ends_with(';')
                || trimmed.ends_with('{')
            {
                return false;
            }
            if matches!(trimmed, "class" | "struct" | "union" | "interface")
                || is_split_export_head(trimmed)
            {
                return true;
            }
        }
        false
    }

    pub(super) fn current_opens_class_base_clause(&self) -> bool {
        if self.stack_state.has_question_in_current_brace() {
            return false;
        }
        if self.code_opens_class_base_clause(self.current.trim_end()) {
            return true;
        }
        if self.split_class_export_pending_base {
            return true;
        }
        let current = self.current.trim();
        let single_name = current.chars().next().is_some_and(is_identifier_start)
            && current.chars().all(is_identifier_continue);
        single_name
            && (self
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|line| is_split_export_head(line.trim()))
                || self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| is_split_export_head(line.trim()))
                || self.in_open_class_head())
    }

    pub(super) fn code_opens_class_base_clause(&self, before: &str) -> bool {
        if before.is_empty() || before.ends_with(':') || before.contains('?') {
            return false;
        }
        if signature_ends_with_parameter_list(before) {
            return false;
        }
        let statement = before
            .rsplit([';', '{', '}'])
            .next()
            .unwrap_or(before)
            .trim_start();
        statement
            .split(|ch: char| !is_identifier_continue(ch))
            .any(|word| matches!(word, "class" | "struct" | "union" | "interface"))
    }

    pub(super) fn colon_leads_class_base_clause(&self) -> bool {
        if self.current_opens_class_base_clause() {
            return true;
        }
        if self.stack_state.has_question_in_current_brace()
            || !self.current[..self.current_trailing_comment_split_limit()]
                .trim()
                .is_empty()
        {
            return false;
        }
        let Some(line) = self.output.iter().rev().find(|line| {
            let trimmed = line.trim_start();
            !trimmed.is_empty() && !trimmed.starts_with(['#', ':', ','])
        }) else {
            return false;
        };
        let code = &line[..trailing_comment_split_limit(line)];
        self.code_opens_class_base_clause(code.trim_end())
    }

    pub(super) fn try_join_class_base_line(&mut self, line: &str) -> bool {
        if !self.may_have_class_base_access {
            return false;
        }
        let current = line.trim_start();
        if !(current.starts_with("public ")
            || current.starts_with("protected ")
            || current.starts_with("private "))
        {
            return false;
        }
        if !self
            .output
            .last()
            .is_some_and(|previous| previous.trim_end().ends_with(':'))
        {
            return false;
        }
        let before_previous = self.output.len().saturating_sub(1);
        let in_class_head = self.output[..before_previous]
            .iter()
            .rev()
            .take_while(|line| {
                let trimmed = line.trim();
                trimmed != "{" && !trimmed.starts_with("};")
            })
            .any(|line| {
                let trimmed = line.trim();
                matches!(trimmed, "class" | "struct" | "union") || is_split_export_head(trimmed)
            });
        if !in_class_head {
            return false;
        }
        let Some(previous) = self.output.last_mut() else {
            return false;
        };
        previous.push(' ');
        previous.push_str(current);
        self.previous_pre_adjust_line = Some(previous.clone());
        true
    }

    pub(super) fn prepare_split_class_head_continuation(&mut self) {
        if !self.token_input.token_begins_source_line
            || !self.current.is_empty()
            || !self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| is_split_export_head(line.trim()))
        {
            return;
        }
        self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
        self.continuation_indent.next_line_indent_spaces = None;
        self.split_class_export_pending_base = true;
    }

    pub(super) fn finish_split_class_head_line(&mut self) {
        let header_indent = self.state.indent();
        self.finish_line();
        self.stack_state.clear_continuation_indents();
        self.continuation_indent.next_line_indent = Some(header_indent + 1);
        self.continuation_indent.next_line_indent_spaces = None;
        self.split_class_export_pending_base = true;
        self.previous_was_newline = true;
    }

    pub(super) fn split_class_head_indent_spaces(&self, current: &str) -> Option<usize> {
        if current == "{" || current == ";" || current.starts_with("};") {
            return None;
        }
        for line in self.output.iter().rev().take(8) {
            let trimmed = line.trim();
            if trimmed.contains('{') || trimmed.starts_with("};") || trimmed.ends_with(';') {
                break;
            }
            if matches!(trimmed, "class" | "struct" | "union") || is_split_export_head(trimmed) {
                return Some(
                    leading_visual_width(line, self.options.tab_width) + self.options.indent_width,
                );
            }
        }
        None
    }

    pub(super) fn simple_template_base_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with(':') {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        (previous.trim_start().starts_with("struct ")
            && previous.contains(" <")
            && previous.contains('>'))
        .then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width * 3
        })
    }

    pub(super) fn commented_class_head_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line
            .trim_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        previous.trim_start().starts_with("class //").then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width
        })
    }

    pub(super) fn class_base_logical_operand_indent_spaces(
        &self,
        line: &str,
        kind: LineKind,
    ) -> Option<usize> {
        if kind != LineKind::Normal || !line.trim_start().starts_with("sizeof(") {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        ((previous_trimmed.starts_with("struct ")
            || previous_trimmed.starts_with("class ")
            || previous_trimmed.starts_with("union "))
            && previous_code.contains(':')
            && (previous_code.ends_with("&&") || previous_code.ends_with("||")))
        .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(super) fn template_base_colon_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !current.starts_with(':')
            || !(previous_trimmed.starts_with("struct ") || previous_trimmed.starts_with("class "))
            || max_template_angle_depth(previous_code) <= 1
        {
            return None;
        }
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        if previous_code.contains(',') && previous_code.contains("sizeof(") {
            return Some(previous_indent);
        }
        if has_outer_template_comma(previous_code) && !previous_code.contains(" < ") {
            return Some(previous_indent + self.options.indent_width);
        }
        let aligned =
            previous_indent + visual_width_from(previous_trimmed, 0, self.options.tab_width) + 2;
        Some(
            if aligned.saturating_sub(previous_indent) > self.options.max_continuation_indent {
                previous_indent + self.options.indent_width * 3
            } else {
                aligned
            },
        )
    }
}

fn max_template_angle_depth(line: &str) -> usize {
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for ch in line.chars() {
        match ch {
            '<' => {
                depth += 1;
                max_depth = max_depth.max(depth);
            }
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max_depth
}

fn has_outer_template_comma(line: &str) -> bool {
    let mut angle_depth = 0usize;
    let mut paren_depth = 0usize;
    for ch in line.chars() {
        match ch {
            '(' | '[' => paren_depth += 1,
            ')' | ']' => paren_depth = paren_depth.saturating_sub(1),
            '<' if paren_depth == 0 => angle_depth += 1,
            '>' if paren_depth == 0 => angle_depth = angle_depth.saturating_sub(1),
            ',' if paren_depth == 0 && angle_depth == 1 => return true,
            _ => {}
        }
    }
    false
}
