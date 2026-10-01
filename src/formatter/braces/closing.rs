use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::braces::classification::{
    ExternCGuard, is_lambda_body_header, is_namespace_or_module_block_header,
};
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
    line_brace_imbalance, preprocessor_directive, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::formatter::tokens::literals::starts_string_literal_token;
use crate::formatter::tokens::operators::head_ends_binary_operator;

pub(crate) fn starts_post_closing_declaration(line: &str) -> bool {
    let Some(tail) = line.trim_start().strip_prefix('}') else {
        return false;
    };
    let tail = tail.trim_start();
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
    let leading = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
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
    pub(crate) fn compound_closing_brace_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with("}, ") {
            return None;
        }
        let mut closed_blocks = 0usize;
        for opening in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = opening[..trailing_comment_split_limit(opening)].trim_end();
            let opens = code.chars().filter(|ch| *ch == '{').count();
            let closes = code.chars().filter(|ch| *ch == '}').count();
            if opens > closes + closed_blocks && !code.trim_start().starts_with('{') {
                return Some(leading_visual_width(opening, self.options.tab_width));
            }
            // A sibling `}, {` opened the element this line closes.
            if closed_blocks == 0 && code.trim_start().starts_with('}') && code.ends_with('{') {
                return Some(leading_visual_width(opening, self.options.tab_width));
            }
            closed_blocks += closes;
            closed_blocks = closed_blocks.saturating_sub(opens);
        }
        None
    }

    pub(crate) fn unmatched_closing_brace_indent_spaces(&self, line: &str) -> Option<usize> {
        (line.trim() == "}" && self.layout.nesting.last_closed_brace_type.is_none()).then_some(0)
    }

    fn isolated_opening_brace_indent_from_output(&self) -> Option<usize> {
        if self.options.brace_style == BraceStyle::Horstmann {
            return None;
        }
        let tab_width = self.options.tab_width;
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
            depth += meta.closes;
            if meta.opens > depth {
                return match meta.open_shape {
                    OpenBraceShape::Isolated => Some(self.output.lead_width(index, tab_width)),
                    OpenBraceShape::Label => {
                        Some(self.layout.indentation.indent() * self.options.indent_width)
                    }
                    OpenBraceShape::Other => None,
                };
            }
            depth -= meta.opens;
        }
        None
    }

    pub(crate) fn isolated_closing_brace_indent_spaces(
        &self,
        line: &str,
        case_unindent_closing_line: bool,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !(line.trim() == "}" || trimmed.starts_with("} else") || trimmed.starts_with("}else"))
            || case_unindent_closing_line
            || (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
                && self
                    .output
                    .last_line_outside_comment()
                    .is_some_and(|line| preprocessor_directive(line.trim_start()).is_some())
        {
            return None;
        }
        self.isolated_opening_brace_indent_from_output()
    }

    pub(crate) fn align_isolated_closing_brace_line(&self, line: String) -> String {
        let line_start = line.trim_start();
        if !(line_start == "}" || line_start.starts_with("} else"))
            || self.isolated_opening_brace_is_switch_label()
        {
            return line;
        }
        let Some(mut spaces) = self.isolated_opening_brace_indent_from_output() else {
            return line;
        };
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
        let current = leading_visual_width(&line, self.options.tab_width);
        if current < spaces || structural_switch_indent.is_some() && current != spaces {
            format!("{}{}", " ".repeat(spaces), line_start)
        } else {
            line
        }
    }

    pub(crate) fn continuation_adjacent_closing_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Horstmann
                | BraceStyle::Pico
        ) || !line.trim_start().starts_with('}')
        {
            return None;
        }
        for (index, previous) in self.output.iter().enumerate().rev() {
            if previous.trim() != "{" {
                continue;
            }
            let before_open = self.output[..index]
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())?;
            let code = before_open[..trailing_comment_split_limit(before_open)].trim_end();
            return (head_ends_binary_operator(code) || code.ends_with("->"))
                .then(|| leading_visual_width(previous, self.options.tab_width));
        }
        None
    }

    pub(crate) fn gnu_command_closing_brace_indent_spaces(&self, line: &str) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Gnu || line.trim() != "}" {
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

    pub(crate) fn ratliff_command_closing_header_indent_spaces(&self, line: &str) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Ratliff || !line.trim_start().starts_with("} ") {
            return None;
        }
        self.layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Command)
            .map(|frame| frame.header_indent_column + self.options.indent_width)
    }

    pub(crate) fn lambda_closing_brace_indent_spaces(&self, line: &str) -> Option<usize> {
        if !line.trim_start().starts_with('}') {
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
            let current_ends_open = self.current.trim_end().ends_with('{');
            let previous_ends_open = self
                .output
                .last()
                .is_some_and(|line| line.trim_end().ends_with('{'));
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
                    previous.truncate(previous.trim_end().len());
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
            && self
                .current
                .trim_end()
                .ends_with(['+', '-', '*', '/', '%', '&', '|', '!', '~'])
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
        if attached_statement_expression {
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
        let move_one_line_block_comment = self.options.break_one_line_blocks
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
                    line.trim() == "{"
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
            .any(|line| is_lambda_body_header(line.trim_end()));
        if should_indent_closing_brace || self.layout.nesting.last_closed_brace_extra_indent > 0 {
            if closing_brace_is_lambda && matches!(next, Some(Token::Symbol(';' | ','))) {
                self.layout.continuation_indent.next_line_indent =
                    Some(self.layout.indentation.indent());
            } else {
                self.layout.continuation_indent.next_line_indent = Some(
                    self.layout.indentation.indent()
                        + 1
                        + self.case_body_indent_extra(LineKind::Normal),
                );
            }
            self.layout.continuation_indent.next_line_indent_spaces = None;
        } else if matches!(next, Some(Token::Symbol(';'))) {
            self.layout.continuation_indent.next_line_indent = Some(
                self.layout.indentation.indent() + self.case_body_indent_extra(LineKind::Normal),
            );
        } else if matches!(next, Some(Token::Symbol(','))) {
            self.layout.continuation_indent.next_line_indent =
                Some(self.layout.indentation.indent());
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
                .rposition(|line| {
                    line[..trailing_comment_split_limit(line)]
                        .trim_end()
                        .ends_with('{')
                })
                .and_then(|open_index| {
                    (open_index + 1..self.output.len())
                        .find(|index| !self.output[*index].trim().is_empty())
                })
                .or_else(|| self.output.len().checked_sub(1));
            if let Some(line) = target_index.and_then(|index| self.output.get_mut(index)) {
                line.push_str(whitespace_before_brace);
                line.push_str("   ");
                line.push_str(comment.trim_end());
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
        } else if move_one_line_block_comment {
            self.finish_line();
            self.unwind_else_if_break_depths();
        } else if self.should_attach_closing_header(next)
            || self.should_attach_post_closing_declaration(next)
            || source_attached_word_after_closing
            || matches!(next, Some(Token::Comment(_, _)))
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
            self.unwind_else_if_break_depths_unless_else(next);
        }
        if unmatched_closing_brace {
            self.layout.continuation_indent.set_next_line_spaces(0);
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
            self.layout.frame_stack.clear_stream_frames();
            self.layout.frame_stack.clear_logical_frames();
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
        }
        self.observe_block_spacing_close_brace();
        if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
            && !self.braceless_body_continues(next)
        {
            self.layout.indentation.exit_braceless_block();
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
            Some(Token::Word(word)) if word == "else" || word == "catch" => true,
            Some(Token::Word(word)) if word == "while" => {
                self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")
            }
            _ => false,
        }
    }

    pub(crate) fn unwind_else_if_break_depths_unless_else(&mut self, next: Option<&Token>) {
        if !matches!(next, Some(Token::Word(word)) if word == "else") {
            self.unwind_else_if_break_depths();
        }
    }

    fn unwind_else_if_break_depths(&mut self) {
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
            .is_none_or(|previous| previous.trim() != "}")
        {
            return false;
        }
        let Some(previous) = self.take_last_output_line_for_attach() else {
            return false;
        };
        let previous_trimmed = previous.trim();

        self.current.push_str(previous_trimmed);
        self.current.push(' ');
        self.layout.previous = PreviousToken::Other;
        true
    }

    fn take_last_output_line_for_attach(&mut self) -> Option<String> {
        let (line, tokens) = self.output.pop_with_tokens()?;
        self.current.restore_tokens(tokens);
        Some(line)
    }

    pub(crate) fn split_else_body_closing_indent_spaces(&self, line: &str) -> Option<usize> {
        (self.preprocessor.split_else.extra_indent
            && line.trim() == "}"
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
        line: &str,
        indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let active = self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.preprocessor_split_else_active();
        if line.trim() != "}" || !active || !self.recent_split_else_closing_context_active() {
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
        line: &str,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trim() == "}"
            && self.options.brace_style != BraceStyle::Ratliff
            && self.layout.indentation.indent() == 1
            && self.layout.line_adjuster.total_case_unindent_depth() == 0
            && current_spaces.is_some_and(|spaces| spaces > output_spaces)
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trim() == "}")
            && !self.recent_split_else_closing_context_active())
        .then_some(output_spaces)
    }

    pub(crate) fn root_preprocessor_closing_brace_indent_reset(
        &self,
        line: &str,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trim() == "}"
            && self.layout.indentation.indent() == 0
            && current_spaces.is_some_and(|spaces| spaces > output_spaces)
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| previous.trim_start().starts_with("#endif")))
        .then_some(output_spaces)
    }

    pub(crate) fn nested_if_closing_brace_indent_reset(
        &self,
        line: &str,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        (line.trim() == "}"
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
        line: &str,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let mut spaces = self
            .output
            .current_closing_brace_open(self.options.tab_width)
            .and_then(|(open_spaces, _, open)| {
                let extra = same_line_nested_header_extra(open);
                (extra > 0).then_some(open_spaces + extra * self.options.indent_width)
            });
        for opening in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = opening[..trailing_comment_split_limit(opening)].trim_end();
            let trimmed = code.trim_start();
            if trimmed == "}" {
                break;
            }
            if code.ends_with('{') {
                let extra = same_line_nested_header_extra(trimmed);
                if extra > 0 {
                    spaces = Some(
                        leading_visual_width(opening, self.options.tab_width)
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
        line: &str,
        normal_indent: usize,
    ) -> Option<usize> {
        if line.trim() != "}" || self.options.brace_style != BraceStyle::Ratliff {
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
            return Some(frame.header_indent_column + self.options.indent_width);
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
        line: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        for opening in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = opening[..trailing_comment_split_limit(opening)].trim_end();
            let trimmed = code.trim_start();
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
        line: &str,
        line_kind: LineKind,
        split_else_state_active: bool,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim() != "}"
            || self.token_input.token_source_line_indent == 0
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !((previous_code.ends_with(';') && !previous_code.ends_with("};"))
            || previous_code.trim() == "}")
        {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width)
            .saturating_sub(self.options.indent_width);
        (current_spaces.unwrap_or(output_spaces) < target).then_some(target)
    }

    pub(crate) fn none_style_conditional_closing_brace_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim() != "}"
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let mut spaces = None;
        if previous_code.ends_with(';')
            && !previous_code.ends_with("};")
            && let Some(braceless_else) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
        {
            let trimmed = braceless_else[..trailing_comment_split_limit(braceless_else)]
                .trim_end()
                .trim_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let target = leading_visual_width(braceless_else, self.options.tab_width)
                    .saturating_sub(self.options.indent_width);
                if current_spaces > target {
                    spaces = Some(target);
                }
            }
        }
        if preprocessor_directive(previous.trim_start()) != Some("endif") {
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
            .filter(|line| !line.trim().is_empty())
        {
            if let Some(directive) = preprocessor_directive(candidate.trim_start()) {
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
        let code = before_branch[..trailing_comment_split_limit(before_branch)].trim_end();
        code.ends_with('{')
            .then(|| leading_visual_width(before_branch, self.options.tab_width))
            .or(spaces)
    }

    pub(crate) fn preprocessor_interrupted_closing_brace_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context {
            return None;
        }
        let trimmed = line.trim();
        let trimmed_start = line.trim_start();
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
            .filter(|line| !line.trim().is_empty())
            .find(|candidate| {
                let code = candidate[..trailing_comment_split_limit(candidate)].trim_end();
                let (closes, opens) = line_brace_imbalance(code);
                if isolated
                    && depth == 1
                    && opens > 0
                    && closes > 0
                    && code.trim_start().starts_with("} else")
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
        let matching_code = matching_open[..trailing_comment_split_limit(matching_open)].trim_end();
        if isolated
            && matching_code.trim_start().starts_with(')')
            && matching_code.ends_with('{')
            && let Some(spaces) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != matching_open.as_str())
                .skip(1)
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    (unmatched_open_paren_column(code).is_some()
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
        Some(leading_visual_width(matching_open, self.options.tab_width))
    }

    pub(crate) fn structural_split_else_closing_brace_indent_spaces(
        &self,
        line: &str,
        current_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
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
            self.output.scoped().iter().rev().take(8).any(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.ends_with(");") && starts_string_literal_token(code.trim_start())
            }) && self.output.scoped().iter().rev().take(8).any(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                unmatched_open_paren_column(code).is_some()
                    && !starts_string_literal_token(code.trim_start())
                    && !code.ends_with(';')
            });
        let split_else_chain = structural_split_else_chain
            || self
                .output
                .scoped()
                .iter()
                .rev()
                .take(128)
                .any(|line| line.trim() == "else" || line.trim_end().ends_with("} else"));
        if structural_split_else_chain
            && !open_trimmed.starts_with("} else")
            && !open_trimmed.starts_with("}else")
            && (previous_code.ends_with(';') || previous_code.trim() == "}")
            && let Some(spaces) = self.current_closing_multiline_header_indent()
        {
            return Some(spaces + case_unindent_spaces);
        }
        if self.layout.line_adjuster.total_case_unindent_depth() == 0
            && recent_adjacent_string_call
            && (previous_code.trim() == "}" || previous_code.ends_with(';'))
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
            && previous.trim_end().ends_with(':')
            && !previous_code.contains('?')
        {
            return Some(open_spaces);
        }
        (split_else_chain
            && previous_code.trim() == "}"
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
        line: &str,
        split_else_context: bool,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let case_unindent_spaces =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if !split_else_context || !line.trim_start().starts_with('}') || case_unindent_spaces == 0 {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if open_trimmed.starts_with("switch")
            || self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trim() == "}")
                && (open_trimmed.starts_with("case ") || open_trimmed.starts_with("default:"))
        {
            return None;
        }
        let target = open_spaces + case_unindent_spaces;
        (current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(crate) fn split_else_closing_indent_ceiling(
        &self,
        line: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || line.trim() != "}"
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
        line: &str,
        indent: usize,
    ) -> Option<usize> {
        if line.trim() != "}"
            || self
                .output
                .last_line_outside_comment()
                .is_none_or(|previous| preprocessor_directive(previous.trim_start()).is_none())
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
        line: &str,
        indent: usize,
        current_spaces: Option<usize>,
        recent_split_else_chain: bool,
    ) -> Option<usize> {
        if !recent_split_else_chain || line.trim() != "}" {
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
    (line.trim() == "}" && normal_indent == 0 && !options.indent_braces).then_some(0)
}
