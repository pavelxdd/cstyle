//! Indentation of continuation lines and maximum-length line splitting.

use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::has_hash_outside_literals;

use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::formatter::constructs::class_declarations::code_opens_class_base_clause;
use crate::formatter::constructs::headers::is_header;
use crate::formatter::constructs::return_types::is_return_type_line;
use crate::formatter::constructs::switch_cases::find_case_colon;
use crate::formatter::continuation::max_length::lambda_parameter_continuation_indent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::frame::{ColonRole, LogicalOperator};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::syntax::language::{is_leading_continuation_operator, is_macro_like_word};
use crate::formatter::syntax::{
    assignment_declarator_offset, function_head_has_assignment, function_name_start, language,
};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{
    advance_quoted_literal, is_comment_line, is_comment_only_line, line_comment_split_limit,
    trailing_comment_split_limit, unmatched_open_paren_column, unmatched_open_paren_columns,
};
use crate::formatter::tokens::operators::{
    array_bound_operator_column, find_assignment_operator, head_ends_binary_operator,
    head_starts_binary_operator, starts_with_chain_operator,
};
use crate::formatter::tokens::pointers::is_pointer_declaration_segment;
use crate::source::lex::{is_identifier_continue, is_word_char};

pub(crate) mod call_arguments;
pub(crate) mod max_length;
pub(crate) mod operator_chains;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct ContinuationIndentState {
    pub(crate) pending_literal_continuation_indent_spaces: Option<usize>,
    pub(crate) next_line_indent: Option<usize>,
    pub(crate) next_line_indent_spaces: Option<usize>,
    pub(crate) next_input_line_continuation_indent: Option<ContinuationIndent>,
    pub(crate) input_line_continuation_indent: Option<ContinuationIndent>,
    pub(crate) logical_chain_indent_spaces: Option<usize>,
    pub(crate) after_one_shot_continuation_indent: Option<ContinuationIndent>,
    pub(crate) clear_continuation_after_line: Option<usize>,
}

impl ContinuationIndentState {
    /// Starts the next line at an exact column and drops any pending level.
    pub(crate) fn set_next_line_spaces(&mut self, spaces: usize) {
        self.next_line_indent = None;
        self.next_line_indent_spaces = Some(spaces);
    }

    /// Starts the next line at an indent level and drops any pending column.
    pub(crate) fn set_next_line_level(&mut self, level: usize) {
        self.next_line_indent = Some(level);
        self.next_line_indent_spaces = None;
    }

    /// Leaves the next line at its structural indent.
    pub(crate) fn clear_next_line(&mut self) {
        self.next_line_indent = None;
        self.next_line_indent_spaces = None;
    }
}

fn declaration_comma_continuation_column(line: &str) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let comma = match chars.len().checked_sub(1) {
        Some(comma) if comma > 0 => comma,
        _ => return 0,
    };
    if !is_word_char(chars[0]) {
        return 0;
    }
    let mut column = 0;
    while column < comma && is_word_char(chars[column]) {
        column += 1;
    }
    column += 1;
    if column >= comma || column < 4 {
        return 0;
    }
    while column < comma && matches!(chars[column], ' ' | '\t') {
        column += 1;
    }
    if column >= comma {
        return 0;
    }
    column
}

impl FormatEngine<'_> {
    pub(crate) fn reset_continuation_after_empty_line(&mut self) {
        let in_continuation = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim_ascii().is_empty())
            .is_some_and(|previous| {
                let code = self.output.code_of(previous).trim_ascii_end();
                let trimmed = code.trim_ascii_start();
                !code.is_empty()
                    && (head_ends_binary_operator(code)
                        || code.ends_with(',')
                        || self.open_paren_column_of(code).is_some()
                        || starts_with_chain_operator(trimmed)
                        || trimmed.starts_with(['+', '-', '*', '/', '%']))
            })
            || self
                .layout
                .nesting
                .current_continuation_indent_spaces()
                .is_some();
        if in_continuation {
            return;
        }
        self.layout.continuation_indent.clear_next_line();
        self.layout.nesting.clear_continuation_indents();
        operator_chains::clear_logical_chain_indent(
            &mut self.layout.continuation_indent.logical_chain_indent_spaces,
        );
    }

    pub(crate) fn recent_paren_continuation_indent_spaces(&self) -> Option<usize> {
        // An open paren in comment text continues nothing.
        if self.output.pending_tokens().is_some() && !self.pending_line_in_parens() {
            return None;
        }
        let len = self.output.len();
        let scoped = self.output.scoped();
        let start = len - scoped.len();
        // Parens that later lines close match the last opens before them.
        let mut later_closes = 0usize;
        for (offset, line) in scoped.iter().enumerate().rev().take(12) {
            if line.trim_ascii().is_empty() {
                return None;
            }
            // Lines a comment spans hold no code.
            if self.output.comment_start_index(start + offset) != start + offset {
                continue;
            }
            let code = self.output.code_of(line).trim_ascii_end();
            // Neither does a directive, nor the lines it continues over.
            if self.output.is_directive_line(start + offset)
                || code.ends_with(';')
                || code.ends_with('{')
                || code.ends_with('}')
            {
                return None;
            }
            if is_comment_line(line.trim_ascii_start()) || code.trim_ascii().is_empty() {
                continue;
            }
            let (closes, mut opens) = self.paren_imbalance_of(code);
            let matched = later_closes.min(opens.len());
            opens.truncate(opens.len() - matched);
            later_closes = later_closes - matched + closes;
            // An open paren ending its line sets no column.
            if let Some(&open) = opens
                .iter()
                .rev()
                .find(|&&open| code[open + 1..].chars().any(|ch| !ch.is_whitespace()))
            {
                let after_open = &code[open + 1..];
                let content_offset = after_open
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(0, |(offset, _)| offset);
                return Some(visual_width_from(
                    &code[..open + 1 + content_offset],
                    0,
                    self.options.tab_width,
                ));
            }
        }
        None
    }

    pub(crate) fn active_output_paren_continuation_indent_spaces(&self) -> Option<usize> {
        let start = self
            .output
            .iter()
            .rposition(|line| {
                let code = self.output.code_of(line).trim_ascii_end();
                code.ends_with([';', '{', '}'])
            })
            .map_or(0, |index| index + 1);
        let mut openers = Vec::new();
        for line in &self.output[start..] {
            let code = self.output.code_of(line).trim_ascii_end();
            let (closes, opens) = self.paren_imbalance_of(code);
            for _ in 0..closes {
                openers.pop();
            }
            let line_indent = leading_visual_width(line, self.options.tab_width);
            for open in opens {
                openers.push((
                    visual_width_from(&code[..open], 0, self.options.tab_width),
                    line_indent,
                ));
            }
        }
        let &(column, line_indent) = openers.last()?;
        let opener_indent = column + 1;
        let base = self.continuation_base_indent() * self.options.indent_width;
        if opener_indent < self.options.max_continuation_indent {
            return Some(opener_indent);
        }
        if line_indent > base {
            return Some(line_indent + self.options.indent_width * 2);
        }
        openers[..openers.len() - 1]
            .iter()
            .rev()
            .map(|(column, _)| column + 1)
            .find(|spaces| *spaces < self.options.max_continuation_indent)
            .or(Some(base + self.options.indent_width * 2))
    }

    pub(crate) fn line_aligns_to_open_paren_content(&self, line: &str) -> bool {
        if line.trim_ascii().is_empty() {
            return false;
        }
        let mut balance: i32 = 0;
        for index in (0..self.output.len()).rev() {
            let meta = *self.output.brace_meta(index);
            if self.output.code_trimmed(index).is_empty() {
                continue;
            }
            balance += meta.paren_open_count as i32 - meta.paren_closes as i32;
            if balance > 0 {
                let Some(column) = meta.paren_last_open_column else {
                    return false;
                };
                let code = self.output.code(index);
                return code[column + 1..].chars().any(|ch| !ch.is_whitespace());
            }
        }
        false
    }

    pub(crate) fn is_top_level_table_macro_row(&self) -> bool {
        self.layout.nesting.brace_type_stack.is_empty()
            && self.layout.nesting.paren_depth == 0
            && self.current.trim_ascii_start().starts_with('.')
    }

    fn line_ends_with_bare_angle_operator(&self) -> bool {
        if self.current.trim_ascii_start().starts_with("template") {
            return false;
        }
        let trimmed = self.current.trim_ascii_end();
        (trimmed.ends_with('<') && !trimmed.ends_with("<<"))
            || (trimmed.ends_with('>') && !trimmed.ends_with(">>") && !trimmed.ends_with("->"))
    }

    fn current_ends_with_pointer_declarator(&self) -> bool {
        let trimmed = self.current.trim_ascii_end();
        if trimmed.ends_with("||") {
            return false;
        }
        if let Some(before) = trimmed.strip_suffix("&&")
            && !is_pointer_declaration_segment(before.trim_ascii_end())
        {
            return false;
        }
        self.layout.nesting.paren_depth == 0
            && trimmed.ends_with(['*', '&', '^'])
            && self.looks_like_pointer_declaration_context()
    }

    pub(crate) fn is_continuation_break(&self) -> bool {
        if self.is_complete_template_declaration_line() {
            return false;
        }
        let trimmed = self.current.trim_ascii_end();
        if self.current_is_preindented && is_comment_only_line(trimmed) && trimmed.ends_with("*/") {
            return false;
        }
        if self.template_continuation_closes_on_line(trimmed) {
            return false;
        }
        let code_before_trailing_comment = self.output.code_of(trimmed).trim_ascii_end();
        let line_comment_limit = line_comment_split_limit(trimmed);
        let code_before_line_comment = trimmed[..line_comment_limit].trim_ascii_end();
        if line_comment_limit < trimmed.len()
            && has_hash_outside_literals(code_before_line_comment)
            && !code_before_line_comment.trim_ascii_start().starts_with('#')
        {
            return false;
        }
        if self.layout.indentation.statement_depth() == 0
            && (trimmed.starts_with("//")
                || trimmed.ends_with(';')
                || trimmed.starts_with("/*") && trimmed.ends_with("*/")
                || trimmed.ends_with("*/") && code_before_trailing_comment.ends_with(';'))
        {
            return false;
        }
        if ends_logical_operator(code_before_trailing_comment)
            && (!self.current_ends_with_pointer_declarator()
                || self.logical_chain_head_indent_spaces().is_some())
        {
            return true;
        }
        if head_ends_binary_operator(code_before_trailing_comment)
            && self.logical_chain_head_indent_spaces().is_some()
        {
            return true;
        }
        if code_before_trailing_comment.ends_with(',')
            && self.in_enum_declaration_brace()
            && (code_before_trailing_comment.contains('{')
                || self.current_line_indent_spaces()
                    > self.layout.indentation.indent() * self.options.indent_width)
        {
            return true;
        }
        if (self.next_line.leads_with_open_paren || self.next_line.leads_with_noexcept)
            && self.current_looks_like_split_function_head()
        {
            return false;
        }
        let bare_question_line = trimmed == "?";
        let bare_leading_operator_line =
            is_leading_continuation_operator(trimmed) || matches!(trimmed, "*" | "&" | "^");
        self.layout.indentation.statement_depth() > 0
            || (self.next_line.leads_with_assignment
                && self.layout.indentation.statement_depth() == 0
                && !self.current.trim_ascii().is_empty())
            || (matches!(self.layout.previous, PreviousToken::Operator)
                && !bare_leading_operator_line
                && !self.line_ends_with_bare_angle_operator()
                && !self.current_ends_with_pointer_declarator()
                && !trimmed.ends_with("->")
                && (self.layout.indentation.statement_depth() != 0 || !trimmed.ends_with("::")))
            || (matches!(self.layout.previous, PreviousToken::Comma)
                && !self.in_initializer_brace()
                && !self.in_enum_declaration_brace())
            || self.macro_call_argument_indent_spaces().is_some()
            || self.has_active_continuation_indent()
            || self.current_is_operator_led_continuation()
            || self
                .split_aggregate_declaration_name_indent_spaces()
                .is_some()
            || self.assignment_continuation_indent_spaces().is_some()
            || self.return_continuation_indent_spaces().is_some()
            || self
                .operator_led_return_continuation_indent_spaces()
                .is_some()
            || self.layout.nesting.question_depth > 0
            || self.is_stream_continuation_break()
            || (self.layout.in_class_base_clause && !self.next_line.leads_with_open_brace)
            || (!bare_question_line && self.current.trim_ascii_end().ends_with('?'))
            || self.current.trim_ascii_end().ends_with(" :")
    }

    fn current_looks_like_split_function_head(&self) -> bool {
        let line = self.current.trim_ascii_end();
        if line.is_empty() || function_head_has_assignment(line) || is_header(self.options, line) {
            return false;
        }
        if self.open_paren_column_of(line).is_some() {
            return false;
        }
        let first_word = line
            .split(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty());
        if first_word.is_some_and(|word| {
            is_header(self.options, word)
                || matches!(
                    word,
                    "return" | "throw" | "delete" | "new" | "co_return" | "co_await" | "co_yield"
                )
        }) {
            return false;
        }
        let Some(name_start) = function_name_start(line) else {
            return false;
        };
        !line[..name_start].trim_ascii_end().is_empty()
            && !line[name_start..].trim_ascii_start().is_empty()
    }

    pub(crate) fn for_header_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        let trimmed = line.trim_ascii_start();
        if !trimmed
            .strip_prefix("for")
            .is_some_and(|tail| tail.trim_ascii_start().starts_with('('))
        {
            return None;
        }
        let prefix_len = line.len() - trimmed.len();
        let base_spaces = if prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            prefix_len
        };
        let open = trimmed.find('(')?;
        Some(self.apply_min_conditional_indent(base_spaces, base_spaces + open + 1))
    }

    pub(crate) fn current_line_indent_spaces(&self) -> usize {
        let split_else_extra =
            self.preprocessor.split_else.extra_levels * self.options.indent_width;
        if let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces {
            return spaces + split_else_extra;
        }
        if let Some(level) = self.layout.continuation_indent.next_line_indent {
            return level * self.options.indent_width + split_else_extra;
        }
        self.continuation_base_indent() * self.options.indent_width + split_else_extra
    }

    /// [`Self::current_line_indent_spaces`] for a line that starts a
    /// statement, which the `if`s an `else` broke from stand deeper.
    pub(crate) fn statement_line_indent_spaces(&self) -> usize {
        let else_if_break_extra = if self.options.no_indent_if_after_else
            || self
                .layout
                .continuation_indent
                .next_line_indent_spaces
                .is_some()
            || self.layout.continuation_indent.next_line_indent.is_some()
        {
            0
        } else {
            self.layout.else_if_break_depths.len() * self.options.indent_width
        };
        self.current_line_indent_spaces() + else_if_break_extra
    }

    pub(crate) fn inline_brace_call_indent_spaces(&self, current: &str) -> Option<usize> {
        let is_current_line = std::ptr::eq(current.as_ptr(), self.current.as_ptr());
        let current_line_brace = if is_current_line {
            Some(self.current_last_open_brace()?)
        } else {
            None
        };
        let current = current.trim_end_matches('(').trim_ascii_end();
        let current_prefix_len = current.len() - current.trim_ascii_start().len();
        let mut line_indent_spaces = if current_prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            current_prefix_len
        };
        let code = current.trim_ascii_start();
        line_indent_spaces = line_indent_spaces.max(self.token_input.token_source_line_indent);
        let brace = if let Some(brace) = current_line_brace {
            if brace >= current.len() {
                return None;
            }
            brace.checked_sub(current_prefix_len)?
        } else {
            code.rfind('{')?
        };
        let after_brace = &code[brace + 1..];
        if after_brace.is_empty() || after_brace.chars().any(char::is_whitespace) {
            return None;
        }
        Some(line_indent_spaces + visual_width_from(&code[..brace + 1], 0, self.options.tab_width))
    }

    pub(crate) fn register_current_continuation_indent(&mut self, next: Option<&Token>) {
        let current_prefix_len = self.current.len() - self.current.trim_ascii_start().len();
        let mut line_indent_spaces = if current_prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            current_prefix_len
        };
        if !self.options.indent_after_parens
            && let Some(ternary_indent) = self.ternary_colon_branch_render_indent()
        {
            line_indent_spaces = line_indent_spaces.max(ternary_indent);
        }
        let constructor_member_base = (self.layout.nesting.paren_depth == 1)
            .then(|| self.constructor_member_line_base_indent_spaces())
            .flatten();
        if let Some(base) = constructor_member_base {
            line_indent_spaces = base;
        }
        let previous_indent = if constructor_member_base.is_some() {
            line_indent_spaces
        } else {
            self.layout
                .nesting
                .current_continuation_indent_spaces()
                .unwrap_or(line_indent_spaces)
        };
        let continuation_spaces = self.options.continuation_indent * self.options.indent_width;
        let current_columns = if current_prefix_len == 0 {
            self.current_visual_width_from(line_indent_spaces)
        } else {
            visual_width_from(
                self.current.trim_ascii_start(),
                line_indent_spaces,
                self.options.tab_width,
            )
        };
        let has_next = !matches!(next, None | Some(Token::Newline));
        let attach_delta = if self.options.indent_after_parens {
            0
        } else {
            self.attached_return_type_indent_delta(line_indent_spaces)
                .unwrap_or(0)
        };
        let mut spaces = if !has_next {
            let base = self
                .layout
                .nesting
                .current_continuation_indent_spaces()
                .or_else(|| self.return_continuation_indent_spaces())
                .or_else(|| self.assignment_continuation_indent_spaces())
                .unwrap_or(previous_indent);
            base + continuation_spaces + attach_delta
        } else if self.options.indent_after_parens {
            if self.return_continuation_indent_spaces().is_some()
                && previous_indent == line_indent_spaces
            {
                line_indent_spaces + continuation_spaces * 2
            } else {
                previous_indent + continuation_spaces
            }
        } else {
            self.apply_min_conditional_indent(
                line_indent_spaces,
                line_indent_spaces + current_columns + attach_delta,
            )
        };
        if !has_next
            && let Some(spaces_for_params) =
                self.union_return_parameter_continuation_indent_spaces(line_indent_spaces)
        {
            spaces = spaces_for_params;
        }
        if !has_next
            && self.current.trim_ascii_end().ends_with('(')
            && contains_word(&self.current, "new")
            && self
                .paren_imbalance_of(self.current.trim_ascii_end())
                .1
                .len()
                == 1
            && let Some(assignment_spaces) = self.assignment_continuation_indent_spaces()
        {
            spaces = assignment_spaces;
        }
        let inline_brace_call_indent = if !has_next && self.current.trim_ascii_end().ends_with('(')
        {
            self.inline_brace_call_indent_spaces(&self.current)
        } else {
            None
        };
        if let Some(indent) = inline_brace_call_indent {
            spaces = indent + continuation_spaces;
        }
        let statement_base_spaces = self.continuation_base_indent() * self.options.indent_width;
        let trailing_open_paren = !has_next && self.current.trim_ascii_end().ends_with('(');
        let trailing_first_paren = trailing_open_paren && self.layout.nesting.paren_depth == 1;
        let trailing_assignment = !has_next && {
            let head = self.current.trim_ascii_end();
            head.ends_with('=')
                && !head.ends_with("==")
                && !head.ends_with("!=")
                && !head.ends_with("<=")
                && !head.ends_with(">=")
        };
        if trailing_assignment
            && let Some(spaces_for_enum) = self.run_in_enum_assignment_continuation_spaces()
        {
            spaces = spaces_for_enum;
        }
        let over_max =
            spaces.saturating_sub(statement_base_spaces) > self.options.max_continuation_indent;
        let capped_over_max = (has_next || trailing_open_paren || trailing_assignment)
            && over_max
            && inline_brace_call_indent.is_none();
        if capped_over_max {
            let fallback = line_indent_spaces + self.options.indent_width * 2;
            if self.options.indent_after_parens {
                spaces = statement_base_spaces
                    + self
                        .options
                        .max_continuation_indent
                        .max(self.options.indent_width * 2);
            } else {
                let opened_on_statement_line = line_indent_spaces <= statement_base_spaces;
                let enclosing_paren = (opened_on_statement_line
                    && self.layout.nesting.paren_depth >= 2)
                    .then(|| self.layout.nesting.current_continuation_indent_spaces())
                    .flatten();
                let enclosing_paren = if self.current_is_conditional_header_continuation() {
                    enclosing_paren.map(|column| column.max(fallback))
                } else {
                    enclosing_paren
                };
                spaces = if has_next {
                    enclosing_paren.unwrap_or_else(|| {
                        self.assignment_rhs_continuation_column()
                            .filter(|rhs| {
                                rhs.saturating_sub(statement_base_spaces)
                                    <= self.options.max_continuation_indent
                            })
                            .map_or(fallback, |rhs| rhs.max(fallback))
                    })
                } else {
                    fallback
                };
            }
        }
        if self.layout.nesting.paren_depth == 1 {
            self.layout.nesting.trim_to_current_statement_continuation();
        }
        if capped_over_max || (trailing_first_paren && over_max) {
            self.layout
                .nesting
                .push_continuation_indent_spaces_raw(spaces);
        } else {
            self.layout
                .nesting
                .register_continuation_indent_spaces(spaces);
        }
    }

    fn ternary_colon_branch_render_indent(&self) -> Option<usize> {
        if let Some(spaces) = self.ternary_colon_branch_frame_indent() {
            return Some(spaces);
        }
        let current = self.current.trim_ascii_start();
        if current.starts_with([')', '}', '?', ':']) {
            return None;
        }
        let previous = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim_ascii().is_empty())?;
        let previous_code = self.output.code_of(previous).trim_ascii_end();
        if !previous_code.ends_with(':') || !previous_code.contains('?') {
            return None;
        }
        let unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        self.open_paren_column_of(previous_code)
            .map(|open| open + 1 + unindent)
    }

    fn ternary_colon_branch_frame_indent(&self) -> Option<usize> {
        let current = self.current.trim_ascii_start();
        if current.starts_with([')', '}', '?', ':']) {
            return None;
        }
        let frame = self.layout.frame_stack.active_ternary()?;
        if frame.colon_role != Some(ColonRole::Ternary) {
            return None;
        }
        let unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        frame
            .parent_delimiter
            .and_then(|id| self.layout.frame_stack.delimiter_by_id(id))
            .map(|delimiter| delimiter.opener_output_column + 1 + unindent)
    }

    fn union_return_parameter_continuation_indent_spaces(
        &self,
        line_indent_spaces: usize,
    ) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        if !line.ends_with('(') {
            return None;
        }
        let before = line[..line.len() - 1].trim_ascii_end();
        if before.is_empty() || before.contains('=') {
            return None;
        }
        let name_start = function_name_start(before)?;
        let return_type = before[..name_start].trim_ascii_end();
        let name = before[name_start..].trim_ascii_start();
        if return_type.is_empty() || name.is_empty() || is_header(self.options, name) {
            return None;
        }
        return_type
            .split_whitespace()
            .any(|word| word == "union")
            .then_some(
                line_indent_spaces
                    + self.options.continuation_indent * self.options.indent_width * 2,
            )
    }

    fn has_active_continuation_indent(&self) -> bool {
        self.layout
            .nesting
            .current_continuation_indent_spaces()
            .is_some()
            && !self.in_initializer_brace()
            && !self.innermost_init_block_brace()
            && !self.in_aggregate_declaration_brace()
            && (self.layout.command_state.current_header.is_none()
                || self.layout.indentation.statement_depth() != 0)
    }

    pub(crate) fn current_ends_base_clause_colon(&self) -> bool {
        if self.layout.nesting.question_depth > 0 {
            return false;
        }
        let line = self.current.trim_ascii_end();
        let Some(before) = line.strip_suffix(':') else {
            return false;
        };
        let before = before.trim_ascii_end();
        if before.is_empty() || before.ends_with(':') || before.contains('?') {
            return false;
        }
        if before.ends_with(')') {
            return true;
        }
        let statement =
            crate::formatter::text::line_scan::statement_tail(before).trim_ascii_start();
        statement
            .split(|ch: char| !is_identifier_continue(ch))
            .any(|word| matches!(word, "class" | "struct" | "union" | "interface"))
    }

    fn class_base_clause_indent_spaces(&self) -> usize {
        let current_code = &self.current[..self.current_trailing_comment_split_limit()];
        if self.current_opens_class_base_clause()
            || code_opens_class_base_clause(current_code.trim_ascii_end())
            || self.current_ends_base_clause_colon()
        {
            return self.current_line_indent_spaces() + self.options.indent_width;
        }
        for scan_index in self.output.scoped_range().rev() {
            let line = &self.output[scan_index];
            let code = &self.output.code_before_comment(scan_index);
            let trimmed = code.trim_ascii();
            if trimmed.is_empty() || trimmed.starts_with(['#', ':', ',']) {
                continue;
            }
            if code_opens_class_base_clause(code.trim_ascii_end()) {
                return leading_visual_width(line, self.options.tab_width)
                    + self.options.indent_width;
            }
            break;
        }
        (self.continuation_base_indent() + 1) * self.options.indent_width
    }

    pub(crate) fn next_continuation_indent(&self) -> ContinuationIndent {
        // A case label continued before its colon goes on at its body
        // and one level in, whatever the continuation indent.
        let current = self.current.trim_ascii_start();
        if (current.starts_with("case ") || current.starts_with("case("))
            && find_case_colon(current).is_none()
            && self.layout.nesting.paren_depth == 0
        {
            // The label itself stands a level out unless switches indent.
            let label_spaces = self.current_line_indent_spaces().saturating_sub(
                usize::from(!self.options.indent_switches) * self.options.indent_width,
            );
            return ContinuationIndent::Spaces(label_spaces + 2 * self.options.indent_width);
        }
        if self.options.max_code_length.is_some()
            && let Some(spaces) = lambda_parameter_continuation_indent(
                self.current.trim_ascii_end(),
                self.current_line_indent_spaces(),
                self.options.indent_width,
                self.options.max_continuation_indent,
                self.options.continuation_indent * self.options.indent_width,
                matches!(
                    self.options.brace_style,
                    BraceStyle::Allman
                        | BraceStyle::Whitesmith
                        | BraceStyle::Vtk
                        | BraceStyle::Gnu
                        | BraceStyle::Horstmann
                ),
            )
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if self.options.indent_after_parens
            && self.current_paren_is_lambda_parameter_list()
            && matches!(
                self.options.brace_style,
                BraceStyle::Attach | BraceStyle::OneTrueBrace | BraceStyle::Ratliff
            )
        {
            return ContinuationIndent::Spaces(self.current_line_indent_spaces());
        }
        if self.options.indent_after_parens
            && self.current.trim_ascii_end().ends_with('(')
            && contains_word(&self.current, "new")
            && unmatched_open_paren_columns(self.current.trim_ascii_end()).len() >= 2
            && let Some(spaces) = self.assignment_continuation_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces + self.options.indent_width);
        }
        if self.options.indent_after_parens
            && self.current.trim_ascii_end().ends_with(',')
            && contains_word(&self.current, "new")
            && let Some(previous) = self.output.last_line_outside_comment()
            && let Some((assignment, operator)) = find_assignment_operator(previous)
        {
            let after_operator = assignment + operator.len();
            let value_start = previous[after_operator..]
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(previous.len(), |(offset, _)| after_operator + offset);
            let value_column =
                visual_width_from(&previous[..value_start], 0, self.options.tab_width);
            return ContinuationIndent::Spaces(value_column + self.options.indent_width);
        }
        if !self.options.indent_after_parens
            && self.layout.frame_stack.active_ternary().is_some()
            && self.current_line_indent_spaces()
                > self.continuation_base_indent() * self.options.indent_width
            && head_ends_binary_operator(self.current.trim_ascii_end())
        {
            return ContinuationIndent::Spaces(self.current_line_indent_spaces());
        }
        if self.options.indent_after_parens
            && self.for_header_continuation_indent_spaces().is_some()
        {
            return ContinuationIndent::Spaces(
                self.current_line_indent_spaces()
                    + self.options.continuation_indent * self.options.indent_width,
            );
        }
        if self.options.indent_after_parens
            && self.current_is_conditional_header_continuation()
            && self.current_line_indent_spaces()
                > self.continuation_base_indent() * self.options.indent_width
        {
            return ContinuationIndent::Spaces(self.current_line_indent_spaces());
        }
        if self.options.max_code_length.is_some()
            && self.options.indent_after_parens
            && self.current_is_conditional_header_continuation()
        {
            return ContinuationIndent::Spaces(
                (self.continuation_base_indent() + self.options.continuation_indent)
                    * self.options.indent_width,
            );
        }
        if self.layout.in_class_base_clause {
            return ContinuationIndent::Spaces(self.class_base_clause_indent_spaces());
        }
        if self.current_ends_base_clause_colon() {
            return ContinuationIndent::Level(self.continuation_base_indent() + 1);
        }
        if !self.options.indent_after_parens
            && self.current_is_operator_led_continuation()
            && unmatched_open_paren_column(self.current.trim_ascii_end()).is_none()
        {
            return ContinuationIndent::Spaces(self.current_line_indent_spaces());
        }
        if let Some(spaces) = self.declaration_continuation_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.split_aggregate_declaration_name_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.chained_ternary_continuation_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.asm_colon_continuation_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.array_bound_operator_continuation_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens
            && matches!(self.layout.previous, PreviousToken::Comma)
            && let Some(spaces) = self.macro_call_argument_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens
            && self.current_ends_logical_operator()
            && unmatched_open_paren_column(self.current.trim_ascii_end()).is_none()
        {
            let line = self.current.trim_ascii_end();
            if !line.ends_with("||")
                && self.current_is_conditional_header_continuation()
                && let Some(spaces) = self.layout.nesting.current_continuation_indent_spaces()
            {
                return ContinuationIndent::Spaces(spaces);
            }
            let normal = self.continuation_base_indent() * self.options.indent_width
                + self.options.continuation_indent * self.options.indent_width;
            if let Some(spaces) = self.logical_chain_head_indent_spaces() {
                if line.ends_with("||")
                    && line.trim_ascii_start().starts_with(['!', '('])
                    && self.current_return_logical_tail_indent_spaces().is_some()
                    && line[..line.len().saturating_sub(2)]
                        .trim_ascii_end()
                        .ends_with(')')
                {
                    return ContinuationIndent::Spaces(spaces.saturating_sub(1));
                }
                if self.current_is_conditional_header_continuation()
                    && line.contains(')')
                    && spaces <= normal + 1
                {
                    return ContinuationIndent::Spaces(normal);
                }
                if !line.ends_with("||")
                    && !self.current_is_conditional_header_continuation()
                    && let Some(paren) = self.layout.nesting.current_continuation_indent_spaces()
                    && paren > spaces
                {
                    return ContinuationIndent::Spaces(paren);
                }
                return ContinuationIndent::Spaces(spaces);
            }
        }
        if !self.options.indent_after_parens
            && self.current_ends_logical_operator()
            && let Some(column) = unmatched_open_paren_column(self.current.trim_ascii_end())
            && self.current.trim_ascii_end()[column + 1..].starts_with('(')
            && let Some(spaces) = self.logical_continuation_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens {
            let line = self.current.trim_ascii_end();
            if line.ends_with(':')
                && line.contains('?')
                && self.open_paren_column_of(line).is_none()
                && !self.current_is_conditional_header_continuation()
            {
                let spaces = self
                    .assignment_continuation_indent_spaces()
                    .or_else(|| self.return_continuation_indent_spaces())
                    .unwrap_or_else(|| self.current_line_indent_spaces());
                return ContinuationIndent::Spaces(spaces);
            }
        }
        if !self.options.indent_after_parens
            && self.current_ends_logical_operator()
            && let Some(spaces) = self.for_header_continuation_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.run_in_enum_assignment_continuation_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if let Some(spaces) = self.parameter_default_operator_continuation_indent_spaces() {
            return ContinuationIndent::Spaces(spaces);
        }
        if self.layout.nesting.current_paren_is_inline_brace_call()
            && let (Some(spaces), Some(paren_spaces)) = (
                self.layout.nesting.current_continuation_indent_spaces(),
                self.layout.nesting.current_paren_indent_spaces(),
            )
            && spaces > paren_spaces
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens {
            let line = self.current.trim_ascii_end();
            if line.trim_ascii_start().starts_with(": ")
                && head_ends_binary_operator(line)
                && let Some(open) = self.open_paren_column_of(line)
            {
                return ContinuationIndent::Spaces(self.current_line_indent_spaces() + open + 1);
            }
        }

        if !self.options.indent_after_parens
            && self.layout.frame_stack.active_delimiter().is_some()
            && let Some(spaces) = self.layout.nesting.current_continuation_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens
            && !self.in_initializer_brace()
            && !self.in_aggregate_declaration_brace()
            && head_ends_binary_operator(self.current.trim_ascii_end())
        {
            if let Some(spaces) = self.logical_chain_head_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces {
                return ContinuationIndent::Spaces(spaces);
            }
        }
        if !self.options.indent_after_parens {
            if let Some(spaces) = self.return_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.stream_operator_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
        }

        // A subscript left open aligns the next line past its bracket, as
        // astyle counts brackets as parentheses.
        if !self.options.indent_after_parens
            && let line = self.current.trim_ascii_end()
            && let Some(open) = self.open_paren_column_of(line)
            && line[open..].starts_with('[')
            // An Objective-C message aligns its own way.
            && self.layout.frame_stack.bracket_depth() > 0
            && !self.layout.objc.message_active
            && line[..open]
                .chars()
                .next_back()
                .is_some_and(|ch| ch == ']' || ch == '_' || ch.is_alphanumeric())
        {
            let indent = self.current_line_indent_spaces();
            let lead = self.current.len() - self.current.trim_ascii_start().len();
            return ContinuationIndent::Spaces(
                indent
                    + visual_width_from(
                        &self.current[lead..open + 1],
                        indent,
                        self.options.tab_width,
                    ),
            );
        }
        if (!self.in_initializer_brace() || self.innermost_brace_is_compound_literal())
            && !self.innermost_init_block_brace()
            && !self.in_aggregate_declaration_brace()
            && let Some(spaces) = self.layout.nesting.current_continuation_indent_spaces()
        {
            return ContinuationIndent::Spaces(spaces);
        }
        if !self.options.indent_after_parens {
            let line = self.current.trim_ascii_end();
            if line.ends_with(':')
                && line.contains('?')
                && line.matches('(').count() >= 2
                && !self.current_is_conditional_header_continuation()
            {
                return ContinuationIndent::Spaces(
                    self.current_line_indent_spaces() + self.options.indent_width * 2,
                );
            }
            if let Some(spaces) = self.logical_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            // An enumerator's pending column, set by a comment before it, is
            // the line's own: its value continues past its `=`.
            if let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces
                && !self.in_enum_declaration_brace()
            {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.operator_led_return_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.aligned_after_paren_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.assignment_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.return_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.return_operator_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.stream_operator_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
            if let Some(spaces) = self.declaration_continuation_indent_spaces() {
                return ContinuationIndent::Spaces(spaces);
            }
        }

        let base_indent = self.continuation_base_indent();
        let line = self.current.trim_ascii_end();
        if line.ends_with(':')
            && line.contains('?')
            && line.matches('(').count() >= 2
            && !self.current_is_conditional_header_continuation()
        {
            return ContinuationIndent::Spaces(
                self.current_line_indent_spaces() + self.options.indent_width * 2,
            );
        }
        if head_ends_binary_operator(line)
            && let Some(column) = self.current_inline_array_column()
        {
            return ContinuationIndent::Spaces(column);
        }
        if (self.in_initializer_brace() && !self.innermost_brace_is_compound_literal())
            || self.innermost_init_block_brace()
        {
            return ContinuationIndent::Level(base_indent);
        }
        let max_level = self.options.max_continuation_indent / self.options.indent_width.max(1);
        let indent =
            (base_indent + self.options.continuation_indent).min(base_indent + max_level.max(1));
        let base_spaces = base_indent * self.options.indent_width;
        let spaces =
            self.apply_min_conditional_indent(base_spaces, indent * self.options.indent_width);
        if line.contains('?')
            && self
                .output
                .last()
                .is_some_and(|line| line.trim_ascii() == "{")
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim_ascii().is_empty())
                .is_some_and(|line| {
                    line.trim_ascii_end().ends_with(')')
                        && leading_visual_width(line, self.options.tab_width) > 0
                })
        {
            return ContinuationIndent::Spaces(spaces + 1);
        }
        ContinuationIndent::Spaces(spaces)
    }

    pub(crate) fn macro_call_argument_indent_spaces(&self) -> Option<usize> {
        let trimmed = self.current.trim_ascii_start();
        let open = trimmed.find('(')?;
        let name = trimmed[..open].trim_ascii_end();
        let unmatched = unmatched_open_paren_columns(trimmed);
        if !is_macro_like_word(name) || !unmatched.contains(&open) {
            return None;
        }
        if unmatched.last() != Some(&open) {
            return None;
        }
        let current_prefix_len = self.current.len() - self.current.trim_ascii_start().len();
        let base_spaces = if current_prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            current_prefix_len
        }
        .max(self.continuation_base_indent() * self.options.indent_width)
        .max(
            ContinuationIndent::Level(
                self.layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal),
            )
            .columns(self.options.indent_width),
        );
        let padding = trimmed
            .chars()
            .skip(open + 1)
            .take_while(|ch| ch.is_whitespace())
            .collect::<String>();
        let padding_width =
            visual_width_from(&padding, base_spaces + open + 1, self.options.tab_width);
        Some(base_spaces + open + 1 + padding_width)
    }

    pub(crate) fn trailing_open_bracket_indent_spaces(&self) -> Option<usize> {
        let current = self.current.trim_ascii_end();
        if !current.ends_with('[') {
            return None;
        }
        let column = current.rfind('[')?;
        Some(self.continuation_base_indent() * self.options.indent_width + column + 3)
    }

    fn aligned_after_paren_indent_spaces(&self) -> Option<usize> {
        let base_spaces = self
            .layout
            .continuation_indent
            .next_line_indent_spaces
            .unwrap_or_else(|| self.continuation_base_indent() * self.options.indent_width);
        if let Some(spaces) = self.trailing_open_paren_continuation_indent_spaces(base_spaces) {
            return Some(spaces);
        }
        let column = unmatched_open_paren_column(&self.current)?;
        let head_width =
            visual_width_from(&self.current[..column], base_spaces, self.options.tab_width);
        let padding = self
            .current
            .chars()
            .skip(column + 1)
            .take_while(|ch| ch.is_whitespace())
            .collect::<String>();
        let padding_width = visual_width_from(
            &padding,
            base_spaces + head_width + 1,
            self.options.tab_width,
        );
        let spaces = self.apply_min_conditional_indent(
            base_spaces,
            base_spaces
                + head_width
                + 1
                + padding_width
                + self
                    .attached_return_type_indent_delta(base_spaces)
                    .unwrap_or(0),
        );
        let max_spaces = self.options.max_continuation_indent;
        if spaces.saturating_sub(base_spaces) > max_spaces {
            return Some(base_spaces + self.options.indent_width * 2);
        }
        Some(spaces)
    }

    fn attached_return_type_indent_delta(&self, current_indent_spaces: usize) -> Option<usize> {
        if !self.options.attach_return_type && !self.options.attach_return_type_decl {
            return None;
        }
        let open = self.current.rfind('(')?;
        let before = self.current[..open].trim_ascii();
        if before.is_empty()
            || before.contains('=')
            || is_header(self.options, before)
            || !matches!(function_name_start(before), Some(0))
        {
            return None;
        }
        let previous = self.output.last()?;
        let previous_trimmed = previous.trim_ascii();
        if previous_trimmed.ends_with(':')
            || !is_return_type_line(previous_trimmed)
            || self
                .current
                .tokens()
                .is_some_and(|span| !self.attaches_return_type(span.first))
        {
            return None;
        }
        let previous_prefix_len = previous.len() - previous.trim_ascii_start().len();
        let current_prefix_len = self.current.len() - self.current.trim_ascii_start().len();
        let current_indent_spaces = current_indent_spaces.max(current_prefix_len);
        let separator_len = usize::from(!previous_trimmed.ends_with(['*', '&', '^']));
        Some(
            (previous_prefix_len + previous_trimmed.len() + separator_len)
                .saturating_sub(current_indent_spaces),
        )
    }

    fn trailing_open_paren_continuation_indent_spaces(&self, base_spaces: usize) -> Option<usize> {
        let columns = unmatched_open_paren_columns(&self.current);
        let trailing_column = *columns.last()?;
        if self.current.trim_ascii_end().chars().count() != trailing_column + 1 {
            return None;
        }
        let previous_indent = if columns.len() >= 2 {
            let column = columns[columns.len() - 2];
            self.apply_min_conditional_indent(base_spaces, base_spaces + column + 1)
        } else if let Some(spaces) = self.assignment_continuation_indent_spaces() {
            return Some(spaces + self.options.continuation_indent * self.options.indent_width);
        } else if let Some(spaces) = self.return_continuation_indent_spaces() {
            return Some(spaces + self.options.continuation_indent * self.options.indent_width);
        } else {
            base_spaces
        };
        let spaces = previous_indent + self.options.continuation_indent * self.options.indent_width;
        if spaces > self.options.max_continuation_indent {
            Some(base_spaces + self.options.indent_width * 2)
        } else {
            Some(spaces)
        }
    }

    pub(crate) fn logical_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        if !self.current_ends_logical_operator() {
            return None;
        }
        let base_spaces = self.continuation_base_indent() * self.options.indent_width;
        let line_base_spaces = self
            .layout
            .continuation_indent
            .next_line_indent_spaces
            .unwrap_or(base_spaces);
        if let Some(column) = self.open_paren_column_of(line) {
            let paren_offset = if line.ends_with("||")
                && line[..line.len().saturating_sub(2)]
                    .trim_ascii_end()
                    .ends_with(')')
                && !line[column + 1..].starts_with('(')
            {
                0
            } else {
                1
            };
            let spaces = self.apply_min_conditional_indent(
                line_base_spaces,
                line_base_spaces + column + paren_offset,
            );
            if spaces <= self.options.max_continuation_indent {
                return Some(spaces);
            }
        }
        let normal = base_spaces + self.options.continuation_indent * self.options.indent_width;
        if let Some(spaces) = self.logical_chain_head_indent_spaces() {
            if line.ends_with("||")
                && line[..line.len().saturating_sub(2)]
                    .trim_ascii_end()
                    .ends_with(')')
            {
                return Some(spaces.saturating_sub(1));
            }
            if self.current_is_conditional_header_continuation()
                && line.contains(')')
                && spaces <= normal + 1
            {
                return Some(normal);
            }
            return Some(spaces);
        }
        Some(normal)
    }

    fn current_ends_logical_operator(&self) -> bool {
        ends_logical_operator(self.current.trim_ascii_end())
    }

    fn logical_chain_head_indent_spaces(&self) -> Option<usize> {
        self.current_assignment_logical_tail_indent_spaces()
            .or_else(|| self.assignment_continuation_indent_spaces())
            .or_else(|| self.return_continuation_indent_spaces())
            .or_else(|| self.current_return_logical_tail_indent_spaces())
            .or_else(|| self.previous_return_continuation_indent_spaces())
            .or(self.layout.continuation_indent.logical_chain_indent_spaces)
    }

    fn current_assignment_logical_tail_indent_spaces(&self) -> Option<usize> {
        let code = self.current.trim_ascii_start();
        if (code.ends_with("&&") || code.ends_with("||")) && code.starts_with('=') {
            return Some(
                self.current_line_indent_spaces() + code.len() - code[1..].trim_ascii_start().len(),
            );
        }
        None
    }

    fn current_return_logical_tail_indent_spaces(&self) -> Option<usize> {
        let code = self.current.trim_ascii_start();
        if (code.ends_with("&&") || code.ends_with("||"))
            && !code.starts_with("return ")
            && !head_starts_binary_operator(code)
            && self.previous_return_continuation_indent_spaces().is_some()
        {
            let case_unindent =
                self.layout.line_adjuster.pending_case_unindent() * self.options.indent_width;
            return Some(
                self.current_line_indent_spaces()
                    .saturating_sub(case_unindent),
            );
        }
        None
    }

    fn previous_return_continuation_indent_spaces(&self) -> Option<usize> {
        for line in self.output.scoped().iter().rev().take(8) {
            let trimmed = line.trim_ascii_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let code = self.output.code_of(trimmed).trim_ascii_end();
            if code.starts_with("return ") && !code.ends_with(';') {
                return Some(
                    leading_visual_width(line, self.options.tab_width)
                        + return_value_column_offset(code),
                );
            }
            if !code.is_empty()
                && (head_starts_binary_operator(code)
                    || code.ends_with("&&")
                    || code.ends_with("||"))
            {
                continue;
            }
            break;
        }
        None
    }

    fn operator_led_return_continuation_indent_spaces(&self) -> Option<usize> {
        let trimmed = self.current.trim_ascii_end();
        let head = trimmed.trim_ascii_start();
        if !head_starts_binary_operator(head) {
            return None;
        }
        if self.layout.nesting.paren_depth > 0 || self.open_paren_column_of(trimmed).is_some() {
            return None;
        }
        self.previous_return_continuation_indent_spaces()
    }

    pub(crate) fn previous_logical_continuation_indent_spaces(
        &self,
        operator: &str,
    ) -> Option<usize> {
        let previous_line = self.output.len().checked_sub(1)?;
        let frame = self
            .layout
            .frame_stack
            .active_logical_on_output_line(previous_line)?;
        let matches_operator = matches!(
            (operator, frame.operator),
            ("&&", LogicalOperator::And) | ("||", LogicalOperator::Or)
        );
        (matches_operator && frame.operator_starts_output_line).then_some(frame.line_indent_spaces)
    }

    fn declaration_continuation_indent_spaces(&self) -> Option<usize> {
        if self.layout.indentation.statement_depth() != 0
            || self.layout.in_class_base_clause
            || self.in_enum_declaration_brace()
            || self.in_initializer_brace()
        {
            return None;
        }
        let line = self.current.trim_ascii_end();
        if !line.ends_with(',') || line.contains('=') {
            return None;
        }
        if let Some(paren) = line.find('(') {
            let before_paren = line[..paren].trim_ascii_end();
            if !before_paren.contains(char::is_whitespace) {
                // A call or a cast leading a statement declares nothing:
                // astyle continues nothing past its comma.
                let leads_statement = before_paren
                    .trim_ascii_start()
                    .chars()
                    .all(is_identifier_continue)
                    && !is_header(self.options, before_paren.trim_ascii_start())
                    && self.open_paren_column_of(line).is_none()
                    && matches!(
                        self.layout.nesting.brace_type_stack.last(),
                        Some(BraceType::Command | BraceType::Definition)
                    );
                return leads_statement.then(|| self.current_line_indent_spaces());
            }
        }
        let current_prefix_len = line.len() - line.trim_ascii_start().len();
        let prefix_len = if current_prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            current_prefix_len
        };
        if let Some(previous) = self
            .output
            .scoped()
            .iter()
            .rev()
            .find(|line| !line.trim_ascii().is_empty())
        {
            let previous_code = self.output.code_of(previous).trim_ascii_end();
            if previous_code.ends_with(',')
                && !previous_code.contains('=')
                && previous_code.find('(').is_none_or(|paren| {
                    previous_code[..paren]
                        .trim_ascii_end()
                        .contains(char::is_whitespace)
                })
            {
                let previous_prefix = leading_visual_width(previous, self.options.tab_width);
                if previous_prefix
                    + declaration_comma_continuation_column(previous_code.trim_ascii_start())
                    == prefix_len
                {
                    return Some(prefix_len);
                }
            }
        }
        Some(prefix_len + declaration_comma_continuation_column(line.trim_ascii_start()))
    }

    fn asm_colon_continuation_indent_spaces(&self) -> Option<usize> {
        if !self.current.trim_ascii_start().starts_with(':') {
            return None;
        }
        let saw_asm = self.current.contains("asm")
            || self
                .output
                .scoped()
                .iter()
                .rev()
                .take(8)
                .take_while(|line| !line.trim_ascii_end().ends_with(';'))
                .any(|line| line.contains("asm"));
        saw_asm.then_some(self.current_line_indent_spaces())
    }

    fn chained_ternary_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        let current_indent = self.current_line_indent_spaces();
        let base_indent = self.continuation_base_indent() * self.options.indent_width;
        (line.ends_with(':') && line.contains('?') && current_indent > base_indent)
            .then_some(current_indent)
    }

    fn split_aggregate_declaration_name_indent_spaces(&self) -> Option<usize> {
        if !self.newline_breaks_statement
            || self.next_line.word_followed_by_open_paren
            || self.layout.indentation.statement_depth() != 0
        {
            return None;
        }
        let line = self.current.trim_ascii_end();
        if line.trim_ascii_start().starts_with(['/', '*'])
            || ["//", "/*", "*/"]
                .iter()
                .any(|marker| line.contains(marker))
            || line.ends_with([';', '{', '}', '='])
            || line.contains_any_byte(b"([)],")
            || self.next_line.leads_with_class_base
        {
            return None;
        }
        let words = line
            .split(|ch: char| !is_identifier_continue(ch))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        let aggregate_index = words
            .iter()
            .position(|word| matches!(*word, "struct" | "union"))?;
        (words.len() > aggregate_index + 1)
            .then_some(self.current_line_indent_spaces() + self.options.indent_width)
    }

    fn run_in_enum_assignment_continuation_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        let trimmed = line.trim_ascii_start();
        let after_brace = trimmed.find('{')? + 1;
        if !trimmed[..after_brace]
            .trim_ascii_start()
            .starts_with("enum")
        {
            return None;
        }
        let after_brace_space = trimmed[after_brace..]
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .count();
        Some(
            self.current_line_indent_spaces()
                + after_brace
                + after_brace_space
                + self.options.indent_width,
        )
    }

    pub(crate) fn enum_member_missing_comma_indent_spaces(&self, previous: &str) -> Option<usize> {
        let previous_content = previous.trim_ascii_start();
        if previous_content.starts_with("/*") || previous_content.starts_with("//") {
            return None;
        }
        let mut comment_limit = previous_content
            .find("//")
            .unwrap_or(previous_content.len());
        if let Some(block_comment) = previous_content.find("/*")
            && previous_content[..block_comment]
                .trim_ascii_end()
                .ends_with(',')
        {
            comment_limit = comment_limit.min(block_comment);
        }
        let previous_code = previous_content[..comment_limit].trim_ascii_end();
        let assignment = previous_code.rfind('=')?;
        if previous_code.contains("==")
            || previous_code.contains("!=")
            || previous_code.contains("<=")
            || previous_code.contains(">=")
            || previous_code.ends_with(',')
            || previous_code.ends_with(';')
            || previous_code.ends_with('{')
            || previous_code.ends_with('}')
        {
            return None;
        }
        let in_enum = self
            .output
            .scoped()
            .iter()
            .rev()
            .take_while(|line| {
                let line = line.trim_ascii_start();
                !line.starts_with('}') && !line.ends_with(';')
            })
            .any(|line| {
                let line = line.trim_ascii_start();
                line == "enum" || line.starts_with("enum ")
            });
        if !in_enum {
            return None;
        }
        let after_assignment = &previous_code[assignment + 1..];
        let value_start = assignment
            + 1
            + after_assignment
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(after_assignment.len(), |(offset, _)| offset);
        let leading = leading_visual_width(previous, self.options.tab_width);
        Some(
            leading
                + visual_width_from(
                    &previous_code[..value_start],
                    leading,
                    self.options.tab_width,
                ),
        )
    }

    pub(crate) fn array_bound_operator_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        if !head_ends_binary_operator(line) {
            return None;
        }
        Some(self.current_line_indent_spaces() + array_bound_operator_column(line)?)
    }

    fn parameter_default_operator_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        if self.layout.nesting.paren_depth == 0 || !head_ends_binary_operator(line) {
            return None;
        }
        let content = line.trim_ascii_start();
        let code = self.output.code_of(content).trim_ascii_end();
        let open = self.open_paren_column_of(code)?;
        let assignment = find_single_assignment_after(code, open + 1)?;
        let after_assignment = &code[assignment + 1..];
        let value_offset = assignment
            + 1
            + after_assignment
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(0, |(index, _)| index);
        let current_prefix_len = line.len() - content.len();
        let base = if current_prefix_len == 0 {
            self.current_line_indent_spaces()
        } else {
            current_prefix_len
        };
        Some(base + value_offset)
    }

    pub(crate) fn assignment_continuation_indent_spaces(&self) -> Option<usize> {
        if self.in_initializer_brace() && !self.innermost_brace_is_compound_literal() {
            return None;
        }
        let line = self.current.trim_ascii_end();
        let code = self.output.code_of(line).trim_ascii_end();
        if code.ends_with(',')
            || code.ends_with(';')
            || code.ends_with(':') && find_case_colon(code).is_some()
        {
            return None;
        }
        let (operator_start, operator) = find_assignment_operator(code)?;
        let after_operator = operator_start + operator.len();
        let rest = &code[after_operator..];
        let value = rest.trim_ascii_start();
        if value.is_empty() || value.starts_with(':') && !value.starts_with("::") {
            return None;
        }
        let value_offset = after_operator
            + rest
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(1, |(index, _)| index);
        let base = self.continuation_base_indent() * self.options.indent_width;
        Some(base + visual_width_from(&code[..value_offset], base, self.options.tab_width))
    }

    fn assignment_rhs_continuation_column(&self) -> Option<usize> {
        if self.in_initializer_brace() && !self.innermost_brace_is_compound_literal() {
            return None;
        }
        let line = self.current.trim_ascii_end();
        let code = self.output.code_of(line).trim_ascii_end();
        if code.ends_with(':') && find_case_colon(code).is_some() {
            return None;
        }
        let mut search_start = 0;
        let mut last_operator = None;
        while let Some((relative_start, operator)) = find_assignment_operator(&code[search_start..])
        {
            let operator_start = search_start + relative_start;
            last_operator = Some((operator_start, operator));
            search_start = operator_start + operator.len();
        }
        let (operator_start, operator) = last_operator?;
        let after_operator = operator_start + operator.len();
        let rest = &code[after_operator..];
        let value = rest.trim_ascii_start();
        if value.is_empty() || value.starts_with(':') && !value.starts_with("::") {
            return None;
        }
        let value_offset = after_operator
            + rest
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(1, |(index, _)| index);
        let base = self.continuation_base_indent() * self.options.indent_width;
        Some(base + visual_width_from(&code[..value_offset], base, self.options.tab_width))
    }

    fn return_operator_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        (head_ends_binary_operator(line)).then(|| self.return_continuation_indent_spaces())?
    }

    pub(crate) fn return_continuation_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        let trimmed = line.trim_ascii_start();
        let after_return = trimmed.strip_prefix("return")?;
        if after_return
            .chars()
            .next()
            .is_some_and(is_identifier_continue)
            || trimmed.ends_with(';')
        {
            return None;
        }
        let base = self.continuation_base_indent() * self.options.indent_width;
        let gap = after_return.len() - after_return.trim_ascii_start().len();
        // A tab between `return` and its value still sets the value's column.
        if !after_return.trim_ascii_start().is_empty()
            && !after_return.trim_ascii_start().contains('\t')
            && after_return[..gap].contains('\t')
        {
            let start = base + (line.len() - trimmed.len());
            return Some(
                start
                    + visual_width_from(
                        &trimmed[.."return".len() + gap],
                        start,
                        self.options.tab_width,
                    ),
            );
        }
        if after_return.is_empty() || after_return.contains('\t') {
            return Some(base + self.options.continuation_indent * self.options.indent_width);
        }
        Some(base + (line.len() - trimmed.len()) + return_value_column_offset(trimmed))
    }

    fn is_stream_continuation_break(&self) -> bool {
        self.stream_operator_indent_spaces().is_some()
    }

    fn stream_operator_indent_spaces(&self) -> Option<usize> {
        let line = self.current.trim_ascii_end();
        if line.ends_with("*/") && !line.contains("/*") {
            return None;
        }
        let code = &self.output.code_of(line);
        let trimmed_code = code.trim_ascii_start();
        if (trimmed_code.starts_with("<<") || trimmed_code.starts_with(">>"))
            && code.trim_ascii_end().ends_with('{')
        {
            return None;
        }
        let first_word = code
            .split(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty())?;
        if !language::STREAM_NAMES.contains(&first_word)
            && (self.layout.nesting.paren_depth > 0
                || self.in_initializer_brace()
                || is_header(self.options, first_word))
        {
            return None;
        }
        let stream = self.layout.frame_stack.active_stream()?;
        (stream.operator_output_line == self.output.len()).then_some(stream.chain_anchor_column)
    }

    pub(crate) fn previous_stream_chain_indent_spaces(&self) -> Option<usize> {
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .layout
            .frame_stack
            .first_stream_on_output_line(previous_line)
            .or_else(|| {
                self.layout
                    .frame_stack
                    .stream_before_output_line(self.output.len())
            })?;
        let base = self.continuation_base_indent() * self.options.indent_width;
        if stream.line_contains_nested_brace {
            return Some(base + self.options.indent_width * 2);
        }
        if stream.line_indent_spaces > base
            && stream.operator_output_column != stream.line_indent_spaces
        {
            return Some(stream.line_indent_spaces);
        }
        if let Some(spaces) = stream.assignment_value_start_column {
            return Some(spaces);
        }
        if stream.operator_output_column.saturating_sub(base) > self.options.max_continuation_indent
        {
            return Some(base + self.options.indent_width * 2);
        }
        Some(stream.operator_output_column)
    }

    pub(crate) fn continuation_base_indent(&self) -> usize {
        let braceless_extra = self.layout.pending_braceless_block_bias.map_or(0, |level| {
            level.saturating_sub(self.layout.indentation.indent())
        });
        self.layout.indentation.indent()
            + braceless_extra
            + self.case_body_indent_extra(LineKind::Normal)
    }

    pub(crate) fn apply_min_conditional_indent(&self, base_spaces: usize, spaces: usize) -> usize {
        if self.options.indent_after_parens && self.current_is_conditional_header_continuation() {
            return spaces;
        }
        if self.is_min_conditional_continuation() {
            let floor_base =
                base_spaces.min(self.continuation_base_indent() * self.options.indent_width);
            spaces.max(floor_base + min_conditional_indent_spaces(self.options))
        } else {
            spaces
        }
    }

    fn is_min_conditional_continuation(&self) -> bool {
        if self.current_is_conditional_header_continuation() {
            return true;
        }
        let line = self.current.trim_ascii_end();
        line.ends_with('?') || line.ends_with(" :")
    }

    fn current_is_operator_led_continuation(&self) -> bool {
        if self.layout.indentation.statement_depth() != 0
            || self.layout.command_state.current_header.is_some()
        {
            return false;
        }
        let trimmed = self.current.trim_ascii_start();
        let trimmed_end = trimmed.trim_ascii_end();
        if trimmed_end.is_empty() || trimmed_end.ends_with(';') {
            return false;
        }
        if self.current_line_indent_spaces()
            <= self.continuation_base_indent() * self.options.indent_width
        {
            return false;
        }
        starts_with_chain_operator(trimmed)
    }

    pub(crate) fn current_is_conditional_header_continuation(&self) -> bool {
        self.layout.indentation.statement_depth() > 0
            && self
                .layout
                .command_state
                .current_header
                .as_deref()
                .is_some_and(|header| matches!(header, "if" | "for" | "while" | "switch"))
    }

    pub(crate) fn set_next_continuation_indent(&mut self, indent: ContinuationIndent) {
        self.layout
            .continuation_indent
            .next_input_line_continuation_indent = Some(indent);
        let level = match indent {
            ContinuationIndent::Level(level) => {
                self.layout.continuation_indent.set_next_line_level(level);
                level
            }
            ContinuationIndent::Spaces(spaces) => {
                self.layout.continuation_indent.set_next_line_spaces(spaces);
                spaces / self.options.indent_width.max(1)
            }
        };
        self.layout.indentation.register_continuation_indent(level);
        self.layout.run_in_state.current_run_in_indent = Some(level);
    }
}

fn is_nested_template_type(type_prefix: &str) -> bool {
    if !type_prefix.trim_ascii_end().ends_with('>') {
        return false;
    }
    let mut depth = 0usize;
    let mut max_depth = 0usize;
    for ch in type_prefix.chars() {
        match ch {
            '<' => {
                depth += 1;
                max_depth = max_depth.max(depth);
            }
            '>' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ => {}
        }
    }
    depth == 0 && max_depth >= 2
}

fn find_single_assignment_after(line: &str, start: usize) -> Option<usize> {
    let mut result = None;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut depth = 0i32;
    let mut chars = line.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some((_, '/'))) {
            break;
        }
        if index >= start && matches!(ch, '(' | '[') {
            depth += 1;
            continue;
        }
        if index >= start && matches!(ch, ')' | ']') {
            depth -= 1;
            continue;
        }
        if index < start || depth != 0 || ch != '=' {
            continue;
        }
        let previous = line[..index].chars().next_back();
        let next = line[index + 1..].chars().next();
        if matches!(previous, Some('=' | '!' | '<' | '>')) || next == Some('=') {
            continue;
        }
        result = Some(index);
    }
    result
}

fn contains_word(line: &str, expected: &str) -> bool {
    line.split(|ch: char| !is_identifier_continue(ch))
        .any(|word| word == expected)
}

fn ends_logical_operator(line: &str) -> bool {
    let line = line.trim_ascii_end();
    if line.ends_with("||") || line.ends_with("&&") {
        return true;
    }
    ["and", "or"].into_iter().any(|operator| {
        line.strip_suffix(operator).is_some_and(|head| {
            head.chars()
                .next_back()
                .is_none_or(|ch| !is_identifier_continue(ch))
        })
    })
}

fn return_value_column_offset(line: &str) -> usize {
    let after_return = &line["return".len()..];
    "return".len()
        + after_return
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map_or(after_return.len(), |(index, _)| index)
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ContinuationIndent {
    Level(usize),
    Spaces(usize),
}

impl ContinuationIndent {
    pub(crate) fn columns(self, indent_width: usize) -> usize {
        match self {
            Self::Level(level) => level * indent_width,
            Self::Spaces(columns) => columns,
        }
    }
}

pub(crate) fn split_declaration_assignment_indent_spaces(
    options: &FormatOptions,
    current: &str,
    previous: &str,
) -> Option<usize> {
    if current
        .trim_ascii_start()
        .starts_with(['#', '(', ')', '{', '}', '.', '?', ':'])
    {
        return None;
    }
    let previous_code = previous[..trailing_comment_split_limit(previous)].trim_ascii_end();
    let (assignment, operator) = find_assignment_operator(previous_code)?;
    if operator != "="
        || !previous_code[assignment + operator.len()..]
            .trim_ascii()
            .is_empty()
    {
        return None;
    }
    let content = previous_code.trim_ascii_start();
    let declarator_offset = assignment_declarator_offset(content)?;
    if !is_nested_template_type(&content[..declarator_offset]) {
        return None;
    }
    let declarator_start = previous_code.len() - content.len() + declarator_offset;
    let base = leading_visual_width(previous, options.tab_width) + options.indent_width;
    let mut spaces = visual_width_from(&previous_code[..declarator_start], 0, options.tab_width);
    if previous_code[..declarator_start].contains('<') && spaces > base {
        spaces += 1;
    }
    Some(spaces.max(base))
}

pub(crate) fn min_conditional_indent_spaces(options: &FormatOptions) -> usize {
    match options.min_conditional_indent {
        MinConditionalIndent::Zero => 0,
        MinConditionalIndent::One => options.indent_width,
        MinConditionalIndent::Two => options.indent_width * 2,
        MinConditionalIndent::OneHalf => options.indent_width / 2,
    }
}
