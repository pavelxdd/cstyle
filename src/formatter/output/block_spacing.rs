use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::constructs::assembly::is_asm_block_header;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::BraceType;
use crate::formatter::structure::blocks::is_code_token;
use crate::source::lex::leading_identifier;

#[derive(Default)]
pub(crate) struct BlockSpacingState {
    append_blank: bool,
    prepend_blank: bool,
    active_header: Option<String>,
    header_expects_body: bool,
    pending_semicolon: bool,
    pending_one_line_block: bool,
    closed_empty_block: bool,
    pending_closed_empty_block: Option<bool>,
    case_block_before_directive: bool,
}

impl FormatEngine<'_> {
    pub(crate) fn observe_block_spacing_header(&mut self, word: &str) {
        if !self.options.break_blocks {
            return;
        }
        let previous_header = self.block_spacing.active_header.take();
        let previous_header_expects_body = self.block_spacing.header_expects_body;
        self.block_spacing.active_header = Some(word.to_string());
        self.block_spacing.header_expects_body = true;

        if self.previous_block_spacing_line_is_comment_only() {
            return;
        }
        if word == "while"
            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")
            && self.layout.command_state.previous_command_char == Some('}')
        {
            self.clear_block_spacing_blanks();
            return;
        }
        if is_break_blocks_closing_header(word) {
            // A body ending in a trailing block comment keeps its closing
            // header.
            if self.current_is_blank()
                && self
                    .layout
                    .previous_pre_adjust_line
                    .as_deref()
                    .is_some_and(|line| {
                        let code = &self.output.code_of(line);
                        !code.trim().is_empty() && line[code.len()..].trim_start().starts_with("/*")
                    })
            {
                self.block_spacing.append_blank = false;
            }
            if self.options.break_closing_header_blocks
                && self.current_is_blank()
                && self.layout.command_state.previous_command_char == Some('}')
                && !self.block_spacing.closed_empty_block
            {
                self.block_spacing.prepend_blank = true;
            }
            return;
        }
        if !self.current_is_blank()
            || (self.layout.command_state.previous_command_char == Some('{')
                && (!self.preprocessor.last_output_was_preprocessor
                    || self.comment_precedes_directives()))
            || (self.options.brace_style == BraceStyle::Pico
                && self.output.last().is_some_and(|line| line.trim() == "{"))
        {
            return;
        }
        // An `if` that a directive splits from its `else` continues the chain.
        if is_break_blocks_opening_header(self.options, word)
            && (previous_header.is_none()
                || self.preprocessor.last_output_was_preprocessor
                    // A header a directive splits from its alternative.
                    && !(previous_header_expects_body
                        && matches!(word, "if" | "while" | "for"))
                    && !(previous_header.as_deref() == Some("else")
                        || matches!(previous_header.as_deref(), Some("case" | "default"))
                            && (!matches!(word, "case" | "default")
                                || self.last_code_line_ends_label())))
        {
            self.block_spacing.prepend_blank = true;
        }
    }

    pub(crate) fn observe_block_spacing_comment(&mut self, tokens: &[Token], index: usize) {
        if !self.options.break_blocks {
            return;
        }
        // Comments before a closing header belong to it.
        if !self.options.break_closing_header_blocks
            && self
                .following_break_blocks_header(tokens, index + 1)
                .is_some_and(|word| is_break_blocks_closing_header(&word))
        {
            self.block_spacing.append_blank = false;
        }
        if self.previous_block_spacing_line_is_comment_only() {
            return;
        }
        let Some(previous) = self.layout.previous_pre_adjust_line.as_deref() else {
            return;
        };
        let previous = previous.trim_start();
        if previous.is_empty()
            || self.layout.indentation.indent() == 0
            || (self.layout.command_state.previous_command_char == Some('{')
                && !previous.starts_with('#'))
        {
            return;
        }
        let Some(word) = self.following_break_blocks_header(tokens, index + 1) else {
            return;
        };
        if is_break_blocks_opening_header(self.options, &word)
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
        if !self.options.break_blocks || !self.current.trim().is_empty() {
            return false;
        }
        // Only the last of a run of empty lines stays.
        let follows_empty_line = self
            .output
            .last()
            .is_some_and(|line| line.trim().is_empty());
        if self.previous_block_spacing_line_is_comment_only()
            && following_index
                .filter(|index| matches!(tokens.get(*index), Some(Token::Comment(_, _))))
                .and_then(|index| self.following_break_blocks_header(tokens, index + 1))
                .is_some_and(|word| {
                    is_break_blocks_opening_header(self.options, &word)
                        || (self.options.break_closing_header_blocks
                            && is_break_blocks_closing_header(&word))
                })
        {
            return true;
        }
        // astyle keeps an empty line before comments that lead to a header,
        // unless a block opens right before.
        if !follows_empty_line
            && self.options.delete_empty_lines
            && self.last_code_line_opens_no_block()
            && following_index
                .filter(|index| matches!(tokens.get(*index), Some(Token::Comment(_, _))))
                .and_then(|index| {
                    tokens[index..].iter().find_map(|token| match token {
                        Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => None,
                        Token::Word(word) => Some(Some(word)),
                        _ => Some(None),
                    })
                })
                .flatten()
                .is_some_and(|word| is_break_blocks_opening_header(self.options, word))
        {
            return true;
        }
        // Deleting the empty line after a comment loses the comment's hold
        // on the header after it, which astyle then breaks away from it
        // again.
        !follows_empty_line
            && self.previous_block_spacing_line_is_comment_only()
            && following_index.is_some_and(|index| match tokens.get(index) {
                Some(Token::Word(word)) => {
                    is_break_blocks_opening_header(self.options, word)
                        && self.last_code_line_opens_no_block()
                }
                // So does a closing header attached to the block's brace.
                Some(Token::Symbol('}')) => {
                    self.options.break_closing_header_blocks
                        && !self.options.break_closing_braces
                        && matches!(
                            self.options.brace_style,
                            BraceStyle::Attach
                                | BraceStyle::OneTrueBrace
                                | BraceStyle::WebKit
                                | BraceStyle::Pico
                                | BraceStyle::Lisp
                        )
                        && matches!(
                            tokens[index + 1..]
                                .iter()
                                .find(|token| !matches!(token, Token::Whitespace(_))),
                            Some(Token::Word(word)) if is_break_blocks_closing_header(word)
                        )
                }
                _ => false,
            })
    }

    /// Whether the last output line holding code, past directives, is a
    /// label with no statement after it.
    fn last_code_line_ends_label(&self) -> bool {
        (0..self.output.len())
            .rev()
            .find(|&index| {
                let trimmed = self.output.trimmed(index);
                !trimmed.is_empty()
                    && !trimmed.starts_with('#')
                    && self.output.directive_of_continuation(index).is_none()
                    && self.output.comment_start_index(index) == index
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with("/*")
            })
            .is_some_and(|index| {
                self.output
                    .code_before_comment(index)
                    .trim_end()
                    .ends_with(':')
            })
    }

    /// Whether the last output line holding code ends other than with `{`.
    fn last_code_line_opens_no_block(&self) -> bool {
        (0..self.output.len())
            .rev()
            .find(|&index| {
                let trimmed = self.output.trimmed(index);
                !trimmed.is_empty()
                    && self.output.comment_start_index(index) == index
                    && !trimmed.starts_with("//")
                    && !trimmed.starts_with("/*")
            })
            .is_some_and(|index| {
                !self
                    .output
                    .code_before_comment(index)
                    .trim_end()
                    .ends_with('{')
            })
    }

    pub(crate) fn schedule_block_spacing_semicolon(&mut self) {
        if self.options.break_blocks {
            self.block_spacing.pending_semicolon = true;
        }
    }

    pub(crate) fn observe_block_spacing_semicolon(&mut self) {
        self.block_spacing.closed_empty_block = false;
        if !self.options.break_blocks
            || !self.block_spacing.header_expects_body
            || self.layout.nesting.paren_depth > 0
        {
            return;
        }
        let header_appends = self
            .block_spacing
            .active_header
            .as_deref()
            .is_some_and(|header| !matches!(header, "case" | "default"));
        let line_is_broken = self.options.break_one_line_statements
            || (self.layout.line_state.is_one_line_block && self.options.break_one_line_blocks);
        // A `do` loop's `while` ends the block its line closes.
        let closes_do_loop = self.block_spacing.active_header.as_deref() == Some("while")
            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do");
        if header_appends
            && (line_is_broken || closes_do_loop || !self.layout.line_state.is_multi_statement_line)
        {
            self.block_spacing.append_blank = true;
        }
        self.clear_block_spacing_header();
    }

    pub(super) fn observe_finished_block_spacing_line(&mut self, ends_statement: bool) {
        if !self.options.break_blocks {
            return;
        }
        if std::mem::take(&mut self.block_spacing.pending_semicolon) {
            self.observe_block_spacing_semicolon();
        }
        // A label kept on one line with its statements ends with them.
        if ends_statement
            && matches!(
                self.block_spacing.active_header.as_deref(),
                Some("case" | "default")
            )
        {
            self.clear_block_spacing_header();
        }
        if std::mem::take(&mut self.block_spacing.pending_one_line_block) {
            self.block_spacing.append_blank = true;
        }
        if let Some(empty) = self.block_spacing.pending_closed_empty_block.take() {
            self.block_spacing.closed_empty_block = empty;
        }
    }

    pub(crate) fn observe_block_spacing_one_line_block(
        &mut self,
        brace_type: BraceType,
        holds_no_code: bool,
    ) {
        if !self.options.break_blocks {
            return;
        }
        // A block of comments alone is as empty once its line ends.
        self.block_spacing.pending_closed_empty_block = Some(holds_no_code);
        self.block_spacing.pending_one_line_block = brace_type == BraceType::Command;
        self.clear_block_spacing_header();
    }

    pub(crate) fn observe_block_spacing_open_brace(&mut self) {
        if self.options.break_blocks {
            self.clear_block_spacing_header();
        }
    }

    pub(crate) fn observe_block_spacing_inline_close_brace(&mut self) {
        if self.options.break_blocks {
            self.clear_block_spacing_header();
        }
    }

    pub(crate) fn observe_block_spacing_close_brace(&mut self, comment_follows: bool) {
        if !self.options.break_blocks {
            return;
        }
        let closed_header = self.layout.nesting.last_closed_brace_header.as_deref();
        // astyle reads an assembly block as no command block.
        let closed_command_header = self.layout.nesting.last_closed_brace_type
            == Some(BraceType::Command)
            && !closed_header.is_some_and(is_asm_block_header)
            || closed_header.is_some_and(|header| {
                is_standard_break_blocks_opening_header(header)
                    || is_break_blocks_closing_header(header)
            });
        // A case block's brace parts from a directive after it.
        self.block_spacing.case_block_before_directive =
            closed_header.is_some_and(|header| matches!(header, "case" | "default"));
        if closed_command_header
            && closed_header.is_some_and(|header| !matches!(header, "case" | "default"))
        {
            // A comment after the brace still ends its line.
            if comment_follows && !self.current_is_blank() {
                self.block_spacing.pending_one_line_block = true;
            } else {
                self.block_spacing.append_blank = true;
            }
        }
        self.clear_block_spacing_header();
    }

    pub(crate) fn observe_block_spacing_body_start(&mut self) {
        if self.options.break_blocks {
            self.block_spacing.header_expects_body = false;
        }
    }

    pub(crate) fn take_block_spacing_blank(&mut self, line: &str) -> bool {
        if !self.options.break_blocks {
            return false;
        }
        let previous_opens = self
            .layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|previous| previous.trim_end().ends_with('{'));
        if line.trim_start().starts_with('}') {
            // A block of comments alone is as empty.
            let holds_no_code = self
                .output
                .pending_tokens()
                .and_then(|span| self.tree.groups.closed_at(span.first))
                .is_some_and(|group| {
                    let open = self.tree.groups.get(group).open;
                    let close = self.tree.groups.get(group).close.unwrap_or(open);
                    !self.tree.tokens[open + 1..close].iter().any(is_code_token)
                });
            // So is one ending in an empty block of its own.
            let ends_empty_block = self
                .layout
                .previous_pre_adjust_line
                .as_deref()
                .is_some_and(|previous| self.output.code_of(previous).trim_end().ends_with("{}"));
            self.block_spacing.closed_empty_block =
                previous_opens || holds_no_code || ends_empty_block;
        }
        let case_block_before_directive =
            std::mem::take(&mut self.block_spacing.case_block_before_directive);
        if case_block_before_directive
            && line.trim_start().starts_with('#')
            && self
                .layout
                .previous_pre_adjust_line
                .as_deref()
                .is_some_and(|previous| previous.trim() == "}")
        {
            self.block_spacing.prepend_blank = false;
            self.block_spacing.append_blank = false;
            return true;
        }
        let prepend = std::mem::take(&mut self.block_spacing.prepend_blank);
        let append = std::mem::take(&mut self.block_spacing.append_blank);
        if !prepend && !append {
            return false;
        }
        match self.layout.previous_pre_adjust_line.as_deref() {
            Some(previous) if !previous.trim().is_empty() => {}
            None if prepend => return true,
            _ => return false,
        }
        if prepend {
            return true;
        }
        let trimmed = line.trim_start();
        // An empty block stays closed up to its closing header.
        let closed_empty_block = self.block_spacing.closed_empty_block;
        if let Some(after) = trimmed.strip_prefix('}') {
            let next = leading_identifier(after.trim_start());
            return self.options.break_closing_header_blocks
                && !self.block_spacing.closed_empty_block
                && is_break_blocks_closing_header(next);
        }
        let first = leading_identifier(trimmed);
        // A body kept on one line with its header ends no block before its
        // closing header.
        !is_break_blocks_closing_header(first)
            || self.options.break_closing_header_blocks
                && self.options.break_one_line_statements
                && !closed_empty_block
    }

    pub(crate) fn reset_block_spacing(&mut self) {
        self.block_spacing = BlockSpacingState::default();
    }

    fn following_break_blocks_header(&self, tokens: &[Token], start: usize) -> Option<String> {
        // Within a switch the lookahead runs past empty lines.
        let stop_on_blank = self.block_spacing.active_header.is_none()
            && self.layout.line_adjuster.switch_depth() == 0;
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

    /// Whether the directive lines just output follow a comment-only
    /// line, which astyle keeps with the header after them.
    fn comment_precedes_directives(&self) -> bool {
        (0..self.output.len())
            .rev()
            .find(|&index| {
                let line = self.output.trimmed(index);
                !line.is_empty() && !line.starts_with('#')
            })
            .is_some_and(|index| {
                let start = self.output.comment_start_index(index);
                let line = self.output.trimmed(start);
                line.starts_with("//") || line.starts_with("/*")
            })
    }

    fn previous_block_spacing_line_is_comment_only(&self) -> bool {
        self.layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                        && trimmed
                            .find("*/")
                            .is_none_or(|close| {
                                let rest = trimmed[close + 2..].trim();
                                rest.is_empty() || rest.starts_with("//") || rest.starts_with("/*")
                            })
                    // The last row of a block comment that opened a line.
                    || !trimmed.is_empty()
                        && self.output.last_non_empty_index().is_some_and(|index| {
                            let start = self.output.comment_start_index(index);
                            start != index && self.output.trimmed(start).starts_with("/*")
                        })
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

pub(crate) fn is_break_blocks_closing_header(word: &str) -> bool {
    matches!(
        word,
        "else" | "catch" | "@catch" | "@finally" | "__finally" | "__except" | "finally"
    )
}

fn is_standard_break_blocks_opening_header(word: &str) -> bool {
    matches!(
        word,
        "if" | "for" | "while" | "switch" | "do" | "try" | "__try" | "case" | "default"
    )
}

fn is_break_blocks_opening_header(options: &FormatOptions, word: &str) -> bool {
    is_standard_break_blocks_opening_header(word)
        || options.control_headers.iter().any(|header| header == word)
}
