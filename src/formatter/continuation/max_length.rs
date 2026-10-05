use crate::config::{BraceStyle, FormatOptions, PointerAlign, ReferenceAlign};
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
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::syntax::language::{self, is_non_type_keyword, is_pointer_type_word};
use crate::formatter::syntax::{
    TemplateAngle, function_name_start, known_template_angle_role, scoped_name_is_constructor,
    template_openers,
};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::{
    advance_quoted_literal, last_unmatched_open_delimiter, unmatched_open_bracket_column,
    unmatched_open_paren_column, unmatched_open_paren_columns,
};
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::operators::{
    array_bound_operator_column, head_ends_assignment_operator,
};
use crate::formatter::tokens::pointers::is_pointer_declaration_segment;
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

#[derive(Default)]
pub(crate) struct MaxLengthLineState {
    suffix_width: usize,
    /// The braces and `while` attached after the statement on its line.
    while_suffix: Option<String>,
    objc_message_indent_spaces: Option<usize>,
    /// The part of a split line being published stands where astyle's
    /// stack or the syntax tree put it.
    anchored_part: bool,
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

    pub(crate) fn set_while_suffix(&mut self, suffix: Option<String>) {
        self.while_suffix = suffix;
    }

    fn objc_message_indent_spaces(&self) -> Option<usize> {
        self.objc_message_indent_spaces
    }

    pub(crate) fn set_objc_message_indent_spaces(&mut self, spaces: Option<usize>) {
        self.objc_message_indent_spaces = spaces;
    }

    pub(crate) fn take_anchored_part(&mut self) -> bool {
        std::mem::take(&mut self.anchored_part)
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
        let initializer_row = self.initializer_row();
        if should_skip_split(line) || initializer_row {
            self.push_output_line_with_indent(line, structural_level, indent);
            return;
        }
        let width = max_code_length.max(1);
        let configured_indent_width = indent.columns(self.options.indent_width);
        let base_indent_width = || {
            let prefix = match indent {
                ContinuationIndent::Level(level) => self.options.indent_prefix(level),
                ContinuationIndent::Spaces(spaces) => self
                    .options
                    .continuation_indent_prefix(structural_level, spaces),
            };
            let adjusted = self
                .layout
                .line_adjuster
                .clone()
                .adjust_line(format!("{prefix}{line}"));
            configured_indent_width.max(leading_visual_width(&adjusted, self.options.tab_width))
        };
        let brace_row_layout =
            self.max_length_brace_row_layout(line, structural_level, base_indent_width, width);
        // A line that fits the first row with any suffix that may join it
        // stays whole.
        let suffix_room = self.max_length_line.suffix_width().max(1).max(
            self.max_length_line
                .while_suffix
                .as_ref()
                .map_or(0, String::len),
        );
        if line.len() + suffix_room <= brace_row_layout.first_width {
            self.push_output_line_with_indent(line, structural_level, indent);
            return;
        }
        let base_indent_width = base_indent_width();
        // The space before a brace the source attached to the line counts
        // in its length though the brace moves to a line of its own.
        let broken_attached_brace = !line.trimmed_end().ends_with('{')
            && self.output.pending_tokens().is_some_and(|span| {
                matches!(
                    self.tree.tokens.get(span.last + 1),
                    Some(Token::Whitespace(_))
                ) && matches!(
                    self.tree.tokens.get(span.last + 2),
                    Some(Token::Symbol('{'))
                ) && self
                    .tree
                    .groups
                    .opened_at(span.last + 2)
                    .is_some_and(|group| {
                        matches!(
                            self.tree.blocks.kind(group),
                            Some(BlockKind::Control | BlockKind::FunctionBody)
                        )
                    })
            });
        let first_width = brace_row_layout.first_width;
        let suffix_width = if line.trimmed_end().ends_with(';') {
            self.max_length_line.suffix_width()
        } else {
            usize::from(broken_attached_brace)
        };
        let final_first_width = first_width.saturating_sub(suffix_width).max(1);
        let first_rules = SplitRules::new(self.options).with_offset(brace_row_layout.prefix_width);
        // A `while` attached after the statement's braces shares its line,
        // so the line splits where the whole of it would, and not before
        // the `while` when it would split there.
        let while_suffix = line
            .trimmed_end()
            .ends_with(';')
            .then(|| self.max_length_line.while_suffix.clone())
            .flatten();
        let split = match &while_suffix {
            Some(suffix) => split_result(
                &format!("{}{suffix}", line.trimmed_end()),
                first_width,
                first_rules.with_closers_following_statement(),
            )
            .filter(|split| split.split_at < line.trimmed_end().len())
            .and_then(|split| split_result_at(line, split.split_at, first_width, first_rules)),
            None => split_result(line, first_width, first_rules).or_else(|| {
                (suffix_width > 0).then(|| split_result(line, final_first_width, first_rules))?
            }),
        };
        let Some(split) = split else {
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
        let inline_body_indent_extra = (!splits_before_label(&split))
            .then(|| {
                max_length_inline_case_body_indent_extra(self.options, line)
                    .or_else(|| max_length_inline_access_body_indent_extra(self.options, line))
            })
            .flatten();
        if ends_statement_before_comment(&split.head) {
            next_indent = indent;
        }
        // A `goto` split from its label continues one level in.
        if split
            .head
            .trimmed_end()
            .strip_suffix("goto")
            .is_some_and(|before| !before.ends_with(is_identifier_continue))
        {
            next_indent = ContinuationIndent::Spaces(base_indent_width + self.options.indent_width);
        }
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
            let tail_width = width;
            let Some(split) = split_result(&tail, tail_width, SplitRules::new(self.options))
                .or_else(|| {
                    (suffix_width > 0).then(|| {
                        split_result(
                            &tail,
                            tail_width.saturating_sub(suffix_width).max(1),
                            SplitRules::new(self.options),
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
            if let Some(spaces) = self.split_part_indent(&tail) {
                next_indent = ContinuationIndent::Spaces(spaces);
                self.max_length_line.anchored_part = true;
            }
            self.push_output_line_with_indent(&split.head, next_structural_level, next_indent);
            self.max_length_line.anchored_part = false;
            tail = split.tail;
            next_indent = following_indent;
        }
        if !tail.trimmed().is_empty() {
            self.set_split_part_tokens(source_tokens, line, &tail);
            if let Some(spaces) = self.split_part_indent(&tail) {
                next_indent = ContinuationIndent::Spaces(spaces);
                self.max_length_line.anchored_part = true;
            }
            self.push_output_line_with_indent(&tail, next_structural_level, next_indent);
            self.max_length_line.anchored_part = false;
        }
    }

    /// Whether `group` is the body of an enum, whose rows astyle keeps whole
    /// like an initializer's.
    fn is_enum_body(&self, group: GroupId) -> bool {
        let open = self.tree.groups.get(group).open;
        self.tree.blocks.kind(group) == Some(BlockKind::Aggregate)
            && self.tree.blocks.owner(group).is_some_and(|owner| {
                self.tree.tokens[owner..open]
                    .iter()
                    .any(|token| matches!(token, Token::Word(word) if word == "enum"))
            })
    }

    /// The indent astyle's continuation stack, or else the syntax tree,
    /// gives the split `part`, once its source tokens are pending.
    fn split_part_indent(&self, part: &str) -> Option<usize> {
        let first = self.output.pending_tokens()?.first;
        let token = self.tree.tokens.get(first)?;
        if !part.trimmed_start().starts_with(&*token_text(token)) {
            return None;
        }
        self.split_part_stack_indent(first)
            .or_else(|| self.tree_anchor_indent(first, part))
    }

    /// Gives the part of the split `line` that starts with `part` the source
    /// tokens from its first one on.
    fn set_split_part_tokens(&mut self, source: Option<TokenSpan>, line: &str, part: &str) {
        let Some(source) = source else {
            return;
        };
        let part = part.trimmed_start();
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
            let Some(offset) = line[cursor..].find(&*text) else {
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
        let current = line.trimmed_start();
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
            .find(|line| !line.trimmed().is_empty())?;
        let previous_code = self.output.code_of(previous).trimmed_end();
        let previous_trimmed = previous_code.trimmed_start();
        if !previous_trimmed.starts_with("using ") || !previous_code.ends_with('=') {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.options.continuation_indent * self.options.indent_width,
        )
    }
}

/// A split right after a block comment that follows a finished statement
/// starts the next statement.
fn ends_statement_before_comment(head: &str) -> bool {
    let mut code = head.trimmed_end();
    if !code.ends_with("*/") {
        return false;
    }
    while code.ends_with("*/") {
        let Some(open) = code.rfind("/*") else {
            return false;
        };
        code = code[..open].trimmed_end();
    }
    code.is_empty() || code.ends_with([';', '{', '}'])
}

impl FormatEngine<'_> {
    /// Rows of an initializer, and of the braces astyle takes for one,
    /// stay whole.
    fn initializer_row(&self) -> bool {
        self.output
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
                ) || self.is_enum_body(group)
                    || self.is_directive_block(group)
                        && self
                            .tree
                            .previous_code_token(self.tree.groups.get(group).open)
                            .is_some_and(|before| {
                                matches!(self.tree.tokens[before], Token::Symbol('{'))
                            })
            })
    }

    /// Splits a row led by a block comment with code after it.
    pub(crate) fn split_comment_led_line(&self, line: &str) -> Option<(String, String, usize)> {
        let width = self.options.max_code_length?.max(1);
        let lead = &line[..line.len() - line.trimmed_start().len()];
        let code = line.trimmed_start();
        if !code.starts_with("/*") || code.contains('\x0c') || self.initializer_row() {
            return None;
        }
        let close = code.find("*/")?;
        if !holds_code(&code[close + 2..]) {
            return None;
        }
        let split = split_result(code, width, SplitRules::new(self.options))?;
        let lead_width = leading_visual_width(line, self.options.tab_width);
        let tail_spaces = if ends_statement_before_comment(&split.head) {
            lead_width
        } else {
            lead_width + self.options.continuation_indent * self.options.indent_width
        };
        Some((format!("{lead}{}", split.head), split.tail, tail_spaces))
    }
}

fn should_skip_split(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
            && trimmed
                .find("*/")
                .is_none_or(|close| !holds_code(&trimmed[close + 2..]))
        || trimmed
            .trim_start_matches('*')
            .chars()
            .next()
            .is_none_or(|ch| matches!(ch, ' ' | '\t' | '/'))
            && trimmed.starts_with('*')
        || trimmed.starts_with('#')
        || trimmed.starts_with("asm(")
        || trimmed.starts_with("__asm__")
}

fn is_single_string_call_at(line: &str, open: usize) -> bool {
    let Some(close) = matching_close_paren(line, open) else {
        return false;
    };
    let arg = line[open + 1..close].trimmed();
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
    let line = line.trimmed_start();
    let Some(open) = line.find('(') else {
        return false;
    };
    let Some(close) = matching_close_paren(line, open) else {
        return false;
    };
    scoped_name_is_constructor(line[..open].trimmed_end())
        && line[close + 1..].trimmed_start().starts_with(':')
}

pub(super) fn lambda_parameter_continuation_indent(
    line: &str,
    base_indent_width: usize,
    indent_width: usize,
    max_continuation_indent: usize,
    configured_continuation_spaces: usize,
    break_style: bool,
) -> Option<usize> {
    let line = line.trimmed_end();
    let before_lambda = line.strip_suffix('(')?.trimmed_end();
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
            previous.trimmed_start(),
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

fn splits_before_label(split: &SplitResult) -> bool {
    split.head.trimmed_end().ends_with(':')
        && ["case", "default"].into_iter().any(|word| {
            split.tail.starts_with(word)
                && !split.tail[word.len()..].starts_with(is_identifier_continue)
        })
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
    // Macro calls the head closes register nothing either.
    let macro_groups = head.contains('(')
        && unmatched_open_paren_columns(head).is_empty()
        && split.tail.starts_with(is_identifier_continue)
        && head.match_indices('(').all(|(at, _)| {
            // A function pointer's `(*name)` and the parameters after it.
            head[at + 1..].trimmed_start().starts_with('*')
                || head[..at].trimmed_end().ends_with(')')
                || head[..at]
                    .rsplit(|ch: char| !is_identifier_continue(ch))
                    .next()
                    .is_some_and(|word| {
                        !word.is_empty()
                            && !matches!(
                                word,
                                "if" | "while" | "for" | "switch" | "return" | "sizeof" | "catch"
                            )
                    })
        });
    // Member access registers nothing.
    let unaccessed_head = head.replace("->", ".");
    if !following_split
        && unaccessed_head.chars().all(|ch| {
            is_identifier_continue(ch)
                || ch.is_whitespace()
                || matches!(ch, '*' | '&' | ':' | '<' | '>' | '.')
                || macro_groups && matches!(ch, '(' | ')' | ',')
        })
        && head.ends_with(|ch: char| {
            is_identifier_continue(ch) || matches!(ch, '*' | '&' | '>') || macro_groups && ch == ')'
        })
        && !head
            .split(|ch: char| !is_identifier_continue(ch))
            .any(|word| {
                matches!(word, "return" | "case" | "goto")
                    || matches!(word, "struct" | "union" | "class")
                        && (macro_groups || !line.contains('('))
            })
    {
        return Some(ContinuationIndent::Spaces(base_indent_width));
    }
    // A label split from the labels before it lines up with them.
    if splits_before_label(split) {
        return Some(ContinuationIndent::Spaces(base_indent_width));
    }
    // An array bound continues under its trailing operator.
    if last_unmatched_open_delimiter(head)
        .is_some_and(|(open, at)| open == '[' && !head[..at].contains(']'))
        && let Some(column) = array_bound_operator_column(head)
    {
        return Some(ContinuationIndent::Spaces(base_indent_width + column));
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
    if head.trimmed_end().ends_with("<=>") {
        return Some(configured_indent);
    }
    if split.kind == SplitKind::Delimiter
        && head.trimmed_end().ends_with('(')
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
            && head.trimmed_end().ends_with('(')
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
    ) && !head.trimmed_end().ends_with(['(', '[']);
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
    if let Some(spaces) = nested_new_continuation_indent(
        line,
        &open_columns,
        inputs,
        head.trimmed_end().ends_with('('),
    ) {
        return Some(ContinuationIndent::Spaces(spaces));
    }
    let all_openers_over_max = !open_columns.is_empty()
        && open_columns
            .iter()
            .all(|column| *column >= max_continuation_indent);
    if all_openers_over_max && let Some(spaces) = assignment_value_indent(line, base_indent_width) {
        let call_body_extra =
            usize::from(head.trimmed_end().ends_with('(')) * configured_continuation_spaces;
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
        && head.trimmed_end().ends_with('(')
        && !head.contains(')')
    {
        return Some(ContinuationIndent::Spaces(
            current_indent_width + configured_continuation_spaces,
        ));
    }

    // A paren ending the line stacks one continuation past the indent
    // before it.
    if !following_split && head.trimmed_end().ends_with('(') {
        let columns = unmatched_open_paren_columns(head);
        let previous = match columns.len().checked_sub(2).map(|outer| columns[outer]) {
            Some(outer) => {
                let registered = if outer < max_continuation_indent {
                    base_indent_width + outer + 1
                } else {
                    base_indent_width + indent_width * 2
                };
                if language::is_header(trailing_word(head[..outer].trimmed_end())) {
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
    let Some(before) = head.trimmed_end().strip_suffix('(').map(str::trim_end) else {
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
    let return_type = before[..name_start].trimmed_end();
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
    let operand = line[..shift].trimmed_end();
    if operand.is_empty()
        || operand.contains_any_byte(b"(=?")
        || operand.trimmed_start().starts_with("return")
    {
        return None;
    }
    Some(base_indent_width + shift)
}

fn return_value_indent(line: &str, base_indent_width: usize) -> Option<usize> {
    let leading = line.len() - line.trimmed_start().len();
    let trimmed = line.trimmed_start();
    let tail = trimmed.strip_prefix("return")?;
    if tail.chars().next().is_some_and(is_identifier_continue) {
        return None;
    }
    let gap = tail.len() - tail.trimmed_start().len();
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
    let head = head.trimmed_end();
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
    if !line.contains('<') {
        return Vec::new();
    }
    let tokens = tokenize(line);
    let openers = template_openers(&tokens);
    let mut ranges = Vec::new();
    let mut outer_start = None;
    let mut depth = 0usize;
    let mut offset = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        let text = token_text(token);
        match known_template_angle_role(&tokens, index, depth, || openers[index]) {
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

fn split_result(line: &str, width: usize, rules: SplitRules) -> Option<SplitResult> {
    if line.len() <= width {
        return None;
    }
    astyle_split_result(line, width, rules)
}

fn split_result_kind(line: &str, split_at: usize, priority: usize) -> SplitKind {
    let head = line[..split_at].trimmed_end();
    let tail = line[split_at..].trimmed_start();
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
        .trimmed_end()
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
    let line = line.trimmed_end();
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
            return (line[..argument_start + argument_len].trimmed_end().len() > width)
                .then_some((end, 55));
        }
        if matches!(operator, "::" | "->" | "<<" | ">>" | "~" | "!")
            || matches!(operator, "&" | "-" | "+" | "*") && is_prefix_operator(&line[..start])
        {
            return None;
        }
        if is_pointer_split_operator(line, start, end, operator)
            || line[end..].trimmed_start().starts_with("/*")
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
            if line[..end].trimmed_end().len() > width && ends_single_string_call(&line[..start]) {
                Some((start, 70))
            } else {
                Some((end, 70))
            }
        } else if operator == "+"
            && line[..start]
                .trimmed_end()
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
        // A line splits before no closing brace.
        ';' if line[end..].trimmed_start().starts_with('}') => None,
        ';' => Some((end, 75)),
        // astyle splits after no paren that a literal or paren follows.
        '(' if line[end..]
            .trimmed_start()
            .starts_with([')', '(', '"', '\'']) =>
        {
            None
        }
        '(' if is_single_string_call_at(line, index) => None,
        '(' if is_lambda_capture_header(line[..index].trimmed_end()) => Some((end, 75)),
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
        ' ' | '\t' if line[end..].trimmed_start().starts_with('{') => None,
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
    let before = before.trimmed_end();
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
    let before = line[..start].trimmed_end();
    let after = line[end..].trimmed_start();
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
    let before = line[..operator_start].trimmed_end();
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
    let segment = before[delimiter_index + delimiter.len_utf8()..].trimmed();
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
    is_declaration_head(before[..open_index].trimmed_end())
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
    let return_type = head[..name_start].trimmed_end();
    let name = head[name_start..].trimmed_start();
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
            let word = trailing_word(before.trimmed_end());
            !word.is_empty()
                && before.trimmed_end().ends_with(word)
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

/// The options that shape where astyle splits a line.
#[derive(Clone, Copy, Default)]
struct SplitRules {
    break_after_logical: bool,
    pointer_to_type: bool,
    reference_to_type: bool,
    /// Columns ahead of the line on its output row, which count in the
    /// positions astyle weighs its split points by.
    offset: usize,
    /// The line's closing braces close blocks the statement before them
    /// sits in, not one-line blocks.
    closers_follow_statement: bool,
}

impl SplitRules {
    fn new(options: &FormatOptions) -> Self {
        let pointer_to_type = options.pointer_align == PointerAlign::Type;
        Self {
            break_after_logical: options.break_after_logical,
            pointer_to_type,
            reference_to_type: options.reference_align == ReferenceAlign::Type
                || options.reference_align == ReferenceAlign::SameAsPointer && pointer_to_type,
            offset: 0,
            closers_follow_statement: false,
        }
    }

    fn with_offset(self, offset: usize) -> Self {
        Self { offset, ..self }
    }

    fn with_closers_following_statement(self) -> Self {
        Self {
            closers_follow_statement: true,
            ..self
        }
    }
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
fn astyle_split_point(line: &str, width: usize, rules: SplitRules) -> Option<usize> {
    let break_after_logical = rules.break_after_logical;
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
    let mut unbroken_depth = if rules.closers_follow_statement {
        0
    } else {
        unopened_closing_braces(line)
    };
    let mut clear_after_brace = false;
    // A case label registers no points up to its colon.
    let mut in_case = false;
    while index < bytes.len() {
        let byte = bytes[index];
        let mut end = index + 1;
        let case_points = (fit, pending);
        let mut in_label = in_case;
        if quote.is_none() && !in_comment {
            if in_case && byte == b':' && peek(index + 1) != b':' {
                in_case = false;
            } else if !in_case
                && matches!(previous_non_space, b' ' | b'{' | b':' | b';' | b'}')
                && ["case", "default"].into_iter().any(|word| {
                    line[index..].starts_with(word)
                        && !index
                            .checked_sub(1)
                            .is_some_and(|before| name_char(bytes[before]))
                        && !bytes
                            .get(index + word.len())
                            .is_some_and(|after| name_char(*after))
                })
            {
                in_case = true;
                in_label = true;
            }
        }
        // The brace itself may still split; what follows it drops the points.
        if std::mem::take(&mut clear_after_brace) && !(byte == b'}' && unbroken_depth == 1) {
            fit = [0; 5];
            pending = [0; 5];
        }
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
            if byte == b'{' && (unbroken_depth > 0 || holds_code(&line[end..])) {
                // An initializer's brace drops them already.
                if unbroken_depth == 0
                    && matches!(previous_non_space, b'=' | b',' | b'(' | b'{' | b'[')
                {
                    fit = [0; 5];
                    pending = [0; 5];
                } else {
                    clear_after_brace = unbroken_depth == 0;
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
                        // Nor before a pointer or reference aligned to its type.
                        let to_type = match next {
                            b'*' => rules.pointer_to_type,
                            b'&' => rules.reference_to_type,
                            _ => false,
                        };
                        if !matches!(next, b')' | b'(' | b':')
                            && previous_non_space != b'('
                            && (!to_type || potential_operator(previous_non_space))
                        {
                            register((&mut fit, &mut pending), WHITESPACE, index, index <= width);
                        }
                    }
                    b')' => {
                        let member_access =
                            next == b'-' && line[end..].trimmed_start().starts_with("->");
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
        if in_label {
            (fit, pending) = case_points;
        }
        if end > width {
            let split = astyle_find_split_point(&fit, &pending, width, end, rules.offset, || {
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

fn holds_code(text: &str) -> bool {
    let mut rest = text.trimmed_start();
    loop {
        if rest.is_empty() || rest.starts_with("//") {
            return false;
        }
        let Some(body) = rest.strip_prefix("/*") else {
            break;
        };
        let Some(close) = body.find("*/") else {
            return false;
        };
        rest = body[close + 2..].trimmed_start();
    }
    // A `#` lexes as a directive at the start of what is lexed.
    !rest.starts_with('#') || tokenize(text).iter().any(is_code_token)
}

fn unopened_closing_braces(line: &str) -> usize {
    if !line.contains('}') {
        return 0;
    }
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
    offset: usize,
    at_line_end: impl Fn() -> bool,
) -> usize {
    let position = |at: usize| if at == 0 { 0 } else { at + offset };
    let full_width = width + offset;
    let mut split = fit[SEMI];
    if position(fit[AND_OR]) >= ASTYLE_MIN_CODE_LENGTH {
        split = fit[AND_OR];
    }
    if position(split) < ASTYLE_MIN_CODE_LENGTH {
        split = fit[WHITESPACE];
        if fit[PAREN] > split || position(fit[PAREN]) as f64 >= full_width as f64 * 0.7 {
            split = fit[PAREN];
        }
        if fit[COMMA] > split || position(fit[COMMA]) as f64 >= full_width as f64 * 0.3 {
            split = fit[COMMA];
        }
    }
    if position(split) < ASTYLE_MIN_CODE_LENGTH {
        return [SEMI, AND_OR, COMMA, PAREN, WHITESPACE]
            .into_iter()
            .map(|kind| pending[kind])
            .filter(|&at| at > 0)
            .min()
            .unwrap_or(0);
    }
    if length - split > full_width && at_line_end() {
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

fn astyle_split_result(line: &str, width: usize, rules: SplitRules) -> Option<SplitResult> {
    let split_at = astyle_split_point(line, width, rules)?;
    split_result_at(line, split_at, width, rules)
}

fn split_result_at(
    line: &str,
    split_at: usize,
    width: usize,
    rules: SplitRules,
) -> Option<SplitResult> {
    let break_after_logical = rules.break_after_logical;
    let head = line[..split_at].trimmed_end().to_string();
    let tail = line[split_at..].trimmed_start().to_string();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    // The split takes the class of the strongest point at it.
    let head_end = line[..split_at].trimmed_end().len();
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
            .filter(|&(at, _)| line[..at].trimmed_end().len() == head_end)
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
        let result = split_result("call(alpha, beta, gamma, delta)", 20, SplitRules::default())
            .expect("split");

        assert_eq!(result.kind, SplitKind::Comma);
        assert_eq!(result.priority, 60);
        assert_eq!(result.anchor_column, Some(4));
        assert_eq!(result.indent, ContinuationIndent::Spaces(5));
    }

    #[test]
    fn split_result_records_logical_operator_metadata() {
        let result =
            split_result("alpha && beta && gamma", 14, SplitRules::default()).expect("split");

        assert_eq!(result.kind, SplitKind::LogicalOperator);
        assert_eq!(result.priority, 80);
        assert_eq!(result.indent, ContinuationIndent::Level(1));
    }
}
