use crate::config::BraceStyle;
use crate::formatter::constructs::headers::{
    is_attachable_closing_header, is_header, same_line_nested_header_extra, starts_header_word,
};
use crate::formatter::constructs::labels;
use crate::formatter::engine::FormatEngine;
use crate::formatter::output::model::{LineDelimiters, LineLayout, LineReplayLayout};
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{ContainsAnyByte, code_holds_word, preprocessor_directive};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::leading_identifier;

impl FormatEngine<'_> {
    pub(crate) fn apply_brace_header_case_and_initializer_correction_layout(
        &mut self,
        line: &LineView<'_>,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_start = line.trimmed_start();
        let line_kind = layout.line_kind;
        let indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        if self.options.indent_braces
            && matches!(
                self.options.brace_style,
                BraceStyle::None | BraceStyle::Allman
            )
            && line_kind == LineKind::Normal
            && !line_start.starts_with_any(b"#{}")
        {
            let previous_brace_indent =
                self.output
                    .last_line_outside_comment()
                    .and_then(|previous| {
                        let code = self.output.code_trimmed_of(previous);
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
            && line_start.starts_with(':')
            && !line_start.starts_with("::")
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
        ) && line_start.starts_with_any(b".[")
            && self.current_inline_array_column().is_none()
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
            && !line_start.starts_with_any(b"#{}/")
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trimmed() == "{")
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
            && !line_start.starts_with_any(b"#{}")
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trimmed() == "{")
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
                .is_some_and(|line| preprocessor_directive(line.trimmed_start()) == Some("endif"))
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
        if line.trimmed() == "{"
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        } else if line_kind == LineKind::Normal
            && !line_start.starts_with_any(b"#{}/")
            && !is_header(self.options, leading_identifier(line))
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trimmed() == "{")
            && let Some(spaces) = indented_command_body
        {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && is_header(self.options, leading_identifier(line))
            && same_line_nested_header_extra(line_start) == 0
            && !(self.options.no_indent_if_after_else
                && starts_header_word(line_start, "if")
                && self
                    .output
                    .last_non_empty_scoped()
                    .is_some_and(|previous| matches!(previous.trimmed(), "else" | "} else")))
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
        if line_start.starts_with("else")
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
            && line_start.starts_with(']')
            && let Some(spaces) = self.bracket_rows_closing_indent_spaces()
        {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && line_start.starts_with(']')
            && let Some(previous) = self.output.last_line_outside_comment()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if ["for", "while", "switch"].iter().any(|header| {
                let trimmed = previous_code.trimmed_start();
                trimmed == *header
                    || trimmed
                        .strip_prefix(header)
                        .is_some_and(|rest| rest.starts_with_any(b" \t"))
            }) && !previous_code.contains('(')
            {
                exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        if let Some((column, true)) = self.inline_array.current_closed_body_column.take()
            && line_kind == LineKind::Normal
            && !line_start.starts_with('}')
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
        line: &LineView<'_>,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_start = line.trimmed_start();
        let line_kind = layout.line_kind;
        let normal_indent = layout.normal_indent;
        let class_scope_label = layout.class_scope_label;
        let indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        // Read only by the rules whose own tests pass.
        let delimiters = std::cell::OnceCell::new();
        if let Some(spaces) = self.post_block_case_body_indent_override(
            line,
            line_kind,
            || self.line_delimiters(line, &delimiters),
            is_attachable_closing_header(leading_identifier(line)),
        ) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.nested_case_label_indent_override(line_kind) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) =
            self.active_label_block_indent_spaces(line, line_kind, indent == normal_indent, || {
                self.line_delimiters(line, &delimiters)
            })
        {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.closed_label_block_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(case_layout) = self.active_case_block_body_layout(
            line,
            line_kind,
            indent == normal_indent,
            || self.line_delimiters(line, &delimiters),
            exact_indent_spaces,
        ) {
            exact_indent_spaces = Some(case_layout.exact_indent_spaces);
        }
        // A broken `{` stands at the header that opened its frame.
        if matches!(
            self.options.brace_style,
            BraceStyle::Allman | BraceStyle::Horstmann | BraceStyle::Pico
        ) && !self.options.indent_braces
            && line.trimmed() == "{"
            && let Some(header) = self
                .layout
                .frame_stack
                .active_brace()
                .filter(|frame| {
                    frame.semantic_kind == BraceSemanticKind::Command && !frame.case_block
                })
                .and_then(|frame| frame.header.as_deref())
                .filter(|header| !matches!(*header, "case" | "default"))
            && let Some(previous) = self.output.last_line_outside_comment()
            && starts_header_word(previous.trimmed_start(), header)
            && same_line_nested_header_extra(previous) == 0
        {
            exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.case_unindent_spaces(),
            );
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
        }
        if let Some(spaces) = self.lambda_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.initializer_or_array_opening_brace_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if line_kind == LineKind::Normal
            && (line_start.starts_with("///") || line_start.starts_with("//!"))
            && self
                .layout
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Aggregate)
            && let Some(spaces) = self.active_body_comment_indent_spaces()
        {
            exact_indent_spaces = Some(spaces);
        }
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }
    /// The delimiters of `line`, read once into `cell`.
    fn line_delimiters(
        &self,
        line: &LineView<'_>,
        cell: &std::cell::OnceCell<LineDelimiters>,
    ) -> LineDelimiters {
        *cell.get_or_init(|| {
            let (closing_parens, opening_parens) = line.paren_imbalance();
            LineDelimiters {
                closes_outer: closing_parens > opening_parens.len(),
                owned_continuation: self.layout.frame_stack.active_delimiter().is_some()
                    || self.operator_chain_owns_continuation(line),
            }
        })
    }

    /// An `else` starting a line takes the indent of the line holding its
    /// `if`, from the structure tree.
    pub(crate) fn apply_final_recovery_floor_and_replay_layout(
        &self,
        line: &LineView<'_>,
        replay: &LineReplayLayout,
        mut layout: LineLayout,
    ) -> LineLayout {
        let line_start = line.trimmed_start();
        let line_kind = layout.line_kind;
        let indent = layout.indent;
        let mut exact_indent_spaces = layout.exact_indent_spaces;
        // A plain statement follows a code line ending with `;`, not `,`,
        // `(` or `{`, and starts with a word.
        let plain = self.plain_statement_line;
        if !plain
            && line_kind == LineKind::Normal
            && let Some(spaces) = replay.closed_delimiter_continuation_indent
            && !line_start.starts_with_any(b"#(){}")
            && self
                .argument_after_lambda_call_argument_indent_spaces(line)
                .is_none()
            && !self.has_over_max_new_call_context()
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let previous_code = self.output.code_trimmed_of(previous);
                    previous_code.ends_with(',')
                        && self.paren_closes_of(previous_code) > 0
                        && !(self.options.indent_after_parens
                            && code_holds_word(previous_code, "new"))
                })
        {
            exact_indent_spaces = Some(
                spaces
                    + self.layout.line_adjuster.total_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if !plain && let Some(spaces) = self.maximum_length_new_call_argument_indent_spaces() {
            exact_indent_spaces = Some(spaces);
        }
        if !plain
            && self.options.indent_after_parens
            && let Some(previous) = self.output.last_line_outside_comment()
            && self.output.code_trimmed_of(previous).ends_with(',')
            && code_holds_word(previous, "new")
        {
            exact_indent_spaces = Some(leading_visual_width(previous, self.options.tab_width));
        }
        if !plain {
            if let Some(spaces) =
                self.maximum_length_capped_open_paren_argument_indent_spaces(line_kind)
            {
                exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.maximum_length_logical_header_indent_spaces(line, line_kind)
            {
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
        }
        if let Some(spaces) = replay.constructor_lambda_header_indent_spaces {
            exact_indent_spaces = Some(spaces);
        }
        if !plain {
            if let Some(spaces) = self.lambda_closing_brace_indent_spaces(line) {
                exact_indent_spaces = Some(spaces);
            }
            if let Some(spaces) = self.lambda_body_indent_spaces_after_opening_brace(line) {
                exact_indent_spaces = Some(spaces);
            }
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
        if line_kind == LineKind::Normal
            && !line_start.starts_with_any(b"{}")
            && !starts_header_word(line_start, "switch")
            && let Some(spaces) = self.direct_switch_body_indent_spaces()
        {
            let current = exact_indent_spaces.unwrap_or(indent * self.options.indent_width);
            if current < spaces {
                exact_indent_spaces = Some(spaces);
            }
        }
        // A label after a pico closer, which runs into the row before, takes
        // no level: the row after it stands in the closed block.
        if self.options.brace_style == BraceStyle::Pico
            && line_kind == LineKind::Normal
            && !line_start.starts_with_any(b"{}#")
            && let Some(previous) = self.output.last_line_outside_comment()
            && self
                .output
                .code_trimmed_of(previous)
                .trimmed_start()
                .strip_prefix('}')
                .is_some_and(|after| {
                    let label = after.trimmed();
                    (starts_header_word(label, "case") || label.starts_with("default"))
                        && label.ends_with(':')
                })
            && let Some(frame) = self.layout.frame_stack.last_closed_brace()
        {
            exact_indent_spaces = Some(
                frame.body_indent_column
                    + self.layout.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        if let Some(spaces) = self.restored_preprocessor_branch_body_indent_spaces(line) {
            exact_indent_spaces = Some(spaces);
        }
        if let Some(spaces) = self.objc_line_indent_override(line) {
            exact_indent_spaces = Some(spaces);
        }
        layout.exact_indent_spaces = exact_indent_spaces;
        layout
    }

    /// The column of a `]` leading its line after rows its `[` opened at
    /// the end of a line: a level before the rows, never before that line.
    fn bracket_rows_closing_indent_spaces(&self) -> Option<usize> {
        let mut depth = 0usize;
        let mut first_row = None;
        for index in (0..self.output.len()).rev() {
            let code = self.output.code_before_comment(index).trimmed_end();
            for byte in code.bytes().rev() {
                match byte {
                    b']' => depth += 1,
                    b'[' if depth == 0 => {
                        if !code.ends_with('[') {
                            return None;
                        }
                        let opener = self.output.lead_width(index, self.options.tab_width);
                        let row = self.output.lead_width(first_row?, self.options.tab_width);
                        return Some(row.saturating_sub(self.options.indent_width).max(opener));
                    }
                    b'[' => depth -= 1,
                    b';' | b'{' | b'}' if depth == 0 => return None,
                    _ => {}
                }
            }
            if !code.trimmed().is_empty() {
                first_row = Some(index);
            }
        }
        None
    }
}
