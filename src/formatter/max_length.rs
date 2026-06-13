use super::brace_classification::is_lambda_capture_header;
use super::columns::leading_visual_width;
use super::headers::is_conditional_header_line;
use super::language::{self, is_non_type_keyword, is_pointer_type_word};
use super::line_scan::{
    inline_brace_pair_range, trailing_comment_split_limit, trailing_comment_start,
    unmatched_open_bracket_column, unmatched_open_paren_column, unmatched_open_paren_columns,
};
use super::operators::head_ends_assignment_operator;
use super::pointers::is_pointer_declaration_segment;
use super::syntax::{function_name_start, scoped_name_is_constructor, template_angle_role};
use super::token::{token_text, tokenize};
use super::{BraceStyle, ContinuationIndent, FormatEngine, TemplateAngle};
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

#[derive(Default)]
pub(super) struct MaxLengthLineState {
    suffix_width: usize,
    objc_message_indent_spaces: Option<usize>,
}

impl MaxLengthLineState {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(super) fn suffix_width(&self) -> usize {
        self.suffix_width
    }

    pub(super) fn set_suffix_width(&mut self, width: usize) {
        self.suffix_width = width;
    }

    pub(super) fn objc_message_indent_spaces(&self) -> Option<usize> {
        self.objc_message_indent_spaces
    }

    pub(super) fn set_objc_message_indent_spaces(&mut self, spaces: Option<usize>) {
        self.objc_message_indent_spaces = spaces;
    }
}

impl FormatEngine<'_> {
    pub(super) fn push_formatted_line_with_indent(
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
        if should_skip_split(line) {
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
        let mut line_adjuster = self.line_adjuster.clone();
        let adjusted = line_adjuster.adjust_line(format!("{prefix}{line}"));
        let base_indent_width =
            configured_indent_width.max(leading_visual_width(&adjusted, self.options.tab_width));
        let brace_row_layout =
            self.max_length_brace_row_layout(line, structural_level, base_indent_width, width);
        let first_width = brace_row_layout.first_width;
        let suffix_width = line
            .trim_end()
            .ends_with(';')
            .then_some(self.max_length_line.suffix_width())
            .unwrap_or(0);
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
        let mut next_indent = continuation_indent_for_split(
            line,
            &split.head,
            &split,
            base_indent_width,
            configured_indent_width,
            self.options.indent_width,
            self.options.max_continuation_indent,
            self.options.indent_after_parens,
            current_line_owner,
            self.options.continuation_indent * self.options.indent_width,
            base_indent_width,
            false,
            break_lambda_parameters,
        )
        .unwrap_or(split_indent);
        if brace_row_layout.attaches_lisp_closer {
            next_indent = ContinuationIndent::Level(self.state.indent());
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
        let inline_body_indent_extra = self
            .max_length_inline_case_body_indent_extra(line)
            .or_else(|| self.max_length_inline_access_body_indent_extra(line));
        if let Some(extra) = inline_body_indent_extra {
            next_indent =
                ContinuationIndent::Spaces(next_indent.columns(self.options.indent_width) + extra);
        }
        let (mut constructor_replay, adjusted_next_indent) = self
            .start_max_length_constructor_replay(
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
                &split.head,
                &split,
                base_indent_width,
                configured_indent_width,
                self.options.indent_width,
                self.options.max_continuation_indent,
                self.options.indent_after_parens,
                next_indent,
                self.options.continuation_indent * self.options.indent_width,
                next_indent.columns(self.options.indent_width),
                true,
                break_lambda_parameters,
            )
            .unwrap_or(next_indent);
            following_indent = self.advance_max_length_constructor_replay(
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
            self.push_output_line_with_indent(&split.head, next_structural_level, next_indent);
            tail = split.tail;
            next_indent = following_indent;
        }
        if !tail.trim().is_empty() {
            self.push_output_line_with_indent(&tail, next_structural_level, next_indent);
        }
    }

    pub(super) fn maximum_length_using_alias_rhs_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if self.options.max_code_length.is_none()
            || current.is_empty()
            || current.starts_with(['#', '{', '}'])
        {
            return None;
        }
        let previous = self
            .output
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
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
        || trimmed.starts_with("asm(")
        || trimmed.starts_with("__asm__")
        || (contains_single_string_call(trimmed)
            && !contains_unquoted_plus(trimmed)
            && !contains_unquoted_comparison_operator(trimmed))
}

fn contains_unquoted_comparison_operator(line: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    let bytes = line.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        } else if matches!(
            bytes.get(index..index + 2),
            Some(b"==" | b"!=" | b"<=" | b">=")
        ) {
            return true;
        }
        index += 1;
    }
    false
}

fn contains_unquoted_plus(line: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for ch in line.chars() {
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        } else if ch == '+' {
            return true;
        }
    }
    false
}

fn contains_single_string_call(line: &str) -> bool {
    line.char_indices().any(|(index, ch)| {
        if ch != '(' || index == 0 {
            return false;
        }
        let before = line[..index].trim_end();
        if !before
            .chars()
            .next_back()
            .is_some_and(is_identifier_continue)
        {
            return false;
        }
        let Some(close) = matching_close_paren(line, index) else {
            return false;
        };
        let arg = line[index + 1..close].trim();
        arg.starts_with('"') && arg.ends_with('"')
    })
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
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
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
    pub(super) fn replayed_lambda_parameter_indent_spaces(
        &self,
        line_closed_lambda_parameter_list: bool,
        break_lambda_parameters: bool,
    ) -> Option<usize> {
        if !line_closed_lambda_parameter_list {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
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

fn continuation_indent_for_split(
    line: &str,
    head: &str,
    split: &SplitResult,
    base_indent_width: usize,
    limit_base_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
    indent_after_parens: bool,
    configured_indent: ContinuationIndent,
    configured_continuation_spaces: usize,
    current_indent_width: usize,
    following_split: bool,
    break_lambda_parameters: bool,
) -> Option<ContinuationIndent> {
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
    if operator_split {
        if let Some(spaces) = assignment_value_indent(line, base_indent_width)
            && !head_ends_assignment_operator(head)
        {
            return Some(ContinuationIndent::Spaces(spaces));
        }
        if let Some(spaces) = return_value_indent(line, base_indent_width) {
            return Some(ContinuationIndent::Spaces(spaces));
        }
    }

    let open_columns = unmatched_open_paren_columns(head);
    if let Some(spaces) = nested_new_continuation_indent(
        line,
        &open_columns,
        base_indent_width,
        limit_base_indent_width,
        current_indent_width,
        indent_width,
        max_continuation_indent,
        configured_continuation_spaces,
        head.trim_end().ends_with('('),
    ) {
        return Some(ContinuationIndent::Spaces(spaces));
    }
    let all_openers_over_max = !open_columns.is_empty()
        && open_columns
            .iter()
            .all(|column| *column + 1 >= max_continuation_indent);
    let assignment_member_call = top_level_assignment_index(line).is_some_and(|assignment| {
        open_columns.last().is_some_and(|open| {
            let call_head = &line[assignment + 1..*open];
            call_head.contains('.') || call_head.contains("->")
        })
    });
    if (all_openers_over_max || assignment_member_call)
        && let Some(spaces) = assignment_value_indent(line, base_indent_width)
    {
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
    base_indent_width: usize,
    limit_base_indent_width: usize,
    current_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
    configured_continuation_spaces: usize,
    trailing_open_paren: bool,
) -> Option<usize> {
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
        .find(|column| *column + 1 < max_continuation_indent)
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

fn adjust_overflowing_trailing_operator(
    line: &str,
    split_at: usize,
    priority: usize,
    width: usize,
) -> usize {
    let head = line[..split_at].trim_end();
    if priority == 70
        && head.ends_with('"')
        && line[..split_at]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
        && line[split_at..].starts_with('+')
        && line[..split_at + 1].trim_end().len() <= width
    {
        return split_at + 1;
    }
    if head.len() <= width || priority != 55 {
        return split_at;
    }
    let Some((last_index, _)) = head.char_indices().next_back() else {
        return split_at;
    };
    let Some((start, _, operator)) = operator_bounds_containing(line, last_index) else {
        return split_at;
    };
    if matches!(
        operator,
        "+" | "-" | "*" | "/" | "%" | "|" | "&" | "^" | "&&" | "||"
    ) && line[..start].trim_end().len() <= width
    {
        start
    } else {
        split_at
    }
}

fn split_result(line: &str, width: usize, prefer_logical_operator: bool) -> Option<SplitResult> {
    if line.len() <= width {
        return None;
    }

    let inline_brace_pair = inline_brace_pair_range(line);
    let inline_brace_header_fits =
        inline_brace_pair.is_some_and(|(start, _)| line[..start].trim_end().len() <= width);
    let comment_start = trailing_comment_start(line);
    let comment_limit = comment_start
        .map(|index| line[..index].trim_end().len())
        .unwrap_or(line.len());
    let side_comment_text_only_overflow = comment_start.is_some_and(|index| index <= width);
    let conditional_header_comment_only_overflow = side_comment_text_only_overflow
        && is_conditional_header_line(line)
        && comment_start.is_some_and(|start| start.saturating_sub(comment_limit) <= 1);
    let boundary = deferred_split_boundary(line, width);
    let template_ranges = template_argument_ranges(line);
    let mut candidates: Vec<(usize, usize, usize)> = Vec::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    let mut delimiter_depth = 0usize;
    for (index, ch) in line.char_indices() {
        if index > boundary
            && line[..index].trim_end().len() > width
            && !ends_single_string_call(&line[..index])
        {
            break;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }
        if in_block_comment {
            if ch == '/' && line[..index].ends_with('*') {
                in_block_comment = false;
            }
            continue;
        }
        if line[index..].starts_with("/*") {
            in_block_comment = true;
            continue;
        }
        if line[index..].starts_with("//") {
            break;
        }
        if matches!(ch, '"' | '\'') {
            quote = Some(ch);
            continue;
        }
        let candidate = pointer_cast_group_adjacent_split_point(line, index, ch, width)
            .or_else(|| logical_word_split_point(line, index, prefer_logical_operator))
            .or_else(|| split_point_at(line, index, ch, prefer_logical_operator, width))
            .or_else(|| pointer_cast_group_split_point(line, index, ch, width))
            .or_else(|| pointer_whitespace_split_point(line, index, ch, width));
        if let Some((split_at, priority)) = candidate
            && split_at < comment_limit
            && inline_brace_pair.is_none_or(|(start, end)| {
                if inline_brace_header_fits {
                    split_at >= end
                } else {
                    split_at <= start
                }
            })
            && !(side_comment_text_only_overflow
                && (priority <= 54 || conditional_header_comment_only_overflow))
            && !template_ranges
                .iter()
                .any(|&(start, end)| start <= index && index < end)
        {
            candidates.push((split_at, priority, delimiter_depth));
        }
        if let Some((split_at, priority)) =
            logical_fallback_split_point(line, index, prefer_logical_operator)
            && split_at < comment_limit
            && inline_brace_pair.is_none_or(|(start, end)| {
                if inline_brace_header_fits {
                    split_at >= end
                } else {
                    split_at <= start
                }
            })
            && !conditional_header_comment_only_overflow
            && !template_ranges
                .iter()
                .any(|&(start, end)| start <= index && index < end)
        {
            candidates.push((split_at, priority, delimiter_depth));
        }
        match ch {
            '(' | '[' => delimiter_depth += 1,
            ')' | ']' => delimiter_depth = delimiter_depth.saturating_sub(1),
            _ => {}
        }
    }

    let (mut structural, mut plain): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|(_, priority, _)| is_structural_split_class(*priority));
    structural.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| {
            let a_delimiter = line[..a.0].trim_end().ends_with(['(', '[']);
            let b_delimiter = line[..b.0].trim_end().ends_with(['(', '[']);
            if a.1 == 55 && !(a_delimiter && b_delimiter) {
                a.2.cmp(&b.2).then(b.0.cmp(&a.0))
            } else {
                b.0.cmp(&a.0)
            }
        })
    });
    plain.sort_by(|a, b| a.2.cmp(&b.2).then(b.0.cmp(&a.0)).then(b.1.cmp(&a.1)));
    structural
        .into_iter()
        .chain(plain)
        .find_map(|(split_at, priority, _)| {
            let split_at = adjust_overflowing_trailing_operator(line, split_at, priority, width);
            let head = line[..split_at].trim_end().to_string();
            let tail = line[split_at..].trim_start().to_string();
            let keeps_unsplittable_string = matches!(priority, 10 | 55 | 70)
                && head.ends_with('"')
                && tail.starts_with('+')
                && head.contains('"')
                && top_level_assignment_index(&head).is_none();
            let keeps_unsplittable_string_call = priority == 70
                && ends_single_string_call(&head)
                && ["==", "!=", "<", ">"]
                    .iter()
                    .any(|operator| tail.starts_with(operator));
            let keeps_unsplittable_head =
                head.len() > width && split_result(&head, width, prefer_logical_operator).is_none();
            if head.is_empty()
                || tail.is_empty()
                || matches!(
                    head.trim(),
                    "+" | "-" | "*" | "/" | "%" | "|" | "&" | "^" | "<<" | ">>" | "&&" | "||"
                )
                || head.len() > width
                    && !(keeps_unsplittable_string
                        || keeps_unsplittable_string_call
                        || keeps_unsplittable_head)
            {
                return None;
            }
            if split_at > 0
                && line[..split_at].ends_with('(')
                && is_single_string_call_at(line, split_at - 1)
            {
                return None;
            }
            if head.ends_with('(') && tail.contains('"') && contains_unquoted_plus(&tail) {
                return None;
            }
            if (head.ends_with('(') || head.ends_with('['))
                && tail.len() > width
                && split_result(&tail, width, prefer_logical_operator).is_none()
            {
                return None;
            }
            let anchor_column = unmatched_open_paren_column(&head);
            Some(SplitResult {
                head,
                tail,
                split_at,
                kind: split_result_kind(line, split_at, priority),
                priority,
                anchor_column,
                indent: anchor_column
                    .map(|column| ContinuationIndent::Spaces(column + 1))
                    .unwrap_or(ContinuationIndent::Level(1)),
            })
        })
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

fn is_structural_split_class(priority: usize) -> bool {
    matches!(priority, 39 | 55 | 59 | 60 | 70 | 75 | 79 | 80)
}

fn deferred_split_boundary(line: &str, width: usize) -> usize {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        let in_quote = quote.is_some();
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
        } else if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        }
        let is_code = !in_quote && !matches!(ch, '"' | '\'');
        if is_code && index + ch.len_utf8() > width {
            return index;
        }
    }
    line.len()
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

fn logical_fallback_split_point(
    line: &str,
    index: usize,
    prefer_after_logical: bool,
) -> Option<(usize, usize)> {
    if !prefer_after_logical {
        return None;
    }
    let rest = line.get(index..)?;
    if ["and", "or"].into_iter().any(|word| {
        rest.starts_with(word)
            && line[..index]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_identifier_continue(ch))
            && line[index + word.len()..]
                .chars()
                .next()
                .is_none_or(|ch| !is_identifier_continue(ch))
    }) {
        return Some((index, 79));
    }
    let (start, _, operator) = operator_bounds_containing(line, index)?;
    (index == start && matches!(operator, "&&" | "||")).then_some((start, 79))
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
    let Some(close) = line.len().checked_sub(1) else {
        return false;
    };
    if !line[close..].starts_with(')') {
        return false;
    }
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
        if matches!(operator, "::" | "->" | "<<" | ">>") {
            return None;
        }
        if is_pointer_split_operator(line, start, end, operator)
            || line[end..].trim_start().starts_with("/*")
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
        '(' if line[end..].trim_start().starts_with(')') => None,
        '(' if is_single_string_call_at(line, index) => None,
        '(' if is_lambda_capture_header(line[..index].trim_end()) => Some((end, 75)),
        '(' if is_function_call_split(line, index) => Some((end, 55)),
        '[' if is_objc_message_open(line, index) => None,
        '(' | '[' => Some((end, 40)),
        ' ' | '\t'
            if unmatched_open_bracket_column(&line[..index])
                .is_some_and(|open| is_objc_message_open(line, open)) =>
        {
            Some((end, 39))
        }
        ' ' | '\t' if !whitespace_touches_pointer_operator(line, index) => Some((end, 10)),
        _ => None,
    }
}

fn pointer_cast_group_adjacent_split_point(
    line: &str,
    index: usize,
    ch: char,
    width: usize,
) -> Option<(usize, usize)> {
    if !is_identifier_start(ch) {
        return None;
    }
    let head = line[..index].trim_end();
    let tail = line[index..].trim_start();
    if head.is_empty() || tail.is_empty() || tail.len() > width {
        return None;
    }
    (head.ends_with("*)") && head.contains('(')).then_some((index, 59))
}

fn pointer_cast_group_split_point(
    line: &str,
    index: usize,
    ch: char,
    width: usize,
) -> Option<(usize, usize)> {
    if !matches!(ch, ' ' | '\t') {
        return None;
    }
    let split_at = index + ch.len_utf8();
    let head = line[..split_at].trim_end();
    let tail = line[split_at..].trim_start();
    if head.is_empty() || tail.is_empty() || tail.len() > width {
        return None;
    }
    (head.ends_with("*)") && head.contains('(')).then_some((split_at, 59))
}

fn pointer_whitespace_split_point(
    line: &str,
    index: usize,
    ch: char,
    _width: usize,
) -> Option<(usize, usize)> {
    if !matches!(ch, ' ' | '\t')
        || !whitespace_precedes_pointer_operator(line, index)
        || unmatched_open_paren_column(&line[..index]).is_none()
    {
        return None;
    }
    let split_at = index + ch.len_utf8();
    let head = line[..split_at].trim_end();
    let tail = line[split_at..].trim_start();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    Some((split_at, 59))
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

fn whitespace_precedes_pointer_operator(line: &str, index: usize) -> bool {
    let Some((start, end, ch)) = line[index + 1..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(offset, ch)| {
            let start = index + 1 + offset;
            (start, start + ch.len_utf8(), ch)
        })
    else {
        return false;
    };
    matches!(ch, '*' | '&' | '^') && is_pointer_split_operator(line, start, end, &line[start..end])
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

#[cfg(test)]
mod tests {
    use super::*;

    fn split_pair(
        line: &str,
        width: usize,
        prefer_logical_operator: bool,
    ) -> Option<(String, String)> {
        split_result(line, width, prefer_logical_operator).map(|result| (result.head, result.tail))
    }

    #[test]
    fn split_result_uses_ordered_split_point_classes() {
        assert_eq!(
            split_pair("alpha beta gamma delta", 16, false),
            Some(("alpha beta gamma".to_string(), "delta".to_string()))
        );
        assert_eq!(
            split_pair("call(alpha, beta, gamma)", 18, false),
            Some(("call(alpha, beta,".to_string(), "gamma)".to_string()))
        );
        assert_eq!(
            split_pair("for(i = 0; i < n; i++)", 15, false),
            Some(("for(i = 0;".to_string(), "i < n; i++)".to_string()))
        );
        assert_eq!(
            split_pair("value = (alpha + beta)", 15, false),
            Some(("value = (alpha".to_string(), "+ beta)".to_string()))
        );
        assert_eq!(
            split_pair("value = call(alpha beta)", 15, false),
            Some(("value = call(".to_string(), "alpha beta)".to_string()))
        );
        assert_eq!(
            split_pair("call(alpha beta)", 6, false),
            Some(("call(".to_string(), "alpha beta)".to_string()))
        );
        assert_eq!(split_pair("char *name other", 7, false), None);
        assert_eq!(
            split_pair("alpha * beta", 8, false),
            Some(("alpha *".to_string(), "beta".to_string()))
        );
        assert_eq!(
            split_pair("alpha <= beta == gamma", 16, false),
            Some(("alpha <= beta ==".to_string(), "gamma".to_string()))
        );
        assert_eq!(
            split_pair("alpha && beta && gamma", 18, false),
            Some(("alpha && beta".to_string(), "&& gamma".to_string()))
        );
        assert_eq!(
            split_pair("alpha && beta && gamma", 18, true),
            Some(("alpha && beta &&".to_string(), "gamma".to_string()))
        );
        assert_eq!(
            split_pair("alpha || beta", 7, false),
            Some(("alpha".to_string(), "|| beta".to_string()))
        );
        assert_eq!(
            split_pair("alpha += beta", 7, false),
            Some(("alpha".to_string(), "+= beta".to_string()))
        );
        assert_eq!(
            split_pair("alpha -= beta", 7, false),
            Some(("alpha".to_string(), "-= beta".to_string()))
        );
        assert_eq!(
            split_pair("ptr->member tail", 12, false),
            Some(("ptr->member".to_string(), "tail".to_string()))
        );
        assert_eq!(
            split_pair("value++ tail", 6, false),
            Some(("value++".to_string(), "tail".to_string()))
        );
        assert_eq!(
            split_pair("value-- tail", 6, false),
            Some(("value--".to_string(), "tail".to_string()))
        );
    }

    #[test]
    fn split_result_records_comma_metadata() {
        let result = split_result("call(alpha, beta, gamma, delta)", 20, false).expect("split");

        assert_eq!(result.kind, SplitKind::Comma);
        assert_eq!(result.priority, 60);
        assert_eq!(result.anchor_column, Some(4));
        assert_eq!(result.indent, ContinuationIndent::Spaces(5));
    }

    #[test]
    fn inline_brace_body_allows_split_after_its_closing_brace() {
        assert_eq!(
            split_pair(
                "call([](){return alpha+beta+gamma;}, delta, epsilon)",
                40,
                false,
            ),
            Some((
                "call([](){return alpha+beta+gamma;},".to_string(),
                "delta, epsilon)".to_string(),
            )),
        );
    }

    #[test]
    fn split_result_records_logical_operator_metadata() {
        let result = split_result("alpha && beta && gamma", 14, false).expect("split");

        assert_eq!(result.kind, SplitKind::LogicalOperator);
        assert_eq!(result.priority, 80);
        assert_eq!(result.indent, ContinuationIndent::Level(1));
    }
}
