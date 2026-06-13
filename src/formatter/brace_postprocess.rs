use super::FormatEngine;
use super::brace_classification::{is_lambda_body_header, is_namespace_or_module_block_header};
use super::columns::{leading_visual_width, visual_width_from};
use super::indentation::LineKind;
use super::labels;
use super::line_scan::{line_ends_with_comment, trailing_comment_split_limit};
use super::preprocessor::preprocessor_directive;
use super::switch_cases;
use super::token::{self, Token};
use crate::config::{BraceStyle, FormatOptions, IndentStyle};

pub(super) struct MaxLengthBraceRowLayout {
    pub(super) first_width: usize,
    pub(super) attaches_lisp_closer: bool,
}

pub(super) fn postprocess_brace_style(output: String, options: &FormatOptions) -> String {
    match options.brace_style {
        BraceStyle::Pico => {
            let run_in = run_in_horstmann_opening_braces(&output, options);
            attach_lisp_closing_braces(&run_in, options.line_break())
        }
        BraceStyle::Lisp => attach_lisp_closing_braces(&output, options.line_break()),
        BraceStyle::Horstmann => run_in_horstmann_opening_braces(&output, options),
        _ => output,
    }
}

impl FormatEngine<'_> {
    pub(super) fn merge_source_run_in_braces(&mut self) {
        let mut indices = std::mem::take(&mut self.source_run_in_brace_lines);
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            let Some(next_line) = self.output.get(index + 1) else {
                continue;
            };
            if next_line.trim().is_empty() || self.output[index].trim() != "{" {
                continue;
            }
            let next_line = self.output.remove(index + 1);
            let fill = horstmann_run_in_fill(&self.output[index], &next_line, self.options);
            let merged = format!("{}{}{}", self.output[index], fill, next_line.trim_start());
            self.output.set(index, merged);
        }
    }

    pub(super) fn replayed_lisp_attached_suffix_indent_spaces(&self) -> Option<usize> {
        if self.options.max_code_length.is_none()
            || !matches!(
                self.options.brace_style,
                BraceStyle::Pico | BraceStyle::Lisp
            )
            || !self.output.last_non_empty_line().is_some_and(|previous| {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.trim_start().starts_with('}') && code.ends_with(',')
            })
        {
            return None;
        }
        let spaces = self.state.indent() * self.options.indent_width;
        (spaces == self.token_input.input_source_indent).then_some(spaces)
    }

    pub(super) fn max_length_brace_row_layout(
        &self,
        line: &str,
        structural_level: usize,
        base_indent_width: usize,
        width: usize,
    ) -> MaxLengthBraceRowLayout {
        let attaches_lisp_closer = matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) && line.trim_start().starts_with('}')
            && self.output.last().is_some_and(|previous| {
                !previous.trim().is_empty()
                    && preprocessor_directive(previous.trim_start()).is_none()
                    && !line_ends_with_comment(previous)
            });
        let first_width = if attaches_lisp_closer && let Some(previous) = self.output.last() {
            let separator_width = usize::from(!previous.trim_end().ends_with('{'));
            width
                .saturating_sub(
                    visual_width_from(previous, 0, self.options.tab_width) + separator_width,
                )
                .max(1)
        } else if matches!(
            self.options.brace_style,
            BraceStyle::Horstmann | BraceStyle::Pico
        ) && let Some(brace) = self.output.last()
            && brace.trim() == "{"
            && !line.trim_start().starts_with(['#', '}'])
            && !line.trim_start().starts_with("//")
            && !line.contains("*INDENT-OFF*")
            && !labels::is_access_label(line, &self.options.access_labels)
            && !self.output[..self.output.len() - 1]
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| is_namespace_or_module_block_header(line))
        {
            let prefix = self
                .options
                .continuation_indent_prefix(structural_level, base_indent_width);
            let next = format!("{prefix}{}", line.trim_start());
            let fill = horstmann_run_in_fill(brace, &next, self.options);
            let run_in_width = format!("{brace}{fill}").len();
            let mut has_word_logical = false;
            let mut has_symbol_logical = false;
            for token in token::tokenize(line) {
                match token {
                    Token::Word(word) if matches!(word.as_str(), "and" | "or") => {
                        has_word_logical = true;
                    }
                    Token::Operator(operator) if matches!(operator.as_str(), "&&" | "||") => {
                        has_symbol_logical = true;
                    }
                    _ => {}
                }
            }
            let strict_logical_boundary =
                has_word_logical || (self.options.break_after_logical && has_symbol_logical);
            width
                .saturating_sub(run_in_width + usize::from(strict_logical_boundary))
                .max(1)
        } else {
            width
        };
        MaxLengthBraceRowLayout {
            first_width,
            attaches_lisp_closer,
        }
    }

    pub(super) fn try_emit_whitesmith_lambda_close(&mut self, line: &str) -> bool {
        if !(matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) && line.trim() == "};"
            && self
                .output
                .iter()
                .rev()
                .take(4)
                .any(|line| is_lambda_body_header(line.trim_end())))
        {
            return false;
        }
        let base = self.state.line_indent(LineKind::Normal, self.options);
        let indent = if self.options.brace_style == BraceStyle::Vtk && base == 0 {
            base
        } else {
            base + 1
        };
        self.push_output_line(line.trim(), indent);
        true
    }

    pub(super) fn try_split_lambda_body_header(&mut self, line: &str) -> bool {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Horstmann
        ) {
            return false;
        }
        let Some(open) = line.rfind('{') else {
            return false;
        };
        if !(is_lambda_body_header(line[..open].trim_end()) && {
            let head = line[..open].trim_end();
            let one_line_body = line[open + 1..].contains('}');
            !(one_line_body && (!self.options.break_one_line_blocks || head.contains("->")))
        }) {
            return false;
        }
        self.finish_line_text(line[..open].trim_end());
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) {
            let base = self.state.line_indent(LineKind::Normal, self.options);
            self.push_output_line("{", base + 1);
        } else {
            self.finish_line_text("{");
        }
        true
    }

    pub(super) fn try_split_operator_body(&mut self, line: &str) -> bool {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Whitesmith
                | BraceStyle::Horstmann
        ) {
            return false;
        }
        let Some(open) = line.rfind('{') else {
            return false;
        };
        let Some(close) = line.rfind('}') else {
            return false;
        };
        if !(open < close
            && line[..open].contains("operator")
            && line[..open].trim_end().ends_with(')'))
        {
            return false;
        }
        let head = line[..open].trim_end();
        let body = line[open + 1..close].trim();
        let base = self.state.line_indent(LineKind::Normal, self.options);
        if self.options.brace_style == BraceStyle::Horstmann {
            self.push_output_line(head, base);
            if !body.is_empty() {
                self.push_output_line(&format!("{{   {body}"), base);
            } else {
                self.push_output_line("{", base);
            }
            self.push_output_line("}", base);
            return true;
        }
        let brace_indent = if self.options.brace_style == BraceStyle::Whitesmith {
            base + 1
        } else {
            base
        };
        self.push_output_line(head, base);
        self.push_output_line("{", brace_indent);
        if !body.is_empty() {
            self.push_output_line(body, base + 1);
        }
        self.push_output_line("}", brace_indent);
        true
    }
}

fn split_output_lines<'a>(output: &'a str, line_break: &str) -> Vec<&'a str> {
    let mut lines: Vec<&str> = output.split(line_break).collect();
    if lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }
    lines
}

fn raw_literal_lines(output: &str, line_break: &str) -> Vec<bool> {
    let mut raw_lines = vec![false; split_output_lines(output, line_break).len()];
    let mut line_index = 0usize;
    for token in token::tokenize(output) {
        let text = token::token_text(&token);
        let line_breaks = text.bytes().filter(|byte| *byte == b'\n').count();
        if matches!(&token, Token::StringLiteral(literal) if ["u8R\"", "LR\"", "uR\"", "UR\"", "R\""]
            .into_iter()
            .any(|prefix| literal.starts_with(prefix)))
        {
            let end = line_index.saturating_add(line_breaks);
            for raw_line in raw_lines
                .iter_mut()
                .take(end.saturating_add(1))
                .skip(line_index.saturating_add(1))
            {
                *raw_line = true;
            }
        }
        line_index = line_index.saturating_add(line_breaks);
    }
    raw_lines
}

fn attach_lisp_closing_braces(output: &str, line_break: &str) -> String {
    let raw_lines = raw_literal_lines(output, line_break);
    let mut lines: Vec<String> = Vec::new();
    for (index, line) in split_output_lines(output, line_break)
        .into_iter()
        .enumerate()
    {
        let trimmed = line.trim();
        if !raw_lines[index]
            && trimmed.starts_with('}')
            && let Some(previous) = lines.last_mut()
            && !previous.trim().is_empty()
            && preprocessor_directive(previous.trim_start()).is_none()
            && !previous.trim_end().ends_with('\\')
            && !line_ends_with_comment(previous)
        {
            if !previous.trim_end().ends_with('{') {
                previous.push(' ');
            }
            previous.push_str(trimmed);
        } else {
            lines.push(line.to_string());
        }
    }
    finish_postprocessed_lines(lines, line_break, output.ends_with(line_break))
}

fn run_in_horstmann_opening_braces(output: &str, options: &FormatOptions) -> String {
    let line_break = options.line_break();
    let raw_lines = raw_literal_lines(output, line_break);
    let mut lines = Vec::new();
    let input = split_output_lines(output, line_break);
    let mut index = 0usize;
    while index < input.len() {
        let line = input[index];
        if !raw_lines[index]
            && line.trim() == "{"
            && let Some(next) = input.get(index + 1)
            && !next.trim().is_empty()
            && !next.trim_start().starts_with('#')
            && !next.trim_start().starts_with('}')
            && !next.starts_with("//")
            && !next.contains("*INDENT-OFF*")
            && !run_in_next_line_is_access_label(line, next, options)
            && !previous_line_is_namespace_header(&input, index)
        {
            let fill = horstmann_run_in_fill(line, next, options);
            let next = next.trim_start();
            let next = if options.strip_comment_prefix {
                next.strip_prefix("/*  ")
                    .map_or_else(|| next.to_string(), |rest| format!("/* {rest}"))
            } else {
                next.to_string()
            };
            lines.push(format!("{line}{fill}{next}"));
            index += 2;
        } else {
            lines.push(line.to_string());
            index += 1;
        }
    }
    finish_postprocessed_lines(lines, line_break, output.ends_with(line_break))
}

fn run_in_next_line_is_access_label(
    brace_line: &str,
    next_line: &str,
    options: &FormatOptions,
) -> bool {
    labels::is_access_label(next_line, &options.access_labels)
        && leading_visual_width(next_line, options.tab_width)
            <= leading_visual_width(brace_line, options.tab_width)
}

fn previous_line_is_namespace_header(input: &[&str], before: usize) -> bool {
    input[..before]
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| is_namespace_or_module_block_header(line))
}

pub(super) fn horstmann_run_in_fill(
    brace_line: &str,
    next_line: &str,
    options: &FormatOptions,
) -> String {
    let switch_label = switch_cases::find_case_colon(next_line.trim_start()).is_some();
    if !matches!(options.indent_style, IndentStyle::ForceTabs)
        && labels::is_access_label(next_line, &options.access_labels)
    {
        let tab_width = options.tab_width.max(1);
        let brace_column = leading_visual_width(brace_line, tab_width);
        let target_column = leading_visual_width(next_line, tab_width);
        if options.indent_style == IndentStyle::Tabs {
            let mut fill = String::new();
            let mut column = brace_column + 1;
            while column < target_column {
                let next_stop = (column / tab_width + 1) * tab_width;
                if next_stop > target_column {
                    break;
                }
                fill.push('\t');
                column = next_stop;
            }
            fill.push_str(&" ".repeat(target_column.saturating_sub(column)));
            return if fill.is_empty() {
                " ".to_string()
            } else {
                fill
            };
        }
        return " ".repeat(target_column.saturating_sub(brace_column + 1).max(1));
    }
    match options.indent_style {
        IndentStyle::Spaces => {
            let brace_column = leading_visual_width(brace_line, options.tab_width.max(1));
            let target_column = if switch_label {
                brace_column + options.indent_width
            } else {
                leading_visual_width(next_line, options.tab_width.max(1))
            };
            " ".repeat(
                target_column
                    .saturating_sub(brace_column + 1)
                    .max(options.indent_width.saturating_sub(1)),
            )
        }
        IndentStyle::Tabs => "\t".to_string(),
        IndentStyle::ForceTabs => {
            let tab_width = options.tab_width.max(1);
            let brace_column = leading_visual_width(brace_line, tab_width);
            let target_column = if switch_label {
                brace_column + options.indent_width.max(1)
            } else {
                leading_visual_width(next_line, tab_width)
                    .max(brace_column + options.indent_width.max(1))
            };
            if options.tab_width > options.indent_width {
                return " ".repeat(target_column.saturating_sub(brace_column + 1));
            }
            let mut fill = String::new();
            let mut column = brace_column + 1;
            while column < target_column {
                let next_stop = (column / tab_width + 1) * tab_width;
                if next_stop <= target_column {
                    fill.push('\t');
                    column = next_stop;
                } else {
                    break;
                }
            }
            fill.push_str(&" ".repeat(target_column.saturating_sub(column)));
            fill
        }
    }
}

fn finish_postprocessed_lines(
    lines: Vec<String>,
    line_break: &str,
    trailing_break: bool,
) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut output = lines.join(line_break);
    if trailing_break {
        output.push_str(line_break);
    }
    output
}
