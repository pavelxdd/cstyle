use super::assembly::is_asm_block_header;
use super::brace_classification::{
    is_class_like_brace_type, is_lambda_body_header, is_lambda_capture_header,
};
use super::columns::leading_visual_width;
use super::compound_literals::line_ends_compound_literal_cast;
use super::indentation::LineKind;
use super::initializer_braces::bracket_starts_initializer_designator;
use super::language;
use super::language::is_macro_like_word;
use super::line_scan::{
    has_unmatched_open_brace, line_ends_with_comment, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use super::preprocessor::{is_conditional_preprocessor, is_known_preprocessor_directive};

use super::state::{FormatterBraceType, PreviousToken, TemplateAngle};
use super::syntax::template_angle_role;
use super::token::{
    CommentKind, Token, matching_close_paren_index, next_non_layout_token_index,
    next_non_whitespace, token_text,
};
use super::{FormatEngine, TokenPushContext};
use crate::config::{BraceStyle, FormatOptions, IndentStyle};
use crate::source::lex::{is_identifier_continue, is_word_char, trailing_word};

impl FormatEngine<'_> {
    pub(super) fn try_add_braces_to_statement(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        start: usize,
        line_end: usize,
    ) -> Option<usize> {
        if !(self.options.add_braces || self.options.add_one_line_braces) {
            return None;
        }
        if self.command_state.preprocessor_after_header {
            if self.stack_state.paren_depth > 0
                || matches!(tokens.get(start), Some(Token::Symbol('{')))
            {
                return None;
            }
            if matches!(
                tokens.get(start),
                Some(
                    Token::Whitespace(_)
                        | Token::Newline
                        | Token::Preprocessor(_)
                        | Token::Comment(_, _)
                )
            ) {
                return None;
            }
            let body_indent = self
                .state
                .indent()
                .max(self.pending_braceless_block_bias.unwrap_or(0))
                + 1;
            self.continuation_indent.next_line_indent = Some(body_indent);
            self.continuation_indent.next_line_indent_spaces = None;
            self.pending_braceless_block_bias = Some(body_indent);
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            return None;
        }
        let header = self.command_state.current_header.as_deref()?;
        if !is_add_braces_header(header)
            || is_defer_header(header)
            || token_range_has_line_comment(tokens, line_start, start)
        {
            return None;
        }
        let header_is_else = header == "else";
        let header_is_do = header == "do";
        if matches!(header, "if" | "for" | "while")
            && (self.command_state.previous_command_char != Some(')')
                || self.stack_state.paren_depth > 0)
        {
            return None;
        }
        let statement_start = next_non_whitespace(tokens, start, line_end)?;
        match tokens.get(statement_start)? {
            Token::Symbol('(')
            | Token::Symbol('{')
            | Token::Symbol(';')
            | Token::Comment(_, _)
            | Token::Preprocessor(_)
            | Token::Newline => return None,
            Token::Word(word) if self.is_header(word) => return None,
            _ => {}
        }

        let semicolon = find_statement_semicolon(tokens, statement_start, line_end)?;
        if tokens[statement_start..semicolon]
            .iter()
            .any(|token| matches!(token, Token::Newline))
        {
            return None;
        }
        if !self.options.break_one_line_headers
            && (self.options.add_one_line_braces || !self.options.break_one_line_blocks)
            && !self.options.lisp_add_one_line_braces_breaks_blocks()
        {
            let statement_starts_line = token_begins_line(tokens, statement_start);
            if statement_starts_line && !self.current_is_blank() {
                let brace_indent_extra = usize::from(
                    self.options.indent_braces || self.options.brace_style == BraceStyle::Gnu,
                );
                let block_indent = self
                    .state
                    .indent()
                    .max(self.pending_braceless_block_bias.unwrap_or(0))
                    + brace_indent_extra;
                self.finish_line();
                self.continuation_indent.next_line_indent = Some(block_indent);
                self.continuation_indent.next_line_indent_spaces = None;
            }
            let mut block_tokens = Vec::with_capacity(semicolon - statement_start + 5);
            block_tokens.push(Token::Symbol('{'));
            let body_gap = if statement_starts_line && self.options.brace_style == BraceStyle::Pico
            {
                " ".repeat(self.options.indent_width.saturating_sub(1))
            } else {
                " ".to_string()
            };
            block_tokens.push(Token::Whitespace(body_gap));
            block_tokens.extend_from_slice(&tokens[statement_start..=semicolon]);
            block_tokens.push(Token::Whitespace(" ".to_string()));
            block_tokens.push(Token::Symbol('}'));
            self.push_attached_one_line_block(
                &block_tokens,
                FormatterBraceType::Command,
                None::<&str>,
                None::<&str>,
                false,
                None,
            );
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            if header_is_do {
                self.stack_state.last_closed_brace_header = Some("do".to_string());
            }
            let next = next_statement_token(tokens, semicolon + 1, tokens.len(), true)
                .and_then(|next_index| tokens.get(next_index));
            let same_line_next = next_statement_token(tokens, semicolon + 1, line_end, false)
                .and_then(|next_index| tokens.get(next_index));
            let next_is_else = matches!(next, Some(Token::Word(word)) if word == "else");
            let next_is_closing_while = header_is_do
                && matches!(same_line_next, Some(Token::Word(word)) if word == "while");
            let next_is_closing_header = next_is_else || next_is_closing_while;
            let attach_closing_header = self.should_attach_closing_header(next);
            let keep_following_statement = !self.options.break_one_line_statements
                && same_line_next.is_some()
                && (!next_is_closing_header || attach_closing_header);
            let nested_header_level = self.inline_nested_header_braceless_bias.take();
            if (next_is_else
                && attach_closing_header
                && (nested_header_level.is_none() || !self.options.break_one_line_statements))
                || keep_following_statement
            {
                self.ensure_space();
            } else {
                self.finish_line();
            }
            if self.current_is_blank() {
                if let Some(level) = nested_header_level {
                    let delta = level.saturating_sub(self.state.indent());
                    if delta > 0 {
                        self.state.enter_braceless_block(delta);
                    }
                }
            } else if matches!(next, Some(Token::Word(word)) if word == "else") {
                self.inline_nested_header_braceless_bias = nested_header_level;
            }
            if header_is_else && !matches!(next, Some(Token::Word(word)) if word == "else") {
                while let Some((base, delta)) = self.state.last_braceless_block()
                    && self.state.indent() == base + delta
                {
                    self.state.exit_braceless_block();
                    if self.frame_stack.active_braceless_header().is_some() {
                        self.frame_stack.pop_braceless_header();
                    }
                }
            }
        } else {
            self.token_input.token_begins_source_line = false;
            self.line_state.is_one_line_block = true;
            self.push_open_brace(None, usize::MAX, false);
            self.push_replayed_statement(
                tokens,
                statement_start,
                semicolon,
                None,
                statement_start,
                None,
            );
            let next = next_statement_token(tokens, semicolon + 1, tokens.len(), true)
                .and_then(|next_index| tokens.get(next_index));
            self.token_input.previous_input_whitespace = Some(" ".to_string());
            self.push_close_brace(next, false);
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            if header_is_else && !matches!(next, Some(Token::Word(word)) if word == "else") {
                while let Some((base, delta)) = self.state.last_braceless_block()
                    && self.state.indent() == base + delta
                {
                    self.state.exit_braceless_block();
                    if self.frame_stack.active_braceless_header().is_some() {
                        self.frame_stack.pop_braceless_header();
                    }
                }
            }
        }
        Some(semicolon + 1)
    }

    pub(super) fn try_remove_braces_from_statement(
        &mut self,
        tokens: &[Token],
        start: usize,
        line_end: usize,
    ) -> Option<usize> {
        if !self.options.remove_braces
            || !matches!(tokens.get(start), Some(Token::Symbol('{')))
            || !is_remove_braces_opening(tokens, start)
        {
            return None;
        }
        let header = self.command_state.current_header.as_deref()?;
        if !is_remove_braces_header(header) || is_defer_header(header) {
            return None;
        }
        let (statement_start, semicolon, close_index) =
            removable_statement_brace_range(tokens, start, line_end, false)?;
        let block_starts_line = token_begins_line(tokens, start);
        let removed_opening_gap =
            (!self.options.break_one_line_blocks && !block_starts_line).then(|| {
                let mut gap = match tokens.get(start.wrapping_sub(1)) {
                    Some(Token::Whitespace(whitespace)) => whitespace.clone(),
                    _ => String::new(),
                };
                for token in &tokens[start + 1..statement_start] {
                    if let Token::Whitespace(whitespace) = token {
                        gap.push_str(whitespace);
                    }
                }
                gap
            });
        if self.options.break_one_line_blocks || block_starts_line {
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.command_state.current_header = None;
        }
        let following_index = next_statement_token(tokens, close_index + 1, line_end, false);
        let following = following_index.and_then(|index| tokens.get(index));
        let following_is_header =
            matches!(following, Some(Token::Word(word)) if self.is_header(word));
        let keep_body_with_following = following.is_some()
            && self.options.keeps_multi_statement_line()
            && !(self.options.break_one_line_headers && following_is_header);
        if keep_body_with_following {
            self.line_state.is_multi_statement_line = true;
            self.line_state.is_one_line_block = false;
        }
        self.push_replayed_statement(
            tokens,
            statement_start,
            semicolon,
            following,
            start,
            removed_opening_gap.as_deref(),
        );
        if keep_body_with_following {
            self.trim_current_end_horizontal_space();
            for token in &tokens[semicolon + 1..following_index.unwrap_or(close_index + 1)] {
                match token {
                    Token::Whitespace(whitespace) => self.current.push_str(whitespace),
                    Token::Symbol('}') => self.current.push(' '),
                    _ => {}
                }
            }
        }
        if !keep_body_with_following
            && matches!(
                self.options.brace_style,
                BraceStyle::Pico | BraceStyle::Lisp
            )
            && following_index.is_none()
            && next_statement_token(tokens, close_index + 1, tokens.len(), true)
                .is_some_and(|index| matches!(tokens.get(index), Some(Token::Symbol('}'))))
        {
            let closing_gap = tokens[semicolon + 1..close_index]
                .iter()
                .filter_map(|token| match token {
                    Token::Whitespace(whitespace) => Some(whitespace.as_str()),
                    _ => None,
                })
                .collect::<String>();
            if self.current_is_blank() {
                if let Some(previous) = self.output.last_mut() {
                    previous.truncate(previous.trim_end().len());
                    previous.push_str(&closing_gap);
                }
            } else {
                self.trim_current_end_horizontal_space();
                self.current.push_str(&closing_gap);
                self.preserve_run_in_join_space = true;
            }
        }
        Some(close_index + 1)
    }

    pub(super) fn try_break_one_line_header(
        &mut self,
        tokens: &[Token],
        line_start: usize,
        start: usize,
        line_end: usize,
    ) -> bool {
        let split_else_after_preprocessor = self.is_after_preprocessor_split_else()
            && self.preprocessor.split_else.trigger_output_len != Some(self.output.len());
        if !split_else_after_preprocessor && !self.options.break_one_line_headers
            || token_range_has_line_comment(tokens, line_start, start)
        {
            return false;
        }
        let header = self.command_state.current_header.as_deref();
        if !split_else_after_preprocessor {
            let Some(header) = header else {
                return false;
            };
            if !(is_add_braces_header(header) || header == "switch") || is_defer_header(header) {
                return false;
            }
        }
        if matches!(header, Some("if" | "for" | "while" | "switch"))
            && (self.command_state.previous_command_char != Some(')')
                || self.stack_state.paren_depth > 0)
        {
            return false;
        }
        let Some(statement_start) = next_non_whitespace(tokens, start, line_end) else {
            return false;
        };
        let header_brace_depth = tokens[line_start..start]
            .iter()
            .fold(0usize, |depth, token| match token {
                Token::Symbol('{') => depth + 1,
                Token::Symbol('}') => depth.saturating_sub(1),
                _ => depth,
            });
        let preceding_top_level_statement = {
            let mut brace_depth = 0usize;
            let mut paren_depth = 0usize;
            tokens[line_start..start].iter().any(|token| match token {
                Token::Symbol('{') => {
                    brace_depth += 1;
                    false
                }
                Token::Symbol('}') => {
                    brace_depth = brace_depth.saturating_sub(1);
                    false
                }
                Token::Symbol('(') => {
                    paren_depth += 1;
                    false
                }
                Token::Symbol(')') => {
                    paren_depth = paren_depth.saturating_sub(1);
                    false
                }
                Token::Symbol(';') => brace_depth == header_brace_depth && paren_depth == 0,
                _ => false,
            })
        };
        let keeps_multi_statement_line = self.options.keeps_multi_statement_line()
            && (preceding_top_level_statement
                || find_statement_semicolon(tokens, statement_start, line_end)
                    .and_then(|semicolon| {
                        next_statement_token(tokens, semicolon + 1, line_end, false)
                    })
                    .is_some_and(|index| !matches!(tokens.get(index), Some(Token::Symbol('}')))));
        if keeps_multi_statement_line {
            return false;
        }
        if split_else_after_preprocessor
            || (self.command_state.preprocessor_after_header && header == Some("else"))
        {
            match tokens.get(statement_start) {
                Some(Token::Word(word)) if self.is_header(word) => {
                    let header_body_braceless =
                        header_body_start(tokens, statement_start, line_end).is_some_and(|index| {
                            !matches!(tokens.get(index), Some(Token::Symbol('{')))
                        });
                    let header_has_else = word == "if"
                        && find_statement_semicolon(tokens, statement_start, tokens.len())
                            .and_then(|index| next_non_layout_token_index(tokens, index + 1))
                            .is_some_and(|index| {
                                matches!(tokens.get(index), Some(Token::Word(next)) if next == "else")
                            });
                    self.finish_line();
                    self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
                    self.continuation_indent.next_line_indent_spaces = None;
                    self.preprocessor.split_else.extra_indent = true;
                    self.preprocessor.split_else.extra_levels += 1;
                    self.preprocessor.split_else.pending_body = false;
                    self.preprocessor.split_else.trigger_output_len = Some(self.output.len());
                    self.preprocessor.split_else.body_braceless =
                        header_body_braceless && !header_has_else;
                    self.preprocessor.split_else.brace_indent = self.state.indent();
                    self.command_state.current_header = None;
                    self.command_state.preprocessor_after_header = false;
                    return true;
                }
                Some(Token::Symbol('{')) if self.preprocessor.split_else.extra_indent => {
                    self.finish_line();
                    self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
                    self.continuation_indent.next_line_indent_spaces = None;
                    self.preprocessor.split_else.pending_body = false;
                    self.preprocessor.split_else.trigger_output_len = Some(self.output.len());
                    self.command_state.preprocessor_after_header = false;
                    return true;
                }
                _ => {}
            }
        }
        match tokens.get(statement_start) {
            Some(Token::Symbol('{')) if self.options.brace_style == BraceStyle::Pico => {
                if next_non_whitespace(tokens, statement_start + 1, line_end).is_some_and(|index| {
                    matches!(
                        tokens.get(index),
                        Some(Token::Comment(CommentKind::Line, _))
                    )
                }) {
                    return false;
                }
                let header_indent = self
                    .state
                    .indent()
                    .max(self.pending_braceless_block_bias.unwrap_or(0));
                self.finish_line();
                self.continuation_indent.next_line_indent = Some(header_indent);
                self.continuation_indent.next_line_indent_spaces = None;
                return false;
            }
            Some(Token::Symbol('{') | Token::Symbol(';'))
            | Some(Token::Comment(_, _) | Token::Preprocessor(_) | Token::Newline) => return false,
            Some(Token::Word(word))
                if is_add_braces_header(word)
                    && !is_defer_header(word)
                    && (header != Some("else") || word != "if")
                    && !split_else_after_preprocessor =>
            {
                let header_indent = self
                    .continuation_indent
                    .next_line_indent
                    .unwrap_or_else(|| self.state.indent())
                    .max(self.state.indent() + self.case_body_indent_extra(LineKind::Normal))
                    .max(self.pending_braceless_block_bias.unwrap_or(0));
                self.finish_line();
                self.continuation_indent.next_line_indent = Some(header_indent + 1);
                self.continuation_indent.next_line_indent_spaces = None;
                self.pending_braceless_block_bias = Some(header_indent + 1);
                self.command_state.current_header = None;
                self.previous_was_newline = true;
                return true;
            }
            Some(Token::Word(word)) if self.is_header(word) => return false,
            Some(Token::Symbol('#')) => {
                let header_indent = self
                    .state
                    .indent()
                    .max(self.pending_braceless_block_bias.unwrap_or(0));
                let conditional_after_else = header == Some("else")
                    && matches!(
                        tokens.get(statement_start + 1),
                        Some(Token::Word(word)) if matches!(word.as_str(), "if" | "ifdef" | "ifndef")
                    );
                let directive = match tokens.get(statement_start + 1) {
                    Some(Token::Word(word)) => Some(word.as_str()),
                    _ => None,
                };
                let known_preprocessor = directive.is_some_and(is_known_preprocessor_directive);
                let conditional_preprocessor = directive.is_some_and(is_conditional_preprocessor);
                self.finish_line();
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces =
                    known_preprocessor.then_some(if conditional_preprocessor {
                        0
                    } else {
                        header_indent * self.options.indent_width
                    });
                if conditional_after_else {
                    self.state.clear_continuation_indents();
                    self.stack_state.clear_continuation_indents();
                    self.continuation_indent.logical_chain_indent_spaces = None;
                    self.preprocessor.split_else.pending_body = true;
                    self.preprocessor.split_else.body_braceless = true;
                    self.preprocessor.split_else.trigger_output_len = Some(usize::MAX);
                }
                self.previous_was_newline = true;
                self.command_state.current_header = None;
                self.command_state.preprocessor_after_header = false;
                return true;
            }
            Some(_) => {}
            None => return false,
        }

        let header_indent = self
            .state
            .indent()
            .max(self.pending_braceless_block_bias.unwrap_or(0));
        self.finish_line();
        let split_else_keeps_body_level = split_else_after_preprocessor
            && (self.preprocessor.split_else.extra_levels > 0
                || split_else_preprocessor_follows_closing_brace(&self.output));
        let body_indent = header_indent
            + usize::from(!split_else_after_preprocessor || split_else_keeps_body_level);
        self.continuation_indent.next_line_indent = Some(body_indent);
        self.continuation_indent.next_line_indent_spaces = None;
        if !split_else_after_preprocessor {
            self.pending_braceless_block_bias = Some(body_indent);
        }
        self.previous_was_newline = true;
        if split_else_after_preprocessor {
            self.preprocessor.split_else.pending_body = false;
            self.preprocessor.split_else.trigger_output_len = Some(self.output.len());
            self.preprocessor.split_else.body_braceless = true;
        }
        self.command_state.current_header = None;
        self.command_state.preprocessor_after_header = false;
        true
    }

    pub(super) fn try_break_braceless_header_body(
        &mut self,
        tokens: &[Token],
        newline_index: usize,
    ) -> bool {
        let adding_braces = self.options.add_braces || self.options.add_one_line_braces;
        let Some(header) = self.command_state.current_header.as_deref() else {
            return false;
        };
        if !is_add_braces_header(header) || is_defer_header(header) {
            return false;
        }
        if matches!(header, "if" | "for" | "while") {
            let open_parens_are_outside_current_block = self
                .stack_state
                .current_brace_paren_depth()
                .is_some_and(|depth| depth == self.stack_state.paren_depth);
            if self.command_state.previous_command_char != Some(')')
                || (self.stack_state.paren_depth > 0 && !open_parens_are_outside_current_block)
            {
                return false;
            }
        }
        let Some(body_index) = next_non_layout_token_index(tokens, newline_index + 1) else {
            return false;
        };
        match &tokens[body_index] {
            Token::Symbol('{') | Token::Comment(_, _) | Token::Preprocessor(_) => return false,
            Token::Word(word)
                if header == "else"
                    && word == "if"
                    && !self
                        .previous_pre_adjust_line
                        .as_deref()
                        .is_some_and(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            code.trim_start() == "else" && line_ends_with_comment(line)
                        }) =>
            {
                return false;
            }
            _ => {}
        }
        let body_is_nested_header =
            matches!(&tokens[body_index], Token::Word(word) if is_add_braces_header(word));
        if adding_braces && !body_is_nested_header {
            let body_is_multi_line = find_statement_semicolon(tokens, body_index, tokens.len())
                .is_some_and(|semicolon| {
                    tokens[body_index..semicolon]
                        .iter()
                        .any(|token| matches!(token, Token::Newline))
                });
            if !body_is_multi_line {
                return false;
            }
        }
        let preserves_return_continuation_column = self.options.brace_style
            == BraceStyle::Whitesmith
            && self.output.last_non_empty_line().is_some_and(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with("return ") && code.ends_with(':')
            });
        let semantic_header = self
            .frame_stack
            .active_header()
            .filter(|frame| {
                frame.header == header
                    && (preserves_return_continuation_column
                        || frame
                            .line_indent_spaces
                            .is_multiple_of(self.options.indent_width))
            })
            .map(|frame| (frame.line_indent_spaces, frame.body_indent_spaces));
        let (header_indent, exact_body_indent) = if self.command_state.header_broken_before_comment
        {
            self.command_state.header_broken_before_comment = false;
            (self.state.indent(), None)
        } else if let Some((line_indent, body_indent)) = semantic_header {
            (
                line_indent / self.options.indent_width,
                (!line_indent.is_multiple_of(self.options.indent_width)).then_some(body_indent),
            )
        } else {
            (
                self.continuation_indent
                    .next_line_indent
                    .unwrap_or_else(|| self.state.indent())
                    .max(self.state.indent() + self.case_body_indent_extra(LineKind::Normal))
                    .max(self.pending_braceless_block_bias.unwrap_or(0)),
                None,
            )
        };
        self.finish_line();
        if let Some(spaces) = exact_body_indent {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
            self.pending_braceless_block_bias = Some(spaces / self.options.indent_width);
        } else {
            self.continuation_indent.next_line_indent = Some(header_indent + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.pending_braceless_block_bias = Some(header_indent + 1);
        }
        self.command_state.current_header = None;
        self.previous_was_newline = true;
        true
    }

    pub(super) fn try_break_else_if(&mut self, tokens: &[Token], start: usize) -> bool {
        if !self.options.break_else_ifs
            || !matches!(tokens.get(start), Some(Token::Word(word)) if word == "if")
            || self.command_state.current_header.as_deref() != Some("else")
        {
            return false;
        }
        if !self.current_is_blank() {
            self.finish_line();
        }
        self.else_if_break_depths.push(self.state.indent());
        true
    }

    pub(super) fn is_after_preprocessor_split_else(&self) -> bool {
        self.preprocessor.split_else.pending_body && self.preprocessor.split_else.after_line
    }

    pub(super) fn try_push_one_line_defer_block(
        &mut self,
        tokens: &[Token],
        start: usize,
        line_end: usize,
    ) -> Option<usize> {
        let Token::Word(word) = tokens.get(start)? else {
            return None;
        };
        if !is_defer_header(word) {
            return None;
        }

        let open_index = next_non_whitespace(tokens, start + 1, line_end)?;
        if !matches!(tokens.get(open_index), Some(Token::Symbol('{'))) {
            return None;
        }
        let close_index = self.matching_brace_on_current_line(open_index)?;
        let line = format_one_line_block_tokens(
            &tokens[start..=close_index],
            self.options,
            Some(FormatterBraceType::DeferArray),
            None,
        );
        self.push_output_line(&line, self.state.indent());
        self.command_state.current_header = None;
        self.command_state.preprocessor_after_header = false;
        self.command_state.previous_command_char = Some('}');
        self.command_state.previous_non_ws_char = Some('}');
        self.previous = PreviousToken::None;
        self.previous_was_newline = false;
        Some(close_index + 1)
    }

    // A bare `{` in a non-command scope (file scope or a bare block) is an
    // array-type brace: it opens after no code, after another bare block, or
    // right after the enclosing bare `{`. Command context (`;`, a header, a
    // command block close) makes it a statement block instead.
    fn bare_scope_one_line_block_type(
        &self,
        tokens: &[Token],
        start: usize,
    ) -> Option<FormatterBraceType> {
        let bare_scope = matches!(
            self.stack_state.brace_type_stack.last(),
            None | Some(FormatterBraceType::NonStatement)
        );
        if !bare_scope || self.stack_state.paren_depth > 0 {
            return None;
        }
        let begins_line = token_begins_line(tokens, start) && self.current_is_blank();
        let kept = match self.command_state.previous_command_char {
            None => begins_line,
            Some('{') => {
                begins_line
                    && matches!(
                        self.stack_state.brace_type_stack.last(),
                        Some(FormatterBraceType::NonStatement)
                    )
            }
            Some('}') => {
                matches!(
                    self.stack_state.last_closed_brace_type,
                    Some(FormatterBraceType::Array | FormatterBraceType::NonStatement)
                ) && (begins_line || self.current.trim_end().ends_with('}'))
            }
            _ => false,
        };
        kept.then_some(FormatterBraceType::Array)
    }

    fn previous_output_code_ends_assignment(&self) -> bool {
        self.output.last().is_some_and(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.ends_with('=')
        })
    }

    fn current_or_previous_is_lambda_capture_header(&self) -> bool {
        let current = self.current.trim_end();
        is_lambda_capture_header(current)
            || (current.is_empty()
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| is_lambda_capture_header(line.trim_end())))
    }

    fn in_declaration_brace_scope(&self) -> bool {
        self.stack_state
            .brace_type_stack
            .last()
            .is_none_or(|brace_type| {
                matches!(
                    brace_type,
                    FormatterBraceType::Namespace
                        | FormatterBraceType::Class
                        | FormatterBraceType::Interface
                        | FormatterBraceType::Struct
                        | FormatterBraceType::Union
                        | FormatterBraceType::Enum
                        | FormatterBraceType::Extern
                )
            })
    }

    pub(super) fn inferred_definition_brace(&self, tokens: &[Token], brace_index: usize) -> bool {
        if !self.in_declaration_brace_scope() || !segment_follows_inferred_type(tokens, brace_index)
        {
            return false;
        }
        previous_code_token(tokens, brace_index, 0).is_some_and(|previous| {
            !matches!(tokens.get(previous), Some(Token::Operator(operator)) if operator == "=")
        })
    }

    fn inferred_capture_lambda_breaks(&self, tokens: &[Token], brace_index: usize) -> bool {
        self.current_or_previous_is_lambda_capture_header()
            && self.inferred_definition_brace(tokens, brace_index)
    }

    pub(super) fn try_push_one_line_initializer_block(
        &mut self,
        tokens: &[Token],
        start: usize,
        line_start: usize,
        line_end: usize,
    ) -> Option<usize> {
        if !matches!(tokens.get(start), Some(Token::Symbol('{'))) {
            return None;
        }
        if self.is_objc_method_line()
            || self
                .command_state
                .current_header
                .as_deref()
                .is_some_and(|header| {
                    matches!(header, "autoreleasepool" | "@try" | "@catch" | "@finally")
                })
            || self.inferred_definition_brace(tokens, start)
        {
            return None;
        }
        let brace_type = initializer_brace_type(tokens, start, line_start)
            .or_else(|| {
                let previous = previous_non_whitespace(tokens, start, line_start);
                (previous.is_some_and(|index| matches!(tokens[index], Token::Symbol(':')))
                    && tokens[line_start..start]
                        .iter()
                        .any(|token| matches!(token, Token::Word(word) if word == "for"))
                    && self.open_lambda_body_indent_spaces().is_some())
                .then_some(FormatterBraceType::Array)
            })
            .or_else(|| {
                (self.current_is_blank() && self.previous_output_code_ends_assignment())
                    .then_some(FormatterBraceType::Init)
            })
            .or_else(|| {
                let previous_code = self.output.last().map(|line| {
                    line[..trailing_comment_split_limit(line)]
                        .trim_end()
                        .to_string()
                });
                (self.current_is_blank()
                    && previous_code
                        .as_deref()
                        .is_some_and(|code| code.ends_with(','))
                    && (self.in_initializer_brace()
                        || previous_code
                            .as_deref()
                            .is_some_and(has_unmatched_open_brace)))
                .then_some(FormatterBraceType::Array)
            })
            .or_else(|| {
                (self.current_is_blank() && self.stack_state.paren_depth > 0)
                    .then_some(FormatterBraceType::Array)
            })
            .or_else(|| {
                self.stack_state
                    .brace_type_stack
                    .last()
                    .is_some_and(|brace_type| {
                        matches!(
                            brace_type,
                            FormatterBraceType::Array | FormatterBraceType::CompoundLiteral
                        )
                    })
                    .then_some(FormatterBraceType::Array)
            })
            .or_else(|| self.bare_scope_one_line_block_type(tokens, start))?;
        let close_index = self.matching_brace_on_current_line(start)?;
        let inferred_capture_lambda = self.inferred_capture_lambda_breaks(tokens, start);

        let previous_line_lambda_header = self.current_is_blank()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| is_lambda_body_header(line.trim_end()));
        let lambda_header = self.current_is_lambda_body_header()
            || previous_line_lambda_header
            || (self.current.trim_end().ends_with(')')
                && self.current.contains('[')
                && self.current.contains(']'));
        if lambda_header
            && self.options.break_one_line_blocks
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
            )
        {
            return None;
        }
        if one_line_block_contains_plain_lambda_body(&tokens[start..=close_index]) {
            return None;
        }
        if (lambda_header || inferred_capture_lambda)
            && self.options.break_one_line_blocks
            && !self.current.contains("->")
            && !line_ends_compound_literal_cast(self.current.trim_end())
            && !is_empty_one_line_block_tokens(&tokens[start..=close_index])
            && !is_comment_only_one_line_block_tokens(&tokens[start..=close_index])
            && !is_semicolon_only_one_line_block_tokens(&tokens[start..=close_index])
        {
            return None;
        }
        if self.options.break_one_line_blocks
            && !token_begins_line(tokens, start)
            && self
                .compound_literal
                .forced_break_depths
                .last()
                .is_some_and(|depth| *depth == self.stack_state.brace_header_stack.len())
            && !is_empty_one_line_block_tokens(&tokens[start..=close_index])
            && !is_comment_only_one_line_block_tokens(&tokens[start..=close_index])
        {
            return None;
        }
        let source_gap = (start > line_start)
            .then(|| match tokens.get(start - 1) {
                Some(Token::Whitespace(gap)) => Some(gap.as_str()),
                _ => None,
            })
            .flatten();
        if token_begins_line(tokens, start)
            && self.current_is_blank()
            && self.previous_output_code_ends_assignment()
        {
            self.continuation_indent.next_line_indent = Some(self.state.indent());
            self.continuation_indent.next_line_indent_spaces = None;
        }
        if token_begins_line(tokens, start)
            && self.current_is_blank()
            && let Some(previous) = self.output.last()
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',') && has_unmatched_open_brace(previous_code) {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if token_begins_line(tokens, start)
            && self.current_is_blank()
            && self.options.indent_braces
            && matches!(
                brace_type,
                FormatterBraceType::Array
                    | FormatterBraceType::Init
                    | FormatterBraceType::CompoundLiteral
            )
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::Init
                        | FormatterBraceType::CompoundLiteral
                )
            )
        {
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
        }
        if token_begins_line(tokens, start)
            && self.current_is_blank()
            && self.stack_state.paren_depth > 0
            && !(self.continuation_indent.next_line_indent_spaces.is_some()
                && matches!(
                    self.stack_state.brace_type_stack.last(),
                    Some(
                        FormatterBraceType::Array
                            | FormatterBraceType::Init
                            | FormatterBraceType::CompoundLiteral
                    )
                ))
        {
            self.continuation_indent.next_line_indent = Some(self.state.indent());
            self.continuation_indent.next_line_indent_spaces = None;
        }
        if brace_type == FormatterBraceType::Enum
            && !self.options.attach_enum
            && !self.current_is_blank()
        {
            let indent = self.state.indent();
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(indent);
            self.continuation_indent.next_line_indent_spaces = None;
        }
        if brace_type == FormatterBraceType::Array
            && token_begins_line(tokens, start)
            && !self.current_is_blank()
            && self.command_state.previous_command_char == Some('}')
            && matches!(
                self.stack_state.brace_type_stack.last(),
                None | Some(FormatterBraceType::NonStatement)
            )
        {
            let indent = self.state.indent();
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(indent);
            self.continuation_indent.next_line_indent_spaces = None;
        }
        self.push_attached_one_line_block(
            &tokens[start..=close_index],
            brace_type,
            None::<&str>,
            source_gap,
            false,
            None,
        );
        if brace_type == FormatterBraceType::CompoundLiteral {
            self.compound_literal.just_closed = true;
        }
        if brace_type == FormatterBraceType::Enum {
            let next = next_non_whitespace(tokens, close_index + 1, line_end)
                .and_then(|next_index| tokens.get(next_index));
            if matches!(next, Some(Token::Word(_)) | Some(Token::Symbol('['))) {
                self.ensure_space();
            }
        }
        Some(close_index + 1)
    }

    pub(super) fn try_push_kept_one_line_block(
        &mut self,
        tokens: &[Token],
        start: usize,
        line_end: usize,
    ) -> Option<usize> {
        if !matches!(tokens.get(start), Some(Token::Symbol('{'))) {
            return None;
        }
        let close_index = self.matching_brace_on_current_line(start)?;
        let break_one_line_blocks = self.options.break_one_line_blocks
            || self.options.lisp_add_one_line_braces_breaks_blocks()
            || (self.options.break_one_line_headers
                && !(self.options.brace_style == BraceStyle::Pico
                    && self.command_state.current_header.as_deref() == Some("switch"))
                && self.command_state.current_header.is_some());
        if self.options.break_one_line_headers
            && self.command_state.current_header.is_some()
            && tokens[start + 1..close_index]
                .iter()
                .any(|token| matches!(token, Token::Word(word) if is_add_braces_header(word) || word == "switch"))
        {
            return None;
        }
        if self.options.break_one_line_statements
            && one_line_block_contains_case_label(&tokens[start..=close_index])
        {
            return None;
        }
        if let Some(next_index) =
            self.try_push_one_line_preprocessor_block(tokens, start, close_index, line_end)
        {
            return Some(next_index);
        }
        let is_empty_block = is_empty_one_line_block_tokens(&tokens[start..=close_index]);
        let is_comment_only_block =
            is_comment_only_one_line_block_tokens(&tokens[start..=close_index]);
        let is_semicolon_only_block =
            is_semicolon_only_one_line_block_tokens(&tokens[start..=close_index]);
        let inferred_capture_lambda = self.inferred_capture_lambda_breaks(tokens, start);
        let is_asm_block = is_asm_block_header(trailing_word(&self.current))
            || self
                .command_state
                .current_header
                .as_deref()
                .is_some_and(is_asm_block_header);
        let source_separate_macro_block = token_begins_line(tokens, start)
            && self
                .current
                .split_whitespace()
                .next()
                .is_some_and(is_macro_like_word)
            && self.current.split_whitespace().count() == 1;
        let backslash_continuation_block =
            !token_begins_line(tokens, start) && self.current.trim_end().ends_with('\\');
        let previous_line_lambda_header = self.current_is_blank()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| is_lambda_body_header(line.trim_end()));
        let previous_line_trailing_return_lambda_header = self.current_is_blank()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let line = line.trim_end();
                    is_lambda_body_header(line) && line.contains("->")
                });
        let lambda_header = self.current_is_lambda_body_header()
            || previous_line_lambda_header
            || (self.current.trim_end().ends_with(')')
                && self.current.contains('[')
                && self.current.contains(']'));
        let parameterized_lambda_header = self.current_is_lambda_body_header()
            || (self.current.trim_end().ends_with(')')
                && self.current.contains('[')
                && self.current.contains(']'));
        let in_declaration_scope = match self.stack_state.brace_type_stack.last() {
            None => self.stack_state.paren_depth == 0,
            Some(brace_type) => matches!(
                brace_type,
                FormatterBraceType::Namespace
                    | FormatterBraceType::Class
                    | FormatterBraceType::Interface
                    | FormatterBraceType::Struct
                    | FormatterBraceType::Union
                    | FormatterBraceType::Enum
                    | FormatterBraceType::Extern
            ),
        };
        let trailing_return_lambda_body = lambda_header
            && (self.current.contains("->") || previous_line_trailing_return_lambda_header)
            && !(in_declaration_scope && self.current_ends_trailing_return_definition());
        if (parameterized_lambda_header || inferred_capture_lambda)
            && break_one_line_blocks
            && !self.current.contains("->")
            && !is_empty_block
            && !is_comment_only_block
            && !is_semicolon_only_block
        {
            return None;
        }
        let operator_header =
            self.current.trim_end().ends_with(')') && self.current.contains("operator");
        let non_attaching_lambda_style = matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Horstmann
        );
        let non_attaching_lambda_body = (lambda_header || operator_header)
            && non_attaching_lambda_style
            && break_one_line_blocks
            && !trailing_return_lambda_body;
        if break_one_line_blocks
            && non_attaching_lambda_style
            && (one_line_block_contains_lambda_body(&tokens[start..=close_index])
                || one_line_block_contains_operator_body(&tokens[start..=close_index]))
        {
            return None;
        }
        if non_attaching_lambda_body
            && !is_empty_block
            && !is_comment_only_block
            && !is_semicolon_only_block
        {
            return None;
        }
        if break_one_line_blocks
            && !is_empty_block
            && !is_comment_only_block
            && !is_semicolon_only_block
            && !is_asm_block
            && !source_separate_macro_block
            && !backslash_continuation_block
            && !trailing_return_lambda_body
        {
            return None;
        }
        if self.previous_was_newline || source_separate_macro_block {
            self.finish_line();
        }
        let opening_body_gap = (self.options.brace_style == BraceStyle::Pico
            && self.current_is_blank()
            && (self.command_state.current_header.is_some()
                || self.current_ends_definition_header()
                || self.output_ends_objc_method_header()))
        .then(|| {
            if self.options.indent_style == IndentStyle::Tabs {
                "\t".to_string()
            } else {
                " ".repeat(self.options.indent_width.saturating_sub(1))
            }
        });
        let empty_block_after_operator = is_empty_block && self.previous == PreviousToken::Operator;
        let brace_header = is_asm_block.then_some("_asm");
        let brace_type = if inferred_capture_lambda || self.inferred_definition_brace(tokens, start)
        {
            self.classify_opening_brace(brace_header, self.pending_extern);
            FormatterBraceType::Definition
        } else {
            self.classify_opening_brace(brace_header, self.pending_extern)
        };
        if token_begins_line(tokens, start) && self.current_is_blank() {
            self.continuation_indent.next_line_indent = Some(self.state.indent());
            self.continuation_indent.next_line_indent_spaces = None;
        }
        let source_gap = match tokens.get(start.wrapping_sub(1)) {
            Some(Token::Whitespace(gap)) => Some(gap.as_str()),
            _ => None,
        };
        let leading_gap = is_asm_block.then(|| source_gap.unwrap_or(""));
        self.push_attached_one_line_block(
            &tokens[start..=close_index],
            brace_type,
            leading_gap,
            source_gap,
            source_separate_macro_block,
            opening_body_gap,
        );
        if is_empty_block
            && (self.options.brace_style == BraceStyle::None
                || self.is_attached_closing_header_style())
            && self.command_state.current_header.as_deref() == Some("do")
            && let Some(while_index) = next_non_whitespace(tokens, close_index + 1, line_end)
            && matches!(tokens.get(while_index), Some(Token::Word(word)) if word == "while")
            && let Some(semi_index) = (while_index..line_end)
                .find(|index| matches!(tokens.get(*index), Some(Token::Symbol(';'))))
        {
            for token in &tokens[close_index + 1..=semi_index] {
                self.current.push_str(&token_text(token));
            }
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.command_state.previous_command_char = Some(';');
            self.command_state.previous_non_ws_char = Some(';');
            let trailing = next_non_whitespace(tokens, semi_index + 1, line_end)
                .and_then(|index| tokens.get(index));
            if !matches!(trailing, Some(Token::Comment(_, _))) {
                self.finish_line();
            }
            return Some(semi_index + 1);
        }
        if self.command_state.current_header.as_deref() == Some("do") {
            self.stack_state.last_closed_brace_header = Some("do".to_string());
        }
        self.command_state.current_header = None;
        self.command_state.preprocessor_after_header = false;
        let next = next_non_whitespace(tokens, close_index + 1, line_end)
            .and_then(|next_index| tokens.get(next_index));
        let init_block_continues_expression = brace_type == FormatterBraceType::Init
            && matches!(next, Some(Token::Symbol('(' | ')')));
        let empty_value_block_continues_expression = is_empty_block
            && (matches!(next, Some(Token::Symbol('(' | ')')))
                || (empty_block_after_operator && next.is_some()));
        let lambda_block_continues_expression =
            (lambda_header || operator_header) && matches!(next, Some(Token::Symbol('(' | ')')));
        let next_is_pointer_declarator =
            matches!(next, Some(Token::Operator(op)) if matches!(op.as_str(), "*" | "&"));
        let aggregate_trailing_declarator = (is_class_like_brace_type(brace_type)
            || brace_type == FormatterBraceType::Enum)
            && (matches!(next, Some(Token::Word(_))) || next_is_pointer_declarator);
        let next_is_closing_header = matches!(
            next,
            Some(Token::Word(word))
                if matches!(word.as_str(), "else" | "catch" | "@catch" | "__finally" | "__except" | "while")
        );
        if !init_block_continues_expression
            && !empty_value_block_continues_expression
            && !lambda_block_continues_expression
            && !aggregate_trailing_declarator
            && (!next_is_closing_header
                || self.options.break_one_line_statements
                || self.options.brace_style == BraceStyle::Lisp
                || !self.should_attach_closing_header(next))
            && !matches!(next, Some(Token::Symbol(';' | ',')))
            && !matches!(next, Some(Token::Comment(_, _)))
        {
            self.finish_line();
            if brace_type == FormatterBraceType::Command {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            }
        }
        Some(close_index + 1)
    }

    pub(super) fn try_push_one_line_preprocessor_block(
        &mut self,
        tokens: &[Token],
        start: usize,
        close_index: usize,
        line_end: usize,
    ) -> Option<usize> {
        let hash_index = (start + 1..close_index)
            .find(|index| matches!(tokens.get(*index), Some(Token::Symbol('#'))))?;
        if previous_non_whitespace(tokens, start, 0)
            .is_some_and(|index| matches!(tokens[index], Token::Operator(_)))
        {
            return None;
        }
        let next =
            next_non_whitespace(tokens, start + 1, close_index).and_then(|index| tokens.get(index));
        self.set_input_whitespace(tokens, start, 0);
        self.token_input.token_begins_source_line = token_begins_line(tokens, start);
        self.push_open_brace(next, start, self.inferred_definition_brace(tokens, start));
        if self.output.last().is_some_and(|line| line.trim() == "{") {
            let brace_line = self.output.len() - 1;
            self.source_run_in_brace_lines
                .retain(|index| *index != brace_line);
        }

        let line = tokens[hash_index..close_index]
            .iter()
            .map(token_text)
            .collect::<String>();
        self.adjust_and_publish_line(line);
        self.preprocessor.last_output_was_preprocessor = true;

        let next = next_non_whitespace(tokens, close_index + 1, line_end)
            .and_then(|index| tokens.get(index));
        self.set_input_whitespace(tokens, close_index, 0);
        self.token_input.token_begins_source_line = token_begins_line(tokens, close_index);
        self.push_close_brace(next, false);
        let trimmed_len = self.current.trim_end().len();
        if self.current[..trimmed_len].ends_with("{}") {
            let open = trimmed_len.saturating_sub(2);
            if self.should_space_before_one_line_block(FormatterBraceType::Command)
                && open > 0
                && !self.current.as_bytes()[open - 1].is_ascii_whitespace()
            {
                self.current.insert(open, ' ');
            }
            if !matches!(
                next,
                Some(Token::Symbol(';' | ',' | ')')) | Some(Token::Comment(_, _))
            ) {
                self.finish_line();
            }
        }
        Some(close_index + 1)
    }

    pub(super) fn push_attached_one_line_block(
        &mut self,
        tokens: &[Token],
        brace_type: FormatterBraceType,
        leading_gap: Option<&str>,
        source_gap: Option<&str>,
        preserve_raw: bool,
        opening_body_gap: Option<String>,
    ) {
        let block = if preserve_raw || is_comment_only_one_line_block_tokens(tokens) {
            tokens.iter().map(token_text).collect::<String>()
        } else {
            format_one_line_block_tokens(
                tokens,
                self.options,
                Some(brace_type),
                opening_body_gap.as_deref(),
            )
        };
        let braced_init = (brace_type == FormatterBraceType::Init
            && (self
                .command_state
                .previous_command_char
                .is_some_and(|ch| is_identifier_continue(ch) || ch == '>' || ch == ']')
                || self.current.trim_end().ends_with('>')))
            || self.is_nested_designated_init_field();
        let after_comma = self.command_state.previous_command_char == Some(',');
        let run_in_array_gap_after_brace = self.current.ends_with([' ', '\t'])
            && matches!(self.current.trim_end().chars().next_back(), Some('{' | '['))
            && matches!(
                brace_type,
                FormatterBraceType::Array | FormatterBraceType::CompoundLiteral
            );
        if !after_comma && !run_in_array_gap_after_brace {
            self.trim_current_end();
        }
        let array_element_after_brace =
            matches!(self.current.trim_end().chars().next_back(), Some('{' | '['))
                && matches!(
                    brace_type,
                    FormatterBraceType::Array | FormatterBraceType::CompoundLiteral
                );
        match leading_gap {
            Some(gap) => self.current.push_str(gap),
            None if after_comma => {}
            None if array_element_after_brace && run_in_array_gap_after_brace => {
                let target_column = if self.current.trim_end() == "{" {
                    Some(
                        leading_visual_width(&self.current, self.options.tab_width)
                            + self.options.indent_width,
                    )
                } else {
                    self.current_inline_array_column()
                };
                if let Some(column) = target_column {
                    let current_column = self.current_visual_width();
                    if current_column < column {
                        self.current.push_str(&" ".repeat(column - current_column));
                    }
                }
            }
            None if array_element_after_brace => {
                self.current.push_str(source_gap.unwrap_or_default());
            }
            None if self.command_state.previous_command_char == Some('(') => match source_gap {
                _ if self.options.pad_parens_inside && self.options.unpad_parens => {
                    self.trim_current_end_horizontal_space();
                    self.current
                        .push(if source_gap.is_some_and(|gap| gap.ends_with('\t')) {
                            '\t'
                        } else {
                            ' '
                        });
                }
                Some(gap) if !gap.is_empty() => self.current.push_str(gap),
                _ if self.options.pad_parens_inside => {
                    self.ensure_space();
                }
                _ => {}
            },
            None if self.current_is_lambda_body_header()
                && super::brace_classification::lambda_header_has_trailing_return(
                    self.current.trim_end(),
                ) =>
            {
                self.current.push_str(source_gap.unwrap_or_default());
            }
            None if self.should_space_before_one_line_block(brace_type) => match source_gap {
                Some(gap) if gap.len() > 1 => self.current.push_str(gap),
                _ => self.ensure_space(),
            },
            None if braced_init
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| line.trim_start().starts_with("#if")) =>
            {
                self.ensure_space();
            }
            None if braced_init
                && unmatched_open_paren_column(&self.current).is_none()
                && ((block.trim_start().starts_with("{-")
                    && (self.state.current_preproc_indent().is_some()
                        || !self.preprocessor.branch_stack.is_empty()))
                    || self
                        .output
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| line.trim_start().starts_with("#endif"))) =>
            {
                self.ensure_space();
            }
            None if braced_init
                && self.options.pad_operators
                && self.current.trim_end().ends_with('=') =>
            {
                self.ensure_space();
            }
            None if braced_init => self.current.push_str(source_gap.unwrap_or_default()),
            None => {}
        }
        let collapsed_non_empty_command_block = brace_type == FormatterBraceType::Command
            && block == "{}"
            && !is_empty_one_line_block_tokens(tokens);
        if collapsed_non_empty_command_block && self.should_space_before_one_line_block(brace_type)
        {
            self.ensure_space();
        }
        self.current.push_str(&block);
        if collapsed_non_empty_command_block {
            self.finish_line();
        }
        self.stack_state.last_closed_brace_type = Some(brace_type);
        self.command_state.previous_command_char = Some('}');
        self.command_state.previous_non_ws_char = Some('}');
        self.observe_block_spacing_one_line_block(brace_type);
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn is_nested_designated_init_field(&self) -> bool {
        self.command_state.previous_command_char == Some('=')
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::Init
                        | FormatterBraceType::CompoundLiteral
                        | FormatterBraceType::Enum
                )
            )
    }

    pub(super) fn should_space_before_one_line_block(
        &self,
        brace_type: FormatterBraceType,
    ) -> bool {
        if self.command_state.previous_command_char == Some('(') {
            return self.options.pad_parens_inside;
        }
        if self.is_nested_designated_init_field() {
            return false;
        }
        if matches!(self.current.trim_end().chars().next_back(), Some('{' | '['))
            && matches!(
                brace_type,
                FormatterBraceType::Array | FormatterBraceType::CompoundLiteral
            )
        {
            return false;
        }
        !matches!(brace_type, FormatterBraceType::Init)
            || self.current.trim_end().ends_with('>')
            || !self
                .command_state
                .previous_command_char
                .is_some_and(is_identifier_continue)
    }

    pub(super) fn push_inline_open_brace(&mut self) {
        let inside_aggregate = self.inline_array.aggregate_braces.last() == Some(&true);
        let is_aggregate = self.inline_open_brace_is_aggregate();
        self.inline_array.aggregate_braces.push(is_aggregate);
        let braced_init = is_aggregate
            && (inside_aggregate
                || self
                    .command_state
                    .previous_command_char
                    .is_some_and(is_word_char));
        if !matches!(self.command_state.previous_command_char, Some(',' | '{')) {
            self.trim_current_end();
            if self.previous == PreviousToken::OpenParen
                || self.command_state.previous_command_char == Some('(')
            {
                if self.options.pad_parens_inside {
                    self.pad_inside_paren_space();
                } else {
                    self.emit_source_space();
                }
            } else if braced_init {
                if self.options.pad_operators && self.current.trim_end().ends_with('=') {
                    self.emit_source_space_or_ensure();
                } else {
                    self.emit_source_space();
                }
            } else if self
                .command_state
                .previous_command_char
                .is_some_and(|ch| is_word_char(ch) || ch == '>')
                && self.current_is_lambda_body_header()
                && super::brace_classification::lambda_header_has_trailing_return(
                    self.current.trim_end(),
                )
            {
                self.emit_source_space();
            } else {
                self.emit_source_space_or_ensure();
            }
        }
        self.current.push('{');
        self.emit_trailing_source_space();
        self.command_state.observe_char('{');
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn inline_open_brace_is_aggregate(&self) -> bool {
        if self.current.trim_end().ends_with('@') {
            return true;
        }
        match self.inline_array.aggregate_braces.last() {
            Some(true) => true,
            Some(false) => match self.command_state.previous_command_char {
                Some('=') => true,
                Some(')') => self.current_ends_compound_literal_type(),
                _ => false,
            },
            None => matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Array
                        | FormatterBraceType::Init
                        | FormatterBraceType::CompoundLiteral
                        | FormatterBraceType::Enum
                )
            ),
        }
    }

    pub(super) fn attach_closing_brace_mode(&self) -> bool {
        matches!(
            self.options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp
        )
    }

    pub(super) fn push_inline_close_brace(&mut self, next: Option<&Token>) {
        let is_aggregate = self.inline_array.aggregate_braces.pop().unwrap_or(false);
        let closes_compound_literal = is_aggregate
            && self
                .current
                .rsplit_once('{')
                .is_some_and(|(head, _)| line_ends_compound_literal_cast(head.trim_end()));
        if is_aggregate && self.attach_closing_brace_mode() {
            self.emit_source_space_or_ensure();
        } else {
            self.emit_source_space();
        }
        self.current.push('}');
        self.command_state.observe_char('}');
        self.compound_literal.just_closed = closes_compound_literal;
        if matches!(next, Some(Token::Word(_) | Token::Number(_))) {
            self.emit_trailing_source_space_or_ensure();
        } else {
            self.emit_trailing_source_space();
        }
        self.observe_block_spacing_inline_close_brace();
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn push_inline_semicolon(&mut self, next: Option<&Token>) {
        self.emit_source_space();
        self.current.push(';');
        self.command_state.observe_char(';');
        match next {
            Some(Token::Symbol(';' | ')' | '}')) | Some(Token::Comment(..)) | None => {
                self.emit_trailing_source_space();
            }
            _ => self.emit_trailing_source_space_or_ensure(),
        }
        self.command_state.current_header = None;
        self.command_state.preprocessor_after_header = false;
        self.command_state.pending_block_word = None;
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    pub(super) fn push_replayed_statement(
        &mut self,
        tokens: &[Token],
        start: usize,
        end_inclusive: usize,
        following: Option<&Token>,
        whitespace_lower: usize,
        leading_whitespace: Option<&str>,
    ) {
        for index in start..=end_inclusive {
            if matches!(tokens[index], Token::Whitespace(_)) {
                continue;
            }
            let next_index = next_non_whitespace(tokens, index + 1, end_inclusive + 1);
            let next = next_index
                .and_then(|next_index| tokens.get(next_index))
                .or((index == end_inclusive).then_some(following).flatten());
            self.set_input_whitespace(tokens, index, whitespace_lower);
            if index == start
                && let Some(whitespace) = leading_whitespace
            {
                self.token_input.previous_input_was_adjacent = false;
                self.token_input.previous_input_whitespace = Some(whitespace.to_string());
            }
            self.push_token(
                &tokens[index],
                TokenPushContext {
                    next,
                    next_is_adjacent: next_index == Some(index + 1),
                    following_operator: None,
                    template_angle: TemplateAngle::None,
                    token_index: usize::MAX,
                    starts_initializer_designator: false,
                    inferred_definition_brace: false,
                    following_closing_braces: 0,
                },
            );
        }
    }
}

pub(super) fn is_defer_header(word: &str) -> bool {
    matches!(word, "defer" | "_Defer")
}

pub(super) fn is_add_braces_header(word: &str) -> bool {
    matches!(
        word,
        "if" | "else" | "for" | "foreach" | "Q_FOREACH" | "while" | "do"
    )
}

fn token_range_has_line_comment(tokens: &[Token], start: usize, end: usize) -> bool {
    tokens[start..end]
        .iter()
        .any(|token| matches!(token, Token::Comment(CommentKind::Line, _)))
}

fn split_else_preprocessor_follows_closing_brace(output: &[String]) -> bool {
    let mut saw_preprocessor = false;
    for line in output.iter().rev().take(8) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('#') {
            saw_preprocessor = true;
            continue;
        }
        return saw_preprocessor && trimmed.ends_with("} else");
    }
    false
}

fn is_remove_braces_header(word: &str) -> bool {
    matches!(word, "if" | "else" | "for" | "while")
}

pub(super) fn add_cross_line_statement_braces(
    tokens: &[Token],
    attach_added_braces: bool,
) -> Vec<Token> {
    let mut insert_before = vec![Vec::<Token>::new(); tokens.len() + 1];
    let mut covered_until = 0usize;
    for header_index in 0..tokens.len() {
        if header_index < covered_until {
            continue;
        }
        let Some((header_end, open_insert, close_insert)) =
            add_braces_insertion_range(tokens, header_index)
        else {
            continue;
        };
        if open_insert >= close_insert {
            continue;
        }
        if attach_added_braces
            && open_brace_attaches_to_header_line(tokens, header_end, open_insert)
        {
            let brace_insert = (header_end + 1..open_insert - 1)
                .find(|&index| matches!(tokens[index], Token::Comment(_, _)))
                .unwrap_or(open_insert - 1);
            insert_before[brace_insert].push(Token::Symbol('{'));
        } else {
            insert_before[open_insert].push(Token::Symbol('{'));
            insert_before[open_insert].push(Token::Newline);
        }
        insert_before[close_insert].push(Token::Newline);
        insert_before[close_insert].push(Token::Symbol('}'));
        covered_until = close_insert;
    }

    let mut output =
        Vec::with_capacity(tokens.len() + insert_before.iter().map(Vec::len).sum::<usize>());
    for (index, token) in tokens.iter().cloned().enumerate() {
        output.append(&mut insert_before[index]);
        output.push(token);
    }
    output.append(&mut insert_before[tokens.len()]);
    output
}

fn open_brace_attaches_to_header_line(
    tokens: &[Token],
    header_end: usize,
    open_insert: usize,
) -> bool {
    if open_insert == 0 || !matches!(tokens.get(open_insert - 1), Some(Token::Newline)) {
        return false;
    }
    if !tokens[header_end + 1..open_insert - 1]
        .iter()
        .all(|token| matches!(token, Token::Whitespace(_) | Token::Comment(_, _)))
    {
        return false;
    }
    tokens[open_insert..]
        .iter()
        .find(|token| !matches!(token, Token::Whitespace(_)))
        .is_none_or(|token| !matches!(token, Token::Comment(_, _)))
}

fn header_body_start(tokens: &[Token], header_index: usize, line_end: usize) -> Option<usize> {
    let header = match tokens.get(header_index)? {
        Token::Word(word) => word.as_str(),
        _ => return None,
    };
    let header_end = if header == "else" {
        header_index
    } else {
        matching_close_paren_index(
            tokens,
            header_condition_open_paren(tokens, header_index, line_end, header)?,
        )?
    };
    next_non_layout_token_index(tokens, header_end + 1)
}

fn header_condition_open_paren(
    tokens: &[Token],
    header_index: usize,
    end: usize,
    header: &str,
) -> Option<usize> {
    let mut open_paren = next_statement_token(tokens, header_index + 1, end, true)?;
    if header == "if"
        && matches!(tokens.get(open_paren), Some(Token::Word(word)) if word == "constexpr")
    {
        open_paren = next_statement_token(tokens, open_paren + 1, end, true)?;
    }
    matches!(tokens.get(open_paren), Some(Token::Symbol('('))).then_some(open_paren)
}

fn add_braces_insertion_range(
    tokens: &[Token],
    header_index: usize,
) -> Option<(usize, usize, usize)> {
    let header = match tokens.get(header_index)? {
        Token::Word(word) if is_add_braces_header(word) && !is_defer_header(word) => word.as_str(),
        _ => return None,
    };
    let header_end = if header == "else" {
        header_index
    } else {
        matching_close_paren_index(
            tokens,
            header_condition_open_paren(tokens, header_index, tokens.len(), header)?,
        )?
    };
    let statement_start = next_add_braces_statement_token(tokens, header_end + 1)?;
    if !tokens[header_end + 1..statement_start]
        .iter()
        .any(|token| matches!(token, Token::Newline))
    {
        return None;
    }
    match tokens.get(statement_start)? {
        Token::Symbol('{') | Token::Symbol(';') | Token::Preprocessor(_) | Token::Newline => {
            return None;
        }
        Token::Comment(_, _) => return None,
        Token::Word(word) if language::is_header(word) => return None,
        _ => {}
    }
    let semicolon = find_statement_semicolon(tokens, statement_start, tokens.len())?;
    if tokens[statement_start..semicolon]
        .iter()
        .any(|token| matches!(token, Token::Newline))
    {
        return None;
    }
    let (_, statement_line_end) = line_bounds(tokens, semicolon);
    Some((
        header_end,
        line_bounds(tokens, statement_start).0,
        statement_line_end,
    ))
}

fn next_add_braces_statement_token(tokens: &[Token], start: usize) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => {}
            Token::Preprocessor(_) => return None,
            _ => return Some(index),
        }
    }
    None
}

pub(super) fn remove_cross_line_statement_braces(tokens: &[Token]) -> Vec<Token> {
    let mut remove = vec![false; tokens.len()];
    let mut replace_with_space = vec![false; tokens.len()];
    for open_index in 0..tokens.len() {
        if remove[open_index] || !matches!(tokens[open_index], Token::Symbol('{')) {
            continue;
        }
        if !is_remove_braces_opening(tokens, open_index) {
            continue;
        }
        let Some((_, _, close_index)) =
            removable_statement_brace_range(tokens, open_index, tokens.len(), true)
        else {
            continue;
        };
        if !tokens[open_index..=close_index]
            .iter()
            .any(|token| matches!(token, Token::Newline))
        {
            continue;
        }
        mark_removed_brace(tokens, open_index, &mut remove);
        if opening_brace_has_line_comment(tokens, open_index) {
            replace_with_space[open_index] = true;
        }
        mark_removed_brace(tokens, close_index, &mut remove);
        let (_, close_line_end) = line_bounds(tokens, close_index);
        if tokens[close_index + 1..close_line_end]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_)))
            .is_some_and(|token| {
                matches!(
                    token,
                    Token::Word(_) | Token::Number(_) | Token::Comment(_, _)
                )
            })
        {
            replace_with_space[close_index] = true;
        }
    }

    let mut output = Vec::with_capacity(tokens.len());
    for (index, token) in tokens.iter().cloned().enumerate() {
        let token = if replace_with_space[index] {
            Some(Token::Whitespace(" ".to_string()))
        } else {
            (!remove[index]).then_some(token)
        };
        let Some(token) = token else {
            continue;
        };
        match token {
            Token::Whitespace(whitespace) => {
                if let Some(Token::Whitespace(previous)) = output.last_mut() {
                    previous.push_str(&whitespace);
                } else {
                    output.push(Token::Whitespace(whitespace));
                }
            }
            token => output.push(token),
        }
    }
    output
}

fn opening_brace_has_line_comment(tokens: &[Token], open_index: usize) -> bool {
    let (_, line_end) = line_bounds(tokens, open_index);
    tokens[open_index + 1..line_end]
        .iter()
        .find(|token| !matches!(token, Token::Whitespace(_)))
        .is_some_and(|token| matches!(token, Token::Comment(CommentKind::Line, _)))
}

fn is_remove_braces_opening(tokens: &[Token], open_index: usize) -> bool {
    let Some(previous) = previous_non_layout_token(tokens, open_index) else {
        return false;
    };
    match tokens.get(previous) {
        Some(Token::Word(word)) => word == "else",
        Some(Token::Symbol(')')) => is_remove_braces_paren_header(tokens, previous),
        _ => false,
    }
}

fn is_remove_braces_paren_header(tokens: &[Token], close_paren: usize) -> bool {
    let Some(open_paren) = matching_open_paren_global(tokens, close_paren) else {
        return false;
    };
    previous_non_layout_token(tokens, open_paren).is_some_and(|header| match tokens.get(header) {
        Some(Token::Word(word)) if matches!(word.as_str(), "if" | "for" | "while") => true,
        Some(Token::Word(word)) if word == "constexpr" => previous_non_layout_token(tokens, header)
            .is_some_and(
                |index| matches!(tokens.get(index), Some(Token::Word(word)) if word == "if"),
            ),
        _ => false,
    })
}

fn matching_open_paren_global(tokens: &[Token], close_paren: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in (0..=close_paren).rev() {
        match tokens[index] {
            Token::Symbol(')') => depth += 1,
            Token::Symbol('(') => {
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

fn mark_removed_brace(tokens: &[Token], brace_index: usize, remove: &mut [bool]) {
    let (line_start, line_end) = line_bounds(tokens, brace_index);
    let only_brace_on_line = (line_start..line_end)
        .all(|index| index == brace_index || matches!(tokens[index], Token::Whitespace(_)));
    if only_brace_on_line {
        let end = if matches!(tokens.get(line_end), Some(Token::Newline)) {
            line_end + 1
        } else {
            line_end
        };
        for slot in &mut remove[line_start..end] {
            *slot = true;
        }
    } else {
        remove[brace_index] = true;
    }
}

fn line_bounds(tokens: &[Token], index: usize) -> (usize, usize) {
    let start = (0..index)
        .rev()
        .find(|candidate| matches!(tokens[*candidate], Token::Newline))
        .map_or(0, |newline| newline + 1);
    let end = (index + 1..tokens.len())
        .find(|candidate| matches!(tokens[*candidate], Token::Newline))
        .unwrap_or(tokens.len());
    (start, end)
}

fn removable_statement_brace_range(
    tokens: &[Token],
    open_index: usize,
    line_end: usize,
    allow_newlines: bool,
) -> Option<(usize, usize, usize)> {
    let first = next_statement_token(tokens, open_index + 1, line_end, allow_newlines)?;
    let statement_start = if allow_newlines
        && opening_brace_has_line_comment(tokens, open_index)
        && matches!(
            tokens.get(first),
            Some(Token::Comment(CommentKind::Line, _))
        ) {
        next_statement_token(tokens, first + 1, line_end, true)?
    } else {
        first
    };
    match tokens.get(statement_start)? {
        Token::Word(word) if language::is_header(word) => return None,
        Token::Comment(_, _) | Token::Preprocessor(_) | Token::Symbol('{') | Token::Symbol('}') => {
            return None;
        }
        _ => {}
    }
    let semicolon = find_statement_semicolon(tokens, statement_start, line_end)?;
    let close_index = next_statement_token(tokens, semicolon + 1, line_end, allow_newlines)?;
    if matches!(tokens.get(close_index), Some(Token::Symbol('}'))) {
        Some((statement_start, semicolon, close_index))
    } else {
        None
    }
}

fn find_statement_semicolon(tokens: &[Token], start: usize, line_end: usize) -> Option<usize> {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    for (index, token) in tokens.iter().enumerate().take(line_end).skip(start) {
        match token {
            Token::Symbol('{') | Token::Comment(_, _) | Token::Preprocessor(_) => return None,
            Token::Symbol('(') => paren_depth += 1,
            Token::Symbol(')') => paren_depth = paren_depth.saturating_sub(1),
            Token::Symbol('[') => bracket_depth += 1,
            Token::Symbol(']') => bracket_depth = bracket_depth.saturating_sub(1),
            Token::Symbol(';') if paren_depth == 0 && bracket_depth == 0 => return Some(index),
            _ => {}
        }
    }
    None
}

pub(super) fn following_operator_after_next_word(
    tokens: &[Token],
    start: usize,
    end: usize,
) -> Option<&str> {
    let word_index = next_non_whitespace(tokens, start, end)?;
    if !matches!(tokens.get(word_index), Some(Token::Word(_))) {
        return None;
    }
    let operator_index = next_non_whitespace(tokens, word_index + 1, end)?;
    match tokens.get(operator_index) {
        Some(Token::Operator(operator)) => Some(operator.as_str()),
        Some(Token::Symbol(':')) => Some(":"),
        _ => None,
    }
}

fn segment_follows_inferred_type(tokens: &[Token], brace_index: usize) -> bool {
    let segment_start = tokens[..brace_index]
        .iter()
        .rposition(|token| matches!(token, Token::Symbol('{' | '}' | ';')))
        .map_or(0, |index| index + 1);
    let mut template_depth = 0usize;
    for index in segment_start..brace_index {
        match template_angle_role(tokens, index, brace_index, template_depth) {
            TemplateAngle::Open => template_depth += 1,
            TemplateAngle::Close(count) => {
                template_depth = template_depth.saturating_sub(count);
            }
            TemplateAngle::None => {}
        }
        if template_depth > 0
            || !matches!(tokens.get(index), Some(Token::Word(word)) if word == "auto")
        {
            continue;
        }
        let inside_decltype = previous_code_token(tokens, index, segment_start).is_some_and(|open| {
            matches!(tokens.get(open), Some(Token::Symbol('(')))
                && previous_code_token(tokens, open, segment_start)
                    .is_some_and(|word| matches!(tokens.get(word), Some(Token::Word(word)) if word == "decltype"))
        });
        if !inside_decltype {
            return true;
        }
    }
    false
}

fn previous_code_token(tokens: &[Token], before: usize, start: usize) -> Option<usize> {
    (start..before).rev().find(|index| {
        !matches!(
            tokens[*index],
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
        )
    })
}

fn next_statement_token(
    tokens: &[Token],
    start: usize,
    end: usize,
    allow_newlines: bool,
) -> Option<usize> {
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token {
            Token::Whitespace(_) => {}
            Token::Newline if allow_newlines => {}
            Token::Newline => return None,
            _ => return Some(index),
        }
    }
    None
}

fn previous_non_layout_token(tokens: &[Token], before: usize) -> Option<usize> {
    (0..before)
        .rev()
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_) | Token::Newline))
}

fn initializer_brace_type(
    tokens: &[Token],
    open_index: usize,
    line_start: usize,
) -> Option<FormatterBraceType> {
    let previous = previous_non_whitespace(tokens, open_index, line_start)?;
    let segment_start = (line_start..open_index)
        .rev()
        .find(|index| matches!(tokens[*index], Token::Symbol('{' | '}' | ';')))
        .map_or(line_start, |index| index + 1);
    let has_block_word = tokens[segment_start..open_index].iter().any(|token| {
        matches!(token, Token::Word(word) if language::BLOCK_WORDS.contains(&word.as_str()) || language::PRE_BLOCK_WORDS.contains(&word.as_str()))
    });
    if tokens[segment_start..open_index]
        .iter()
        .any(|token| matches!(token, Token::Word(word) if word == "enum"))
    {
        return Some(FormatterBraceType::Enum);
    }
    if tokens[segment_start..open_index]
        .iter()
        .any(|token| matches!(token, Token::Operator(operator) if operator == "->"))
    {
        return None;
    }
    match tokens.get(previous)? {
        Token::Operator(operator) if operator == "=" => Some(FormatterBraceType::Array),
        Token::Operator(operator) if operator == ">" && !has_block_word => {
            Some(FormatterBraceType::Init)
        }
        Token::Symbol(',') | Token::Symbol('@') => Some(FormatterBraceType::Array),
        Token::Symbol('(') if !has_block_word => Some(FormatterBraceType::Array),
        Token::Symbol(')') if is_compound_literal_before_brace(tokens, previous, line_start) => {
            Some(FormatterBraceType::CompoundLiteral)
        }
        Token::Symbol(']')
            if is_new_array_initializer_before_brace(tokens, previous, line_start) =>
        {
            Some(FormatterBraceType::Array)
        }
        Token::Symbol(']') | Token::Number(_) if !has_block_word => Some(FormatterBraceType::Init),
        Token::Word(word)
            if !has_block_word
                && !language::is_header(word)
                && !is_asm_block_header(word)
                && !language::PRE_COMMAND_QUALIFIERS.contains(&word.as_str()) =>
        {
            Some(FormatterBraceType::Init)
        }
        _ => None,
    }
}

pub(super) fn previous_non_whitespace(
    tokens: &[Token],
    before: usize,
    line_start: usize,
) -> Option<usize> {
    (line_start..before)
        .rev()
        .find(|index| !matches!(tokens[*index], Token::Whitespace(_)))
}

fn is_compound_literal_before_brace(
    tokens: &[Token],
    close_paren: usize,
    line_start: usize,
) -> bool {
    let Some(open_paren) = matching_open_paren(tokens, close_paren, line_start) else {
        return false;
    };
    if close_paren == open_paren + 1 {
        return false;
    }
    if has_top_level_comma(tokens, open_paren + 1, close_paren) {
        return false;
    }
    let previous_index = previous_non_whitespace(tokens, open_paren, line_start);
    let previous = previous_index.and_then(|index| tokens.get(index));
    let mut depth = 0i32;
    for token in &tokens[line_start..open_paren] {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth -= 1,
            _ => {}
        }
    }
    let nested_compound_context = matches!(
        previous,
        Some(Token::Symbol('(' | ',')) | Some(Token::Operator(_))
    );
    if depth != 0 && !nested_compound_context {
        return false;
    }
    if let Some(Token::Word(word)) = previous {
        return word == language::RETURN;
    }
    if matches!(previous, Some(Token::Symbol(']')))
        || (depth == 0 && matches!(previous, Some(Token::Symbol(')'))))
    {
        return false;
    }
    if let Some(index) = previous_index
        && operator_overload_name_ends_at(tokens, index, line_start)
    {
        return false;
    }
    true
}

fn operator_overload_name_ends_at(tokens: &[Token], end: usize, line_start: usize) -> bool {
    let mut index = end;
    let mut saw_symbol = false;
    loop {
        match tokens.get(index) {
            Some(Token::Whitespace(_)) => {}
            Some(Token::Operator(_)) | Some(Token::Symbol('[' | ']' | '(' | ')')) => {
                saw_symbol = true
            }
            Some(Token::Word(word)) if word == "operator" => return saw_symbol,
            _ => return false,
        }
        if index == line_start {
            return false;
        }
        index -= 1;
    }
}

fn has_top_level_comma(tokens: &[Token], start: usize, end: usize) -> bool {
    let mut depth = 0usize;
    for token in &tokens[start..end] {
        match token {
            Token::Symbol('(') => depth += 1,
            Token::Symbol(')') => depth = depth.saturating_sub(1),
            Token::Symbol(',') if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn is_new_array_initializer_before_brace(
    tokens: &[Token],
    close_bracket: usize,
    line_start: usize,
) -> bool {
    tokens[line_start..close_bracket]
        .iter()
        .any(|token| matches!(token, Token::Word(word) if word == language::NEW))
}

fn matching_open_paren(tokens: &[Token], close_paren: usize, line_start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in (line_start..=close_paren).rev() {
        match tokens.get(index)? {
            Token::Symbol(')') => depth += 1,
            Token::Symbol('(') => {
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

fn format_one_line_block_tokens(
    tokens: &[Token],
    options: &FormatOptions,
    brace_type: Option<FormatterBraceType>,
    opening_body_gap: Option<&str>,
) -> String {
    if is_semicolon_only_one_line_block_tokens(tokens) {
        return tokens
            .iter()
            .map(token_text)
            .collect::<String>()
            .trim_end()
            .to_string();
    }
    let adjusted_tokens = opening_body_gap.and_then(|gap| {
        let first_body = next_non_whitespace(tokens, 1, tokens.len())?;
        if matches!(tokens.get(first_body), Some(Token::Symbol('}'))) {
            return None;
        }
        let mut adjusted = tokens.to_vec();
        adjusted.splice(1..first_body, [Token::Whitespace(gap.to_string())]);
        Some(adjusted)
    });
    let tokens = adjusted_tokens.as_deref().unwrap_or(tokens);
    let mut formatter = FormatEngine::new(options);
    formatter.one_line_block_mode = true;
    if let Some(brace_type) = brace_type {
        formatter.stack_state.brace_type_stack.push(brace_type);
    }
    for (index, token) in tokens.iter().enumerate() {
        let next = next_non_whitespace(tokens, index + 1, tokens.len())
            .and_then(|next_index| tokens.get(next_index));
        let next_is_adjacent = tokens
            .get(index + 1)
            .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
        let following_operator =
            following_operator_after_next_word(tokens, index + 1, tokens.len());
        formatter.token_input.previous_input_was_adjacent = index > 0
            && tokens
                .get(index - 1)
                .is_some_and(|token| !matches!(token, Token::Whitespace(_) | Token::Newline));
        formatter.token_input.previous_input_whitespace = (index > 0)
            .then(|| tokens.get(index - 1))
            .flatten()
            .and_then(|token| match token {
                Token::Whitespace(ws) => Some(ws.clone()),
                _ => None,
            })
            .filter(|_| !matches!(tokens.get(index.wrapping_sub(2)), Some(Token::Newline)));
        formatter.token_input.next_input_whitespace =
            tokens.get(index + 1).and_then(|token| match token {
                Token::Whitespace(ws) => Some(ws.clone()),
                _ => None,
            });
        formatter.token_input.token_begins_source_line = tokens[..index]
            .iter()
            .all(|token| matches!(token, Token::Whitespace(_) | Token::Newline));
        let template_angle = template_angle_role(
            tokens,
            index,
            tokens.len(),
            formatter.line_state.template_angle_depth,
        );
        formatter.push_token(
            token,
            TokenPushContext {
                next,
                next_is_adjacent,
                following_operator,
                template_angle,
                token_index: index,
                starts_initializer_designator: bracket_starts_initializer_designator(
                    tokens,
                    index,
                    tokens.len(),
                ),
                inferred_definition_brace: false,
                following_closing_braces: 0,
            },
        );
    }
    formatter.current.trim_end().to_string()
}

fn token_begins_line(tokens: &[Token], index: usize) -> bool {
    tokens[..index]
        .iter()
        .rev()
        .take_while(|token| !matches!(token, Token::Newline))
        .all(|token| matches!(token, Token::Whitespace(_)))
}

fn is_empty_one_line_block_tokens(tokens: &[Token]) -> bool {
    let significant = significant_one_line_block_tokens(tokens);
    matches!(
        significant.as_slice(),
        [Token::Symbol('{'), Token::Symbol('}')]
    )
}

#[cfg(test)]
mod tests {
    use super::super::token::tokenize;
    use super::*;

    #[test]
    fn add_braces_keeps_braced_condition_interrupted_by_preprocessor() {
        let tokens = tokenize(
            "void run(){if(alphaCondition&&\n#if ENABLED\nbetaCondition\n#endif\nzetaCondition){call();}}\n",
        );

        assert_eq!(add_cross_line_statement_braces(&tokens, true), tokens);
    }
}

fn one_line_block_contains_case_label(tokens: &[Token]) -> bool {
    tokens.iter().enumerate().any(|(index, token)| match token {
        Token::Word(word) if word == "case" => true,
        Token::Word(word) if word == "default" => tokens[index + 1..]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
            .is_some_and(|token| matches!(token, Token::Symbol(':'))),
        _ => false,
    })
}

fn is_semicolon_only_one_line_block_tokens(tokens: &[Token]) -> bool {
    let significant = significant_one_line_block_tokens(tokens);
    matches!(
        significant.as_slice(),
        [Token::Symbol('{'), Token::Symbol(';'), Token::Symbol('}')]
    )
}

fn is_comment_only_one_line_block_tokens(tokens: &[Token]) -> bool {
    let significant = significant_one_line_block_tokens(tokens);
    matches!(
        significant.as_slice(),
        [Token::Symbol('{'), Token::Comment(_, _), Token::Symbol('}')]
            | [
                Token::Symbol('{'),
                Token::Symbol(';'),
                Token::Comment(_, _),
                Token::Symbol('}')
            ]
    )
}

fn one_line_block_contains_lambda_body(tokens: &[Token]) -> bool {
    one_line_block_contains_body_header(tokens, is_lambda_body_header)
}

fn one_line_block_contains_plain_lambda_body(tokens: &[Token]) -> bool {
    one_line_block_contains_body_header(tokens, |head| {
        is_lambda_body_header(head) && !head.contains("->")
    })
}

fn one_line_block_contains_operator_body(tokens: &[Token]) -> bool {
    one_line_block_contains_body_header(tokens, |head| {
        head.trim_end().ends_with(')') && head.contains("operator")
    })
}

fn one_line_block_contains_body_header(
    tokens: &[Token],
    matches_header: impl Fn(&str) -> bool,
) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        if !matches!(token, Token::Symbol('{')) || index == 0 {
            return false;
        }
        let mut head = String::new();
        for token in tokens[..index].iter().rev() {
            if matches!(
                token,
                Token::Symbol(';') | Token::Symbol('{') | Token::Symbol('}')
            ) {
                break;
            }
            head.insert_str(0, &token_text(token));
        }
        matches_header(head.trim_end())
    })
}

fn significant_one_line_block_tokens(tokens: &[Token]) -> Vec<&Token> {
    tokens
        .iter()
        .filter(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
        .collect::<Vec<_>>()
}
