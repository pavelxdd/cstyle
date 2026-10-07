use crate::config::{BraceStyle, FormatOptions, PointerAlign, ReferenceAlign};
use crate::formatter::braces::classification::ExternCGuard;
use crate::formatter::braces::initializers::initializer_brace_line_comment_gap;
use crate::formatter::braces::postprocess::horstmann_run_in_fill;
use crate::formatter::braces::rewrite::is_standard_add_braces_header;
use crate::formatter::constructs::headers::is_header;
use crate::formatter::constructs::labels;
use crate::formatter::constructs::switch_cases::find_case_colon;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{CommentKind, Token, token_char_len};
use crate::formatter::output::block_spacing::is_break_blocks_closing_header;
use crate::formatter::output::buffer::LineFilter;
use crate::formatter::preprocessor::PreprocessorRegion;
use crate::formatter::state::frame::{
    BraceSemanticKind, CommentFrame, CommentFrameKind, ParenRole,
};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::is_code_token;
use crate::formatter::structure::blocks::next_code_token;
use crate::formatter::syntax::language;
use crate::formatter::text::columns::{
    drop_leading_columns, leading_visual_width, visual_column_at, visual_width_from,
};
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, has_unmatched_open_brace, is_comment_line, is_comment_only_line,
    line_brace_imbalance, line_ends_with_comment, preprocessor_directive,
    trailing_comment_split_limit, unmatched_open_brace_content_offset,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::disabled_formatting::DisabledFormattingState;
use crate::formatter::tokens::operators::{
    find_assignment_operator, head_ends_assignment_operator, head_ends_binary_operator,
    starts_with_chain_operator,
};

fn comment_starts_header_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|tail| tail.starts_with(|ch: char| ch == '(' || ch.is_whitespace()))
}

fn post_closing_declaration_owns_comment(line: &str) -> bool {
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
    !matches!(word, "else" | "catch" | "while" | "__finally" | "__except")
}

pub(crate) fn line_comment_backslash_trailing_space(line: &str) -> bool {
    if !line.chars().next_back().is_some_and(char::is_whitespace) {
        return false;
    }
    let comment = trailing_comment_split_limit(line);
    let tail = line[comment..].trimmed_start();
    comment < line.len() && tail.starts_with("//") && tail.trimmed_end().ends_with('\\')
}

/// Indent candidates for a comment token, computed before it is placed.
#[derive(Clone, Copy)]
struct CommentIndents {
    standalone_line_comment: bool,
    function_try_initializer: bool,
    open_paren: Option<usize>,
    line_continuation: Option<ContinuationIndent>,
    line_stream_chain: Option<usize>,
    ternary_branch: Option<usize>,
    definition_header: Option<usize>,
    case_label: Option<usize>,
    column1_case_label: Option<usize>,
    user_label: Option<usize>,
    control_header: Option<usize>,
    block_continuation: Option<usize>,
    lambda_parameter: Option<usize>,
}

/// How an own-line multi-line block comment is reindented, decided from its
/// opening line.
#[derive(Clone, Copy)]
struct OwnLineBlockComment<'a> {
    opener_prefix: &'a str,
    opener_output_column: usize,
    trim_amount: usize,
    run_in_opener: bool,
    unindented_namespace_run_in_comment: bool,
}

impl FormatEngine<'_> {
    pub(crate) fn schedule_run_in_comment_brace_merge(&mut self, brace_line: usize) {
        self.comments.run_in_comment_brace_lines.push(brace_line);
    }

    /// Whether the comment starting on output line `line` is to join the
    /// brace line before it.
    pub(crate) fn comment_runs_into_brace(&self, line: usize) -> bool {
        line.checked_sub(1)
            .is_some_and(|brace| self.comments.run_in_comment_brace_lines.contains(&brace))
    }

    pub(crate) fn merge_run_in_comment_braces(&mut self) {
        let mut indices = std::mem::take(&mut self.comments.run_in_comment_brace_lines);
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            let Some(comment_line) = self.output.get(index + 1) else {
                continue;
            };
            let trimmed = comment_line.trimmed_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*"))
                || trimmed.contains("*INDENT-OFF*")
            {
                continue;
            }
            if self.output[index].trimmed() != "{" {
                continue;
            }
            let comment_line = self.output.remove(index + 1);
            let fill = horstmann_run_in_fill(&self.output[index], &comment_line, self.options);
            let merged = format!(
                "{}{}{}",
                self.output[index],
                fill,
                comment_line.trimmed_start()
            );
            self.output.set(index, merged);
        }
    }

    pub(crate) fn push_raw_comment_output_line(&mut self, line: String) {
        if self.take_block_spacing_blank(&line) {
            self.push_empty_line();
        }
        self.layout.line_adjuster.observe_raw_comment_line(&line);
        self.adjust_and_publish_line(line);
    }

    pub(crate) fn align_adjacent_block_comments_before_adjustment(&self, line: String) -> String {
        let trimmed = line.trimmed_start();
        let adjacent = trimmed
            .strip_prefix("/*")
            .and_then(|rest| rest.split_once("*/"))
            .is_some_and(|(_, after)| after.trimmed_start().starts_with("/*"));
        if !adjacent {
            return line;
        }
        let target = self
            .layout
            .nesting
            .current_continuation_indent_spaces()
            .or_else(|| self.active_body_comment_indent_spaces())
            .unwrap_or_else(|| {
                (self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width
            });
        if leading_visual_width(&line, self.options.tab_width) > target {
            format!("{}{}", " ".repeat(target), trimmed)
        } else {
            line
        }
    }

    pub(crate) fn observe_raw_output_comment_frame(&mut self, line: &str) {
        let output_spaces = leading_visual_width(line, self.options.tab_width);
        self.observe_output_comment_frame(line, output_spaces, false);
    }

    pub(crate) fn observe_formatted_output_comment_frame(
        &mut self,
        line: &LineView<'_>,
        output_spaces: usize,
    ) {
        self.observe_output_comment_frame(line, output_spaces, true);
    }

    pub(crate) fn line_comment_continuation_anchor_column(&self) -> Option<usize> {
        self.layout
            .frame_stack
            .active_comment()
            .and_then(|frame| frame.continuation_anchor_column)
    }

    fn observe_output_comment_frame(
        &mut self,
        line: &str,
        output_spaces: usize,
        record_split_else_indent: bool,
    ) {
        let trimmed = line.trimmed_start();
        if trimmed.starts_with("//") {
            if let Some(frame) = self.layout.frame_stack.active_comment_mut() {
                frame.continuation_anchor_column = Some(output_spaces);
            }
            if record_split_else_indent {
                self.record_split_else_comment_body_indent(line, output_spaces);
            }
        } else if let Some(frame) = self.layout.frame_stack.active_comment()
            && !trimmed.is_empty()
            && !is_comment_line(trimmed)
            // Code standing left of the comments ends the block they were in,
            // whatever trails it.
            && (!line_ends_with_comment(trimmed)
                || frame
                    .continuation_anchor_column
                    .is_some_and(|anchor| output_spaces < anchor))
        {
            self.layout.frame_stack.clear_comments();
        }
    }

    pub(crate) fn split_else_comment_row_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !is_comment_line(line.trimmed_start()) {
            return None;
        }
        let mut result = self.split_else_preprocessor_branch_body_indent_spaces();
        if let Some(previous) = self.output.last_line_outside_comment() {
            let previous_code = self.output.code_trimmed_of(previous);
            let previous_trimmed = previous_code.trimmed_start();
            if previous_trimmed == "else" || previous_trimmed.ends_with("} else") {
                result = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        result
    }

    pub(crate) fn none_style_post_comment_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line_start.starts_with_any(b"#{}")
            || is_comment_line(line_start)
        {
            return None;
        }
        let mut comment_indent = None;
        for previous in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
        {
            if is_comment_line(previous.trimmed_start()) {
                comment_indent = Some(leading_visual_width(previous, self.options.tab_width));
                continue;
            }
            let previous_trimmed = previous.trimmed();
            if (previous_trimmed == "else" || previous_trimmed.ends_with("} else"))
                && let Some(spaces) = comment_indent
            {
                let else_indent = leading_visual_width(previous, self.options.tab_width);
                return Some(if spaces > else_indent {
                    spaces
                } else {
                    else_indent + self.options.indent_width
                });
            }
            break;
        }
        None
    }

    pub(crate) fn split_else_immediate_post_comment_indent_floor(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line_start.starts_with_any(b"#{}")
            || is_comment_line(line_start)
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !is_comment_line(previous.trimmed_start()) {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces.unwrap_or(output_spaces) < target).then_some(target)
    }

    pub(crate) fn structural_split_else_post_comment_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: usize,
        body_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        if !structural_split_else_chain || line.trimmed_start().starts_with('#') {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        (previous_spaces == body_spaces
            && is_comment_line(previous.trimmed_start())
            && current_spaces < body_spaces)
            .then_some(body_spaces)
    }

    pub(crate) fn preprocessor_else_comment_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trimmed_start().starts_with_any(b"#{}") {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if preprocessor_directive(previous.trimmed_start()) != Some("endif") {
            return None;
        }
        let mut before_lines = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .filter(|line| !line.trimmed().is_empty());
        let before = before_lines.next()?;
        ((is_comment_line(before.trimmed_start()) || before.trimmed_start().starts_with("/*"))
            && before_lines
                .next()
                .is_some_and(|line| preprocessor_directive(line.trimmed_start()) == Some("else")))
        .then(|| leading_visual_width(before, self.options.tab_width))
    }

    pub(crate) fn try_finish_preindented_comment_line(
        &mut self,
        close_paren_ends_declaration: bool,
    ) -> bool {
        if !self.current_is_preindented || !is_comment_line(&self.current) {
            return false;
        }
        let line_continuation_indent = self
            .layout
            .continuation_indent
            .input_line_continuation_indent
            .take()
            .map(|indent| indent.columns(self.options.indent_width))
            .map(|captured| {
                self.layout
                    .frame_stack
                    .active_delimiter()
                    .filter(|frame| frame.opener_output_line < self.output.len())
                    .and_then(|frame| frame.continuation_indent_column)
                    .unwrap_or(captured)
            });
        let line = self.take_current();
        let mut trimmed = line.trimmed_end().to_string();
        if trimmed.trimmed_start().starts_with("/*") {
            let follows_switch = self.output.last_non_empty_scoped().is_some_and(|line| {
                let code = self.output.code_trimmed_of(line);
                code.trimmed_start().starts_with("switch") && code.ends_with('{')
            });
            if !follows_switch {
                if let Some(spaces) = self.split_else_preprocessor_branch_body_indent_spaces() {
                    let prefix = self
                        .options
                        .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                    trimmed = format!("{prefix}{}", trimmed.trimmed_start());
                } else if let Some(spaces) = line_continuation_indent.or_else(|| {
                    // A control header's condition placed its comments.
                    (!self
                        .layout
                        .frame_stack
                        .active_delimiter()
                        .is_some_and(|frame| frame.role == ParenRole::Header))
                    .then(|| self.recent_paren_continuation_indent_spaces())
                    .flatten()
                }) {
                    let prefix = self
                        .options
                        .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                    trimmed = format!("{prefix}{}", trimmed.trimmed_start());
                }
            }
        }
        if trimmed
            .split_once("*/")
            .is_some_and(|(_, after)| after.trimmed_end().ends_with(';'))
        {
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.layout.nesting.clear_continuation_indents();
        }
        if !trimmed.trimmed().is_empty() {
            let closes_standalone_block_comment = trimmed.trimmed_start().starts_with("*/");
            let frame_column = self
                .layout
                .frame_stack
                .active_comment()
                .filter(|frame| frame.kind == CommentFrameKind::Block && frame.multiline)
                .map(|frame| frame.output_column);
            let code_follows_comment = trimmed
                .split_once("*/")
                .is_some_and(|(_, after)| !after.trimmed().is_empty());
            if let Some((head, tail, spaces)) = self.split_comment_led_line(&trimmed) {
                self.push_raw_comment_output_line(head);
                let level = self.layout.indentation.indent();
                self.push_formatted_line_exact(&tail, level, spaces);
            } else {
                self.push_raw_comment_output_line(trimmed);
            }
            // The line after a comment closes at the column the comment opened.
            let next_indent = closes_standalone_block_comment.then(|| {
                frame_column.unwrap_or_else(|| {
                    self.output
                        .comment_indent_width(self.output.len() - 1, self.options.tab_width)
                }) + self.layout.line_adjuster.total_case_unindent_depth()
                    * self.options.indent_width
            });
            if close_paren_ends_declaration {
                self.comments
                    .previous_block_comment_close_paren_ended_declaration = true;
            }
            if let Some(spaces) = next_indent {
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
            // A call the code after the comment closes ends the statement
            // as on any line.
            if code_follows_comment
                && let Some(spaces) = self
                    .layout
                    .continuation_indent
                    .clear_continuation_after_line
                    .take()
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
                self.layout.nesting.clear_continuation_indents();
            }
        }
        self.reset_after_finished_line();
        true
    }

    /// Whether the last non-empty output line is `} name` closing a block at
    /// file scope, which a following comment belongs after.
    fn last_line_closes_top_level_declaration(&self) -> bool {
        self.output
            .last_non_empty_scoped()
            .is_some_and(|line| post_closing_declaration_owns_comment(line))
            && !self.last_line_closes_nested_block()
    }

    pub(crate) fn push_inline_comment(&mut self, comment: &str) {
        if self.token_input.previous_input_was_adjacent {
            self.trim_current_end();
            if comment.trimmed_start().starts_with("//") {
                self.ensure_space();
            }
        } else {
            self.emit_source_space();
        }
        self.current.push_str(comment.trimmed_end());
        self.emit_trailing_source_space();
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    fn comment_frame_kind(kind: CommentKind) -> CommentFrameKind {
        match kind {
            CommentKind::Line => CommentFrameKind::Line,
            CommentKind::Block => CommentFrameKind::Block,
        }
    }

    fn record_comment_frame(&mut self, kind: CommentKind, output_column: usize, multiline: bool) {
        let continuation_anchor_column = (kind == CommentKind::Line)
            .then(|| {
                self.layout
                    .frame_stack
                    .active_comment()
                    .and_then(|frame| frame.continuation_anchor_column)
            })
            .flatten();
        self.layout.frame_stack.push_comment(CommentFrame {
            kind: Self::comment_frame_kind(kind),
            output_column,
            multiline,
            continuation_anchor_column,
        });
    }

    pub(super) fn reindent_trailing_comment(&mut self, line_kind: LineKind) -> bool {
        let mut end = self.output.len();
        while end > 0 && self.output[end - 1].trimmed().is_empty() {
            end -= 1;
        }
        if end == 0 {
            return false;
        }
        let last = self.output[end - 1].trimmed_start();
        // Code or a second block comment after a comment, as
        // `/* note */ MACRO`, makes the line a statement of the code before it.
        if text_follows_comment_close(last) {
            return false;
        }
        let mut indent = (self.layout.indentation.line_indent(line_kind, self.options)
            + self.case_body_indent_extra(line_kind))
        .saturating_sub(self.layout.line_adjuster.pending_case_unindent());
        if last.starts_with("//") {
            let mut start = end - 1;
            while start > 0 && self.output[start - 1].trimmed_start().starts_with("//") {
                start -= 1;
            }
            let prefix = self.options.indent_prefix(indent);
            for line in self.output.range_mut(start..end) {
                if line.starts_with("//") {
                    continue;
                }
                *line = format!("{prefix}{}", line.trimmed_start());
            }
            return true;
        }
        if !(last.starts_with("/*") || last.starts_with('*') || last.starts_with("*/")) {
            if !last.contains("*/") {
                return false;
            }
            let mut start = end - 1;
            // A statement or a closing brace whose trailing comment ends the
            // output stays put.
            if last.contains("/*") {
                return false;
            }
            while start > 0 && !self.output[start].trimmed_start().starts_with("/*") {
                let text = self.output[start].trimmed();
                if text.is_empty()
                    || text.ends_with(';')
                    || text.ends_with('{')
                    || text.ends_with('}')
                {
                    return false;
                }
                start -= 1;
            }
            if !self.output[start].trimmed_start().starts_with("/*")
                || line_kind == LineKind::SwitchLabel && self.output_comment_trailed_code(start)
            {
                return false;
            }
            let preserve_relative = if line_kind == LineKind::SwitchLabel && last.starts_with('}') {
                indent = self.layout.indentation.indent();
                false
            } else {
                true
            };
            self.reindent_output_range(start, end, indent, preserve_relative);
            return true;
        }

        let mut start = end - 1;
        while start > 0 {
            let text = self.output[start].trimmed_start();
            let previous = self.output[start - 1].trimmed_start();
            if text.starts_with("/*") {
                if text.ends_with("*/") && previous.starts_with("/*") && previous.ends_with("*/") {
                    start -= 1;
                    continue;
                }
                break;
            }
            if !(previous.starts_with("/*") || previous.starts_with('*')) {
                break;
            }
            start -= 1;
        }
        if !self.output[start].trimmed_start().starts_with("/*")
            || line_kind == LineKind::SwitchLabel && self.output_comment_trailed_code(start)
        {
            return false;
        }
        if line_kind == LineKind::SwitchLabel
            && let Some(switch_line) = self.output[..start].iter().rev().find(|line| {
                let code = self.output.code_trimmed_of(line);
                code.trimmed_start().starts_with("switch") && code.ends_with('{')
            })
        {
            let switch_body_indent = usize::from(
                self.options.indent_switches || self.options.brace_style == BraceStyle::Ratliff,
            ) * self.options.indent_width;
            indent = (leading_visual_width(switch_line, self.options.tab_width)
                + switch_body_indent)
                / self.options.indent_width;
        }
        self.reindent_output_range(start, end, indent, true);
        true
    }

    /// Whether the comment opening on output line `line` followed code on
    /// its source line.
    pub(crate) fn output_comment_trailed_code(&self, line: usize) -> bool {
        self.output.comment_token(line).is_some_and(|comment| {
            self.tree.tokens[..comment]
                .iter()
                .rev()
                .find(|token| !matches!(token, Token::Whitespace(_)))
                .is_some_and(|token| !matches!(token, Token::Newline))
        })
    }

    fn reindent_output_range(
        &mut self,
        start: usize,
        end: usize,
        indent: usize,
        preserve_relative: bool,
    ) {
        let prefix = self.options.indent_prefix(indent);
        let tab_width = self.options.tab_width.max(1);
        let opener_leading = leading_visual_width(&self.output[start], tab_width);
        for (offset, line) in self.output.range_mut(start..end).iter_mut().enumerate() {
            let text = line.trimmed_start();
            if text.is_empty() {
                line.clear();
            } else if offset == 0 || !preserve_relative {
                *line = format!("{prefix}{text}");
            } else {
                let relative = leading_visual_width(line, tab_width).saturating_sub(opener_leading);
                *line = format!("{prefix}{}{text}", " ".repeat(relative));
            }
        }
    }

    pub(crate) fn active_body_comment_indent_spaces(&self) -> Option<usize> {
        let frame = self.layout.frame_stack.active_brace()?;
        let unindented_extern_c = frame.brace_type == BraceType::Extern
            && self.extern_c_guard == ExternCGuard::InsideBlock;
        let body_column = if unindented_extern_c
            || frame.brace_type == BraceType::Namespace
                && (!self.options.indent_namespaces
                    || self.options.brace_style == BraceStyle::Whitesmith)
        {
            frame.sibling_indent_column
        } else if frame.semantic_kind == BraceSemanticKind::Aggregate || frame.case_block {
            frame.body_indent_column.max(
                (self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width,
            )
        } else {
            frame.body_indent_column
        };
        if let Some(line) = self.output.last_line_outside_comment().filter(|line| {
            let code = self.output.code_of(line).trimmed();
            code.starts_with('}') && !code.ends_with('{')
        }) {
            Some(leading_visual_width(line, self.options.tab_width).min(body_column))
        } else {
            Some(body_column)
        }
    }

    fn previous_case_label_body_indent_spaces(&self) -> Option<usize> {
        let line = self.output.last_non_empty_scoped()?;
        let code = self.output.code_trimmed_of(line);
        let candidate = code
            .trimmed_start()
            .strip_prefix('{')
            .map_or(code.trimmed_start(), str::trim_start);
        let colon = find_case_colon(candidate)?;
        candidate[colon + 1..].trimmed().is_empty().then(|| {
            let offset = code.len() - candidate.len();
            visual_width_from(&code[..offset], 0, self.options.tab_width)
                + self.options.indent_width
        })
    }

    pub(crate) fn push_comment(&mut self, kind: CommentKind, comment: &str) {
        let line_comment_starts_reordered_brace_body =
            kind == CommentKind::Line && self.comments.line_comment_starts_reordered_brace_body;
        if line_comment_starts_reordered_brace_body {
            self.comments.line_comment_starts_reordered_brace_body = false;
        }
        let reordered_brace_line_comment_gap = if kind == CommentKind::Line {
            self.comments.reordered_brace_line_comment_gap.take()
        } else {
            None
        };
        let indents = self.comment_indents(kind, comment);
        let CommentIndents {
            definition_header: definition_header_comment_indent,
            block_continuation: block_comment_continuation_indent,
            lambda_parameter: lambda_parameter_comment_indent,
            ..
        } = indents;
        if let Some(spaces) = lambda_parameter_comment_indent {
            self.layout
                .continuation_indent
                .input_line_continuation_indent = Some(ContinuationIndent::Spaces(spaces));
        }
        if comment.contains("*INDENT-OFF*") && !self.layout.line_state.indent_off_follows_code {
            self.finish_line();
            if self.token_input.token_begins_source_line && self.token_input.token_source_column > 0
            {
                self.current
                    .push_str(&" ".repeat(self.token_input.token_source_column));
            }
            self.current.push_str(comment.trimmed_end());
            self.disabled_formatting = Some(DisabledFormattingState::capture(self));
            return;
        }

        if self.should_break_header_before_comment() {
            self.finish_line();
            self.layout.continuation_indent.next_line_indent = Some(self.statement_level() + 1);
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.layout.command_state.header_broken_before_comment = true;
        }

        if comment.contains('\n') {
            if kind == CommentKind::Line {
                self.push_continued_line_comment(comment);
                return;
            }
            self.push_multiline_block_comment(comment, block_comment_continuation_indent);
            return;
        }

        if definition_header_comment_indent.is_some() && self.current.trimmed().is_empty() {
            let prefix = self.previous_output_indent_prefix();
            self.clear_current();
            self.current.push_str(&prefix);
            self.current_is_preindented = true;
        }

        if kind == CommentKind::Block
            && self.current.trimmed_end().ends_with('{')
            && self
                .layout
                .nesting
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| {
                    matches!(
                        brace_type,
                        BraceType::Command | BraceType::Definition | BraceType::NonStatement
                    )
                })
            && !self
                .token_input
                .next_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| whitespace.contains('\n'))
        {
            self.finish_line();
        }

        if kind == CommentKind::Line
            && !line_comment_starts_reordered_brace_body
            && self.current.trimmed().is_empty()
            && !self.token_input.token_begins_source_line
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| !whitespace.contains('\n'))
            && self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with("*/"))
        {
            let whitespace = self
                .token_input
                .previous_input_whitespace
                .clone()
                .unwrap_or_default();
            if let Some(line) = self.output.last_mut() {
                if let Some(gap) = reordered_brace_line_comment_gap.as_deref() {
                    line.push_str(gap);
                }
                line.push_str(&whitespace);
                line.push_str(comment.trimmed_end());
            }
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }

        self.position_comment_start(kind, comment, &indents);
        self.push_positioned_comment(kind, comment, &indents);
    }

    fn comment_indents(&self, kind: CommentKind, comment: &str) -> CommentIndents {
        let function_try_initializer_comment = self.options.break_one_line_statements
            && self
                .layout
                .frame_stack
                .active_constructor_initializer()
                .is_some_and(|frame| frame.function_try)
            && self.current.trimmed_end().ends_with(':');
        let standalone_line_comment = kind == CommentKind::Line
            && self.current.trimmed().is_empty()
            && comment.starts_with("//");
        let open_paren_comment_indent = (self.layout.previous == PreviousToken::OpenParen)
            .then(|| self.comment_after_open_paren_indent_spaces());
        let close_paren_comment_indent = (self.layout.previous == PreviousToken::CloseParen)
            .then(|| {
                self.assignment_continuation_indent_spaces()
                    .or_else(|| self.return_continuation_indent_spaces())
            })
            .flatten();
        let previous_line_ends_operator = self
            .output
            .last_lines_where(LineFilter::NoDirectiveOrLineComment)[0]
            .map(|index| &self.output[index])
            .is_some_and(|line| {
                let head = self.output.code_trimmed_of(line);
                head_ends_binary_operator(head) || head_ends_assignment_operator(head)
            });
        let line_comment_can_carry_continuation = !standalone_line_comment
            || previous_line_ends_operator
            || matches!(
                self.layout.previous,
                PreviousToken::Operator | PreviousToken::Comma
            );
        let line_comment_continuation_indent = (kind == CommentKind::Line
            && !self.in_enum_declaration_brace()
            && line_comment_can_carry_continuation
            && !(standalone_line_comment
                && comment.trimmed_end().ends_with(':')
                && !previous_line_ends_operator)
            && (!has_hash_outside_literals(&self.current)
                || self.current.trimmed_start().starts_with('#'))
            && self.is_continuation_break())
        .then(|| {
            open_paren_comment_indent
                .or(close_paren_comment_indent)
                .map_or_else(
                    || self.next_continuation_indent(),
                    ContinuationIndent::Spaces,
                )
        });
        let line_comment_stream_chain_indent = (kind == CommentKind::Line
            && standalone_line_comment)
            .then(|| self.previous_stream_chain_line_comment_indent_spaces())
            .flatten();
        let ternary_branch_comment_indent_spaces =
            self.ternary_branch_line_comment_indent_spaces(kind, standalone_line_comment);
        let switch_label_block_comment_indent =
            self.switch_label_block_comment_indent_spaces(kind, comment);
        let case_block_comment_indent =
            self.case_block_comment_indent_spaces(kind, switch_label_block_comment_indent);
        let definition_header_comment_indent = self.definition_header_comment_indent_spaces();
        let case_label_comment_indent = (kind == CommentKind::Block
            && self.current.trimmed().is_empty())
        .then(|| self.previous_case_label_body_indent_spaces())
        .flatten();
        let column1_case_label_comment_indent = (kind == CommentKind::Line
            && self.current.trimmed().is_empty())
        .then(|| self.previous_case_label_body_indent_spaces())
        .flatten();
        let user_label_comment_indent = self.user_label_comment_indent_spaces(kind);
        let control_header_comment_indent = self.control_header_comment_indent_spaces(kind);
        let block_comment_continuation_indent = case_block_comment_indent
            .or(case_label_comment_indent)
            .or(control_header_comment_indent)
            .or(user_label_comment_indent)
            .or_else(|| {
                self.standalone_block_comment_indent_spaces(kind, definition_header_comment_indent)
            });
        let lambda_parameter_comment_indent = self.lambda_parameter_comment_indent_spaces(kind);
        CommentIndents {
            standalone_line_comment,
            function_try_initializer: function_try_initializer_comment,
            open_paren: open_paren_comment_indent,
            line_continuation: line_comment_continuation_indent,
            line_stream_chain: line_comment_stream_chain_indent,
            ternary_branch: ternary_branch_comment_indent_spaces,
            definition_header: definition_header_comment_indent,
            case_label: case_label_comment_indent,
            column1_case_label: column1_case_label_comment_indent,
            user_label: user_label_comment_indent,
            control_header: control_header_comment_indent,
            block_continuation: block_comment_continuation_indent,
            lambda_parameter: lambda_parameter_comment_indent,
        }
    }

    /// Moves the current line to where the comment starts: breaks the line or
    /// sets the indentation the comment line will use.
    fn position_comment_start(
        &mut self,
        kind: CommentKind,
        comment: &str,
        indents: &CommentIndents,
    ) {
        let CommentIndents {
            standalone_line_comment,
            line_continuation: line_comment_continuation_indent,
            line_stream_chain: line_comment_stream_chain_indent,
            definition_header: definition_header_comment_indent,
            case_label: case_label_comment_indent,
            column1_case_label: column1_case_label_comment_indent,
            user_label: user_label_comment_indent,
            control_header: control_header_comment_indent,
            block_continuation: block_comment_continuation_indent,
            lambda_parameter: lambda_parameter_comment_indent,
            ..
        } = *indents;
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && self.layout.frame_stack.active_brace().is_none()
            && self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| self.output.code_trimmed_of(line).ends_with(';'))
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.current_is_preindented = true;
            self.layout.nesting.clear_continuation_indents();
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous_code.trimmed_start() == "else"
                || previous_code.trimmed_start().ends_with("} else")
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                let prefix = self
                    .options
                    .continuation_indent_prefix(spaces / self.options.indent_width.max(1), spaces);
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.options.brace_style == BraceStyle::None
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous_code.ends_with('{') && previous_code.trimmed_start().starts_with("} else") {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.line_state.trailing_comment_columns.clear();
                self.token_input.previous_input_whitespace = Some(String::new().into());
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.options.brace_style == BraceStyle::None
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            let previous_trimmed = previous_code.trimmed_start();
            if previous_code.ends_with('{')
                && (comment_starts_header_word(previous_trimmed, "if")
                    || comment_starts_header_word(previous_trimmed, "while")
                    || comment_starts_header_word(previous_trimmed, "for")
                    || previous_trimmed.starts_with("else if")
                    || previous_trimmed.starts_with("} else"))
                && self.output.scoped().iter().rev().take(64).any(|line| {
                    let trimmed = self.output.code_trimmed_of(line).trimmed_start();
                    trimmed == "else" || trimmed.ends_with("} else")
                })
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .take_while(|line| {
                        let code = self.output.code_trimmed_of(line);
                        !(leading_visual_width(line, self.options.tab_width) == 0
                            && code.ends_with('{')
                            && !code.trimmed_start().starts_with('#'))
                    })
                    .take(64)
                    .any(|line| line.trimmed_start().starts_with('#'))
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && self.preprocessor.split_else.extra_indent
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous_code.trimmed_start() == "}" {
                let spaces = leading_visual_width(previous, self.options.tab_width);
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if preprocessor_directive(previous_code.trimmed_start())
                .is_some_and(|directive| directive == "else" || directive.starts_with("elif"))
            {
                let spaces = (self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width;
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && let Some(spaces) = self.preprocessor_split_braceless_comment_indent_spaces()
        {
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if self.options.brace_style == BraceStyle::None
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
            && preprocessor_directive(previous.trimmed_start()) == Some("endif")
            && let Some(header) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| {
                    let trimmed = line.trimmed_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
        {
            let trimmed = self.output.code_trimmed_of(header).trimmed_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let spaces = leading_visual_width(header, self.options.tab_width)
                    + self.options.indent_width;
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && (self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal))
                == 0
            && self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| preprocessor_directive(line.trimmed_start()) == Some("endif"))
        {
            self.current_is_preindented = true;
            self.layout.continuation_indent.next_line_indent_spaces = Some(0);
        }
        if kind == CommentKind::Line
            && self.current.trimmed().is_empty()
            && let Some(spaces) = self.enum_value_comment_continuation_indent_spaces()
        {
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            if previous_code.trimmed_start().starts_with("switch") && previous_code.ends_with('{') {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && self.last_line_closes_top_level_declaration()
        {
            self.current_is_preindented = true;
            self.layout.continuation_indent.next_line_indent_spaces = Some(0);
        }
        if kind == CommentKind::Line
            && self.current.trimmed().is_empty()
            && let Some(previous) = self.output.last_non_empty_scoped()
            && previous.trimmed_start().starts_with('?')
        {
            let spaces = leading_visual_width(previous, self.options.tab_width);
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        let mut brace_element_comment_indent = None;
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && !comment.contains('\n')
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let previous_code = self.output.code_trimmed_of(previous);
            // A directive continues no statement into the comment.
            let after_directive = self
                .output
                .last_non_empty_index()
                .is_some_and(|index| self.output.is_directive_line(index));
            if previous_code.ends_with(',') && !after_directive {
                brace_element_comment_indent = unmatched_open_brace_content_offset(previous_code)
                    .map(|offset| {
                        visual_width_from(&previous[..offset], 0, self.options.tab_width)
                    });
                let spaces = brace_element_comment_indent
                    .unwrap_or_else(|| leading_visual_width(previous, self.options.tab_width));
                let structural_level = self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal);
                let prefix = self
                    .options
                    .continuation_indent_prefix(structural_level, spaces);
                self.clear_current();
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && !comment.contains('\n')
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let code = self.output.code_trimmed_of(previous);
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            let after_directive = self
                .output
                .last_non_empty_index()
                .is_some_and(|index| self.output.is_directive_line(index));
            if !after_directive
                && let Some(spaces) =
                    self.block_comment_call_opener_indent_spaces(code, previous_indent)
            {
                self.clear_current();
                let prefix = self
                    .options
                    .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && !comment.contains('\n')
            && (self.preprocessor.split_else.extra_indent || self.preprocessor_split_else_active())
            && let Some(previous) = self.output.last_non_empty_scoped()
        {
            let code = self.output.code_trimmed_of(previous);
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if code.ends_with(';')
                && previous_indent > self.current_line_indent_spaces() + self.options.indent_width
            {
                self.clear_current();
                self.current.push_str(&" ".repeat(previous_indent));
                self.current_is_preindented = true;
                self.layout.continuation_indent.next_line_indent_spaces = Some(previous_indent);
            }
        }
        let follows_objc_interface = self
            .output
            .last_non_empty_scoped()
            .is_some_and(|line| line.trimmed_start().starts_with("@interface"));
        // A control header's condition continues where its lines do, past
        // the minimum conditional indent.
        let in_header_condition = self
            .layout
            .frame_stack
            .active_delimiter()
            .is_some_and(|frame| frame.role == ParenRole::Header);
        let header_condition_row_indent = in_header_condition
            .then(|| {
                self.output
                    .last_non_empty_scoped()
                    .filter(|line| {
                        let trimmed = line.trimmed_start();
                        !is_comment_only_line(trimmed)
                            && !["if", "while", "for"]
                                .iter()
                                .any(|header| comment_starts_header_word(trimmed, header))
                            && !trimmed.starts_with("} while")
                            && !trimmed.starts_with("else if")
                            && !trimmed.starts_with("} else if")
                    })
                    .map(|line| leading_visual_width(line, self.options.tab_width))
            })
            .flatten();
        if kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && !comment.contains('\n')
            && !self.last_line_closes_top_level_declaration()
            && let Some(spaces) = brace_element_comment_indent
                .or(case_label_comment_indent)
                .or(control_header_comment_indent)
                .or(user_label_comment_indent)
                .or(lambda_parameter_comment_indent)
                .or(header_condition_row_indent)
                .or_else(|| {
                    self.options
                        .indent_after_parens
                        .then_some(block_comment_continuation_indent)
                        .flatten()
                })
                .or_else(|| {
                    self.output.last_non_empty_scoped().and_then(|line| {
                        let code = self.output.code_trimmed_of(line);
                        let previous_indent = leading_visual_width(line, self.options.tab_width);
                        self.block_comment_call_opener_indent_spaces(code, previous_indent)
                            .or_else(|| {
                                if in_header_condition {
                                    return None;
                                }
                                // A brace opened after the paren holds the comment.
                                self.open_paren_column_of(code)
                                    .filter(|&column| {
                                        !has_unmatched_open_brace(&code[column + 1..])
                                    })
                                    .map(|column| {
                                        let after_open = &code[column + 1..];
                                        let content_offset = after_open
                                            .char_indices()
                                            .find(|(_, ch)| !ch.is_whitespace())
                                            .map_or(0, |(offset, _)| offset);
                                        visual_width_from(
                                            &code[..column + 1 + content_offset],
                                            0,
                                            self.options.tab_width,
                                        )
                                    })
                            })
                    })
                })
                .or_else(|| self.current_inline_array_column())
                .or(block_comment_continuation_indent)
                .or_else(|| {
                    self.output.last_non_empty_scoped().and_then(|line| {
                        let code = self.output.code_trimmed_of(line);
                        code.ends_with(',')
                            .then(|| leading_visual_width(line, self.options.tab_width))
                    })
                })
                .or_else(|| {
                    self.output.last_non_empty_scoped().and_then(|line| {
                        let code = self.output.code_trimmed_of(line);
                        let trimmed = code.trimmed_start();
                        (trimmed == "else" || trimmed.ends_with("} else")).then(|| {
                            leading_visual_width(line, self.options.tab_width)
                                + self.options.indent_width
                        })
                    })
                })
                .or_else(|| self.active_body_comment_indent_spaces())
                .or_else(|| follows_objc_interface.then_some(self.options.indent_width))
        {
            let output_len = self.output.len();
            if let Some((_, delimiter)) = self.layout.frame_stack.active_delimiter_mut()
                && delimiter.opener_output_line < output_len
            {
                delimiter.continuation_indent_column = Some(spaces);
            }
            if self
                .output
                .last_non_empty_scoped()
                .and_then(|line| self.open_paren_column_of(line.trimmed_end()))
                .is_some()
            {
                self.layout
                    .continuation_indent
                    .clear_continuation_after_line = Some(spaces);
            }
            let current_indent = leading_visual_width(&self.current, self.options.tab_width);
            if current_indent != spaces {
                let structural_level = if follows_objc_interface {
                    1
                } else {
                    self.layout
                        .indentation
                        .line_indent(LineKind::Normal, self.options)
                        + self.case_body_indent_extra(LineKind::Normal)
                };
                let prefix = self
                    .options
                    .continuation_indent_prefix(structural_level, spaces);
                self.clear_current();
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
            }
            self.layout.continuation_indent.next_line_indent_spaces =
                (!follows_objc_interface).then_some(spaces);
            if kind == CommentKind::Block
                && !follows_objc_interface
                && (!self.token_input.has_next_meaningful_token
                    || self
                        .token_input
                        .next_input_whitespace
                        .as_deref()
                        .is_some_and(|whitespace| whitespace.contains('\n')))
            {
                self.layout
                    .continuation_indent
                    .next_input_line_continuation_indent = Some(ContinuationIndent::Spaces(spaces));
            }
        }
        if kind == CommentKind::Line
            && standalone_line_comment
            && (self.layout.line_state.column1_line_comment
                || self
                    .layout
                    .indentation
                    .current_preprocessor_indent()
                    .is_some())
            && self.options.indent_col1_comments
            && line_comment_continuation_indent.is_none()
            && line_comment_stream_chain_indent.is_none()
            && self
                .enum_value_comment_continuation_indent_spaces()
                .is_none()
            && !self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trimmed_start().starts_with('?'))
        {
            let preprocessor_indent = self
                .split_else_preprocessor_branch_body_indent_spaces()
                .or_else(|| {
                    self.layout
                        .indentation
                        .current_preprocessor_indent()
                        .map(|indent| {
                            indent
                                .spaces
                                .unwrap_or(indent.level * self.options.indent_width)
                                + if self.preprocessor_split_else_active() {
                                    self.preprocessor.split_else.extra_levels.max(1)
                                        * self.options.indent_width
                                } else {
                                    0
                                }
                        })
                });
            // A group opened outside the block holding the comment, as an
            // include guard, sets no column inside the block.
            let preprocessor_indent = preprocessor_indent.filter(|&spaces| {
                self.active_body_comment_indent_spaces()
                    .is_none_or(|body| body <= spaces)
            });
            let spaces = preprocessor_indent
                .or(control_header_comment_indent)
                .or(definition_header_comment_indent)
                .or(case_label_comment_indent)
                .or(column1_case_label_comment_indent)
                .or_else(|| self.active_body_comment_indent_spaces())
                .unwrap_or(0);
            let exact_semantic_comment = kind == CommentKind::Line
                && (column1_case_label_comment_indent.is_some()
                    || preprocessor_indent.is_none()
                        && control_header_comment_indent.is_none()
                        && definition_header_comment_indent.is_some()
                    || (comment.starts_with("///") || comment.starts_with("//!"))
                        && self.layout.frame_stack.active_brace().is_some_and(|frame| {
                            frame.semantic_kind == BraceSemanticKind::Aggregate
                        }));
            self.clear_current();
            if exact_semantic_comment {
                let prefix = self
                    .options
                    .continuation_indent_prefix(spaces / self.options.indent_width.max(1), spaces);
                self.current.push_str(&prefix);
            }
            self.current_is_preindented = exact_semantic_comment;
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if self.current.trimmed().is_empty()
            && self.token_input.token_begins_source_line
            && self.token_input.token_source_column == 0
            && !self.options.indent_col1_comments
            && self
                .layout
                .indentation
                .current_preprocessor_indent()
                .is_some()
            && self.preprocessor_region(false) == PreprocessorRegion::TopLevel
            // An indented preprocessor block indents its comments.
            && !(self.options.indent_preproc_block
                && self.preprocessor.indented_block_stack.last() == Some(&true))
        {
            self.clear_current();
            self.current_is_preindented = true;
        }
        if self.layout.previous == PreviousToken::OpenParen {
            let comment_ends_line = kind == CommentKind::Line
                || !self.token_input.has_next_meaningful_token
                || self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.contains('\n'));
            let outside_pad =
                self.options.pad_parens_outside || self.options.pad_first_paren_outside;
            if comment_ends_line && self.options.unpad_parens {
                // A comment ending its line keeps its column as a trailing
                // comment does.
                self.pad_before_trailing_comment(kind, comment);
            } else if self.options.pad_parens_inside {
                self.pad_inside_paren_space();
            } else if comment_ends_line && outside_pad {
                self.emit_source_space_or_ensure();
            } else if self.options.unpad_parens {
                self.trim_current_end_horizontal_space();
            } else {
                self.emit_source_space();
            }
        } else if !self.current.trimmed().is_empty() {
            self.pad_before_trailing_comment(kind, comment);
        }
    }

    fn push_positioned_comment(
        &mut self,
        kind: CommentKind,
        comment: &str,
        indents: &CommentIndents,
    ) {
        let CommentIndents {
            function_try_initializer: function_try_initializer_comment,
            open_paren: open_paren_comment_indent,
            line_continuation: line_comment_continuation_indent,
            line_stream_chain: line_comment_stream_chain_indent,
            ternary_branch: ternary_branch_comment_indent_spaces,
            lambda_parameter: lambda_parameter_comment_indent,
            ..
        } = *indents;
        let comment_text = if kind == CommentKind::Line && comment.trimmed_end().ends_with('\\') {
            comment
        } else {
            comment.trimmed_end()
        };
        if let Some(spaces) = line_comment_stream_chain_indent {
            self.clear_current();
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
        }
        if let Some(spaces) = ternary_branch_comment_indent_spaces {
            let structural_level = self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal);
            let prefix = self
                .options
                .continuation_indent_prefix(structural_level, spaces);
            self.clear_current();
            self.current.push_str(&prefix);
            self.current_is_preindented = true;
        }
        let output_column = leading_visual_width(&self.current, self.options.tab_width.max(1));
        self.record_comment_frame(kind, output_column, false);
        self.current.push_str(comment_text);
        if kind == CommentKind::Block
            && self.token_input.has_next_meaningful_token
            && lambda_parameter_comment_indent.is_some()
        {
            self.current_is_preindented = false;
        }
        if kind == CommentKind::Block {
            if open_paren_comment_indent.is_some() && !self.token_input.has_next_meaningful_token
                || self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.contains('\n'))
            {
                let after_post_closing_declaration = self.last_line_closes_top_level_declaration();
                self.finish_line();
                if after_post_closing_declaration {
                    self.layout.continuation_indent.next_line_indent = None;
                    self.layout.continuation_indent.next_line_indent_spaces =
                        Some(self.options.indent_width);
                }
                let column = self.current_inline_array_column().or_else(|| {
                    self.output
                        .last()
                        .map(|line| leading_visual_width(line, self.options.tab_width))
                        .filter(|spaces| *spaces > 0)
                });
                if let Some(column) = column {
                    self.layout.continuation_indent.next_line_indent = None;
                    self.layout.continuation_indent.next_line_indent_spaces = Some(
                        column
                            + self.layout.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
                if let Some(spaces) = open_paren_comment_indent {
                    self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
                    self.layout
                        .nesting
                        .push_continuation_indent_spaces_raw(spaces);
                }
            } else {
                self.emit_trailing_source_space();
            }
        }
        if function_try_initializer_comment
            && kind == CommentKind::Block
            && !self.current_is_blank()
        {
            self.finish_line();
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        if kind == CommentKind::Line {
            self.finish_line();
            if let Some(spaces) =
                line_comment_stream_chain_indent.or(ternary_branch_comment_indent_spaces)
            {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(spaces);
            } else if let Some(indent) = line_comment_continuation_indent {
                self.set_next_continuation_indent(indent);
            }
        }
        if function_try_initializer_comment {
            self.layout.continuation_indent.next_line_indent = Some(self.statement_level() + 1);
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        }
    }

    fn ternary_branch_line_comment_indent_spaces(
        &self,
        kind: CommentKind,
        standalone_line_comment: bool,
    ) -> Option<usize> {
        (kind == CommentKind::Line && standalone_line_comment)
            .then(|| {
                let previous = self.output.last_line_outside_comment()?;
                let code = self.output.code_trimmed_of(previous);
                (code.trimmed_start().starts_with('?')
                    && self.open_paren_column_of(code).is_none()
                    && self.layout.frame_stack.active_ternary().is_some())
                .then(|| leading_visual_width(previous, self.options.tab_width))
            })
            .flatten()
    }

    fn switch_label_block_comment_indent_spaces(
        &self,
        kind: CommentKind,
        comment: &str,
    ) -> Option<usize> {
        (kind == CommentKind::Block
            && self.current.trimmed().is_empty()
            && comment.lines().any(|line| {
                let trimmed = line
                    .trimmed_start()
                    .strip_prefix("/*")
                    .unwrap_or(line.trimmed_start())
                    .trimmed_start();
                trimmed.starts_with("case ") || trimmed.starts_with("default:")
            })
            && self
                .layout
                .nesting
                .brace_header_stack
                .iter()
                .any(|header| header.as_deref() == Some("switch")))
        .then(|| self.layout.indentation.indent().saturating_sub(1) * self.options.indent_width)
    }

    fn case_block_comment_indent_spaces(
        &self,
        kind: CommentKind,
        switch_label_block_comment_indent: Option<usize>,
    ) -> Option<usize> {
        switch_label_block_comment_indent.or_else(|| {
            (kind == CommentKind::Block
                && self.current.trimmed().is_empty()
                && self
                    .output
                    .last_non_empty_scoped()
                    .is_some_and(|line| line.trimmed() == "}")
                && self
                    .layout
                    .nesting
                    .brace_header_stack
                    .iter()
                    .any(|header| header.as_deref() == Some("switch")))
            .then(|| {
                // After a control block in the case body the comment stands
                // at the block's header, wherever the style put its `}`.
                if self
                    .layout
                    .frame_stack
                    .last_closed_brace()
                    .is_some_and(|frame| {
                        frame.semantic_kind == BraceSemanticKind::Command
                            && frame.header.as_deref().is_none_or(|header| {
                                !matches!(header, "case" | "default" | "switch")
                            })
                    })
                    && let Some(spaces) = self.closed_block_header_lead()
                {
                    return spaces + self.case_unindent_spaces();
                }
                let introduces_label = self.comment_precedes_switch_label();
                let previous_indent = self
                    .output
                    .last_non_empty_scoped()
                    .map(|line| leading_visual_width(line, self.options.tab_width))
                    .unwrap_or(0);
                if introduces_label {
                    let base = self.layout.indentation.indent().saturating_sub(1)
                        * self.options.indent_width;
                    base.max(previous_indent)
                } else {
                    (self.layout.indentation.indent() * self.options.indent_width)
                        .max(previous_indent)
                }
            })
        })
    }

    /// Whether the code after the comment being placed, past other
    /// comments, is a `case` or `default` label.
    fn comment_precedes_switch_label(&self) -> bool {
        let tokens = &self.tree.tokens;
        self.current.active_comment().is_some_and(|comment| {
            tokens[comment + 1..]
                .iter()
                .find(|token| {
                    !matches!(
                        token,
                        Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
                    )
                })
                .is_some_and(
                    |token| matches!(token, Token::Word(word) if word == "case" || word == "default"),
                )
        })
    }

    /// Lead of the header line of the block the last code line closes.
    fn closed_block_header_lead(&self) -> Option<usize> {
        let lines = self.output.scoped();
        let close = lines.iter().rposition(|line| !line.trimmed().is_empty())?;
        let mut depth = 0usize;
        for index in (0..=close).rev() {
            let line = &lines[index];
            let code = self.output.code_of(line).trimmed();
            let (closes, opens) = line_brace_imbalance(code);
            depth += closes;
            if index < close && opens >= depth {
                let mut header = if code == "{" {
                    lines[..index]
                        .iter()
                        .rposition(|line| !line.trimmed().is_empty())?
                } else {
                    index
                };
                // A header split over lines starts where its parens open.
                let paren_balance = |line: &str| {
                    let code = &self.output.code_of(line);
                    code.matches('(').count() as isize - code.matches(')').count() as isize
                };
                let mut balance = paren_balance(&lines[header]);
                while balance < 0 && header > 0 {
                    header -= 1;
                    balance += paren_balance(&lines[header]);
                }
                return Some(leading_visual_width(&lines[header], self.options.tab_width));
            }
            depth = depth.saturating_sub(opens);
        }
        None
    }

    fn definition_header_comment_indent_spaces(&self) -> Option<usize> {
        (!self.preprocessor.last_output_was_preprocessor
            && !self.layout.line_adjuster.is_in_macro_block()
            && self
                .layout
                .frame_stack
                .active_constructor_initializer()
                .is_none()
            && self.current.trimmed().is_empty())
        .then(|| {
            let index = (0..self.output.len())
                .rev()
                .find(|&index| !self.output.trimmed(index).is_empty())?;
            // Comment text that reads like a call is no header.
            if !self
                .output
                .line_tokens(index)
                .is_some_and(|span| is_code_token(&self.tree.tokens[span.first]))
            {
                return None;
            }
            self.output.last_non_empty_scoped().and_then(|line| {
                let code = self.output.code_trimmed_of(line);
                let trimmed = code.trimmed_start();
                let header = trimmed
                    .split(|ch: char| ch == '(' || ch.is_whitespace())
                    .next()
                    .unwrap_or_default();
                (trimmed.contains('(')
                    && trimmed.contains(')')
                    && !trimmed.starts_with_any(b"}#")
                    && !trimmed.ends_with_any(b";{")
                    && !trimmed.contains('=')
                    && !language::is_header(header))
                .then(|| leading_visual_width(line, self.options.tab_width))
            })
        })
        .flatten()
    }

    fn user_label_comment_indent_spaces(&self, kind: CommentKind) -> Option<usize> {
        (kind == CommentKind::Block && self.current.trimmed().is_empty())
            .then(|| {
                let line = self.output.last_non_empty_scoped()?;
                let code = self.output.code_of(line).trimmed();
                let label = code.strip_suffix(':')?.trimmed_end();
                (labels::line_kind(code, &self.options.access_labels) == LineKind::Label
                    && !labels::is_access_label_start(label, &self.options.access_labels))
                .then(|| {
                    (self
                        .layout
                        .indentation
                        .line_indent(LineKind::Normal, self.options)
                        + self.case_body_indent_extra(LineKind::Normal))
                        * self.options.indent_width
                })
            })
            .flatten()
    }

    fn control_header_comment_indent_spaces(&self, kind: CommentKind) -> Option<usize> {
        (self.current.trimmed().is_empty()
            || (kind == CommentKind::Block && self.should_break_header_before_comment()))
        .then(|| {
            let header = self.layout.frame_stack.active_header()?;
            let current = self.layout.command_state.current_header.as_deref()?;
            if matches!(current, "case" | "default") {
                return None;
            }
            let pending_header_line = if self.current.trimmed().is_empty() {
                self.output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| {
                        let trimmed = line.trimmed_start();
                        !trimmed.is_empty() && !is_comment_only_line(trimmed)
                    })
                    .is_some_and(|line| {
                        let code = self.output.code_of(line).trimmed_start();
                        let candidate = code
                            .strip_prefix('}')
                            .map_or(code, |tail| tail.trimmed_start());
                        candidate == current
                            || comment_starts_header_word(candidate, current)
                            || (self.layout.command_state.previous_command_char == Some(')')
                                && candidate.trimmed_end().ends_with(')'))
                    })
            } else {
                self.should_break_header_before_comment()
            };
            // A `)` closing a paren inside the condition leaves it open.
            (header.header == current
                && pending_header_line
                && (language::is_non_paren_header(current)
                    || self.layout.command_state.previous_command_char == Some(')')
                        && self.layout.nesting.paren_depth == 0))
                .then_some(header.body_indent_spaces)
        })
        .flatten()
    }

    /// Whether the comment being pushed comes right after a `switch` brace
    /// and before a statement that is no case label: it stands at the body.
    fn comment_precedes_switch_statement(&self) -> bool {
        let tokens = &self.tree.tokens;
        !self.options.indent_cases
            && self
                .current
                .active_comment()
                .and_then(|comment| next_code_token(tokens, comment + 1))
                .is_some_and(|next| match &tokens[next] {
                    Token::Word(word) => !matches!(word.as_str(), "case" | "default"),
                    Token::Symbol('}' | '#') | Token::Preprocessor(_) => false,
                    _ => true,
                })
    }

    fn standalone_block_comment_indent_spaces(
        &self,
        kind: CommentKind,
        definition_header_comment_indent: Option<usize>,
    ) -> Option<usize> {
        if kind != CommentKind::Block || !self.current.trimmed().is_empty() {
            return None;
        }
        let previous_line = self.output.last_non_empty_scoped();
        if let Some(line) = previous_line {
            let code = self.output.code_trimmed_of(line);
            let trimmed = code.trimmed_start();
            let raw_trimmed = line.trimmed();
            if raw_trimmed.starts_with("/*") && raw_trimmed.ends_with("*/") {
                return Some(leading_visual_width(line, self.options.tab_width));
            }
            if trimmed.starts_with("switch") && code.ends_with('{') {
                return Some(
                    leading_visual_width(line, self.options.tab_width)
                        + usize::from(self.comment_precedes_switch_statement())
                            * (1 + usize::from(self.options.brace_style == BraceStyle::Ratliff))
                            * self.options.indent_width,
                );
            }
            if trimmed.starts_with("} else") {
                return Some(
                    leading_visual_width(line, self.options.tab_width)
                        + self.options.indent_width
                        + self.case_unindent_spaces(),
                );
            }
            if preprocessor_directive(trimmed) == Some("endif")
                && let Some(header) = self.output.scoped().iter().rev().skip(1).find(|line| {
                    let trimmed = line.trimmed_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
            {
                let code = self.output.code_trimmed_of(header);
                let trimmed = code.trimmed_start();
                if trimmed == "else" || trimmed.ends_with("} else") {
                    return Some(
                        leading_visual_width(header, self.options.tab_width)
                            + self.options.indent_width
                            + self.case_unindent_spaces(),
                    );
                }
            }
            if trimmed.starts_with("} ")
                && !trimmed.ends_with('{')
                && !self.last_line_closes_nested_block()
            {
                return Some(0);
            }
            // A macro call that closes all its parens at file scope ends its
            // statement without `;`.
            if self.layout.nesting.brace_type_stack.is_empty()
                && self.layout.nesting.paren_depth == 0
                && code.ends_with(')')
                && !trimmed.starts_with('#')
                && self
                    .output
                    .last_non_empty_index()
                    .is_none_or(|index| self.output.directive_of_continuation(index).is_none())
            {
                let lines = self.output.scoped();
                let paren_balance = |line: &str| {
                    let code = &self.output.code_of(line);
                    code.matches('(').count() as isize - code.matches(')').count() as isize
                };
                let mut balance = 0isize;
                for line in lines.iter().rev() {
                    if line.trimmed().is_empty() {
                        break;
                    }
                    balance += paren_balance(line);
                    if balance >= 0 {
                        break;
                    }
                }
                if balance == 0 {
                    return Some(0);
                }
            }
        }
        let previous_statement_indent = previous_line.and_then(|line| {
            let code = self.output.code_trimmed_of(line);
            let after_braceless_header = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip_while(|candidate| candidate.as_str() != line.as_str())
                .skip(1)
                .find(|candidate| !candidate.trimmed().is_empty())
                .is_some_and(|candidate| {
                    let code = self.output.code_trimmed_of(candidate);
                    let trimmed = code.trimmed_start();
                    let header = trimmed
                        .split(|ch: char| ch != '_' && !ch.is_ascii_alphanumeric())
                        .next()
                        .unwrap_or_default();
                    !trimmed.ends_with_any(b"{;")
                        && (matches!(header, "if" | "for" | "while")
                            || trimmed == "else"
                            || trimmed.starts_with("else if"))
                });
            // The output line lost the case unindents the comment is yet to.
            let previous_indent =
                leading_visual_width(line, self.options.tab_width) + self.case_unindent_spaces();
            let body_indent = (self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal))
                * self.options.indent_width;
            (code.ends_with(';')
                && self.open_paren_column_of(code).is_none()
                && !after_braceless_header
                && previous_indent <= body_indent)
                .then_some(previous_indent)
        });
        let previous_operator_indent = previous_line.and_then(|line| {
            let head = line.trimmed_end();
            (head_ends_binary_operator(head) || head_ends_assignment_operator(head))
                .then(|| self.layout.nesting.current_continuation_indent_spaces())
                .flatten()
        });
        let previous_opening_body_indent = previous_line.and_then(|line| {
            let code = self.output.code_trimmed_of(line);
            code.ends_with('{').then(|| {
                let body_indent = (self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal)
                    + self.else_if_break_extra())
                    * self.options.indent_width;
                let trimmed = code.trimmed_start();
                if comment_starts_header_word(trimmed, "if")
                    || comment_starts_header_word(trimmed, "for")
                    || comment_starts_header_word(trimmed, "while")
                    || trimmed.starts_with("else if")
                    || trimmed.starts_with("} else")
                    || trimmed.starts_with("case ")
                    || trimmed.starts_with("default:")
                {
                    body_indent.max(
                        leading_visual_width(line, self.options.tab_width)
                            + self.options.indent_width,
                    )
                } else {
                    body_indent
                }
            })
        });
        let previous_paren_indent = previous_line.and_then(|line| {
            self.open_paren_column_of(line.trimmed_end()).map(|column| {
                self.layout
                    .nesting
                    .current_continuation_indent_spaces()
                    .unwrap_or(column + 1)
            })
        });
        previous_statement_indent
            .or(previous_operator_indent)
            .or(previous_opening_body_indent)
            .or(previous_paren_indent)
            .or(definition_header_comment_indent)
            .or_else(|| self.layout.nesting.current_continuation_indent_spaces())
            .or_else(|| self.active_body_comment_indent_spaces())
            .or_else(|| {
                (self.layout.indentation.statement_depth() > 0)
                    .then(|| self.current_line_indent_spaces())
            })
            .or_else(|| {
                previous_line.and_then(|line| {
                    let code = self.output.code_trimmed_of(line);
                    code.ends_with('{').then(|| {
                        let leading = leading_visual_width(line, self.options.tab_width);
                        if !self.options.indent_namespaces
                            && matches!(
                                self.layout.nesting.brace_type_stack.last(),
                                Some(BraceType::Namespace | BraceType::Extern)
                            )
                        {
                            leading
                        } else {
                            leading + self.options.indent_width
                        }
                    })
                })
            })
            .or_else(|| {
                (self.token_input.token_begins_source_line
                    && self.token_input.token_source_column > 0
                    && self.layout.indentation.indent() > 0)
                    .then(|| self.layout.indentation.indent() * self.options.indent_width)
            })
    }

    /// The levels that else-if chains broken before their `if` add.
    pub(crate) fn else_if_break_extra(&self) -> usize {
        if self.options.no_indent_if_after_else {
            0
        } else {
            self.layout.else_if_break_depths.len()
        }
    }

    fn lambda_parameter_comment_indent_spaces(&self, kind: CommentKind) -> Option<usize> {
        (kind == CommentKind::Block && self.current.trimmed().is_empty())
            .then(|| {
                let frame = self
                    .layout
                    .frame_stack
                    .active_delimiter()
                    .filter(|frame| frame.lambda_parameter_list)?;
                Some(
                    if matches!(
                        self.options.brace_style,
                        BraceStyle::Attach | BraceStyle::OneTrueBrace | BraceStyle::Ratliff
                    ) {
                        frame.line_indent_spaces
                    } else {
                        frame
                            .continuation_indent_column
                            .unwrap_or(frame.opener_output_column + 1)
                    },
                )
            })
            .flatten()
    }

    fn comment_after_open_paren_indent_spaces(&self) -> usize {
        let base = self
            .layout
            .frame_stack
            .enclosing_delimiter()
            .filter(|frame| frame.opener_output_line == self.output.len())
            .map(|frame| {
                frame
                    .call
                    .as_ref()
                    .and_then(|call| call.first_argument_column)
                    .unwrap_or(frame.opener_output_column + 1)
            })
            .or_else(|| self.assignment_continuation_indent_spaces())
            .or_else(|| self.return_continuation_indent_spaces())
            .or_else(|| {
                let catch_header = self.layout.command_state.current_header.as_deref()
                    == Some("catch")
                    || self
                        .current
                        .trimmed_start()
                        .strip_prefix("catch")
                        .is_some_and(|rest| rest.chars().next().is_some_and(char::is_whitespace));
                catch_header.then(|| self.current_line_indent_spaces() + self.options.indent_width)
            })
            .unwrap_or_else(|| self.current_line_indent_spaces());
        base + self.options.indent_width
    }

    fn previous_stream_chain_line_comment_indent_spaces(&self) -> Option<usize> {
        let previous_line = self.output.len().checked_sub(1)?;
        let previous = self.output.get(previous_line)?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !starts_with_chain_operator(previous_code.trimmed_start())
            || self.layout.frame_stack.active_delimiter().is_none()
            || self
                .layout
                .frame_stack
                .active_stream_on_output_line(previous_line)
                .is_none()
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    fn block_comment_call_opener_indent_spaces(
        &self,
        code: &str,
        previous_indent: usize,
    ) -> Option<usize> {
        if !code.ends_with('(') {
            return None;
        }
        let assignment_value_indent =
            find_assignment_operator(code).map(|(assignment, operator)| {
                let after_operator = assignment + operator.len();
                let value_start = code[after_operator..]
                    .char_indices()
                    .find(|(_, ch)| !ch.is_whitespace())
                    .map_or(code.len(), |(offset, _)| after_operator + offset);
                visual_width_from(&code[..value_start], 0, self.options.tab_width)
                    + self.options.indent_width
            });
        if assignment_value_indent.is_some() && self.previous_line_assigns_same_left_side(code) {
            return assignment_value_indent;
        }
        if self.output.scoped().iter().rev().take(16).any(|line| {
            preprocessor_directive(line.trimmed_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        }) {
            return Some(previous_indent + self.options.indent_width);
        }
        if let Some(spaces) = self
            .layout
            .nesting
            .current_continuation_indent_spaces()
            .filter(|spaces| *spaces > previous_indent)
        {
            return Some(spaces);
        }
        if let Some(spaces) = assignment_value_indent {
            return Some(spaces);
        }
        code.rfind('(')
    }

    fn previous_line_assigns_same_left_side(&self, code: &str) -> bool {
        let Some((assignment, _)) = find_assignment_operator(code) else {
            return false;
        };
        let left = code[..assignment].trimmed();
        !left.is_empty()
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
                .and_then(|line| {
                    let previous = self.output.code_trimmed_of(line);
                    find_assignment_operator(previous)
                        .map(|(assignment, _)| previous[..assignment].trimmed())
                })
                .is_some_and(|previous_left| previous_left == left)
    }

    fn preprocessor_split_braceless_comment_indent_spaces(&self) -> Option<usize> {
        if !self
            .output
            .last_non_empty_scoped()?
            .trimmed_start()
            .starts_with("#endif")
        {
            return None;
        }
        let scope_start = self.output.len() - self.output.scoped().len();
        for (offset, line) in self.output.scoped().iter().enumerate().rev().skip(1) {
            let trimmed = line.trimmed_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // A macro's body takes no part in the code around it.
            if self
                .output
                .directive_of_continuation(scope_start + offset)
                .is_some()
            {
                return None;
            }
            if trimmed.starts_with("if")
                && trimmed["if".len()..]
                    .chars()
                    .next()
                    .is_none_or(|ch| !(ch == '_' || ch.is_ascii_alphanumeric()))
            {
                return Some(
                    leading_visual_width(line, self.options.tab_width) + self.options.indent_width,
                );
            }
            return None;
        }
        None
    }

    fn push_continued_line_comment(&mut self, comment: &str) {
        let mut lines = comment.lines();
        let Some(first) = lines.next() else {
            return;
        };
        if !self.current.trimmed().is_empty() {
            self.pad_before_trailing_comment(CommentKind::Line, first);
        }
        let output_column = leading_visual_width(&self.current, self.options.tab_width.max(1));
        self.record_comment_frame(CommentKind::Line, output_column, true);
        self.current.push_str(first.trimmed_end());
        self.finish_line();
        for line in lines {
            self.push_output_line(line.trimmed(), self.layout.indentation.indent());
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    fn push_multiline_block_comment(&mut self, comment: &str, opener_indent: Option<usize>) {
        // A comment among a compound literal's elements stands with them.
        let opener_indent = self
            .innermost_brace_is_compound_literal()
            .then(|| self.current_inline_array_column())
            .flatten()
            .or(opener_indent);
        let interrupted_header = self.layout.command_state.current_header.clone();
        let open_paren_comment_indent = (self.layout.previous == PreviousToken::OpenParen)
            .then(|| self.comment_after_open_paren_indent_spaces());
        if !self.current.trimmed().is_empty() {
            // The element after a comment trailing an element of a brace
            // stands with the elements.
            let element_column = self
                .current
                .trimmed_end()
                .ends_with(',')
                .then(|| self.current_inline_array_column())
                .flatten();
            self.push_trailing_block_comment_lines(comment);
            if let Some(column) = element_column {
                self.layout.continuation_indent.set_next_line_spaces(column);
            }
            self.attach_source_space_after_block_comment();
            if self.layout.command_state.current_header.is_none() {
                self.layout.command_state.current_header = interrupted_header;
            }
            if let Some(spaces) = open_paren_comment_indent {
                self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
                self.layout
                    .nesting
                    .push_continuation_indent_spaces_raw(spaces);
            }
            self.layout.previous = PreviousToken::Other;
            return;
        }

        self.finish_line();
        if self.layout.command_state.current_header.is_none() {
            self.layout.command_state.current_header = interrupted_header;
        }
        let tab_width = self.options.tab_width.max(1);
        let unindented_namespace_run_in_comment = self.token_input.token_line_opens_with_brace
            && !self.token_input.token_begins_source_line
            && !self.options.indent_namespaces
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Namespace | BraceType::Extern)
            )
            && self.output.last().is_some_and(|line| line.trimmed() == "{");
        let case_comment_unindent = if !self.options.indent_switches
            && self
                .layout
                .nesting
                .brace_header_stack
                .last()
                .is_some_and(|header| header.as_deref() == Some("case"))
        {
            self.options.indent_width
        } else {
            0
        };
        let mut opener_prefix = if unindented_namespace_run_in_comment {
            let spaces = self
                .output
                .last_non_empty_scoped()
                .map_or(0, |line| leading_visual_width(line, self.options.tab_width));
            " ".repeat(spaces)
        } else if self.token_input.token_line_opens_with_brace
            && !self.token_input.token_begins_source_line
        {
            let case_brace_extra = if self.output.last().is_some_and(|line| {
                let trimmed = line.trimmed();
                trimmed.starts_with("case ")
                    || trimmed.starts_with("default:") && trimmed.contains('{')
            }) {
                self.options.indent_width
            } else {
                0
            };
            let spaces = self.token_input.token_source_column
                + self.layout.line_adjuster.pending_case_unindent() * self.options.indent_width
                + case_brace_extra;
            self.options.continuation_indent_prefix(0, spaces)
        } else if let Some(previous) = self.output.last_non_empty_scoped()
            && {
                let code = self.output.code_trimmed_of(previous);
                code.trimmed_start().starts_with("switch") && code.ends_with('{')
            }
        {
            // A comment that trailed the brace stands in the case bodies.
            let levels = if self.token_input.token_begins_source_line {
                usize::from(self.comment_precedes_switch_statement())
                    * (1 + usize::from(self.options.brace_style == BraceStyle::Ratliff))
            } else {
                1 + usize::from(
                    self.options.indent_switches || self.options.brace_style == BraceStyle::Ratliff,
                )
            };
            " ".repeat(
                leading_visual_width(previous, self.options.tab_width)
                    + levels * self.options.indent_width,
            )
        } else if let Some(previous) = self.output.last_non_empty_scoped()
            && {
                let code = self.output.code_trimmed_of(previous);
                code.ends_with('{')
                    && (code.trimmed_start().starts_with("case ")
                        || code.trimmed_start().starts_with("default:"))
            }
        {
            " ".repeat(
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width
                    + case_comment_unindent,
            )
        } else if let Some(spaces) = self.split_else_preprocessor_branch_body_indent_spaces() {
            let spaces = opener_indent.map_or(spaces, |indent| indent.max(spaces));
            " ".repeat(spaces)
        } else {
            match opener_indent {
                Some(spaces) => self
                    .options
                    .continuation_indent_prefix(self.continuation_base_indent(), spaces),
                None => {
                    if self
                        .output
                        .last_non_empty_scoped()
                        .is_some_and(|line| line.trimmed_start().starts_with("@interface"))
                    {
                        self.options.indent_prefix(1)
                    } else {
                        self.options.indent_prefix(
                            self.layout
                                .continuation_indent
                                .next_line_indent
                                .unwrap_or_else(|| self.continuation_base_indent()),
                        )
                    }
                }
            }
        };
        if let Some(previous) = self.output.last_non_empty_scoped() {
            let code = self.output.code_trimmed_of(previous);
            if code.ends_with('{')
                && (code.trimmed_start().starts_with("case ")
                    || code.trimmed_start().starts_with("default:"))
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width
                    + case_comment_unindent;
                if leading_visual_width(&opener_prefix, tab_width) < spaces {
                    opener_prefix = " ".repeat(spaces);
                }
            }
        }
        if self
            .layout
            .nesting
            .brace_header_stack
            .last()
            .is_some_and(|header| header.as_deref() == Some("case"))
        {
            let spaces = self.current_line_indent_spaces();
            if leading_visual_width(&opener_prefix, tab_width) < spaces {
                opener_prefix = " ".repeat(spaces);
            }
        }
        let opener_output_column = leading_visual_width(&opener_prefix, tab_width);
        self.record_comment_frame(CommentKind::Block, opener_output_column, true);
        let trim_amount = if unindented_namespace_run_in_comment {
            self.token_input.token_source_column + self.options.indent_width.saturating_sub(1)
        } else if self.token_input.token_begins_source_line
            || self.token_input.token_line_opens_with_brace
        {
            self.token_input.token_source_column
        } else {
            self.token_input.token_source_line_indent
        };
        let run_in_opener = self.token_input.token_line_opens_with_brace
            && !self.token_input.token_begins_source_line
            && self.layout.command_state.current_header.is_none()
            && !self.options.remove_braces
            && match self.layout.nesting.brace_type_stack.last() {
                Some(BraceType::Command) => self.options.brace_style == BraceStyle::OneTrueBrace,
                Some(BraceType::Array | BraceType::Initializer) => true,
                _ => false,
            }
            && self.output.last().is_some_and(|line| line.trimmed() == "{");
        self.push_own_line_block_comment_lines(
            comment,
            &OwnLineBlockComment {
                opener_prefix: &opener_prefix,
                opener_output_column,
                trim_amount,
                run_in_opener,
                unindented_namespace_run_in_comment,
            },
        );
        if case_comment_unindent > 0 && self.current_is_preindented {
            self.finish_line();
            self.layout.continuation_indent.next_line_indent_spaces = Some(opener_output_column);
        } else if case_comment_unindent > 0 {
            self.layout.continuation_indent.next_line_indent_spaces = Some(opener_output_column);
        }
        self.attach_source_space_after_block_comment();
        self.layout.previous = PreviousToken::Other;
    }

    fn push_trailing_block_comment_lines(&mut self, comment: &str) {
        let mut lines = comment.lines().peekable();
        if let Some(first) = lines.next() {
            if self.layout.previous == PreviousToken::OpenParen {
                if self.options.pad_parens_inside {
                    self.pad_inside_paren_space();
                } else {
                    self.emit_source_space();
                }
            } else {
                self.pad_before_trailing_comment(CommentKind::Block, first);
            }
            let output_column = leading_visual_width(&self.current, self.options.tab_width.max(1));
            self.record_comment_frame(CommentKind::Block, output_column, true);
            self.current.push_str(first.trimmed_end());
            if lines.peek().is_some() {
                self.finish_line();
            }
            let last_line_has_trailing_token = self.token_input.has_next_meaningful_token;
            while let Some(line) = lines.next() {
                let shifted = if self.options.strip_comment_prefix {
                    // astyle keeps the text of a row of a comment opened
                    // after code at its column, past a leading `*`, and no
                    // further left than the code.
                    let tab_width = self.options.tab_width.max(1);
                    let text = line.trimmed_start();
                    let body = if text.starts_with('*') && !text.starts_with("*/") {
                        &text[1..]
                    } else {
                        text
                    };
                    // Rows of `**` and rows without a `*` indented with tabs
                    // keep their text where it stands.
                    let keeps_row = if text.starts_with('*') {
                        body.trimmed_start().starts_with('*')
                    } else {
                        line[..line.len() - text.len()].contains('\t')
                    };
                    let column = visual_width_from(
                        &line[..line.len() - body.trimmed_start().len()],
                        0,
                        tab_width,
                    )
                    .max(self.current_line_indent_spaces());
                    if body.trimmed().is_empty() {
                        String::new()
                    } else if text.starts_with("*/") || keeps_row {
                        line.trimmed_end().to_string()
                    } else {
                        format!("{}{}", " ".repeat(column), body.trimmed())
                    }
                } else {
                    line.trimmed_end().to_string()
                };
                // astyle writes the rows of a comment opened after code as
                // they stand.
                let verbatim = shifted == line.trimmed_end();
                let is_last_line = lines.peek().is_none();
                let last_line_starts_with_star = line.trimmed_start().starts_with('*');
                let keep_line_open =
                    is_last_line && (last_line_has_trailing_token || !last_line_starts_with_star);
                if keep_line_open {
                    self.current.push_str(&shifted);
                    self.current_is_preindented = true;
                    self.current_is_verbatim = verbatim;
                } else {
                    let published = self.output.len();
                    self.push_raw_comment_output_line(shifted);
                    if verbatim && self.output.len() > published {
                        self.output.mark_last_verbatim();
                    }
                }
            }
        }
    }

    fn push_own_line_block_comment_lines(
        &mut self,
        comment: &str,
        layout: &OwnLineBlockComment<'_>,
    ) {
        let OwnLineBlockComment {
            opener_prefix,
            opener_output_column,
            trim_amount,
            run_in_opener,
            unindented_namespace_run_in_comment,
        } = *layout;
        let tab_width = self.options.tab_width.max(1);
        // A source run-in brace takes the opener back onto its line later.
        let opener_on_own_line = !run_in_opener
            && !self
                .source_run_in_brace_lines
                .contains(&self.output.len().wrapping_sub(1));
        let mut lines = comment.lines().enumerate().peekable();
        while let Some((index, line)) = lines.next() {
            if index == 0 && run_in_opener {
                // The comment keeps the gap it had after the brace.
                let gap = self
                    .token_input
                    .previous_input_whitespace
                    .clone()
                    .filter(|ws| !ws.contains('\n'))
                    .unwrap_or_default();
                if let Some(brace_line) = self.output.last_mut() {
                    brace_line.push_str(&gap);
                    brace_line.push_str(line.trimmed_end());
                }
                continue;
            }
            let formatted = if self.options.strip_comment_prefix {
                strip_block_comment_line(self.options, line, index == 0, opener_prefix, trim_amount)
            } else if index == 0 {
                format!("{opener_prefix}{}", line.trimmed_end())
            } else {
                if line.trimmed().is_empty() {
                    String::new()
                } else {
                    let kept = drop_leading_columns(line, trim_amount, tab_width);
                    let trimmed_kept = kept.trimmed_start();
                    let is_last_line = lines.peek().is_none();
                    let decorative_closer = is_decorative_block_comment_closer(trimmed_kept);
                    let source_closer_leading = leading_visual_width(line, tab_width);
                    let closer_prefix = if decorative_closer && is_last_line {
                        self.output
                            .last()
                            .or(self.layout.previous_pre_adjust_line.as_ref())
                            .and_then(|previous| {
                                let leading =
                                    leading_visual_width(previous, self.options.tab_width);
                                (previous.trimmed_start().starts_with('*')
                                    && if self.token_input.token_line_opens_with_brace {
                                        leading >= opener_output_column
                                            && source_closer_leading > trim_amount
                                    } else {
                                        source_closer_leading < trim_amount
                                            && leading == source_closer_leading
                                    })
                                .then(|| {
                                    previous[..previous.len() - previous.trimmed_start().len()]
                                        .to_string()
                                })
                            })
                            .or_else(|| {
                                kept.starts_with(" */").then(|| format!("{opener_prefix} "))
                            })
                    } else {
                        None
                    };
                    let star_shift = index > 1
                        && self.token_input.token_begins_source_line
                        && trim_amount > opener_output_column
                        && kept.starts_with('*')
                        && leading_visual_width(line, tab_width) < trim_amount
                        && (!decorative_closer || (!is_last_line && index > 2));
                    if let Some(prefix) = closer_prefix {
                        format!("{}{}", prefix, trimmed_kept.trimmed_end())
                    } else if unindented_namespace_run_in_comment {
                        format!("{opener_prefix}{}", kept.trimmed_end())
                    } else if self.token_input.token_line_opens_with_brace && opener_on_own_line {
                        // The opener stands on its own line, which the rows
                        // follow wherever the line moves.
                        let source_line_column = leading_visual_width(line, tab_width);
                        let body_offset = if decorative_closer && is_last_line {
                            0
                        } else if trimmed_kept.starts_with('*') {
                            source_line_column.saturating_sub(trim_amount).min(1)
                        } else {
                            source_line_column.saturating_sub(trim_amount)
                        };
                        format!(
                            "{}{}",
                            " ".repeat(opener_output_column + body_offset),
                            trimmed_kept.trimmed_end()
                        )
                    } else if self.token_input.token_line_opens_with_brace {
                        let source_line_column = leading_visual_width(line, tab_width);
                        let body_offset = if decorative_closer && is_last_line {
                            0
                        } else if trimmed_kept.starts_with('*') {
                            // A star keeps its source offset from the `/*`.
                            source_line_column.saturating_sub(trim_amount).min(1)
                        } else {
                            source_line_column.saturating_sub(trim_amount)
                        };
                        let indent = if self.options.indent_classes
                            && matches!(
                                self.layout.nesting.brace_type_stack.last(),
                                Some(BraceType::Class)
                            ) {
                            self.layout.indentation.indent().saturating_sub(1)
                        } else {
                            self.layout.indentation.indent()
                        };
                        let merged_comment_column =
                            self.layout.frame_stack.active_brace().map_or_else(
                                || {
                                    ContinuationIndent::Level(indent)
                                        .columns(self.options.indent_width)
                                },
                                |frame| frame.body_indent_column.max(opener_output_column),
                            );
                        let target = merged_comment_column + body_offset;
                        format!(
                            "{}{}",
                            self.options.continuation_indent_prefix(
                                merged_comment_column / self.options.indent_width.max(1),
                                target,
                            ),
                            trimmed_kept.trimmed_end()
                        )
                    } else if star_shift {
                        format!("{opener_prefix} {}", kept.trimmed_end())
                    } else {
                        format!("{opener_prefix}{}", kept.trimmed_end())
                    }
                }
            };
            if lines.peek().is_none()
                && !formatted.is_empty()
                && self.token_input.token_line_opens_with_brace
            {
                self.push_raw_comment_output_line(formatted);
            } else if lines.peek().is_none() && !formatted.is_empty() {
                self.current.push_str(&formatted);
                self.current_is_preindented = true;
            } else if formatted.is_empty() {
                self.push_empty_line();
            } else {
                self.push_raw_comment_output_line(formatted);
            }
        }
    }

    fn attach_source_space_after_block_comment(&mut self) {
        if !self.current_is_preindented {
            return;
        }
        if let Some(whitespace) = self
            .token_input
            .next_input_whitespace
            .clone()
            .filter(|whitespace| !whitespace.contains('\n'))
        {
            self.current.push_str(&whitespace);
        }
    }

    fn should_break_header_before_comment(&self) -> bool {
        self.previous_was_newline
            && !self.current.trimmed().is_empty()
            && self
                .layout
                .command_state
                .current_header
                .as_deref()
                .is_some_and(is_standard_add_braces_header)
    }

    fn enum_value_comment_continuation_indent_spaces(&self) -> Option<usize> {
        if !matches!(
            self.layout.nesting.brace_type_stack.last(),
            Some(BraceType::Enum)
        ) {
            return None;
        }
        let previous = self
            .output
            .last()
            .filter(|line| !line.trimmed().is_empty())?;
        let (code, _) = previous.split_once("//")?;
        if code.trimmed_end().ends_with(',') {
            return None;
        }
        let value_start = code.find('=')? + 1;
        Some(
            value_start
                + code[value_start..]
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .count(),
        )
    }

    /// Whether the brace starting the source line of the comment being
    /// pushed is still open at the comment, as a brace run into its row.
    fn comment_follows_open_source_line_brace(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some(comment) = self.current.active_comment() else {
            return false;
        };
        let line_start = tokens[..comment]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |newline| newline + 1);
        next_code_token(tokens, line_start)
            .filter(|&first| first < comment && matches!(tokens[first], Token::Symbol('{')))
            .and_then(|first| self.tree.groups.opened_at(first))
            .is_some_and(|group| {
                self.tree
                    .groups
                    .get(group)
                    .close
                    .is_none_or(|close| close > comment)
            })
    }

    /// Whether the current line starts with code its source line had a
    /// control header before, the header now on a line of its own.
    /// How much wider the code on the current line came out than its
    /// source tokens, whitespace runs counted as written.
    fn current_segment_growth(&self) -> Option<isize> {
        let tokens = &self.tree.tokens;
        let first = self.current.tokens()?.first;
        let comment = self.current.active_comment()?;
        if comment <= first || comment > tokens.len() {
            return None;
        }
        let mut end = comment;
        while end > first && matches!(tokens[end - 1], Token::Whitespace(_)) {
            end -= 1;
        }
        if tokens[first..end]
            .iter()
            .any(|token| matches!(token, Token::Newline | Token::Comment(..)))
        {
            return None;
        }
        let source: usize = tokens[first..end].iter().map(token_char_len).sum();
        let code = self.current.trimmed().chars().count();
        Some(code as isize - source as isize)
    }

    /// Whether the current line starts with a statement its source line
    /// had another statement before, now on a line of its own.
    fn current_line_broke_off_statement(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some(first) = self.current.tokens().map(|span| span.first) else {
            return false;
        };
        let line_start = tokens[..first]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |index| index + 1);
        self.tree
            .previous_code_token(first)
            .is_some_and(|previous| {
                previous >= line_start && matches!(tokens[previous], Token::Symbol(';'))
            })
    }

    fn current_line_broke_off_header(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some(first) = self.current.tokens().map(|span| span.first) else {
            return false;
        };
        let line_start = tokens[..first]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |index| index + 1);
        // An `if` broken from its `else` pads as on the `else`'s line, unless
        // a brace leaves it.
        let breaks_else_if = matches!(&tokens[first], Token::Word(word) if word == "if")
            && !tokens[first..]
                .iter()
                .take_while(|token| !matches!(token, Token::Newline))
                .any(|token| matches!(token, Token::Symbol('{')));
        tokens[line_start..first]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_)))
            .is_some_and(|token| {
                matches!(token, Token::Word(word)
                    if is_header(self.options, word) && !(breaks_else_if && word == "else"))
            })
    }

    fn pad_before_trailing_comment(&mut self, kind: CommentKind, comment: &str) {
        let had_formatter_space = (self.current.ends_with(' ') || self.current.ends_with('\t'))
            && matches!(
                self.current.trimmed_end().chars().next_back(),
                Some('*' | '&' | '^')
            )
            && !self.current.trimmed_end().ends_with("&&");
        let formatter_gap = had_formatter_space.then(|| {
            let start = self.current.trim_end_matches([' ', '\t']).len();
            self.current[start..].to_string()
        });
        while self.current.ends_with(' ') || self.current.ends_with('\t') {
            self.current.pop();
        }
        let gap = self
            .token_input
            .previous_input_whitespace
            .clone()
            .unwrap_or_default();
        // Braces added around the statement before move its comment no
        // further.
        if std::mem::take(&mut self.comments.follows_added_one_line_block) {
            // Padding moves the comment as on any line; the four columns of
            // the added braces do not.
            let padded_gap = (!gap.is_empty()
                && !gap.contains('\t')
                && !matches!(
                    self.options.brace_style,
                    BraceStyle::Pico | BraceStyle::Lisp
                )
                && !self.layout.line_state.trailing_comment_columns.is_empty())
            .then(|| {
                let target = self
                    .layout
                    .line_state
                    .trailing_comment_columns
                    .pop_front()
                    .unwrap_or_default();
                let out_indent = self
                    .current
                    .chars()
                    .take_while(|ch| ch.is_whitespace())
                    .count();
                let code_len = (self.current_char_len() - out_indent).saturating_sub(4);
                target.saturating_sub(code_len).max(1)
            });
            if gap.is_empty() {
                self.ensure_space();
            } else if let Some(width) = padded_gap {
                self.current.push_str(&" ".repeat(width));
            } else {
                self.current.push_str(&gap);
                // Attaching a bare closer takes a space out of its gap.
                if matches!(
                    self.options.brace_style,
                    BraceStyle::Pico | BraceStyle::Lisp
                ) && gap.len() > 1
                    && gap.bytes().all(|byte| byte == b' ')
                    && self
                        .current
                        .trimmed()
                        .chars()
                        .all(|ch| matches!(ch, '}' | ';' | ' '))
                {
                    self.current.push(' ');
                }
            }
            return;
        }
        if self.layout.line_state.is_multi_statement_line {
            if gap.is_empty() {
                if kind == CommentKind::Block && comment.contains("NOPAD") {
                    self.ensure_space();
                }
            } else if !gap.contains('\t') && self.current_line_broke_off_statement() {
                // Padding the statement broken off takes from the gap.
                let growth = self.current_segment_growth().unwrap_or(0);
                let width = (gap.chars().count() as isize - growth).max(1) as usize;
                self.current.push_str(&" ".repeat(width));
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        let case_body_split_from_label = self.output.last().is_some_and(|line| {
            let trimmed = line.trimmed();
            trimmed.starts_with("case ") || trimmed == "default:"
        }) && self.layout.line_state.passed_colon
            && self.layout.line_state.passed_semicolon;
        if case_body_split_from_label {
            self.current.push_str(&gap);
            return;
        }
        if kind == CommentKind::Block
            && (self.token_input.token_followed_by_line_comment_on_line
                || self.token_input.next_token_is_line_comment
                || self.layout.line_state.trailing_comment_columns.len() > 1)
            && self.current.holds_close_brace()
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line && self.current.trimmed_end().ends_with("*/") {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line
            && self.layout.line_state.passed_semicolon
            && self.layout.line_state.trailing_comment_columns.is_empty()
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line
            && has_hash_outside_literals(&self.current)
            && !self.current.trimmed_start().starts_with('#')
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        let target_column = (!self.layout.line_state.trailing_comment_columns.is_empty())
            .then(|| self.layout.line_state.trailing_comment_columns.pop_front())
            .flatten();
        let trimmed_code = self.current.trimmed();
        if trimmed_code.starts_with("case ") || trimmed_code.starts_with("default:") {
            self.current.push_str(&gap);
            return;
        }
        if trimmed_code.starts_with('}')
            && trimmed_code[1..].chars().all(|ch| ch == ';' || ch == ',')
        {
            self.current.push_str(&gap);
            return;
        }
        if kind == CommentKind::Line
            && trimmed_code == "{"
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
        {
            let gap = initializer_brace_line_comment_gap(self.options, &self.current);
            self.current.push_str(&gap);
            return;
        }
        // A row a brace runs into keeps its gap; padding a row moves its
        // comment no further.
        if kind == CommentKind::Block
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
            && self.current.trimmed_start().starts_with('{')
            && self.comment_follows_open_source_line_brace()
        {
            self.current.push_str(&gap);
            return;
        }
        // A statement broken off its header's line keeps the gap before its
        // comment.
        if target_column.is_some()
            && !gap.is_empty()
            && !gap.contains('\t')
            && self.current_line_broke_off_header()
        {
            // Padding the statement takes from the gap as on any line.
            let growth = self.current_segment_growth().unwrap_or(0);
            let width = (gap.chars().count() as isize - growth).max(1) as usize;
            self.current.push_str(&" ".repeat(width));
            return;
        }
        // astyle moves no block comment that code follows on its line.
        let code_follows = kind == CommentKind::Block
            && self.token_input.has_next_meaningful_token
            && !self.token_input.next_token_is_line_comment
            && !self
                .token_input
                .next_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| whitespace.contains('\n'));
        if code_follows
            && target_column.is_some()
            && !gap.is_empty()
            && !gap.contains('\t')
            && !self.current.trimmed_end().ends_with_any(b"*&^")
        {
            self.current.push_str(&gap);
            return;
        }
        let Some(target_column) = target_column else {
            if gap.is_empty() {
                let keeps_adjacent_comment = self.layout.previous == PreviousToken::Comma
                    || self.layout.previous == PreviousToken::Operator
                        && (!self.options.pad_operators
                            || self.layout.line_state.operator_padding_disabled);
                if kind != CommentKind::Block
                    && !self.current.trimmed_end().ends_with("*/")
                    && !keeps_adjacent_comment
                {
                    self.ensure_space();
                }
            } else {
                self.current.push_str(&gap);
            }
            return;
        };
        let pointer_alignment_moves_symbol = self.options.pointer_align != PointerAlign::None
            || !matches!(
                self.options.reference_align,
                ReferenceAlign::None | ReferenceAlign::SameAsPointer
            );
        if had_formatter_space && pointer_alignment_moves_symbol {
            self.current
                .push_str(formatter_gap.as_deref().unwrap_or_default());
            return;
        }
        let out_indent = self
            .current
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .count();
        let code_len = self.current_char_len() - out_indent;
        let gap_chars = gap.chars().count();
        // A block comment set against the code stays there unless padding
        // moved the code into its column, when astyle parts them.
        if kind == CommentKind::Block && gap.is_empty() {
            let ends_line = !self.token_input.has_next_meaningful_token
                || self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.contains('\n'));
            if code_len > target_column {
                self.ensure_space();
            } else if ends_line {
                // Code unpadding shortened keeps a trailing comment at its
                // column.
                self.current.push_str(&" ".repeat(target_column - code_len));
            }
            return;
        }
        let space_pad = (code_len + gap_chars) as isize - target_column as isize;
        // A wide gap before a line comment only shrinks.
        if kind == CommentKind::Line && gap_chars >= self.options.indent_width * 2 && space_pad < 0
        {
            self.current.push_str(&gap);
            return;
        }
        if gap.contains('\t') {
            self.current.push_str(&gap);
        } else if space_pad < 0 {
            self.current.push_str(&gap);
            self.current.push_str(&" ".repeat((-space_pad) as usize));
        } else if space_pad > 0 {
            let final_gap = (gap_chars as isize - space_pad).max(1) as usize;
            self.current.push_str(&" ".repeat(final_gap));
        } else if gap.is_empty() && had_formatter_space {
            self.ensure_space();
        } else {
            self.current.push_str(&gap);
        }
    }
}

fn is_decorative_block_comment_closer(line: &str) -> bool {
    line.ends_with("*/") && line.chars().all(|ch| matches!(ch, '*' | '/'))
}

pub(crate) fn trailing_comment_columns(tokens: &[Token]) -> Vec<usize> {
    let mut columns = Vec::new();
    let mut column = 0usize;
    let mut seen_code = false;
    let mut seen_token = false;
    let mut open_brace_depth = 0usize;
    let mut leading_indent = 0usize;
    let mut saw_closing_brace = false;
    let mut saw_closing_header = false;
    let mut code_after_open_brace = false;
    let mut segment_start_column = 0usize;
    let mut pending_segment_start = false;
    let mut first_code_is_open_brace = false;
    let mut last_code_is_open_brace = false;
    for token in tokens {
        let code_before_token = code_after_open_brace;
        if !seen_code
            && !matches!(
                token,
                Token::Whitespace(_) | Token::Newline | Token::Comment(..)
            )
        {
            first_code_is_open_brace = matches!(token, Token::Symbol('{'));
        }
        let is_code_after_brace = open_brace_depth > 0
            && first_code_is_open_brace
            && !matches!(
                token,
                Token::Whitespace(_) | Token::Newline | Token::Comment(..)
            );
        if is_code_after_brace {
            code_after_open_brace = true;
            if pending_segment_start {
                segment_start_column = column;
                pending_segment_start = false;
            }
        }
        if !matches!(
            token,
            Token::Whitespace(_) | Token::Newline | Token::Comment(..)
        ) {
            last_code_is_open_brace = matches!(token, Token::Symbol('{'));
        }
        match token {
            Token::Whitespace(text) => {
                let width = text.chars().count();
                if !seen_token {
                    leading_indent += width;
                }
                column += width;
            }
            Token::Newline => break,
            Token::Comment(_, comment) => {
                if seen_code && !comment.contains('\n') {
                    // A brace ending the code of a line leads to its body.
                    let after_ending_brace = open_brace_depth == 1
                        && last_code_is_open_brace
                        && !first_code_is_open_brace;
                    if open_brace_depth == 0
                        || (open_brace_depth == 1 && saw_closing_header)
                        || after_ending_brace
                    {
                        columns.push(column.saturating_sub(leading_indent));
                    } else if code_after_open_brace {
                        columns.push(column.saturating_sub(segment_start_column));
                    }
                }
                column += comment.chars().count();
                seen_token = true;
            }
            Token::Word(word) => {
                // A closing header leading its line joins the brace before.
                if (saw_closing_brace || !seen_code) && is_break_blocks_closing_header(word) {
                    saw_closing_header = true;
                }
                seen_code = true;
                seen_token = true;
                column += token_char_len(token);
            }
            Token::Symbol('{') => {
                seen_code = true;
                seen_token = true;
                open_brace_depth += 1;
                // Braces within the block's code leave its segment as it is.
                if open_brace_depth == 1 || !code_before_token {
                    code_after_open_brace = false;
                    pending_segment_start = true;
                }
                column += 1;
            }
            Token::Symbol('}') => {
                seen_code = true;
                seen_token = true;
                open_brace_depth = open_brace_depth.saturating_sub(1);
                saw_closing_brace = true;
                if open_brace_depth == 0 {
                    code_after_open_brace = false;
                }
                column += 1;
            }
            _ => {
                seen_code = true;
                seen_token = true;
                column += token_char_len(token);
            }
        }
    }
    columns
}

/// Comment placement decisions carried between tokens and lines.
#[derive(Debug, Default)]
pub(crate) struct CommentState {
    run_in_comment_brace_lines: Vec<usize>,
    pub(crate) line_comment_starts_reordered_brace_body: bool,
    pub(crate) reordered_brace_line_comment_gap: Option<String>,
    pub(crate) next_comment_ends_line: bool,
    pub(crate) skip_next_attached_comment: bool,
    pub(crate) follows_added_one_line_block: bool,
    pub(crate) block_comment_close_paren_ends_declaration: bool,
    pub(crate) previous_block_comment_close_paren_ended_declaration: bool,
}

fn strip_block_comment_line(
    options: &FormatOptions,
    line: &str,
    is_opener: bool,
    prefix: &str,
    opener_source_column: usize,
) -> String {
    let indent_len = options.indent_width;
    let tab_width = options.tab_width.max(1);
    let chars: Vec<char> = line.chars().collect();

    if is_opener {
        let Some(mut content_start) = chars[2..]
            .iter()
            .position(|&ch| ch != ' ' && ch != '\t')
            .map(|pos| pos + 2)
        else {
            return format!("{prefix}/*");
        };
        if matches!(chars[content_start], '*' | '!') {
            match chars[content_start + 1..]
                .iter()
                .position(|&ch| ch != ' ' && ch != '\t')
                .map(|pos| content_start + 1 + pos)
            {
                Some(next) if chars[next] != '*' => content_start = next,
                _ => return format!("{prefix}{}", line.trimmed_end()),
            }
        }
        let content_column = visual_column_at(&chars, content_start, tab_width);
        let insert = indent_len.saturating_sub(content_column);
        let head: String = chars[..content_start].iter().collect();
        let tail: String = chars[content_start..].iter().collect();
        return format!("{prefix}{head}{}{}", " ".repeat(insert), tail.trimmed_end());
    }

    let Some(first) = chars.iter().position(|&ch| ch != ' ' && ch != '\t') else {
        return String::new();
    };
    if chars[first] == '*' && chars.get(first + 1) == Some(&'/') {
        return format!("{prefix}*/");
    }
    if chars[first] == '*' {
        let Some(second) = chars[first + 1..]
            .iter()
            .position(|&ch| ch != ' ' && ch != '\t')
            .map(|pos| first + 1 + pos)
        else {
            return String::new();
        };
        if chars[second] == '*' {
            let rel =
                visual_column_at(&chars, first, tab_width).saturating_sub(opener_source_column);
            let content: String = chars[first..].iter().collect();
            return format!("{prefix}{}{}", " ".repeat(rel), content.trimmed_end());
        }
        // A tab before the text leaves the line as it was, less its `*`.
        let own_lead = lead_past_column(&chars[..first], opener_source_column, tab_width);
        if own_lead.contains(&'\t') || chars[first + 1..second].contains(&'\t') {
            let mut kept: String = own_lead.iter().chain(&chars[first + 1..]).collect();
            kept = kept.trimmed_end().to_string();
            if kept.ends_with('*') {
                kept.pop();
                kept = kept.trimmed_end().to_string();
            }
            return format!("{prefix}{kept}");
        }
        let rel = visual_column_at(&chars, second, tab_width)
            .saturating_sub(opener_source_column)
            .max(indent_len);
        let mut content = chars[second..]
            .iter()
            .collect::<String>()
            .trimmed_end()
            .to_string();
        if content.ends_with('*') {
            content.pop();
            content = content.trimmed_end().to_string();
        }
        return format!("{prefix}{}{content}", " ".repeat(rel));
    }
    let content: String = chars[first..].iter().collect();
    // A line indented with a tab past the opener's column keeps its indent.
    let own_lead = lead_past_column(&chars[..first], opener_source_column, tab_width);
    if own_lead.contains(&'\t') {
        let lead: String = own_lead.iter().collect();
        return format!("{prefix}{lead}{}", content.trimmed_end());
    }
    let rel = visual_column_at(&chars, first, tab_width)
        .saturating_sub(opener_source_column)
        .max(indent_len);
    format!("{prefix}{}{}", " ".repeat(rel), content.trimmed_end())
}

/// The part of the leading whitespace `lead` past visual column `column`.
fn lead_past_column(lead: &[char], column: usize, tab_width: usize) -> &[char] {
    let mut width = 0;
    for (index, &ch) in lead.iter().enumerate() {
        if width >= column {
            return &lead[index..];
        }
        width += if ch == '\t' {
            tab_width - width % tab_width
        } else {
            1
        };
    }
    &lead[lead.len()..]
}

/// Whether a line holds code or another block comment after its first
/// `*/`.
pub(crate) fn text_follows_comment_close(line: &str) -> bool {
    line.split_once("*/").is_some_and(|(_, after)| {
        let after = after.trimmed();
        !after.is_empty() && !after.starts_with("//")
    })
}
