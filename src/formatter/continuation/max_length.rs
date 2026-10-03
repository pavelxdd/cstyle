use crate::config::BraceStyle;
use crate::formatter::braces::classification::is_lambda_capture_header;
use crate::formatter::constructs::constructor_initializers::{
    advance_max_length_constructor_replay, start_max_length_constructor_replay,
};
use crate::formatter::constructs::headers::is_conditional_header_line;
use crate::formatter::constructs::labels::max_length_inline_access_body_indent_extra;
use crate::formatter::constructs::switch_cases::max_length_inline_case_body_indent_extra;
use crate::formatter::continuation::{ContinuationIndent, min_conditional_indent_spaces};
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, token_text, tokenize};
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::structure::blocks::is_code_token;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::language::{self, is_non_type_keyword, is_pointer_type_word};
use crate::formatter::syntax::{
    TemplateAngle, function_name_start, scoped_name_is_constructor, template_angle_role,
};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    advance_quoted_literal, trailing_comment_split_limit, unmatched_open_bracket_column,
    unmatched_open_paren_column, unmatched_open_paren_columns,
};
use crate::formatter::tokens::operators::head_ends_assignment_operator;
use crate::formatter::tokens::pointers::is_pointer_declaration_segment;
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

#[derive(Default)]
pub(crate) struct MaxLengthLineState {
    suffix_width: usize,
    objc_message_indent_spaces: Option<usize>,
}

impl MaxLengthLineState {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn suffix_width(&self) -> usize {
        self.suffix_width
    }

    pub(crate) fn set_suffix_width(&mut self, width: usize) {
        self.suffix_width = width;
    }

    fn objc_message_indent_spaces(&self) -> Option<usize> {
        self.objc_message_indent_spaces
    }

    pub(crate) fn set_objc_message_indent_spaces(&mut self, spaces: Option<usize>) {
        self.objc_message_indent_spaces = spaces;
    }
}

impl FormatEngine<'_> {
    pub(crate) fn push_formatted_line_with_indent(
        &mut self,
        line: &str,
        structural_level: usize,
        indent: ContinuationIndent,
        continuation_indent: ContinuationIndent,
    ) {
        let Some(max_code_length) = self.options.max_code_length else {
            self.push_output_line_with_indent(line, structural_level, indent);
            return;
        };
        // Rows of an initializer stay whole, as astyle keeps them.
        let initializer_row = self
            .output
            .pending_tokens()
            .filter(|span| span.first < self.tree.tokens.len())
            .and_then(|span| self.tree.groups.enclosing(span.first))
            .and_then(|group| {
                self.tree
                    .groups
                    .ancestors(group)
                    .find(|&id| self.tree.groups.get(id).delimiter == Delimiter::Brace)
            })
            .is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
            });
        if should_skip_split(line) || initializer_row {
            self.push_output_line_with_indent(line, structural_level, indent);
            return;
        }
        let width = max_code_length.max(1);
        let configured_indent_width = indent.columns(self.options.indent_width);
        let prefix = match indent {
            ContinuationIndent::Level(level) => self.options.indent_prefix(level),
            ContinuationIndent::Spaces(spaces) => self
                .options
                .continuation_indent_prefix(structural_level, spaces),
        };
        let mut line_adjuster = self.layout.line_adjuster.clone();
        let adjusted = line_adjuster.adjust_line(format!("{prefix}{line}"));
        let base_indent_width =
            configured_indent_width.max(leading_visual_width(&adjusted, self.options.tab_width));
        let brace_row_layout =
            self.max_length_brace_row_layout(line, structural_level, base_indent_width, width);
        let first_width = brace_row_layout.first_width;
        let suffix_width = if line.trim_end().ends_with(';') {
            self.max_length_line.suffix_width()
        } else {
            0
        };
        let final_first_width = first_width.saturating_sub(suffix_width).max(1);
        let Some(split) = split_result(line, first_width, self.options.break_after_logical)
            .or_else(|| {
                (suffix_width > 0).then(|| {
                    split_result(line, final_first_width, self.options.break_after_logical)
                })?
            })
        else {
            self.push_output_line_with_indent(line, structural_level, indent);
            return;
        };
        let current_line_owner = if brace_row_layout.attaches_lisp_closer
            || configured_indent_width > structural_level * self.options.indent_width
            || (self.header_paren.depth.is_some() && !is_conditional_header_line(line))
        {
            indent
        } else {
            continuation_indent
        };
        let split_indent = if split.anchor_column.is_some() {
            split.indent
        } else {
            current_line_owner
        };
        let break_lambda_parameters = matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Horstmann
        );
        let split_indent_inputs = SplitIndentInputs {
            base_indent_width,
            limit_base_indent_width: configured_indent_width,
            current_indent_width: base_indent_width,
            indent_width: self.options.indent_width,
            max_continuation_indent: self.options.max_continuation_indent,
            configured_continuation_spaces: self.options.continuation_indent
                * self.options.indent_width,
            min_conditional_spaces: min_conditional_indent_spaces(self.options),
            configured_indent: current_line_owner,
            indent_after_parens: self.options.indent_after_parens,
            following_split: false,
            break_lambda_parameters,
        };
        let mut next_indent = continuation_indent_for_split(line, &split, &split_indent_inputs)
            .unwrap_or(split_indent);
        if brace_row_layout.attaches_lisp_closer {
            next_indent = ContinuationIndent::Level(self.layout.indentation.indent());
        }
        let conditional_floor =
            self.maximum_length_conditional_continuation_floor(line, base_indent_width);
        if let Some(spaces) = self.max_length_line.objc_message_indent_spaces() {
            next_indent = ContinuationIndent::Spaces(spaces);
        }
        if let Some(floor) = conditional_floor
            && next_indent.columns(self.options.indent_width) < floor
        {
            next_indent = ContinuationIndent::Spaces(floor);
        }
        let inline_body_indent_extra = max_length_inline_case_body_indent_extra(self.options, line)
            .or_else(|| max_length_inline_access_body_indent_extra(self.options, line));
        if let Some(extra) = inline_body_indent_extra {
            next_indent =
                ContinuationIndent::Spaces(next_indent.columns(self.options.indent_width) + extra);
        }
        let (mut constructor_replay, adjusted_next_indent) = start_max_length_constructor_replay(
            self.options,
            line,
            &split.head,
            &split.tail,
            base_indent_width,
            structural_level,
            next_indent,
        );
        next_indent = adjusted_next_indent;
        let next_structural_level = if let Some(level) = constructor_replay.structural_level() {
            level
        } else if inline_body_indent_extra.is_some() {
            structural_level + 1
        } else {
            structural_level
        };
        let source_tokens = self.output.pending_tokens();
        self.push_output_line_with_indent(&split.head, structural_level, indent);
        let mut tail = split.tail;
        loop {
            let tail_width = if trailing_comment_split_limit(&tail) < tail.len() {
                width
                    .saturating_sub(next_indent.columns(self.options.indent_width))
                    .max(1)
            } else {
                width
            };
            let Some(split) = split_result(&tail, tail_width, self.options.break_after_logical)
                .or_else(|| {
                    (suffix_width > 0).then(|| {
                        split_result(
                            &tail,
                            tail_width.saturating_sub(suffix_width).max(1),
                            self.options.break_after_logical,
                        )
                    })?
                })
            else {
                break;
            };
            let mut following_indent = continuation_indent_for_split(
                &tail,
                &split,
                &SplitIndentInputs {
                    current_indent_width: next_indent.columns(self.options.indent_width),
                    configured_indent: next_indent,
                    following_split: true,
                    ..split_indent_inputs
                },
            )
            .unwrap_or(next_indent);
            following_indent = advance_max_length_constructor_replay(
                self.options,
                &mut constructor_replay,
                &split.head,
                base_indent_width,
                next_indent,
                following_indent,
            );
            if let Some(spaces) = self.max_length_line.objc_message_indent_spaces() {
                following_indent = ContinuationIndent::Spaces(spaces);
            }
            if let Some(floor) = conditional_floor
                && following_indent.columns(self.options.indent_width) < floor
            {
                following_indent = ContinuationIndent::Spaces(floor);
            }
            self.set_split_part_tokens(source_tokens, line, &tail);
            self.push_output_line_with_indent(&split.head, next_structural_level, next_indent);
            tail = split.tail;
            next_indent = following_indent;
        }
        if !tail.trim().is_empty() {
            self.set_split_part_tokens(source_tokens, line, &tail);
            self.push_output_line_with_indent(&tail, next_structural_level, next_indent);
        }
    }

    /// Gives the part of the split `line` that starts with `part` the source
    /// tokens from its first one on.
    fn set_split_part_tokens(&mut self, source: Option<TokenSpan>, line: &str, part: &str) {
        let Some(source) = source else {
            return;
        };
        let part = part.trim_start();
        if !line.ends_with(part) {
            return;
        }
        let from = line.len() - part.len();
        let mut cursor = 0;
        for index in source.first..=source.last {
            let token = &self.tree.tokens[index];
            if !is_code_token(token) {
                continue;
            }
            let text = token_text(token);
            let Some(offset) = line[cursor..].find(&text) else {
                return;
            };
            if cursor + offset >= from {
                self.output.set_pending_tokens(Some(TokenSpan {
                    first: index,
                    last: source.last,
                }));
                return;
            }
            cursor += offset + text.len();
        }
    }

    pub(crate) fn maximum_length_using_alias_rhs_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if self.options.max_code_length.is_none()
            || current.is_empty()
            || current.starts_with(['#', '{', '}'])
        {
            return None;
        }
        let previous = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !previous_trimmed.starts_with("using ") || !previous_code.ends_with('=') {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.options.continuation_indent * self.options.indent_width,
        )
    }
}

fn should_skip_split(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed
            .strip_prefix('*')
            .is_some_and(|rest| rest.is_empty() || rest.starts_with([' ', '\t', '/', '*']))
        || trimmed.starts_with('#')
        || trimmed.starts_with("asm(")
        || trimmed.starts_with("__asm__")
}

fn is_single_string_call_at(line: &str, open: usize) -> bool {
    let Some(close) = matching_close_paren(line, open) else {
        return false;
    };
    let arg = line[open + 1..close].trim();
    arg.starts_with('"') && arg.ends_with('"')
}

fn matching_close_paren(line: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in line[open..].char_indices() {
        let absolute = open + index;
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
            continue;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(absolute);
                }
            }
            _ => {}
        }
    }
    None
}

fn line_has_constructor_initializer(line: &str) -> bool {
    let line = line.trim_start();
    let Some(open) = line.find('(') else {
        return false;
    };
    let Some(close) = matching_close_paren(line, open) else {
        return false;
    };
    scoped_name_is_constructor(line[..open].trim_end())
        && line[close + 1..].trim_start().starts_with(':')
}

pub(super) fn lambda_parameter_continuation_indent(
    line: &str,
    base_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
    configured_continuation_spaces: usize,
    break_style: bool,
) -> Option<usize> {
    let line = line.trim_end();
    let before_lambda = line.strip_suffix('(')?.trim_end();
    if !is_lambda_capture_header(before_lambda) {
        return None;
    }
    let constructor_initializer = line_has_constructor_initializer(line);
    let structural_base = if constructor_initializer {
        base_indent_width.max(indent_width)
    } else {
        base_indent_width
    };
    if !break_style && !constructor_initializer {
        return Some(structural_base);
    }
    let mut open_columns = unmatched_open_paren_columns(line);
    open_columns.pop()?;
    let parent = open_columns.pop()?;
    let target = structural_base + parent + 1 + configured_continuation_spaces;
    Some(target.min(structural_base + max_continuation_indent.saturating_sub(1)))
}

impl FormatEngine<'_> {
    pub(crate) fn replayed_lambda_parameter_indent_spaces(
        &self,
        line_closed_lambda_parameter_list: bool,
        break_lambda_parameters: bool,
    ) -> Option<usize> {
        if !line_closed_lambda_parameter_list {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let base = leading_visual_width(previous, self.options.tab_width);
        lambda_parameter_continuation_indent(
            previous.trim_start(),
            base,
            self.options.indent_width,
            self.options.max_continuation_indent,
            self.options.continuation_indent * self.options.indent_width,
            break_lambda_parameters,
        )
    }
}

#[derive(Clone, Copy)]
struct SplitIndentInputs {
    base_indent_width: usize,
    limit_base_indent_width: usize,
    current_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
    configured_continuation_spaces: usize,
    min_conditional_spaces: usize,
    configured_indent: ContinuationIndent,
    indent_after_parens: bool,
    following_split: bool,
    break_lambda_parameters: bool,
}

fn continuation_indent_for_split(
    line: &str,
    split: &SplitResult,
    inputs: &SplitIndentInputs,
) -> Option<ContinuationIndent> {
    let SplitIndentInputs {
        base_indent_width,
        indent_width,
        max_continuation_indent,
        configured_continuation_spaces,
        current_indent_width,
        configured_indent,
        indent_after_parens,
        following_split,
        break_lambda_parameters,
        ..
    } = *inputs;
    let head = split.head.as_str();
    // A declaration split at the whitespace before its name continues
    // nothing astyle registered, unless an aggregate keyword leads it.
    if !following_split
        && head.chars().all(|ch| {
            is_identifier_continue(ch)
                || ch.is_whitespace()
                || matches!(ch, '*' | '&' | ':' | '<' | '>')
        })
        && head.ends_with(|ch: char| is_identifier_continue(ch) || matches!(ch, '*' | '&' | '>'))
        && !head
            .split(|ch: char| !is_identifier_continue(ch))
            .any(|word| {
                matches!(word, "return" | "case" | "goto")
                    || matches!(word, "struct" | "union" | "enum" | "class") && !line.contains('(')
            })
    {
        return Some(ContinuationIndent::Spaces(base_indent_width));
    }
    let has_open_paren = !unmatched_open_paren_columns(head).is_empty();
    if let Some(spaces) = lambda_parameter_continuation_indent(
        head,
        base_indent_width,
        indent_width,
        max_continuation_indent,
        configured_continuation_spaces,
        break_lambda_parameters,
    ) {
        return Some(ContinuationIndent::Spaces(spaces));
    }
    if head.trim_end().ends_with("<=>") {
        return Some(configured_indent);
    }
    if split.kind == SplitKind::Delimiter
        && head.trim_end().ends_with('(')
        && split_function_declaration_head(head)
    {
        return Some(configured_indent);
    }
    if indent_after_parens {
        if has_open_paren
            && let Some(spaces) = assignment_value_indent(line, base_indent_width)
            && !head_ends_assignment_operator(head)
        {
            return Some(ContinuationIndent::Spaces(spaces + indent_width));
        }
        if has_open_paren
            && current_indent_width > base_indent_width
            && head.trim_end().ends_with('(')
            && !head.contains(')')
        {
            return Some(ContinuationIndent::Spaces(
                current_indent_width + configured_continuation_spaces,
            ));
        }
        return Some(configured_indent);
    }

    if split.kind == SplitKind::Whitespace
        && let Some(spaces) = stream_chain_continuation_indent(line, base_indent_width)
    {
        return Some(ContinuationIndent::Spaces(
            if current_indent_width > base_indent_width {
                current_indent_width
            } else {
                spaces
            },
        ));
    }

    let operator_split = matches!(
        split.kind,
        SplitKind::LogicalOperator
            | SplitKind::AssignmentOrComparison
            | SplitKind::ArithmeticOperator
            | SplitKind::StringConcat
    ) && !head.trim_end().ends_with(['(', '[']);
    // A paren opened after the assignment stacks past its value.
    let paren_after_assignment = top_level_assignment_index(line).is_some_and(|assignment| {
        unmatched_open_paren_columns(head)
            .iter()
            .any(|&open| open > assignment)
    });
    if operator_split {
        if let Some(spaces) = assignment_value_indent(line, base_indent_width)
            && !head_ends_assignment_operator(head)
            && !paren_after_assignment
        {
            return Some(ContinuationIndent::Spaces(spaces));
        }
        if let Some(spaces) = return_value_indent(line, base_indent_width) {
            return Some(ContinuationIndent::Spaces(spaces));
        }
    }

    let open_columns = unmatched_open_paren_columns(head);
    if let Some(spaces) =
        nested_new_continuation_indent(line, &open_columns, inputs, head.trim_end().ends_with('('))
    {
        return Some(ContinuationIndent::Spaces(spaces));
    }
    let all_openers_over_max = !open_columns.is_empty()
        && open_columns
            .iter()
            .all(|column| *column >= max_continuation_indent);
    if all_openers_over_max && let Some(spaces) = assignment_value_indent(line, base_indent_width) {
        let call_body_extra =
            usize::from(head.trim_end().ends_with('(')) * configured_continuation_spaces;
        let target = spaces + call_body_extra;
        if target.saturating_sub(base_indent_width) > max_continuation_indent {
            return Some(ContinuationIndent::Spaces(
                base_indent_width + indent_width * 2,
            ));
        }
        return Some(ContinuationIndent::Spaces(target));
    }
    if all_openers_over_max
        && following_split
        && head.trim_end().ends_with('(')
        && !head.contains(')')
    {
        return Some(ContinuationIndent::Spaces(
            current_indent_width + configured_continuation_spaces,
        ));
    }

    // A paren ending the line stacks one continuation past the indent
    // before it.
    if !following_split && head.trim_end().ends_with('(') {
        let columns = unmatched_open_paren_columns(head);
        let previous = match columns.len().checked_sub(2).map(|outer| columns[outer]) {
            Some(outer) => {
                let registered = if outer < max_continuation_indent {
                    base_indent_width + outer + 1
                } else {
                    base_indent_width + indent_width * 2
                };
                if language::is_header(trailing_word(head[..outer].trim_end())) {
                    registered.max(base_indent_width + inputs.min_conditional_spaces)
                } else {
                    registered
                }
            }
            None => return_value_indent(head, base_indent_width)
                .or_else(|| assignment_value_indent(head, base_indent_width))
                .unwrap_or(base_indent_width),
        };
        let target = previous + configured_continuation_spaces;
        return Some(ContinuationIndent::Spaces(
            if target - base_indent_width > max_continuation_indent {
                base_indent_width + indent_width * 2
            } else {
                target
            },
        ));
    }
    paren_continuation_indent(
        head,
        base_indent_width,
        indent_width,
        max_continuation_indent,
    )
    .or_else(|| assignment_continuation_indent(line, head, base_indent_width))
}

fn split_function_declaration_head(head: &str) -> bool {
    let Some(before) = head.trim_end().strip_suffix('(').map(str::trim_end) else {
        return false;
    };
    if top_level_assignment_index(before).is_some()
        || before.starts_with("return ")
        || before.starts_with("new ")
        || before.contains('(')
    {
        return false;
    }
    if scoped_name_is_constructor(before) {
        return true;
    }
    let Some(name_start) = function_name_start(before) else {
        return false;
    };
    let return_type = before[..name_start].trim_end();
    !return_type.is_empty() && !return_type.ends_with('.') && !return_type.ends_with("->")
}

fn nested_new_continuation_indent(
    line: &str,
    open_columns: &[usize],
    inputs: &SplitIndentInputs,
    trailing_open_paren: bool,
) -> Option<usize> {
    let SplitIndentInputs {
        base_indent_width,
        limit_base_indent_width,
        current_indent_width,
        indent_width,
        max_continuation_indent,
        configured_continuation_spaces,
        ..
    } = *inputs;
    let &deepest = open_columns.last()?;
    let has_new = line[..deepest].match_indices("new").any(|(index, _)| {
        let before = line[..index].chars().next_back();
        let after = line[index + "new".len()..].chars().next();
        before.is_none_or(|ch| !is_identifier_continue(ch))
            && after.is_some_and(char::is_whitespace)
    });
    let limit_current_indent_width =
        limit_base_indent_width + current_indent_width.saturating_sub(base_indent_width);
    if !has_new
        || limit_current_indent_width + deepest + 1 < max_continuation_indent
        || current_indent_width == base_indent_width && !trailing_open_paren
    {
        return None;
    }
    if current_indent_width > base_indent_width {
        return Some(current_indent_width + indent_width * 2);
    }
    let outer_column = open_columns[..open_columns.len() - 1]
        .iter()
        .rev()
        .copied()
        .find(|column| limit_base_indent_width + column + 1 < max_continuation_indent)?;
    let outer = base_indent_width + outer_column + 1;
    let configured = outer + configured_continuation_spaces;
    Some(
        if limit_base_indent_width + outer_column + 1 + configured_continuation_spaces
            > max_continuation_indent
        {
            base_indent_width + indent_width * 2
        } else {
            configured
        },
    )
}

fn stream_chain_continuation_indent(line: &str, base_indent_width: usize) -> Option<usize> {
    let shift = line.find("<<")?;
    let operand = line[..shift].trim_end();
    if operand.is_empty()
        || operand.contains(['(', '=', '?'])
        || operand.trim_start().starts_with("return")
    {
        return None;
    }
    Some(base_indent_width + shift)
}

fn return_value_indent(line: &str, base_indent_width: usize) -> Option<usize> {
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let tail = trimmed.strip_prefix("return")?;
    if tail.chars().next().is_some_and(is_identifier_continue) {
        return None;
    }
    let gap = tail.len() - tail.trim_start().len();
    Some(base_indent_width + leading + "return".len() + gap)
}

fn assignment_value_indent(line: &str, base_indent_width: usize) -> Option<usize> {
    let assignment = top_level_assignment_index(line)?;
    let after_assignment = line[assignment + 1..]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    Some(base_indent_width + assignment + 1 + after_assignment)
}

fn paren_continuation_indent(
    head: &str,
    base_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
) -> Option<ContinuationIndent> {
    let columns = unmatched_open_paren_columns(head);
    if columns.is_empty() {
        return None;
    }
    columns
        .into_iter()
        .rev()
        .find(|column| *column < max_continuation_indent)
        .map(|column| ContinuationIndent::Spaces(base_indent_width + column + 1))
        .or(Some(ContinuationIndent::Spaces(
            base_indent_width + indent_width * 2,
        )))
}

fn assignment_continuation_indent(
    line: &str,
    head: &str,
    base_indent_width: usize,
) -> Option<ContinuationIndent> {
    let head = head.trim_end();
    if head.ends_with('=') || head.ends_with('(') {
        return None;
    }
    let assignment = top_level_assignment_index(line)?;
    if head.len() <= assignment {
        return None;
    }
    let after_assignment = line[assignment + 1..]
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum::<usize>();
    Some(ContinuationIndent::Spaces(
        base_indent_width + assignment + 1 + after_assignment,
    ))
}

fn top_level_assignment_index(line: &str) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut previous = '\0';
    let mut iter = line.char_indices().peekable();
    while let Some((index, ch)) = iter.next() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '<' if paren_depth == 0 && bracket_depth == 0 => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            '=' if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                let next = iter.peek().map(|(_, ch)| *ch).unwrap_or('\0');
                if !matches!(
                    previous,
                    '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^'
                ) && next != '='
                {
                    return Some(index);
                }
            }
            _ => {}
        }
        if !ch.is_whitespace() {
            previous = ch;
        }
    }
    None
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum SplitKind {
    LogicalOperator,
    AssignmentOrComparison,
    ArithmeticOperator,
    StringConcat,
    Comma,
    Semicolon,
    Delimiter,
    Pointer,
    Whitespace,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SplitResult {
    head: String,
    tail: String,
    split_at: usize,
    kind: SplitKind,
    priority: usize,
    anchor_column: Option<usize>,
    indent: ContinuationIndent,
}

fn template_argument_ranges(line: &str) -> Vec<(usize, usize)> {
    let tokens = tokenize(line);
    let mut ranges = Vec::new();
    let mut outer_start = None;
    let mut depth = 0usize;
    let mut offset = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        let text = token_text(token);
        match template_angle_role(&tokens, index, tokens.len(), depth) {
            TemplateAngle::Open => {
                if depth == 0 {
                    outer_start = Some(offset);
                }
                depth += 1;
            }
            TemplateAngle::Close(count) => {
                depth = depth.saturating_sub(count);
                if depth == 0
                    && let Some(start) = outer_start.take()
                {
                    ranges.push((start, offset + text.len()));
                }
            }
            TemplateAngle::None => {}
        }
        offset += text.len();
    }
    if let Some(start) = outer_start {
        ranges.push((start, line.len()));
    }
    ranges
}

const ASTYLE_MIN_CODE_LENGTH: usize = 10;

fn split_result(line: &str, width: usize, prefer_logical_operator: bool) -> Option<SplitResult> {
    if line.len() <= width {
        return None;
    }
    astyle_split_result(line, width, prefer_logical_operator)
}

fn split_result_kind(line: &str, split_at: usize, priority: usize) -> SplitKind {
    let head = line[..split_at].trim_end();
    let tail = line[split_at..].trim_start();
    if head.ends_with(['(', '[']) {
        return SplitKind::Delimiter;
    }
    if matches!(priority, 79 | 80) {
        return SplitKind::LogicalOperator;
    }
    if priority == 70 || priority == 54 {
        if head.ends_with('"') && tail.starts_with('+') {
            return SplitKind::StringConcat;
        }
        return SplitKind::AssignmentOrComparison;
    }
    match priority {
        75 => SplitKind::Semicolon,
        59 => SplitKind::Pointer,
        60 => SplitKind::Comma,
        55 => SplitKind::ArithmeticOperator,
        40 => SplitKind::Delimiter,
        _ => SplitKind::Whitespace,
    }
}

fn logical_word_split_point(
    line: &str,
    index: usize,
    prefer_after_logical: bool,
) -> Option<(usize, usize)> {
    let rest = line.get(index..)?;
    let word = ["and", "or"]
        .into_iter()
        .find(|word| rest.starts_with(word))?;
    let end = index + word.len();
    let before = line[..index].chars().next_back();
    let after = line[end..].chars().next();
    if before.is_some_and(is_identifier_continue) || after.is_some_and(is_identifier_continue) {
        return None;
    }
    Some((if prefer_after_logical { end } else { index }, 80))
}

fn is_objc_selector_colon(line: &str, colon: usize) -> bool {
    let (segment, continued_message) =
        if let Some(open) = unmatched_open_bracket_column(&line[..colon]) {
            (&line[open + 1..colon], false)
        } else {
            if !line[colon..].contains(']') {
                return false;
            }
            (&line[..colon], true)
        };
    if segment.contains('?') || (!continued_message && !segment.chars().any(char::is_whitespace)) {
        return false;
    }
    segment
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(is_identifier_continue)
}

fn is_objc_message_open(line: &str, open: usize) -> bool {
    line[open + 1..]
        .match_indices(':')
        .map(|(offset, _)| open + 1 + offset)
        .any(|colon| is_objc_selector_colon(line, colon))
}

fn ends_single_string_call(line: &str) -> bool {
    let line = line.trim_end();
    if !line.ends_with(')') {
        return false;
    }
    let close = line.len() - 1;
    line.match_indices('(').rev().any(|(open, _)| {
        matching_close_paren(line, open) == Some(close) && is_single_string_call_at(line, open)
    })
}

fn split_point_at(
    line: &str,
    index: usize,
    ch: char,
    prefer_logical_operator: bool,
    width: usize,
) -> Option<(usize, usize)> {
    if let Some((start, end, operator)) = operator_bounds_containing(line, index) {
        if operator == ":" && is_objc_selector_colon(line, start) {
            let argument_start = line[end..]
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(line.len(), |(offset, _)| end + offset);
            let argument_len = line[argument_start..]
                .chars()
                .take_while(|ch| is_identifier_continue(*ch))
                .map(char::len_utf8)
                .sum::<usize>();
            return (line[..argument_start + argument_len].trim_end().len() > width)
                .then_some((end, 55));
        }
        if matches!(operator, "::" | "->" | "<<" | ">>" | "~" | "!")
            || matches!(operator, "&" | "-" | "+" | "*") && is_prefix_operator(&line[..start])
        {
            return None;
        }
        if is_pointer_split_operator(line, start, end, operator)
            || line[end..].trim_start().starts_with("/*")
        {
            return None;
        }
        // astyle splits at an unpadded operator only for `+`, `-`, the
        // logical and the comparison operators other than `<` and `>`, or
        // at a paren beside it.
        let unpadded = line[..start]
            .chars()
            .next_back()
            .is_some_and(|ch| !ch.is_whitespace() && !matches!(ch, ')' | ']'))
            && line[end..]
                .chars()
                .next()
                .is_some_and(|ch| !ch.is_whitespace() && ch != '(');
        if unpadded && matches!(operator, "<" | ">" | "|" | "&" | "^" | "*" | "/" | "%")
            || is_declarator_pointer(line, start, end)
        {
            return None;
        }
        return if matches!(operator, "&&" | "||") {
            if prefer_logical_operator {
                Some((end, 80))
            } else {
                Some((start, 80))
            }
        } else if operator == "=" {
            Some((end, 54))
        } else if language::ASSIGNMENT_OPERATORS.contains(&operator)
            || matches!(operator, "==" | "!=" | "<=>" | "<=" | ">=" | "<" | ">")
        {
            if line[..end].trim_end().len() > width && ends_single_string_call(&line[..start]) {
                Some((start, 70))
            } else {
                Some((end, 70))
            }
        } else if operator == "+"
            && line[..start]
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(|ch| ch == '"')
        {
            let padded = line[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
            Some((start, if padded { 70 } else { 55 }))
        } else if matches!(operator, "|" | "&" | "^") {
            Some((end, 55))
        } else if matches!(operator, "+" | "-" | "*" | "/" | "%")
            && line[..start]
                .chars()
                .next_back()
                .is_some_and(|ch| !ch.is_whitespace())
            && line[end..]
                .chars()
                .next()
                .is_some_and(|ch| !ch.is_whitespace())
        {
            Some((start, 55))
        } else {
            Some((end, 55))
        };
    }
    let end = index + ch.len_utf8();
    match ch {
        ',' => Some((end, 60)),
        ';' => Some((end, 75)),
        // astyle splits after no paren that a literal or paren follows.
        '(' if line[end..].trim_start().starts_with([')', '(', '"', '\'']) => None,
        '(' if is_single_string_call_at(line, index) => None,
        '(' if is_lambda_capture_header(line[..index].trim_end()) => Some((end, 75)),
        '(' if is_function_call_split(line, index) => Some((end, 55)),
        // astyle splits after no bracket.
        '[' => None,
        '(' => Some((end, 40)),
        ' ' | '\t'
            if unmatched_open_bracket_column(&line[..index])
                .is_some_and(|open| is_objc_message_open(line, open)) =>
        {
            Some((end, 39))
        }
        // A block's opening brace stays with its head.
        ' ' | '\t' if line[end..].trim_start().starts_with('{') => None,
        ' ' | '\t'
            if !whitespace_touches_pointer_operator(line, index)
                || declarator_pointer_follows(line, index) =>
        {
            Some((end, 10))
        }
        _ => None,
    }
}

/// Whether an operator after `before` applies to the operand after it.
fn is_prefix_operator(before: &str) -> bool {
    let before = before.trim_end();
    match before.chars().next_back() {
        None => true,
        Some(ch) if is_identifier_continue(ch) => {
            let word_start = before
                .rfind(|ch: char| !is_identifier_continue(ch))
                .map_or(0, |index| index + 1);
            matches!(&before[word_start..], "return" | "case")
        }
        Some(ch) => matches!(
            ch,
            '=' | '('
                | ','
                | '['
                | '{'
                | '?'
                | ':'
                | ';'
                | '!'
                | '~'
                | '<'
                | '>'
                | '+'
                | '-'
                | '*'
                | '/'
                | '%'
                | '&'
                | '|'
                | '^'
        ),
    }
}

fn is_function_call_split(line: &str, open_paren: usize) -> bool {
    line[..open_paren]
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(is_identifier_continue)
}

fn is_pointer_split_operator(line: &str, start: usize, end: usize, operator: &str) -> bool {
    if !matches!(operator, "*" | "&" | "^") {
        return false;
    }
    let before = line[..start].trim_end();
    let after = line[end..].trim_start();
    if after
        .chars()
        .next()
        .is_none_or(|ch| !(is_identifier_start(ch) || ch == ')' || matches!(ch, '*' | '&' | '^')))
    {
        return false;
    }
    before.ends_with('(')
        || before.ends_with('*')
        || before.ends_with('&')
        || before.ends_with('^')
        || is_pointer_type_word(trailing_word(before))
        || is_local_pointer_declarator(line, start)
}

fn is_local_pointer_declarator(line: &str, operator_start: usize) -> bool {
    let before = line[..operator_start].trim_end();
    let Some((delimiter_index, delimiter)) = before
        .char_indices()
        .rev()
        .find(|(_, ch)| matches!(ch, '(' | ',' | ';' | '{' | '}'))
    else {
        return false;
    };
    if matches!(delimiter, ';' | '{' | '}') {
        return false;
    }
    let segment = before[delimiter_index + delimiter.len_utf8()..].trim();
    if !is_pointer_declaration_segment(segment) {
        return false;
    }
    let open_index = if delimiter == '(' {
        delimiter_index
    } else {
        let Some(open_index) = containing_open_paren_before(before, delimiter_index) else {
            return false;
        };
        open_index
    };
    is_declaration_head(before[..open_index].trim_end())
}

fn containing_open_paren_before(line: &str, limit: usize) -> Option<usize> {
    let mut stack = Vec::new();
    for (index, ch) in line[..limit].char_indices() {
        match ch {
            '(' => stack.push(index),
            ')' => {
                stack.pop();
            }
            _ => {}
        }
    }
    stack.pop()
}

fn is_declaration_head(head: &str) -> bool {
    if head.is_empty() || head.contains('=') {
        return false;
    }
    if scoped_name_is_constructor(head) {
        return true;
    }
    let Some(name_start) = function_name_start(head) else {
        return false;
    };
    let return_type = head[..name_start].trim_end();
    let name = head[name_start..].trim_start();
    if return_type.is_empty() || name.is_empty() || language::is_header(name) {
        return false;
    }
    let last_type_word = return_type
        .rsplit(|ch: char| !is_identifier_continue(ch))
        .find(|word| !word.is_empty());
    !last_type_word.is_some_and(is_non_type_keyword)
}

/// Whether the `*` or `&` at `start..end` binds to the name of a
/// statement-level declaration, as in `Type *name`: astyle splits before
/// it at the whitespace, never after it.
fn is_declarator_pointer(line: &str, start: usize, end: usize) -> bool {
    let before = &line[..start];
    matches!(&line[start..end], "*" | "&")
        && before.ends_with([' ', '\t'])
        && line[end..]
            .chars()
            .next()
            .is_some_and(|ch| is_identifier_start(ch) || ch == '*')
        && unmatched_open_paren_column(before).is_none()
        && top_level_assignment_index(before).is_none()
        && {
            let word = trailing_word(before.trim_end());
            !word.is_empty()
                && before.trim_end().ends_with(word)
                && !language::is_non_type_keyword(word)
        }
}

/// Whether a declarator pointer follows the whitespace at `index`.
fn declarator_pointer_follows(line: &str, index: usize) -> bool {
    line[index..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .is_some_and(|(offset, ch)| {
            let start = index + offset;
            is_declarator_pointer(line, start, start + ch.len_utf8())
        })
}

fn whitespace_touches_pointer_operator(line: &str, index: usize) -> bool {
    let previous = line[..index]
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(offset, ch)| (offset, offset + ch.len_utf8(), ch));
    let next = line[index + 1..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(offset, ch)| {
            let start = index + 1 + offset;
            (start, start + ch.len_utf8(), ch)
        });

    [previous, next]
        .into_iter()
        .flatten()
        .any(|(start, end, ch)| {
            matches!(ch, '*' | '&' | '^')
                && is_pointer_split_operator(line, start, end, &line[start..end])
        })
}

fn operator_bounds_containing(line: &str, index: usize) -> Option<(usize, usize, &'static str)> {
    language::OPERATORS
        .iter()
        .copied()
        .filter_map(|operator| {
            let min_start = index.saturating_sub(operator.len().saturating_sub(1));
            (min_start..=index)
                .filter(|start| line.is_char_boundary(*start))
                .find_map(|start| {
                    let end = start + operator.len();
                    (line[start..].starts_with(operator) && index < end)
                        .then_some((start, end, operator))
                })
        })
        .max_by_key(|(_, _, operator)| operator.len())
}

const SEMI: usize = 0;
const AND_OR: usize = 1;
const COMMA: usize = 2;
const PAREN: usize = 3;
const WHITESPACE: usize = 4;

/// The operators astyle appends whole, longest first.
const ASTYLE_OPERATORS: [&str; 46] = [
    "<=>", ">>>=", "<<<=", ">>=", "<<=", ">>>", "<<<", "->*", "...", "+=", "-=", "*=", "/=", "%=",
    "|=", "&=", "^=", "==", "++", "--", "!=", ">=", "<=", ">>", "<<", "??", "=>", "->", "&&", "||",
    "::", "<?", ">?", "+", "-", "*", "/", "%", "?", ":", "=", "<", ">", "!", "|", "&",
];

/// The point where astyle splits `line`, replayed as astyle appends it:
/// each appended piece registers its split points, and the first time the
/// line runs past `width` astyle splits at the best one.
fn astyle_split_point(line: &str, width: usize, break_after_logical: bool) -> Option<usize> {
    let bytes = line.as_bytes();
    let templates = template_argument_ranges(line);
    let mut fit = [0usize; 5];
    let mut pending = [0usize; 5];
    fn register(points: (&mut [usize; 5], &mut [usize; 5]), kind: usize, at: usize, fits: bool) {
        if fits {
            points.0[kind] = at;
        } else {
            points.1[kind] = at;
        }
    }
    let peek = |from: usize| -> u8 {
        bytes[from.min(bytes.len())..]
            .iter()
            .copied()
            .find(|byte| !matches!(byte, b' ' | b'\t'))
            .unwrap_or(b' ')
    };
    let name_char = |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_');
    let potential_operator = |byte: u8| {
        byte.is_ascii_punctuation()
            && !matches!(
                byte,
                b'{' | b'}' | b'(' | b')' | b'[' | b']' | b';' | b',' | b'#' | b'\\' | b'\'' | b'"'
            )
    };
    let mut previous_non_space = b' ';
    let mut index = 0;
    let mut quote: Option<u8> = None;
    let mut in_comment = false;
    // A line closing a block it did not open lies in a one-line block.
    let mut unbroken_depth = unopened_closing_braces(line);
    while index < bytes.len() {
        let byte = bytes[index];
        let mut end = index + 1;
        if let Some(open) = quote {
            if byte == b'\\' {
                end = (index + 2).min(bytes.len());
            } else if byte == open {
                quote = None;
                previous_non_space = byte;
            }
        } else if in_comment {
            if line[index..].starts_with("*/") {
                end = index + 2;
                in_comment = false;
            }
        } else if line[index..].starts_with("//") {
            end = bytes.len();
        } else if line[index..].starts_with("/*") {
            end = index + 2;
            in_comment = true;
        } else if matches!(byte, b'"' | b'\'') {
            quote = Some(byte);
        } else if templates
            .iter()
            .any(|&(start, stop)| start <= index && index < stop)
        {
            if !matches!(byte, b' ' | b'\t') {
                previous_non_space = byte;
            }
        } else if let Some(word) = ["and", "or"].into_iter().find(|word| {
            line[index..].starts_with(word)
                && !index
                    .checked_sub(1)
                    .is_some_and(|before| name_char(bytes[before]))
                && !bytes
                    .get(index + word.len())
                    .is_some_and(|after| name_char(*after))
        }) {
            end = index + word.len();
            let at = if break_after_logical {
                end
            } else if index > 0 && matches!(bytes[index - 1], b' ' | b'\t') {
                index - 1
            } else {
                index
            };
            if unbroken_depth == 0 {
                register((&mut fit, &mut pending), AND_OR, at, at <= width);
            }
            previous_non_space = bytes[end - 1];
        } else if let Some(operator) = ASTYLE_OPERATORS
            .iter()
            .find(|operator| line[index..].starts_with(**operator))
            .filter(|_| potential_operator(byte))
        {
            end = index + operator.len();
            let next = peek(index + 1);
            let before = index.checked_sub(1).map(|before| bytes[before]);
            if next != b'/' && unbroken_depth == 0 {
                match *operator {
                    "||" | "&&" => {
                        if break_after_logical {
                            register((&mut fit, &mut pending), AND_OR, end, end <= width);
                        } else {
                            let at = if before.is_some_and(|byte| matches!(byte, b' ' | b'\t')) {
                                index - 1
                            } else {
                                index
                            };
                            register((&mut fit, &mut pending), AND_OR, at, at <= width);
                        }
                    }
                    "==" | "!=" | ">=" | "<=" | "<=>" => {
                        register((&mut fit, &mut pending), WHITESPACE, end, end <= width)
                    }
                    "+" | "-" | "?"
                        if before.is_some_and(|byte| {
                            name_char(byte) || matches!(byte, b')' | b']' | b'"')
                        }) && !(*operator != "?" && in_exponent(line, index)) =>
                    {
                        register((&mut fit, &mut pending), WHITESPACE, index, index <= width);
                    }
                    "=" | ":" => {
                        let at = if end < width { end } else { index };
                        if previous_non_space == b']' {
                            register((&mut fit, &mut pending), WHITESPACE, at, index <= width);
                        } else if before
                            .is_some_and(|byte| name_char(byte) || matches!(byte, b')' | b']'))
                        {
                            register((&mut fit, &mut pending), WHITESPACE, at, end <= width);
                        }
                    }
                    _ => {}
                }
            }
            previous_non_space = bytes[end - 1];
        } else {
            let next = peek(end);
            // A brace with code after it on the line drops the points before
            // it and registers none up to its closing brace.
            if byte == b'{' && (unbroken_depth > 0 || !line[end..].trim_start().is_empty()) {
                if unbroken_depth == 0 {
                    fit = [0; 5];
                    pending = [0; 5];
                }
                unbroken_depth += 1;
            } else if byte == b'}' && unbroken_depth > 0 {
                unbroken_depth -= 1;
            }
            let blocked = unbroken_depth > 0
                || next == b'/'
                || matches!(byte, b'{' | b'}' | b'[' | b']')
                || matches!(previous_non_space, b'{' | b'}' | b'[')
                || matches!(next, b'{' | b'}' | b'[' | b']');
            if !blocked {
                match byte {
                    b' ' | b'\t' => {
                        if !matches!(next, b')' | b'(' | b':') && previous_non_space != b'(' {
                            register((&mut fit, &mut pending), WHITESPACE, index, index <= width);
                        }
                    }
                    b')' => {
                        let member_access =
                            next == b'-' && line[end..].trim_start().starts_with("->");
                        if !(matches!(next, b')' | b' ' | b';' | b',' | b'.') || member_access) {
                            register((&mut fit, &mut pending), WHITESPACE, end, end <= width);
                        }
                    }
                    b',' => register((&mut fit, &mut pending), COMMA, end, end <= width),
                    b'(' => {
                        if !matches!(next, b')' | b'(' | b'"' | b'\'') {
                            let at = if potential_operator(previous_non_space) {
                                index
                            } else {
                                end
                            };
                            register((&mut fit, &mut pending), PAREN, at, end <= width);
                        }
                    }
                    b';' => {
                        if !matches!(next, b' ' | b'}' | b'/') {
                            register((&mut fit, &mut pending), SEMI, end, end <= width);
                        }
                    }
                    _ => {}
                }
            }
            if !matches!(byte, b' ' | b'\t') {
                previous_non_space = byte;
            }
        }
        if end > width {
            let split = astyle_find_split_point(&fit, &pending, width, end, || {
                let word_end = if name_char(byte) {
                    index
                        + bytes[index..]
                            .iter()
                            .take_while(|byte| name_char(**byte))
                            .count()
                } else {
                    index + 2
                };
                word_end + 1 > bytes.len()
            });
            if split > 0 && split < end {
                return Some(split);
            }
        }
        index = end;
    }
    None
}

fn unopened_closing_braces(line: &str) -> usize {
    let mut depth = 0isize;
    let mut lowest = 0isize;
    for token in tokenize(line) {
        match token {
            Token::Symbol('{') => depth += 1,
            Token::Symbol('}') => {
                depth -= 1;
                lowest = lowest.min(depth);
            }
            _ => {}
        }
    }
    lowest.unsigned_abs()
}

fn astyle_find_split_point(
    fit: &[usize; 5],
    pending: &[usize; 5],
    width: usize,
    length: usize,
    at_line_end: impl Fn() -> bool,
) -> usize {
    let mut split = fit[SEMI];
    if fit[AND_OR] >= ASTYLE_MIN_CODE_LENGTH {
        split = fit[AND_OR];
    }
    if split < ASTYLE_MIN_CODE_LENGTH {
        split = fit[WHITESPACE];
        if fit[PAREN] > split || fit[PAREN] as f64 >= width as f64 * 0.7 {
            split = fit[PAREN];
        }
        if fit[COMMA] > split || fit[COMMA] as f64 >= width as f64 * 0.3 {
            split = fit[COMMA];
        }
    }
    if split < ASTYLE_MIN_CODE_LENGTH {
        return [SEMI, AND_OR, COMMA, PAREN, WHITESPACE]
            .into_iter()
            .map(|kind| pending[kind])
            .filter(|&at| at > 0)
            .min()
            .unwrap_or(0);
    }
    if length - split > width && at_line_end() {
        if fit[WHITESPACE] > split + 3 {
            split = fit[WHITESPACE];
        }
        if fit[PAREN] > split {
            split = fit[PAREN];
        }
    }
    split
}

fn in_exponent(line: &str, index: usize) -> bool {
    let head = &line[..index];
    head.ends_with(['e', 'E'])
        && head
            .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_'))
            .next()
            .is_some_and(|word| word.starts_with(|ch: char| ch.is_ascii_digit()))
}

fn astyle_split_result(line: &str, width: usize, break_after_logical: bool) -> Option<SplitResult> {
    let split_at = astyle_split_point(line, width, break_after_logical)?;
    let head = line[..split_at].trim_end().to_string();
    let tail = line[split_at..].trim_start().to_string();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    // The split takes the class of the strongest point at it.
    let head_end = line[..split_at].trim_end().len();
    let priority = (split_at.saturating_sub(4)..(split_at + 3).min(line.len()))
        .filter(|&index| line.is_char_boundary(index))
        .filter_map(|index| {
            let ch = line[index..].chars().next()?;
            [
                logical_word_split_point(line, index, break_after_logical),
                split_point_at(line, index, ch, break_after_logical, width),
            ]
            .into_iter()
            .flatten()
            .filter(|&(at, _)| line[..at].trim_end().len() == head_end)
            .map(|(_, priority)| priority)
            .max()
        })
        .max()
        .unwrap_or(if head.ends_with(',') {
            60
        } else if head.ends_with(';') {
            75
        } else if head.ends_with('(') {
            40
        } else {
            10
        });
    let anchor_column = unmatched_open_paren_column(&head);
    Some(SplitResult {
        kind: split_result_kind(line, split_at, priority),
        head,
        tail,
        split_at,
        priority,
        anchor_column,
        indent: anchor_column
            .map(|column| ContinuationIndent::Spaces(column + 1))
            .unwrap_or(ContinuationIndent::Level(1)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_result_records_comma_metadata() {
        let result = split_result("call(alpha, beta, gamma, delta)", 20, false).expect("split");

        assert_eq!(result.kind, SplitKind::Comma);
        assert_eq!(result.priority, 60);
        assert_eq!(result.anchor_column, Some(4));
        assert_eq!(result.indent, ContinuationIndent::Spaces(5));
    }

    #[test]
    fn split_result_records_logical_operator_metadata() {
        let result = split_result("alpha && beta && gamma", 14, false).expect("split");

        assert_eq!(result.kind, SplitKind::LogicalOperator);
        assert_eq!(result.priority, 80);
        assert_eq!(result.indent, ContinuationIndent::Level(1));
    }
}
