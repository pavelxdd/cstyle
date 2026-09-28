use crate::config::{BraceStyle, IndentStyle};
use crate::formatter::constructs::headers::{
    is_attachable_closing_header, is_header, same_line_nested_header_extra, starts_header_word,
};
use crate::formatter::constructs::labels;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::output::model::{LineLayout, LineReplayLayout};
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    line_paren_imbalance, preprocessor_directive, trailing_comment_split_limit,
};
use crate::source::lex::{is_identifier_continue, leading_identifier};

impl FormatEngine<'_> {
    pub(crate) fn apply_brace_header_case_and_initializer_correction_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        if self.options.indent_braces
            && matches!(
                self.options.brace_style,
                BraceStyle::None | BraceStyle::Allman
            )
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
        {
            let previous_brace_indent =
                self.output
                    .last_line_outside_comment()
                    .and_then(|previous| {
                        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                        code.ends_with('{')
                            .then(|| leading_visual_width(previous, self.options.tab_width))
                    });
            let target = previous_brace_indent
                .unwrap_or_else(|| indent.saturating_sub(1) * self.options.indent_width);
            if exact_indent_spaces.unwrap_or(indent * self.options.indent_width) > target {
                exact_indent_spaces = Some(target);
            }
        }
        if let Some(spaces) =
            self.active_case_control_closing_indent_override(line, indent, exact_indent_spaces)
        {
            exact_indent_spaces = Some(spaces);
        }
        if self.current_line_has_class_initializer_colon
            && line.trim_start().starts_with(':')
            && !line.trim_start().starts_with("::")
        {
            exact_indent_spaces = Some(indent * self.options.indent_width);
        }
        if let Some(spaces) = self.compound_case_label_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.split_switch_closing_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) && line.trim_start().starts_with(['.', '['])
            && let Some(frame) = self
                .layout
                .frame_stack
                .active_brace()
                .filter(|frame| frame.semantic_kind == BraceSemanticKind::CompoundLiteral)
        {
            exact_indent_spaces = Some(frame.sibling_indent_column);
        }
        if self.options.brace_style == BraceStyle::Whitesmith
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trim() == "{")
            && let Some(frame) = self.layout.frame_stack.active_brace().filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Definition
                    && self.layout.nesting.brace_type_stack.last() == Some(&frame.brace_type)
            })
        {
            exact_indent_spaces = Some(frame.sibling_indent_column);
        }
        let indented_command_body = self.indented_command_body_indent_spaces();
        let active_brace = self.layout.frame_stack.active_brace();
        let current_header_owns_active_brace = is_header(self.options, leading_identifier(line))
            && active_brace
                .zip(self.layout.frame_stack.active_header())
                .is_some_and(|(brace, header)| {
                    brace.header.as_deref() == Some(header.header.as_str())
                        && brace.header_indent_column == header.line_indent_spaces
                });
        let previous_output_brace = if current_header_owns_active_brace {
            self.layout.frame_stack.enclosing_brace()
        } else {
            active_brace
        };
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trim() == "{")
            && self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| preprocessor_directive(line.trim_start()) == Some("endif"))
            && let Some(frame) = previous_output_brace.filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command && frame.header.is_some()
            })
        {
            exact_indent_spaces = Some(
                frame.body_indent_column
                    + self.layout.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if line.trim() == "{"
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        } else if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}', '/'])
            && !is_header(self.options, leading_identifier(line))
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trim() == "{")
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && is_header(self.options, leading_identifier(line))
            && same_line_nested_header_extra(line.trim_start()) == 0
            && !(self.options.no_indent_if_after_else
                && starts_header_word(line.trim_start(), "if")
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|previous| matches!(previous.trim(), "else" | "} else")))
            && let Some(header) = self.layout.frame_stack.active_header().filter(|header| {
                header
                    .line_indent_spaces
                    .is_multiple_of(self.options.indent_width)
            })
        {
            let current = indent * self.options.indent_width;
            if exact_indent_spaces.is_none() && current < header.line_indent_spaces {
                exact_indent_spaces = Some(header.line_indent_spaces);
            }
        }
        if line.trim_start().starts_with("else")
            && let Some(header) = self
                .layout
                .frame_stack
                .active_header()
                .filter(|header| header.header == "else")
        {
            exact_indent_spaces = Some(header.line_indent_spaces);
        }
        if let Some(spaces) =
            self.vtk_or_ratliff_headerless_command_opening_brace_indent_spaces(line)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.gnu_command_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.gnu_command_closing_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.ratliff_command_closing_header_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && line.trim_start().starts_with(']')
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if ["for", "while", "switch"].iter().any(|header| {
                let trimmed = previous_code.trim_start();
                trimmed == *header
                    || trimmed
                        .strip_prefix(header)
                        .is_some_and(|rest| rest.starts_with([' ', '\t']))
            }) && !previous_code.contains('(')
            {
                exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some((column, true)) = self.inline_array.current_closed_body_column.take()
            && line_kind == LineKind::Normal
            && !line.trim_start().starts_with('}')
            && line.contains('}')
        {
            exact_indent_spaces = Some(column);
        }
        if let Some(spaces) = self.pending_split_else_braceless_body_indent_spaces(
            line,
            line_kind,
            indent * self.options.indent_width,
            exact_indent_spaces,
        ) {
            exact_indent_spaces = Some(spaces);
        }
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }

    pub(crate) fn apply_label_switch_case_and_opening_brace_correction_layout(
        &mut self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let normal_indent = layout.normal_indent;
        let class_scope_label = layout.class_scope_label;
        let mut indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        let (line_closing_parens, line_opening_parens) = line_paren_imbalance(line);
        let line_closes_outer_delimiter = line_closing_parens > line_opening_parens.len();
        let line_has_owned_continuation = self.layout.frame_stack.active_delimiter().is_some()
            || self.operator_chain_owns_continuation(line);
        if let Some(spaces) = self.post_block_case_body_indent_override(
            line,
            line_kind,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
            is_attachable_closing_header(leading_identifier(line)),
        ) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.nested_case_label_indent_override(line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.active_label_block_indent_spaces(
            line,
            line_kind,
            indent == normal_indent,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
        ) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.closed_label_block_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(case_layout) = self.active_case_block_body_layout(
            line,
            line_kind,
            indent == normal_indent,
            line_closes_outer_delimiter,
            line_has_owned_continuation,
            exact_indent_spaces,
        ) {
            exact_indent_spaces = Some(case_layout.exact_indent_spaces);
            if let Some(minimum) = case_layout.minimum_indent_level {
                indent = indent.max(minimum);
            }
        }
        if let Some(spaces) =
            self.switch_case_frame_closing_indent_override(line, exact_indent_spaces)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(label_layout) = labels::default_line_layout(
            line_kind,
            class_scope_label,
            indent,
            self.case_body_indent_extra(LineKind::Normal),
            self.options,
        ) {
            exact_indent_spaces = Some(label_layout.indent_spaces);
            if let Some(level) = label_layout.indent_level {
                indent = level;
            }
        }
        if let Some(spaces) = self.lambda_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.initializer_or_array_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && (line.trim_start().starts_with("///") || line.trim_start().starts_with("//!"))
            && self
                .layout
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Aggregate)
            && let Some(spaces) = self.active_body_comment_indent_spaces()
        {
            exact_indent_spaces = Some(spaces);
        }
        layout.indent = indent;
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }

    /// An `else` starting a line takes the indent of the line holding its
    /// `if`, from the structure tree.
    pub(crate) fn apply_else_matching_if_layout(
        &self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind != LineKind::Normal || line.trim_start().starts_with('#') {
            return layout;
        }
        if let Some(spaces) = self.else_matching_if_indent() {
            layout.exact_indent_spaces = Some(spaces);
        } else if let Some(spaces) = self.braceless_body_indent() {
            layout.exact_indent_spaces = Some(spaces);
        }
        layout
    }

    /// A braceless body starting a line takes one level past the line
    /// holding its header.
    fn braceless_body_indent(&self) -> Option<usize> {
        // Added braces make the body a block.
        if self.options.add_braces || self.options.add_one_line_braces {
            return None;
        }
        let first = self.output.pending_tokens()?.first;
        let tokens = &self.tree.tokens;
        let header = self.tree.statements.braceless_header(first)?;
        // astyle loses track of a body after a block in its header, such as
        // a lambda in the condition.
        if tokens[header..first]
            .iter()
            .any(|token| matches!(token, Token::Symbol('{')))
        {
            return None;
        }
        let line = self.line_led_by(header)?;
        // `else while (x)` nests two headers on one line; `else if` is one.
        let nested = !matches!(&tokens[header], Token::Word(word) if word == "if")
            && self
                .tree
                .previous_code_token(header)
                .is_some_and(|previous| {
                    matches!(&tokens[previous], Token::Word(word) if word == "else")
                        && self.output.line_with_token(previous) == Some(line)
                });
        let levels = 1 + usize::from(nested);
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + levels * self.options.indent_width
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    /// The output line holding the token `token` at its start, after at
    /// most `}` and `else`.
    fn line_led_by(&self, token: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let line = self.output.line_with_token(token)?;
        let first = self.output.line_tokens(line)?.first;
        let mut index = token;
        while index != first {
            index = self.tree.previous_code_token(index)?;
            if !matches!(&tokens[index], Token::Symbol('}'))
                && !matches!(&tokens[index], Token::Word(word) if word == "else")
            {
                return None;
            }
        }
        Some(line)
    }

    fn else_matching_if_indent(&self) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let else_token = self.output.pending_tokens()?.first;
        if !matches!(&tokens[else_token], Token::Word(word) if word == "else") {
            return None;
        }
        let if_token = self.tree.statements.if_of_else(else_token)?;
        let if_line = self.output.line_with_token(if_token)?;
        // The `if` leads its line, after at most `}` and `else`.
        let first = self.output.line_tokens(if_line)?.first;
        let mut token = if_token;
        while token != first {
            token = self.tree.previous_code_token(token)?;
            if !matches!(&tokens[token], Token::Symbol('}'))
                && !matches!(&tokens[token], Token::Word(word) if word == "else")
            {
                return None;
            }
        }
        // Layout indents come before the case-block unindent is taken off;
        // the `if` line has had it taken off already.
        Some(
            self.output.lead_width(if_line, self.options.tab_width)
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width,
        )
    }

    pub(crate) fn apply_final_recovery_floor_and_replay_layout(
        &mut self,
        line: &str,
        replay: &LineReplayLayout,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_kind = layout.line_kind;
        let mut indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '(', ')', '{', '}'])
            && self
                .argument_after_lambda_call_argument_indent_spaces(line)
                .is_none()
            && !self.has_over_max_new_call_context()
            && let Some(spaces) = replay.closed_delimiter_continuation_indent
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let previous_code =
                        previous[..trailing_comment_split_limit(previous)].trim_end();
                    previous_code.ends_with(',')
                        && line_paren_imbalance(previous_code).0 > 0
                        && !(self.options.indent_after_parens
                            && previous_code
                                .split(|ch: char| !is_identifier_continue(ch))
                                .any(|word| word == "new"))
                })
        {
            exact_indent_spaces = Some(
                spaces
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if let Some(spaces) = self.maximum_length_new_call_argument_indent_spaces() {
            exact_indent_spaces = Some(spaces);
        }
        if self.options.indent_after_parens
            && let Some(previous) = self.output.last_line_outside_comment()
            && previous[..trailing_comment_split_limit(previous)]
                .trim_end()
                .ends_with(',')
            && previous
                .split(|ch: char| !is_identifier_continue(ch))
                .any(|word| word == "new")
        {
            exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        if let Some(spaces) =
            self.maximum_length_capped_open_paren_argument_indent_spaces(line_kind)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.maximum_length_logical_header_indent_spaces(line, line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && self.output.last_line_outside_comment().is_some()
            && let Some(spaces) = self.trailing_stream_top_level_indent_spaces(line_kind)
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.maximum_length_return_chain_indent_spaces(line, line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.constructor_lambda_header_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.lambda_closing_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.lambda_body_indent_spaces_after_opening_brace(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.inline_body_owner_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.lisp_attached_suffix_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.header_operator_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = replay.lambda_parameter_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(minimum) = self.split_else_exact_tab_indent_level(exact_indent_spaces) {
            indent = indent.max(minimum);
        }
        if self.options.indent_style != IndentStyle::Spaces
            && let Some(base) = self.constructor_initializer_base_indent_spaces()
            && exact_indent_spaces.is_some_and(|spaces| spaces >= base)
            && base.is_multiple_of(self.options.indent_width)
        {
            indent = indent.max(base / self.options.indent_width);
        }
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['{', '}'])
            && !starts_header_word(line.trim_start(), "switch")
            && let Some(spaces) = self.direct_switch_body_indent_spaces()
        {
            let current = exact_indent_spaces.unwrap_or(indent * self.options.indent_width);
            if current < spaces {
                exact_indent_spaces = Some(spaces);
            }
        }
        if let Some(spaces) = self.restored_preprocessor_branch_body_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.objc_line_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        layout.indent = indent;
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }
}
