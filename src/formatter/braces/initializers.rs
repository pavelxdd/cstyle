use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::formatter::braces::classification::is_lambda_capture_header;
use crate::formatter::braces::compound_literals::line_ends_compound_literal_cast;
use crate::formatter::braces::postprocess::horstmann_run_in_fill;
use crate::formatter::constructs::headers::is_braceless_header_line;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::index_hash::IndexSet;
use crate::formatter::lexer::{Token, next_non_whitespace};
use crate::formatter::output::buffer::OutputBuffer;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::frame::{BraceSemanticKind, ParenRole};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, preprocessor_directive, trailing_comment_split_limit,
    unmatched_open_brace_content_offset,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;

pub(crate) struct CompoundLiteralOpeningLayout {
    pub(crate) line_indent_spaces: usize,
    pub(crate) brace_indent_spaces: usize,
}

fn line_opens_typed_initializer(line: &str) -> bool {
    let code = line[..trailing_comment_split_limit(line)].trimmed_end();
    let Some(open) = code.rfind('{') else {
        return false;
    };
    let before = code[..open].trimmed_end();
    before.contains('<') && before.ends_with('>')
}

/// Whether `line`, a declaration up to its `=`, declares a `struct`, whose
/// aggregate astyle closes at the statement rather than under its brace.
fn line_declares_struct(line: &str) -> bool {
    let head = line.trimmed_end().trim_end_matches('=');
    let head = head.split(['[', '(']).next().unwrap_or(head);
    head.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .any(|word| word == "struct")
}

pub(crate) fn has_nested_designated_init_brace(tokens: &[Token]) -> bool {
    let mut saw_open = false;
    let mut saw_designator = false;
    for token in tokens {
        match token {
            Token::Whitespace(_) | Token::Newline => {}
            Token::Symbol('{') if saw_designator => return true,
            Token::Symbol('{') => saw_open = true,
            Token::Symbol('.') if saw_open => saw_designator = true,
            _ => {}
        }
    }
    false
}

pub(crate) fn bracket_starts_initializer_designator(
    tokens: &[Token],
    start: usize,
    end: usize,
) -> bool {
    bracket_starts_initializer_designator_by(tokens, start, end, |open| {
        matching_close_bracket_on_line(tokens, open, end)
    })
}

/// [`bracket_starts_initializer_designator`] with the `]` closing each `[`
/// before `end` found by `close_of`.
pub(crate) fn bracket_starts_initializer_designator_by(
    tokens: &[Token],
    start: usize,
    end: usize,
    close_of: impl Fn(usize) -> Option<usize>,
) -> bool {
    bracket_chain_starts_initializer_designator(tokens, start, end, close_of, |_| {})
}

/// [`bracket_starts_initializer_designator_by`], telling `visit` each `[` of
/// the chain read: every one of them has the same answer.
pub(crate) fn bracket_chain_starts_initializer_designator(
    tokens: &[Token],
    start: usize,
    end: usize,
    close_of: impl Fn(usize) -> Option<usize>,
    mut visit: impl FnMut(usize),
) -> bool {
    if !matches!(tokens.get(start), Some(Token::Symbol('['))) {
        return false;
    }
    let mut open = start;
    loop {
        visit(open);
        let Some(close) = close_of(open) else {
            return false;
        };
        let Some(next) = next_non_whitespace(tokens, close + 1, end) else {
            return false;
        };
        match tokens.get(next) {
            Some(Token::Symbol('[')) => open = next,
            Some(Token::Operator(operator)) if operator == "=" => return true,
            _ => return false,
        }
    }
}

fn matching_close_bracket_on_line(tokens: &[Token], open: usize, end: usize) -> Option<usize> {
    if !matches!(tokens.get(open), Some(Token::Symbol('['))) {
        return None;
    }
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(end).skip(open) {
        match token {
            Token::Symbol('[') => depth += 1,
            Token::Symbol(']') => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

impl FormatEngine<'_> {
    pub(crate) fn clear_macro_interrupted_initializer_frames(&mut self) {
        while self
            .inline_array
            .frames
            .last()
            .and_then(|frame| self.output.get(frame.output_line))
            .is_some_and(|line| {
                has_hash_outside_literals(line) && !line.trimmed_start().starts_with('#')
            })
        {
            self.inline_array.frames.pop();
            if matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral)
            ) {
                self.exit_brace_state();
            }
        }
    }

    pub(crate) fn designated_initializer_source_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.min_conditional_indent != MinConditionalIndent::Zero {
            return None;
        }
        let trimmed = line.trimmed_start();
        let previous = self.output.last_line_outside_comment();
        if !(self.in_initializer_brace()
            || self.in_aggregate_declaration_brace()
            || (trimmed.starts_with('[')
                && previous.is_some_and(|line| line.trimmed_start().starts_with('['))))
        {
            return None;
        }
        if !trimmed.starts_with('[')
            && !((trimmed.starts_with('.') || trimmed.starts_with("},"))
                && self.output.designator_since_closed_row())
        {
            return None;
        }
        // A closer the layout broke off a row has no source indent of its own.
        if trimmed.starts_with('}') && !self.pending_line_starts_source_line() {
            return None;
        }
        if trimmed.starts_with('[')
            && self.token_input.input_source_indent == 0
            && let Some(previous) = previous
            && previous.trimmed_start().starts_with('[')
            && previous.trimmed_end().ends_with(',')
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        Some(self.token_input.input_source_indent)
    }

    /// Whether the first token of the line being laid out began a line of
    /// the source.
    fn pending_line_starts_source_line(&self) -> bool {
        self.output.pending_tokens().is_some_and(|span| {
            self.tree.tokens[..span.first]
                .iter()
                .rev()
                .take_while(|token| !matches!(token, Token::Newline))
                .all(|token| matches!(token, Token::Whitespace(_)))
        })
    }

    pub(crate) fn range_designator_source_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if self.options.min_conditional_indent != MinConditionalIndent::Zero
            || !line_start.starts_with('[')
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !previous.trimmed_start().starts_with('[') {
            return None;
        }
        if self.token_input.input_source_indent == 0 && line_start.contains("...") {
            Some(leading_visual_width(previous, self.options.tab_width))
        } else {
            Some(self.token_input.input_source_indent)
        }
    }

    pub(crate) fn designated_initializer_source_indent_floor(
        &self,
        line: &LineView<'_>,
        kind: LineKind,
        current: Option<usize>,
    ) -> Option<usize> {
        if kind != LineKind::Normal
            || !line.trimmed_start().starts_with('.')
            || !(self.in_initializer_brace()
                || self.output_has_open_initializer_brace()
                || self.current_inline_array_column().is_some())
        {
            return None;
        }
        let spaces = self.layout.indentation.indent() * self.options.indent_width;
        (self.token_input.input_source_indent >= spaces)
            .then(|| current.map_or(spaces, |value| value.max(spaces)))
    }

    pub(crate) fn recent_double_brace_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        let opening = self
            .output
            .scoped()
            .iter()
            .rev()
            .take(4)
            .find(|previous| previous.trimmed_end().ends_with("{{"))?;
        let opening_indent = leading_visual_width(opening, self.options.tab_width);
        if !line.trimmed_start().starts_with_any(b"{}") {
            Some(opening_indent + self.options.indent_width * 2)
        } else if line.trimmed() == "}" {
            Some(opening_indent + self.options.indent_width)
        } else {
            None
        }
    }

    pub(crate) fn closed_initializer_or_array_indent_spaces(
        &self,
        line: &LineView<'_>,
        indent: usize,
        normal_indent: usize,
    ) -> Option<usize> {
        if !matches!(
            self.layout.nesting.last_closed_brace_type,
            Some(BraceType::Array | BraceType::CompoundLiteral | BraceType::Initializer)
        ) {
            return None;
        }
        if line.trimmed_start().starts_with("}, ") {
            return Some(indent.max(self.continuation_base_indent()) * self.options.indent_width);
        }
        if line.trimmed() != "}"
            || !self
                .output
                .last_non_empty_scoped()
                .is_some_and(|previous| self.output.code_trimmed_of(previous).ends_with(')'))
        {
            return None;
        }
        Some((normal_indent * self.options.indent_width).max(
            self.continuation_base_indent() * self.options.indent_width + self.options.indent_width,
        ))
    }

    pub(crate) fn initializer_or_array_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "{"
            || !matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            )
        {
            return None;
        }
        // A compound literal's element brace stands at its elements.
        if self.innermost_brace_is_compound_literal()
            && self.current_inline_array_column().is_some()
        {
            return None;
        }
        // VTK indents only initializer braces nested in a block.
        if self.options.brace_style == BraceStyle::Vtk
            && self
                .output
                .pending_tokens()
                .and_then(|span| self.tree.groups.opened_at(span.first))
                .is_some_and(|group| self.tree.groups.get(group).parent.is_none())
        {
            return None;
        }
        let brace = self.layout.frame_stack.active_brace().filter(|frame| {
            matches!(
                frame.semantic_kind,
                BraceSemanticKind::Array | BraceSemanticKind::Initializer
            )
        })?;
        if let Some(delimiter) = self
            .layout
            .frame_stack
            .active_delimiter()
            .filter(|frame| frame.role == ParenRole::CastOrGroup)
        {
            return Some(delimiter.opener_output_column);
        }
        if self.layout.frame_stack.active_delimiter().is_some() {
            return None;
        }
        // The style already indented the `}` of a struct the declarator
        // follows.
        if let Some(previous) = self.output.last_line_outside_comment()
            && previous.trimmed_start().starts_with('}')
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        Some(brace.body_indent_column)
    }

    pub(crate) fn initializer_or_array_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "}" {
            return None;
        }
        let (open_spaces, _, open) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if open.starts_with("},{") {
            return Some(open_spaces);
        }
        if !open.ends_with("{{") {
            return None;
        }
        if !open.trimmed_start().starts_with('.') {
            return Some(open_spaces + self.options.indent_width);
        }
        let previous_ends_comma = self
            .output
            .last_non_empty_scoped()
            .is_some_and(|previous| self.output.code_trimmed_of(previous).ends_with(','));
        Some(open_spaces + usize::from(previous_ends_comma) * self.options.indent_width)
    }

    pub(crate) fn compound_literal_opening_layout(
        &self,
        line: &LineView<'_>,
        normal_indent: usize,
        indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> Option<CompoundLiteralOpeningLayout> {
        if self
            .layout
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Command)
            || !line
                .trimmed_end()
                .strip_suffix('{')
                .is_some_and(|prefix| line_ends_compound_literal_cast(prefix.trimmed_end()))
            || self
                .output
                .pending_tokens()
                .is_some_and(|span| self.multiline_literal_argument_parens(span.first).is_some())
        {
            return None;
        }
        let normal_spaces = normal_indent * self.options.indent_width;
        let line_indent_spaces =
            if self.in_initializer_brace() || self.in_aggregate_declaration_brace() {
                if line.trimmed_start().starts_with('.') {
                    let limit = self.token_input.input_source_indent.max(normal_spaces);
                    // Only a field of the same initializer sets the column.
                    self.output
                        .scoped()
                        .iter()
                        .rev()
                        .map_while(|line| {
                            let code = self.output.code_of(line).trimmed();
                            let field = code.starts_with('.');
                            (field || !(code.ends_with(';') || code.ends_with('{')))
                                .then_some((line, field))
                        })
                        .find(|&(line, field)| {
                            field && leading_visual_width(line, self.options.tab_width) <= limit
                        })
                        .map(|(previous, _)| leading_visual_width(previous, self.options.tab_width))
                        .unwrap_or_else(|| exact_indent_spaces.unwrap_or(normal_spaces))
                } else {
                    self.output
                        .last_non_empty_scoped()
                        .and_then(|previous| {
                            let code = self.output.code_trimmed_of(previous);
                            code.ends_with('{').then(|| {
                                leading_visual_width(previous, self.options.tab_width)
                                    + self.options.indent_width
                            })
                        })
                        .unwrap_or(normal_spaces)
                }
            } else if self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let code = self.output.code_trimmed_of(previous);
                    is_braceless_header_line(code.trimmed_start())
                })
            {
                exact_indent_spaces.unwrap_or(indent * self.options.indent_width)
            } else {
                normal_spaces
            };
        let brace_indent_spaces = line_indent_spaces;
        Some(CompoundLiteralOpeningLayout {
            line_indent_spaces,
            brace_indent_spaces,
        })
    }

    pub(super) fn open_expanded_init_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: BraceType,
        block_indent_extra: usize,
    ) {
        self.emit_source_space_or_ensure();
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        self.finish_line();
        self.layout
            .nesting
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.layout
            .indentation
            .enter_block_with_extra(false, block_indent_extra);
        self.layout.previous = PreviousToken::Other;
    }

    pub(super) fn open_multiline_attached_initializer_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: BraceType,
        block_indent_extra: usize,
        force_break_one_line: bool,
    ) {
        let control_paren_indent = self.control_paren_init_brace_indent_spaces();
        let brace_begins_line = self.current_is_blank();
        let enclosing_body_column = self.current_inline_array_column();
        let indented_initializer_brace = brace_begins_line
            && control_paren_indent.is_none()
            && enclosing_body_column.is_some()
            && matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            );
        let opening_indent = control_paren_indent
            .or(enclosing_body_column)
            .unwrap_or_else(|| self.current_line_indent_spaces())
            + usize::from(indented_initializer_brace) * self.options.indent_width;
        let body_indent = if indented_initializer_brace {
            opening_indent
        } else {
            opening_indent + self.options.indent_width
        };

        self.emit_opening_brace_space(brace_type);
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        if brace_begins_line {
            self.layout
                .continuation_indent
                .set_next_line_spaces(opening_indent);
        }
        self.finish_line();
        self.update_current_brace_indent_from_last_output_line();
        self.layout
            .nesting
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.layout.indentation.enter_block_without_indent(false);
        if force_break_one_line {
            self.layout
                .compound_literal
                .forced_break_depths
                .push(self.layout.nesting.brace_header_stack.len());
        }
        let brace_column = if self.options.brace_style == BraceStyle::Ratliff
            && brace_type == BraceType::CompoundLiteral
        {
            opening_indent + self.options.indent_width
        } else {
            opening_indent
        };
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.layout.nesting.brace_header_stack.len(),
            body_column: body_indent,
            brace_column,
            output_line: self.output.len().saturating_sub(1),
            aggregate_assignment: control_paren_indent.is_some(),
            leveled_literal: false,
        });
        self.layout
            .continuation_indent
            .set_next_line_spaces(body_indent);
        self.layout.previous = PreviousToken::Other;
    }

    pub(super) fn open_attached_range_for_init_brace(
        &mut self,
        brace_header: Option<String>,
        block_indent_extra: usize,
    ) {
        let opening_indent = self
            .for_header_continuation_indent_spaces()
            .unwrap_or_else(|| self.current_line_indent_spaces() + self.options.indent_width * 2);
        self.emit_source_space_or_ensure();
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        self.finish_line();
        self.layout
            .nesting
            .enter_brace(brace_header, BraceType::Array, block_indent_extra);
        self.layout.indentation.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.layout.nesting.brace_header_stack.len(),
            body_column: opening_indent + self.options.indent_width,
            brace_column: opening_indent,
            output_line: self.output.len().saturating_sub(1),
            aggregate_assignment: false,
            leveled_literal: false,
        });
        self.update_current_brace_indent_columns(
            opening_indent + self.options.indent_width,
            opening_indent,
        );
        self.layout
            .continuation_indent
            .set_next_line_spaces(opening_indent + self.options.indent_width);
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = true;
    }

    pub(super) fn open_range_for_init_brace(
        &mut self,
        _brace_header: Option<String>,
        _brace_type: BraceType,
        _block_indent_extra: usize,
    ) {
        let opening_indent = self
            .for_header_continuation_indent_spaces()
            .unwrap_or_else(|| self.current_line_indent_spaces() + self.options.indent_width * 2);
        self.finish_line();
        self.layout
            .continuation_indent
            .set_next_line_spaces(opening_indent);
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        self.layout.nesting.enter_brace(None, BraceType::Array, 0);
        self.layout.indentation.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.layout.nesting.brace_header_stack.len(),
            body_column: opening_indent + 1,
            brace_column: opening_indent,
            output_line: self.output.len(),
            aggregate_assignment: true,
            leveled_literal: false,
        });
        self.update_current_brace_indent_columns(opening_indent + 1, opening_indent);
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn open_inline_array_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: BraceType,
        block_indent_extra: usize,
        token_index: usize,
        first_is_brace: bool,
    ) {
        // An attaching style that breaks enum braces, as Mozilla, breaks
        // one even when its first value runs in.
        let broken_enum_brace = brace_type == BraceType::Enum
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && !self.options.attach_enum
            && !self.current_is_blank();
        if broken_enum_brace {
            let indent = self.layout.indentation.indent();
            self.finish_line();
            self.layout.continuation_indent.set_next_line_level(indent);
        }
        let enclosed = brace_type == BraceType::Array
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(
                    BraceType::Array
                        | BraceType::Initializer
                        | BraceType::CompoundLiteral
                        | BraceType::Enum
                )
            );
        let double_brace_initializer =
            matches!(brace_type, BraceType::Array | BraceType::Initializer)
                && self.current.trimmed_end().ends_with('{');
        let run_in_after_comma = enclosed
            && !self.token_input.token_begins_source_line
            && self.current.trimmed_end().ends_with(',');
        let range_for_header_initializer =
            double_brace_initializer && self.current.trimmed_start().starts_with("for (");
        let break_first = (enclosed || double_brace_initializer)
            && !first_is_brace
            && !run_in_after_comma
            && !range_for_header_initializer;
        let nested = enclosed
            || double_brace_initializer
            || (brace_type == BraceType::Array
                && self.inline_array.nested_brace_arrays.contains(&token_index));
        let constructor_indent = self
            .layout
            .frame_stack
            .active_constructor_initializer()
            .map(|frame| frame.colon_line_indent_spaces);
        let base_indent = if constructor_indent.is_some() && self.layout.nesting.paren_depth == 0 {
            if self.current.trimmed_start().starts_with_any(b":,") {
                constructor_indent.unwrap_or_else(|| self.current_line_indent_spaces())
            } else {
                self.constructor_initializer_base_indent_spaces()
                    .unwrap_or_else(|| self.current_line_indent_spaces())
            }
        } else {
            self.statement_line_indent_spaces()
                .max(constructor_indent.unwrap_or(0))
        };
        // A compound literal outside parentheses is a value as an assigned
        // aggregate is.
        let aggregate_assign = self.current.trimmed_end().ends_with('=')
            || brace_type == BraceType::CompoundLiteral && self.layout.nesting.paren_depth == 0;
        let struct_declaration = !nested
            && self.current.trimmed_end().ends_with('=')
            && line_declares_struct(&self.current);
        if self.current_is_lambda_body_header()
            || is_lambda_capture_header(self.current.trimmed_end())
        {
            self.emit_source_space_or_ensure();
        } else {
            match self.current.trimmed_end().chars().next_back() {
                Some('[') => self.emit_source_space_or_ensure(),
                Some('(') if self.options.pad_parens_inside => {
                    self.pad_inside_paren_space();
                }
                Some('(') => self.emit_source_space(),
                Some('{') if self.current.ends_with_any(b" \t") => {}
                Some('@') => self.emit_source_space_or_ensure(),
                _ if brace_type == BraceType::Initializer
                    && self.current.trimmed_end().ends_with('>') =>
                {
                    self.emit_source_space_or_ensure();
                }
                _ if aggregate_assign || brace_type == BraceType::CompoundLiteral => {
                    self.emit_source_space_or_ensure();
                }
                _ => self.emit_source_space(),
            }
        }
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        // Parens that indent after them make an enum body run in after its
        // brace a block level past its line.
        let enum_block_level = self.options.indent_after_parens
            && brace_type == BraceType::Enum
            && !nested
            && !break_first;
        let brace_column = if enum_block_level {
            base_indent
        } else if nested {
            if run_in_after_comma && !first_is_brace {
                base_indent + self.options.indent_width
            } else {
                base_indent
            }
        } else {
            base_indent + self.current_char_len() - 1
        };
        if !break_first {
            self.emit_trailing_source_space();
        }
        let mut column = if enum_block_level {
            base_indent + self.options.indent_width
        } else if nested {
            if double_brace_initializer {
                base_indent + self.options.indent_width * 2
            } else {
                base_indent + self.options.indent_width
            }
        } else {
            // Padding parens outside parts a first `(` from the brace.
            let padded_paren = (self.options.pad_parens_outside
                || self.options.pad_first_paren_outside)
                && !self.current.ends_with_any(b" \t")
                && matches!(
                    self.tree.tokens.get(token_index + 1),
                    Some(Token::Symbol('('))
                );
            // The elements align to the brace's tab-expanded column; its
            // closer counts each tab as one column.
            base_indent + self.current_visual_width_from(base_indent) + usize::from(padded_paren)
        };
        // A compound literal stands the rows after its brace's line a level
        // past its statement, where its `}` closes. One among a call's
        // arguments keeps its rows with them.
        let leveled_literal = brace_type == BraceType::CompoundLiteral
            && !nested
            && !break_first
            && self.layout.nesting.paren_depth == 0;
        if leveled_literal {
            column = base_indent + self.options.indent_width;
        }
        let statement_base = ContinuationIndent::Level(
            self.layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal),
        )
        .columns(self.options.indent_width);
        if brace_type == BraceType::Initializer
            && line_opens_typed_initializer(&self.current)
            && column.saturating_sub(statement_base) > self.options.max_continuation_indent
        {
            column = base_indent + self.options.indent_width * 2;
        }
        let current_trimmed = self.current.trimmed_start();
        let run_in_nested_brace =
            current_trimmed.starts_with("{{") || self.current.contains_from_first_byte("{ {");
        let stored_brace_column = if broken_enum_brace {
            brace_column
        } else if self.current.trimmed() == "{" {
            column
        } else if run_in_nested_brace {
            base_indent + self.options.indent_width
        } else if struct_declaration {
            base_indent
        } else if leveled_literal {
            // Ratliff closes it at its rows.
            base_indent
                + usize::from(self.options.brace_style == BraceStyle::Ratliff)
                    * self.options.indent_width
        } else {
            brace_column
        };
        self.layout
            .nesting
            .enter_brace(brace_header, brace_type, block_indent_extra);
        self.layout.indentation.enter_block_without_indent(false);
        self.inline_array.frames.push(InlineArrayFrame {
            depth: self.layout.nesting.brace_header_stack.len(),
            body_column: column,
            brace_column: stored_brace_column,
            output_line: self.output.len(),
            aggregate_assignment: aggregate_assign,
            leveled_literal,
        });
        if self.current.trimmed() == "{" {
            self.update_current_brace_indent_columns(column + self.options.indent_width, column);
        } else if run_in_nested_brace {
            self.update_current_brace_indent_columns(
                base_indent + self.options.indent_width * 2,
                base_indent + self.options.indent_width,
            );
        } else {
            self.update_current_brace_indent_columns(
                base_indent + self.options.indent_width,
                base_indent,
            );
        }
        if break_first {
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(column);
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn close_inline_array_brace(&mut self) {
        // An enum's last row, which its `}` breaks off below, stands among
        // the elements.
        let broken_enum_row = !self.current_is_blank()
            && self.layout.nesting.brace_type_stack.last() == Some(&BraceType::Enum)
            && self.inline_array.frames.last().is_some_and(|frame| {
                frame.depth == self.layout.nesting.brace_header_stack.len()
                    && self.output.len() > frame.output_line
            });
        // So does the last row of a compound literal whose rows stand a
        // level past its statement: its `}` breaks off below.
        let broken_literal_row = !self.current_is_blank()
            && self.inline_array.frames.last().is_some_and(|frame| {
                frame.leveled_literal
                    && frame.depth == self.layout.nesting.brace_header_stack.len()
                    && self.output.len() > frame.output_line
            });
        if self.current_is_blank() {
            self.layout.frame_stack.clear_closed_braces();
        } else if self.token_input.token_begins_source_line
            && self.innermost_brace_is_compound_literal()
            || broken_enum_row
            || broken_literal_row
        {
            // The row before a `}` that leads its line stands among the
            // elements, so it publishes while the brace is open.
            self.finish_line();
        }
        let closing_brace_type = self.layout.nesting.brace_type_stack.last().copied();
        let closing_compound_literal =
            matches!(closing_brace_type, Some(BraceType::CompoundLiteral));
        let closing_enum = matches!(closing_brace_type, Some(BraceType::Enum));
        let inline_array = self.inline_array.frames.pop();
        let body_column = inline_array.map(|frame| frame.body_column);
        let in_constructor_initializer = self
            .layout
            .frame_stack
            .active_constructor_initializer()
            .is_some();
        self.inline_array.current_closed_body_column =
            body_column.map(|column| (column, in_constructor_initializer));
        let brace_column = inline_array.map(|frame| frame.brace_column);
        let open_output_len = inline_array
            .map(|frame| frame.output_line)
            .unwrap_or(self.output.len());
        let aggregate_assign = inline_array.is_some_and(|frame| frame.aggregate_assignment);
        let objc_dictionary = self
            .output
            .get(open_output_len)
            .is_some_and(|line| line.contains_from_first_byte("@ {"));
        let return_initializer = self
            .output
            .get(open_output_len)
            .is_some_and(|line| line.trimmed_start().starts_with("return "));
        // A returned `{{` keeps its closers on the last row.
        let enclosed_run_in = self.output.get(open_output_len).is_some_and(|line| {
            line.contains_from_first_byte("{ {")
                || !return_initializer && line.contains_from_first_byte("{{")
        });
        let range_for_initializer = self.output.get(open_output_len).is_some_and(|line| {
            let trimmed = line.trimmed_start();
            trimmed.starts_with("for ") && trimmed.trimmed_end().ends_with('{')
        });
        // A compound literal its line opened closes on a line of its own.
        let closing_line_opened_literal = closing_compound_literal
            && (inline_array.is_some_and(|frame| frame.leveled_literal)
                || self
                    .output
                    .get(open_output_len)
                    .is_some_and(|line| self.output.code_trimmed_of(line).ends_with('{')));
        let call_argument_array_column = self.output.get(open_output_len).and_then(|line| {
            line.rfind(", {")
                .map(|comma| visual_width_from(&line[..comma + 2], 0, self.options.tab_width))
        });
        let call_argument_array = call_argument_array_column.is_some();
        let typed_initializer = matches!(closing_brace_type, Some(BraceType::Initializer))
            && self
                .output
                .get(open_output_len)
                .is_some_and(|line| line_opens_typed_initializer(line));
        // A style indenting braces indents the `}` of a compound literal
        // that its line opened.
        let closing_brace_indent = if closing_line_opened_literal
            && self.options.brace_style != BraceStyle::Ratliff
            && self.should_indent_brace_line(BraceType::Array)
        {
            self.options.indent_width
        } else {
            0
        };
        let closing_column = call_argument_array_column
            .or(brace_column)
            .map(|column| column + closing_brace_indent);
        let forced_break = self
            .layout
            .compound_literal
            .forced_break_depths
            .last()
            .is_some_and(|depth| *depth == self.layout.nesting.brace_header_stack.len());
        if forced_break {
            self.layout.compound_literal.forced_break_depths.pop();
        }
        // An inner closer the line holds stands among the rows of the brace
        // it closes into.
        let finished_inner_closer = !forced_break
            && !broken_enum_row
            && !self.token_input.token_begins_source_line
            && self.output.len() > open_output_len
            && self.current.trimmed_start().starts_with('}')
            && (closing_line_opened_literal
                || aggregate_assign
                || objc_dictionary
                || closing_enum
                || enclosed_run_in
                || range_for_initializer
                || call_argument_array
                || typed_initializer);
        if finished_inner_closer {
            self.finish_line();
        }
        self.exit_brace_state();
        if let Some(frame) = self.layout.frame_stack.last_closed_brace_mut() {
            if in_constructor_initializer && let Some(column) = body_column {
                frame.body_indent_column = column;
            }
            if let Some(column) = closing_column {
                frame.sibling_indent_column = column;
            }
        }
        if forced_break {
            if !self.current_is_blank() {
                self.finish_line();
            }
            if let Some(column) = closing_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
            self.trim_current_end();
            self.mark_closed_brace_output_position();
            self.current.push('}');
            self.layout.command_state.observe_char('}');
            self.layout.compound_literal.just_closed = closing_compound_literal;
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        let parameterized_lambda_initializer_close = self.current_is_blank()
            && self.output.last().is_some_and(|line| line.trimmed() == "}")
            && self.output.get(open_output_len).is_some_and(|line| {
                let code = self.output.code_trimmed_of(line);
                code.contains_from_first_byte("](") || code.contains_from_first_byte("] (")
            });
        if parameterized_lambda_initializer_close {
            if let Some((previous, tokens)) = self.output.pop_with_tokens() {
                self.current.replace(previous);
                self.current.restore_tokens(tokens);
            }
        } else if self.token_input.token_begins_source_line {
            if !self.current_is_blank() {
                self.finish_line();
            }
            if let Some(column) = closing_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
        } else if broken_enum_row || broken_literal_row || finished_inner_closer {
            if let Some(column) = closing_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
        } else if self.output.len() > open_output_len
            && !self.current_is_blank()
            && (closing_line_opened_literal
                || aggregate_assign
                || objc_dictionary
                || closing_enum
                || enclosed_run_in
                || range_for_initializer
                || call_argument_array
                || typed_initializer)
        {
            self.finish_line();
            if let Some(column) = closing_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
        }
        let return_initializer_gap = return_initializer.then(|| {
            self.token_input
                .previous_input_whitespace
                .clone()
                .filter(|gap| !gap.is_empty() && gap.chars().all(|ch| ch == ' ' || ch == '\t'))
        });
        let source_closing_gap = (!closing_line_opened_literal
            && !aggregate_assign
            && !range_for_initializer
            && !call_argument_array)
            .then(|| {
                self.token_input
                    .previous_input_whitespace
                    .clone()
                    .filter(|gap| !gap.is_empty() && gap.chars().all(|ch| ch == ' ' || ch == '\t'))
            });
        if let Some(Some(gap)) = return_initializer_gap.or(source_closing_gap) {
            self.trim_current_end_horizontal_space();
            self.current.push_str(&gap);
        } else {
            self.trim_current_end_horizontal_space();
        }
        self.mark_closed_brace_output_position();
        self.current.push('}');
        self.layout.command_state.observe_char('}');
        self.layout.compound_literal.just_closed = closing_compound_literal;
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(crate) fn current_inline_array_column(&self) -> Option<usize> {
        self.inline_array
            .frames
            .last()
            .filter(|frame| frame.depth == self.layout.nesting.brace_header_stack.len())
            .map(|frame| frame.body_column)
    }

    /// Whether the line continues a brace whose first element shares its
    /// line, so it aligns to that element.
    pub(crate) fn continues_aligned_brace_elements(&self) -> bool {
        self.current_inline_array_column().is_some()
            || self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let code = self.output.code_trimmed_of(previous);
                    code.ends_with(',') && unmatched_open_brace_content_offset(code).is_some()
                })
    }

    pub(crate) fn active_initializer_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        closing: bool,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        let starts_member_opener =
            !closing && line_start.starts_with_any(b".[") && line.contains('{');
        let frame = if closing {
            let trimmed = line_start;
            let previous_closes_brace = self
                .output
                .last()
                .is_some_and(|line| line.trimmed_start().starts_with('}'));
            let closed = if previous_closes_brace
                || trimmed.starts_with("},")
                || trimmed.starts_with("};")
                || trimmed.starts_with("})")
            {
                self.layout.frame_stack.last_closed_brace()
            } else {
                self.layout
                    .frame_stack
                    .first_closed_brace()
                    .or_else(|| self.layout.frame_stack.last_closed_brace())
            };
            closed.or_else(|| self.layout.frame_stack.active_brace())?
        } else if starts_member_opener {
            let opened_braces = line.chars().filter(|ch| *ch == '{').count();
            self.layout
                .frame_stack
                .brace_before_top(opened_braces)
                .or_else(|| self.layout.frame_stack.enclosing_brace())
                .or_else(|| self.layout.frame_stack.active_brace())?
        } else {
            self.layout.frame_stack.active_brace()?
        };
        if !matches!(
            frame.semantic_kind,
            BraceSemanticKind::Array
                | BraceSemanticKind::CompoundLiteral
                | BraceSemanticKind::Initializer
        ) {
            return None;
        }
        Some(if closing {
            frame.sibling_indent_column
        } else {
            frame.body_indent_column
        })
    }

    pub(crate) fn initializer_member_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if !self.in_initializer_brace() {
            return None;
        }
        let trimmed = line.trimmed_start();
        let closing = trimmed.starts_with('}');
        let designator = trimmed.starts_with('.') || trimmed.starts_with('[');
        if !closing && !designator {
            return None;
        }
        if closing && let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces {
            return Some(spaces);
        }
        // A compound literal's fields stand at its elements.
        if designator
            && self.innermost_brace_is_compound_literal()
            && let Some(column) = self.current_inline_array_column()
        {
            return Some(column);
        }
        if !closing
            && let Some(previous) = self.output.last()
            && previous.trimmed_end().ends_with("},{")
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if !closing
            && designator
            && let Some(previous) = self.output.last_code_line_in_scope()
            && previous.trimmed_start().starts_with("},")
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.case_unindent_spaces(),
            );
        }
        if !closing
            && let Some(previous) = self.output.last()
            && (previous.contains_from_first_byte("{{") || previous.contains_from_first_byte("{ {"))
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2,
            );
        }
        if !closing
            && designator
            && let Some(previous) = self.output.last()
            && previous.trimmed_end().ends_with('{')
        {
            if previous.trimmed() == "{"
                && self
                    .layout
                    .frame_stack
                    .active_brace()
                    .is_some_and(|frame| self.should_indent_brace_line(frame.brace_type))
            {
                return Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.case_unindent_spaces(),
                );
            }
            // Ratliff closes the aggregate a declaration defines at its
            // body; the rows stand a level past the declaration.
            let lead = leading_visual_width(previous, self.options.tab_width);
            let lead = if self.options.brace_style == BraceStyle::Ratliff
                && previous.trimmed_start().starts_with('}')
            {
                lead.saturating_sub(self.options.indent_width)
            } else {
                lead
            };
            return Some(lead + self.options.indent_width + self.case_unindent_spaces());
        }
        // A style indenting braces closes a compound literal at its fields.
        if closing
            && self.layout.nesting.last_closed_brace_type == Some(BraceType::CompoundLiteral)
            && (self.options.brace_style == BraceStyle::Ratliff
                || self.should_indent_brace_line(BraceType::Array))
            && let Some(previous) = self.output.last()
            && previous.trimmed_start().starts_with('.')
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if closing
            && let Some(previous) = self.output.last()
            && previous.trimmed_start().starts_with('.')
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    .saturating_sub(self.options.indent_width)
                    + self.case_unindent_spaces(),
            );
        }
        if !closing
            && let Some(mut spaces) = self.active_initializer_brace_indent_spaces(line, closing)
        {
            if designator
                && let Some(previous) = self.output.last()
                && previous.trimmed_start().starts_with_any(b".[")
            {
                spaces = leading_visual_width(previous, self.options.tab_width);
            }
            if designator {
                if self.output_has_open_initializer_brace() || self.in_initializer_brace() {
                    spaces =
                        spaces.max(self.layout.indentation.indent() * self.options.indent_width);
                }
                if let Some(index) = self.row_initializer_opener_line() {
                    let previous = &self.output[index];
                    let code = self.output.code_before_comment(index).trimmed_end();
                    {
                        // An indented brace on its own line stands at its rows.
                        let brace_indent = if code.trimmed_start() == "{"
                            && self.layout.frame_stack.active_brace().is_some_and(|frame| {
                                self.should_indent_brace_line(frame.brace_type)
                            }) {
                            0
                        } else {
                            self.options.indent_width
                        };
                        spaces = spaces.max(
                            leading_visual_width(previous, self.options.tab_width)
                                + brace_indent
                                + self.case_unindent_spaces(),
                        );
                    }
                }
            }
            return Some(spaces);
        }
        if closing
            && !matches!(
                self.layout.nesting.last_closed_brace_type,
                Some(BraceType::Array | BraceType::CompoundLiteral | BraceType::Initializer)
            )
        {
            return None;
        }
        let mut depth = 0usize;
        for index in self.output.scoped_range().rev() {
            let previous = &self.output[index];
            // An indent holds no brace, and the braces a `{` follows end at
            // the line's start as at its indent.
            let mut chars = self.output.trimmed(index).chars().rev();
            while let Some(ch) = chars.next() {
                match ch {
                    '}' => depth += 1,
                    '{' if depth == 0 => {
                        let mut levels = 1usize;
                        for left in chars.by_ref() {
                            if left == '{' {
                                levels += 1;
                            } else if left.is_whitespace() {
                                continue;
                            } else {
                                break;
                            }
                        }
                        let prefix_len = leading_visual_width(previous, self.options.tab_width);
                        // Ratliff closes a group opened after code at its rows, as
                        // a style indenting braces closes a compound literal.
                        let closes_at_rows = (self.options.brace_style == BraceStyle::Ratliff
                            || self.layout.nesting.last_closed_brace_type
                                == Some(BraceType::CompoundLiteral)
                                && self.should_indent_brace_line(BraceType::Array))
                            && !previous.trimmed_start().starts_with_any(b"{}");
                        let inner_levels = levels - usize::from(closing && !closes_at_rows);
                        return Some(prefix_len + inner_levels * self.options.indent_width);
                    }
                    '{' => depth = depth.saturating_sub(1),
                    _ => {}
                }
            }
        }
        None
    }

    pub(crate) fn initializer_brace_continuation_anchor(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let head = line.trimmed_end().strip_suffix('{')?.trimmed_end();
        if !head.starts_with('=') || head.as_bytes().get(1) == Some(&b'=') {
            return None;
        }
        if self.layout.nesting.paren_depth > 0 {
            return None;
        }
        let frame = self.layout.frame_stack.active_brace()?;
        if !matches!(
            frame.semantic_kind,
            BraceSemanticKind::Array
                | BraceSemanticKind::Initializer
                | BraceSemanticKind::CompoundLiteral
        ) {
            return None;
        }
        Some(self.continuation_base_indent() * self.options.indent_width)
    }

    pub(crate) fn output_has_open_initializer_brace(&self) -> bool {
        let key = (self.output.len(), self.output.version());
        if let Some((cached, open)) = self.open_initializer_brace_cache.get()
            && cached == key
        {
            return open;
        }
        let open = self.scan_open_initializer_brace();
        self.open_initializer_brace_cache.set(Some((key, open)));
        open
    }

    fn scan_open_initializer_brace(&self) -> bool {
        let len = self.output.len();
        // The last of the last 16 lines that ends a statement or leaves a
        // brace open decides.
        self.output
            .last_line_looked(
                &self.brace_decision_look,
                len.saturating_sub(16),
                len,
                |index| {
                    let code = self.output.code(index);
                    let trimmed = code.trimmed();
                    !(trimmed.is_empty() || trimmed.starts_with('#'))
                        && (code.ends_with(';')
                            || trimmed == "{"
                            || trimmed == "}"
                            || self.output.code_has_unmatched_open_brace(index))
                },
            )
            .is_some_and(|index| {
                let code = self.output.code(index);
                let trimmed = code.trimmed();
                !(code.ends_with(';') || trimmed == "{" || trimmed == "}")
                    && (code.contains("({")
                        || code.contains("= {")
                        || code.contains_from_first_byte("{{"))
            })
    }

    pub(crate) fn preprocessor_branch_initializer_member_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed_start().starts_with('#')
            || !(self.current_inline_array_column().is_some()
                || self.in_initializer_brace()
                || self.in_aggregate_declaration_brace())
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !preprocessor_directive(previous.trimmed_start())
            .is_some_and(is_conditional_preprocessor)
        {
            return None;
        }
        let mut rows = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .filter(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            });
        let mut row = rows.next()?;
        if !self.output.code_trimmed_of(row).ends_with(',') {
            return None;
        }
        // A member split over rows stands at the row that opens it.
        let mut pending_closes = 0usize;
        loop {
            let (closes, opens) = self.paren_imbalance_of(self.output.code_of(row));
            pending_closes = (pending_closes + closes).saturating_sub(opens.len());
            if pending_closes == 0 {
                break;
            }
            row = rows.next()?;
        }
        // Members after a brace that opens mid-row stand where the first one
        // does.
        let code = self.output.code_of(row);
        if let Some(content) = unmatched_open_brace_content_offset(code) {
            return Some(visual_width_from(
                &code[..content],
                0,
                self.options.tab_width,
            ));
        }
        Some(leading_visual_width(row, self.options.tab_width))
    }

    pub(crate) fn split_else_initializer_closing_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if !split_else_context
            || case_unindent_spaces == 0
            || !line.trimmed_start().starts_with("};")
        {
            return None;
        }
        self.active_initializer_brace_indent_spaces(line, true)
            .map(|spaces| spaces + case_unindent_spaces)
    }

    pub(crate) fn split_else_commented_aggregate_member_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        split_else_extra_indent: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if !split_else_extra_indent
            || line_kind != LineKind::Normal
            || line.trimmed_start().starts_with_any(b"#{}")
            || case_unindent_spaces == 0
            || !self.in_aggregate_declaration_brace()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !previous_code.ends_with(';')
            || !(previous_code.len() < previous.trimmed_end().len() || previous_code.contains("/*"))
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + case_unindent_spaces)
    }

    pub(crate) fn aggregate_member_case_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: usize,
        normal_indent: usize,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if case_unindent_spaces == 0 || current_spaces > normal_indent * self.options.indent_width {
            return None;
        }
        // A sibling member or closing brace already placed in pre-unindent
        // columns stays.
        let reference = if line.trimmed_start().starts_with('}') {
            self.output
                .current_closing_brace_open(self.options.tab_width)
                .map(|(width, ..)| width)
        } else {
            self.output
                .last_line_outside_comment()
                .and_then(|previous| {
                    let code = self.output.code_of(previous).trimmed();
                    let width = leading_visual_width(previous, self.options.tab_width);
                    // A first member under a brace on its own line sits a
                    // level past it.
                    match code {
                        "{" => Some(width + self.options.indent_width),
                        _ if code.ends_with('{') => None,
                        _ => Some(width),
                    }
                })
        };
        if reference.is_some_and(|width| width + case_unindent_spaces == current_spaces) {
            return None;
        }
        let aggregate_member = self.in_aggregate_declaration_brace()
            || self.output.scoped_range().rev().take(16).any(|index| {
                let code = self.output.code_before_comment_trimmed(index);
                code.trimmed_start().starts_with("static const struct") && code.ends_with('{')
            });
        aggregate_member.then_some(current_spaces + case_unindent_spaces)
    }

    /// The line opening the initializer the next row stands in, looking
    /// back over rows no `;` or `}` ends; groups closed before the row hold
    /// no opener of it.
    fn row_initializer_opener_line(&self) -> Option<usize> {
        let len = self.output.len();
        let mut openers = self.row_openers.borrow_mut();
        openers.keep_unchanged(&self.output);
        let answers = &openers.answers;
        let mut known = answers.len();
        let mut depth = 0usize;
        let mut index = len;
        // The looks from the line counts this one passes at no depth before
        // any group end as this one does.
        let mut same_from = len;
        let mut at_no_depth = true;
        let mut statement_end = None;
        let found = loop {
            while known > 0 && answers[known - 1].0 as usize > index {
                known -= 1;
            }
            if depth == 0
                && let Some(&(from, to, found)) = known.checked_sub(1).map(|at| &answers[at])
                && to as usize >= index
            {
                if at_no_depth {
                    same_from = from as usize;
                }
                break found.map(|line| line as usize);
            }
            if at_no_depth {
                same_from = index;
            }
            let Some(line) = index.checked_sub(1) else {
                break None;
            };
            index = line;
            let code = self.output.code_before_comment(index).trimmed_end();
            if depth == 0 && code.ends_with('{') && self.output_line_opens_initializer(index, code)
            {
                break Some(index);
            }
            if code.ends_with(';') || code.ends_with('}') {
                statement_end = Some(index);
                break None;
            }
            let meta = self.output.brace_meta(index);
            depth = (depth + meta.closes()).saturating_sub(meta.opens());
            at_no_depth &= depth == 0;
        };
        openers.record(same_from..=len, found, statement_end);
        found
    }

    fn output_line_opens_initializer(&self, index: usize, code: &str) -> bool {
        let trimmed = code.trimmed();
        if code.contains("= {") || code.contains("({") || code.contains_from_first_byte("{{") {
            return true;
        }
        if trimmed.ends_with('{') {
            let head = trimmed.trim_end_matches('{').trimmed_end();
            if line_ends_compound_literal_cast(head) {
                return true;
            }
        }
        if trimmed != "{" {
            return false;
        }
        self.output[..index]
            .iter()
            .rev()
            .find(|line| !line.trimmed().is_empty())
            .is_some_and(|previous| {
                let previous = self.output.code_trimmed_of(previous);
                previous.contains('=') && previous.ends_with(')')
            })
    }
}

/// The answers of the looks back for the line opening an initializer's
/// rows, each for the line counts from its first to its second that a look
/// starts from, in order; each holds while the lines before its start stay
/// as the output `version` had them.
#[derive(Debug, Default)]
pub(crate) struct RowOpeners {
    version: u64,
    answers: Vec<(u32, u32, Option<u32>)>,
}

impl RowOpeners {
    fn keep_unchanged(&mut self, output: &OutputBuffer) {
        if self.version == output.version() {
            return;
        }
        match output.lowest_change_since(self.version) {
            Some(lowest) => self.keep_below(lowest.saturating_add(1)),
            None => self.answers.clear(),
        }
        self.version = output.version();
    }

    /// Keeps the answers for the line counts below `end`.
    fn keep_below(&mut self, end: usize) {
        let kept = self
            .answers
            .partition_point(|&(from, _, _)| (from as usize) < end);
        self.answers.truncate(kept);
        if let Some(last) = self.answers.last_mut()
            && last.1 as usize >= end
        {
            last.1 = line_count(end - 1);
        }
    }

    fn record(
        &mut self,
        starts: std::ops::RangeInclusive<usize>,
        found: Option<usize>,
        statement_end: Option<usize>,
    ) {
        if let Some(end) = statement_end {
            // Every look reaching the end of a statement stops there.
            let reached = self
                .answers
                .partition_point(|&(_, to, _)| to as usize <= end);
            self.answers.drain(..reached);
            if let Some(first) = self.answers.first_mut() {
                first.0 = first.0.max(line_count(end + 1));
            }
        }
        self.keep_below(*starts.start());
        self.answers.push((
            line_count(*starts.start()),
            line_count(*starts.end()),
            found.map(line_count),
        ));
    }
}

fn line_count(count: usize) -> u32 {
    u32::try_from(count).expect("the output holds under 4G lines")
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct InlineArrayFrame {
    pub(crate) depth: usize,
    pub(crate) body_column: usize,
    pub(crate) brace_column: usize,
    pub(crate) output_line: usize,
    pub(crate) aggregate_assignment: bool,
    /// A compound literal whose rows run in after its brace, which stands
    /// them a level past its statement and closes on a line of its own.
    pub(crate) leveled_literal: bool,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct InlineArrayState {
    pub(crate) initializer_designator_bracket_depth: usize,
    pub(crate) frames: Vec<InlineArrayFrame>,
    pub(crate) current_closed_body_column: Option<(usize, bool)>,
    pub(super) aggregate_braces: Vec<bool>,
    pub(crate) nested_brace_arrays: IndexSet<usize>,
}

pub(crate) fn initializer_brace_line_comment_gap(
    options: &FormatOptions,
    brace_line: &str,
) -> String {
    if options.brace_style != BraceStyle::Horstmann {
        return "   ".to_string();
    }
    let brace_column = leading_visual_width(brace_line, options.tab_width);
    let target = brace_column + options.indent_width;
    horstmann_run_in_fill(brace_line, &" ".repeat(target), options)
}
