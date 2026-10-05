use crate::config::BraceStyle;
use crate::formatter::constructs::headers::{
    is_braceless_header_line, line_is_control_body_header, starts_header_word,
};
use crate::formatter::engine::FormatEngine;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::BraceType;
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    is_comment_line, is_comment_only_line, preprocessor_directive,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::literals::starts_string_literal_token;
use crate::formatter::tokens::operators::starts_with_chain_operator;
use crate::source::lex::is_identifier_start;

pub(crate) struct SplitElseLineStart {
    extra_levels: usize,
    trigger_is_current_output: bool,
    extra_indent_active: bool,
}

pub(crate) struct StructuralSplitElseBodyContext {
    structural_chain: bool,
    body_indent_spaces: usize,
    split_else_chain: bool,
    recent_preprocessor: bool,
    recent_adjacent_string_call_body: bool,
    opening_is_else: bool,
    opening_is_control: bool,
    case_unindent_spaces: usize,
}

pub(crate) struct RecentSplitElseChainContext {
    chain_active: bool,
    interrupted_header_active: bool,
}

pub(crate) struct SplitElsePreprocessorContext {
    emitted_region_active: bool,
    layout_active: bool,
}

impl StructuralSplitElseBodyContext {
    pub(crate) fn structural_chain(&self) -> bool {
        self.structural_chain
    }

    pub(crate) fn body_indent_spaces(&self) -> usize {
        self.body_indent_spaces
    }
}

impl RecentSplitElseChainContext {
    pub(crate) fn chain_active(&self) -> bool {
        self.chain_active
    }

    pub(crate) fn interrupted_header_active(&self) -> bool {
        self.interrupted_header_active
    }
}

impl SplitElsePreprocessorContext {
    pub(crate) fn emitted_region_active(&self) -> bool {
        self.emitted_region_active
    }

    pub(crate) fn layout_active(&self) -> bool {
        self.layout_active
    }
}

impl SplitElseLineStart {
    pub(crate) fn extra_levels(&self) -> usize {
        self.extra_levels
    }

    pub(crate) fn adjust_brace_level(&self, level: usize) -> usize {
        if self.trigger_is_current_output && self.extra_indent_active {
            level + self.extra_levels.saturating_sub(1)
        } else {
            level
        }
    }

    pub(crate) fn adjust_pending_level(&self, level: usize, included_base_indent: usize) -> usize {
        let included_extra = level
            .saturating_sub(included_base_indent)
            .min(self.extra_levels);
        let explicit_extra = if self.trigger_is_current_output {
            self.extra_levels.saturating_sub(1)
        } else {
            self.extra_levels.saturating_sub(included_extra)
        };
        level + explicit_extra
    }
}

fn embedded_branch_separator(code: &str) -> bool {
    if !code.contains('#') {
        return false;
    }
    let trimmed = code.trimmed_start();
    if trimmed.starts_with('#') || code.contains("#if") {
        return false;
    }
    ["#else", "#elif"].iter().any(|marker| {
        code.find(marker)
            .is_some_and(|index| code[index + marker.len()..].trimmed().is_empty())
    })
}

impl FormatEngine<'_> {
    pub(crate) fn normalize_ready_preprocessor_line(&self, line: String) -> String {
        if !self.preprocessor.may_have_preprocessor {
            return line;
        }
        let line_start = line.trimmed_start();
        let line = if line_start.starts_with('#') && !line_start.starts_with("#define") {
            line.trimmed_end().to_string()
        } else {
            line
        };
        let line_start = line.trimmed_start();
        if line_start.starts_with("#if") && line.contains("#else") {
            line_start.to_string()
        } else {
            line
        }
    }

    pub(crate) fn split_else_body_indent_active(&self) -> bool {
        self.preprocessor.split_else.extra_indent
    }

    pub(crate) fn split_else_braceless_body_active(&self) -> bool {
        self.preprocessor.split_else.extra_indent && self.preprocessor.split_else.body_braceless
    }

    pub(crate) fn split_else_line_layout_active(&self) -> bool {
        self.preprocessor_split_else_active()
            || self.preprocessor.split_else.trigger_output_len.is_some()
    }

    pub(crate) fn clear_split_else_closing_state_on_empty_line(&mut self) {
        if self.preprocessor.split_else.extra_levels == 0 {
            self.preprocessor.split_else.clear_pending_after_brace = false;
            self.preprocessor.split_else.closing_brace_has_else = false;
        }
    }

    pub(crate) fn take_split_else_comment_body_indent_spaces(
        &mut self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !self.preprocessor.split_else.extra_indent
            || !self.preprocessor.split_else.body_braceless
            || line_start.starts_with("//")
        {
            return None;
        }
        let spaces = self
            .preprocessor
            .split_else
            .comment_body_indent_spaces
            .take()?;
        if line_is_control_body_header(line_start) {
            self.preprocessor.split_else.body_braceless = false;
            None
        } else {
            Some(spaces + self.options.indent_width)
        }
    }

    pub(crate) fn record_split_else_comment_body_indent(
        &mut self,
        line: &str,
        output_spaces: usize,
    ) {
        if line.trimmed_start().starts_with("//")
            && self.preprocessor.split_else.extra_indent
            && self.preprocessor.split_else.body_braceless
        {
            self.preprocessor.split_else.comment_body_indent_spaces = Some(output_spaces);
        }
    }

    pub(crate) fn nonconditional_directive_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        normal_indent: usize,
    ) -> Option<usize> {
        let current = line.trimmed_start();
        if !current.chars().next().is_some_and(is_identifier_start)
            || self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.layout.indentation.indent() != 0
            || self.token_input.token_source_line_indent != 0
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?.trimmed_start();
        if !previous.starts_with('#') {
            return None;
        }
        let directive = preprocessor_directive(previous)?;
        if is_conditional_preprocessor(directive)
            || ["if", "ifdef", "ifndef", "elif", "else", "endif"]
                .iter()
                .any(|prefix| directive.starts_with(prefix))
        {
            return None;
        }
        Some(normal_indent * self.options.indent_width)
    }

    pub(crate) fn none_style_split_else_blank_gap_sibling_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: usize,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line_start.starts_with(['#', '{', '}'])
            || is_comment_line(line_start)
        {
            return None;
        }
        let after_blank = self.output.last().is_some_and(|line| line.is_empty())
            || self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| whitespace.matches('\n').count() > 1);
        if !after_blank {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        let mut spaces = None;
        if previous_trimmed == "else" || previous_trimmed.ends_with("} else") {
            let target =
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width;
            if current_spaces < target {
                spaces = Some(target);
            }
        }
        if preprocessor_directive(previous_trimmed) == Some("endif")
            && let Some(before_preprocessor) =
                self.output.scoped().iter().rev().skip(1).find(|line| {
                    let trimmed = line.trimmed_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
        {
            let trimmed = self
                .output
                .code_of(before_preprocessor)
                .trimmed_end()
                .trimmed_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let extra = usize::from(trimmed == "else") * self.options.indent_width;
                spaces =
                    Some(leading_visual_width(before_preprocessor, self.options.tab_width) + extra);
            }
        }
        spaces
    }

    pub(crate) fn none_style_split_else_body_indent_floor(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        split_else_state_active: bool,
        current_spaces: usize,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line_start.starts_with(['#', '{', '}'])
            || is_comment_line(line_start)
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let anchor = if preprocessor_directive(previous_code.trimmed_start()).is_some() {
            self.output.scoped().iter().rev().skip(1).find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
        } else {
            Some(previous)
        }?;
        let anchor_code = self.output.code_of(anchor).trimmed_end();
        let anchor_trimmed = anchor_code.trimmed_start();
        let anchor_is_header = starts_header_word(anchor_trimmed, "if")
            || starts_header_word(anchor_trimmed, "while")
            || starts_header_word(anchor_trimmed, "for")
            || anchor_trimmed.starts_with("else if")
            || anchor_trimmed.starts_with("} else");
        let split_header_spaces = if anchor_code.ends_with('{') && !anchor_is_header {
            self.output
                .scoped()
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != anchor.as_str())
                .skip(1)
                .filter(|line| {
                    let trimmed = line.trimmed_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
                .take(8)
                .find_map(|line| {
                    let code = self.output.code_of(line).trimmed_end();
                    let trimmed = code.trimmed_start();
                    (starts_header_word(trimmed, "if")
                        || starts_header_word(trimmed, "while")
                        || starts_header_word(trimmed, "for")
                        || trimmed.starts_with("else if")
                        || trimmed.starts_with("} else"))
                    .then_some(
                        leading_visual_width(line, self.options.tab_width)
                            + self.options.indent_width,
                    )
                })
        } else {
            None
        };
        let spaces = if let Some(spaces) = split_header_spaces {
            spaces
        } else if anchor_code.ends_with('{') && anchor_is_header {
            leading_visual_width(anchor, self.options.tab_width) + self.options.indent_width
        } else if (anchor_code.ends_with(';') && !anchor_code.ends_with("};"))
            || anchor_code.trimmed() == "}"
        {
            leading_visual_width(anchor, self.options.tab_width)
        } else {
            return None;
        };
        (current_spaces < spaces).then_some(spaces)
    }

    pub(crate) fn structural_split_else_body_context(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<StructuralSplitElseBodyContext> {
        let trimmed = line.trimmed_start();
        let needs_context = trimmed.starts_with('}')
            || trimmed.starts_with("&&")
            || trimmed.starts_with("||")
            || trimmed.starts_with(',')
            || trimmed.starts_with(");")
            || starts_string_literal_token(trimmed)
            || self.preprocessor_split_else_active();
        if line_kind != LineKind::Normal || trimmed.starts_with('#') || !needs_context {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        self.output.last_line_outside_comment()?;
        let structural_chain = self.preprocessor_split_else_active();
        let body_indent_spaces = if structural_chain {
            self.current_closing_multiline_header_indent()
                .map(|spaces| spaces + self.options.indent_width)
                .unwrap_or(open_spaces + self.options.indent_width)
        } else {
            open_spaces + self.options.indent_width
        };
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
        let recent_adjacent_string_call_body = recent_adjacent_string_call
            && self.output.scoped_range().rev().take(8).any(|index| {
                let line = &self.output[index];
                let code = self.output.code_before_comment_trimmed(index);
                self.open_paren_column_of(code).is_some()
                    && !starts_string_literal_token(code.trimmed_start())
                    && !code.ends_with(';')
                    && leading_visual_width(line, self.options.tab_width) == body_indent_spaces
            });
        let split_else_chain = structural_chain || self.output.recent_scoped_else_line(128);
        let recent_preprocessor = split_else_chain
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .take_while(|line| !line.trimmed().is_empty())
                .take(32)
                .any(|line| line.trimmed_start().starts_with('#'));
        Some(StructuralSplitElseBodyContext {
            structural_chain,
            body_indent_spaces,
            split_else_chain,
            recent_preprocessor,
            recent_adjacent_string_call_body,
            opening_is_else: open_trimmed.starts_with("} else")
                || open_trimmed.starts_with("}else"),
            opening_is_control: starts_header_word(open_trimmed, "if")
                || starts_header_word(open_trimmed, "for")
                || starts_header_word(open_trimmed, "while")
                || starts_header_word(open_trimmed, "switch"),
            case_unindent_spaces: self.layout.line_adjuster.total_case_unindent_depth()
                * self.options.indent_width,
        })
    }

    pub(crate) fn structural_split_else_ordinary_row_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_spaces: usize,
        context: &StructuralSplitElseBodyContext,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_start.starts_with(['{', '}']) {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        let body_spaces = context.body_indent_spaces;
        if context.recent_preprocessor
            && previous_code.ends_with('{')
            && current_spaces < body_spaces
        {
            return Some(body_spaces);
        }
        if context.recent_preprocessor
            && context.case_unindent_spaces == 0
            && previous_code.ends_with(';')
            && previous_spaces == body_spaces
            && previous_spaces > current_spaces
            && !line_is_control_body_header(line_start)
            && !starts_string_literal_token(line_start)
            && !is_comment_line(line_start)
        {
            return Some(previous_spaces);
        }
        if context.structural_chain
            && starts_string_literal_token(previous_code.trimmed_start())
            && previous_code.ends_with(';')
            && current_spaces < body_spaces
        {
            return Some(body_spaces);
        }
        if context.structural_chain
            && context.case_unindent_spaces == 0
            && previous_spaces == body_spaces
            && previous_code.ends_with(");")
            && current_spaces > previous_spaces
            && !line_is_control_body_header(line_start)
            && !starts_string_literal_token(line_start)
            && !is_comment_line(line_start)
        {
            return Some(previous_spaces);
        }
        if context.structural_chain
            && previous_spaces > body_spaces
            && previous_code.ends_with(");")
            && current_spaces + self.options.indent_width < body_spaces
        {
            return Some(body_spaces);
        }
        if previous_spaces == body_spaces
            && (previous_code.ends_with(';') || previous_code.trimmed() == "}")
            && (context.recent_adjacent_string_call_body
                || context.split_else_chain
                    && (line_is_control_body_header(line_start)
                        || is_comment_line(line_start)
                        || context.opening_is_else
                        || context.structural_chain
                            && current_spaces + self.options.indent_width < body_spaces
                            && context.opening_is_control
                        || starts_header_word(line_start, "if")
                        || starts_header_word(line_start, "for")
                        || starts_header_word(line_start, "while")
                        || starts_header_word(line_start, "switch")))
        {
            let target = if context.opening_is_else {
                previous_spaces
            } else {
                previous_spaces + context.case_unindent_spaces
            };
            return Some(current_spaces.max(target));
        }
        None
    }

    pub(crate) fn structural_split_else_trailing_body_indent_spaces(
        &self,
        current_spaces: usize,
        context: &StructuralSplitElseBodyContext,
    ) -> Option<usize> {
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        (context.recent_adjacent_string_call_body
            && previous_code.ends_with(");")
            && starts_string_literal_token(previous_code.trimmed_start())
            && current_spaces < context.body_indent_spaces)
            .then_some(context.body_indent_spaces)
    }

    pub(crate) fn split_else_branch_body_indent_override(
        &self,
        current_spaces: usize,
    ) -> Option<usize> {
        let spaces = self.split_else_preprocessor_branch_body_indent_spaces()?;
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        (current_spaces < spaces
            || preprocessor_directive(previous_code.trimmed_start())
                .is_some_and(|directive| directive == "else" || directive.starts_with("elif")))
        .then_some(spaces)
    }

    pub(crate) fn split_else_reduced_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        let spaces = current_spaces?;
        if !self.split_else_body_indent_active()
            || line_kind == LineKind::SwitchLabel
            || line_start.starts_with('}')
            || self.line_aligns_to_open_paren_content(line)
            || self.current_inline_array_column().is_some()
            || starts_with_chain_operator(line_start)
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment();
        if previous.is_some_and(|previous| {
            let code = self.output.code_of(previous).trimmed_end();
            let trimmed = code.trimmed_start();
            code.ends_with(';')
                && (spaces == leading_visual_width(previous, self.options.tab_width)
                    || self.layout.line_adjuster.total_case_unindent_depth() > 0
                        && (spaces
                            == leading_visual_width(previous, self.options.tab_width)
                                + self.adjusted_line_indent_delta(previous)
                            || spaces
                                == leading_visual_width(previous, self.options.tab_width)
                                    + self.layout.line_adjuster.total_case_unindent_depth()
                                        * self.options.indent_width
                            || starts_header_word(trimmed, "if")
                            || starts_header_word(trimmed, "while")
                            || starts_header_word(trimmed, "for")))
        }) || previous.is_some_and(|previous| {
            let code = self.output.code_of(previous).trimmed_end();
            is_comment_line(previous.trimmed_start()) || code.ends_with('{')
        }) || self
            .output
            .last_line_outside_comment()
            .is_some_and(|previous| self.output.code_of(previous).trimmed() == "{")
            || line.trimmed() == "{"
                && previous.is_some_and(|previous| {
                    preprocessor_directive(previous.trimmed_start())
                        .is_some_and(is_conditional_preprocessor)
                })
        {
            return None;
        }
        Some(spaces.saturating_sub(self.options.indent_width))
    }

    pub(crate) fn embedded_preprocessor_branch_body_base_spaces(&self) -> Option<usize> {
        for index in (0..self.output.len()).rev().take(8) {
            let line = &self.output[index];
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            if trimmed.is_empty() {
                continue;
            }
            if embedded_branch_separator(code) {
                return Some(
                    leading_visual_width(line, self.options.tab_width) + self.options.indent_width,
                );
            }
            if trimmed.starts_with('#') || trimmed.starts_with(['{', '}']) {
                break;
            }
        }
        None
    }

    pub(crate) fn restored_preprocessor_branch_body_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let spaces = is_preprocessor_branch_body(line).then(|| {
            self.preprocessor.branch_stack.last().and_then(|branch| {
                branch
                    .restore_body_indent
                    .then_some(branch.first_body_indent_spaces)
                    .flatten()
            })
        })??;
        // A style indenting braces sets a block's `{` a level past the body.
        Some(if self.pending_line_opens_indented_plain_block() {
            spaces + self.options.indent_width
        } else {
            spaces
        })
    }

    /// Whether the line being laid out opens a nested `{ ... }` block in a
    /// style that sets its brace a level past the body.
    fn pending_line_opens_indented_plain_block(&self) -> bool {
        self.output
            .pending_tokens()
            .and_then(|span| self.tree.groups.opened_at(span.first))
            .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Block))
            && self.should_indent_brace_line(BraceType::Command)
    }

    pub(crate) fn record_preprocessor_branch_body_indent(
        &mut self,
        line: &str,
        emitted_indent_spaces: usize,
    ) {
        if !is_preprocessor_branch_body(line) {
            return;
        }
        // A style indenting braces sets a `{` a level past its body.
        let body_indent_spaces = if line.trimmed_start().starts_with('{')
            && self.should_indent_brace_line(BraceType::Command)
        {
            emitted_indent_spaces.saturating_sub(self.options.indent_width)
        } else {
            emitted_indent_spaces
        };
        if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
            if branch.first_body_indent_spaces.is_none() {
                branch.first_body_indent_spaces = Some(body_indent_spaces);
            }
            branch.restore_body_indent = false;
        }
    }

    pub(crate) fn prepare_split_else_line_start(
        &mut self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> SplitElseLineStart {
        if line_kind == LineKind::Normal
            && line.trimmed() == "{"
            && self.preprocessor.split_else.extra_indent
            && self
                .output
                .last()
                .is_some_and(|line| line.trimmed().is_empty())
        {
            self.clear_preprocessor_split_else_indent();
        }
        self.update_preprocessor_split_else_state(line, line_kind);
        SplitElseLineStart {
            extra_levels: if line_kind == LineKind::Normal {
                self.preprocessor.split_else.extra_levels
            } else {
                0
            },
            trigger_is_current_output: self.preprocessor.split_else.trigger_output_len
                == Some(self.output.len()),
            extra_indent_active: self.preprocessor.split_else.extra_indent,
        }
    }

    pub(crate) fn end_preprocessor_split_else(&mut self) {
        self.preprocessor.split_else.reset();
    }

    fn clear_preprocessor_split_else_indent(&mut self) {
        if self
            .layout
            .frame_stack
            .active_header()
            .is_some_and(|frame| frame.header == "else")
        {
            self.layout.frame_stack.clear_header();
        }
        self.preprocessor.split_else.reset();
    }

    fn update_preprocessor_split_else_state(&mut self, line: &LineView<'_>, line_kind: LineKind) {
        if line_kind != LineKind::Normal {
            return;
        }
        let trimmed = line.trimmed();
        if self.preprocessor.split_else.extra_indent
            && self.layout.indentation.indent() == 0
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("else")
            && !trimmed.starts_with('}')
        {
            self.clear_preprocessor_split_else_indent();
        }
        // The block holding the split chain has closed.
        if self.preprocessor.split_else.extra_indent
            && self.layout.indentation.indent() < self.preprocessor.split_else.brace_indent
            && !trimmed.is_empty()
            && !trimmed.starts_with(['#', '}'])
        {
            self.clear_preprocessor_split_else_indent();
        }
        if self.preprocessor.split_else.clear_pending_after_brace && !trimmed.is_empty() {
            self.preprocessor.split_else.clear_pending_after_brace = false;
            let closing_brace_has_else = self.preprocessor.split_else.closing_brace_has_else;
            self.preprocessor.split_else.closing_brace_has_else = false;
            let continues_else_chain = trimmed.strip_prefix("else").is_some_and(|rest| {
                rest.is_empty() || rest.starts_with(|ch: char| ch.is_whitespace() || ch == '{')
            });
            if !(continues_else_chain
                || self.preprocessor.split_else.pending_body
                || trimmed.starts_with('}') && closing_brace_has_else)
            {
                self.clear_preprocessor_split_else_indent();
            }
        }
        if self.preprocessor.split_else.pending_body {
            if trimmed.starts_with('{') {
                self.preprocessor.split_else.pending_body = false;
                self.preprocessor.split_else.trigger_output_len = Some(self.output.len());
            } else if !trimmed.is_empty() {
                if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                    && base + delta == self.layout.indentation.indent()
                {
                    self.layout.indentation.exit_braceless_block();
                }
                self.preprocessor.split_else.extra_indent = true;
                self.preprocessor.split_else.extra_levels += 1;
                self.preprocessor.split_else.pending_body = false;
                self.preprocessor.split_else.body_braceless = trimmed.starts_with("//");
                self.preprocessor.split_else.brace_indent = self.layout.indentation.indent();
            }
        }
    }

    fn recent_output_has_split_else(&self, limit: usize) -> bool {
        // Only an `else` split from its body starts a split-else region.
        if self
            .current_source_token()
            .is_some_and(|token| !self.tree.statements.split_else_before(token))
        {
            return false;
        }
        self.output.recent_code_else_line(limit)
    }

    fn recent_output_has_preprocessor(&self, limit: usize) -> bool {
        self.output
            .has_hash_led_code_line_from(self.output.len().saturating_sub(limit))
    }

    pub(crate) fn commented_split_else_preprocessor_region_active(&self) -> bool {
        self.recent_output_has_split_else(64)
            && self.recent_split_else_region_has_preprocessor(64)
            && self.recent_split_else_region_has_block_comment(64)
    }

    pub(crate) fn recent_split_else_preprocessor_region_active(&self) -> bool {
        self.recent_output_has_split_else(128)
            && self.recent_split_else_region_has_preprocessor(256)
    }

    pub(crate) fn recent_split_else_output_chain_active(&self) -> bool {
        self.recent_output_has_split_else(128)
    }

    pub(crate) fn recent_split_else_operator_region_active(&self) -> bool {
        self.recent_output_has_split_else(128)
            && self.recent_split_else_region_has_preprocessor(128)
    }

    pub(crate) fn recent_split_else_logical_statement_region_active(&self) -> bool {
        self.recent_output_has_split_else(256) && self.recent_output_has_preprocessor(256)
    }

    pub(crate) fn recent_split_else_call_region_active(&self) -> bool {
        self.recent_output_has_split_else(128) && self.recent_output_has_preprocessor(256)
    }

    pub(crate) fn recent_split_else_closing_context_active(&self) -> bool {
        self.recent_output_has_split_else(256)
    }

    pub(crate) fn split_else_preprocessor_context(
        &self,
        line_start_active: bool,
    ) -> SplitElsePreprocessorContext {
        let emitted_region_active = line_start_active
            && self.recent_output_has_split_else(256)
            && self.recent_split_else_region_has_preprocessor(256);
        SplitElsePreprocessorContext {
            emitted_region_active,
            layout_active: self.split_else_line_layout_active() || emitted_region_active,
        }
    }

    /// First source token of the line being laid out, or of the last line
    /// that recorded its tokens.
    fn current_source_token(&self) -> Option<usize> {
        self.output
            .pending_tokens()
            .map(|span| span.first)
            .or_else(|| {
                (0..self.output.len())
                    .rev()
                    .find_map(|index| self.output.line_tokens(index))
                    .map(|span| span.first)
            })
    }

    /// First output line inside the function body that holds the line being
    /// laid out, from the structure tree.
    fn current_function_body_start(&self) -> Option<usize> {
        let token = self.current_source_token()?;
        let groups = &self.tree.groups;
        let body = groups
            .ancestors(groups.enclosing(token)?)
            .find(|&id| self.tree.blocks.kind(id) == Some(BlockKind::FunctionBody))?;
        Some(self.output.line_with_token(groups.get(body).open)? + 1)
    }

    pub(crate) fn recent_split_else_chain_context(
        &self,
        line_start_active: bool,
    ) -> RecentSplitElseChainContext {
        // An else chain never reaches past the function it is in.
        let recent = match self.current_function_body_start() {
            Some(start) => start.min(self.output.len())..self.output.len(),
            None => self.output.scoped_range(),
        };
        let window_start = recent.start.max(recent.end.saturating_sub(128));
        let chain_active = line_start_active
            || (self.output.may_have_else() && self.output.has_else_line_from(window_start));
        let has_preprocessor =
            self.output.may_have_hash() && self.output.has_hash_led_line_from(window_start);
        let in_split_else_body = chain_active
            && self
                .current_source_token()
                .is_some_and(|token| self.tree.statements.in_split_else_body(token));
        let follows_preprocessor_boundary = self.output.may_have_hash()
            && self
                .output
                .iter()
                .enumerate()
                .rev()
                .find(|(_, line)| !line.trimmed().is_empty())
                .is_some_and(|(previous_index, previous)| {
                    preprocessor_directive(previous.trimmed_start()).is_some_and(|directive| {
                        matches!(directive, "endif" | "else" | "if" | "ifdef" | "ifndef")
                            && (matches!(directive, "endif" | "else")
                                || self.output[..previous_index]
                                    .iter()
                                    .rev()
                                    .find(|line| !line.trimmed().is_empty())
                                    .is_some_and(|line| {
                                        let trimmed =
                                            self.output.code_of(line).trimmed_end().trimmed_start();
                                        preprocessor_directive(trimmed).is_some()
                                            || trimmed == "else"
                                            || trimmed.ends_with("} else")
                                            || trimmed.ends_with("}else")
                                            || is_comment_line(line.trimmed_start())
                                    }))
                    })
                });
        RecentSplitElseChainContext {
            chain_active,
            interrupted_header_active: chain_active
                && (line_start_active || has_preprocessor || in_split_else_body)
                && !follows_preprocessor_boundary,
        }
    }

    fn recent_split_else_region_has_preprocessor(&self, limit: usize) -> bool {
        self.recent_split_else_region_any(limit, |trimmed| trimmed.starts_with('#'))
    }

    fn recent_split_else_region_has_block_comment(&self, limit: usize) -> bool {
        self.recent_split_else_region_any(limit, |trimmed| {
            trimmed.starts_with("/*") || trimmed.starts_with('*')
        })
    }

    fn recent_split_else_region_any(
        &self,
        limit: usize,
        mut matches: impl FnMut(&str) -> bool,
    ) -> bool {
        for (checked, index) in (0..self.output.len()).rev().enumerate() {
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            if code.ends_with('{')
                && !trimmed.starts_with('#')
                && !self.output[index].starts_with([' ', '\t'])
            {
                break;
            }
            if checked >= limit {
                break;
            }
            if matches(trimmed) {
                return true;
            }
        }
        false
    }

    pub(crate) fn split_else_preprocessor_branch_body_indent_spaces(&self) -> Option<usize> {
        let active_split_else = self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.preprocessor_split_else_active();
        if active_split_else
            && let Some(frame) = self
                .layout
                .frame_stack
                .active_header()
                .filter(|frame| frame.header == "else")
        {
            return Some(frame.body_indent_spaces);
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_of(previous).trimmed_end();
        let (previous, _previous_code, previous_directive) =
            if let Some(directive) = preprocessor_directive(previous_code.trimmed_start()) {
                (previous, previous_code, directive)
            } else {
                self.output
                    .scoped()
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .take(8)
                    .find_map(|line| {
                        let code = self.output.code_of(line).trimmed_end();
                        let directive = preprocessor_directive(code.trimmed_start())?;
                        (is_conditional_preprocessor(directive) && code.ends_with('\\'))
                            .then_some((line, code, directive))
                    })?
            };
        if (previous_directive == "else" || previous_directive.starts_with("elif"))
            && let Some(spaces) = self
                .preprocessor
                .branch_stack
                .last()
                .and_then(|branch| branch.first_body_indent_spaces)
        {
            return Some(spaces);
        }
        if matches!(previous_directive, "if" | "ifdef" | "ifndef") {
            let split_else_chain = active_split_else || self.output.recent_scoped_else_line(128);
            if split_else_chain {
                for branch in self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .take(16)
                {
                    let branch_code = self.output.code_of(branch).trimmed_end();
                    let branch_trimmed = branch_code.trimmed_start();
                    if branch_trimmed.is_empty() {
                        continue;
                    }
                    if branch_trimmed.starts_with('#') {
                        continue;
                    }
                    if branch_trimmed == "else"
                        || branch_trimmed.ends_with("} else")
                        || branch_trimmed.ends_with(" else")
                    {
                        return Some(
                            leading_visual_width(branch, self.options.tab_width)
                                + self.options.indent_width,
                        );
                    }
                    break;
                }
                if active_split_else
                    && let Some((open_spaces, _, _)) = self
                        .output
                        .current_closing_brace_open(self.options.tab_width)
                {
                    return Some(
                        self.current_closing_multiline_header_indent()
                            .unwrap_or(open_spaces)
                            + self.options.indent_width,
                    );
                }
            }
            return None;
        }
        if !(previous_directive == "else"
            || previous_directive.starts_with("elif")
            || previous_directive == "endif")
        {
            return None;
        }
        let split_else_branch = active_split_else
            || self.recent_split_else_region_any(128, |line| {
                line == "else" || line.ends_with("} else")
            });
        for (branch_index, branch) in self
            .output
            .iter()
            .enumerate()
            .rev()
            .skip_while(|(_, line)| line.as_str() != previous.as_str())
            .skip(1)
        {
            let branch_code = self.output.code_before_comment(branch_index).trimmed_end();
            let branch_trimmed = branch_code.trimmed_start();
            let branch_raw_trimmed = branch.trimmed_start();
            if branch_trimmed.is_empty()
                && !(is_comment_line(branch_raw_trimmed) || branch_raw_trimmed.starts_with("/*"))
            {
                continue;
            }
            if branch_trimmed.starts_with('#') {
                if previous_directive == "endif" {
                    continue;
                }
                break;
            }
            if previous_directive == "else" || previous_directive.starts_with("elif") {
                if branch_trimmed == "}"
                    || split_else_branch
                        && !(is_comment_line(branch_raw_trimmed)
                            || branch_raw_trimmed.starts_with("/*"))
                {
                    return Some(leading_visual_width(branch, self.options.tab_width));
                }
            } else if branch_trimmed == "else"
                || branch_trimmed.ends_with("} else")
                || branch_trimmed.ends_with(" else")
                || (is_braceless_header_line(branch_trimmed)
                    || starts_header_word(branch_trimmed, "if"))
                    && !branch_code.ends_with([';', '}'])
            {
                return Some(
                    leading_visual_width(branch, self.options.tab_width)
                        + self.options.indent_width,
                );
            } else if split_else_branch
                && (is_comment_line(branch_raw_trimmed) || branch_raw_trimmed.starts_with("/*"))
            {
                return Some(
                    self.output
                        .comment_indent_width(branch_index, self.options.tab_width),
                );
            }
            break;
        }
        if active_split_else
            && let Some((open_spaces, _, _)) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
        {
            return Some(
                self.current_closing_multiline_header_indent()
                    .unwrap_or(open_spaces)
                    + self.options.indent_width,
            );
        }
        None
    }

    pub(crate) fn split_else_branch_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "{"
            || self.layout.frame_stack.active_brace().is_some_and(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command
                    && frame.header.as_deref() == Some("else")
            })
        {
            return None;
        }
        let branch_body_spaces = self.split_else_preprocessor_branch_body_indent_spaces()?;
        let scope_start = self.output.len() - self.output.scoped().len();
        let nearest_body_spaces = (scope_start..self.output.len())
            .rev()
            .skip(1)
            .find_map(|index| {
                let line = &self.output[index];
                let code = self.output.code_before_comment(index).trimmed_end();
                let trimmed = code.trimmed_start();
                if trimmed.is_empty() || self.output.is_directive_line(index) {
                    return None;
                }
                let spaces = leading_visual_width(line, self.options.tab_width);
                Some(
                    if trimmed == "else" || trimmed.ends_with("} else") || trimmed.ends_with('{') {
                        spaces + self.options.indent_width
                    } else {
                        spaces
                    },
                )
            })
            .unwrap_or(branch_body_spaces);
        Some(branch_body_spaces.max(nearest_body_spaces))
    }

    pub(crate) fn observe_split_else_body_closing(
        &mut self,
        line: &LineView<'_>,
        output_spaces: usize,
    ) {
        if !self.preprocessor.split_else.extra_indent {
            return;
        }
        let body_indent_limit = (self.preprocessor.split_else.brace_indent
            + self.preprocessor.split_else.extra_levels
            + 1)
            * self.options.indent_width;
        let previous_line_is_else = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trimmed().is_empty())
            .is_some_and(|previous| previous.trimmed() == "else");
        let closes_by_brace = self.output.code_of(line).trimmed() == "}"
            && self.layout.indentation.indent() <= self.preprocessor.split_else.brace_indent;
        let closes_by_statement = line.ends_with(';')
            && !starts_string_literal_token(line.trimmed_start())
            && (self.preprocessor.split_else.body_braceless
                || (self.layout.indentation.indent() <= self.preprocessor.split_else.brace_indent
                    && previous_line_is_else
                    && output_spaces <= body_indent_limit));
        if closes_by_brace {
            self.preprocessor.split_else.clear_pending_after_brace = true;
        } else if closes_by_statement {
            self.clear_preprocessor_split_else_indent();
        }
    }

    pub(crate) fn split_else_local_type_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context {
            return None;
        }
        let trimmed = line.trimmed_start();
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        if previous_code.ends_with('{')
            && (trimmed.contains("struct")
                || previous_code.trimmed_start().contains("struct")
                || previous_code.trimmed_start().starts_with('}'))
            && !trimmed.starts_with(['#', '}'])
            && !is_comment_line(trimmed)
        {
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        if previous_code.ends_with("};")
            && !trimmed.starts_with(['#', '{', '}'])
            && !is_comment_line(trimmed)
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        None
    }

    pub(crate) fn split_else_post_local_type_statement_indent_spaces(
        &self,
        previous_spaces: usize,
    ) -> Option<usize> {
        let inside_local_struct = self.output.scoped_range().rev().take(8).any(|index| {
            let code = self.output.code_before_comment_trimmed(index);
            code.ends_with('{') && code.trimmed_start().contains("struct")
        });
        let after_local_struct = self.output.scoped_range().rev().take(8).any(|index| {
            let line = &self.output[index];
            let code = self.output.code_before_comment_trimmed(index);
            code.ends_with("};")
                && leading_visual_width(line, self.options.tab_width) <= previous_spaces
        });
        (inside_local_struct || after_local_struct).then_some(previous_spaces)
    }

    pub(crate) fn split_else_local_type_line_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        split_else_context: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if !split_else_context
            || line_kind != LineKind::Normal
            || line_start.starts_with('#')
            || case_unindent_spaces == 0
        {
            return None;
        }
        let trimmed = line_start;
        if trimmed.starts_with("} ") && line.trimmed_end().ends_with('{') {
            let header = self.output.scoped().iter().rev().take(16).find(|line| {
                let code = self.output.code_of(line).trimmed_end();
                let trimmed = code.trimmed_start();
                trimmed.ends_with(" struct {")
                    || trimmed.ends_with(" union {")
                    || trimmed.ends_with(" enum {")
                    || trimmed.starts_with("static const struct {")
            })?;
            return Some(
                leading_visual_width(header, self.options.tab_width) + case_unindent_spaces,
            );
        }
        if !trimmed.starts_with("};") {
            return None;
        }
        self.output
            .scoped()
            .iter()
            .rev()
            .find(|line| {
                let code = self.output.code_of(line).trimmed_end();
                code.trimmed_start().starts_with('}') && code.ends_with('{')
            })
            .map(|opener| {
                leading_visual_width(opener, self.options.tab_width) + case_unindent_spaces
            })
    }

    pub(crate) fn split_else_braced_member_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
        normal_indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || line_kind != LineKind::Normal
            || line.trimmed_start().starts_with(['#', '{', '}'])
        {
            return None;
        }
        let case_unindent_spaces =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if case_unindent_spaces == 0 {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        let normal_spaces = normal_indent * self.options.indent_width;
        let follows_braced_declaration = previous_code.trimmed() == "};"
            || previous_code.ends_with(';')
                && (previous_indent > normal_spaces
                    || self
                        .output
                        .scoped()
                        .iter()
                        .rev()
                        .take(4)
                        .any(|line| self.output.code_of(line).trimmed() == "};"))
                && current_spaces
                    .is_none_or(|spaces| spaces <= normal_spaces + case_unindent_spaces);
        follows_braced_declaration.then_some(previous_indent + case_unindent_spaces)
    }
}

fn is_preprocessor_branch_body(line: &str) -> bool {
    let line_start = line.trimmed_start();
    !line.trimmed().is_empty() && !line_start.starts_with('#') && !is_comment_only_line(line_start)
}
