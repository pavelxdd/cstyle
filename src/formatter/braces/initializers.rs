use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::formatter::braces::classification::is_lambda_capture_header;
use crate::formatter::braces::compound_literals::line_ends_compound_literal_cast;
use crate::formatter::braces::postprocess::horstmann_run_in_fill;
use crate::formatter::constructs::headers::is_braceless_header_line;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{CommentKind, Token, next_non_whitespace};
use crate::formatter::output::buffer::OutputBuffer;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::frame::{BraceSemanticKind, ParenRole};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::BraceListRows;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, has_unmatched_open_brace, line_ends_with_comment, preprocessor_directive,
    trailing_comment_split_limit, unmatched_open_brace_content_offset,
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

/// Whether the last mark before `brace` that is no part of a name, a
/// separator or a comment is an operator ending in `=`.
fn brace_follows_assignment(tokens: &[Token], brace: usize) -> bool {
    tokens[..brace]
        .iter()
        .rev()
        .find(|token| match token {
            Token::Word(_)
            | Token::Number(_)
            | Token::Whitespace(_)
            | Token::Newline
            | Token::Comment(..) => false,
            Token::Symbol(ch) => !matches!(ch, '.' | ',' | ';'),
            Token::Operator(operator) => operator != ".",
            _ => true,
        })
        .is_some_and(|token| matches!(token, Token::Operator(operator) if operator.ends_with('=')))
}

/// Whether the declaration the `=` before `brace` ends declares a `struct`
/// outside its parentheses and brackets, on any of its lines: astyle closes
/// its aggregate at the statement rather than under its brace.
fn statement_declares_struct(tokens: &[Token], brace: usize) -> bool {
    let mut depth = 0usize;
    for token in tokens[..brace].iter().rev() {
        match token {
            Token::Symbol(')' | ']') => depth += 1,
            Token::Symbol('(' | '[') if depth > 0 => depth -= 1,
            Token::Symbol(';' | '{' | '}' | '(' | '[') => return false,
            Token::Word(word) if depth == 0 && word == "struct" => return true,
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

    /// Whether the line being laid out starts among the elements of an
    /// initializer list.
    fn pending_line_starts_in_initializer(&self) -> bool {
        self.output
            .pending_tokens()
            .and_then(|span| self.tree.groups.enclosing(span.first))
            .is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
            })
    }

    pub(crate) fn range_designator_source_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if self.options.min_conditional_indent != MinConditionalIndent::Zero
            || !line_start.starts_with('[')
            || !self.pending_line_starts_in_initializer()
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

    /// Whether the pending line starts with the closer of a list that is an
    /// argument in parens: the closer stands where the list closes.
    pub(crate) fn line_closes_list_argument(&self) -> bool {
        self.output
            .pending_tokens()
            .and_then(|span| self.tree.groups.closed_at(span.first))
            .and_then(|group| self.tree.groups.get(group).parent)
            .is_some_and(|parent| self.tree.groups.get(parent).delimiter == Delimiter::Paren)
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
        // A comment after the closer leaves it the closer of its own row.
        let code = line[..trailing_comment_split_limit(line)].trimmed();
        if code.starts_with("}, ")
            && !self.line_closes_list_argument()
            && !self.line_closes_run_in_declarator_list()
        {
            return Some(indent.max(self.continuation_base_indent()) * self.options.indent_width);
        }
        if code != "}"
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

    /// Whether the pending line starts a row of a list nested in a list
    /// that a style running rows in after braces keeps on its brace line,
    /// where others break after the brace and level its rows.
    pub(crate) fn pending_row_of_levelled_run_in_list(&self) -> bool {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Horstmann | BraceStyle::Pico
        ) {
            return false;
        }
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.enclosing(span.first))
            .is_some_and(|list| {
                let open = groups.get(list).open;
                self.tree.blocks.kind(list) == Some(BlockKind::Initializer)
                    && groups.get(list).parent.is_some_and(|parent| {
                        self.tree.blocks.kind(parent) == Some(BlockKind::Initializer)
                    })
                    && !self.inline_array.list_rows.aligned_children.contains(&open)
            })
    }

    /// Whether the pending line starts a row of an initializer list whose
    /// elements run in after its brace, and opens a list aligned among
    /// them.
    /// Whether the pending row is an element row of an initializer list
    /// whose first element follows its brace on the brace's line.
    fn pending_row_in_run_in_list(&self) -> bool {
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.enclosing(span.first))
            .is_some_and(|list| {
                self.tree.blocks.kind(list) == Some(BlockKind::Initializer)
                    && self
                        .output
                        .line_with_token(groups.get(list).open)
                        .and_then(|line| self.output.line_tokens(line))
                        .is_some_and(|open_span| open_span.last != groups.get(list).open)
            })
    }

    fn pending_row_of_run_in_list(&self) -> bool {
        let groups = &self.tree.groups;
        let Some(span) = self.output.pending_tokens() else {
            return false;
        };
        groups.enclosing(span.first).is_some_and(|list| {
            self.tree.blocks.kind(list) == Some(BlockKind::Initializer)
                && self
                    .output
                    .line_with_token(groups.get(list).open)
                    .and_then(|line| self.output.line_tokens(line))
                    .is_some_and(|open_span| open_span.last != groups.get(list).open)
        }) && (span.first..=span.last).any(|index| {
            self.inline_array
                .list_rows
                .aligned_children
                .contains(&index)
        })
    }

    /// Whether the pending line opens with the `}` of an initializer list
    /// run in after its declarator, which stays under its brace.
    pub(crate) fn line_closes_run_in_declarator_list(&self) -> bool {
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.closed_at(span.first))
            .is_some_and(|list| {
                groups.get(list).parent.is_none_or(|parent| {
                    !matches!(
                        self.tree.blocks.kind(parent),
                        Some(BlockKind::Initializer | BlockKind::CompoundLiteral) | None
                    )
                }) && self
                    .output
                    .line_with_token(groups.get(list).open)
                    .is_some_and(|line| self.runs_in_list_on(list, line))
            })
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
        // VTK indents only initializer braces nested in code.
        if self.options.brace_style == BraceStyle::Vtk
            && self
                .output
                .pending_tokens()
                .and_then(|span| self.tree.groups.opened_at(span.first))
                .is_some_and(|group| self.vtk_flush_scope(group))
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
            return Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.case_unindent_spaces(),
            );
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
                .is_some_and(|span| self.token_in_parens(span.first))
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

    pub(super) fn open_multiline_attached_initializer_brace(
        &mut self,
        brace_header: Option<String>,
        brace_type: BraceType,
        block_indent_extra: usize,
        force_break_one_line: bool,
        trailing_comment: Option<&str>,
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
        if let Some(comment) = trailing_comment {
            self.push_attached_comment_with_source_gap(comment);
        }
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
            line_lead: None,
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
        trailing_comment: Option<&str>,
    ) {
        let opening_indent = self
            .for_header_continuation_indent_spaces()
            .unwrap_or_else(|| self.current_line_indent_spaces() + self.options.indent_width * 2);
        self.emit_source_space_or_ensure();
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        if let Some(comment) = trailing_comment {
            self.push_attached_comment_with_source_gap(comment);
        }
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
            line_lead: None,
        });
        self.update_current_brace_indent_columns(
            opening_indent + self.options.indent_width,
            opening_indent,
        );
        self.layout
            .continuation_indent
            .set_next_line_spaces(opening_indent + self.options.indent_width);
        self.layout.previous = PreviousToken::Other;
        // The source's own break after the brace ends the line it opened.
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
        // A list among a call's arguments is no element of a list around
        // the call.
        let in_parens = self
            .tree
            .groups
            .opened_at(token_index)
            .and_then(|group| self.tree.groups.get(group).parent)
            .is_some_and(|parent| self.tree.groups.get(parent).delimiter != Delimiter::Brace);
        let enclosed = matches!(brace_type, BraceType::Array | BraceType::CompoundLiteral)
            && !in_parens
            && !self
                .inline_array
                .list_rows
                .aligned_children
                .contains(&token_index)
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
            double_brace_initializer && self.token_in_for_header(token_index);
        // Styles running rows in after braces keep the first element there.
        let runs_in = matches!(
            self.options.brace_style,
            BraceStyle::Horstmann | BraceStyle::Pico
        );
        // The default style runs a source line a brace opens in after that
        // brace, which keeps a list opened mid-row there whole.
        let row_runs_in = self.options.brace_style == BraceStyle::None
            && self.tree.tokens[..token_index]
                .iter()
                .rev()
                .take_while(|token| !matches!(token, Token::Newline))
                .filter(|token| !matches!(token, Token::Whitespace(_)))
                .last()
                .is_some_and(|token| matches!(token, Token::Symbol('{')));
        // A list opened after a comma among the rows of a list that sets
        // them a level in breaks after its brace when it spans rows, as a
        // row's block does.
        let spanning_mid_row_list = run_in_after_comma
            && !row_runs_in
            && self.enclosing_list_rows_leveled(token_index)
            && self
                .tree
                .groups
                .opened_at(token_index)
                .is_some_and(|group| {
                    let groups = &self.tree.groups;
                    // A row of its own elements starts on a later line.
                    groups.get(group).close.is_some_and(|close| {
                        (token_index + 1..close).any(|index| {
                            matches!(self.tree.tokens[index], Token::Newline)
                                && groups.enclosing(index) == Some(group)
                                && next_code_token(&self.tree.tokens, index + 1) != Some(close)
                        })
                    })
                });
        let break_first = (enclosed || double_brace_initializer)
            && !first_is_brace
            && (!run_in_after_comma || spanning_mid_row_list)
            && !runs_in
            && !range_for_header_initializer;
        let nested = enclosed
            || double_brace_initializer
            || (brace_type == BraceType::Array
                && self.inline_array.list_rows.nested.contains(&token_index));
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
        let declares_struct = self.current.trimmed_end().ends_with('=')
            && statement_declares_struct(&self.tree.tokens, token_index);
        let struct_declaration = !nested && declares_struct;
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
                Some('@') => self.emit_source_space(),
                _ if brace_type == BraceType::Initializer
                    && self.current.trimmed_end().ends_with('>') =>
                {
                    self.emit_source_space_or_ensure();
                }
                // A compound literal among a list's elements keeps the gap
                // written before its brace, as any brace there does.
                _ if self.opens_compound_literal_in_list(brace_type) => {
                    self.emit_source_space();
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
        // brace a continuation past its line.
        let enum_block_level = self.options.indent_after_parens
            && brace_type == BraceType::Enum
            && !nested
            && !break_first;
        // A line a comment preindented carries its own indent.
        let text_start = if self.current_is_preindented {
            0
        } else {
            base_indent
        };
        let brace_column = if enum_block_level {
            base_indent
        } else if nested {
            if run_in_after_comma && !first_is_brace {
                base_indent + self.options.indent_width
            } else {
                base_indent
            }
        } else {
            text_start + self.current_char_len() - 1
        };
        if !break_first {
            self.emit_trailing_source_space();
        }
        let statement_base = ContinuationIndent::Level(
            self.layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal),
        )
        .columns(self.options.indent_width);
        // A declarator on a line after its struct type levels the rows from
        // the statement, as on the type's line.
        let rows_base = if nested && declares_struct {
            base_indent.min(statement_base)
        } else {
            base_indent
        };
        let mut column = if enum_block_level {
            base_indent + self.options.continuation_indent * self.options.indent_width
        } else if nested {
            let base_indent = rows_base;
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
                    token_index
                        .checked_add(1)
                        .and_then(|next| self.tree.tokens.get(next)),
                    Some(Token::Symbol('('))
                );
            // The elements align to the brace's tab-expanded column; its
            // closer counts each tab as one column. They align to a first
            // element comments lead.
            let column =
                text_start + self.current_visual_width_from(text_start) + usize::from(padded_paren);
            column
                + leading_comments_width(
                    &self.tree.tokens,
                    token_index,
                    column,
                    self.options.tab_width,
                )
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
        // Elements after a brace past the maximum continuation indent stand
        // two levels past their line, or at the parens or list around them;
        // an assigned value's list keeps them at its brace.
        if !nested
            && !leveled_literal
            && column.saturating_sub(statement_base) > self.options.max_continuation_indent
            && next_non_whitespace(&self.tree.tokens, token_index + 1, self.tree.tokens.len())
                .is_some_and(|next| !matches!(self.tree.tokens[next], Token::Newline))
            && !brace_follows_assignment(&self.tree.tokens, token_index)
        {
            // A stream the statement opens with stands as its parens do.
            let enclosing = if self.layout.nesting.paren_depth > 0 {
                self.layout.nesting.current_continuation_indent_spaces()
            } else {
                self.layout
                    .frame_stack
                    .active_stream()
                    .filter(|stream| stream.assignment_value_start_column.is_none())
                    .map(|stream| stream.chain_anchor_column)
            };
            column = (base_indent + self.options.indent_width * 2).max(
                enclosing
                    .into_iter()
                    .chain(self.current_inline_array_column())
                    .max()
                    .unwrap_or(0),
            );
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
            // The statement's level, though its declarator continues it.
            (self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal))
                * self.options.indent_width
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
            line_lead: (!self.current_is_preindented).then_some(base_indent),
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
                rows_base + self.options.indent_width,
                rows_base,
            );
        }
        if break_first {
            self.finish_line();
            self.layout.continuation_indent.set_next_line_spaces(column);
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    /// Whether the list around the one `brace` opens sets its rows a level
    /// in: its brace ends its source line or opens on a nested brace.
    fn enclosing_list_rows_leveled(&self, brace: usize) -> bool {
        let groups = &self.tree.groups;
        groups
            .opened_at(brace)
            .and_then(|group| groups.get(group).parent)
            .is_some_and(|parent| {
                let open = groups.get(parent).open;
                self.tree.tokens[open + 1..]
                    .iter()
                    .find(|token| !matches!(token, Token::Whitespace(_) | Token::Comment(..)))
                    .is_some_and(|token| matches!(token, Token::Newline | Token::Symbol('{')))
                    // A brace the layout broke its row after levels its rows
                    // too.
                    || self
                        .output
                        .line_with_token(open)
                        .and_then(|line| self.output.line_tokens(line))
                        .is_some_and(|span| span.last == open)
            })
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
        let inline_array = self.inline_array.frames.last().copied();
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
        // A returned `{{` or `{ {` keeps its closers on the last row.
        // A typed initializer keeps its closer on its last row, as does a
        // compound literal among a call's arguments.
        let in_paren_literal = closing_compound_literal && self.layout.nesting.paren_depth > 0;
        let enclosed_run_in = closing_brace_type != Some(BraceType::Initializer)
            && !in_paren_literal
            && !return_initializer
            && self.output.get(open_output_len).is_some_and(|line| {
                line.contains_from_first_byte("{ {") || line.contains_from_first_byte("{{")
            });
        let range_for_initializer = self
            .current
            .active_token()
            .filter(|&close| matches!(self.tree.tokens[close], Token::Symbol('}')))
            .and_then(|close| self.tree.groups.closed_at(close))
            .is_some_and(|list| self.token_in_for_header(self.tree.groups.get(list).open))
            && self
                .output
                .get(open_output_len)
                .is_some_and(|line| line.trimmed_end().ends_with('{'));
        // A compound literal its line opened closes on a line of its own.
        let closing_line_opened_literal = closing_compound_literal
            && (inline_array.is_some_and(|frame| frame.leveled_literal)
                || self
                    .output
                    .get(open_output_len)
                    .is_some_and(|line| self.output.code_trimmed_of(line).ends_with('{')));
        // The last `, {` of the line holding the list's brace opens the list
        // only when no later brace of the line opens another.
        let later_brace_on_line = self
            .current
            .active_token()
            .and_then(|close| self.tree.groups.closed_at(close))
            .map(|group| self.tree.groups.get(group).open)
            .and_then(|open| {
                let span = self
                    .output
                    .line_tokens(self.output.line_with_token(open)?)?;
                Some(
                    self.tree.tokens[open + 1..=span.last]
                        .iter()
                        .any(|token| matches!(token, Token::Symbol('{'))),
                )
            })
            .unwrap_or(false);
        // Comments between the comma and the brace take no part.
        let open_follows_comma = self
            .current
            .active_token()
            .and_then(|close| self.tree.groups.closed_at(close))
            .and_then(|group| {
                self.tree
                    .previous_code_token(self.tree.groups.get(group).open)
            })
            .is_some_and(|before| matches!(self.tree.tokens[before], Token::Symbol(',')));
        // A list after a call's argument is one wherever its line opens
        // more lists.
        let follows_call_argument = open_follows_comma
            && self
                .current
                .active_token()
                .and_then(|close| self.tree.groups.closed_at(close))
                .and_then(|group| self.tree.groups.get(group).parent)
                .is_some_and(|parent| self.tree.groups.get(parent).delimiter == Delimiter::Paren);
        let call_argument_array_column = self
            .output
            .get(open_output_len)
            .filter(|_| !later_brace_on_line || follows_call_argument)
            .and_then(|line| {
                brace_column.filter(|_| open_follows_comma).or_else(|| {
                    line.rfind(", {").map(|comma| {
                        visual_width_from(&line[..comma + 2], 0, self.options.tab_width)
                    })
                })
            });
        let call_argument_array = call_argument_array_column.is_some();
        // The list's own brace decides, whatever braces its line opens later.
        let follows_template_type = self
            .current
            .active_token()
            .filter(|&close| matches!(self.tree.tokens[close], Token::Symbol('}')))
            .and_then(|close| self.tree.groups.closed_at(close))
            .and_then(|list| self.tree.previous_code_token(self.tree.groups.get(list).open))
            .map(|before| matches!(&self.tree.tokens[before], Token::Operator(operator) if operator == ">"));
        let typed_initializer = matches!(closing_brace_type, Some(BraceType::Initializer))
            && follows_template_type.unwrap_or_else(|| {
                self.output
                    .get(open_output_len)
                    .is_some_and(|line| line_opens_typed_initializer(line))
            });
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
        // Run-in closers stay after a block comment their line holds.
        let run_in_after_comment = matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) && self.current.trimmed_end().ends_with("*/");
        // An inner closer the line holds stands among the rows of the brace
        // it closes into.
        let finished_inner_closer = !forced_break
            && !broken_enum_row
            && !run_in_after_comment
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
        // A row a closer breaks off below stands among the elements of the
        // list it closes, not those of a list enclosing it.
        let enclosing_list = self
            .inline_array
            .frames
            .len()
            .checked_sub(2)
            .and_then(|index| self.inline_array.frames.get(index))
            .is_some_and(|outer| inline_array.is_some_and(|frame| outer.depth + 1 == frame.depth));
        let row_breaks_off = enclosing_list
            && !finished_inner_closer
            && !self.current_is_blank()
            && !broken_enum_row
            && !broken_literal_row
            && (forced_break
                || self.token_input.token_begins_source_line
                || !run_in_after_comment
                    && self.output.len() > open_output_len
                    && (closing_line_opened_literal
                        || aggregate_assign
                        || objc_dictionary
                        || closing_enum
                        || enclosed_run_in
                        || range_for_initializer
                        || call_argument_array
                        || typed_initializer));
        // A row of a compound literal the closer parts from it below
        // publishes while the literal is open, as one a closer leading its
        // line leaves.
        let closer_parts_literal_row = closing_line_opened_literal
            && !forced_break
            && !row_breaks_off
            && !finished_inner_closer
            && !broken_literal_row
            && !self.token_input.token_begins_source_line
            && !self.current_is_blank()
            && !run_in_after_comment
            && self.output.len() > open_output_len;
        if row_breaks_off || closer_parts_literal_row {
            self.finish_line();
        }
        self.inline_array.frames.pop();
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
        } else if self.token_input.token_begins_source_line || self.current_is_blank() {
            // A closer a finished row leaves on a line of its own stands
            // like one that leads its source line.
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
            && !run_in_after_comment
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
        // Lisp and pico set a space before a closer a row holds; one right
        // after a block comment keeps the gap the source left.
        let spaced_closer = if run_in_after_comment {
            self.token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|gap| !gap.is_empty())
        } else {
            matches!(
                self.options.brace_style,
                BraceStyle::Pico | BraceStyle::Lisp
            ) && !self.current_is_blank()
        };
        if let Some(Some(gap)) = return_initializer_gap.or(source_closing_gap) {
            self.trim_current_end_horizontal_space();
            self.current.push_str(&gap);
        } else {
            self.trim_current_end_horizontal_space();
            if spaced_closer {
                self.ensure_space();
            }
        }
        self.mark_closed_brace_output_position();
        self.current.push('}');
        self.layout.command_state.observe_char('}');
        self.layout.compound_literal.just_closed = closing_compound_literal;
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    /// A list's columns move with the line holding its brace when the line
    /// publishes at another lead than it was laid out with.
    pub(crate) fn shift_inline_arrays_to_published_line(&mut self, published: usize) {
        if self.output.len() <= published {
            return;
        }
        let lead = self.output.lead_width(published, self.options.tab_width)
            + self.layout.line_adjuster.last_line_case_unindent();
        // Lisp and pico join a closer's line to the row before; its lists'
        // rows align to where its text then stands.
        let target = self
            .lisp_joined_text_column(published)
            .map_or(lead, |column| {
                column + self.layout.line_adjuster.last_line_case_unindent()
            });
        for frame in &mut self.inline_array.frames {
            if frame.output_line != published {
                continue;
            }
            let laid_out = frame.line_lead.take().unwrap_or(lead);
            if laid_out != target {
                let shift = |column: usize| (column + target).saturating_sub(laid_out);
                frame.body_column = shift(frame.body_column);
                frame.brace_column = shift(frame.brace_column);
            }
        }
    }

    /// The column the text of output line `line` stands at once lisp and
    /// pico join it, with the closers leading it, to the row before.
    fn lisp_joined_text_column(&self, line: usize) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) {
            return None;
        }
        let joins = |index: usize| {
            index > 0
                && self.output.trimmed(index).starts_with('}')
                && self.output.line_tokens(index).is_some()
                && {
                    let previous = &self.output[index - 1];
                    !previous.trimmed().is_empty()
                        && preprocessor_directive(previous.trimmed_start()).is_none()
                        && !previous.trimmed_end().ends_with('\\')
                        && !line_ends_with_comment(previous)
                }
        };
        if !joins(line) {
            return None;
        }
        let tab_width = self.options.tab_width;
        let gap =
            |previous: usize| usize::from(!self.output[previous].trimmed_end().ends_with('{'));
        // A line joining the line asked last goes on from that line's column,
        // while the lines up to it stand as they were.
        if let Some((asked, column, version)) = self.lisp_join_column.get()
            && asked + 1 == line
            && self
                .output
                .lowest_change_since(version)
                .is_some_and(|lowest| lowest > asked)
            && joins(asked)
        {
            let end = visual_width_from(self.output.trimmed(asked), column, tab_width) + gap(asked);
            self.lisp_join_column
                .set(Some((line, end, self.output.version())));
            return Some(end);
        }
        let mut start = line;
        while joins(start) {
            start -= 1;
        }
        let mut end = visual_width_from(self.output[start].trimmed_end(), 0, tab_width);
        for index in start + 1..=line {
            end += gap(index - 1);
            if index < line {
                end = visual_width_from(self.output.trimmed(index), end, tab_width);
            }
        }
        self.lisp_join_column
            .set(Some((line, end, self.output.version())));
        Some(end)
    }

    pub(crate) fn current_inline_array_column(&self) -> Option<usize> {
        self.inline_array
            .frames
            .last()
            .filter(|frame| frame.depth == self.layout.nesting.brace_header_stack.len())
            .map(|frame| frame.body_column)
    }

    /// The column of the rows of the list the line is in, when they stand a
    /// level past the column the list closes at rather than past its brace.
    pub(crate) fn current_leveled_list_row_column(&self) -> Option<usize> {
        self.inline_array
            .frames
            .last()
            .filter(|frame| {
                frame.depth == self.layout.nesting.brace_header_stack.len()
                    && frame.body_column == frame.brace_column + self.options.indent_width
            })
            .map(|frame| frame.body_column)
    }

    /// The column of the list the pending line continues; lists its own
    /// braces open do not hold it.
    pub(crate) fn line_start_inline_array_column(&self) -> Option<usize> {
        let pending = self.output.len();
        let frames = &self.inline_array.frames;
        // A row a list it starts in closes stands among that list's rows.
        if let (line, open, Some(column)) = self.inline_array.line_start
            && line == pending
            && frames.len() < open
            && self
                .output
                .pending_tokens()
                .and_then(|span| self.tree.groups.enclosing(span.first))
                .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Brace)
        {
            return Some(column);
        }
        let opened_here = frames
            .iter()
            .rev()
            .take_while(|frame| frame.output_line == pending)
            .count();
        frames[..frames.len() - opened_here]
            .last()
            .filter(|frame| {
                frame.depth + opened_here == self.layout.nesting.brace_header_stack.len()
            })
            .map(|frame| frame.body_column)
    }

    /// The column of the list around the one the pending line continues.
    pub(crate) fn enclosing_inline_array_column(&self) -> Option<usize> {
        let pending = self.output.len();
        let frames = &self.inline_array.frames;
        let opened_here = frames
            .iter()
            .rev()
            .take_while(|frame| frame.output_line == pending)
            .count();
        let [.., outer, inner] = &frames[..frames.len() - opened_here] else {
            return None;
        };
        (outer.depth + 1 == inner.depth).then_some(outer.body_column)
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

    /// The lead of the row starting the element whose list a `},` closes
    /// right before the pending line.
    fn pending_closed_element_row_lead(&self) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let first = self.output.pending_tokens()?.first;
        let comma = self.tree.previous_code_token(first)?;
        let close = self.tree.previous_code_token(comma)?;
        if !matches!(tokens[comma], Token::Symbol(',')) {
            return None;
        }
        let group = groups.closed_at(close)?;
        let parent = groups.get(group).parent?;
        let mut start = groups.get(group).open;
        while let Some(before) = self.tree.previous_code_token(start) {
            if groups.opened_at(before) == Some(parent)
                || matches!(tokens[before], Token::Symbol(','))
                    && groups.enclosing(before) == Some(parent)
            {
                break;
            }
            start = groups
                .closed_at(before)
                .map_or(before, |closed| groups.get(closed).open);
        }
        let line = self.output.line_with_token(start)?;
        Some(self.output.lead_width(line, self.options.tab_width))
    }

    /// Whether the innermost open brace opens on the pending row.
    fn pending_row_opens_innermost_brace(&self) -> bool {
        self.layout
            .frame_stack
            .active_brace()
            .and_then(|frame| frame.open_token)
            .zip(self.output.pending_tokens())
            .is_some_and(|(open, span)| (span.first..=span.last).contains(&open))
    }

    /// The column of the rows of the aggregate a `struct` declaration
    /// assigns: a level past the statement, as its other rows stand, though
    /// its declarator continues the statement on a line of its own.
    fn struct_declaration_rows_spaces(&self) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let span = self.output.pending_tokens()?;
        let group = groups.get(groups.enclosing(span.first)?);
        let brace = group.open;
        let ends_line = tokens[brace + 1..]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_) | Token::Comment(..)))
            .is_none_or(|token| matches!(token, Token::Newline));
        if group.delimiter != Delimiter::Brace || !ends_line {
            return None;
        }
        Some(
            self.struct_declaration_statement_lead(brace)?
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The lead of the line starting the `struct` declaration whose value
    /// `brace` opens, though its declarator stands on a later line.
    pub(crate) fn struct_declaration_statement_lead(&self, brace: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !brace_follows_assignment(tokens, brace) || !statement_declares_struct(tokens, brace) {
            return None;
        }
        // The statement starts after the one before it and any label.
        let after = tokens[..brace]
            .iter()
            .rposition(|token| matches!(token, Token::Symbol(';' | '{' | '}')))
            .map_or(0, |index| index + 1);
        let start = (after..brace)
            .rev()
            .find(|&index| self.tree.statements.starts_block_statement(index))
            .or_else(|| (after..brace).find(|&index| is_code_token(&tokens[index])))?;
        let line = self.output.line_with_token(start)?;
        Some(self.output.lead_width(line, self.options.tab_width))
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
        if closing && self.line_closes_run_in_declarator_list() {
            return None;
        }
        // A designator row of a list aligned after its brace stands at its
        // elements, though a list it opens holds the line.
        if designator && has_unmatched_open_brace(line) && self.pending_row_of_run_in_list() {
            return None;
        }
        // So does any other designator row of a list whose first element
        // runs in after its brace, whatever rows its value spans.
        if designator
            && !has_unmatched_open_brace(line)
            && self.pending_row_in_run_in_list()
            && let Some(column) = self.line_start_inline_array_column()
        {
            return Some(column);
        }
        if closing && let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces {
            return Some(spaces);
        }
        // A compound literal's fields stand at its elements; a row opening
        // one as a value stands in the list around it.
        if designator
            && self.innermost_brace_is_compound_literal()
            && !self.pending_row_opens_innermost_brace()
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
            // The row of the element the closer ends sets the column; a
            // style indenting braces sets the closer apart from it.
            return Some(
                self.pending_closed_element_row_lead()
                    .unwrap_or_else(|| leading_visual_width(previous, self.options.tab_width))
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
            // The list holding the row sets it, not a brace the row opens;
            // VTK sets a list's brace flush at a declaration scope.
            let list = if self.pending_row_opens_innermost_brace() {
                self.layout.frame_stack.enclosing_brace()
            } else {
                self.layout.frame_stack.active_brace()
            };
            let list_brace_indented = list.is_some_and(|frame| {
                match frame
                    .open_token
                    .and_then(|open| self.tree.groups.opened_at(open))
                {
                    Some(group) if self.options.brace_style == BraceStyle::Vtk => {
                        !self.vtk_flush_scope(group)
                    }
                    _ => self.should_indent_brace_line(frame.brace_type),
                }
            });
            if previous.trimmed() == "{" && list_brace_indented {
                return Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.case_unindent_spaces(),
                );
            }
            if let Some(spaces) = self.struct_declaration_rows_spaces() {
                return Some(spaces);
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
                if let Some(rows) = self.struct_declaration_rows_spaces() {
                    spaces = spaces.min(rows);
                } else if let Some(index) = self.row_initializer_opener_line() {
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
            // Directives between the members hold no member.
            self.previous_code_line_past_comments_and_directives()
                .map(|previous| {
                    let code = self.output.code_of(previous).trimmed();
                    let width = leading_visual_width(previous, self.options.tab_width);
                    // A first member sits a level past the line its brace
                    // ends, or at a brace the style indents on its own line.
                    if code == "{" && self.should_indent_brace_line(BraceType::Struct) {
                        width
                    } else if code.ends_with('{') {
                        width + self.options.indent_width
                    } else {
                        width
                    }
                })
        };
        if reference.is_some_and(|width| width + case_unindent_spaces == current_spaces) {
            return None;
        }
        let aggregate_member = self.in_aggregate_declaration_brace();
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
    /// The lead the brace's line was laid out with, until it publishes.
    pub(crate) line_lead: Option<usize>,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct InlineArrayState {
    pub(crate) initializer_designator_bracket_depth: usize,
    pub(crate) frames: Vec<InlineArrayFrame>,
    pub(crate) current_closed_body_column: Option<(usize, bool)>,
    pub(super) aggregate_braces: Vec<bool>,
    /// Shared with the snapshots taken at conditionals, which never change
    /// it.
    pub(crate) list_rows: std::rc::Rc<BraceListRows>,
    /// The line after the last published one, the lists open where it
    /// starts, and the column of the innermost.
    pub(crate) line_start: (usize, usize, Option<usize>),
}

/// Whether the style runs a list's first row in after a brace leading it.
pub(crate) fn runs_list_rows_in(style: BraceStyle) -> bool {
    matches!(
        style,
        BraceStyle::None | BraceStyle::Horstmann | BraceStyle::Pico
    )
}

/// The fill between a list brace leading its row and a comment run in after
/// it, which stands where the list's rows do.
pub(crate) fn initializer_brace_comment_gap(options: &FormatOptions, brace_line: &str) -> String {
    let brace_column = leading_visual_width(brace_line, options.tab_width);
    let target = brace_column + options.indent_width;
    horstmann_run_in_fill(brace_line, &" ".repeat(target), options)
}

/// The width of the block comments, with the space after each, before the
/// first element on the line of the brace at `brace`, from `column`.
fn leading_comments_width(
    tokens: &[Token],
    brace: usize,
    column: usize,
    tab_width: usize,
) -> usize {
    let Some(rest) = brace.checked_add(1).and_then(|start| tokens.get(start..)) else {
        return 0;
    };
    let mut rest = rest.iter();
    // The space after the brace is already on the line.
    let mut next = rest.next();
    if matches!(next, Some(Token::Whitespace(_))) {
        next = rest.next();
    }
    let mut led = String::new();
    while let Some(Token::Comment(CommentKind::Block, comment)) = next {
        if comment.contains('\n') {
            return 0;
        }
        led.push_str(comment);
        next = rest.next();
        if let Some(Token::Whitespace(space)) = next {
            led.push_str(space);
            next = rest.next();
        }
    }
    let element_follows = next.is_some_and(|token| {
        !matches!(
            token,
            Token::Newline | Token::Comment(..) | Token::Preprocessor(_)
        )
    });
    if element_follows && !led.is_empty() {
        visual_width_from(&led, column, tab_width)
    } else {
        0
    }
}
