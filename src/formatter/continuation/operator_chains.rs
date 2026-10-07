use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::formatter::braces::compound_literals::line_ends_compound_literal_cast;
use crate::formatter::constructs::headers::{
    is_braceless_header_line, is_conditional_header_line, line_is_control_body_header,
    starts_header_word,
};
use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, token_text};
use crate::formatter::state::frame::{
    ColonRole, FrameStack, LogicalOperator, ParenRole, TernaryOwnerRole,
};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::text::columns::column_after;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{
    ContainsAnyByte, is_comment_line, last_unmatched_open_delimiter, trailing_comment_split_limit,
    unmatched_open_paren_column, unmatched_open_paren_columns,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::literals::{starts_string_literal_token, string_literal_token_end};
use crate::formatter::tokens::operators::{
    find_assignment_operator, head_ends_binary_operator, starts_ternary_arm,
    starts_with_chain_operator,
};
use crate::source::lex::is_identifier_start;

pub(crate) fn starts_operator_chain_continuation(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    starts_with_chain_operator(trimmed) || starts_ternary_arm(trimmed)
}

pub(crate) fn clear_operator_chain_frames(frame_stack: &mut FrameStack) {
    frame_stack.clear_stream_frames();
    frame_stack.clear_logical_frames();
}

pub(super) fn clear_logical_chain_indent(logical_chain_indent_spaces: &mut Option<usize>) {
    *logical_chain_indent_spaces = None;
}

pub(crate) fn clear_operator_chain_state(
    frame_stack: &mut FrameStack,
    logical_chain_indent_spaces: &mut Option<usize>,
) {
    clear_operator_chain_frames(frame_stack);
    clear_logical_chain_indent(logical_chain_indent_spaces);
}

pub(crate) fn clear_stream_frames_and_logical_indent(
    frame_stack: &mut FrameStack,
    logical_chain_indent_spaces: &mut Option<usize>,
) {
    frame_stack.clear_stream_frames();
    clear_logical_chain_indent(logical_chain_indent_spaces);
}

impl FormatEngine<'_> {
    pub(crate) fn ready_embedded_preprocessor_return_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !current.chars().next().is_some_and(is_identifier_start) || !self.output.may_have_hash()
        {
            return None;
        }
        let previous_index = (0..self.output.len())
            .rev()
            .find(|&index| !self.output.trimmed(index).is_empty())?;
        let previous_code = self.output.code(previous_index);
        if !self
            .output
            .code_trimmed(previous_index)
            .starts_with("return ")
            || !previous_code.contains('#')
            || previous_code.ends_with(';')
            // A row of a macro body stays as written.
            || previous_code.trimmed_end().ends_with('\\')
        {
            return None;
        }
        let spaces = self
            .output
            .lead_width(previous_index, self.options.tab_width)
            + "return ".len();
        (leading_visual_width(line, self.options.tab_width) < spaces).then_some(spaces)
    }

    pub(crate) fn replayed_header_operator_indent_spaces(
        &self,
        line: &LineView<'_>,
        delimiter_owner: Option<usize>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if self.options.max_code_length.is_none()
            || !(line_start.starts_with("&&") || line_start.starts_with("||"))
            || !self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    is_conditional_header_line(self.output.code_of(previous).trimmed_start())
                })
        {
            return None;
        }
        let owner = delimiter_owner?;
        (owner == self.token_input.input_source_indent).then_some(owner)
    }

    pub(crate) fn maximum_length_return_chain_indent_spaces(
        &self,
        line: &LineView<'_>,
        kind: LineKind,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if kind != LineKind::Normal
            || self.options.max_code_length.is_none()
            || self.options.indent_after_parens
            || !starts_with_chain_operator(current)
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        if !starts_header_word(previous_trimmed, "return")
            || last_unmatched_open_delimiter(previous_code).is_some_and(|(open, _)| open == '[')
        {
            return None;
        }
        let after_return = &previous_trimmed["return".len()..];
        let value_offset = "return".len()
            + after_return
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(after_return.len(), |(index, _)| index);
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + visual_width_from(&previous_trimmed[..value_offset], 0, self.options.tab_width),
        )
    }

    pub(crate) fn first_ordinary_ternary_arm_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        // Indenting after parens stacks nothing for a `?`.
        if line_kind != LineKind::Normal
            || self.options.indent_after_parens
            || !line
                .trimmed_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
            || !self.output.may_have_question()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        // A `?` in a literal asks nothing; one inside parentheses leaves
        // its arms to the parentheses' continuation, as does an arm whose
        // line continues parentheses opened after the `?`.
        let pending_group = self
            .output
            .pending_tokens()
            .map(|span| self.tree.groups.enclosing(span.first));
        let asks = self
            .output
            .last_non_empty_index()
            .and_then(|index| self.output.line_tokens(index))
            .is_none_or(|span| {
                (span.first..=span.last.min(self.tree.tokens.len() - 1)).any(|index| {
                    matches!(self.tree.tokens[index], Token::Symbol('?'))
                        && self.tree.groups.enclosing(index).is_none_or(|group| {
                            self.tree.groups.get(group).delimiter != Delimiter::Paren
                        })
                        && pending_group
                            .is_none_or(|group| group == self.tree.groups.enclosing(index))
                })
            });
        (asks
            && previous_code.contains('?')
            && !previous_code.trimmed_start().starts_with('#')
            && !previous_code.contains(':')
            && !previous_code.ends_with(';'))
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(crate) fn ordinary_ternary_colon_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        normal_indent: usize,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !current.starts_with(':') {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let mut spaces = previous.trimmed_start().starts_with('?').then(|| {
            leading_visual_width(previous, self.options.tab_width)
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        });
        if line_kind == LineKind::Normal
            && !current.starts_with("::")
            && self.layout.nesting.paren_depth == 0
            && previous_code.contains('?')
            && !previous_code.contains('<')
            && find_assignment_operator(previous_code).is_none()
            && self
                .assignment_rhs_first_line_indent(previous, true)
                .is_none()
            && !previous_code.trimmed_start().starts_with("return ")
            && self.open_paren_column_of(previous_code).is_none()
        {
            spaces = Some(normal_indent * self.options.indent_width);
        }
        spaces
    }

    pub(crate) fn completed_ternary_call_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"?:") {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with("),")
            || !previous_code.contains('?')
            || !previous_code.contains(':')
        {
            return None;
        }
        let open = self.open_paren_column_of(previous_code)?;
        let padding = previous_code
            .chars()
            .skip(open + 1)
            .take_while(|ch| ch.is_whitespace())
            .collect::<String>();
        Some(open + 1 + visual_width_from(&padding, open + 1, self.options.tab_width))
    }

    pub(crate) fn operand_after_question_row_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line
                .trimmed_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        (previous_trimmed != "?"
            && previous_trimmed.starts_with('?')
            && previous_trimmed.ends_with('?'))
        .then(|| {
            leading_visual_width(previous, self.options.tab_width)
                + visual_width_from(previous_trimmed, 0, self.options.tab_width)
                + self.options.indent_width
                + 1
        })
    }

    pub(crate) fn leading_operator_after_ternary_colon_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with_any(b"<>|&+-*/%=!?:,.~")
        {
            return None;
        }
        // After the `:`, a sign, `&`, `*`, `!`, or `~` opens the arm.
        if line.trimmed_start().starts_with_any(b"&*-+!~")
            && self.output.pending_tokens().is_some_and(|span| {
                self.tree
                    .previous_code_token(span.first)
                    .is_some_and(|previous| {
                        matches!(self.tree.tokens[previous], Token::Symbol(':'))
                    })
            })
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        (previous_code.contains('?') && previous_code.ends_with(':'))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn scoped_ternary_continuation_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed_start().starts_with_any(b"}#")
            || !line
                .trimmed_start()
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        (previous_code.contains('?')
            && previous_code.contains("::")
            && self.open_paren_column_of(previous_code).is_none())
        .then(|| self.layout.indentation.indent() * self.options.indent_width)
    }

    pub(crate) fn allman_operator_or_preprocessor_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        normal_indent: usize,
        header_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::Allman
            || current.starts_with_any(b"{}#")
            || current.starts_with("//")
            || current.starts_with("/*")
            // A compound literal's field continues no operator.
            || current.starts_with('.') && self.innermost_brace_is_compound_literal()
        {
            return None;
        }
        let previous_index = self.output.last_non_empty_index()?;
        let previous = &self.output[previous_index];
        let previous_code = self
            .output
            .code_before_comment(previous_index)
            .trimmed_end();
        let previous_trimmed = previous_code.trimmed_start();
        let current_starts_operator = current.starts_with_any(b"<>|&+-*/%=!?:,.");
        // The ` */` of a block comment does not start with an operator.
        let previous_starts_operator = self.output.comment_start_index(previous_index)
            == previous_index
            && previous_trimmed.starts_with_any(b"<>|&+-*/%=!?:,.");
        if current_starts_operator && previous_starts_operator {
            if self.preprocessor.split_else.extra_indent
                && (current.starts_with("&&") || current.starts_with("||"))
                && let Some(spaces) = header_indent_spaces
            {
                return Some(spaces);
            }
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            return Some(match find_assignment_operator(previous_trimmed) {
                Some((0, operator)) => {
                    let after = &previous_trimmed[operator.len()..];
                    let operand = operator.len() + (after.len() - after.trimmed_start().len());
                    previous_indent
                        + visual_width_from(&previous_trimmed[..operand], 0, self.options.tab_width)
                }
                _ => previous_indent,
            });
        }
        // A `#` in a literal is no directive.
        let hash_only_in_literals = self.output.line_tokens(previous_index).is_some_and(|span| {
            let tokens = &self.tree.tokens[span.first..=span.last.min(self.tree.tokens.len() - 1)];
            tokens.iter().any(|token| {
                matches!(token, Token::StringLiteral(text) | Token::CharLiteral(text) if text.contains('#'))
            }) && !tokens.iter().any(|token| match token {
                Token::Preprocessor(_) => true,
                Token::StringLiteral(_) | Token::CharLiteral(_) | Token::Comment(..) => false,
                other => token_text(other).contains('#'),
            })
        });
        if previous_code.contains('#')
            && !hash_only_in_literals
            && !previous_trimmed.starts_with_any(b"#/")
            && !previous_trimmed.starts_with("return ")
        {
            let mut spaces = normal_indent * self.options.indent_width;
            if self.layout.indentation.indent() > 1
                && previous_code.contains_from_first_byte("#else")
                && !current_starts_operator
            {
                spaces = spaces.saturating_sub(self.options.indent_width);
            } else if self.layout.indentation.indent() > 1
                && previous_starts_operator
                && current.chars().next().is_some_and(is_identifier_start)
            {
                spaces = self.options.indent_width;
            }
            return Some(spaces);
        }
        None
    }

    pub(crate) fn stream_after_closed_or_inline_row_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !(current.starts_with("<<") || current.starts_with(">>"))
            || self.output.code_trimmed_of(line).ends_with('{')
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_trimmed = previous.trimmed_end();
        if !previous_trimmed.ends_with('}')
            && !((previous_trimmed.contains(" << ") || previous_trimmed.contains(" >> "))
                && self.open_paren_column_of(previous_trimmed).is_none())
        {
            return None;
        }
        self.previous_stream_chain_indent_spaces()
    }

    pub(crate) fn logical_continuation_after_commented_noexcept_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !self.may_have_noexcept || line_start.starts_with("//") || line_start.starts_with('{') {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous.contains("//")
            || !previous_code.ends_with("&&")
            || !(0..self.output.len()).rev().take(8).any(|index| {
                let code = self.output.code(index);
                previous.contains("//") && code.contains("noexcept(") && code.ends_with('(')
            })
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn braceless_ternary_comma_sibling_indent_spaces(
        &self,
        previous_code: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !previous_code.ends_with(',') || !self.previous_statement_is_braceless_ternary() {
            return None;
        }
        let target = self.open_paren_column_of(previous_code)?
            + 1
            + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        (current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(crate) fn observe_operator_chain_line_context(
        &mut self,
        output_line_index: usize,
        code: &str,
    ) {
        let (paren_closes, unmatched_open_paren) = self.paren_closes_and_open_column_of(code);
        self.layout.frame_stack.mark_stream_line_context(
            output_line_index,
            code.ends_with("<<") || code.ends_with(">>"),
            code.contains_from_first_byte("{ {"),
            unmatched_open_paren.is_some(),
            code.ends_with(')'),
            paren_closes > 0,
        );
        self.layout.frame_stack.mark_logical_line_context(
            output_line_index,
            unmatched_open_paren,
            code.ends_with(')'),
            paren_closes > 0,
        );
        if self.layout.line_state.ternary_colon || code.trimmed_start().starts_with(':') {
            self.layout
                .frame_stack
                .mark_last_ternary_colon_output_line(output_line_index);
        }
        if self.layout.frame_stack.has_open_ternary() {
            self.layout
                .frame_stack
                .mark_line_ended_open_ternary(output_line_index);
        }
    }

    pub(crate) fn observe_operator_chain_output_line(
        &mut self,
        output_line_index: usize,
    ) -> Option<usize> {
        let output_line = self.output.get(output_line_index)?;
        let output_indent = leading_visual_width(output_line, self.options.tab_width);
        self.layout
            .frame_stack
            .mark_stream_line_output_indent(output_line_index, output_indent);
        self.layout
            .frame_stack
            .mark_logical_line_output_indent(output_line_index, output_indent);
        self.observe_ternary_colon_output_line(output_line_index);
        Some(output_indent)
    }

    pub(crate) fn observe_ternary_colon_output_line(&mut self, output_line_index: usize) {
        let starts_with_colon = output_line_index < self.output.len()
            && self
                .output
                .code_before_comment(output_line_index)
                .trimmed_start()
                .starts_with(':');
        if starts_with_colon {
            self.layout
                .frame_stack
                .mark_last_ternary_colon_output_line(output_line_index);
        }
    }

    /// Marks a ready line led by `:`; returns the `:` line and the tail
    /// line it splits into, if it does.
    pub(crate) fn postprocess_ready_operator_chain_line(
        &mut self,
        output_line_index: usize,
        line: &LineView<'_>,
    ) -> Option<(String, String)> {
        if !line.trimmed_start().starts_with(':') {
            return None;
        }
        self.layout
            .frame_stack
            .mark_last_ternary_colon_output_line(output_line_index);
        self.split_ternary_colon_after_chained_true_arm(line)
    }

    pub(crate) fn stream_chain_frame_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let frame = self.layout.frame_stack.active_stream()?;
        frame
            .after_multiline_braced_operand
            .then_some(frame.chain_anchor_column)
    }

    pub(crate) fn parenthesized_after_trailing_stream_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with('(') {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .layout
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if !stream.operator_ends_output_line {
            return None;
        }
        Some(
            if stream
                .operator_output_column
                .saturating_sub(stream.line_indent_spaces)
                > self.options.max_continuation_indent
            {
                stream.line_indent_spaces + self.options.indent_width * 2
            } else {
                stream.operator_output_column
            },
        )
    }

    pub(crate) fn comment_separated_stream_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !starts_with_chain_operator(line.trimmed_start()) {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !is_comment_line(previous.trimmed_start()) {
            return None;
        }
        let before_comment = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trimmed().is_empty())?;
        if !starts_with_chain_operator(before_comment.trimmed_start()) {
            return None;
        }
        let before_comment_line = self
            .output
            .iter()
            .position(|line| line.as_str() == before_comment.as_str())?;
        self.layout
            .frame_stack
            .active_stream_on_output_line(before_comment_line)?;
        Some(leading_visual_width(before_comment, self.options.tab_width))
    }

    pub(crate) fn comment_separated_leading_operator_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !starts_with_chain_operator(line.trimmed_start()) {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !is_comment_line(previous.trimmed_start()) {
            return None;
        }
        let before_comment = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trimmed().is_empty())?;
        starts_with_chain_operator(before_comment.trimmed_start())
            .then(|| leading_visual_width(before_comment, self.options.tab_width))
    }

    pub(crate) fn comment_terminated_logical_chain_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"#}:") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let split = trailing_comment_split_limit(previous);
        let previous_code = previous[..split].trimmed_end();
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        (split < previous.len()
            && previous_code.ends_with("||")
            && current_spaces.unwrap_or(0) < previous_indent)
            .then_some(previous_indent)
    }

    pub(crate) fn maximum_length_logical_header_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.max_code_length.is_none()
            || !self.options.indent_after_parens
            || !(line_start.starts_with("&&") || line_start.starts_with("||"))
        {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        let configured = (self.continuation_base_indent() + self.options.continuation_indent)
            * self.options.indent_width;
        (self.token_input.token_source_line_indent == configured
            && self.open_paren_column_of(previous_code).is_some()
            && ["if", "for", "while", "switch"]
                .iter()
                .any(|header| starts_header_word(previous_trimmed, header)))
        .then_some(configured)
    }

    pub(crate) fn trailing_stream_top_level_indent_spaces(
        &self,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with("<<") && !previous_code.ends_with(">>") {
            return None;
        }
        // A shift inside a subscript continues past its bracket.
        if last_unmatched_open_delimiter(previous_code).is_some_and(|(open, _)| open == '[') {
            return None;
        }
        self.line_after_trailing_stream_operator_indent_spaces()
            .or_else(|| self.previous_stream_chain_indent_spaces())
    }

    pub(crate) fn set_next_line_indent_after_ternary_colon(
        &mut self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: usize,
    ) {
        if line_kind != LineKind::Normal
            || !line.trimmed_start().starts_with_any(b"<>|&+-*/%=!?:,.~")
        {
            return;
        }
        let Some(previous) = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trimmed().is_empty())
        else {
            return;
        };
        let previous_code = self.output.code_trimmed_of(previous);
        if previous_code.contains('?')
            && previous_code.ends_with(':')
            && !self.output.code_trimmed_of(line).ends_with(';')
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(current_spaces);
        }
    }

    pub(crate) fn pico_leading_operator_after_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Pico
            || !line.trimmed_start().starts_with_any(b"<>|&+-*/%=!?:,.")
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        previous_code
            .ends_with('}')
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn stream_after_string_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let string = self
            .layout
            .frame_stack
            .string_continuation_on_output_line(previous_line)?;
        (string.has_stream_context
            && string.literal_start_column == string.line_indent_spaces
            && !string.has_opening_context
            && !string.inside_delimiter_context)
            .then_some(
                string.line_indent_spaces
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            )
    }

    pub(crate) fn stream_after_closed_brace_frame_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let brace = self.layout.frame_stack.last_closed_brace()?;
        if brace.close_output_line != Some(previous_line) || !brace.close_ends_output_line {
            return None;
        }
        Some(self.layout.frame_stack.active_stream()?.chain_anchor_column)
    }

    pub(crate) fn previous_leading_stream_frame_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .layout
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        (stream.operator_output_column == stream.line_indent_spaces)
            .then_some(stream.line_indent_spaces)
    }

    pub(crate) fn stream_after_ternary_colon_frame_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        if self.layout.frame_stack.last_ternary_colon_output_line() != Some(previous_line) {
            return None;
        }
        Some(self.layout.frame_stack.active_stream()?.chain_anchor_column)
    }

    pub(crate) fn line_start_stream_adjacent_string_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !starts_string_literal_token(line_start) {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .string_continuation_before_output_line(self.output.len())?;
        if !frame.line_starts_with_chain_operator || !frame.has_opening_context {
            return None;
        }
        let current = line_start;
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if string_literal_token_end(current, 0).is_some_and(|end| {
            let rest = current[end..].trimmed_start();
            rest.starts_with("<<") || rest.starts_with(">>")
        }) {
            return Some(frame.line_indent_spaces + case_unindent);
        }
        Some(frame.literal_start_column + case_unindent)
    }

    pub(crate) fn line_after_trailing_stream_operator_indent_spaces(&self) -> Option<usize> {
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .layout
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if !stream.operator_ends_output_line {
            return None;
        }
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if let Some(delimiter) = self.layout.frame_stack.active_delimiter()
            && delimiter.opener_output_line < previous_line
            && stream.line_indent_spaces == delimiter.opener_output_column + 1
        {
            return Some(stream.line_indent_spaces + case_unindent);
        }
        if let Some(string) = self
            .layout
            .frame_stack
            .string_continuation_before_output_line(self.output.len())
            && string.output_line == previous_line
            && string.literal_start_column == stream.line_indent_spaces
        {
            return Some(stream.line_indent_spaces + case_unindent);
        }
        if let Some(delimiter) = self.layout.frame_stack.active_delimiter()
            && delimiter.opener_output_line == previous_line
            && delimiter.opener_output_column < stream.operator_output_column
        {
            return Some(delimiter.opener_output_column + 1 + case_unindent);
        }
        None
    }

    pub(crate) fn string_after_stream_string_indent_spaces(&self) -> Option<usize> {
        if self.in_initializer_brace() || self.current_inline_array_column().is_some() {
            return None;
        }
        let string = self
            .layout
            .frame_stack
            .string_continuation_before_output_line(self.output.len())?;
        let stream = self
            .layout
            .frame_stack
            .first_stream_on_output_line(string.output_line)?;
        if stream.operator_output_column >= string.literal_start_column
            || string.has_open_brace_before_literal
        {
            return None;
        }
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let delimiter_count = self
            .layout
            .frame_stack
            .delimiter_count_after_output_column(string.output_line, stream.operator_output_column);
        if delimiter_count >= 2 {
            let max_column = string.line_indent_spaces + self.options.max_continuation_indent;
            let spaces = self
                .layout
                .frame_stack
                .last_delimiter_column_after_output_column(
                    string.output_line,
                    stream.operator_output_column,
                )
                .filter(|column| *column <= max_column)
                .unwrap_or(string.line_indent_spaces + self.options.indent_width * 2);
            return Some(spaces + case_unindent);
        }
        let spaces = if stream
            .operator_output_column
            .saturating_sub(string.line_indent_spaces)
            > self.options.max_continuation_indent
        {
            string.line_indent_spaces + self.options.indent_width * 2
        } else {
            stream.operator_output_column
        };
        Some(spaces + case_unindent)
    }

    pub(crate) fn ternary_operator_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#?:{}") {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !head_ends_binary_operator(previous_code) {
            return None;
        }
        let mut question_indent = None;
        for scan_index in self.output.scoped_range().rev().take(12) {
            let raw = &self.output[scan_index];
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            if code.contains('?') {
                question_indent = Some(leading_visual_width(raw, self.options.tab_width));
            }
            if find_assignment_operator(code).is_some() {
                let base = self.continuation_base_indent() * self.options.indent_width;
                return question_indent.filter(|spaces| *spaces > base);
            }
            if code.ends_with_any(b";{}") {
                return None;
            }
        }
        None
    }

    pub(crate) fn assignment_ternary_branch_after_colon_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#?:{}") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(':') {
            return None;
        }
        let mut branch_indent = Some(leading_visual_width(previous, self.options.tab_width));
        for scan_index in self.output.scoped_range().rev().skip(1).take(12) {
            let raw = &self.output[scan_index];
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if code.contains('?')
                && find_assignment_operator(code).is_some()
                && !code.ends_with(';')
            {
                return branch_indent;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
            branch_indent = Some(leading_visual_width(raw, self.options.tab_width));
        }
        None
    }

    pub(crate) fn return_ternary_branch_after_colon_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#?:{}") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(':') {
            return None;
        }
        let mut branch_indent = Some(leading_visual_width(previous, self.options.tab_width));
        for scan_index in self.output.scoped_range().rev().skip(1).take(12) {
            let raw = &self.output[scan_index];
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("return ") && trimmed.contains('?') && !code.ends_with(';') {
                return branch_indent;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
            branch_indent = Some(leading_visual_width(raw, self.options.tab_width));
        }
        None
    }

    pub(crate) fn return_ternary_call_argument_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#?:{}") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with('(') {
            return None;
        }
        for scan_index in self.output.scoped_range().rev().skip(1).take(12) {
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("return ") && trimmed.contains('?') {
                return Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return None;
            }
        }
        None
    }

    pub(crate) fn ternary_call_clear_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || !line.trimmed_end().ends_with(");") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(',') || self.open_paren_column_of(previous_code).is_none() {
            return None;
        }
        let in_preprocessor_else_context =
            self.output.scoped_range().rev().take(128).any(|index| {
                let trimmed = self.output.code_before_comment(index).trimmed();
                trimmed == "else" || trimmed.ends_with("} else")
            }) && self
                .output
                .scoped_range()
                .rev()
                .take_while(|&index| {
                    let code = self.output.code_before_comment(index).trimmed_end();
                    !(self.output.lead_width(index, self.options.tab_width) == 0
                        && code.ends_with('{')
                        && !code.trimmed_start().starts_with('#'))
                })
                .take(128)
                .any(|index| self.output.trimmed(index).starts_with('#'));
        in_preprocessor_else_context
            .then_some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn recent_ternary_argument_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if current.starts_with_any(b"#(){}") || !self.output.may_have_question() {
            return None;
        }
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(',')
            || current
                .rsplit_once('{')
                .is_some_and(|(prefix, _)| line_ends_compound_literal_cast(prefix.trimmed_end()))
            || self.open_paren_column_of(previous_code).is_some()
        {
            return None;
        }
        let mut saw_colon_argument = false;
        let has_recent_ternary_argument = (0..self.output.len()).rev().take(16).any(|index| {
            let code = self.output.code(index);
            let trimmed_code = self.output.code_trimmed(index);
            if trimmed_code.starts_with(':') && code.ends_with(',') {
                saw_colon_argument = true;
                false
            } else {
                saw_colon_argument && trimmed_code.contains('?')
            }
        });
        has_recent_ternary_argument.then_some(
            leading_visual_width(previous, self.options.tab_width)
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(crate) fn contextual_ternary_argument_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if current.starts_with_any(b"#(){}")
            || !current.ends_with(");")
            || !self.output.may_have_question()
        {
            return None;
        }
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with(',') || self.open_paren_column_of(previous_code).is_some() {
            return None;
        }
        let follows_ternary_argument = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .take_while(|line| {
                let code = self.output.code_trimmed_of(line);
                !(code.ends_with(';') || code == "{" || code == "}")
            })
            .any(|line| self.output.code_of(line).contains('?'));
        follows_ternary_argument.then_some(
            leading_visual_width(previous, self.options.tab_width)
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    fn assignment_rhs_first_line_indent(
        &self,
        previous: &str,
        require_question: bool,
    ) -> Option<usize> {
        let tab_width = self.options.tab_width;
        let range = self.output.scoped_range();
        let last = range
            .end
            .checked_sub(2)
            .filter(|&last| last >= range.start)?;
        let walk = self.assignment_rhs_walk(range.start, last);
        walk.assigned?;
        let saw_question = previous.contains('?') || walk.question;
        let candidate_indent = walk.first.map_or_else(
            || leading_visual_width(previous, tab_width),
            |first| leading_visual_width(&self.output[first], tab_width),
        );
        (!require_question || saw_question).then_some(candidate_indent)
    }

    /// The look back from line `last` down to `floor` for the line whose
    /// code ends with an assignment, read on from the last look as lines
    /// come.
    fn assignment_rhs_walk(&self, floor: usize, last: usize) -> AssignmentRhsWalk {
        let version = self.output.version();
        let cached = self
            .assignment_rhs_cache
            .get()
            .filter(|walk| (walk.version, walk.floor) == (version, floor) && walk.last <= last);
        let mut walk = AssignmentRhsWalk {
            version,
            floor,
            last,
            assigned: None,
            question: false,
            first: None,
        };
        let stop = cached.map_or(floor, |cached| cached.last + 1);
        let mut ended = false;
        for scan_index in (stop..=last).rev() {
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() {
                continue;
            }
            if code.ends_with('=')
                && !code.ends_with("==")
                && !code.ends_with("!=")
                && !code.ends_with("<=")
                && !code.ends_with(">=")
            {
                walk.assigned = Some(scan_index);
                ended = true;
                break;
            }
            if code.ends_with(';') || code == "{" || code == "}" || trimmed.ends_with(':') {
                ended = true;
                break;
            }
            if trimmed.starts_with('?') {
                walk.question = true;
            }
            walk.first = Some(scan_index);
        }
        if !ended && let Some(cached) = cached {
            walk.assigned = cached.assigned;
            walk.question |= cached.question;
            walk.first = cached.first.or(walk.first);
        }
        self.assignment_rhs_cache.set(Some(walk));
        walk
    }

    pub(crate) fn contextual_ternary_arm_indent_spaces(
        &self,
        line: &LineView<'_>,
        previous: &str,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        let previous_code = self.output.code_trimmed_of(previous);
        // An arm leads the line, or the line follows one ending with `:`.
        if !current.starts_with_any(b"?:") && !previous_code.ends_with(':') {
            return None;
        }
        let previous_trimmed = previous_code.trimmed_start();
        let tab_width = self.options.tab_width;
        let width = self.options.indent_width;
        if current.starts_with('?')
            && previous_trimmed.starts_with("return ")
            && self.recent_base_trailing_return_function_header()
        {
            return Some(leading_visual_width(previous, tab_width));
        }
        if current.starts_with('?')
            && (previous_trimmed.starts_with('(') || previous_trimmed.starts_with("return ("))
            && let Some(open) = self.open_paren_column_of(previous_code)
        {
            return Some(column_after(previous_code, open, tab_width));
        }
        if current.starts_with('?')
            && previous_code.ends_with(')')
            && let Some(call_indent) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take_while(|line| {
                    let code = self.output.code_trimmed_of(line);
                    !(code.ends_with(';')
                        || code == "{"
                        || code == "}"
                        || (code.ends_with('=')
                            && !code.ends_with("==")
                            && !code.ends_with("!=")
                            && !code.ends_with("<=")
                            && !code.ends_with(">=")))
                })
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    code.ends_with('(')
                        .then(|| leading_visual_width(line, tab_width))
                })
        {
            return Some(call_indent);
        }
        if (current.starts_with(": ") || current == ":")
            && previous_code.contains('?')
            && !previous_code.trimmed_start().starts_with("return ")
            && let Some(open) = self.open_paren_column_of(previous_code)
        {
            return Some(
                column_after(previous_code, open, tab_width)
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if let Some(spaces) =
            self.return_ternary_colon_after_multiline_template_declaration_indent_spaces(line)
        {
            return Some(spaces);
        }
        if (current.starts_with(": ") || current == ":")
            && previous_code.contains('?')
            && !previous_code.trimmed_start().starts_with("return ")
            && (self.layout.nesting.paren_depth > 0
                || self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip(1)
                    .take_while(|line| {
                        let code = self.output.code_trimmed_of(line);
                        !(code.ends_with(';') || code == "{" || code == "}")
                    })
                    .any(|line| {
                        let code = self.output.code_trimmed_of(line);
                        code.ends_with('(') || self.open_paren_column_of(code).is_some()
                    }))
        {
            return Some(
                leading_visual_width(previous, tab_width)
                    + self.layout.line_adjuster.total_case_unindent_depth() * width,
            );
        }
        if let Some(spaces) = self.ternary_arm_frame_indent_spaces(current) {
            return Some(spaces);
        }
        let ternary_arm = if current.starts_with('?') {
            Some(false)
        } else if current.starts_with(": ") || current == ":" {
            Some(true)
        } else {
            None
        };
        if let Some(require_question) = ternary_arm
            && let Some(indent) = self.assignment_rhs_first_line_indent(previous, require_question)
        {
            return Some(indent);
        }
        if !current.starts_with_any(b")}?:")
            && previous_code.ends_with(':')
            && !previous_code.contains('?')
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take_while(|line| {
                    let code = self.output.code_trimmed_of(line);
                    !(code.ends_with(';') || code == "{" || code == "}")
                })
                .any(|line| self.output.code_of(line).contains('?'))
            && self
                .return_ternary_branch_after_colon_indent_spaces(line)
                .is_none()
        {
            return Some(
                leading_visual_width(previous, tab_width)
                    + self.layout.line_adjuster.total_case_unindent_depth() * width,
            );
        }
        if !current.starts_with_any(b")}?:")
            && previous_code.ends_with(':')
            && previous_code.contains('?')
            && let Some(open) = self.open_paren_column_of(previous_code)
        {
            let column = column_after(previous_code, open, tab_width);
            let column = self
                .control_condition_header_indent()
                .map_or(column, |header| {
                    column.max(header + min_conditional_indent_spaces(self.options))
                });
            return Some(
                column
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        None
    }

    pub(crate) fn return_chain_indent_spaces(
        &self,
        current: &str,
        previous: &str,
        natural: usize,
    ) -> Option<usize> {
        if !starts_with_chain_operator(current) {
            return None;
        }
        let previous_code = self.output.code_trimmed_of(previous);
        let previous_trimmed = previous_code.trimmed_start();
        if !starts_header_word(previous_trimmed, "return") {
            return None;
        }
        if self.options.indent_after_parens {
            return Some(natural);
        }
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if let Some(open) = self.open_paren_column_of(previous_code) {
            return Some(column_after(previous_code, open, self.options.tab_width) + case_unindent);
        }
        let after_return = &previous_trimmed["return".len()..];
        let value_offset = "return".len()
            + after_return
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map_or(after_return.len(), |(index, _)| index);
        Some(leading_visual_width(previous, self.options.tab_width) + value_offset + case_unindent)
    }

    pub(crate) fn contextual_ternary_colon_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if !current.starts_with(':') {
            return None;
        }
        if let Some(spaces) =
            nested_ternary_colon_sibling_indent_spaces(self.options, current, previous)
        {
            return Some(spaces);
        }
        let previous_code = self.output.code_trimmed_of(previous);
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        (current.starts_with(':') && previous_code.trimmed_start().starts_with('?')).then_some(
            previous_indent
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(crate) fn contextual_stream_brace_indent_spaces(&self, current: &str) -> Option<usize> {
        let width = self.options.indent_width;
        if (current.starts_with("<<") || current.starts_with(">>"))
            && current.contains('{')
            && !current.contains('}')
            && self
                .output
                .len()
                .checked_sub(1)
                .is_some_and(|previous_line| {
                    self.layout
                        .frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_some_and(|stream| {
                            stream.operator_output_column != stream.line_indent_spaces
                        })
                })
        {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>"))
            && current.trimmed_end().ends_with('{')
            && !current.contains('}')
        {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>"))
            && current.contains_from_first_byte("{ {")
        {
            return Some(self.continuation_base_indent() * width);
        }
        if (current.starts_with("<<") || current.starts_with(">>"))
            && self
                .output
                .len()
                .checked_sub(1)
                .is_some_and(|previous_line| {
                    self.layout
                        .frame_stack
                        .active_stream_on_output_line(previous_line)
                        .is_some_and(|stream| stream.line_contains_nested_brace)
                })
        {
            return Some(self.continuation_base_indent() * width + width * 2);
        }
        None
    }

    pub(crate) fn line_follows_logical_operator(&self) -> bool {
        self.output.last_non_empty_scoped().is_some_and(|previous| {
            let code = self.output.code_trimmed_of(previous);
            code.ends_with("||") || code.ends_with("&&")
        })
    }

    pub(crate) fn operator_chain_owns_continuation(&self, line: &LineView<'_>) -> bool {
        let follows_stream_operator =
            self.output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let code = self.output.code_trimmed_of(previous);
                    code.ends_with("<<") || code.ends_with(">>")
                });
        self.layout.frame_stack.active_ternary().is_some()
            || self.line_follows_logical_operator()
            || follows_stream_operator
            || starts_with_chain_operator(line.trimmed_start())
    }

    pub(crate) fn header_operator_continuation_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(trimmed.starts_with("&&") || trimmed.starts_with("||")) {
            return None;
        }
        if !self.split_else_body_indent_active()
            && self
                .layout
                .frame_stack
                .active_delimiter()
                .is_some_and(|delimiter| {
                    delimiter.role == ParenRole::Header
                        && delimiter.opener_output_line + 1 == self.output.len()
                })
        {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        let previous_trimmed = previous_code.trimmed_start();
        let previous_header = previous_trimmed
            .strip_prefix('}')
            .map(str::trim_start)
            .unwrap_or(previous_trimmed);
        if self.split_else_body_indent_active()
            && (previous_trimmed.starts_with("&&") || previous_trimmed.starts_with("||"))
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let (closes, opens) = self.paren_imbalance_of(previous_code);
            if !opens.is_empty() {
                return Some(previous_indent + self.options.indent_width);
            }
            if closes > 0
                && let Some(sibling) =
                    self.output
                        .scoped()
                        .iter()
                        .rev()
                        .skip(1)
                        .take(16)
                        .find(|line| {
                            let code = self.output.code_trimmed_of(line);
                            let trimmed = code.trimmed_start();
                            (trimmed.starts_with("&&") || trimmed.starts_with("||"))
                                && leading_visual_width(line, self.options.tab_width)
                                    < previous_indent
                        })
            {
                return Some(leading_visual_width(sibling, self.options.tab_width));
            }
            return Some(previous_indent);
        }
        if !(starts_header_word(previous_header, "if")
            || starts_header_word(previous_header, "while")
            || previous_header.starts_with("else if"))
        {
            return None;
        }
        self.open_paren_column_of(previous_code).map(|open| {
            let column = column_after(previous_code, open, self.options.tab_width) - 1;
            let base = leading_visual_width(previous, self.options.tab_width);
            let paren_indent = column
                + if previous_header.starts_with("else if(") {
                    2
                } else {
                    1
                };
            if self.split_else_body_indent_active() {
                let starts_nested_group = previous_header
                    .strip_prefix("else if")
                    .or_else(|| previous_header.strip_prefix("if"))
                    .or_else(|| previous_header.strip_prefix("while"))
                    .or_else(|| previous_header.strip_prefix("for"))
                    .or_else(|| previous_header.strip_prefix("switch"))
                    .is_some_and(|tail| tail.trimmed_start().starts_with("( ("));
                if previous_header.starts_with("else if") {
                    paren_indent
                } else if starts_nested_group {
                    base + min_conditional_indent_spaces(self.options)
                } else if self.paren_imbalance_of(previous_code).1.len() > 1 {
                    column + 1
                } else {
                    column
                }
            } else if self.options.indent_after_parens {
                base + self.options.continuation_indent * self.options.indent_width
            } else {
                paren_indent.max(base + min_conditional_indent_spaces(self.options))
            }
        })
    }

    fn previous_statement_is_braceless_ternary(&self) -> bool {
        // The last of the 12 lines before the last one that ends a statement
        // or block decides.
        let range = self.output.scoped_range();
        let end = range.end.saturating_sub(1).max(range.start);
        let start = range.start.max(range.end.saturating_sub(13));
        self.output
            .last_line_looked(&self.statement_end_code_look, start, end, |index| {
                !self.output.code_body(index).is_empty()
                    && self
                        .output
                        .code_before_comment_trimmed(index)
                        .ends_with_any(b";{}")
            })
            .is_some_and(|index| {
                starts_ternary_arm(self.output.code_body(index))
                    && self
                        .output
                        .code_before_comment_trimmed(index)
                        .ends_with(';')
            })
    }

    pub(crate) fn logical_condition_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#{}") {
            return None;
        }
        let scope = self.output.scoped_range();
        if !self
            .output
            .has_if_or_while_line_from(scope.start.max(scope.end.saturating_sub(8)))
        {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let code = self.output.code_trimmed_of(previous);
        if code.ends_with("||") {
            let base = self.continuation_base_indent() * self.options.indent_width;
            let standard = base + self.options.continuation_indent * self.options.indent_width;
            let conditional_floor = base + min_conditional_indent_spaces(self.options);
            if is_braceless_header_line(code.trimmed_start()) && self.leaves_paren_open(code) {
                return None;
            }
            if code.ends_with(") ||")
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip(1)
                    .take(8)
                    .any(|line| {
                        let code = self.output.code_trimmed_of(line);
                        let trimmed = code.trimmed_start();
                        trimmed.starts_with('(')
                            && code.ends_with(',')
                            && trimmed
                                .split_once(',')
                                .is_some_and(|(head, _)| head.contains(')'))
                    })
            {
                return Some(standard);
            }
            if !trimmed.starts_with('(')
                && let Some(spaces) = self.parenthesized_logical_operand_indent_after_call_tail()
            {
                return Some(spaces);
            }
            if self.token_input.token_source_line_indent > standard.max(conditional_floor) {
                return Some(self.token_input.token_source_line_indent);
            }
            if trimmed.starts_with("(!") && code.trimmed_start().contains("((") {
                return None;
            }
            let paren_balance: isize = code
                .chars()
                .map(|ch| match ch {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                })
                .sum();
            if paren_balance < -1 {
                return None;
            }
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let current_indent = self.current_line_indent_spaces() + self.options.indent_width * 2;
            if previous_indent <= current_indent {
                return None;
            }
            if paren_balance == -1
                && self
                    .layout
                    .nesting
                    .current_continuation_indent_spaces()
                    .is_none()
            {
                return None;
            }
            if paren_balance == -1 && !trimmed.starts_with("(!") {
                if self.options.min_conditional_indent == MinConditionalIndent::Zero {
                    return Some(
                        self.continuation_base_indent() * self.options.indent_width
                            + self.options.continuation_indent * self.options.indent_width,
                    );
                }
                return Some(previous_indent.saturating_sub(1));
            }
            if paren_balance == -1 && trimmed.starts_with("(!") {
                return Some(previous_indent.saturating_sub(1));
            }
            return Some(previous_indent);
        }
        None
    }

    fn parenthesized_logical_operand_indent_after_call_tail(&self) -> Option<usize> {
        for scan_index in self.output.scoped_range().rev().skip(1).take(8) {
            let previous = &self.output[scan_index];
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            let trimmed = self.output.code_body(scan_index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('(') && code.ends_with(',') {
                return Some(leading_visual_width(previous, self.options.tab_width) + 1);
            }
            if trimmed.starts_with("if ") || code.ends_with("&&") || code.ends_with(';') {
                return None;
            }
        }
        None
    }

    pub(crate) fn return_ternary_tail_output_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with('?') {
            return None;
        }
        for previous in self.output.scoped().iter().rev().take(12) {
            let code = previous.trimmed_start();
            let code = self.output.code_trimmed_of(code);
            if code.is_empty() || code.starts_with('#') {
                continue;
            }
            if code.ends_with_any(b";{}") {
                return None;
            }
            if code.starts_with("return ") {
                let return_indent = leading_visual_width(previous, self.options.tab_width);
                let inside_switch = self
                    .layout
                    .nesting
                    .brace_header_stack
                    .iter()
                    .any(|header| header.as_deref() == Some("switch"));
                if inside_switch {
                    if self.token_input.token_source_line_indent
                        <= return_indent + self.options.indent_width * 2
                    {
                        return Some(self.current_line_indent_spaces());
                    }
                    return Some(self.token_input.token_source_line_indent);
                }
                if self.recent_base_trailing_return_function_header() {
                    return Some(return_indent);
                }
                return Some(return_indent + "return ".len());
            }
        }
        None
    }

    pub(crate) fn ternary_operator_tail_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trimmed_start().starts_with('(') {
            return None;
        }
        self.output.last_non_empty_scoped().and_then(|previous| {
            let previous_code = self.output.code_trimmed_of(previous);
            let previous_trimmed = previous_code.trimmed_start();
            if previous_trimmed.starts_with(": ") && head_ends_binary_operator(previous_code) {
                self.open_paren_column_of(previous_code)
                    .map(|open| open + 1)
            } else {
                None
            }
        })
    }

    fn split_ternary_colon_after_chained_true_arm(
        &self,
        line: &LineView<'_>,
    ) -> Option<(String, String)> {
        let current = line.trimmed_start();
        let tail = current.strip_prefix(": ")?;
        if tail.is_empty() {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        if !self
            .output
            .code_of(previous)
            .trimmed_start()
            .starts_with('.')
        {
            return None;
        }
        let question_indent =
            self.output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(8)
                .find_map(|line| {
                    self.output
                        .code_of(line)
                        .trimmed_start()
                        .starts_with('?')
                        .then(|| leading_visual_width(line, self.options.tab_width))
                })?;
        let indent = leading_visual_width(line, self.options.tab_width);
        if indent != question_indent {
            return None;
        }
        let prefix = &line[..line.len() - current.len()];
        Some((format!("{prefix}:"), format!("{prefix}{tail}")))
    }

    pub(crate) fn return_ternary_colon_after_multiline_template_declaration_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !(current.starts_with(": ") || current == ":") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        if !self
            .output
            .code_of(previous)
            .trimmed_start()
            .starts_with('?')
        {
            return None;
        }
        if !self.recent_trailing_return_function_after_multiline_template_declaration() {
            return None;
        }
        Some(
            leading_visual_width(previous, self.options.tab_width)
                + self.options.indent_width
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    fn capped_parenthesized_stream_indent(&self, spaces: usize, line_indent: usize) -> usize {
        let base = self.continuation_base_indent() * self.options.indent_width;
        if spaces.saturating_sub(base) > self.options.max_continuation_indent {
            line_indent + self.options.indent_width * 2
        } else {
            spaces
        }
    }

    pub(crate) fn parenthesized_stream_chain_head_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if !current.starts_with("<<") && !current.starts_with(">>") && !current.starts_with("//") {
            return None;
        }
        let delimiter = self.layout.frame_stack.active_delimiter()?;
        if delimiter.opener_output_line >= self.output.len() {
            return None;
        }
        self.parenthesized_stream_indent_for_line(delimiter.opener_output_line)
    }

    pub(crate) fn nested_brace_after_stream_opener_indent_spaces(
        &self,
        current: &str,
        previous_code: &str,
    ) -> Option<usize> {
        if !previous_code.ends_with('(')
            || current.starts_with_any(b"#(){}")
            || !current.contains_from_first_byte("{ {")
        {
            return None;
        }
        let previous_trimmed = previous_code.trimmed_start();
        (previous_trimmed.starts_with("<<") || previous_trimmed.starts_with(">>"))
            .then(|| self.continuation_base_indent() * self.options.indent_width)
    }

    pub(crate) fn previous_line_parenthesized_stream_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("<<") && !trimmed.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        self.parenthesized_stream_indent_for_line(previous_line)
    }

    pub(crate) fn stream_after_closed_parenthesized_head_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !current.starts_with("<<") && !current.starts_with(">>") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        let stream = self
            .layout
            .frame_stack
            .active_stream_on_output_line(previous_line)?;
        if stream.operator_output_column != stream.line_indent_spaces
            || !stream.line_ends_with_close_paren
            || !stream.line_has_positive_paren_delta
            || stream.line_has_unmatched_open_paren
        {
            return None;
        }
        self.layout
            .frame_stack
            .stream_before_output_line_with_unmatched_open_paren(previous_line)
            .map(|head| head.line_indent_spaces)
    }

    fn parenthesized_stream_indent_for_line(&self, line: usize) -> Option<usize> {
        let stream = self.layout.frame_stack.first_stream_on_output_line(line)?;
        if !stream.line_has_unmatched_open_paren {
            return None;
        }
        let delimiter_column = self
            .layout
            .frame_stack
            .first_delimiter_column_after_output_column(line, stream.operator_output_column)?;
        if stream.operator_output_column >= delimiter_column {
            return None;
        }
        let line_indent = stream.line_indent_spaces;
        let stream_spaces = stream.operator_output_column;
        let paren_spaces = delimiter_column + 1;
        let starts_with_operator = stream.operator_output_column == line_indent;
        let spaces = if starts_with_operator
            || paren_spaces.saturating_sub(line_indent) <= self.options.max_continuation_indent
        {
            paren_spaces
        } else {
            stream_spaces
        };
        Some(self.capped_parenthesized_stream_indent(spaces, line_indent))
    }

    pub(crate) fn logical_after_previous_frame_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if !current.starts_with("&&") && !current.starts_with("||") {
            return None;
        }
        let previous_line = self.output.len().checked_sub(1)?;
        if self
            .layout
            .frame_stack
            .active_delimiter()
            .is_some_and(|delimiter| {
                delimiter.role == ParenRole::Header && delimiter.opener_output_line == previous_line
            })
        {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .active_logical_on_output_line(previous_line)?;
        if !frame.operator_starts_output_line {
            return None;
        }
        let current_operator = if current.starts_with("&&") {
            LogicalOperator::And
        } else {
            LogicalOperator::Or
        };
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if frame.line_has_positive_paren_delta {
            if frame.line_ends_with_close_paren {
                let candidate = frame
                    .line_indent_spaces
                    .saturating_sub(1)
                    .saturating_add(case_unindent);
                if self.token_input.token_source_line_indent > 0
                    && self.token_input.token_source_line_indent + self.options.indent_width
                        < candidate
                {
                    return Some(self.token_input.token_source_line_indent);
                }
                return Some(candidate);
            }
            if let Some(open) = self
                .layout
                .frame_stack
                .logical_before_output_line_with_return(previous_line)
                .and_then(|frame| frame.line_unmatched_open_paren_column)
            {
                return Some(open + case_unindent);
            }
            return None;
        }
        if frame.operator != current_operator {
            return None;
        }
        Some(
            frame
                .line_unmatched_open_paren_column
                .map_or(frame.line_indent_spaces, |open| open + 1)
                + case_unindent,
        )
    }

    pub(crate) fn ternary_colon_row_frame_indent_spaces(&self, current: &str) -> Option<usize> {
        if !(current.starts_with(": ") || current == ":") {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.trimmed_start().starts_with(':') || !previous_code.contains('?') {
            return None;
        }
        let frame = self.layout.frame_stack.last_ternary_with_colon()?;
        if frame.colon_role != Some(ColonRole::Ternary) {
            return None;
        }
        frame.colon_output_column
    }

    fn ternary_arm_frame_indent_spaces(&self, current: &str) -> Option<usize> {
        if current.starts_with('?') {
            let frame = self.layout.frame_stack.active_ternary()?;
            if frame.colon_role.is_some()
                || (!matches!(
                    frame.owner_role,
                    TernaryOwnerRole::Assignment | TernaryOwnerRole::Return
                ) && frame.parent_delimiter.is_none())
            {
                return None;
            }
            let case_unindent =
                self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if frame.owner_role == TernaryOwnerRole::Return {
                if self.recent_base_trailing_return_function_header() {
                    return Some(frame.question_indent_spaces + case_unindent);
                }
                if frame.parent_delimiter.is_some()
                    && let Some(return_line) =
                        self.output.scoped().iter().rev().take(8).find(|line| {
                            let code = self.output.code_trimmed_of(line);
                            let trimmed = code.trimmed_start();
                            trimmed.starts_with("return ")
                                && !trimmed.contains('?')
                                && self.open_paren_column_of(code).is_none()
                        })
                {
                    return Some(
                        leading_visual_width(return_line, self.options.tab_width)
                            + "return ".len()
                            + case_unindent,
                    );
                }
            }
            let spaces = frame
                .parent_delimiter
                .and_then(|id| self.layout.frame_stack.delimiter_by_id(id))
                .map_or(frame.branch_anchor_column?, |delimiter| {
                    delimiter.opener_output_column + 1
                });
            return Some(spaces + case_unindent);
        }
        if current.starts_with(": ") || current == ":" {
            let frame = self.layout.frame_stack.last_ternary_with_colon()?;
            if frame.colon_role != Some(ColonRole::Ternary)
                || (!matches!(
                    frame.owner_role,
                    TernaryOwnerRole::Assignment | TernaryOwnerRole::Return
                ) && frame.parent_delimiter.is_none())
            {
                return None;
            }
            let case_unindent =
                self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            if frame.owner_role == TernaryOwnerRole::Return {
                if self.recent_base_trailing_return_function_header() {
                    if self.recent_trailing_return_function_after_multiline_template_declaration() {
                        return Some(
                            frame.question_indent_spaces
                                + self.options.indent_width
                                + case_unindent,
                        );
                    }
                    return Some(frame.question_indent_spaces + case_unindent);
                }
                if frame.parent_delimiter.is_some()
                    && let Some(question_line) = self
                        .output
                        .scoped()
                        .iter()
                        .rev()
                        .take(8)
                        .find(|line| self.output.code_of(line).trimmed_start().starts_with('?'))
                {
                    return Some(
                        leading_visual_width(question_line, self.options.tab_width) + case_unindent,
                    );
                }
            }
            let spaces = frame
                .parent_delimiter
                .and_then(|id| self.layout.frame_stack.delimiter_by_id(id))
                .map_or(frame.branch_anchor_column?, |delimiter| {
                    delimiter.opener_output_column + 1
                });
            return Some(spaces + case_unindent);
        }
        None
    }

    /// Visual column of the `(` that the first of `closes` unmatched `)` on
    /// output line `line` closes, searching the lines above it within the
    /// statement.
    fn opener_column_above(&self, line: usize, mut closes: usize) -> Option<usize> {
        let scope_start = self.output.len() - self.output.scoped().len();
        for index in (scope_start..line).rev() {
            let text = &self.output[index];
            if text.trimmed().is_empty() || self.output.is_directive_line(index) {
                continue;
            }
            let code = self.output.code_before_comment(index).trimmed_end();
            if code.ends_with_any(b";{}") {
                return None;
            }
            let (line_closes, opens) = self.paren_imbalance_of(code);
            if opens.len() >= closes {
                let open = opens[opens.len() - closes];
                return Some(column_after(code, open, self.options.tab_width) - 1);
            }
            closes = closes - opens.len() + line_closes;
        }
        None
    }

    pub(crate) fn ternary_first_arm_indent_spaces(&self, current: &str) -> Option<usize> {
        if current.is_empty() || current.starts_with_any(b"#?:{}") {
            return None;
        }
        let previous_index = self.output.last_non_empty_index()?;
        let code = self
            .output
            .code_before_comment(previous_index)
            .trimmed_end();
        if !code.ends_with('?') {
            return None;
        }
        let indent_width = self.options.indent_width;
        if self.options.indent_after_parens {
            let frame = self.layout.frame_stack.active_ternary()?;
            return Some(
                frame.question_indent_spaces
                    + self.options.continuation_indent * indent_width
                    + self.layout.line_adjuster.total_case_unindent_depth() * indent_width,
            );
        }
        let condition = code[..code.len() - 1].trimmed_end();
        let anchor = if let Some(open) = self.open_paren_column_of(condition) {
            // An assignment inside the open parentheses, as in a `for`
            // header, continues at its value.
            let inner = &condition[open + 1..];
            match find_assignment_operator(inner) {
                Some((operator_index, operator))
                    if self
                        .open_paren_column_of(&inner[..operator_index])
                        .is_none() =>
                {
                    let value = open + 1 + operator_index + operator.len();
                    let after = &condition[value..];
                    visual_width_from(
                        &condition[..value + (after.len() - after.trimmed_start().len())],
                        0,
                        self.options.tab_width,
                    )
                }
                _ => visual_width_from(&condition[..open + 1], 0, self.options.tab_width),
            }
        } else {
            let trimmed = condition.trimmed_start();
            let lead = condition.len() - trimmed.len();
            let operand_byte = if let Some(rest) = trimmed.strip_prefix("return") {
                if !rest.starts_with(char::is_whitespace) {
                    return None;
                }
                lead + "return".len() + (rest.len() - rest.trimmed_start().len())
            } else if let Some((operator_index, operator)) = find_assignment_operator(condition) {
                let after = &condition[operator_index + operator.len()..];
                operator_index + operator.len() + (after.len() - after.trimmed_start().len())
            } else if let (closes @ 1.., _) = self.paren_imbalance_of(condition)
                && let Some(column) = self.opener_column_above(previous_index, closes)
            {
                // The condition closes a group opened on an earlier line; the
                // arms align with that group.
                return Some(
                    column + self.layout.line_adjuster.total_case_unindent_depth() * indent_width,
                );
            } else {
                return Some(
                    leading_visual_width(condition, self.options.tab_width)
                        + self.layout.line_adjuster.total_case_unindent_depth() * indent_width,
                );
            };
            visual_width_from(&condition[..operand_byte], 0, self.options.tab_width)
        };
        Some(anchor + self.layout.line_adjuster.total_case_unindent_depth() * indent_width)
    }

    pub(crate) fn ternary_colon_after_comment_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if !current.starts_with(':') || !previous.trimmed_start().starts_with("//") {
            return None;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !trimmed.starts_with("//")
            })
            .is_some_and(|line| line.trimmed_start().starts_with('?'))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn post_ternary_colon_comma_sibling_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        if current.starts_with_any(b":)}") || !self.output.may_have_question() {
            return None;
        }
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.trimmed_start().starts_with(':')
            || !previous_code.ends_with(',')
            || self.open_paren_column_of(previous_code).is_some()
        {
            return None;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trimmed().is_empty())
            .is_some_and(|line| line.contains('?'))
            .then(|| {
                leading_visual_width(previous, self.options.tab_width)
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width
            })
    }

    pub(crate) fn split_else_operator_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        // Outside the body of a split `else`, the tree places the line.
        if self.output.pending_tokens().is_some_and(|span| {
            !self.tree.statements.in_split_else_body(span.first)
                && !self
                    .tree
                    .groups
                    .enclosing(span.first)
                    .is_some_and(|group| self.tree.functions.is_parameter_list(group))
        }) {
            return None;
        }
        let current = line.trimmed_start();
        if !current.starts_with_any(b"<>+-*/%=!?:,.~&|")
            && !self.output.last_non_empty_index().is_some_and(|index| {
                let previous = self.output[index].trimmed_end();
                previous.ends_with("&&") || previous.ends_with("||") || previous.ends_with('(')
            })
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let split = trailing_comment_split_limit(previous);
        let previous_code = previous[..split].trimmed_end();
        let split_else_chain = self.recent_split_else_output_chain_active();
        let in_split_preprocessor_context = self.recent_split_else_operator_region_active();
        if split_else_chain
            && (current.starts_with("&&") || current.starts_with("||"))
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
        {
            return self.output.scoped().iter().rev().take(8).find_map(|line| {
                let code = self.output.code_trimmed_of(line);
                assignment_value_column(code, self.options.tab_width)
            });
        }
        if in_split_preprocessor_context
            && current.starts_with_any(b"<>+-*/%=!?:,.~")
            && is_conditional_header_line(previous_code)
            && self.open_paren_column_of(previous_code).is_some()
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + min_conditional_indent_spaces(self.options),
            );
        }
        if (split < previous.len() || in_split_preprocessor_context)
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
            && !current.starts_with_any(b"})]")
        {
            return Some(
                assignment_value_column(previous_code, self.options.tab_width).unwrap_or_else(
                    || {
                        if is_conditional_header_line(previous_code) {
                            leading_visual_width(previous, self.options.tab_width)
                                + min_conditional_indent_spaces(self.options)
                        } else {
                            self.open_paren_column_of(previous_code)
                                .map(|column| column + 1)
                                .unwrap_or_else(|| {
                                    leading_visual_width(previous, self.options.tab_width)
                                        + if starts_header_word(
                                            previous_code.trimmed_start(),
                                            "return",
                                        ) {
                                            "return ".len()
                                        } else {
                                            0
                                        }
                                })
                        }
                    },
                ),
            );
        }
        (in_split_preprocessor_context
            && previous_code.ends_with('(')
            && !current.starts_with_any(b"})]"))
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(crate) fn split_else_ternary_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || !line.contains(" ? ") {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        (previous_code.ends_with(':') && previous_code.contains(" ? "))
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn split_else_ternary_comma_sibling_indent_floor(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
        current_spaces: usize,
    ) -> Option<usize> {
        if !split_else_context || !line.contains(" ? ") {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with(',') || !previous_code.contains(" ? ") {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces < target).then_some(target)
    }

    pub(crate) fn split_else_completed_ternary_call_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !split_else_context || line_start.starts_with_any(b"#{}") || is_comment_line(line_start)
        {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with(");") || !previous_code.contains(" ? ") {
            return None;
        }
        self.output.scoped().iter().rev().find_map(|line| {
            let code = self.output.code_trimmed_of(line);
            (code.ends_with(',') && self.open_paren_column_of(code).is_some())
                .then_some(leading_visual_width(line, self.options.tab_width))
        })
    }

    pub(crate) fn split_else_brace_logical_indent_spaces(
        &self,
        line: &LineView<'_>,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        let split_else_chain =
            structural_split_else_chain || self.recent_split_else_output_chain_active();
        if !split_else_chain {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let current = line.trimmed_start();
        let mut spaces = if (current.starts_with("&&") || current.starts_with("||"))
            && (previous_code.ends_with("&&") || previous_code.ends_with("||"))
        {
            self.output.scoped().iter().rev().take(8).find_map(|line| {
                let code = self.output.code_trimmed_of(line);
                assignment_value_column(code, self.options.tab_width)
            })
        } else if current.starts_with("||")
            && previous_code.trimmed_start().starts_with("||")
            && previous_code.ends_with(')')
        {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(8)
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    code.trimmed_start()
                        .starts_with("&& (")
                        .then_some(leading_visual_width(line, self.options.tab_width))
                })
        } else {
            None
        };
        if (current.starts_with("&&") || current.starts_with("||"))
            && line_is_control_body_header(previous_code.trimmed_start())
            && unmatched_open_paren_columns(previous_code).len() == 1
        {
            spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2,
            );
        }
        spaces
    }

    pub(crate) fn none_style_split_else_logical_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        split_else_state_active: bool,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line_start.starts_with_any(b"#{}")
            || is_comment_line(line_start)
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
            || !line_start.starts_with("||")
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        (previous_code.trimmed_start().starts_with("&&")
            && self.open_paren_column_of(previous_code).is_some())
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(crate) fn split_else_completed_logical_statement_indent_spaces(&self) -> Option<usize> {
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if (previous_code.trimmed_start().starts_with("||")
            || previous_code.trimmed_start().starts_with("&&"))
            && previous_code.ends_with(';')
        {
            return self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(8)
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    find_assignment_operator(code)
                        .is_some()
                        .then_some(leading_visual_width(line, self.options.tab_width))
                });
        }
        previous_code.ends_with(';').then(|| {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(4)
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    ((code.ends_with("||") || code.ends_with("&&"))
                        && find_assignment_operator(code).is_some())
                    .then_some(leading_visual_width(line, self.options.tab_width))
                })
        })?
    }

    pub(crate) fn split_else_assignment_logical_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !(current.starts_with("&&") || current.starts_with("||"))
            || !self.recent_split_else_output_chain_active()
        {
            return None;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .take_while(|line| {
                let code = self.output.code_trimmed_of(line);
                !code.ends_with(';') && !code.ends_with('{') && !code.ends_with('}')
            })
            .take(8)
            .find_map(|line| {
                let code = self.output.code_trimmed_of(line);
                assignment_value_column(code, self.options.tab_width)
            })
    }

    pub(crate) fn observe_split_else_logical_statement_indent(
        &mut self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) {
        if line_kind != LineKind::Normal || !line.trimmed_end().ends_with(';') {
            return;
        }
        let current = line.trimmed_start();
        let statement = if current.starts_with("||") || current.starts_with("&&") {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(8)
                .find(|line| {
                    let code = self.output.code_trimmed_of(line);
                    find_assignment_operator(code).is_some()
                })
        } else if self.recent_split_else_logical_statement_region_active() {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .take(4)
                .find(|line| {
                    let code = self.output.code_trimmed_of(line);
                    (code.ends_with("||") || code.ends_with("&&"))
                        && find_assignment_operator(code).is_some()
                })
        } else {
            None
        };
        if let Some(statement) = statement {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces =
                Some(leading_visual_width(statement, self.options.tab_width));
            self.layout.nesting.clear_continuation_indents();
        }
    }
}

fn assignment_value_column(code: &str, tab_width: usize) -> Option<usize> {
    let (assignment, operator) = find_assignment_operator(code)?;
    let after_operator = assignment + operator.len();
    let value_start = code[after_operator..]
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map_or(code.len(), |(offset, _)| after_operator + offset);
    Some(visual_width_from(&code[..value_start], 0, tab_width))
}

pub(crate) fn nested_ternary_colon_sibling_indent_spaces(
    options: &FormatOptions,
    current: &str,
    previous: &str,
) -> Option<usize> {
    if !current.starts_with(':') {
        return None;
    }
    let previous_code = previous[..trailing_comment_split_limit(previous)].trimmed_end();
    (previous_code.trimmed_start().starts_with(':')
        && previous_code.contains('?')
        && unmatched_open_paren_column(previous_code).is_none())
    .then(|| leading_visual_width(previous, options.tab_width))
}

pub(crate) fn inline_stream_opener_argument_indent_spaces(
    current: &str,
    previous_code: &str,
) -> Option<usize> {
    if current.starts_with_any(b"#(){}") || !previous_code.ends_with('(') {
        return None;
    }
    previous_code
        .find(" << ")
        .or_else(|| previous_code.find(" >> "))
        .map(|operator_start| operator_start + 5)
}

/// A look back for the line whose code ends with an assignment: the output
/// version and floor it read at, the last line it read from, the line it
/// found, whether a line after that one leads with `?`, and the first
/// nonblank line after it.
#[derive(Clone, Copy)]
pub(crate) struct AssignmentRhsWalk {
    version: u64,
    floor: usize,
    last: usize,
    assigned: Option<usize>,
    question: bool,
    first: Option<usize>,
}
