use crate::formatter::braces::closing::starts_post_closing_declaration;
use crate::formatter::constructs::switch_cases::{
    case_label_with_trailing_comment, split_switch_label_statement,
};
use crate::formatter::continuation::operator_chains;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::PreviousToken;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    is_comment_line, line_comment_split_limit, preprocessor_directive, trailing_comment_split_limit,
};
use crate::formatter::tokens::comments::line_comment_backslash_trailing_space;
use crate::formatter::tokens::literals::first_string_literal_start;
use crate::source::lex::trailing_word;

impl FormatEngine<'_> {
    pub(crate) fn finish_disabled_line(&mut self) {
        let tokens = self.current.take_tokens();
        self.output.set_pending_tokens(tokens);
        let comments = self.current.take_comments();
        self.output.set_pending_comments(comments);
        let line = self.take_current();
        let published = self.output.len();
        self.publish_unadjusted_line(line);
        if self.output.len() > published {
            self.output.mark_last_verbatim();
        }
        self.layout.previous = PreviousToken::None;
        self.previous_was_newline = false;
    }

    pub(crate) fn finish_line(&mut self) {
        let tokens = self.current.take_tokens();
        self.output.set_pending_tokens(tokens);
        let comments = self.current.take_comments();
        self.output.set_pending_comments(comments);
        let published = self.output.len();
        let first = tokens.map(|span| span.first);
        self.publish_finished_line();
        if std::mem::take(&mut self.current_is_verbatim) && self.output.len() > published {
            self.output.mark_last_verbatim();
        }
        // The code length may have split the line over several.
        if first.is_some()
            && let Some(line) = (published..self.output.len())
                .find(|&line| self.output.line_tokens(line).map(|span| span.first) == first)
        {
            self.align_comments_before_statement(line);
        }
        // A finished blank line publishes nothing; its sources must not
        // reach the next line.
        self.output.clear_pending_sources();
        if std::mem::take(&mut self.layout.unwind_else_if_after_line) {
            self.unwind_else_if_break_depths();
        }
    }

    fn publish_finished_line(&mut self) {
        let block_comment_close_paren_ends_declaration =
            self.comments.block_comment_close_paren_ends_declaration;
        self.comments.block_comment_close_paren_ends_declaration = false;
        self.comments
            .previous_block_comment_close_paren_ended_declaration = false;
        let preserve_raw_literal_line_end = self.layout.literal_line.preserve_raw_literal_line_end;
        let preserve_run_in_join_space = self.preserve_run_in_join_space;
        if self.try_finish_multiline_literal_line() {
            return;
        }
        if self.current_is_preindented && self.current.contains('\x0c') {
            let line = self.take_current();
            self.adjust_and_publish_line(line);
            self.reset_after_finished_line();
            return;
        }
        let preserve_line_comment_trailing_space =
            line_comment_backslash_trailing_space(&self.current);
        if !preserve_line_comment_trailing_space
            && !self.layout.literal_line.unterminated_raw_literal
            && !preserve_raw_literal_line_end
            && !preserve_run_in_join_space
        {
            self.trim_current_end();
        }
        if self.try_finish_preindented_comment_line(block_comment_close_paren_ends_declaration) {
            return;
        }
        if self.current_is_preindented {
            self.layout
                .continuation_indent
                .input_line_continuation_indent
                .take();
            let line = self.take_current();
            let trimmed = line.trim_end().to_string();
            if trimmed
                .split_once("*/")
                .is_some_and(|(_, after)| after.trim_end().ends_with(';'))
            {
                self.layout.continuation_indent.next_line_indent_spaces = None;
                self.layout.nesting.clear_continuation_indents();
            }
            if !trimmed.trim().is_empty() {
                self.push_raw_comment_output_line(trimmed);
                if block_comment_close_paren_ends_declaration {
                    self.comments
                        .previous_block_comment_close_paren_ended_declaration = true;
                }
            }
            self.reset_after_finished_line();
            return;
        }

        let line = if preserve_line_comment_trailing_space
            || preserve_raw_literal_line_end
            || preserve_run_in_join_space
        {
            self.current.trim_start().to_string()
        } else {
            self.current.trim().to_string()
        };
        self.finish_ordinary_line(&line);
    }

    fn finish_ordinary_line(&mut self, line: &str) {
        if !line.is_empty() {
            let output_line_index = self.output.len();
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let code_ends_with_brace = code.ends_with('}');
            self.layout
                .frame_stack
                .mark_last_closed_brace_line_end(output_line_index, code_ends_with_brace);
            self.observe_operator_chain_line_context(output_line_index, code);
            let clear_string_after_line = !code.trim_start().starts_with('#')
                && (code.ends_with(';') || first_string_literal_start(code).is_none());
            let clear_stream_after_line = code.ends_with(';');
            if self.layout.line_state.column1_line_comment
                && !self.options.indent_col1_comments
                && line.starts_with("//")
                && !(self.options.indent_preproc_block
                    && self.preprocessor.indented_block_stack.last() == Some(&true))
            {
                if self.take_block_spacing_blank(line) {
                    self.push_empty_line();
                }
                self.publish_unadjusted_line(line.to_string());
                if let Some(output_indent) =
                    self.observe_operator_chain_output_line(output_line_index)
                {
                    self.layout
                        .frame_stack
                        .mark_delimiter_line_output_indent(output_line_index, output_indent);
                }
                if clear_string_after_line {
                    self.layout.frame_stack.clear_string_continuations();
                }
                self.layout.run_in_state.current_run_in_indent =
                    self.layout.continuation_indent.next_line_indent;
                self.reset_after_finished_line();
                return;
            }

            let case_label_with_comment = case_label_with_trailing_comment(line);
            if self.options.break_one_line_statements
                && let Some((label, mut statement)) = split_switch_label_statement(line)
            {
                self.finish_line_text(&label);
                // Each of several labels on a line takes a line of its own.
                while (statement.starts_with("case ") || statement.starts_with("default"))
                    && let Some((label, rest)) = split_switch_label_statement(&statement)
                {
                    self.finish_line_text(&label);
                    statement = rest;
                }
                if statement.trim_start().starts_with('#') {
                    self.adjust_and_publish_line(statement.trim_start().to_string());
                    self.preprocessor.last_output_was_preprocessor = true;
                } else {
                    let label_spaces = self
                        .output
                        .last()
                        .map(|line| leading_visual_width(line, self.options.tab_width))
                        .unwrap_or(0);
                    let extra = if self.unmatched_closing_brace_recovery {
                        0
                    } else if statement.trim_start().starts_with([
                        '<', '>', '|', '&', '+', '-', '*', '/', '%', '=', '!', '?', ':', ',', '.',
                        '~',
                    ]) {
                        self.options.indent_width * 2
                    } else {
                        self.options.indent_width
                    };
                    self.layout.continuation_indent.next_line_indent = None;
                    self.layout.continuation_indent.next_line_indent_spaces =
                        Some(label_spaces + extra);
                    self.finish_line_text(&statement);
                }
            } else {
                self.finish_line_text(line);
            }
            self.reset_continuation_after_output_line(line, output_line_index);
            self.reset_continuation_after_directive_rows(output_line_index);
            // A closing brace line that ends its declaration, as `} name;`,
            // used the column it was given.
            if code.trim_start().starts_with('}')
                && code.ends_with(';')
                && self.layout.indentation.statement_depth() == 0
                && self.layout.nesting.paren_depth == 0
            {
                self.layout.continuation_indent.next_line_indent_spaces = None;
            }
            if clear_stream_after_line {
                operator_chains::clear_operator_chain_frames(&mut self.layout.frame_stack);
            }
            if clear_string_after_line {
                self.layout.frame_stack.clear_string_continuations();
            }
            self.observe_finished_block_spacing_line(code.ends_with(';'));
            if let Some(spaces) = self
                .layout
                .continuation_indent
                .clear_continuation_after_line
                .take()
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
                self.layout.nesting.clear_continuation_indents();
            }
            if case_label_with_comment && let Some(previous) = self.output.last() {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        if self.unmatched_closing_brace_recovery {
            self.layout.continuation_indent.set_next_line_spaces(0);
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
            operator_chains::clear_operator_chain_state(
                &mut self.layout.frame_stack,
                &mut self.layout.continuation_indent.logical_chain_indent_spaces,
            );
        }
        self.layout.run_in_state.current_run_in_indent =
            self.layout.continuation_indent.next_line_indent;
        self.max_length_line.reset();
        self.layout.literal_line.preserve_raw_literal_line_end = false;
        self.preserve_run_in_join_space = false;
        self.reset_after_finished_line();
    }

    fn reset_continuation_after_output_line(&mut self, line: &str, output_line_index: usize) {
        if let Some(output_indent) = self.observe_operator_chain_output_line(output_line_index)
            && let Some(output_line) = self.output.get(output_line_index)
        {
            self.layout
                .frame_stack
                .mark_delimiter_line_output_indent(output_line_index, output_indent);
            let output_code = output_line[..trailing_comment_split_limit(output_line)].trim_end();
            let line_comment_limit = line_comment_split_limit(line);
            let code_before_line_comment = line[..line_comment_limit].trim_end();
            // A `#` in a literal is no directive.
            let holds_directive = self
                .output
                .line_tokens(output_line_index)
                .is_none_or(|span| {
                    self.tree.tokens[span.first..=span.last.min(self.tree.tokens.len() - 1)]
                        .iter()
                        .any(|token| matches!(token, Token::Preprocessor(_)))
                });
            let embedded_preprocessor = holds_directive
                && (has_hash_outside_literals(output_code)
                    && !output_code.trim_start().starts_with('#')
                    || (line_comment_limit < line.len() || line.trim_end().ends_with(':'))
                        && has_hash_outside_literals(code_before_line_comment)
                        && !code_before_line_comment.trim_start().starts_with('#'));
            if output_code.trim_start().starts_with("return ")
                && holds_directive
                && has_hash_outside_literals(output_code)
                && !output_code.ends_with(';')
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces =
                    Some(output_indent + "return ".len());
            } else if embedded_preprocessor {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces =
                    Some(if output_code.ends_with(':') {
                        output_indent + self.options.indent_width
                    } else {
                        output_indent
                    });
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_stream_frames_and_logical_indent(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            } else if starts_post_closing_declaration(output_code) {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(output_indent);
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_operator_chain_state(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            }
            if output_code.trim_start().starts_with("else,") {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(0);
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_operator_chain_state(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            }
            if output_code.trim_start().starts_with("#define") && !output_code.ends_with('\\') {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = None;
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_operator_chain_state(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            }
            if preprocessor_directive(output_code.trim_start()) == Some("endif")
                && let Some(previous) = self.output[..output_line_index]
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                && (is_comment_line(previous.trim_start())
                    || previous.trim_start().starts_with("/*"))
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces =
                    Some(leading_visual_width(previous, self.options.tab_width));
            }
            if output_code.trim() == "?" {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(output_indent);
                self.layout.nesting.clear_continuation_indents();
            }
            if output_code.trim() == "catch"
                && self.output[..output_line_index]
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_none_or(|line| {
                        !line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .ends_with('}')
                    })
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(output_indent);
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_operator_chain_state(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            }
            if output_code.ends_with("; catch") {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces =
                    Some(self.layout.indentation.indent() * self.options.indent_width);
                self.layout.nesting.clear_continuation_indents();
                operator_chains::clear_operator_chain_state(
                    &mut self.layout.frame_stack,
                    &mut self.layout.continuation_indent.logical_chain_indent_spaces,
                );
            }
        }
    }

    fn reset_continuation_after_directive_rows(&mut self, output_line_index: usize) {
        for line_index in output_line_index..self.output.len() {
            self.observe_ternary_colon_output_line(line_index);
            let output_line = &self.output[line_index];
            let output_code = output_line[..trailing_comment_split_limit(output_line)].trim_end();
            if output_code.trim_start().starts_with("return ")
                && has_hash_outside_literals(output_code)
                && !output_code.ends_with(';')
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(
                    leading_visual_width(output_line, self.options.tab_width) + "return ".len(),
                );
            }
            if output_code.trim_start() == "#else"
                && let Some(previous_line) = self.output[..line_index]
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                && trailing_word(previous_line.trim_end()) == "do"
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(
                    leading_visual_width(previous_line, self.options.tab_width)
                        + self.options.indent_width * 2,
                );
            }
        }
    }

    /// astyle fills an empty line inside braces with the indent of the line
    /// before it, and leaves one at file scope empty. Returns where the fill
    /// of each whitespace-only line came from.
    fn fill_empty_lines(&mut self) -> Vec<EmptyFillSource> {
        let mut sources = Vec::new();
        if !self.options.empty_line_fill {
            return sources;
        }
        // astyle indents each #else and #elif branch with a copy of the
        // state at its #if, and fills an empty line from the state outside
        // every such branch, or from the branch's copy when that is at file
        // scope. A state is the brace depth and the lead of its last line.
        let mut root = (0isize, String::new());
        let mut active: Vec<(isize, String)> = Vec::new();
        let mut waiting: Vec<(isize, String)> = Vec::new();
        let mut conditionals: Vec<(usize, usize)> = Vec::new();
        // The fill of each open conditional that indents its block.
        let mut preprocessor_block_fills: Vec<Option<String>> = Vec::new();
        // The braces of a directive's continued lines open no block.
        let mut continues_directive = false;
        for index in 0..self.output.len() {
            let line = &self.output[index];
            let in_directive = continues_directive;
            continues_directive = (in_directive || line.trim_start().starts_with('#'))
                && line.trim_end().ends_with('\\');
            if in_directive {
                // The empty line a directive continues into stays empty.
                if line.trim().is_empty() {
                    sources.push(EmptyFillSource::Kept);
                    if !line.is_empty() && !self.output.is_verbatim(index) {
                        self.output.set(index, String::new());
                    }
                }
                continue;
            }
            if line.trim().is_empty() {
                // A form feed stays as the page break it marks.
                if self.output.is_verbatim(index) || line.contains('\u{c}') {
                    sources.push(EmptyFillSource::Kept);
                    continue;
                }
                let branch = active.last().filter(|state| state.0 > 0);
                let (fill, source) = if root.0 > 0 {
                    (root.1.clone(), EmptyFillSource::Root)
                } else if let Some(state) = branch {
                    (state.1.clone(), EmptyFillSource::Branch)
                } else if let Some(Some(fill)) = preprocessor_block_fills.last() {
                    (fill.clone(), EmptyFillSource::Kept)
                } else {
                    (String::new(), EmptyFillSource::Kept)
                };
                sources.push(source);
                self.output.set(index, fill);
                continue;
            }
            let state = active.last_mut().unwrap_or(&mut root);
            let meta = self.output.brace_meta(index);
            // The rest of a block comment keeps the indent of its first line;
            // after one that trailed code astyle leaves the next empty line
            // empty.
            let comment_start = self.output.comment_start_index(index);
            if comment_start != index {
                if !self.output.trimmed(comment_start).starts_with("/*") {
                    state.1.clear();
                }
                continue;
            }
            if !meta.code_starts_with_hash {
                state.1 = line[..line.len() - line.trim_start().len()].to_string();
            }
            let code = line.trim_start();
            let starts_word = |name: &str| {
                code.strip_prefix(name).is_some_and(|rest| {
                    !rest.starts_with(|ch: char| ch == '_' || ch.is_alphanumeric())
                })
            };
            if self
                .options
                .macro_blocks
                .iter()
                .any(|(begin, _)| starts_word(begin))
            {
                state.0 += 1;
            } else if self
                .options
                .macro_blocks
                .iter()
                .any(|(_, end)| starts_word(end))
            {
                state.0 -= 1;
            }
            if meta.code_starts_with_hash {
                match preprocessor_directive(line.trim_start()) {
                    Some("if" | "ifdef" | "ifndef") => {
                        let depth = state.0;
                        waiting.push(state.clone());
                        conditionals.push((waiting.len() - 1, active.len()));
                        preprocessor_block_fills.push(self.preprocessor_block_fill(index, depth));
                    }
                    Some("else") => {
                        if let Some(state) = waiting.pop() {
                            active.push(state);
                        }
                    }
                    Some("elif") => {
                        if let Some(state) = waiting.last() {
                            active.push(state.clone());
                        }
                    }
                    Some("endif") => {
                        if let Some((waiting_len, active_len)) = conditionals.pop() {
                            waiting.truncate(waiting_len);
                            active.truncate(active_len);
                        }
                        preprocessor_block_fills.pop();
                    }
                    _ => {}
                }
                continue;
            }
            state.0 += meta.opens as isize - meta.closes as isize;
        }
        sources
    }

    /// The fill of empty lines in the block the conditional on output
    /// line `index` opens at file scope: astyle fills them with the indent of
    /// a block it indents.
    fn preprocessor_block_fill(&self, index: usize, depth: isize) -> Option<String> {
        if !self.options.indent_preproc_block || depth > 0 {
            return None;
        }
        let tab_width = self.options.tab_width;
        let lead = self.output.lead_width(index, tab_width);
        let mut end = index;
        while self.output[end].trim_end().ends_with('\\') && end + 1 < self.output.len() {
            end += 1;
        }
        let next =
            (end + 1..self.output.len()).find(|&next| !self.output[next].trim().is_empty())?;
        (self.output.lead_width(next, tab_width) > lead).then(|| {
            let line = &self.output[index];
            // The output is retabbed by now: the fill takes the output's tabs.
            let mut output_options = self.options.clone();
            output_options.indent_style = self.output_indent_style;
            format!(
                "{}{}",
                &line[..line.len() - line.trim_start().len()],
                output_options.indent_prefix(1)
            )
        })
    }

    /// The finished text, and where the fill of each of its whitespace-only
    /// lines came from.
    pub(crate) fn finish(mut self) -> (String, Vec<EmptyFillSource>) {
        // Whole-output passes look at every construct.
        self.output.clear_scope();
        self.flush_backslash_body_parts();
        self.merge_source_run_in_braces();
        self.merge_run_in_comment_braces();
        self.fuse_adjacent_braces();
        self.attach_statement_expression_braces();
        self.align_comments_before_case_labels();
        self.retab_output();
        let fill_sources = self.fill_empty_lines();
        if self.output.is_empty() {
            (String::new(), fill_sources)
        } else {
            let mut output = self.output.join(self.options.line_break());
            output.push_str(self.options.line_break());
            (output, fill_sources)
        }
    }
}

/// Which state of astyle's beautifiers an empty line takes its fill from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EmptyFillSource {
    /// The code outside every #else and #elif branch.
    Root,
    /// The innermost #else or #elif branch.
    Branch,
    /// A fill that no later move of lines changes.
    Kept,
}
