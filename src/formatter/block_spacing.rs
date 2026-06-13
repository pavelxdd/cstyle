use super::FormatEngine;
use super::state::FormatterBraceType;
use super::token::Token;
use crate::config::BraceStyle;
use crate::source::lex::leading_identifier;

#[derive(Default)]
pub(super) struct BlockSpacingState {
    append_blank: bool,
    prepend_blank: bool,
    active_header: Option<String>,
    header_expects_body: bool,
    pending_semicolon: bool,
    pending_one_line_block: bool,
}

impl FormatEngine<'_> {
    pub(super) fn observe_block_spacing_header(&mut self, word: &str) {
        if !self.options.break_blocks {
            return;
        }
        let previous_header = self.block_spacing.active_header.take();
        self.block_spacing.active_header = Some(word.to_string());
        self.block_spacing.header_expects_body = true;

        if self.previous_block_spacing_line_is_comment_only() {
            return;
        }
        if word == "while" && self.stack_state.last_closed_brace_header.as_deref() == Some("do") {
            self.clear_block_spacing_blanks();
            return;
        }
        if is_break_blocks_closing_header(word) {
            if self.options.break_closing_header_blocks
                && self.current_is_blank()
                && self.command_state.previous_command_char == Some('}')
            {
                self.block_spacing.prepend_blank = true;
            }
            return;
        }
        if !self.current_is_blank()
            || (self.command_state.previous_command_char == Some('{')
                && !self.preprocessor.last_output_was_preprocessor)
            || (self.options.brace_style == BraceStyle::Pico
                && self.output.last().is_some_and(|line| line.trim() == "{"))
        {
            return;
        }
        if self.is_break_blocks_opening_header(word)
            && (previous_header.is_none() || self.preprocessor.last_output_was_preprocessor)
        {
            self.block_spacing.prepend_blank = true;
        }
    }

    pub(super) fn observe_block_spacing_comment(&mut self, tokens: &[Token], index: usize) {
        if !self.options.break_blocks || self.previous_block_spacing_line_is_comment_only() {
            return;
        }
        let Some(previous) = self.previous_pre_adjust_line.as_deref() else {
            return;
        };
        let previous = previous.trim_start();
        if previous.is_empty()
            || self.state.indent() == 0
            || (self.command_state.previous_command_char == Some('{') && !previous.starts_with('#'))
        {
            return;
        }
        let Some(word) = self.following_break_blocks_header(tokens, index + 1) else {
            return;
        };
        if self.is_break_blocks_opening_header(&word)
            || (self.options.break_closing_header_blocks && is_break_blocks_closing_header(&word))
        {
            self.block_spacing.prepend_blank = true;
        }
    }

    pub(super) fn should_preserve_block_spacing_comment_blank(
        &self,
        tokens: &[Token],
        following_index: Option<usize>,
    ) -> bool {
        self.options.break_blocks
            && self.current.trim().is_empty()
            && self.previous_block_spacing_line_is_comment_only()
            && following_index
                .filter(|index| matches!(tokens.get(*index), Some(Token::Comment(_, _))))
                .and_then(|index| self.following_break_blocks_header(tokens, index + 1))
                .is_some_and(|word| {
                    self.is_break_blocks_opening_header(&word)
                        || (self.options.break_closing_header_blocks
                            && is_break_blocks_closing_header(&word))
                })
    }

    pub(super) fn schedule_block_spacing_semicolon(&mut self) {
        if self.options.break_blocks {
            self.block_spacing.pending_semicolon = true;
        }
    }

    pub(super) fn observe_block_spacing_semicolon(&mut self) {
        if !self.options.break_blocks
            || !self.block_spacing.header_expects_body
            || self.stack_state.paren_depth > 0
        {
            return;
        }
        let header_appends = self
            .block_spacing
            .active_header
            .as_deref()
            .is_some_and(|header| !matches!(header, "case" | "default"));
        let line_is_broken = self.options.break_one_line_statements
            || (self.line_state.is_one_line_block && self.options.break_one_line_blocks);
        if header_appends && (line_is_broken || !self.line_state.is_multi_statement_line) {
            self.block_spacing.append_blank = true;
        }
        self.clear_block_spacing_header();
    }

    pub(super) fn observe_finished_block_spacing_line(&mut self) {
        if !self.options.break_blocks {
            return;
        }
        if std::mem::take(&mut self.block_spacing.pending_semicolon) {
            self.observe_block_spacing_semicolon();
        }
        if std::mem::take(&mut self.block_spacing.pending_one_line_block) {
            self.block_spacing.append_blank = true;
        }
    }

    pub(super) fn observe_block_spacing_one_line_block(&mut self, brace_type: FormatterBraceType) {
        if !self.options.break_blocks {
            return;
        }
        self.block_spacing.pending_one_line_block = brace_type == FormatterBraceType::Command;
        self.clear_block_spacing_header();
    }

    pub(super) fn observe_block_spacing_open_brace(&mut self) {
        if self.options.break_blocks {
            self.clear_block_spacing_header();
        }
    }

    pub(super) fn observe_block_spacing_inline_close_brace(&mut self) {
        if self.options.break_blocks {
            self.clear_block_spacing_header();
        }
    }

    pub(super) fn observe_block_spacing_close_brace(&mut self) {
        if !self.options.break_blocks {
            return;
        }
        let closed_header = self.stack_state.last_closed_brace_header.as_deref();
        let closed_command_header = self.stack_state.last_closed_brace_type
            == Some(FormatterBraceType::Command)
            || closed_header.is_some_and(|header| {
                is_break_blocks_opening_header(header) || is_break_blocks_closing_header(header)
            });
        if closed_command_header
            && closed_header.is_some_and(|header| !matches!(header, "case" | "default"))
        {
            self.block_spacing.append_blank = true;
        }
        self.clear_block_spacing_header();
    }

    pub(super) fn observe_block_spacing_body_start(&mut self) {
        if self.options.break_blocks {
            self.block_spacing.header_expects_body = false;
        }
    }

    pub(super) fn take_block_spacing_blank(&mut self, line: &str) -> bool {
        if !self.options.break_blocks {
            return false;
        }
        let prepend = std::mem::take(&mut self.block_spacing.prepend_blank);
        let append = std::mem::take(&mut self.block_spacing.append_blank);
        if !prepend && !append {
            return false;
        }
        match self.previous_pre_adjust_line.as_deref() {
            Some(previous) if !previous.trim().is_empty() => {}
            None if prepend => return true,
            _ => return false,
        }
        if prepend {
            return true;
        }
        let trimmed = line.trim_start();
        if let Some(after) = trimmed.strip_prefix('}') {
            let next = leading_identifier(after.trim_start());
            return self.options.break_closing_header_blocks
                && is_break_blocks_closing_header(next);
        }
        let first = leading_identifier(trimmed);
        !is_break_blocks_closing_header(first) || self.options.break_closing_header_blocks
    }

    pub(super) fn reset_block_spacing(&mut self) {
        self.block_spacing = BlockSpacingState::default();
    }

    fn following_break_blocks_header(&self, tokens: &[Token], start: usize) -> Option<String> {
        let stop_on_blank = self.block_spacing.active_header.is_none();
        let mut newline_run = 0usize;
        for token in &tokens[start.min(tokens.len())..] {
            match token {
                Token::Whitespace(_) => {}
                Token::Newline => {
                    newline_run += 1;
                    if stop_on_blank && newline_run >= 2 {
                        return None;
                    }
                }
                Token::Comment(_, _) => newline_run = 0,
                Token::Word(word) => return Some(word.clone()),
                _ => return None,
            }
        }
        None
    }

    fn is_break_blocks_opening_header(&self, word: &str) -> bool {
        is_break_blocks_opening_header(word)
            || self
                .options
                .control_headers
                .iter()
                .any(|header| header == word)
    }

    fn previous_block_spacing_line_is_comment_only(&self) -> bool {
        self.previous_pre_adjust_line
            .as_deref()
            .is_some_and(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("//") || trimmed.starts_with("/*")
            })
    }

    fn clear_block_spacing_blanks(&mut self) {
        self.block_spacing.append_blank = false;
        self.block_spacing.prepend_blank = false;
        self.block_spacing.pending_one_line_block = false;
    }

    fn clear_block_spacing_header(&mut self) {
        self.block_spacing.active_header = None;
        self.block_spacing.header_expects_body = false;
    }
}

pub(super) fn is_break_blocks_closing_header(word: &str) -> bool {
    matches!(
        word,
        "else" | "catch" | "@catch" | "@finally" | "__finally" | "__except" | "finally"
    )
}

pub(super) fn is_break_blocks_opening_header(word: &str) -> bool {
    matches!(
        word,
        "if" | "for" | "while" | "switch" | "do" | "try" | "__try" | "case" | "default"
    )
}
