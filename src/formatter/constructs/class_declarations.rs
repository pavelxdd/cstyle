use crate::config::FormatOptions;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::syntax::signature_ends_with_parameter_list;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{ContainsAnyByte, trailing_comment_split_limit};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(crate) fn has_base_access_token(tokens: &[Token]) -> bool {
    tokens.iter().any(|token| {
        matches!(
            token,
            Token::Word(word) if matches!(word.as_str(), "public" | "protected" | "private")
        )
    })
}

pub(crate) fn is_split_export_head(line: &str) -> bool {
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
    fn in_open_class_head(&self) -> bool {
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

    pub(crate) fn current_opens_class_base_clause(&self) -> bool {
        if self.layout.nesting.has_question_in_current_brace() {
            return false;
        }
        if code_opens_class_base_clause(self.current.trimmed_end()) {
            return true;
        }
        if self.layout.split_class_export_pending_base {
            return true;
        }
        let current = self.current.trimmed();
        let single_name = current.chars().next().is_some_and(is_identifier_start)
            && current.chars().all(is_identifier_continue);
        single_name
            && (self
                .layout
                .previous_pre_adjust_line
                .as_ref()
                .is_some_and(|line| is_split_export_head(line.trimmed()))
                || self
                    .output
                    .last_non_empty_scoped()
                    .is_some_and(|line| is_split_export_head(line.trimmed()))
                || self.in_open_class_head())
    }

    pub(crate) fn colon_leads_class_base_clause(&self) -> bool {
        if self.current_opens_class_base_clause() {
            return true;
        }
        if self.layout.nesting.has_question_in_current_brace()
            || !self.current[..self.current_trailing_comment_split_limit()]
                .trimmed()
                .is_empty()
        {
            return false;
        }
        let Some(line) = self.output.scoped().iter().rev().find(|line| {
            let trimmed = line.trimmed_start();
            !trimmed.is_empty() && !trimmed.starts_with_any(b"#:,")
        }) else {
            return false;
        };
        let code = &self.output.code_of(line);
        code_opens_class_base_clause(code.trimmed_end())
    }

    pub(crate) fn try_join_class_base_line(&mut self, line: &LineView<'_>) -> bool {
        if !self.may_have_class_base_access {
            return false;
        }
        let current = line.trimmed_start();
        if !(current.starts_with("public ")
            || current.starts_with("protected ")
            || current.starts_with("private "))
        {
            return false;
        }
        if !self
            .output
            .last()
            .is_some_and(|previous| previous.trimmed_end().ends_with(':'))
        {
            return false;
        }
        let before_previous = self.output.len().saturating_sub(1);
        let in_class_head = self.output[..before_previous]
            .iter()
            .rev()
            .take_while(|line| {
                let trimmed = line.trimmed();
                trimmed != "{" && !trimmed.starts_with("};")
            })
            .any(|line| {
                let trimmed = line.trimmed();
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
        self.layout.previous_pre_adjust_line = Some(previous.clone());
        true
    }

    pub(crate) fn prepare_split_class_head_continuation(&mut self) {
        if !self.token_input.token_begins_source_line
            || !self.current.is_empty()
            || !self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| is_split_export_head(line.trimmed()))
        {
            return;
        }
        self.layout
            .continuation_indent
            .set_next_line_level(self.statement_level() + 1);
        self.layout.split_class_export_pending_base = true;
    }

    pub(crate) fn finish_split_class_head_line(&mut self) {
        let header_indent = self.layout.indentation.indent();
        self.finish_line();
        self.layout.nesting.clear_continuation_indents();
        self.layout
            .continuation_indent
            .set_next_line_level(header_indent + 1);
        self.layout.split_class_export_pending_base = true;
        self.previous_was_newline = true;
    }

    pub(crate) fn split_class_head_indent_spaces(&self, current: &str) -> Option<usize> {
        if current == "{" || current == ";" || current.starts_with("};") {
            return None;
        }
        let stops = |trimmed: &str| {
            trimmed.ends_with(';') || trimmed.starts_with("};") || trimmed.contains('{')
        };
        let range = self.output.scoped_range();
        // The last of the last 8 lines that stops the look or is a class
        // head decides.
        let index = self.output.last_line_looked(
            &self.class_head_look,
            range.start.max(range.end.saturating_sub(8)),
            range.end,
            |index| {
                let trimmed = self.output.trimmed(index);
                stops(trimmed)
                    || matches!(trimmed, "class" | "struct" | "union")
                    || is_split_export_head(trimmed)
            },
        )?;
        (!stops(self.output.trimmed(index))).then(|| {
            leading_visual_width(&self.output[index], self.options.tab_width)
                + self.options.indent_width
        })
    }

    pub(crate) fn simple_template_base_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if !line.trimmed_start().starts_with(':') {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        (previous.trimmed_start().starts_with("struct ")
            && previous.contains(" <")
            && previous.contains('>'))
        .then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width * 3
        })
    }

    pub(crate) fn commented_class_head_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if !line
            .trimmed_start()
            .chars()
            .next()
            .is_some_and(is_identifier_start)
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        previous.trimmed_start().starts_with("class //").then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width
        })
    }

    pub(crate) fn class_base_logical_operand_indent_spaces(
        &self,
        line: &LineView<'_>,
        kind: LineKind,
    ) -> Option<usize> {
        if kind != LineKind::Normal || !line.trimmed_start().starts_with("sizeof(") {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        ((previous_trimmed.starts_with("struct ")
            || previous_trimmed.starts_with("class ")
            || previous_trimmed.starts_with("union "))
            && previous_code.contains(':')
            && (previous_code.ends_with("&&") || previous_code.ends_with("||")))
        .then(|| leading_visual_width(previous, self.options.tab_width))
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

pub(crate) fn code_opens_class_base_clause(before: &str) -> bool {
    if before.is_empty() || before.ends_with(':') || before.contains('?') {
        return false;
    }
    if signature_ends_with_parameter_list(before) {
        return false;
    }
    let statement = crate::formatter::text::line_scan::statement_tail(before).trimmed_start();
    statement
        .split(|ch: char| !is_identifier_continue(ch))
        .any(|word| matches!(word, "class" | "struct" | "union" | "interface"))
}

pub(crate) fn template_base_colon_indent_spaces(
    options: &FormatOptions,
    current: &str,
    previous: &str,
) -> Option<usize> {
    if !current.starts_with(':') {
        return None;
    }
    let previous_code = previous[..trailing_comment_split_limit(previous)].trimmed_end();
    let previous_trimmed = previous_code.trimmed_start();
    if !(previous_trimmed.starts_with("struct ") || previous_trimmed.starts_with("class "))
        || max_template_angle_depth(previous_code) <= 1
    {
        return None;
    }
    let previous_indent = leading_visual_width(previous, options.tab_width);
    if previous_code.contains(',') && previous_code.contains("sizeof(") {
        return Some(previous_indent);
    }
    if has_outer_template_comma(previous_code) && !previous_code.contains(" < ") {
        return Some(previous_indent + options.indent_width);
    }
    let aligned = previous_indent + visual_width_from(previous_trimmed, 0, options.tab_width) + 2;
    Some(
        if aligned.saturating_sub(previous_indent) > options.max_continuation_indent {
            previous_indent + options.indent_width * 3
        } else {
            aligned
        },
    )
}
