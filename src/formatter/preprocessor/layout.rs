use super::super::columns::leading_visual_width;
use super::super::frame::BraceSemanticKind;
use super::super::headers::{
    is_braceless_header_line, line_is_control_body_header, starts_header_word,
};
use super::super::indentation::LineKind;
use super::super::line_scan::{
    is_comment_line, is_comment_only_line, trailing_comment_split_limit,
};
use super::super::literals::starts_string_literal_token;
use super::super::operators::starts_with_chain_operator;
use super::super::{FormatEngine, unmatched_open_paren_column};
use super::{is_conditional_preprocessor, preprocessor_directive};
use crate::config::{BraceStyle, IndentStyle};
use crate::source::lex::is_identifier_start;

pub(in crate::formatter) struct SplitElseLineStart {
    extra_levels: usize,
    trigger_is_current_output: bool,
    extra_indent_active: bool,
}

pub(in crate::formatter) struct StructuralSplitElseBodyContext {
    structural_chain: bool,
    body_indent_spaces: usize,
    split_else_chain: bool,
    recent_preprocessor: bool,
    recent_adjacent_string_call_body: bool,
    opening_is_else: bool,
    opening_is_control: bool,
    case_unindent_spaces: usize,
}

pub(in crate::formatter) struct RecentSplitElseChainContext {
    chain_active: bool,
    interrupted_header_active: bool,
}

pub(in crate::formatter) struct SplitElsePreprocessorContext {
    emitted_region_active: bool,
    layout_active: bool,
}

impl StructuralSplitElseBodyContext {
    pub(in crate::formatter) fn structural_chain(&self) -> bool {
        self.structural_chain
    }

    pub(in crate::formatter) fn body_indent_spaces(&self) -> usize {
        self.body_indent_spaces
    }
}

impl RecentSplitElseChainContext {
    pub(in crate::formatter) fn chain_active(&self) -> bool {
        self.chain_active
    }

    pub(in crate::formatter) fn interrupted_header_active(&self) -> bool {
        self.interrupted_header_active
    }
}

impl SplitElsePreprocessorContext {
    pub(in crate::formatter) fn emitted_region_active(&self) -> bool {
        self.emitted_region_active
    }

    pub(in crate::formatter) fn layout_active(&self) -> bool {
        self.layout_active
    }
}

impl SplitElseLineStart {
    pub(in crate::formatter) fn extra_levels(&self) -> usize {
        self.extra_levels
    }

    pub(in crate::formatter) fn adjust_brace_level(&self, level: usize) -> usize {
        if self.trigger_is_current_output && self.extra_indent_active {
            level + self.extra_levels.saturating_sub(1)
        } else {
            level
        }
    }

    pub(in crate::formatter) fn adjust_pending_level(
        &self,
        level: usize,
        included_base_indent: usize,
    ) -> usize {
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

pub(in crate::formatter) fn embedded_branch_separator(code: &str) -> bool {
    let trimmed = code.trim_start();
    if trimmed.starts_with('#') || code.contains("#if") {
        return false;
    }
    ["#else", "#elif"].iter().any(|marker| {
        code.find(marker)
            .is_some_and(|index| code[index + marker.len()..].trim().is_empty())
    })
}

impl FormatEngine<'_> {
    pub(in crate::formatter) fn normalize_ready_preprocessor_line(&self, line: String) -> String {
        if !self.preprocessor.may_have_preprocessor {
            return line;
        }
        let line_start = line.trim_start();
        let line = if line_start.starts_with('#') && !line_start.starts_with("#define") {
            line.trim_end().to_string()
        } else {
            line
        };
        let line_start = line.trim_start();
        if line_start.starts_with("#if") && line.contains("#else") {
            line_start.to_string()
        } else {
            line
        }
    }

    pub(in crate::formatter) fn split_else_body_indent_active(&self) -> bool {
        self.preprocessor.split_else.extra_indent
    }

    pub(in crate::formatter) fn split_else_braceless_body_active(&self) -> bool {
        self.preprocessor.split_else.extra_indent && self.preprocessor.split_else.body_braceless
    }

    pub(in crate::formatter) fn split_else_line_layout_active(&self) -> bool {
        self.preprocessor_split_else_active()
            || self.preprocessor.split_else.trigger_output_len.is_some()
    }

    pub(in crate::formatter) fn clear_split_else_closing_state_on_empty_line(&mut self) {
        if self.preprocessor.split_else.extra_levels == 0 {
            self.preprocessor.split_else.clear_pending_after_brace = false;
            self.preprocessor.split_else.closing_brace_has_else = false;
        }
    }

    pub(in crate::formatter) fn take_split_else_comment_body_indent_spaces(
        &mut self,
        line: &str,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || !self.preprocessor.split_else.body_braceless
            || line.trim_start().starts_with("//")
        {
            return None;
        }
        let spaces = self
            .preprocessor
            .split_else
            .comment_body_indent_spaces
            .take()?;
        if line_is_control_body_header(line.trim_start()) {
            self.preprocessor.split_else.body_braceless = false;
            None
        } else {
            Some(spaces + self.options.indent_width)
        }
    }

    pub(in crate::formatter) fn record_split_else_comment_body_indent(
        &mut self,
        line: &str,
        output_spaces: usize,
    ) {
        if line.trim_start().starts_with("//")
            && self.preprocessor.split_else.extra_indent
            && self.preprocessor.split_else.body_braceless
        {
            self.preprocessor.split_else.comment_body_indent_spaces = Some(output_spaces);
        }
    }

    pub(in crate::formatter) fn nonconditional_directive_sibling_indent_spaces(
        &self,
        line: &str,
        normal_indent: usize,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.chars().next().is_some_and(is_identifier_start)
            || self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.state.indent() != 0
            || self.token_input.token_source_line_indent != 0
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?.trim_start();
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

    pub(in crate::formatter) fn none_style_split_else_blank_gap_sibling_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
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
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let mut spaces = None;
        if previous_trimmed == "else" || previous_trimmed.ends_with("} else") {
            let target =
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width;
            if current_spaces < target {
                spaces = Some(target);
            }
        }
        if preprocessor_directive(previous_trimmed) == Some("endif")
            && let Some(before_preprocessor) = self.output.iter().rev().skip(1).find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
        {
            let trimmed = before_preprocessor[..trailing_comment_split_limit(before_preprocessor)]
                .trim_end()
                .trim_start();
            if trimmed == "else" || trimmed.ends_with("} else") {
                let extra = usize::from(trimmed == "else") * self.options.indent_width;
                spaces =
                    Some(leading_visual_width(before_preprocessor, self.options.tab_width) + extra);
            }
        }
        spaces
    }

    pub(in crate::formatter) fn none_style_split_else_body_indent_floor(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_state_active: bool,
        current_spaces: usize,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || self.options.brace_style != BraceStyle::None
            || line.trim_start().starts_with(['#', '{', '}'])
            || is_comment_line(line.trim_start())
            || !split_else_state_active
            || !self.commented_split_else_preprocessor_region_active()
        {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let anchor = if preprocessor_directive(previous_code.trim_start()).is_some() {
            self.output.iter().rev().skip(1).find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !trimmed.starts_with('#')
            })
        } else {
            Some(previous)
        }?;
        let anchor_code = anchor[..trailing_comment_split_limit(anchor)].trim_end();
        let anchor_trimmed = anchor_code.trim_start();
        let anchor_is_header = starts_header_word(anchor_trimmed, "if")
            || starts_header_word(anchor_trimmed, "while")
            || starts_header_word(anchor_trimmed, "for")
            || anchor_trimmed.starts_with("else if")
            || anchor_trimmed.starts_with("} else");
        let split_header_spaces = if anchor_code.ends_with('{') && !anchor_is_header {
            self.output
                .iter()
                .rev()
                .skip_while(|line| line.as_str() != anchor.as_str())
                .skip(1)
                .filter(|line| {
                    let trimmed = line.trim_start();
                    !trimmed.is_empty() && !trimmed.starts_with('#')
                })
                .take(8)
                .find_map(|line| {
                    let code = line[..trailing_comment_split_limit(line)].trim_end();
                    let trimmed = code.trim_start();
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
            || anchor_code.trim() == "}"
        {
            leading_visual_width(anchor, self.options.tab_width)
        } else {
            return None;
        };
        (current_spaces < spaces).then_some(spaces)
    }

    pub(in crate::formatter) fn structural_split_else_body_context(
        &self,
        line: &str,
        line_kind: LineKind,
    ) -> Option<StructuralSplitElseBodyContext> {
        let trimmed = line.trim_start();
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
        self.output.last_non_empty_line()?;
        let structural_chain = self.preprocessor_split_else_active();
        let body_indent_spaces = if structural_chain {
            self.current_closing_multiline_header_indent()
                .map(|spaces| spaces + self.options.indent_width)
                .unwrap_or(open_spaces + self.options.indent_width)
        } else {
            open_spaces + self.options.indent_width
        };
        let recent_adjacent_string_call = self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.ends_with(");") && starts_string_literal_token(code.trim_start())
        }) && self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            unmatched_open_paren_column(code).is_some()
                && !starts_string_literal_token(code.trim_start())
                && !code.ends_with(';')
        });
        let recent_adjacent_string_call_body = recent_adjacent_string_call
            && self.output.iter().rev().take(8).any(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                unmatched_open_paren_column(code).is_some()
                    && !starts_string_literal_token(code.trim_start())
                    && !code.ends_with(';')
                    && leading_visual_width(line, self.options.tab_width) == body_indent_spaces
            });
        let split_else_chain = structural_chain
            || self
                .output
                .iter()
                .rev()
                .take(128)
                .any(|line| line.trim() == "else" || line.trim_end().ends_with("} else"));
        let recent_preprocessor = split_else_chain
            && self
                .output
                .iter()
                .rev()
                .take_while(|line| !line.trim().is_empty())
                .take(32)
                .any(|line| line.trim_start().starts_with('#'));
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
            case_unindent_spaces: self.line_adjuster.total_case_unindent_depth()
                * self.options.indent_width,
        })
    }

    pub(in crate::formatter) fn structural_split_else_ordinary_row_indent_spaces(
        &self,
        line: &str,
        current_spaces: usize,
        context: &StructuralSplitElseBodyContext,
    ) -> Option<usize> {
        if line.trim_start().starts_with(['{', '}']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_spaces = leading_visual_width(previous, self.options.tab_width);
        let body_spaces = context.body_indent_spaces;
        if context.recent_preprocessor
            && previous_code.ends_with(") {")
            && current_spaces < previous_spaces + self.options.indent_width / 2
        {
            return Some(previous_spaces + self.options.indent_width / 2);
        }
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
            && !line_is_control_body_header(line.trim_start())
            && !starts_string_literal_token(line.trim_start())
            && !is_comment_line(line.trim_start())
        {
            return Some(previous_spaces);
        }
        if context.structural_chain
            && starts_string_literal_token(previous_code.trim_start())
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
            && !line_is_control_body_header(line.trim_start())
            && !starts_string_literal_token(line.trim_start())
            && !is_comment_line(line.trim_start())
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
            && (previous_code.ends_with(';') || previous_code.trim() == "}")
            && (context.recent_adjacent_string_call_body
                || context.split_else_chain
                    && (line_is_control_body_header(line.trim_start())
                        || is_comment_line(line.trim_start())
                        || context.opening_is_else
                        || context.structural_chain
                            && current_spaces + self.options.indent_width < body_spaces
                            && context.opening_is_control
                        || starts_header_word(line.trim_start(), "if")
                        || starts_header_word(line.trim_start(), "for")
                        || starts_header_word(line.trim_start(), "while")
                        || starts_header_word(line.trim_start(), "switch")))
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

    pub(in crate::formatter) fn structural_split_else_trailing_body_indent_spaces(
        &self,
        current_spaces: usize,
        context: &StructuralSplitElseBodyContext,
    ) -> Option<usize> {
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (context.recent_adjacent_string_call_body
            && previous_code.ends_with(");")
            && starts_string_literal_token(previous_code.trim_start())
            && current_spaces < context.body_indent_spaces)
            .then_some(context.body_indent_spaces)
    }

    pub(in crate::formatter) fn split_else_branch_body_indent_override(
        &self,
        current_spaces: usize,
    ) -> Option<usize> {
        let spaces = self.split_else_preprocessor_branch_body_indent_spaces()?;
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (current_spaces < spaces
            || preprocessor_directive(previous_code.trim_start())
                .is_some_and(|directive| directive == "else" || directive.starts_with("elif")))
        .then_some(spaces)
    }

    pub(in crate::formatter) fn split_else_reduced_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let spaces = current_spaces?;
        if !self.split_else_body_indent_active()
            || line_kind == LineKind::SwitchLabel
            || line.trim_start().starts_with('}')
            || self.line_aligns_to_open_paren_content(line)
            || self.current_inline_array_column().is_some()
            || starts_with_chain_operator(line.trim_start())
        {
            return None;
        }
        let previous = self.output.last_non_empty_line();
        if previous.is_some_and(|previous| {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            code.ends_with(';')
                && (spaces == leading_visual_width(previous, self.options.tab_width)
                    || self.line_adjuster.total_case_unindent_depth() > 0
                        && (spaces
                            == leading_visual_width(previous, self.options.tab_width)
                                + self.adjusted_line_indent_delta(previous)
                            || spaces
                                == leading_visual_width(previous, self.options.tab_width)
                                    + self.line_adjuster.total_case_unindent_depth()
                                        * self.options.indent_width
                            || starts_header_word(trimmed, "if")
                            || starts_header_word(trimmed, "while")
                            || starts_header_word(trimmed, "for")))
        }) || previous.is_some_and(|previous| {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            is_comment_line(previous.trim_start()) || code.ends_with('{')
        }) || self.output.last_non_empty_line().is_some_and(|previous| {
            previous[..trailing_comment_split_limit(previous)].trim() == "{"
        }) || line.trim() == "{"
            && previous.is_some_and(|previous| {
                preprocessor_directive(previous.trim_start())
                    .is_some_and(is_conditional_preprocessor)
            })
        {
            return None;
        }
        Some(spaces.saturating_sub(self.options.indent_width))
    }

    pub(in crate::formatter) fn embedded_preprocessor_branch_body_base_spaces(
        &self,
    ) -> Option<usize> {
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

    pub(in crate::formatter) fn restored_preprocessor_branch_body_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        is_preprocessor_branch_body(line).then(|| {
            self.preprocessor.branch_stack.last().and_then(|branch| {
                branch
                    .restore_body_indent
                    .then_some(branch.first_body_indent_spaces)
                    .flatten()
            })
        })?
    }

    pub(in crate::formatter) fn record_preprocessor_branch_body_indent(
        &mut self,
        line: &str,
        emitted_indent_spaces: usize,
    ) {
        if !is_preprocessor_branch_body(line) {
            return;
        }
        if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
            if branch.first_body_indent_spaces.is_none() {
                branch.first_body_indent_spaces = Some(emitted_indent_spaces);
            }
            branch.restore_body_indent = false;
        }
    }

    pub(in crate::formatter) fn prepare_split_else_line_start(
        &mut self,
        line: &str,
        line_kind: LineKind,
    ) -> SplitElseLineStart {
        if line_kind == LineKind::Normal
            && line.trim() == "{"
            && self.preprocessor.split_else.extra_indent
            && self
                .output
                .last()
                .is_some_and(|line| line.trim().is_empty())
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

    pub(in crate::formatter) fn clear_preprocessor_split_else_indent(&mut self) {
        if self
            .frame_stack
            .active_header()
            .is_some_and(|frame| frame.header == "else")
        {
            self.frame_stack.clear_header();
        }
        self.preprocessor.split_else.reset();
    }

    pub(in crate::formatter) fn update_preprocessor_split_else_state(
        &mut self,
        line: &str,
        line_kind: LineKind,
    ) {
        if line_kind != LineKind::Normal {
            return;
        }
        let trimmed = line.trim();
        if self.preprocessor.split_else.extra_indent
            && self.state.indent() == 0
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("else")
            && !trimmed.starts_with('}')
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
                if let Some((base, delta)) = self.state.last_braceless_block()
                    && base + delta == self.state.indent()
                {
                    self.state.exit_braceless_block();
                }
                self.preprocessor.split_else.extra_indent = true;
                self.preprocessor.split_else.extra_levels += 1;
                self.preprocessor.split_else.pending_body = false;
                self.preprocessor.split_else.body_braceless = trimmed.starts_with("//");
                self.preprocessor.split_else.brace_indent = self.state.indent();
            }
        }
    }

    fn recent_output_has_split_else(&self, limit: usize) -> bool {
        (0..self.output.len()).rev().take(limit).any(|index| {
            let trimmed = self.output.code_trimmed(index);
            trimmed == "else" || trimmed.ends_with("} else")
        })
    }

    fn recent_output_has_preprocessor(&self, limit: usize) -> bool {
        (0..self.output.len())
            .rev()
            .take(limit)
            .any(|index| self.output.code_trimmed(index).starts_with('#'))
    }

    pub(in crate::formatter) fn commented_split_else_preprocessor_region_active(&self) -> bool {
        self.recent_output_has_split_else(64)
            && self.recent_split_else_region_has_preprocessor(64)
            && self.recent_split_else_region_has_block_comment(64)
    }

    pub(in crate::formatter) fn recent_split_else_preprocessor_region_active(&self) -> bool {
        self.recent_output_has_split_else(128)
            && self.recent_split_else_region_has_preprocessor(256)
    }

    pub(in crate::formatter) fn recent_split_else_output_chain_active(&self) -> bool {
        self.recent_output_has_split_else(128)
    }

    pub(in crate::formatter) fn recent_split_else_operator_region_active(&self) -> bool {
        self.recent_output_has_split_else(128)
            && self.recent_split_else_region_has_preprocessor(128)
    }

    pub(in crate::formatter) fn recent_split_else_logical_statement_region_active(&self) -> bool {
        self.recent_output_has_split_else(256) && self.recent_output_has_preprocessor(256)
    }

    pub(in crate::formatter) fn recent_split_else_call_region_active(&self) -> bool {
        self.recent_output_has_split_else(128) && self.recent_output_has_preprocessor(256)
    }

    pub(in crate::formatter) fn recent_split_else_closing_context_active(&self) -> bool {
        self.recent_output_has_split_else(256)
    }

    pub(in crate::formatter) fn split_else_preprocessor_context(
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

    pub(in crate::formatter) fn recent_split_else_chain_context(
        &self,
        line_start_active: bool,
    ) -> RecentSplitElseChainContext {
        let chain_active = line_start_active
            || (self.output.may_have_else()
                && self.output.iter().rev().take(128).any(|line| {
                    let trimmed = line.trim();
                    trimmed == "else" || trimmed.ends_with("} else")
                }));
        let has_preprocessor = self.output.may_have_hash()
            && self
                .output
                .iter()
                .rev()
                .take(128)
                .any(|line| line.trim_start().starts_with('#'));
        let mut saw_blank_after_else = false;
        let has_blank_gap = chain_active
            && self.output.may_have_else()
            && self.output.iter().rev().take(128).any(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    saw_blank_after_else = true;
                    return false;
                }
                saw_blank_after_else && (trimmed == "else" || trimmed.ends_with("} else"))
            });
        let follows_preprocessor_boundary = self.output.may_have_hash()
            && self
                .output
                .iter()
                .enumerate()
                .rev()
                .find(|(_, line)| !line.trim().is_empty())
                .is_some_and(|(previous_index, previous)| {
                    preprocessor_directive(previous.trim_start()).is_some_and(|directive| {
                        matches!(directive, "endif" | "else" | "if" | "ifdef" | "ifndef")
                            && (matches!(directive, "endif" | "else")
                                || self.output[..previous_index]
                                    .iter()
                                    .rev()
                                    .find(|line| !line.trim().is_empty())
                                    .is_some_and(|line| {
                                        let trimmed = line[..trailing_comment_split_limit(line)]
                                            .trim_end()
                                            .trim_start();
                                        preprocessor_directive(trimmed).is_some()
                                            || trimmed == "else"
                                            || trimmed.ends_with("} else")
                                            || trimmed.ends_with("}else")
                                            || is_comment_line(line.trim_start())
                                    }))
                    })
                });
        RecentSplitElseChainContext {
            chain_active,
            interrupted_header_active: chain_active
                && (line_start_active || has_preprocessor || has_blank_gap)
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
        let mut checked = 0usize;
        for index in (0..self.output.len()).rev() {
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            if self.output.lead_width(index, self.options.tab_width) == 0
                && code.ends_with('{')
                && !trimmed.starts_with('#')
            {
                break;
            }
            if checked >= limit {
                break;
            }
            if matches(trimmed) {
                return true;
            }
            checked += 1;
        }
        false
    }

    pub(in crate::formatter) fn split_else_preprocessor_branch_body_indent_spaces(
        &self,
    ) -> Option<usize> {
        let active_split_else = self.preprocessor.split_else.extra_indent
            || self.preprocessor.split_else.pending_body
            || self.preprocessor_split_else_active();
        if active_split_else
            && let Some(frame) = self
                .frame_stack
                .active_header()
                .filter(|frame| frame.header == "else")
        {
            return Some(frame.body_indent_spaces);
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let (previous, _previous_code, previous_directive) =
            if let Some(directive) = preprocessor_directive(previous_code.trim_start()) {
                (previous, previous_code, directive)
            } else {
                self.output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .take(8)
                    .find_map(|line| {
                        let code = line[..trailing_comment_split_limit(line)].trim_end();
                        let directive = preprocessor_directive(code.trim_start())?;
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
            let split_else_chain = active_split_else
                || self
                    .output
                    .iter()
                    .rev()
                    .take(128)
                    .any(|line| line.trim() == "else" || line.trim_end().ends_with("} else"));
            if split_else_chain {
                for branch in self
                    .output
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .take(16)
                {
                    let branch_code = branch[..trailing_comment_split_limit(branch)].trim_end();
                    let branch_trimmed = branch_code.trim_start();
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
                if let Some((open_spaces, _, _)) = self
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
        for branch in self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
        {
            let branch_code = branch[..trailing_comment_split_limit(branch)].trim_end();
            let branch_trimmed = branch_code.trim_start();
            let branch_raw_trimmed = branch.trim_start();
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
            {
                return Some(
                    leading_visual_width(branch, self.options.tab_width)
                        + self.options.indent_width,
                );
            } else if is_braceless_header_line(branch_trimmed)
                || starts_header_word(branch_trimmed, "if")
            {
                return Some(
                    leading_visual_width(branch, self.options.tab_width)
                        + self.options.indent_width,
                );
            } else if is_comment_line(branch_raw_trimmed) || branch_raw_trimmed.starts_with("/*") {
                return Some(leading_visual_width(branch, self.options.tab_width));
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

    pub(in crate::formatter) fn split_else_branch_opening_brace_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if line.trim() != "{"
            || self.frame_stack.active_brace().is_some_and(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command
                    && frame.header.as_deref() == Some("else")
            })
        {
            return None;
        }
        let branch_body_spaces = self.split_else_preprocessor_branch_body_indent_spaces()?;
        let nearest_body_spaces = self
            .output
            .iter()
            .rev()
            .skip(1)
            .find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') {
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

    pub(in crate::formatter) fn split_else_exact_tab_indent_level(
        &self,
        exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if self.options.indent_style != IndentStyle::Tabs {
            return None;
        }
        let spaces = self.split_else_preprocessor_branch_body_indent_spaces()?;
        let indent_width = self.options.indent_width.max(1);
        (exact_indent_spaces == Some(spaces) && spaces.is_multiple_of(indent_width))
            .then_some(spaces / indent_width)
    }

    pub(in crate::formatter) fn observe_split_else_body_closing(
        &mut self,
        line: &str,
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
            .iter()
            .rev()
            .skip(1)
            .find(|line| !line.trim().is_empty())
            .is_some_and(|previous| previous.trim() == "else");
        let closes_by_brace =
            line.trim() == "}" && self.state.indent() <= self.preprocessor.split_else.brace_indent;
        let closes_by_statement = line.ends_with(';')
            && !starts_string_literal_token(line.trim_start())
            && (self.preprocessor.split_else.body_braceless
                || (self.state.indent() <= self.preprocessor.split_else.brace_indent
                    && previous_line_is_else
                    && output_spaces <= body_indent_limit));
        if closes_by_brace {
            self.preprocessor.split_else.clear_pending_after_brace = true;
        } else if closes_by_statement {
            self.clear_preprocessor_split_else_indent();
        }
    }

    pub(in crate::formatter) fn split_else_local_type_body_indent_spaces(
        &self,
        line: &str,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context {
            return None;
        }
        let trimmed = line.trim_start();
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if previous_code.ends_with('{')
            && (trimmed.contains("struct")
                || previous_code.trim_start().contains("struct")
                || previous_code.trim_start().starts_with('}'))
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

    pub(in crate::formatter) fn split_else_post_local_type_statement_indent_spaces(
        &self,
        previous_spaces: usize,
    ) -> Option<usize> {
        let inside_local_struct = self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.ends_with('{') && code.trim_start().contains("struct")
        });
        let after_local_struct = self.output.iter().rev().take(8).any(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.ends_with("};")
                && leading_visual_width(line, self.options.tab_width) <= previous_spaces
        });
        (inside_local_struct || after_local_struct).then_some(previous_spaces)
    }

    pub(in crate::formatter) fn split_else_local_type_line_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_context: bool,
        case_unindent_spaces: usize,
    ) -> Option<usize> {
        if !split_else_context
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with('#')
            || case_unindent_spaces == 0
        {
            return None;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("} ") && line.trim_end().ends_with('{') {
            let header = self.output.iter().rev().take(16).find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
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
            .iter()
            .rev()
            .find(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.trim_start().starts_with('}') && code.ends_with('{')
            })
            .map(|opener| {
                leading_visual_width(opener, self.options.tab_width) + case_unindent_spaces
            })
    }

    pub(in crate::formatter) fn split_else_braced_member_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        normal_indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
        {
            return None;
        }
        let case_unindent_spaces =
            self.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if case_unindent_spaces == 0 {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        let normal_spaces = normal_indent * self.options.indent_width;
        let follows_braced_declaration = previous_code.trim() == "};"
            || previous_code.ends_with(';')
                && (previous_indent > normal_spaces
                    || self
                        .output
                        .iter()
                        .rev()
                        .take(4)
                        .any(|line| line[..trailing_comment_split_limit(line)].trim() == "};"))
                && current_spaces
                    .is_none_or(|spaces| spaces <= normal_spaces + case_unindent_spaces);
        follows_braced_declaration.then_some(previous_indent + case_unindent_spaces)
    }
}

fn is_preprocessor_branch_body(line: &str) -> bool {
    !line.trim().is_empty()
        && !line.trim_start().starts_with('#')
        && !is_comment_only_line(line.trim_start())
}
