use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::braces::classification::{
    ExternCGuard, is_lambda_body_header, is_namespace_or_module_block_header,
};
use crate::formatter::braces::rewrite::is_standard_add_braces_header;
use crate::formatter::constructs::headers::{
    is_attachable_closing_header, same_line_nested_header_extra, starts_header_word,
};
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::output::buffer::OpenBraceShape;
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::{BlockKind, next_code_token};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, line_brace_imbalance, preprocessor_directive,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::literals::starts_string_literal_token;
use crate::formatter::tokens::operators::head_ends_binary_operator;

pub(crate) fn starts_post_closing_declaration(line: &str) -> bool {
    let Some(tail) = line.trimmed_start().strip_prefix('}') else {
        return false;
    };
    let tail = tail.trimmed_start();
    if tail.is_empty() || tail.ends_with('{') {
        return false;
    }
    let word = tail
        .split(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
        .next()
        .unwrap_or_default();
    !word.is_empty() && !matches!(word, "while") && !is_attachable_closing_header(word)
}

fn split_return_call_with_comment(line: &mut String) -> Option<String> {
    let leading = line.len() - line.trimmed_start().len();
    let trimmed = line.trimmed_start();
    if !trimmed.starts_with("return ") || !trimmed.contains("//") {
        return None;
    }
    let open = line.find('(')?;
    line[open + 1..].find(");")?;
    let tail = line[open + 1..].to_string();
    line.truncate(open + 1);
    Some(format!("{}{}", " ".repeat(leading + 11), tail))
}

impl FormatEngine<'_> {
    pub(crate) fn compound_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !line.trimmed_start().starts_with("}, ") {
            return None;
        }
        let mut closed_blocks = 0usize;
        for opening in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
        {
            let code = self.output.code_trimmed_of(opening);
            let opens = code.chars().filter(|ch| *ch == '{').count();
            let closes = code.chars().filter(|ch| *ch == '}').count();
            if opens > closes + closed_blocks && !code.trimmed_start().starts_with('{') {
                return Some(leading_visual_width(opening, self.options.tab_width));
            }
            // A sibling `}, {` opened the element this line closes.
            if closed_blocks == 0 && code.trimmed_start().starts_with('}') && code.ends_with('{') {
                return Some(leading_visual_width(opening, self.options.tab_width));
            }
            closed_blocks += closes;
            closed_blocks = closed_blocks.saturating_sub(opens);
        }
        None
    }

    pub(crate) fn unmatched_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        (line.trimmed() == "}" && self.layout.nesting.last_closed_brace_type.is_none()).then_some(0)
    }

    fn isolated_opening_brace_indent_from_output(&self) -> Option<usize> {
        if self.options.brace_style == BraceStyle::Horstmann {
            return None;
        }
        let tab_width = self.options.tab_width;
        if let Some(found) = self.output.innermost_open_brace_line() {
            let index = found?;
            let meta = self.output.brace_meta(index);
            return match meta.open_shape {
                OpenBraceShape::Isolated => Some(self.output.lead_width(index, tab_width)),
                OpenBraceShape::Label => {
                    Some(self.layout.indentation.indent() * self.options.indent_width)
                }
                OpenBraceShape::Other => None,
            };
        }
        let mut depth = 0usize;
        // Depths at each enclosing `#endif`: a later branch of a group
        // repeats the braces of the first one.
        let mut group_depths = Vec::new();
        for index in (0..self.output.len()).rev() {
            // The body of a block comment holds no braces.
            if self.output.comment_start_index(index) != index {
                continue;
            }
            let meta = self.output.brace_meta(index);
            if depth == 0 && meta.code_starts_with_hash {
                return None;
            }
            if meta.code_starts_with_hash {
                match preprocessor_directive(self.output.trimmed(index)) {
                    Some("endif") => group_depths.push(depth),
                    Some("else" | "elif" | "elifdef" | "elifndef") => {
                        if let Some(&group_depth) = group_depths.last() {
                            depth = group_depth;
                        }
                    }
                    Some("if" | "ifdef" | "ifndef") => {
                        group_depths.pop();
                    }
                    _ => {}
                }
            }
            depth += meta.closes();
            if meta.opens() > depth {
                return match meta.open_shape {
                    OpenBraceShape::Isolated => Some(self.output.lead_width(index, tab_width)),
                    OpenBraceShape::Label => {
                        Some(self.layout.indentation.indent() * self.options.indent_width)
                    }
                    OpenBraceShape::Other => None,
                };
            }
            depth -= meta.opens();
        }
        None
    }

    pub(crate) fn isolated_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        case_unindent_closing_line: bool,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !(line.trimmed() == "}" || trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            || case_unindent_closing_line
            || (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
                && self
                    .output
                    .last_line_outside_comment()
                    .is_some_and(|line| preprocessor_directive(line.trimmed_start()).is_some())
        {
            return None;
        }
        self.isolated_opening_brace_indent_from_output()
    }

    /// `line` moved to its opening brace's line, when it is an isolated
    /// closing brace that stands elsewhere.
    pub(crate) fn align_isolated_closing_brace_line(&self, line: &LineView<'_>) -> Option<String> {
        let line_start = line.trimmed_start();
        if !(line_start == "}" || line_start.starts_with("} else"))
            || self.isolated_opening_brace_is_switch_label()
        {
            return None;
        }
        let mut spaces = self.isolated_opening_brace_indent_from_output()?;
        let structural_switch_indent = self
            .layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command
                    && frame.header.as_deref() == Some("switch")
                    && frame.split_header
            })
            .map(|frame| frame.sibling_indent_column);
        if let Some(structural) = structural_switch_indent {
            spaces = structural;
        }
        let current = leading_visual_width(line, self.options.tab_width);
        (current < spaces || structural_switch_indent.is_some() && current != spaces)
            .then(|| format!("{}{}", " ".repeat(spaces), line_start))
    }

    pub(crate) fn continuation_adjacent_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Horstmann
                | BraceStyle::Pico
        ) || !line.trimmed_start().starts_with('}')
        {
            return None;
        }
        let index = self.output.last_open_brace_alone_line()?;
        // Rows of a comment hold no operator.
        let before_open = (0..index).rev().find(|&row| {
            self.output.comment_start_index(row) == row && !self.output.code_trimmed(row).is_empty()
        })?;
        let code = self.output.code_trimmed(before_open);
        (head_ends_binary_operator(code) || code.ends_with("->"))
            .then(|| leading_visual_width(&self.output[index], self.options.tab_width))
    }

    pub(crate) fn gnu_command_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Gnu || line.trimmed() != "}" {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Command)?;
        Some(
            frame.header_indent_column
                + usize::from(
                    frame.header.is_some()
                        && !matches!(frame.header.as_deref(), Some("case" | "default")),
                ) * self.options.indent_width,
        )
    }

    pub(crate) fn ratliff_command_closing_header_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Ratliff {
            return None;
        }
        let after = line.trimmed_start().strip_prefix("} ")?;
        // A comment leaves a case block's closer where the case unindent
        // puts it.
        let comment = after.trimmed_start().starts_with('/');
        self.layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command && !(comment && frame.case_block)
            })
            .map(|frame| frame.header_indent_column + self.options.indent_width)
    }

    pub(crate) fn lambda_closing_brace_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if !line.trimmed_start().starts_with('}') {
            return None;
        }
        self.layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
            .map(|frame| {
                if self.options.brace_style == BraceStyle::Ratliff {
                    frame.body_indent_column
                } else {
                    frame.sibling_indent_column
                }
            })
    }

    pub(crate) fn push_close_brace(&mut self, next: Option<&Token>, next_is_adjacent: bool) {
        let closing_lambda_body = self.current_open_brace_is_lambda_body();
        if self.current_is_blank() {
            self.layout.frame_stack.clear_closed_braces();
        }
        if self.one_line_block_mode {
            self.push_inline_close_brace(next);
            return;
        }
        let whitespace_before_brace = self
            .token_input
            .previous_input_whitespace
            .clone()
            .unwrap_or_default();
        if self.current_inline_array_column().is_some() {
            self.close_inline_array_brace();
            return;
        }
        if matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) && !self.token_input.token_begins_source_line
            && !whitespace_before_brace.is_empty()
        {
            let current_ends_open = self.current.trimmed_end().ends_with('{');
            let previous_ends_open = self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with('{'));
            let formatter_join_gap = if self.current_is_blank() {
                !previous_ends_open
            } else {
                !current_ends_open
            };
            let gap_end = if formatter_join_gap {
                whitespace_before_brace
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index)
            } else {
                whitespace_before_brace.len()
            };
            let source_gap = &whitespace_before_brace[..gap_end];
            if self.current_is_blank() {
                if let Some(previous) = self.output.last_mut() {
                    previous.truncate(previous.trimmed_end().len());
                    previous.push_str(source_gap);
                }
            } else {
                self.trim_current_end_horizontal_space();
                self.current.push_str(source_gap);
                self.preserve_run_in_join_space = true;
            }
        }
        if self.options.brace_style == BraceStyle::Whitesmith
            && self.layout.nesting.brace_type_stack.is_empty()
            && !self.current_is_blank()
            && next_is_adjacent
            && matches!(next, Some(Token::Word(_) | Token::Number(_)))
            && self.current.trimmed_end().ends_with_any(b"+-*/%&|!~")
        {
            self.current.push('}');
            self.layout.command_state.observe_char('}');
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }
        // A statement expression's `}` written after its last statement
        // stays there, as in `_a * 2; })`.
        let attached_statement_expression = !matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) && !self.token_input.token_begins_source_line
            && !self.current_is_blank()
            && self
                .current
                .active_token()
                .is_some_and(|brace| self.closes_statement_expression(brace));
        // An initializer row keeps the closer written after its block
        // comment.
        let attached_after_comment = matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) && !self.token_input.token_begins_source_line
            && self.current.trimmed_end().ends_with("*/")
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Array | BraceType::Initializer)
            );
        if attached_statement_expression {
            self.ensure_space();
        } else if attached_after_comment {
            self.trim_current_end_horizontal_space();
            self.ensure_space();
        } else {
            self.finish_line();
        }
        self.layout.pending_braceless_block_bias = None;
        self.layout.inline_nested_header_braceless_bias = None;
        self.prepare_case_closing_brace();
        let unmatched_closing_brace = self.layout.nesting.brace_type_stack.is_empty();
        if unmatched_closing_brace {
            self.unmatched_closing_brace_recovery = true;
        }
        let closing_frame_indent = self.closing_brace_sibling_indent_column();
        self.exit_brace_state();
        if self.layout.indentation.brace_block_depth() == 0 {
            self.extern_c_guard = ExternCGuard::Idle;
        }
        if self.layout.nesting.last_closed_brace_header.is_some() {
            self.layout.command_state.pre_brace_header_stack.pop();
        }
        self.layout.continuation_indent.next_line_indent_spaces = None;
        self.set_indent_after_closing_brace(next, unmatched_closing_brace, closing_frame_indent);
        self.preprocessor.split_else.closing_brace_has_else =
            matches!(next, Some(Token::Word(word)) if word == "else");
        self.mark_closed_brace_output_position();
        self.current.push('}');
        self.layout.command_state.observe_char('}');
        self.layout.compound_literal.just_closed =
            self.layout.nesting.last_closed_brace_type == Some(BraceType::CompoundLiteral);
        // Breaking one-line headers breaks the one-line blocks they open.
        let move_one_line_block_comment = (self.options.break_one_line_blocks
            || self.comments.closing_broken_added_block
            || self.options.break_one_line_headers
                && self
                    .layout
                    .nesting
                    .last_closed_brace_header
                    .as_deref()
                    .is_some_and(|header| {
                        is_standard_add_braces_header(header) || header == "switch"
                    }))
            && self.layout.line_state.is_one_line_block
            && !matches!(
                self.layout.nesting.last_closed_brace_type,
                Some(BraceType::Array | BraceType::CompoundLiteral | BraceType::Initializer)
            )
            && matches!(next, Some(Token::Comment(_, comment)) if !comment.contains('\n') && !comment.contains('}'));
        if move_one_line_block_comment {
            self.move_one_line_block_comment_after_brace(next, &whitespace_before_brace);
        }
        self.finish_line_after_closing_brace(
            next,
            next_is_adjacent,
            unmatched_closing_brace,
            closing_lambda_body,
            move_one_line_block_comment,
        );
    }

    fn closes_statement_expression(&self, brace: usize) -> bool {
        self.tree
            .groups
            .closed_at(brace)
            .and_then(|group| self.tree.blocks.kind(group))
            == Some(BlockKind::StatementExpression)
    }

    /// Whether the token after the `;` being pushed is a statement
    /// expression's `}` on the same source line.
    pub(crate) fn attached_statement_expression_closer_follows(&self) -> bool {
        // These styles attach every closer already.
        if matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        ) {
            return false;
        }
        let tokens = &self.tree.tokens;
        let Some(semicolon) = self.current.active_token() else {
            return false;
        };
        let Some(next) = next_code_token(tokens, semicolon + 1) else {
            return false;
        };
        self.closes_statement_expression(next)
            && !tokens[semicolon + 1..next]
                .iter()
                .any(|token| matches!(token, Token::Newline | Token::Comment(_, _)))
    }

    fn closing_brace_sibling_indent_column(&self) -> Option<usize> {
        self.layout.frame_stack.active_brace().and_then(|frame| {
            if self.options.brace_style == BraceStyle::Horstmann {
                return None;
            }
            if !matches!(
                frame.brace_type,
                BraceType::Command | BraceType::Definition | BraceType::NonStatement
            ) {
                return None;
            }
            self.output
                .scoped()
                .iter()
                .rev()
                .take(8)
                .find(|line| {
                    line.trimmed() == "{"
                        && leading_visual_width(line, self.options.tab_width)
                            == frame.sibling_indent_column
                })
                .map(|_| frame.sibling_indent_column)
        })
    }

    fn set_indent_after_closing_brace(
        &mut self,
        next: Option<&Token>,
        unmatched_closing_brace: bool,
        closing_frame_indent: Option<usize>,
    ) {
        let should_indent_closing_brace = self
            .layout
            .nesting
            .last_closed_brace_type
            .is_some_and(|brace_type| self.should_indent_brace_line(brace_type));
        let closing_brace_is_lambda = matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) && self
            .output
            .scoped()
            .iter()
            .rev()
            .take(4)
            .any(|line| is_lambda_body_header(line.trimmed_end()));
        if should_indent_closing_brace || self.layout.nesting.last_closed_brace_extra_indent > 0 {
            if closing_brace_is_lambda && matches!(next, Some(Token::Symbol(';' | ','))) {
                self.layout.continuation_indent.next_line_indent = Some(self.statement_level());
            } else {
                self.layout.continuation_indent.next_line_indent = Some(self.statement_level() + 1);
            }
            self.layout.continuation_indent.next_line_indent_spaces = None;
        } else if matches!(next, Some(Token::Symbol(';' | ','))) {
            self.layout.continuation_indent.next_line_indent = Some(self.statement_level());
        } else {
            self.layout.continuation_indent.next_line_indent = None;
        }
        if unmatched_closing_brace {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(0);
        } else if let Some(column) = closing_frame_indent {
            self.layout.continuation_indent.set_next_line_spaces(column);
        }
    }

    fn move_one_line_block_comment_after_brace(
        &mut self,
        next: Option<&Token>,
        whitespace_before_brace: &str,
    ) {
        let mut moved_comment_tail = None;
        if let Some(Token::Comment(_, comment)) = next {
            let target_index = self
                .output
                .iter()
                .rposition(|line| self.output.code_trimmed_of(line).ends_with('{'))
                .and_then(|open_index| {
                    (open_index + 1..self.output.len())
                        .find(|index| !self.output[*index].trimmed().is_empty())
                })
                .or_else(|| self.output.len().checked_sub(1));
            // astyle puts the brace's indent less one after the space
            // that follows the block's first statement, a space at least
            // where another statement follows.
            let gap = format!(
                "{}{}",
                self.first_block_statement_gap()
                    .unwrap_or_else(|| whitespace_before_brace.to_string()),
                " ".repeat(self.options.indent_width.saturating_sub(1))
            );
            if let Some(line) = target_index.and_then(|index| self.output.get_mut(index)) {
                line.push_str(&gap);
                line.push_str(comment.trimmed_end());
                if self
                    .options
                    .max_code_length
                    .is_some_and(|width| line.len() > width)
                {
                    moved_comment_tail = split_return_call_with_comment(line);
                }
                self.comments.skip_next_attached_comment = true;
            }
        }
        if let Some(tail) = moved_comment_tail {
            self.output.push(tail);
        }
    }

    /// The source whitespace after the first statement of the block the
    /// closing brace being pushed ends.
    fn first_block_statement_gap(&self) -> Option<String> {
        let tokens = &self.tree.tokens;
        let close = self
            .current
            .active_token()
            .filter(|&index| matches!(tokens[index], Token::Symbol('}')))?;
        let group = self.tree.groups.closed_at(close)?;
        let open = self.tree.groups.get(group).open;
        let semicolon = (open + 1..close).find(|&index| {
            matches!(tokens[index], Token::Symbol(';'))
                && self.tree.groups.enclosing(index) == Some(group)
        })?;
        let gap = match &tokens[semicolon + 1] {
            Token::Whitespace(whitespace) => whitespace.to_string(),
            _ => String::new(),
        };
        let next = next_code_token(tokens, semicolon + 1)?;
        Some(if gap.is_empty() && next != close {
            " ".to_string()
        } else {
            gap
        })
    }

    fn finish_line_after_closing_brace(
        &mut self,
        next: Option<&Token>,
        next_is_adjacent: bool,
        unmatched_closing_brace: bool,
        closing_lambda_body: bool,
        move_one_line_block_comment: bool,
    ) {
        let source_attached_statement_after_closing = matches!(next, Some(Token::Word(word)) if matches!(word.as_str(), "break" | "continue" | "return" | "goto"));
        let source_attached_word_after_closing = matches!(next, Some(Token::Word(word))
        if !(is_attachable_closing_header(word)
            || word.starts_with("while")
            || word.starts_with("catch")
            || matches!(
                word.as_str(),
                "if" | "for" | "switch" | "case" | "default" | "do" | "try" | "__try"
            )))
            && (self.options.brace_style == BraceStyle::None
                || self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_none_or(|whitespace| !whitespace.contains('\n')))
            && !matches!(
                self.layout.nesting.last_closed_brace_header.as_deref(),
                Some("if" | "for" | "switch" | "while" | "else" | "try" | "catch")
            )
            && !source_attached_statement_after_closing;
        let source_attached_closing = self.options.brace_style == BraceStyle::None
            && self.token_input.token_begins_source_line
            && !source_attached_statement_after_closing
            && !(self.layout.nesting.last_closed_brace_type == Some(BraceType::CompoundLiteral)
                && matches!(next, Some(Token::Symbol('('))))
            && !(self.layout.nesting.last_closed_brace_breaks_before_call
                && matches!(next, Some(Token::Symbol('('))))
            && !matches!(
                next,
                None | Some(Token::Newline) | Some(Token::Word(_) | Token::Number(_))
            );
        let source_attached_operator_after_closing = next_is_adjacent
            && (matches!(
                next,
                Some(Token::Operator(operator)) if operator == "~"
            ) || matches!(next, Some(Token::Symbol('~'))));
        let source_attached_symbol_after_closing =
            next_is_adjacent && matches!(next, Some(Token::Symbol('[' | ':')));
        let source_attached_number_after_closing = matches!(next, Some(Token::Number(_)))
            && self
                .token_input
                .next_input_whitespace
                .as_deref()
                .is_none_or(|whitespace| !whitespace.contains('\n'));
        let attached_ternary_colon = closing_lambda_body
            && matches!(next, Some(Token::Symbol(':')))
            && self
                .layout
                .frame_stack
                .active_ternary()
                .is_some_and(|frame| frame.colon_role.is_none());
        if attached_ternary_colon {
            self.ensure_space();
        } else if (source_attached_closing
            || source_attached_operator_after_closing
            || source_attached_symbol_after_closing)
            && !move_one_line_block_comment
        {
        } else if source_attached_number_after_closing && !move_one_line_block_comment {
            self.ensure_space();
        } else if matches!(next, Some(Token::Symbol('\\'))) && !move_one_line_block_comment {
            // A line splice stays at the end of the line it continues.
        } else if move_one_line_block_comment {
            self.finish_line();
            self.unwind_else_if_break_depths();
        } else if matches!(next, Some(Token::Comment(_, _))) {
            self.ensure_space();
            // A comment after the brace ends the line; the code after it
            // decides whether the else-if chain goes on.
            self.layout.unwind_else_if_after_line = !self.closing_brace_precedes_else();
        } else if self.should_attach_closing_header(next)
            || self.should_attach_post_closing_declaration(next)
            || source_attached_word_after_closing
            || (!self.options.break_one_line_statements
                && matches!(next, Some(Token::Word(word))
                    if !(is_attachable_closing_header(word)
                        || word == "while"
                            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do"))))
        {
            self.ensure_space();
        } else if !matches!(
            next,
            Some(Token::Symbol(';') | Token::Symbol(',') | Token::Symbol(')'))
        ) {
            self.finish_line();
            if !self.closing_brace_precedes_else() {
                self.unwind_else_if_break_depths();
            }
        }
        if unmatched_closing_brace {
            self.layout.continuation_indent.set_next_line_spaces(0);
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
            self.layout.frame_stack.clear_stream_frames();
            self.layout.frame_stack.clear_logical_frames();
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
        }
        self.observe_block_spacing_close_brace(matches!(next, Some(Token::Comment(_, _))));
        if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
            && !self.braceless_body_continues(next)
        {
            self.layout.indentation.exit_braceless_block();
            // The braceless body held the else-if chain the brace ended.
            if self.current_is_blank() && !self.closing_brace_precedes_else() {
                self.unwind_else_if_break_depths();
            }
        }
        self.layout.previous = PreviousToken::Other;
    }

    fn braceless_body_continues(&self, next: Option<&Token>) -> bool {
        // An `else` on a later line continues the body when it belongs to
        // the `if` whose block just closed.
        if matches!(next, Some(Token::Newline | Token::Whitespace(_))) {
            let tokens = &self.tree.tokens;
            return self.layout.nesting.last_closed_brace_header.as_deref() == Some("if")
                && self
                    .current
                    .active_token()
                    .and_then(|brace| {
                        (brace + 1..tokens.len()).find(|&index| {
                            !matches!(
                                tokens[index],
                                Token::Newline | Token::Whitespace(_) | Token::Comment(_, _)
                            )
                        })
                    })
                    .is_some_and(
                        |index| matches!(&tokens[index], Token::Word(word) if word == "else"),
                    );
        }
        match next {
            // An else's block ends its chain; an `else` after it belongs to
            // an `if` outside the body.
            Some(Token::Word(word)) if word == "else" => {
                self.layout.nesting.last_closed_brace_header.as_deref() != Some("else")
            }
            Some(Token::Word(word)) if word == "catch" => true,
            Some(Token::Word(word)) if word == "while" => {
                self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")
            }
            _ => false,
        }
    }

    pub(crate) fn unwind_else_if_break_depths_unless_else(&mut self, next: Option<&Token>) {
        // A plain `else` on a later line keeps the chain's levels; an
        // `else if` records its own.
        let else_follows = matches!(next, Some(Token::Word(word)) if word == "else")
            || matches!(
                next,
                Some(Token::Newline | Token::Whitespace(_) | Token::Comment(..)) | None
            ) && self.next_code_is_plain_else();
        if !else_follows {
            self.unwind_else_if_break_depths();
        }
    }

    fn next_code_is_plain_else(&self) -> bool {
        let tokens = &self.tree.tokens;
        let code_after = |from: usize| {
            (from..tokens.len()).find(|&index| {
                !matches!(
                    tokens[index],
                    Token::Newline | Token::Whitespace(_) | Token::Comment(_, _)
                )
            })
        };
        self.current
            .active_token()
            .filter(|&token| token < tokens.len())
            .and_then(|token| code_after(token + 1))
            .filter(|&index| matches!(&tokens[index], Token::Word(word) if word == "else"))
            .is_some_and(|index| {
                code_after(index + 1).is_none_or(
                    |after| !matches!(&tokens[after], Token::Word(word) if word == "if"),
                )
            })
    }

    /// Whether the code after the `}` being closed, past comments, is an
    /// `else`.
    fn closing_brace_precedes_else(&self) -> bool {
        let tokens = &self.tree.tokens;
        self.current
            .active_token()
            .filter(|&brace| brace < tokens.len())
            .and_then(|brace| {
                (brace + 1..tokens.len()).find(|&index| {
                    !matches!(
                        tokens[index],
                        Token::Newline | Token::Whitespace(_) | Token::Comment(_, _)
                    )
                })
            })
            .is_some_and(|index| matches!(&tokens[index], Token::Word(word) if word == "else"))
    }

    pub(crate) fn unwind_else_if_break_depths(&mut self) {
        let depth = self.layout.indentation.indent();
        while self
            .layout
            .else_if_break_depths
            .last()
            .is_some_and(|recorded| *recorded >= depth)
        {
            self.layout.else_if_break_depths.pop();
        }
    }

    pub(super) fn should_attach_closing_header(&self, next: Option<&Token>) -> bool {
        if matches!(next, Some(Token::Word(word)) if word == "while")
            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")
        {
            if self.options.attach_closing_while {
                return true;
            }
            if self.options.brace_style == BraceStyle::None
                && self.layout.line_state.is_one_line_block
                && (self.options.break_one_line_headers
                    || (self.options.break_one_line_blocks
                        && self.options.break_one_line_statements))
            {
                return false;
            }
            return (is_attached_closing_header_style(self.options)
                || self.options.brace_style == BraceStyle::None)
                && !self.options.break_closing_braces
                && !self.options.indent_braces
                && !self.options.indent_blocks;
        }

        let next_is_closing_header =
            matches!(next, Some(Token::Word(word)) if is_attachable_closing_header(word));
        if self.options.brace_style == BraceStyle::None {
            if self.layout.line_state.is_one_line_block
                && (self.options.break_one_line_headers
                    || (self.options.break_one_line_blocks
                        && self.options.break_one_line_statements))
            {
                return false;
            }
            return next_is_closing_header && !self.options.break_closing_braces;
        }
        is_attached_closing_header_style(self.options)
            && !self.options.break_closing_braces
            && !self.options.indent_braces
            && next_is_closing_header
    }

    pub(crate) fn try_attach_leading_closing_header(&mut self, word: &str) -> bool {
        let is_do_while = word == "while"
            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do");
        let allowed = if is_do_while {
            self.options.attach_closing_while
                || (is_attached_closing_header_style(self.options)
                    && !self.options.break_closing_braces
                    && !self.options.indent_braces
                    && !self.options.indent_blocks)
        } else {
            is_attachable_closing_header(word)
                && is_attached_closing_header_style(self.options)
                && !self.options.break_closing_braces
                && !self.options.indent_braces
        };
        if !allowed || !self.current_is_blank() {
            return false;
        }

        // Only a lone `}` joins the header; any other line stays published,
        // with the sources it was published with.
        if self
            .output
            .last()
            .is_none_or(|previous| previous.trimmed() != "}")
        {
            return false;
        }
        let Some(previous) = self.take_last_output_line_for_attach() else {
            return false;
        };
        let previous_trimmed = previous.trimmed();

        self.current.push_str(previous_trimmed);
        self.current.push(' ');
        self.layout.previous = PreviousToken::Other;
        true
    }

    fn take_last_output_line_for_attach(&mut self) -> Option<String> {
        let (line, tokens) = self.output.pop_with_tokens()?;
        self.current.restore_tokens(tokens);
        // The line adjuster sees the brace again with the header.
        if let Some((index, adjuster)) = self.adjuster_before_lone_brace.take()
            && index == self.output.len()
        {
            self.layout.line_adjuster = adjuster;
        }
        Some(line)
    }

    pub(crate) fn split_else_body_closing_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        (self.preprocessor.split_else.extra_indent
            && line.trimmed() == "}"
            && self.layout.indentation.indent() <= self.preprocessor.split_else.brace_indent)
            .then(|| {
                (self.preprocessor.split_else.brace_indent
                    + self.preprocessor.split_else.extra_levels)
                    * self.options.indent_width
                    + self.options.indent_width
            })
    }

    pub(crate) fn split_else_closing_indent_floor(
        &self,
        line: &LineView<'_>,
        indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let active = self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.preprocessor_split_else_active();
        if line.trimmed() != "}" || !active || !self.recent_split_else_closing_context_active() {
            return None;
        }
        let (open_spaces, _, _) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let current = current_spaces.unwrap_or(indent * self.options.indent_width);
        (current < open_spaces).then_some(open_spaces)
    }

    pub(crate) fn nested_closing_brace_indent_reset(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trimmed() == "}"
            && self.options.brace_style != BraceStyle::Ratliff
            && self.layout.indentation.indent() == 1
            && self.layout.line_adjuster.total_case_unindent_depth() == 0
            && current_spaces.is_some_and(|spaces| spaces > output_spaces)
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trimmed() == "}")
            && !self.recent_split_else_closing_context_active())
        .then_some(output_spaces)
    }

    pub(crate) fn root_preprocessor_closing_brace_indent_reset(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trimmed() == "}"
            && self.layout.indentation.indent() == 0
            && current_spaces.is_some_and(|spaces| spaces > output_spaces)
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trimmed_start().starts_with("#endif")))
        .then_some(output_spaces)
    }

    pub(crate) fn nested_if_closing_brace_indent_reset(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trimmed() == "}"
            && self.options.brace_style != BraceStyle::Ratliff
            && self.layout.indentation.indent() == 1
            && self.layout.line_adjuster.total_case_unindent_depth() == 0
            && current_spaces.is_some_and(|spaces| spaces > output_spaces)
            && self
                .output
                .current_closing_brace_open(self.options.tab_width)
                .is_some_and(|(open_spaces, _, open)| {
                    open_spaces == output_spaces && starts_header_word(open, "if")
                }))
        .then_some(output_spaces)
    }

    pub(crate) fn same_line_nested_header_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "}" {
            return None;
        }
        let mut spaces = self
            .output
            .current_closing_brace_open(self.options.tab_width)
            .and_then(|(open_spaces, _, open)| {
                let extra = same_line_nested_header_extra(open);
                (extra > 0).then_some(open_spaces + extra * self.options.indent_width)
            });
        for index in self
            .output
            .scoped_range()
            .rev()
            .filter(|&index| !self.output.trimmed(index).is_empty())
        {
            let code = self.output.code_before_comment_trimmed(index);
            let trimmed = self.output.code_body(index);
            if trimmed == "}" {
                break;
            }
            if code.ends_with('{') {
                let extra = same_line_nested_header_extra(trimmed);
                if extra > 0 {
                    spaces = Some(
                        self.output.lead_width(index, self.options.tab_width)
                            + extra * self.options.indent_width,
                    );
                }
                break;
            }
        }
        spaces
    }

    pub(crate) fn ratliff_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        normal_indent: usize,
    ) -> Option<usize> {
        if line.trimmed() != "}" || self.options.brace_style != BraceStyle::Ratliff {
            return None;
        }
        let (open_spaces, _, open) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if !self.options.indent_namespaces && is_namespace_or_module_block_header(open) {
            return None;
        }
        let semantic_frame = self.layout.frame_stack.last_closed_brace().filter(|frame| {
            frame.semantic_kind == BraceSemanticKind::Command && frame.header.is_some()
        });
        if let Some(frame) = semantic_frame {
            // An `else` or the header leading its line stands where layout
            // put it, past levels the engine lost.
            let header = if starts_header_word(open, "else")
                || frame
                    .header
                    .as_deref()
                    .is_some_and(|header| starts_header_word(open, header))
            {
                open_spaces.max(frame.header_indent_column)
            } else if let Some(header_spaces) = self.current_closing_multiline_header_indent() {
                header_spaces.max(frame.header_indent_column)
            } else {
                frame.header_indent_column
            };
            return Some(header + self.options.indent_width);
        }
        let case_unindent = if starts_header_word(open, "case") || open.starts_with("default:") {
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        } else {
            0
        };
        let structural_spaces = normal_indent * self.options.indent_width;
        Some(
            (open_spaces + same_line_nested_header_extra(open) * self.options.indent_width)
                .min(structural_spaces)
                + self.options.indent_width
                + case_unindent,
        )
    }

    pub(crate) fn same_line_nested_header_closing_brace_indent_floor(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trimmed() != "}" {
            return None;
        }
        for opening in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
        {
            let code = self.output.code_trimmed_of(opening);
            let trimmed = code.trimmed_start();
            if trimmed == "}" {
                return None;
            }
            if code.ends_with('{') {
                let extra = same_line_nested_header_extra(trimmed);
                return (extra > 0).then(|| {
                    let target = leading_visual_width(opening, self.options.tab_width)
                        + extra * self.options.indent_width;
                    current_spaces.unwrap_or(0).max(target)
                });
            }
        }
        None
    }

    pub(crate) fn split_else_none_style_closing_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        split_else_state_active: bool,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trimmed() != "}"
            || self.token_input.token_source_line_indent == 0
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if !((previous_code.ends_with(';') && !previous_code.ends_with("};"))
            || previous_code.trimmed() == "}")
        {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width)
            .saturating_sub(self.options.indent_width);
        (current_spaces.unwrap_or(output_spaces) < target).then_some(target)
    }

    pub(crate) fn none_style_conditional_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trimmed() != "}"
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let mut spaces = None;
        if previous_code.ends_with(';')
            && !previous_code.ends_with("};")
            && let Some(braceless_else) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
        {
            let trimmed = self.output.code_trimmed_of(braceless_else).trimmed_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let target = leading_visual_width(braceless_else, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if current_spaces > target {
                    spaces = Some(target);
                }
            }
        }
        if preprocessor_directive(previous.trimmed_start()) != Some("endif") {
            return spaces;
        }
        let mut branch_depth = 1usize;
        let mut before_branch = None;
        for candidate in self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .filter(|line| !line.trimmed().is_empty())
        {
            if let Some(directive) = preprocessor_directive(candidate.trimmed_start()) {
                match directive {
                    "endif" => branch_depth += 1,
                    "if" | "ifdef" | "ifndef" => {
                        branch_depth = branch_depth.saturating_sub(1);
                        if branch_depth == 0 {
                            continue;
                        }
                    }
                    _ => {}
                }
            }
            if branch_depth == 0 {
                before_branch = Some(candidate);
                break;
            }
        }
        let Some(before_branch) = before_branch else {
            return spaces;
        };
        let code = self.output.code_trimmed_of(before_branch);
        code.ends_with('{')
            .then(|| leading_visual_width(before_branch, self.options.tab_width))
            .or(spaces)
    }

    pub(crate) fn preprocessor_interrupted_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context {
            return None;
        }
        let trimmed = line.trimmed();
        let trimmed_start = line.trimmed_start();
        let isolated = trimmed == "}";
        let attached = trimmed_start.starts_with('}')
            && !isolated
            && !trimmed_start.starts_with("} else")
            && !trimmed_start.starts_with("}else");
        if !isolated && !attached {
            return None;
        }
        let mut depth = 1usize;
        let matching_open = self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .find(|candidate| {
                let code = self.output.code_trimmed_of(candidate);
                let (closes, opens) = line_brace_imbalance(code);
                if isolated
                    && depth == 1
                    && opens > 0
                    && closes > 0
                    && code.trimmed_start().starts_with("} else")
                {
                    return true;
                }
                depth += closes;
                if opens >= depth {
                    return true;
                }
                depth = depth.saturating_sub(opens);
                false
            })?;
        let matching_code = self.output.code_trimmed_of(matching_open);
        if isolated
            && matching_code.trimmed_start().starts_with(')')
            && matching_code.ends_with('{')
            && let Some(spaces) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != matching_open.as_str())
                .skip(1)
                .find_map(|line| {
                    let code = self.output.code_trimmed_of(line);
                    let trimmed = code.trimmed_start();
                    (self.open_paren_column_of(code).is_some()
                        && (starts_header_word(trimmed, "if")
                            || starts_header_word(trimmed, "while")
                            || starts_header_word(trimmed, "for")
                            || trimmed.starts_with("else if")
                            || trimmed.starts_with("} else")))
                    .then_some(leading_visual_width(line, self.options.tab_width))
                })
        {
            return Some(spaces);
        }
        let ratliff_extra = if attached && self.options.brace_style == BraceStyle::Ratliff {
            self.options.indent_width
        } else {
            0
        };
        Some(leading_visual_width(matching_open, self.options.tab_width) + ratliff_extra)
    }

    pub(crate) fn structural_split_else_closing_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        if line.trimmed() != "}" {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        let body_spaces = if structural_split_else_chain {
            self.current_closing_multiline_header_indent()
                .map(|spaces| spaces + self.options.indent_width)
                .unwrap_or(open_spaces + self.options.indent_width)
        } else {
            open_spaces + self.options.indent_width
        };
        let case_unindent_spaces =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let recent_adjacent_string_call =
            self.output.scoped_range().rev().take(8).any(|index| {
                let code = self.output.code_before_comment_trimmed(index);
                code.ends_with(");") && starts_string_literal_token(code.trimmed_start())
            }) && self.output.scoped_range().rev().take(8).any(|index| {
                let code = self.output.code_before_comment_trimmed(index);
                self.open_paren_column_of(code).is_some()
                    && !starts_string_literal_token(code.trimmed_start())
                    && !code.ends_with(';')
            });
        let split_else_chain =
            structural_split_else_chain || self.output.recent_scoped_else_line(128);
        if structural_split_else_chain
            && !open_trimmed.starts_with("} else")
            && !open_trimmed.starts_with("}else")
            && (previous_code.ends_with(';') || previous_code.trimmed() == "}")
            && let Some(spaces) = self.current_closing_multiline_header_indent()
        {
            return Some(spaces + case_unindent_spaces);
        }
        if self.layout.line_adjuster.total_case_unindent_depth() == 0
            && recent_adjacent_string_call
            && (previous_code.trimmed() == "}" || previous_code.ends_with(';'))
            && previous_spaces == body_spaces
            && current_spaces != open_spaces
        {
            return Some(open_spaces);
        }
        if structural_split_else_chain
            && (open_trimmed.starts_with("} else") || open_trimmed.starts_with("}else"))
            && previous_code.ends_with(';')
            && current_spaces != open_spaces
        {
            return Some(open_spaces);
        }
        if self.layout.line_adjuster.total_case_unindent_depth() == 0
            && previous.trimmed_end().ends_with(':')
            && !previous_code.contains('?')
        {
            return Some(open_spaces);
        }
        (split_else_chain
            && previous_code.trimmed() == "}"
            && (starts_header_word(open_trimmed, "switch")
                || starts_header_word(open_trimmed, "if")
                || starts_header_word(open_trimmed, "for")
                || starts_header_word(open_trimmed, "while")
                || open_trimmed.starts_with("} else")
                || open_trimmed.starts_with("}else"))
            && current_spaces < open_spaces)
            .then_some(open_spaces)
    }

    pub(crate) fn split_else_case_closing_indent_floor(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let case_unindent_spaces =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if !split_else_context
            || !line.trimmed_start().starts_with('}')
            || case_unindent_spaces == 0
        {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if open_trimmed.starts_with("switch")
            || self
                .layout
                .frame_stack
                .last_closed_brace()
                .is_some_and(|frame| frame.header.as_deref() == Some("switch"))
            || self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trimmed() == "}")
                && (open_trimmed.starts_with("case ") || open_trimmed.starts_with("default:"))
        {
            return None;
        }
        let target = open_spaces + case_unindent_spaces;
        (current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(crate) fn split_else_closing_indent_ceiling(
        &self,
        line: &LineView<'_>,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || line.trimmed() != "}"
            || self.layout.line_adjuster.total_case_unindent_depth() != 0
        {
            return None;
        }
        let current = current_spaces?;
        let (open_spaces, _, _) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        (current > open_spaces).then_some(open_spaces)
    }

    pub(crate) fn preprocessor_directive_closing_indent_spaces(
        &self,
        line: &LineView<'_>,
        indent: usize,
    ) -> Option<usize> {
        if line.trimmed() != "}"
            || self
                .output
                .last_line_outside_comment()
                .is_none_or(|previous| preprocessor_directive(previous.trimmed_start()).is_none())
        {
            return None;
        }
        let (open_spaces, _, _) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let natural = indent * self.options.indent_width;
        (open_spaces > natural).then(|| {
            self.current_closing_multiline_header_indent()
                .unwrap_or(open_spaces)
        })
    }

    pub(crate) fn recent_split_else_command_closing_indent_spaces(
        &self,
        line: &LineView<'_>,
        indent: usize,
        current_spaces: Option<usize>,
        recent_split_else_chain: impl FnOnce() -> bool,
    ) -> Option<usize> {
        if line.trimmed() != "}" || !recent_split_else_chain() {
            return None;
        }
        let frame = self.layout.frame_stack.active_brace()?;
        let natural = indent * self.options.indent_width;
        if frame.semantic_kind != BraceSemanticKind::Command
            || frame.sibling_indent_column <= natural
        {
            return None;
        }
        let (open_spaces, _, _) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let target = self
            .current_closing_multiline_header_indent()
            .unwrap_or(open_spaces);
        let current = current_spaces.unwrap_or(natural);
        (current_spaces.is_none() || current > target).then_some(target)
    }

    fn should_attach_post_closing_declaration(&self, next: Option<&Token>) -> bool {
        matches!(
            self.layout.nesting.last_closed_brace_type,
            Some(
                BraceType::Class
                    | BraceType::Interface
                    | BraceType::Struct
                    | BraceType::Union
                    | BraceType::Enum,
            )
        ) && match next {
            Some(Token::Word(_)) | Some(Token::Symbol('[')) => true,
            Some(Token::Operator(op)) => matches!(op.as_str(), "*" | "&" | "^"),
            _ => false,
        }
    }
}

pub(super) fn is_attached_closing_header_style(options: &FormatOptions) -> bool {
    matches!(
        options.brace_style,
        BraceStyle::Attach | BraceStyle::OneTrueBrace | BraceStyle::Ratliff
    )
}

pub(crate) fn top_level_closing_brace_indent_spaces(
    options: &FormatOptions,
    line: &str,
    normal_indent: usize,
) -> Option<usize> {
    (line.trimmed() == "}" && normal_indent == 0 && !options.indent_braces).then_some(0)
}
