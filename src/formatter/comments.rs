use super::block_spacing::is_break_blocks_closing_header;
use super::brace_postprocess::horstmann_run_in_fill;
use super::columns::{
    drop_leading_columns, leading_visual_width, visual_column_at, visual_width_from,
};
use super::disabled_formatting::DisabledFormattingState;
use super::frame::{BraceSemanticKind, CommentFrame, CommentFrameKind};
use super::indentation::LineKind;
use super::labels;
use super::language;
use super::line_scan::{is_comment_line, is_comment_only_line, line_ends_with_comment};
use super::operators::{
    find_assignment_operator, head_ends_assignment_operator, head_ends_binary_operator,
    starts_with_chain_operator,
};
use super::preprocessor::PreprocessorRegion;
use super::preprocessor::preprocessor_directive;
use super::rewrite::is_add_braces_header;
use super::state::ContinuationIndent;
use super::state::FormatterBraceType;
use super::token::{CommentKind, Token, token_char_len};
use super::{
    FormatEngine, PointerAlign, PreviousToken, ReferenceAlign, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::config::BraceStyle;

fn comment_starts_header_word(line: &str, word: &str) -> bool {
    line.strip_prefix(word)
        .is_some_and(|tail| tail.starts_with(|ch: char| ch == '(' || ch.is_whitespace()))
}

fn post_closing_declaration_owns_comment(line: &str) -> bool {
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
    !matches!(word, "else" | "catch" | "while" | "__finally" | "__except")
}

pub(super) fn line_comment_backslash_trailing_space(line: &str) -> bool {
    if !line.chars().next_back().is_some_and(char::is_whitespace) {
        return false;
    }
    let comment = trailing_comment_split_limit(line);
    let tail = line[comment..].trim_start();
    comment < line.len() && tail.starts_with("//") && tail.trim_end().ends_with('\\')
}

impl FormatEngine<'_> {
    pub(super) fn schedule_run_in_comment_brace_merge(&mut self, brace_line: usize) {
        self.run_in_comment_brace_lines.push(brace_line);
    }

    pub(super) fn merge_run_in_comment_braces(&mut self) {
        let mut indices = std::mem::take(&mut self.run_in_comment_brace_lines);
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            let Some(comment_line) = self.output.get(index + 1) else {
                continue;
            };
            let trimmed = comment_line.trim_start();
            if !(trimmed.starts_with("//") || trimmed.starts_with("/*"))
                || trimmed.contains("*INDENT-OFF*")
            {
                continue;
            }
            if self.output[index].trim() != "{" {
                continue;
            }
            let comment_line = self.output.remove(index + 1);
            let fill = horstmann_run_in_fill(&self.output[index], &comment_line, self.options);
            let merged = format!(
                "{}{}{}",
                self.output[index],
                fill,
                comment_line.trim_start()
            );
            self.output.set(index, merged);
        }
    }

    pub(super) fn push_raw_comment_output_line(&mut self, line: String) {
        if self.take_block_spacing_blank(&line) {
            self.push_empty_line();
        }
        self.line_adjuster.observe_raw_comment_line(&line);
        self.adjust_and_publish_line(line);
    }

    pub(super) fn align_adjacent_block_comments_before_adjustment(&self, line: String) -> String {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("/*") || trimmed.match_indices("/*").nth(1).is_none() {
            return line;
        }
        let target = self
            .stack_state
            .current_continuation_indent_spaces()
            .or_else(|| self.active_body_comment_indent_spaces())
            .unwrap_or_else(|| {
                (self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width
            });
        if leading_visual_width(&line, self.options.tab_width) > target {
            format!("{}{}", " ".repeat(target), trimmed)
        } else {
            line
        }
    }

    pub(super) fn observe_raw_output_comment_frame(&mut self, line: &str) {
        let output_spaces = leading_visual_width(line, self.options.tab_width);
        self.observe_output_comment_frame(line, output_spaces, false);
    }

    pub(super) fn observe_formatted_output_comment_frame(
        &mut self,
        line: &str,
        output_spaces: usize,
    ) {
        self.observe_output_comment_frame(line, output_spaces, true);
    }

    pub(super) fn line_comment_continuation_anchor_column(&self) -> Option<usize> {
        self.frame_stack
            .active_comment()
            .and_then(|frame| frame.continuation_anchor_column)
    }

    fn observe_output_comment_frame(
        &mut self,
        line: &str,
        output_spaces: usize,
        record_split_else_indent: bool,
    ) {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            if let Some(frame) = self.frame_stack.active_comment_mut() {
                frame.continuation_anchor_column = Some(output_spaces);
            }
            if record_split_else_indent {
                self.record_split_else_comment_body_indent(line, output_spaces);
            }
        } else if !trimmed.is_empty()
            && !is_comment_line(trimmed)
            && !line_ends_with_comment(trimmed)
        {
            self.frame_stack.clear_comments();
        }
    }

    pub(super) fn split_else_comment_row_indent_spaces(&self, line: &str) -> Option<usize> {
        if !is_comment_line(line.trim_start()) {
            return None;
        }
        let mut result = self.split_else_preprocessor_branch_body_indent_spaces();
        if let Some(previous) = self.output.last_non_empty_line() {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if previous_trimmed == "else" || previous_trimmed.ends_with("} else") {
                result = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
        }
        result
    }

    pub(super) fn none_style_post_comment_sibling_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
        {
            return None;
        }
        let mut comment_indent = None;
        for previous in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            if is_comment_line(previous.trim_start()) {
                comment_indent = Some(leading_visual_width(previous, self.options.tab_width));
                continue;
            }
            let previous_trimmed = previous.trim();
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

    pub(super) fn split_else_immediate_post_comment_indent_floor(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
        output_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if !is_comment_line(previous.trim_start()) {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width);
        (current_spaces.unwrap_or(output_spaces) < target).then_some(target)
    }

    pub(super) fn structural_split_else_post_comment_indent_spaces(
        &self,
        line: &str,
        current_spaces: usize,
        body_spaces: usize,
        structural_split_else_chain: bool,
    ) -> Option<usize> {
        if !structural_split_else_chain || line.trim_start().starts_with('#') {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        (previous_spaces == body_spaces
            && is_comment_line(previous.trim_start())
            && current_spaces < body_spaces)
            .then_some(body_spaces)
    }

    pub(super) fn preprocessor_else_comment_sibling_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal || line.trim_start().starts_with(['#', '{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        if preprocessor_directive(previous.trim_start()) != Some("endif") {
            return None;
        }
        let mut before_lines = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .filter(|line| !line.trim().is_empty());
        let before = before_lines.next()?;
        ((is_comment_line(before.trim_start()) || before.trim_start().starts_with("/*"))
            && before_lines
                .next()
                .is_some_and(|line| preprocessor_directive(line.trim_start()) == Some("else")))
        .then(|| leading_visual_width(before, self.options.tab_width))
    }

    pub(super) fn try_finish_preindented_comment_line(
        &mut self,
        close_paren_ends_declaration: bool,
    ) -> bool {
        if !self.current_is_preindented || !is_comment_line(&self.current) {
            return false;
        }
        let line_continuation_indent = self
            .continuation_indent
            .input_line_continuation_indent
            .take()
            .map(|indent| indent.columns(self.options.indent_width))
            .map(|captured| {
                self.frame_stack
                    .active_delimiter()
                    .filter(|frame| frame.opener_output_line < self.output.len())
                    .and_then(|frame| frame.continuation_indent_column)
                    .unwrap_or(captured)
            });
        let line = self.take_current();
        let mut trimmed = line.trim_end().to_string();
        if trimmed.trim_start().starts_with("/*") {
            let follows_switch = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    code.trim_start().starts_with("switch") && code.ends_with('{')
                });
            if !follows_switch {
                if let Some(spaces) = self.split_else_preprocessor_branch_body_indent_spaces() {
                    let prefix = self
                        .options
                        .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                    trimmed = format!("{prefix}{}", trimmed.trim_start());
                } else if let Some(spaces) = line_continuation_indent
                    .or_else(|| self.recent_paren_continuation_indent_spaces())
                {
                    let prefix = self
                        .options
                        .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                    trimmed = format!("{prefix}{}", trimmed.trim_start());
                }
            }
        }
        if trimmed
            .split_once("*/")
            .is_some_and(|(_, after)| after.trim_end().ends_with(';'))
        {
            self.continuation_indent.next_line_indent_spaces = None;
            self.stack_state.clear_continuation_indents();
        }
        if !trimmed.trim().is_empty() {
            let closes_standalone_block_comment = trimmed.trim_start().starts_with("*/");
            let next_indent = closes_standalone_block_comment.then(|| {
                self.frame_stack
                    .active_comment()
                    .filter(|frame| frame.kind == CommentFrameKind::Block && frame.multiline)
                    .map_or_else(
                        || leading_visual_width(&trimmed, self.options.tab_width),
                        |frame| frame.output_column,
                    )
                    + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width
            });
            self.push_raw_comment_output_line(trimmed);
            if close_paren_ends_declaration {
                self.previous_block_comment_close_paren_ended_declaration = true;
            }
            if let Some(spaces) = next_indent {
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        self.reset_after_finished_line();
        true
    }

    pub(super) fn push_inline_comment(&mut self, comment: &str) {
        if self.token_input.previous_input_was_adjacent {
            self.trim_current_end();
            if comment.trim_start().starts_with("//") {
                self.ensure_space();
            }
        } else {
            self.emit_source_space();
        }
        self.current.push_str(comment.trim_end());
        self.emit_trailing_source_space();
        self.previous = PreviousToken::Other;
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
                self.frame_stack
                    .active_comment()
                    .and_then(|frame| frame.continuation_anchor_column)
            })
            .flatten();
        self.frame_stack.push_comment(CommentFrame {
            kind: Self::comment_frame_kind(kind),
            output_column,
            multiline,
            continuation_anchor_column,
        });
    }

    pub(super) fn reindent_trailing_comment(&mut self, line_kind: LineKind) -> bool {
        let mut end = self.output.len();
        while end > 0 && self.output[end - 1].trim().is_empty() {
            end -= 1;
        }
        if end == 0 {
            return false;
        }
        let last = self.output[end - 1].trim_start();
        let mut indent = (self.state.line_indent(line_kind, self.options)
            + self.case_body_indent_extra(line_kind))
        .saturating_sub(self.line_adjuster.pending_case_unindent());
        if last.starts_with("//") {
            let mut start = end - 1;
            while start > 0 && self.output[start - 1].trim_start().starts_with("//") {
                start -= 1;
            }
            let prefix = self.options.indent_prefix(indent);
            for line in self.output.range_mut(start..end) {
                if line.starts_with("//") {
                    continue;
                }
                *line = format!("{prefix}{}", line.trim_start());
            }
            return true;
        }
        if !(last.starts_with("/*") || last.starts_with('*') || last.starts_with("*/")) {
            if !last.contains("*/") {
                return false;
            }
            let mut start = end - 1;
            while start > 0 && !self.output[start].trim_start().starts_with("/*") {
                let text = self.output[start].trim();
                if text.is_empty()
                    || text.ends_with(';')
                    || text.ends_with('{')
                    || text.ends_with('}')
                {
                    return false;
                }
                start -= 1;
            }
            if !self.output[start].trim_start().starts_with("/*") {
                return false;
            }
            let preserve_relative = if line_kind == LineKind::SwitchLabel && last.starts_with('}') {
                indent = self.state.indent();
                false
            } else {
                true
            };
            self.reindent_output_range(start, end, indent, preserve_relative);
            return true;
        }

        let mut start = end - 1;
        while start > 0 {
            let text = self.output[start].trim_start();
            let previous = self.output[start - 1].trim_start();
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
        if !self.output[start].trim_start().starts_with("/*") {
            return false;
        }
        if line_kind == LineKind::SwitchLabel
            && let Some(switch_line) = self.output[..start].iter().rev().find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with("switch") && code.ends_with('{')
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
            let text = line.trim_start();
            if offset == 0 || !preserve_relative {
                *line = format!("{prefix}{text}");
            } else {
                let relative = leading_visual_width(line, tab_width).saturating_sub(opener_leading);
                *line = format!("{prefix}{}{text}", " ".repeat(relative));
            }
        }
    }

    pub(super) fn active_body_comment_indent_spaces(&self) -> Option<usize> {
        let frame = self.frame_stack.active_brace()?;
        let body_column = if frame.formatter_type == FormatterBraceType::Namespace
            && (!self.options.indent_namespaces
                || self.options.brace_style == BraceStyle::Whitesmith)
        {
            frame.sibling_indent_column
        } else if frame.semantic_kind == BraceSemanticKind::Aggregate || frame.case_block {
            frame.body_indent_column.max(
                (self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width,
            )
        } else {
            frame.body_indent_column
        };
        if let Some(line) = self.output.last_non_empty_line().filter(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim();
            code.starts_with('}') && !code.ends_with('{')
        }) {
            Some(leading_visual_width(line, self.options.tab_width).min(body_column))
        } else {
            Some(body_column)
        }
    }

    fn previous_case_label_body_indent_spaces(&self) -> Option<usize> {
        let line = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        let candidate = code
            .trim_start()
            .strip_prefix('{')
            .map_or(code.trim_start(), str::trim_start);
        let colon = super::switch_cases::find_case_colon(candidate)?;
        candidate[colon + 1..].trim().is_empty().then(|| {
            let offset = code.len() - candidate.len();
            visual_width_from(&code[..offset], 0, self.options.tab_width)
                + self.options.indent_width
        })
    }

    pub(super) fn push_comment(&mut self, kind: CommentKind, comment: &str) {
        let function_try_initializer_comment = self.options.break_one_line_statements
            && self
                .frame_stack
                .active_constructor_initializer()
                .is_some_and(|frame| frame.function_try)
            && self.current.trim_end().ends_with(':');
        let standalone_line_comment = kind == CommentKind::Line
            && self.current.trim().is_empty()
            && comment.starts_with("//");
        let open_paren_comment_indent = (self.previous == PreviousToken::OpenParen)
            .then(|| self.comment_after_open_paren_indent_spaces());
        let close_paren_comment_indent = (self.previous == PreviousToken::CloseParen)
            .then(|| {
                self.assignment_continuation_indent_spaces()
                    .or_else(|| self.return_continuation_indent_spaces())
            })
            .flatten();
        let line_comment_starts_reordered_brace_body =
            kind == CommentKind::Line && self.line_comment_starts_reordered_brace_body;
        if line_comment_starts_reordered_brace_body {
            self.line_comment_starts_reordered_brace_body = false;
        }
        let reordered_brace_line_comment_gap = if kind == CommentKind::Line {
            self.reordered_brace_line_comment_gap.take()
        } else {
            None
        };
        let previous_line_ends_operator = self
            .output
            .iter()
            .rev()
            .find(|line| {
                let head = line.trim_start();
                !head.starts_with('#') && !head.starts_with("//")
            })
            .is_some_and(|line| {
                let head = line[..trailing_comment_split_limit(line)].trim_end();
                head_ends_binary_operator(head) || head_ends_assignment_operator(head)
            });
        let line_comment_can_carry_continuation = !standalone_line_comment
            || previous_line_ends_operator
            || matches!(
                self.previous,
                PreviousToken::Operator | PreviousToken::Comma
            );
        let line_comment_continuation_indent = (kind == CommentKind::Line
            && !self.in_enum_declaration_brace()
            && line_comment_can_carry_continuation
            && !(standalone_line_comment
                && comment.trim_end().ends_with(':')
                && !previous_line_ends_operator)
            && (!self.current.contains('#') || self.current.trim_start().starts_with('#'))
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
        let ternary_branch_comment_indent_spaces = (kind == CommentKind::Line
            && standalone_line_comment)
            .then(|| {
                let previous = self.output.last_non_empty_line()?;
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                (code.trim_start().starts_with('?')
                    && unmatched_open_paren_column(code).is_none()
                    && self.frame_stack.active_ternary().is_some())
                .then(|| leading_visual_width(previous, self.options.tab_width))
            })
            .flatten();
        let switch_label_block_comment_indent = (kind == CommentKind::Block
            && self.current.trim().is_empty()
            && comment.lines().any(|line| {
                let trimmed = line
                    .trim_start()
                    .strip_prefix("/*")
                    .unwrap_or(line.trim_start())
                    .trim_start();
                trimmed.starts_with("case ") || trimmed.starts_with("default:")
            })
            && self
                .stack_state
                .brace_header_stack
                .iter()
                .any(|header| header.as_deref() == Some("switch")))
        .then(|| self.state.indent().saturating_sub(1) * self.options.indent_width);
        let case_block_comment_indent = switch_label_block_comment_indent.or_else(|| {
            (kind == CommentKind::Block
                && self.current.trim().is_empty()
                && self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| line.trim() == "}")
                && self
                    .stack_state
                    .brace_header_stack
                    .iter()
                    .any(|header| header.as_deref() == Some("switch")))
            .then(|| {
                let after_blank = self
                    .output
                    .last()
                    .is_some_and(|line| line.trim().is_empty())
                    || self
                        .token_input
                        .previous_input_whitespace
                        .as_deref()
                        .is_some_and(|whitespace| whitespace.matches('\n').count() > 1);
                let previous_indent = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .map(|line| leading_visual_width(line, self.options.tab_width))
                    .unwrap_or(0);
                if after_blank {
                    let base = self.state.indent().saturating_sub(1) * self.options.indent_width;
                    base.max(previous_indent)
                } else {
                    (self.state.indent() * self.options.indent_width).max(previous_indent)
                }
            })
        });
        let definition_header_comment_indent = (!self.preprocessor.last_output_was_preprocessor
            && !self.line_adjuster.is_in_macro_block()
            && self.frame_stack.active_constructor_initializer().is_none()
            && self.current.trim().is_empty())
        .then(|| {
            self.output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .and_then(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    let header = trimmed
                        .split(|ch: char| ch == '(' || ch.is_whitespace())
                        .next()
                        .unwrap_or_default();
                    (trimmed.contains('(')
                        && trimmed.contains(')')
                        && !trimmed.starts_with(['}', '#'])
                        && !trimmed.ends_with([';', '{'])
                        && !trimmed.contains('=')
                        && !language::is_header(header))
                    .then(|| leading_visual_width(line, self.options.tab_width))
                })
        })
        .flatten();
        let case_label_comment_indent = (kind == CommentKind::Block
            && self.current.trim().is_empty())
        .then(|| self.previous_case_label_body_indent_spaces())
        .flatten();
        let column1_case_label_comment_indent = (kind == CommentKind::Line
            && self.current.trim().is_empty())
        .then(|| self.previous_case_label_body_indent_spaces())
        .flatten();
        let user_label_comment_indent = (kind == CommentKind::Block
            && self.current.trim().is_empty())
        .then(|| {
            let line = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())?;
            let code = line[..trailing_comment_split_limit(line)].trim();
            let label = code.strip_suffix(':')?.trim_end();
            (labels::line_kind(code, &self.options.access_labels) == LineKind::Label
                && !labels::is_access_label_start(label, &self.options.access_labels))
            .then(|| {
                (self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width
            })
        })
        .flatten();
        let control_header_comment_indent = (self.current.trim().is_empty()
            || (kind == CommentKind::Block && self.should_break_header_before_comment()))
        .then(|| {
            let header = self.frame_stack.active_header()?;
            let current = self.command_state.current_header.as_deref()?;
            if matches!(current, "case" | "default") {
                return None;
            }
            let pending_header_line = if self.current.trim().is_empty() {
                self.output
                    .iter()
                    .rev()
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !is_comment_only_line(trimmed)
                    })
                    .is_some_and(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_start();
                        let candidate = code
                            .strip_prefix('}')
                            .map_or(code, |tail| tail.trim_start());
                        candidate == current
                            || comment_starts_header_word(candidate, current)
                            || (self.command_state.previous_command_char == Some(')')
                                && candidate.trim_end().ends_with(')'))
                    })
            } else {
                self.should_break_header_before_comment()
            };
            (header.header == current
                && pending_header_line
                && (language::is_non_paren_header(current)
                    || self.command_state.previous_command_char == Some(')')))
            .then_some(header.body_indent_spaces)
        })
        .flatten();
        let block_comment_continuation_indent = case_block_comment_indent
            .or(case_label_comment_indent)
            .or(control_header_comment_indent)
            .or(user_label_comment_indent)
            .or_else(|| {
                if kind != CommentKind::Block || !self.current.trim().is_empty() {
                    return None;
                }
                let previous_line = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty());
                if let Some(line) = previous_line {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
                    let raw_trimmed = line.trim();
                    if raw_trimmed.starts_with("/*") && raw_trimmed.ends_with("*/") {
                        return Some(leading_visual_width(line, self.options.tab_width));
                    }
                    if trimmed.starts_with("switch") && code.ends_with('{') {
                        return Some(leading_visual_width(line, self.options.tab_width));
                    }
                    if trimmed.starts_with("} else") {
                        return Some(
                            leading_visual_width(line, self.options.tab_width)
                                + self.options.indent_width,
                        );
                    }
                    if preprocessor_directive(trimmed) == Some("endif")
                        && let Some(header) = self.output.iter().rev().skip(1).find(|line| {
                            let trimmed = line.trim_start();
                            !trimmed.is_empty() && !trimmed.starts_with('#')
                        })
                    {
                        let code = header[..trailing_comment_split_limit(header)].trim_end();
                        let trimmed = code.trim_start();
                        if trimmed == "else" || trimmed.ends_with("} else") {
                            return Some(
                                leading_visual_width(header, self.options.tab_width)
                                    + self.options.indent_width,
                            );
                        }
                    }
                    if trimmed.starts_with("} ") && !trimmed.ends_with('{') {
                        return Some(0);
                    }
                }
                let previous_statement_indent = previous_line.and_then(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let after_braceless_header = self
                        .output
                        .iter()
                        .rev()
                        .skip_while(|candidate| candidate.as_str() != line.as_str())
                        .skip(1)
                        .find(|candidate| !candidate.trim().is_empty())
                        .is_some_and(|candidate| {
                            let code =
                                candidate[..trailing_comment_split_limit(candidate)].trim_end();
                            let trimmed = code.trim_start();
                            let header = trimmed
                                .split(|ch: char| ch != '_' && !ch.is_ascii_alphanumeric())
                                .next()
                                .unwrap_or_default();
                            !trimmed.ends_with(['{', ';'])
                                && (matches!(header, "if" | "for" | "while")
                                    || trimmed == "else"
                                    || trimmed.starts_with("else if"))
                        });
                    let previous_indent = leading_visual_width(line, self.options.tab_width);
                    let body_indent = (self.state.line_indent(LineKind::Normal, self.options)
                        + self.case_body_indent_extra(LineKind::Normal))
                        * self.options.indent_width;
                    (code.ends_with(';')
                        && unmatched_open_paren_column(code).is_none()
                        && !after_braceless_header
                        && previous_indent <= body_indent)
                        .then_some(previous_indent)
                });
                let previous_operator_indent = previous_line.and_then(|line| {
                    let head = line.trim_end();
                    (head_ends_binary_operator(head) || head_ends_assignment_operator(head))
                        .then(|| self.stack_state.current_continuation_indent_spaces())
                        .flatten()
                });
                let previous_opening_body_indent = previous_line.and_then(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    code.ends_with('{').then(|| {
                        let body_indent = (self.state.line_indent(LineKind::Normal, self.options)
                            + self.case_body_indent_extra(LineKind::Normal))
                            * self.options.indent_width;
                        let trimmed = code.trim_start();
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
                    unmatched_open_paren_column(line.trim_end()).map(|column| {
                        self.stack_state
                            .current_continuation_indent_spaces()
                            .unwrap_or(column + 1)
                    })
                });
                previous_statement_indent
                    .or(previous_operator_indent)
                    .or(previous_opening_body_indent)
                    .or(previous_paren_indent)
                    .or(definition_header_comment_indent)
                    .or_else(|| self.stack_state.current_continuation_indent_spaces())
                    .or_else(|| self.active_body_comment_indent_spaces())
                    .or_else(|| {
                        (self.state.statement_depth() > 0)
                            .then(|| self.current_line_indent_spaces())
                    })
                    .or_else(|| {
                        previous_line.and_then(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            code.ends_with('{').then(|| {
                                let leading = leading_visual_width(line, self.options.tab_width);
                                if !self.options.indent_namespaces
                                    && matches!(
                                        self.stack_state.brace_type_stack.last(),
                                        Some(
                                            FormatterBraceType::Namespace
                                                | FormatterBraceType::Extern
                                        )
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
                            && self.state.indent() > 0)
                            .then(|| self.state.indent() * self.options.indent_width)
                    })
            });
        let lambda_parameter_comment_indent = (kind == CommentKind::Block
            && self.current.trim().is_empty())
        .then(|| {
            let frame = self
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
        .flatten();
        if let Some(spaces) = lambda_parameter_comment_indent {
            self.continuation_indent.input_line_continuation_indent =
                Some(ContinuationIndent::Spaces(spaces));
        }
        if comment.contains("*INDENT-OFF*") && !self.line_state.indent_off_follows_code {
            self.finish_line();
            if self.token_input.token_begins_source_line && self.token_input.token_source_column > 0
            {
                self.current
                    .push_str(&" ".repeat(self.token_input.token_source_column));
            }
            self.current.push_str(comment.trim_end());
            self.disabled_formatting = Some(DisabledFormattingState::capture(self));
            self.formatting_disabled = true;
            return;
        }

        if self.should_break_header_before_comment() {
            self.finish_line();
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.command_state.header_broken_before_comment = true;
        }

        if comment.contains('\n') {
            if kind == CommentKind::Line {
                self.push_continued_line_comment(comment);
                return;
            }
            self.push_multiline_block_comment(comment, block_comment_continuation_indent);
            return;
        }

        if definition_header_comment_indent.is_some() && self.current.trim().is_empty() {
            let prefix = self.previous_output_indent_prefix();
            self.clear_current();
            self.current.push_str(&prefix);
            self.current_is_preindented = true;
        }

        if kind == CommentKind::Block
            && self.current.trim_end().ends_with('{')
            && self
                .stack_state
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| {
                    matches!(
                        brace_type,
                        FormatterBraceType::Command
                            | FormatterBraceType::Definition
                            | FormatterBraceType::NonStatement
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
            && self.current.trim().is_empty()
            && !self.token_input.token_begins_source_line
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| !whitespace.contains('\n'))
            && self
                .output
                .last()
                .is_some_and(|line| line.trim_end().ends_with("*/"))
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
                line.push_str(comment.trim_end());
            }
            self.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return;
        }

        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && self.frame_stack.active_brace().is_none()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    line[..trailing_comment_split_limit(line)]
                        .trim_end()
                        .ends_with(';')
                })
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            self.current_is_preindented = true;
            self.stack_state.clear_continuation_indents();
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start() == "else"
                || previous_code.trim_start().ends_with("} else")
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                let prefix = self
                    .options
                    .continuation_indent_prefix(spaces / self.options.indent_width.max(1), spaces);
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.options.brace_style == BraceStyle::None
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with('{') && previous_code.trim_start().starts_with("} else") {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.line_state.trailing_comment_columns.clear();
                self.token_input.previous_input_whitespace = Some(String::new());
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.options.brace_style == BraceStyle::None
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = previous_code.trim_start();
            if previous_code.ends_with('{')
                && (comment_starts_header_word(previous_trimmed, "if")
                    || comment_starts_header_word(previous_trimmed, "while")
                    || comment_starts_header_word(previous_trimmed, "for")
                    || previous_trimmed.starts_with("else if")
                    || previous_trimmed.starts_with("} else"))
                && self.output.iter().rev().take(64).any(|line| {
                    let trimmed = line[..trailing_comment_split_limit(line)]
                        .trim_end()
                        .trim_start();
                    trimmed == "else" || trimmed.ends_with("} else")
                })
                && self
                    .output
                    .iter()
                    .rev()
                    .take_while(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        !(leading_visual_width(line, self.options.tab_width) == 0
                            && code.ends_with('{')
                            && !code.trim_start().starts_with('#'))
                    })
                    .take(64)
                    .any(|line| line.trim_start().starts_with('#'))
            {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && self.preprocessor.split_else.extra_indent
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start() == "}" {
                let spaces = leading_visual_width(previous, self.options.tab_width);
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if preprocessor_directive(previous_code.trim_start())
                .is_some_and(|directive| directive == "else" || directive.starts_with("elif"))
            {
                let spaces = (self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width;
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && let Some(spaces) = self.preprocessor_split_braceless_comment_indent_spaces()
        {
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if self.options.brace_style == BraceStyle::None
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
            && preprocessor_directive(previous.trim_start()) == Some("endif")
            && let Some(header) = self
                .output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != previous.as_str())
                .skip(1)
                .find(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
        {
            let trimmed = header[..trailing_comment_split_limit(header)]
                .trim_end()
                .trim_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let spaces = leading_visual_width(header, self.options.tab_width)
                    + self.options.indent_width;
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && (self.state.line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal))
                == 0
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| preprocessor_directive(line.trim_start()) == Some("endif"))
        {
            self.current_is_preindented = true;
            self.continuation_indent.next_line_indent_spaces = Some(0);
        }
        if kind == CommentKind::Line
            && self.current.trim().is_empty()
            && let Some(spaces) = self.enum_value_comment_continuation_indent_spaces()
        {
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.trim_start().starts_with("switch") && previous_code.ends_with('{') {
                let spaces = leading_visual_width(previous, self.options.tab_width)
                    + self.options.indent_width * 2;
                self.clear_current();
                self.current.push_str(&" ".repeat(spaces));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| post_closing_declaration_owns_comment(line))
        {
            self.current_is_preindented = true;
            self.continuation_indent.next_line_indent_spaces = Some(0);
        }
        if kind == CommentKind::Line
            && self.current.trim().is_empty()
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
            && previous.trim_start().starts_with('?')
        {
            let spaces = leading_visual_width(previous, self.options.tab_width);
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && !comment.contains('\n')
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if previous_code.ends_with(',') {
                let spaces = leading_visual_width(previous, self.options.tab_width);
                let structural_level = self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal);
                let prefix = self
                    .options
                    .continuation_indent_prefix(structural_level, spaces);
                self.clear_current();
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && !comment.contains('\n')
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if let Some(spaces) =
                self.block_comment_call_opener_indent_spaces(code, previous_indent)
            {
                self.clear_current();
                let prefix = self
                    .options
                    .continuation_indent_prefix(self.continuation_base_indent(), spaces);
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            }
        }
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && !comment.contains('\n')
            && (self.preprocessor.split_else.extra_indent || self.preprocessor_split_else_active())
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            if code.ends_with(';')
                && previous_indent > self.current_line_indent_spaces() + self.options.indent_width
            {
                self.clear_current();
                self.current.push_str(&" ".repeat(previous_indent));
                self.current_is_preindented = true;
                self.continuation_indent.next_line_indent_spaces = Some(previous_indent);
            }
        }
        let follows_objc_interface = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .is_some_and(|line| line.trim_start().starts_with("@interface"));
        if kind == CommentKind::Block
            && self.current.trim().is_empty()
            && !comment.contains('\n')
            && !self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| post_closing_declaration_owns_comment(line))
            && let Some(spaces) = case_label_comment_indent
                .or(control_header_comment_indent)
                .or(user_label_comment_indent)
                .or(lambda_parameter_comment_indent)
                .or_else(|| {
                    self.options
                        .indent_after_parens
                        .then_some(block_comment_continuation_indent)
                        .flatten()
                })
                .or_else(|| {
                    self.output
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .and_then(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            let previous_indent =
                                leading_visual_width(line, self.options.tab_width);
                            self.block_comment_call_opener_indent_spaces(code, previous_indent)
                                .or_else(|| {
                                    unmatched_open_paren_column(code).map(|column| {
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
                .or(block_comment_continuation_indent)
                .or_else(|| self.current_inline_array_column())
                .or_else(|| {
                    self.output
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .and_then(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            code.ends_with(',')
                                .then(|| leading_visual_width(line, self.options.tab_width))
                        })
                })
                .or_else(|| {
                    self.output
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .and_then(|line| {
                            let code = line[..trailing_comment_split_limit(line)].trim_end();
                            let trimmed = code.trim_start();
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
            if let Some((_, delimiter)) = self.frame_stack.active_delimiter_mut()
                && delimiter.opener_output_line < output_len
            {
                delimiter.continuation_indent_column = Some(spaces);
            }
            if self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .and_then(|line| unmatched_open_paren_column(line.trim_end()))
                .is_some()
            {
                self.continuation_indent.clear_continuation_after_line = Some(spaces);
            }
            let current_indent = leading_visual_width(&self.current, self.options.tab_width);
            if current_indent != spaces {
                let structural_level = if follows_objc_interface {
                    1
                } else {
                    self.state.line_indent(LineKind::Normal, self.options)
                        + self.case_body_indent_extra(LineKind::Normal)
                };
                let prefix = self
                    .options
                    .continuation_indent_prefix(structural_level, spaces);
                self.clear_current();
                self.current.push_str(&prefix);
                self.current_is_preindented = true;
            }
            self.continuation_indent.next_line_indent_spaces =
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
                self.continuation_indent.next_input_line_continuation_indent =
                    Some(ContinuationIndent::Spaces(spaces));
            }
        }
        if kind == CommentKind::Line
            && standalone_line_comment
            && (self.line_state.column1_line_comment
                || self.state.current_preproc_indent().is_some())
            && self.options.indent_col1_comments
            && line_comment_continuation_indent.is_none()
            && line_comment_stream_chain_indent.is_none()
            && self
                .enum_value_comment_continuation_indent_spaces()
                .is_none()
            && !self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim_start().starts_with('?'))
        {
            let preprocessor_indent = self
                .split_else_preprocessor_branch_body_indent_spaces()
                .or_else(|| {
                    self.state.current_preproc_indent().map(|indent| {
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
                        && self.frame_stack.active_brace().is_some_and(|frame| {
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
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(spaces);
        }
        if self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && self.token_input.token_source_column == 0
            && !self.options.indent_col1_comments
            && self.state.current_preproc_indent().is_some()
            && self.preprocessor_region(false) == PreprocessorRegion::TopLevel
        {
            self.clear_current();
            self.current_is_preindented = true;
        }
        if self.previous == PreviousToken::OpenParen {
            let comment_ends_line = kind == CommentKind::Line
                || !self.token_input.has_next_meaningful_token
                || self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.contains('\n'));
            let outside_pad =
                self.options.pad_parens_outside || self.options.pad_first_paren_outside;
            if comment_ends_line && outside_pad && self.options.unpad_parens {
                self.trim_current_end_horizontal_space();
                self.ensure_space();
            } else if self.options.pad_parens_inside {
                self.pad_inside_paren_space();
            } else if comment_ends_line && outside_pad {
                self.emit_source_space_or_ensure();
            } else if self.options.unpad_parens {
                self.trim_current_end_horizontal_space();
            } else {
                self.emit_source_space();
            }
        } else if !self.current.trim().is_empty() {
            self.pad_before_trailing_comment(kind, comment);
        }
        let comment_text = if kind == CommentKind::Line && comment.trim_end().ends_with('\\') {
            comment
        } else {
            comment.trim_end()
        };
        if let Some(spaces) = line_comment_stream_chain_indent {
            self.clear_current();
            self.current.push_str(&" ".repeat(spaces));
            self.current_is_preindented = true;
        }
        if let Some(spaces) = ternary_branch_comment_indent_spaces {
            let structural_level = self.state.line_indent(LineKind::Normal, self.options)
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
                let after_post_closing_declaration = self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| post_closing_declaration_owns_comment(line));
                self.finish_line();
                if after_post_closing_declaration {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces =
                        Some(self.options.indent_width);
                }
                let column = self.current_inline_array_column().or_else(|| {
                    self.output
                        .last()
                        .map(|line| leading_visual_width(line, self.options.tab_width))
                        .filter(|spaces| *spaces > 0)
                });
                if let Some(column) = column {
                    self.continuation_indent.next_line_indent = None;
                    self.continuation_indent.next_line_indent_spaces = Some(
                        column
                            + self.line_adjuster.total_case_unindent_depth()
                                * self.options.indent_width,
                    );
                }
                if let Some(spaces) = open_paren_comment_indent {
                    self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
                    self.stack_state.push_continuation_indent_spaces_raw(spaces);
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
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        if kind == CommentKind::Line {
            self.finish_line();
            if let Some(spaces) =
                line_comment_stream_chain_indent.or(ternary_branch_comment_indent_spaces)
            {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = Some(spaces);
            } else if let Some(indent) = line_comment_continuation_indent {
                self.set_next_continuation_indent(indent);
            }
        }
        if function_try_initializer_comment {
            self.continuation_indent.next_line_indent = Some(self.state.indent() + 1);
            self.continuation_indent.next_line_indent_spaces = None;
            self.previous_was_newline = true;
        }
    }

    fn comment_after_open_paren_indent_spaces(&self) -> usize {
        let base = self
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
                let catch_header = self.command_state.current_header.as_deref() == Some("catch")
                    || self
                        .current
                        .trim_start()
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
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !starts_with_chain_operator(previous_code.trim_start())
            || self.frame_stack.active_delimiter().is_none()
            || self
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
        if self.output.iter().rev().take(16).any(|line| {
            preprocessor_directive(line.trim_start())
                .is_some_and(|directive| matches!(directive, "if" | "ifdef" | "ifndef"))
        }) {
            return Some(previous_indent + self.options.indent_width);
        }
        if let Some(spaces) = self
            .stack_state
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
        let left = code[..assignment].trim();
        !left.is_empty()
            && self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
                .and_then(|line| {
                    let previous = line[..trailing_comment_split_limit(line)].trim_end();
                    find_assignment_operator(previous)
                        .map(|(assignment, _)| previous[..assignment].trim())
                })
                .is_some_and(|previous_left| previous_left == left)
    }

    fn preprocessor_split_braceless_comment_indent_spaces(&self) -> Option<usize> {
        if !self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?
            .trim_start()
            .starts_with("#endif")
        {
            return None;
        }
        for line in self.output.iter().rev().skip(1) {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
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
        if !self.current.trim().is_empty() {
            self.pad_before_trailing_comment(CommentKind::Line, first);
        }
        let output_column = leading_visual_width(&self.current, self.options.tab_width.max(1));
        self.record_comment_frame(CommentKind::Line, output_column, true);
        self.current.push_str(first.trim_end());
        self.finish_line();
        for line in lines {
            self.push_output_line(line.trim(), self.state.indent());
        }
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
    }

    fn push_multiline_block_comment(&mut self, comment: &str, opener_indent: Option<usize>) {
        let interrupted_header = self.command_state.current_header.clone();
        let open_paren_comment_indent = (self.previous == PreviousToken::OpenParen)
            .then(|| self.comment_after_open_paren_indent_spaces());
        if !self.current.trim().is_empty() {
            let mut lines = comment.lines().peekable();
            if let Some(first) = lines.next() {
                if self.previous == PreviousToken::OpenParen {
                    if self.options.pad_parens_inside {
                        self.pad_inside_paren_space();
                    } else {
                        self.emit_source_space();
                    }
                } else {
                    self.pad_before_trailing_comment(CommentKind::Block, first);
                }
                let output_column =
                    leading_visual_width(&self.current, self.options.tab_width.max(1));
                self.record_comment_frame(CommentKind::Block, output_column, true);
                self.current.push_str(first.trim_end());
                if lines.peek().is_some() {
                    self.finish_line();
                }
                let last_line_has_trailing_token = self.token_input.has_next_meaningful_token;
                while let Some(line) = lines.next() {
                    let shifted = if self.options.strip_comment_prefix {
                        let opener_prefix = " ".repeat(self.current_line_indent_spaces());
                        self.strip_block_comment_line(
                            line,
                            false,
                            &opener_prefix,
                            self.token_input.token_source_column,
                        )
                    } else {
                        line.trim_end().to_string()
                    };
                    let is_last_line = lines.peek().is_none();
                    let last_line_starts_with_star = line.trim_start().starts_with('*');
                    let keep_line_open = is_last_line
                        && (last_line_has_trailing_token || !last_line_starts_with_star);
                    if keep_line_open {
                        self.current.push_str(&shifted);
                        self.current_is_preindented = true;
                    } else {
                        self.push_raw_comment_output_line(shifted);
                    }
                }
            }
            self.attach_source_space_after_block_comment();
            if self.command_state.current_header.is_none() {
                self.command_state.current_header = interrupted_header;
            }
            if let Some(spaces) = open_paren_comment_indent {
                self.set_next_continuation_indent(ContinuationIndent::Spaces(spaces));
                self.stack_state.push_continuation_indent_spaces_raw(spaces);
            }
            self.previous = PreviousToken::Other;
            return;
        }

        self.finish_line();
        if self.command_state.current_header.is_none() {
            self.command_state.current_header = interrupted_header;
        }
        let tab_width = self.options.tab_width.max(1);
        let unindented_namespace_run_in_comment = self.token_input.token_line_opens_with_brace
            && !self.token_input.token_begins_source_line
            && !self.options.indent_namespaces
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(FormatterBraceType::Namespace | FormatterBraceType::Extern)
            )
            && self.output.last().is_some_and(|line| line.trim() == "{");
        let case_comment_unindent = if !self.options.indent_switches
            && self
                .stack_state
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
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map_or(0, |line| leading_visual_width(line, self.options.tab_width));
            " ".repeat(spaces)
        } else if self.token_input.token_line_opens_with_brace
            && !self.token_input.token_begins_source_line
        {
            let case_brace_extra = if self.output.last().is_some_and(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("case ")
                    || trimmed.starts_with("default:") && trimmed.contains('{')
            }) {
                self.options.indent_width
            } else {
                0
            };
            let spaces = self.token_input.token_source_column
                + self.line_adjuster.pending_case_unindent() * self.options.indent_width
                + case_brace_extra;
            self.options.continuation_indent_prefix(0, spaces)
        } else if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            && {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.trim_start().starts_with("switch") && code.ends_with('{')
            }
        {
            " ".repeat(leading_visual_width(previous, self.options.tab_width))
        } else if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            && {
                let code = previous[..trailing_comment_split_limit(previous)].trim_end();
                code.ends_with('{')
                    && (code.trim_start().starts_with("case ")
                        || code.trim_start().starts_with("default:"))
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
                        .iter()
                        .rev()
                        .find(|line| !line.trim().is_empty())
                        .is_some_and(|line| line.trim_start().starts_with("@interface"))
                    {
                        self.options.indent_prefix(1)
                    } else {
                        self.options.indent_prefix(
                            self.continuation_indent
                                .next_line_indent
                                .unwrap_or_else(|| self.continuation_base_indent()),
                        )
                    }
                }
            }
        };
        if let Some(previous) = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if code.ends_with('{')
                && (code.trim_start().starts_with("case ")
                    || code.trim_start().starts_with("default:"))
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
            .stack_state
            .brace_header_stack
            .last()
            .is_some_and(|header| header.as_deref() == Some("case"))
        {
            let spaces = self.current_line_indent_spaces();
            if leading_visual_width(&opener_prefix, tab_width) < spaces {
                opener_prefix = " ".repeat(spaces);
            }
        }
        if self
            .stack_state
            .brace_header_stack
            .last()
            .is_some_and(|header| header.as_deref() == Some("case"))
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
            && previous.trim() == "}"
        {
            let previous_indent = leading_visual_width(previous, tab_width)
                + self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
            let opener_indent = leading_visual_width(&opener_prefix, tab_width);
            if previous_indent > opener_indent {
                opener_prefix = " ".repeat(previous_indent);
            }
        }
        let opener_output_col = leading_visual_width(&opener_prefix, tab_width);
        self.record_comment_frame(CommentKind::Block, opener_output_col, true);
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
            && self.command_state.current_header.is_none()
            && !self.options.remove_braces
            && match self.stack_state.brace_type_stack.last() {
                Some(FormatterBraceType::Command) => {
                    self.options.brace_style == BraceStyle::OneTrueBrace
                }
                Some(FormatterBraceType::Array | FormatterBraceType::Init) => true,
                _ => false,
            }
            && self.output.last().is_some_and(|line| line.trim() == "{");
        let mut lines = comment.lines().enumerate().peekable();
        while let Some((index, line)) = lines.next() {
            if index == 0 && run_in_opener {
                let gap = self
                    .token_input
                    .previous_input_whitespace
                    .clone()
                    .filter(|ws| !ws.is_empty() && !ws.contains('\n'))
                    .unwrap_or_else(|| " ".to_string());
                if let Some(brace_line) = self.output.last_mut() {
                    brace_line.push_str(&gap);
                    brace_line.push_str(line.trim_end());
                }
                continue;
            }
            let formatted = if self.options.strip_comment_prefix {
                self.strip_block_comment_line(line, index == 0, &opener_prefix, trim_amount)
            } else if index == 0 {
                format!("{opener_prefix}{}", line.trim_end())
            } else {
                if line.trim().is_empty() {
                    String::new()
                } else {
                    let kept = drop_leading_columns(line, trim_amount, tab_width);
                    let trimmed_kept = kept.trim_start();
                    let is_last_line = lines.peek().is_none();
                    let decorative_closer = is_decorative_block_comment_closer(trimmed_kept);
                    let source_closer_leading = leading_visual_width(line, tab_width);
                    let closer_prefix = if decorative_closer && is_last_line {
                        self.output
                            .last()
                            .or(self.previous_pre_adjust_line.as_ref())
                            .and_then(|previous| {
                                let leading =
                                    leading_visual_width(previous, self.options.tab_width);
                                (previous.trim_start().starts_with('*')
                                    && if self.token_input.token_line_opens_with_brace {
                                        leading >= opener_output_col
                                    } else {
                                        source_closer_leading < trim_amount
                                            && leading == source_closer_leading
                                    })
                                .then(|| {
                                    previous[..previous.len() - previous.trim_start().len()]
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
                        && trim_amount > opener_output_col
                        && kept.starts_with('*')
                        && leading_visual_width(line, tab_width) < trim_amount
                        && (!decorative_closer || (!is_last_line && index > 2));
                    if let Some(prefix) = closer_prefix {
                        format!("{}{}", prefix, trimmed_kept.trim_end())
                    } else if unindented_namespace_run_in_comment {
                        format!("{opener_prefix}{}", kept.trim_end())
                    } else if self.token_input.token_line_opens_with_brace {
                        let source_line_col = leading_visual_width(line, tab_width);
                        let body_offset = if decorative_closer && is_last_line {
                            0
                        } else if trimmed_kept.starts_with('*') {
                            1
                        } else {
                            source_line_col.saturating_sub(trim_amount)
                        };
                        let indent = if self.options.indent_classes
                            && matches!(
                                self.stack_state.brace_type_stack.last(),
                                Some(FormatterBraceType::Class)
                            ) {
                            self.state.indent().saturating_sub(1)
                        } else {
                            self.state.indent()
                        };
                        let merged_comment_col = self.frame_stack.active_brace().map_or_else(
                            || ContinuationIndent::Level(indent).columns(self.options.indent_width),
                            |frame| frame.body_indent_column.max(opener_output_col),
                        );
                        let target = merged_comment_col + body_offset;
                        format!(
                            "{}{}",
                            self.options.continuation_indent_prefix(
                                merged_comment_col / self.options.indent_width.max(1),
                                target,
                            ),
                            trimmed_kept.trim_end()
                        )
                    } else if star_shift {
                        format!("{opener_prefix} {}", kept.trim_end())
                    } else {
                        format!("{opener_prefix}{}", kept.trim_end())
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
        if case_comment_unindent > 0 && self.current_is_preindented {
            self.finish_line();
            self.continuation_indent.next_line_indent_spaces =
                Some(opener_output_col.saturating_sub(case_comment_unindent));
        } else if case_comment_unindent > 0 {
            self.continuation_indent.next_line_indent_spaces =
                Some(opener_output_col.saturating_sub(case_comment_unindent));
        }
        self.attach_source_space_after_block_comment();
        self.previous = PreviousToken::Other;
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
            && !self.current.trim().is_empty()
            && self
                .command_state
                .current_header
                .as_deref()
                .is_some_and(is_add_braces_header)
    }

    fn enum_value_comment_continuation_indent_spaces(&self) -> Option<usize> {
        if !matches!(
            self.stack_state.brace_type_stack.last(),
            Some(FormatterBraceType::Enum)
        ) {
            return None;
        }
        let previous = self.output.last().filter(|line| !line.trim().is_empty())?;
        let (code, _) = previous.split_once("//")?;
        if code.trim_end().ends_with(',') {
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

    fn pad_before_trailing_comment(&mut self, kind: CommentKind, comment: &str) {
        let had_formatter_space = (self.current.ends_with(' ') || self.current.ends_with('\t'))
            && matches!(
                self.current.trim_end().chars().next_back(),
                Some('*' | '&' | '^')
            );
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
        if self.line_state.is_multi_statement_line {
            if gap.is_empty() {
                if kind != CommentKind::Block || comment.contains("NOPAD") {
                    self.ensure_space();
                }
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        let case_body_split_from_label = self.output.last().is_some_and(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("case ") || trimmed == "default:"
        }) && self.line_state.passed_colon
            && self.line_state.passed_semicolon;
        if case_body_split_from_label {
            self.current.push_str(&gap);
            return;
        }
        if kind == CommentKind::Block
            && (self.token_input.token_followed_by_line_comment_on_line
                || self.token_input.next_token_is_line_comment
                || self.line_state.trailing_comment_columns.len() > 1)
            && self.current.contains('}')
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line && self.current.trim_end().ends_with("*/") {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line
            && self.line_state.passed_semicolon
            && self.line_state.trailing_comment_columns.is_empty()
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        if kind == CommentKind::Line
            && self.current.contains('#')
            && !self.current.trim_start().starts_with('#')
        {
            if gap.is_empty() {
                self.ensure_space();
            } else {
                self.current.push_str(&gap);
            }
            return;
        }
        let target_column = (!self.line_state.trailing_comment_columns.is_empty())
            .then(|| self.line_state.trailing_comment_columns.remove(0));
        let trimmed_code = self.current.trim();
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
            let gap = self.initializer_brace_line_comment_gap(&self.current);
            self.current.push_str(&gap);
            return;
        }
        if kind == CommentKind::Block
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
        {
            self.current.push_str(&gap);
            return;
        }
        let Some(target_column) = target_column else {
            if gap.is_empty() {
                let keeps_adjacent_comment = self.previous == PreviousToken::Comma
                    || self.previous == PreviousToken::Operator
                        && (!self.options.pad_operators
                            || self.line_state.operator_padding_disabled);
                if kind != CommentKind::Block
                    && !self.current.trim_end().ends_with("*/")
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
        if kind == CommentKind::Block && gap.is_empty() {
            return;
        }
        let out_indent = self
            .current
            .chars()
            .take_while(|ch| ch.is_whitespace())
            .count();
        let code_len = self.current_char_len() - out_indent;
        let gap_chars = gap.chars().count();
        if kind == CommentKind::Line && gap_chars >= self.options.indent_width * 2 {
            self.current.push_str(&gap);
            return;
        }
        let space_pad = (code_len + gap_chars) as isize - target_column as isize;
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

    fn strip_block_comment_line(
        &self,
        line: &str,
        is_opener: bool,
        prefix: &str,
        opener_source_col: usize,
    ) -> String {
        let indent_len = self.options.indent_width;
        let tab_width = self.options.tab_width.max(1);
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
                    _ => return format!("{prefix}{}", line.trim_end()),
                }
            }
            let content_col = visual_column_at(&chars, content_start, tab_width);
            let insert = indent_len.saturating_sub(content_col);
            let head: String = chars[..content_start].iter().collect();
            let tail: String = chars[content_start..].iter().collect();
            return format!("{prefix}{head}{}{}", " ".repeat(insert), tail.trim_end());
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
                    visual_column_at(&chars, first, tab_width).saturating_sub(opener_source_col);
                let content: String = chars[first..].iter().collect();
                return format!("{prefix}{}{}", " ".repeat(rel), content.trim_end());
            }
            let rel = visual_column_at(&chars, second, tab_width)
                .saturating_sub(opener_source_col)
                .max(indent_len);
            let mut content = chars[second..]
                .iter()
                .collect::<String>()
                .trim_end()
                .to_string();
            if content.ends_with('*') {
                content.pop();
                content = content.trim_end().to_string();
            }
            return format!("{prefix}{}{content}", " ".repeat(rel));
        }
        let rel = visual_column_at(&chars, first, tab_width)
            .saturating_sub(opener_source_col)
            .max(indent_len);
        let content: String = chars[first..].iter().collect();
        format!("{prefix}{}{}", " ".repeat(rel), content.trim_end())
    }
}

fn is_decorative_block_comment_closer(line: &str) -> bool {
    line.ends_with("*/") && line.chars().all(|ch| matches!(ch, '*' | '/'))
}

pub(super) fn trailing_comment_columns(tokens: &[Token]) -> Vec<usize> {
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
    for token in tokens {
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
                    if open_brace_depth == 0 || (open_brace_depth == 1 && saw_closing_header) {
                        columns.push(column.saturating_sub(leading_indent));
                    } else if code_after_open_brace {
                        columns.push(column.saturating_sub(segment_start_column));
                    }
                }
                column += comment.chars().count();
                seen_token = true;
            }
            Token::Word(word) => {
                if saw_closing_brace && is_break_blocks_closing_header(word) {
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
                code_after_open_brace = false;
                pending_segment_start = true;
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
