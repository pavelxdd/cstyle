use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::constructs::assembly::is_asm_block_header;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{CommentKind, Token};
use crate::formatter::state::BraceType;
use crate::formatter::structure::blocks::{is_code_token, next_code_token};
use crate::formatter::text::line_scan::{
    after_leading_comments, holds_code_after_leading_comments, is_bare_comment_line,
};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::leading_identifier;

#[derive(Default)]
pub(crate) struct BlockSpacingState {
    append_blank: bool,
    prepend_blank: bool,
    active_header: Option<String>,
    header_expects_body: bool,
    /// The active header follows a statement on its line, which leaves the
    /// line unpadded.
    header_follows_statement: bool,
    pending_semicolon: bool,
    pending_one_line_block: bool,
    closed_empty_block: bool,
    pending_closed_empty_block: Option<bool>,
    case_block_before_directive: bool,
    pending_case_block_before_directive: bool,
    /// Depths of blocks opened right after a header's block closed, which
    /// astyle reads as more of that header.
    header_continued_depths: Vec<usize>,
}

impl FormatEngine<'_> {
    pub(crate) fn observe_block_spacing_header(&mut self, word: &str) {
        if !self.options.break_blocks {
            return;
        }
        // A closing header goes on with the statement a `;` before it on
        // the line ended: that `;` ends no header's statement.
        if is_break_blocks_closing_header(word) {
            self.block_spacing.pending_semicolon = false;
        }
        let previous_header = self.block_spacing.active_header.take();
        let previous_header_expects_body = self.block_spacing.header_expects_body;
        self.block_spacing.active_header = Some(word.to_string());
        self.block_spacing.header_expects_body = true;
        self.block_spacing.header_follows_statement = self.header_chain_follows_statement();

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
                        !code.trimmed().is_empty()
                            && line[code.len()..].trimmed_start().starts_with("/*")
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
        // A label after the closer of the case before it parts from nothing.
        let leads_after_closer =
            !matches!(word, "case" | "default") && self.current_holds_closer_and_comments();
        if !(self.current_is_blank() || leads_after_closer)
            || (self.layout.command_state.previous_command_char == Some('{')
                && (!self.preprocessor.last_output_was_preprocessor
                    || self.comment_precedes_directives()))
            || (self.options.brace_style == BraceStyle::Pico
                && self.output.last().is_some_and(|line| line.trimmed() == "{"))
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
        let previous = previous.trimmed_start();
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
        if !self.options.break_blocks || !self.current.trimmed().is_empty() {
            return false;
        }
        // Only the last of a run of empty lines stays.
        let follows_empty_line = self
            .output
            .last()
            .is_some_and(|line| line.trimmed().is_empty());
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
                    .trimmed_end()
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
                    .trimmed_end()
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
        // A line that ends the header's statement takes the blank after it,
        // with any statements kept on the line after that one, unless the
        // header itself follows a statement there.
        if header_appends && !self.block_spacing.header_follows_statement {
            self.block_spacing.append_blank = true;
        }
        self.clear_block_spacing_header();
    }

    pub(crate) fn observe_finished_block_spacing_line(&mut self, ends_statement: bool) {
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
        if std::mem::take(&mut self.block_spacing.pending_case_block_before_directive) {
            self.block_spacing.case_block_before_directive = true;
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
        // A list inside the header's parens leaves the header to its body.
        if !matches!(brace_type, BraceType::Command | BraceType::NonStatement)
            && self.layout.nesting.paren_depth > 0
        {
            return;
        }
        // A block of comments alone is as empty once its line ends.
        self.block_spacing.pending_closed_empty_block = Some(holds_no_code);
        // A case label's block kept on one line parts from a directive
        // after it as one spanning rows does.
        let tokens = &self.tree.tokens;
        self.block_spacing.pending_case_block_before_directive = self
            .current
            .active_token()
            .filter(|&open| matches!(tokens[open], Token::Symbol('{')))
            .and_then(|open| self.tree.groups.opened_at(open))
            .and_then(|group| self.tree.blocks.owner(group))
            .is_some_and(|owner| {
                matches!(&tokens[owner], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
            });
        // Only a header's block is padded.
        let header_block = self
            .block_spacing
            .active_header
            .as_deref()
            .is_some_and(|header| !matches!(header, "case" | "default"));
        self.block_spacing.pending_one_line_block =
            matches!(brace_type, BraceType::Command | BraceType::NonStatement)
                && (header_block && !self.block_spacing.header_follows_statement
                    || self.brace_continues_header_block());
        self.clear_block_spacing_header();
    }

    pub(crate) fn observe_block_spacing_open_brace(&mut self) {
        if self.options.break_blocks {
            if self.brace_continues_header_block() {
                let depth = self.layout.nesting.brace_header_stack.len();
                self.block_spacing.header_continued_depths.push(depth);
            }
            self.clear_block_spacing_header();
        }
    }

    /// Whether the brace being pushed opens a block right after a header's
    /// block closed, which astyle reads as more of that header; a directive
    /// or comment between the blocks ends the header.
    fn brace_continues_header_block(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some((brace, close)) = self.current.active_token().and_then(|brace| {
            tokens[..brace]
                .iter()
                .rposition(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
                .map(|close| (brace, close))
        }) else {
            return false;
        };
        let pads_header = |header: &str| {
            !matches!(header, "case" | "default")
                && (is_standard_break_blocks_opening_header(header)
                    || is_break_blocks_closing_header(header))
        };
        // Braces added around the statement before the brace close its
        // header's block there.
        if self.comments.added_block_semicolon == Some(close) {
            return self.tree.statements.previous_sibling(brace).is_some_and(
                |header| matches!(&tokens[header], Token::Word(word) if pads_header(word)),
            );
        }
        if !matches!(tokens[close], Token::Symbol('}')) {
            return false;
        }
        if self
            .layout
            .nesting
            .last_closed_brace_header
            .as_deref()
            .is_some_and(pads_header)
        {
            return true;
        }
        // A block kept on its header's line closes no block the engine
        // tracks; its owner names the header, and a bare block after a
        // header's block carries that header on.
        let mut close = close;
        loop {
            let Some(group) = self.tree.groups.closed_at(close) else {
                return false;
            };
            let open = self.tree.groups.get(group).open;
            if let Some(owner) = self.tree.blocks.owner(group).filter(|&owner| owner != open) {
                return matches!(&tokens[owner], Token::Word(word) if pads_header(word));
            }
            match tokens[..open]
                .iter()
                .rposition(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
            {
                Some(before) if matches!(tokens[before], Token::Symbol('}')) => close = before,
                _ => return false,
            }
        }
    }

    pub(crate) fn observe_block_spacing_inline_close_brace(&mut self) {
        if self.options.break_blocks {
            self.clear_block_spacing_header();
        }
    }

    /// Whether the block the current `}` closes opened under a case label,
    /// only directives between them.
    fn closed_block_follows_case_label(&self) -> bool {
        let tokens = &self.tree.tokens;
        self.current
            .active_token()
            .filter(|&index| matches!(tokens[index], Token::Symbol('}')))
            .and_then(|close| self.tree.groups.closed_at(close))
            .and_then(|group| self.tree.blocks.owner(group))
            .is_some_and(|owner| {
                matches!(&tokens[owner], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
            })
    }

    pub(crate) fn observe_block_spacing_close_brace(&mut self, next: Option<&Token>) {
        if !self.options.break_blocks {
            return;
        }
        // A comment or a statement kept after the brace ends its line first;
        // a header there pads as its own.
        // A closing header a block comment keeps on the brace's line still
        // takes the padding before it.
        let comment_keeps_closing_header = matches!(next, Some(Token::Comment(CommentKind::Block, _)))
            && self
                .current
                .active_token()
                .and_then(|brace| next_code_token(&self.tree.tokens, brace + 1))
                .is_some_and(|index| {
                    matches!(&self.tree.tokens[index], Token::Word(word) if is_break_blocks_closing_header(word))
                });
        let word_goes_on = |word: &str| {
            !(is_break_blocks_closing_header(word)
                || word == "while"
                || is_break_blocks_opening_header(self.options, word))
        };
        // A block comment between the brace and an opening header or a label
        // on its line leaves that to decide; a closing header there keeps
        // the padding the comment gives it.
        let word_after_comment = matches!(next, Some(Token::Comment(CommentKind::Block, _)))
            .then(|| self.current.active_token())
            .flatten()
            .and_then(|brace| {
                let tokens = &self.tree.tokens;
                next_code_token(tokens, brace + 1)
                    .filter(|&index| {
                        !tokens[brace + 1..index]
                            .iter()
                            .any(|token| matches!(token, Token::Newline))
                    })
                    .and_then(|index| match &tokens[index] {
                        Token::Word(word) => Some(word.as_str()),
                        _ => None,
                    })
            });
        let line_goes_on = match next {
            Some(Token::Word(word)) => word_goes_on(word),
            Some(Token::Comment(..)) => word_after_comment
                .is_none_or(|word| !is_break_blocks_opening_header(self.options, word)),
            Some(Token::Symbol(';' | ',' | ')' | '}') | Token::Newline) | None => false,
            Some(_) => true,
        };
        let closed_header = self.layout.nesting.last_closed_brace_header.as_deref();
        // astyle reads an assembly block as no command block.
        let closed_command_header = self.layout.nesting.last_closed_brace_type
            == Some(BraceType::Command)
            && !closed_header.is_some_and(is_asm_block_header)
            || closed_header.is_some_and(|header| {
                is_standard_break_blocks_opening_header(header)
                    || is_break_blocks_closing_header(header)
            });
        // A case label's block parts from a directive after it, a directive
        // between the label and the brace or not.
        self.block_spacing.case_block_before_directive = closed_header
            .is_some_and(|header| matches!(header, "case" | "default"))
            || self.closed_block_follows_case_label();
        let depth = self.layout.nesting.brace_header_stack.len();
        let continues_header = self.block_spacing.header_continued_depths.last() == Some(&depth);
        if continues_header {
            self.block_spacing.header_continued_depths.pop();
        }
        // A case's block parts from what follows it, save the break that ends
        // the case with it.
        let case_block_ends_case = closed_header
            .is_some_and(|header| matches!(header, "case" | "default"))
            && self
                .current
                .active_token()
                .and_then(|close| next_code_token(&self.tree.tokens, close + 1))
                .is_none_or(
                    |next| matches!(&self.tree.tokens[next], Token::Word(word) if word == "break"),
                );
        if closed_command_header
            && (continues_header || closed_header.is_some() && !case_block_ends_case)
            && !self.closes_one_line_block_after_statement()
        {
            if line_goes_on && !self.current_is_blank() {
                self.block_spacing.pending_one_line_block = true;
                self.block_spacing.append_blank |= comment_keeps_closing_header;
            } else {
                self.block_spacing.append_blank = true;
            }
        }
        self.clear_block_spacing_header();
    }

    /// Whether the line being built holds only a closer and comments after
    /// it: a header after them leads the line's code.
    fn current_holds_closer_and_comments(&self) -> bool {
        self.current
            .trimmed_start()
            .strip_prefix('}')
            .is_some_and(|rest| {
                !rest.trimmed().is_empty() && after_leading_comments(rest).is_empty()
            })
    }

    /// Whether the chain the header being pushed belongs to opened after a
    /// statement on its output line, which leaves the line unpadded.
    fn header_chain_follows_statement(&self) -> bool {
        self.current
            .active_token()
            .is_some_and(|header| self.chain_follows_statement(header))
    }

    /// Whether the chain of the header at `header` opened after a statement
    /// on its output line.
    fn chain_follows_statement(&self, mut header: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        // An `else` goes on with the header whose block closed before it.
        while matches!(&tokens[header], Token::Word(word) if word == "else") {
            let Some(owner) = self
                .tree
                .previous_code_token(header)
                .and_then(|close| groups.closed_at(close))
                .and_then(|group| self.tree.blocks.owner(group))
            else {
                return false;
            };
            header = owner;
        }
        let Some(before) = self.tree.previous_code_token(header) else {
            return false;
        };
        let on_current = |index: usize| {
            self.current
                .tokens()
                .is_some_and(|span| span.first <= index)
        };
        matches!(tokens[before], Token::Symbol(';' | ':'))
            && if on_current(header) {
                on_current(before)
            } else {
                self.output
                    .line_with_token(before)
                    .is_some_and(|line| self.output.line_with_token(header) == Some(line))
            }
    }

    /// Whether the `}` being pushed closes a block the source kept on one
    /// line after a header that followed a statement on its line: astyle
    /// pads no such block.
    fn closes_one_line_block_after_statement(&self) -> bool {
        let groups = &self.tree.groups;
        self.current
            .active_token()
            .and_then(|close| groups.closed_at(close))
            .is_some_and(|group| {
                // The block stands on one row when its brace is on the line
                // being built, wherever the source broke it.
                let open = groups.get(group).open;
                self.output.line_with_token(open).is_none()
                    && self
                        .tree
                        .blocks
                        .owner(group)
                        .is_some_and(|header| self.chain_follows_statement(header))
            })
    }

    pub(crate) fn observe_block_spacing_body_start(&mut self) {
        if self.options.break_blocks {
            self.block_spacing.header_expects_body = false;
        }
    }

    pub(crate) fn take_block_spacing_blank(&mut self, line: &str) -> bool {
        let line_start = line.trimmed_start();
        // A blank due after a block waits for the comment ending its line.
        if !self.options.break_blocks || self.output.pending_line_continues_comment() {
            return false;
        }
        let previous_opens = self
            .layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|previous| previous.trimmed_end().ends_with('{'));
        if line_start.starts_with('}') {
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
                .is_some_and(|previous| self.output.code_trimmed_of(previous).ends_with("{}"));
            // A closer right after an empty block's closes it too, to astyle.
            let follows_empty_block_closer = self.block_spacing.closed_empty_block
                && self
                    .layout
                    .previous_pre_adjust_line
                    .as_deref()
                    .is_some_and(|previous| previous.trimmed() == "}");
            self.block_spacing.closed_empty_block =
                previous_opens || holds_no_code || ends_empty_block || follows_empty_block_closer;
        }
        let case_block_before_directive =
            std::mem::take(&mut self.block_spacing.case_block_before_directive);
        if case_block_before_directive
            && line_start.starts_with('#')
            && self
                .layout
                .previous_pre_adjust_line
                .as_deref()
                .is_some_and(|previous| previous.trimmed_end().ends_with('}'))
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
            Some(previous) if !previous.trimmed().is_empty() => {}
            None if prepend => return true,
            _ => return false,
        }
        if prepend {
            return true;
        }
        let trimmed = line_start;
        // An empty block stays closed up to its closing header.
        let closed_empty_block = self.block_spacing.closed_empty_block;
        if let Some(after) = trimmed.strip_prefix('}') {
            // The header may follow a comment after the brace.
            let tokens = &self.tree.tokens;
            let next = self
                .output
                .pending_tokens()
                .filter(|span| matches!(tokens[span.first], Token::Symbol('}')))
                .and_then(|span| {
                    next_code_token(tokens, span.first + 1).filter(|&next| next <= span.last)
                })
                .and_then(|next| match &tokens[next] {
                    Token::Word(word) => Some(word.as_str()),
                    _ => None,
                })
                .unwrap_or_else(|| leading_identifier(after.trimmed_start()));
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
                Token::Word(word) => return Some(word.to_string()),
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
                // Code after the comment's close makes the line no comment.
                let holds_no_code = if start == index {
                    is_bare_comment_line(line)
                } else {
                    self.output
                        .trimmed(index)
                        .split_once("*/")
                        .is_none_or(|(_, after)| !holds_code_after_leading_comments(after))
                };
                (line.starts_with("//") || line.starts_with("/*")) && holds_no_code
            })
    }

    fn previous_block_spacing_line_is_comment_only(&self) -> bool {
        self.layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|line| {
                let trimmed = line.trimmed_start();
                trimmed.starts_with("//")
                    || trimmed.starts_with("/*")
                        && trimmed
                            .find("*/")
                            .is_none_or(|close| {
                                let rest = trimmed[close + 2..].trimmed();
                                rest.is_empty() || rest.starts_with("//") || rest.starts_with("/*")
                            })
                    // The last row of a block comment that opened a line,
                    // with no code after its close.
                    || !trimmed.is_empty()
                        && trimmed
                            .split_once("*/")
                            .is_none_or(|(_, after)| !holds_code_after_leading_comments(after))
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
        self.block_spacing.header_follows_statement = false;
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
