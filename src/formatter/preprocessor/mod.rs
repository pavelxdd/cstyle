use super::FormatEngine;
use super::columns::leading_visual_width;
use super::continuation::ContinuationIndentState;
use super::frame::{BraceSemanticKind, ParenRole};
use super::indentation::IndentationState;
use super::line_adjust;
use super::literals::LiteralLineState;
use super::member_spacing::MemberSpacingBoundary;
use super::state::{
    CommandState, FormatterBraceType, FormatterLineState, FormatterStackState, InlineArrayState,
    PreviousToken, RunInState,
};
use super::switch_cases::SwitchCaseLayoutState;
use super::template_declarations::TemplateDeclarationState;
use super::token::Token;
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};
use std::collections::VecDeque;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct PreprocessorState {
    pub(super) branch_stack: Vec<PreprocessorBranchState>,
    pub(super) indented_block_stack: Vec<bool>,
    pub(super) indentable_blocks: VecDeque<bool>,
    pub(super) split_else: PreprocessorSplitElseState,
    pub(super) may_have_preprocessor: bool,
    pub(super) last_output_was_preprocessor: bool,
}

pub(super) mod layout;

pub(super) fn indent_off_follows_code(tokens: &[Token]) -> bool {
    let mut seen_code = false;
    for token in tokens {
        match token {
            Token::Whitespace(_) | Token::Newline => {}
            Token::Comment(_, comment) if comment.contains("*INDENT-OFF*") => return seen_code,
            Token::Comment(_, _) => {}
            _ => seen_code = true,
        }
    }
    false
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub(super) struct PreprocessorSplitElseState {
    pub(super) extra_indent: bool,
    pub(super) extra_levels: usize,
    pub(super) trigger_output_len: Option<usize>,
    pub(super) pending_body: bool,
    pub(super) clear_pending_after_brace: bool,
    pub(super) closing_brace_has_else: bool,
    pub(super) comment_body_indent_spaces: Option<usize>,
    pub(super) body_braceless: bool,
    pub(super) brace_indent: usize,
    pub(super) after_line: bool,
}

impl PreprocessorSplitElseState {
    fn is_active(self) -> bool {
        self.extra_indent
            || self.pending_body
            || self.clear_pending_after_brace
            || self.extra_levels > 0
    }

    fn reset(&mut self) {
        let brace_indent = self.brace_indent;
        *self = Self {
            brace_indent,
            ..Self::default()
        };
    }

    fn after_branch_restore(mut self, active: Self) -> Self {
        if active.extra_levels == 0 && !active.pending_body {
            self.reset();
            return self;
        }
        if active.extra_levels > self.extra_levels {
            self.extra_indent = true;
            self.extra_levels = active.extra_levels;
            self.trigger_output_len = active.trigger_output_len;
            self.body_braceless = active.body_braceless;
            self.brace_indent = active.brace_indent;
        }
        self.pending_body |= active.pending_body;
        self.clear_pending_after_brace |= active.clear_pending_after_brace;
        self.closing_brace_has_else |= active.closing_brace_has_else;
        if self.comment_body_indent_spaces.is_none() {
            self.comment_body_indent_spaces = active.comment_body_indent_spaces;
        }
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct PreprocessorBranchState {
    pub(super) state: IndentationState,
    pub(super) command_state: CommandState,
    pub(super) stack_state: FormatterStackState,
    pub(super) frame_stack: super::frame::FrameStack,
    pub(super) line_state: FormatterLineState,
    pub(super) run_in_state: RunInState,
    pub(super) line_adjuster: line_adjust::LineAdjuster,
    pub(super) previous_pre_adjust_line: Option<String>,
    pub(super) pending_member_spacing: Option<MemberSpacingBoundary>,
    pub(super) previous: PreviousToken,
    pub(super) literal_line: LiteralLineState,
    pub(super) continuation_indent: ContinuationIndentState,
    pub(super) first_body_indent_spaces: Option<usize>,
    pub(super) restore_body_indent: bool,
    pub(super) objc: super::objective_c::ObjectiveCLineState,
    pub(super) switch_case_layout: SwitchCaseLayoutState,
    pub(super) in_class_base_clause: bool,
    pub(super) split_class_export_pending_base: bool,
    pub(super) preprocessor_split_else: PreprocessorSplitElseState,
    pub(super) template_declaration: TemplateDeclarationState,
    pub(super) else_if_break_depths: Vec<usize>,
    pub(super) compound_literal: super::compound_literals::CompoundLiteralState,
    pub(super) pending_braceless_block_bias: Option<usize>,
    pub(super) inline_nested_header_braceless_bias: Option<usize>,
    pub(super) header_paren: super::headers::HeaderParenState,
    pub(super) inline_array: InlineArrayState,
    pub(super) pending_extern: bool,
    pub(super) cpp_extern_c_brace: u8,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PreprocessorLineIndent {
    Level(usize),
    Exact {
        structural_level: usize,
        spaces: usize,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum PreprocessorRegion {
    TopLevel,
    Namespace,
    Block,
    FieldDeclarationList,
    EnumList,
    MacroBody,
    Unknown,
}

impl FormatEngine<'_> {
    pub(super) fn preprocessor_region(&self, in_macro_body: bool) -> PreprocessorRegion {
        let region = Self::preprocessor_region_from_brace_stack(
            &self.stack_state.brace_type_stack,
            in_macro_body,
        );
        if region == PreprocessorRegion::TopLevel
            && self
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Namespace)
        {
            PreprocessorRegion::Namespace
        } else {
            region
        }
    }

    pub(super) fn preprocessor_region_from_brace_stack(
        brace_type_stack: &[FormatterBraceType],
        in_macro_body: bool,
    ) -> PreprocessorRegion {
        if in_macro_body {
            return PreprocessorRegion::MacroBody;
        }
        match brace_type_stack.last().copied() {
            None => PreprocessorRegion::TopLevel,
            Some(FormatterBraceType::Namespace) => PreprocessorRegion::Namespace,
            Some(FormatterBraceType::Command | FormatterBraceType::Definition) => {
                PreprocessorRegion::Block
            }
            Some(
                FormatterBraceType::Class
                | FormatterBraceType::Interface
                | FormatterBraceType::Struct
                | FormatterBraceType::Union,
            ) => PreprocessorRegion::FieldDeclarationList,
            Some(FormatterBraceType::Enum) => PreprocessorRegion::EnumList,
            Some(_) => PreprocessorRegion::Unknown,
        }
    }

    pub(super) fn preprocessor_region_allows_block_indent(
        &self,
        region: PreprocessorRegion,
    ) -> bool {
        match region {
            PreprocessorRegion::TopLevel => self.state.indent() == 0,
            PreprocessorRegion::Namespace => !self.options.indent_namespaces,
            _ => false,
        }
    }
}

pub(super) fn preprocessor_directive(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix('#')?.trim_start();
    let end = rest
        .find(|ch: char| !ch.is_ascii_alphabetic())
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

pub(super) fn output_has_active_preprocessor_branch(output: &[String]) -> bool {
    output
        .iter()
        .rev()
        .filter(|line| !line.trim().is_empty())
        .find_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("#endif") {
                return Some(false);
            }
            (trimmed.starts_with("#if")
                || trimmed.starts_with("#else")
                || trimmed.starts_with("#elif"))
            .then_some(true)
        })
        .unwrap_or(false)
}

fn is_ndef_preproc_statement(line: &str, directive: &str) -> bool {
    match directive {
        "ifndef" => true,
        "if" => preprocessor_condition(line).is_some_and(is_not_defined_condition),
        _ => false,
    }
}

pub(super) fn preprocessor_block_indentability(tokens: &[Token]) -> VecDeque<bool> {
    let mut conditionals: Vec<(usize, bool)> = Vec::new();
    let mut open_stack: Vec<usize> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let Token::Preprocessor(preprocessor) = token else {
            continue;
        };
        match preprocessor_directive(&preprocessor.text) {
            Some("if" | "ifdef" | "ifndef") => {
                open_stack.push(conditionals.len());
                conditionals.push((index, false));
            }
            Some("endif") => {
                if let Some(open) = open_stack.pop() {
                    conditionals[open].1 = true;
                }
            }
            _ => {}
        }
    }
    let mut result = VecDeque::new();
    for (position, (index, closed)) in conditionals.iter().enumerate() {
        result
            .push_back(*closed && is_indentable_preprocessor_block(tokens, *index, position == 0));
    }
    result
}

fn is_indentable_preprocessor_block(
    tokens: &[Token],
    start: usize,
    is_first_conditional: bool,
) -> bool {
    let mut depth = 0usize;
    let mut paren_depth = 0isize;
    let mut saw_conditional = false;
    let mut potential_header_guard = false;
    let mut potential_header_guard_define = false;
    let mut saw_endif_hash_line = false;
    let mut block_end = None;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Preprocessor(preprocessor) => {
                let line = &preprocessor.text;
                match preprocessor_directive(line) {
                    Some(directive @ ("if" | "ifdef" | "ifndef")) => {
                        depth += 1;
                        saw_conditional = true;
                        if is_first_conditional
                            && depth == 1
                            && is_ndef_preproc_statement(line, directive)
                        {
                            potential_header_guard = true;
                        }
                    }
                    Some("endif") => {
                        depth = depth.saturating_sub(1);
                        if saw_conditional && depth == 0 {
                            if paren_depth != 0 {
                                return false;
                            }
                            block_end = Some(index + 1);
                            break;
                        }
                    }
                    Some("define")
                        if depth > 0
                            && line.lines().count() > 1
                            && (potential_header_guard || !saw_endif_hash_line) =>
                    {
                        return false;
                    }
                    Some("define") if potential_header_guard && depth == 1 => {
                        potential_header_guard_define = true;
                    }
                    _ => {}
                }
            }
            Token::Word(word)
                if depth > 0 && word == "endif" && line_has_hash_after(tokens, index) =>
            {
                saw_endif_hash_line = true;
            }
            Token::Symbol('{' | '}') if depth > 0 => return false,
            Token::Symbol('(') if depth > 0 => paren_depth += 1,
            Token::Symbol(')') if depth > 0 => paren_depth -= 1,
            Token::Symbol(':') if depth > 0 => return false,
            Token::Newline if depth > 0 && paren_depth != 0 => return false,
            _ => {}
        }
    }
    let Some(block_end) = block_end else {
        return false;
    };
    if is_first_conditional
        && potential_header_guard_define
        && !preprocessor_block_followed_by_code(tokens, block_end)
    {
        return false;
    }
    true
}

fn line_has_hash_after(tokens: &[Token], start: usize) -> bool {
    for token in tokens.iter().skip(start + 1) {
        match token {
            Token::Newline | Token::Preprocessor(_) => return false,
            Token::Symbol('#') => return true,
            _ => {}
        }
    }
    false
}

fn preprocessor_block_followed_by_code(tokens: &[Token], start: usize) -> bool {
    tokens.iter().skip(start).any(|token| {
        !matches!(
            token,
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
        )
    })
}

pub(super) fn collapse_pound_whitespace(line: &str) -> String {
    let Some(rest) = line.strip_prefix('#') else {
        return line.to_string();
    };
    let trimmed = rest.trim_start();
    if trimmed.len() == rest.len() {
        line.to_string()
    } else {
        format!("#{trimmed}")
    }
}

fn preprocessor_condition(line: &str) -> Option<&str> {
    let line = line.trim_start().strip_prefix('#')?.trim_start();
    let condition = line.strip_prefix("if")?;
    Some(condition.trim_start())
}

fn is_not_defined_condition(condition: &str) -> bool {
    let condition = condition.trim_start();
    let Some(rest) = condition.strip_prefix('!') else {
        return false;
    };
    let Some(rest) = rest.trim_start().strip_prefix("defined") else {
        return false;
    };
    if rest.chars().next().is_some_and(is_identifier_continue) {
        return false;
    }
    let rest = rest.trim_start();
    if let Some(inner) = rest.strip_prefix('(') {
        let name = inner.trim_start();
        let name_end = name
            .char_indices()
            .take_while(|(_, ch)| is_identifier_continue(*ch))
            .map(|(index, ch)| index + ch.len_utf8())
            .last()
            .unwrap_or(0);
        return name[..name_end]
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && name[name_end..].trim_start().starts_with(')');
    }
    rest.chars().next().is_some_and(is_identifier_start)
}

pub(super) fn is_cplusplus_conditional(line: &str) -> bool {
    match preprocessor_directive(line) {
        Some("ifdef") => preprocessor_directive_argument(line) == Some("__cplusplus"),
        Some("if") => {
            let Some(condition) = preprocessor_condition(line) else {
                return false;
            };
            let Some(rest) = condition.trim_start().strip_prefix("defined") else {
                return false;
            };
            if rest.chars().next().is_some_and(is_identifier_continue) {
                return false;
            }
            let Some(inner) = rest.trim_start().strip_prefix('(') else {
                return false;
            };
            let name = inner.trim_start();
            name.strip_prefix("__cplusplus")
                .is_some_and(|tail| !tail.chars().next().is_some_and(is_identifier_continue))
        }
        _ => false,
    }
}

pub(super) fn is_conditional_preprocessor(directive: &str) -> bool {
    matches!(
        directive,
        "if" | "ifdef" | "ifndef" | "elif" | "elifdef" | "elifndef" | "else" | "endif"
    )
}

pub(super) fn is_known_preprocessor_directive(directive: &str) -> bool {
    is_conditional_preprocessor(directive)
        || matches!(
            directive,
            "define"
                | "include"
                | "include_next"
                | "import"
                | "line"
                | "error"
                | "warning"
                | "pragma"
                | "undef"
                | "region"
                | "endregion"
        )
}

pub(super) fn is_always_indented_preprocessor_line(line: &str, directive: &str) -> bool {
    matches!(directive, "region" | "endregion")
        || (directive == "pragma"
            && preprocessor_directive_argument(line)
                .is_some_and(|argument| matches!(argument, "omp" | "region" | "endregion")))
}

pub(super) fn is_bare_macro_invocation(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

pub(super) fn preprocessor_directive_argument(line: &str) -> Option<&str> {
    let mut parts = line.trim_start().strip_prefix('#')?.split_whitespace();
    parts.next()?;
    parts.next()
}

impl FormatEngine<'_> {
    pub(super) fn preprocessor_split_else_active(&self) -> bool {
        self.preprocessor.split_else.is_active()
    }

    fn mark_preprocessor_split_else_after_chain(&mut self) {
        if self.preprocessor_split_else_active() {
            self.preprocessor.split_else.after_line = true;
        }
    }

    fn preprocessor_line_follows_split_else_output(&self) -> bool {
        let mut saw_preprocessor = false;
        for line in self.output.iter().rev().take(8) {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('#') {
                if preprocessor_directive(trimmed)
                    .is_some_and(|directive| directive == "else" || directive.starts_with("elif"))
                {
                    return false;
                }
                saw_preprocessor = true;
                continue;
            }
            return saw_preprocessor && (trimmed.ends_with("} else") || trimmed == "else");
        }
        false
    }

    pub(super) fn push_preprocessor(
        &mut self,
        line: &str,
        opaque_literal_line_ranges: &[(usize, usize)],
    ) {
        let source_indent = self.current.to_string();
        self.finish_line();
        let directive = line.lines().next().and_then(preprocessor_directive);
        let header_before_preprocessor = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| {
                (
                    trailing_word(line.trim_end()).to_string(),
                    leading_visual_width(line, self.options.tab_width),
                )
            });
        let preserve_source_indent = false;
        let indent_continued_conditional = self.options.indent_preproc_conditional
            && directive.is_some_and(is_conditional_preprocessor);
        let branch_separator =
            directive.is_some_and(|directive| directive == "else" || directive.starts_with("elif"));
        if !branch_separator
            && self
                .output
                .iter()
                .rev()
                .find(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty() && !is_bare_macro_invocation(trimmed)
                })
                .is_some_and(|line| {
                    let trimmed = line.trim();
                    trimmed == "else" || trimmed.ends_with("} else")
                })
        {
            self.preprocessor.split_else.pending_body = true;
        }
        let branch_separator_after_else = branch_separator
            && (self.command_state.current_header.as_deref() == Some("else")
                || self
                    .output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| {
                        let trimmed = line.trim();
                        trimmed == "else" || trimmed.ends_with("} else")
                    }));
        if branch_separator_after_else {
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            self.pending_braceless_block_bias = None;
            self.inline_nested_header_braceless_bias = None;
            self.else_if_break_depths.clear();
            self.preprocessor.split_else.reset();
            if let Some((base, delta)) = self.state.last_braceless_block()
                && base + delta == self.state.indent()
            {
                self.state.exit_braceless_block();
            }
        } else if self.command_state.current_header.is_some()
            && directive.is_some_and(is_known_preprocessor_directive)
        {
            self.command_state.preprocessor_after_header = true;
        } else if self.command_state.current_header.is_some() && directive.is_some() {
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
        }
        let is_define = directive.is_some_and(|directive| directive == "define");

        let parts: Vec<&str> = line.lines().collect();
        let continued_define_contains_directive = is_define
            && parts.iter().skip(1).any(|part| {
                let part = part.trim_end();
                preprocessor_directive(part).is_some() && !part.ends_with('\\')
            });
        if self.options.indent_preproc_define
            && is_define
            && parts.len() > 1
            && !continued_define_contains_directive
            && opaque_literal_line_ranges.is_empty()
        {
            self.push_multiline_define(&parts);
            self.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            self.preprocessor.last_output_was_preprocessor = true;
            return;
        }

        let mut continued_line_comment = false;
        for (index, part) in parts.iter().enumerate() {
            let line_is_continued_comment = continued_line_comment;
            let is_opaque_literal_line = opaque_literal_line_ranges
                .iter()
                .any(|&(start, end)| start <= index && index <= end);
            let is_opaque_literal_continuation = is_opaque_literal_line && index > 0;
            let part = if is_opaque_literal_line {
                *part
            } else {
                part.trim_end()
            };
            let directive = (!line_is_continued_comment && !is_opaque_literal_continuation)
                .then(|| preprocessor_directive(part))
                .flatten();
            let part_known_directive = directive.is_some_and(is_known_preprocessor_directive);
            let part_is_define = directive == Some("define");
            let opening_indentable = if matches!(directive, Some("if" | "ifdef" | "ifndef")) {
                Some(self.should_indent_preprocessor_block())
            } else {
                None
            };
            let in_indentable_block = match opening_indentable {
                Some(indentable) => indentable,
                None => self.preprocessor.indented_block_stack.last() == Some(&true),
            };
            let collapse =
                self.options.indent_preproc_block && directive.is_some() && in_indentable_block;
            if index == 0 && self.take_block_spacing_blank(part) {
                self.push_empty_line();
            }
            let force_unindented_branch_separator = branch_separator_after_else && index == 0;
            let indent = if force_unindented_branch_separator {
                None
            } else if index > 0 && indent_continued_conditional {
                Some(self.current_preprocessor_indent())
            } else if directive == Some("endif")
                && self.preprocessor.branch_stack.is_empty()
                && self.token_input.token_source_line_indent > 0
            {
                Some(PreprocessorLineIndent::Exact {
                    structural_level: 0,
                    spaces: self.token_input.token_source_line_indent,
                })
            } else {
                self.preprocessor_line_indent(part, part_is_define, index)
            };
            let output_line = if let Some(indent) = indent {
                let prefix = match indent {
                    PreprocessorLineIndent::Level(level) => self.options.indent_prefix(level),
                    PreprocessorLineIndent::Exact {
                        structural_level,
                        spaces,
                    } => self
                        .options
                        .continuation_indent_prefix(structural_level, spaces),
                };
                let body = if collapse {
                    collapse_pound_whitespace(part.trim_start())
                } else {
                    part.trim_start().to_string()
                };
                format!("{prefix}{body}")
            } else if collapse {
                let leading = &part[..part.len() - part.trim_start().len()];
                format!("{leading}{}", collapse_pound_whitespace(part.trim_start()))
            } else if force_unindented_branch_separator || line_is_continued_comment {
                part.trim_start().to_string()
            } else if preserve_source_indent && index == 0 {
                format!("{source_indent}{part}")
            } else if directive.is_some()
                && (part_known_directive || self.token_input.token_source_line_indent == 0)
            {
                part.trim_start().to_string()
            } else if directive.is_some() {
                format!(
                    "{}{}",
                    " ".repeat(self.token_input.token_source_line_indent),
                    part.trim_start()
                )
            } else {
                part.to_string()
            };
            if is_opaque_literal_line {
                let structural_start = output_line.len();
                self.adjust_and_publish_raw_literal_line(output_line, structural_start);
            } else {
                self.adjust_and_publish_line(output_line);
            }
            if !line_is_continued_comment && !is_opaque_literal_continuation {
                self.update_preprocessor_state(
                    part,
                    opening_indentable,
                    index == 0 && branch_separator_after_else,
                );
            }
            let line_ends_with_backslash = part.trim_end().ends_with('\\');
            continued_line_comment = if line_is_continued_comment {
                line_ends_with_backslash
            } else if line_ends_with_backslash && !is_opaque_literal_line {
                let comment_start = super::line_scan::line_comment_split_limit(part);
                comment_start < part.len() && part[comment_start..].trim_start().starts_with("//")
            } else {
                false
            };
        }
        if (is_define && !line.trim_end().ends_with('\\'))
            || (!is_define && directive.is_some() && parts.len() > 1)
        {
            if !(is_define && self.preprocessor_split_else_active()) {
                self.continuation_indent.next_line_indent = None;
                self.continuation_indent.next_line_indent_spaces = None;
            }
            self.stack_state.clear_continuation_indents();
            self.frame_stack.clear_stream_frames();
            self.frame_stack.clear_logical_frames();
            self.continuation_indent.logical_chain_indent_spaces = None;
        }
        if directive == Some("endif")
            && let Some(previous) = self.output.last()
            && previous.contains('#')
            && !previous.trim_start().starts_with('#')
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if directive == Some("else")
            && let Some((word, indent)) = header_before_preprocessor
            && word == "do"
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces =
                Some(indent + self.options.indent_width * 2);
        }
        if is_define && parts.len() > 1 && self.preprocessor_split_else_active()
            || self.preprocessor_line_follows_split_else_output()
        {
            self.mark_preprocessor_split_else_after_chain();
        } else {
            self.preprocessor.split_else.after_line = false;
        }
        if line.trim_end().ends_with("&&")
            && let Some(previous) = self.output.last()
        {
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        self.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        self.preprocessor.last_output_was_preprocessor = true;
    }

    pub(super) fn preprocessor_base_level(&self) -> usize {
        self.preprocessor
            .indented_block_stack
            .iter()
            .filter(|&&indented| indented)
            .count()
    }

    fn preprocessor_line_indent(
        &self,
        line: &str,
        is_define: bool,
        define_part_index: usize,
    ) -> Option<PreprocessorLineIndent> {
        if self.options.indent_preproc_define && is_define {
            let continuation = if define_part_index == 0 {
                0
            } else {
                self.options.continuation_indent
            };
            return Some(PreprocessorLineIndent::Level(
                self.preprocessor_base_level() + continuation,
            ));
        }

        let directive = preprocessor_directive(line)?;
        if is_always_indented_preprocessor_line(line, directive) {
            return Some(PreprocessorLineIndent::Level(self.state.indent()));
        }
        if self.line_adjuster.is_in_macro_block() {
            return None;
        }
        if self.options.indent_preproc_conditional
            && is_conditional_preprocessor(directive)
            && self
                .frame_stack
                .active_delimiter()
                .is_some_and(|frame| frame.role == ParenRole::Header)
            && let Some(header) = self.frame_stack.active_header()
        {
            return Some(PreprocessorLineIndent::Exact {
                structural_level: header.body_indent_spaces / self.options.indent_width.max(1),
                spaces: header.body_indent_spaces,
            });
        }
        if self.options.indent_preproc_block && is_conditional_preprocessor(directive) {
            match directive {
                "if" | "ifdef" | "ifndef" => {
                    if self.preprocessor.indented_block_stack.last() == Some(&true) {
                        return Some(PreprocessorLineIndent::Level(self.state.indent()));
                    }
                }
                _ => {
                    if self.preprocessor.indented_block_stack.last() == Some(&true) {
                        return Some(PreprocessorLineIndent::Level(
                            self.state.indent().saturating_sub(1),
                        ));
                    }
                }
            }
        }
        if self.options.indent_preproc_conditional && is_conditional_preprocessor(directive) {
            if matches!(directive, "if" | "ifdef" | "ifndef")
                || self.state.current_preproc_indent().is_some()
            {
                return Some(self.current_preprocessor_indent());
            }
            return None;
        }
        if self.options.indent_preproc_block
            && !is_conditional_preprocessor(directive)
            && self
                .preprocessor
                .indented_block_stack
                .iter()
                .any(|&indented| indented)
        {
            return Some(PreprocessorLineIndent::Level(self.state.indent()));
        }
        if line.trim_start().matches('#').count() > 1
            && self.state.indent() > 0
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| line.trim() == ";")
        {
            return Some(PreprocessorLineIndent::Level(self.state.indent()));
        }
        None
    }

    fn current_preprocessor_indent(&self) -> PreprocessorLineIndent {
        if let Some(spaces) = self.direct_switch_body_indent_spaces() {
            return PreprocessorLineIndent::Exact {
                structural_level: spaces / self.options.indent_width.max(1),
                spaces,
            };
        }
        if let Some(case_label_column) = self.active_case_label_indent_spaces() {
            return PreprocessorLineIndent::Exact {
                structural_level: self.state.indent() + 1,
                spaces: case_label_column + self.options.indent_width,
            };
        }
        if let Some(indent) = self.state.current_preproc_indent() {
            if let Some(spaces) = indent.spaces {
                return PreprocessorLineIndent::Exact {
                    structural_level: indent.level,
                    spaces,
                };
            }
            return PreprocessorLineIndent::Level(indent.level);
        }
        if let Some(spaces) = self.continuation_indent.next_line_indent_spaces {
            PreprocessorLineIndent::Exact {
                structural_level: self.state.indent(),
                spaces,
            }
        } else {
            PreprocessorLineIndent::Level(self.state.indent())
        }
    }

    pub(super) fn update_preprocessor_state(
        &mut self,
        line: &str,
        opening_indentable: Option<bool>,
        branch_separator_after_else: bool,
    ) {
        match preprocessor_directive(line) {
            Some("if" | "ifdef" | "ifndef") => {
                let should_indent_block =
                    opening_indentable.unwrap_or_else(|| self.should_indent_preprocessor_block());
                if should_indent_block {
                    self.state.enter_block();
                    if let Some(spaces) = self.continuation_indent.next_line_indent_spaces.as_mut()
                    {
                        *spaces += self.options.indent_width;
                    }
                }
                self.state.push_preproc_indent(
                    self.state.indent(),
                    self.continuation_indent.next_line_indent_spaces,
                );
                self.preprocessor
                    .indented_block_stack
                    .push(should_indent_block);
                self.preprocessor.branch_stack.push(self.branch_snapshot());
            }
            Some("else" | "elif" | "elifdef" | "elifndef") => {
                if let Some(snapshot) = self.preprocessor.branch_stack.last().cloned() {
                    self.restore_branch_snapshot(snapshot);
                    if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
                        branch.restore_body_indent = branch.first_body_indent_spaces.is_some();
                    }
                }
            }
            Some("endif") => {
                self.preprocessor.branch_stack.pop();
                self.state.pop_preproc_indent();
                if self.preprocessor.indented_block_stack.pop() == Some(true) {
                    self.state.exit_block();
                    if let Some(spaces) = self.continuation_indent.next_line_indent_spaces.as_mut()
                    {
                        *spaces = spaces.saturating_sub(self.options.indent_width);
                    }
                }
            }
            _ => {}
        }
        if branch_separator_after_else {
            self.command_state.current_header = None;
            self.command_state.preprocessor_after_header = false;
            self.continuation_indent.next_line_indent = None;
            self.continuation_indent.next_line_indent_spaces = None;
            self.pending_braceless_block_bias = None;
            self.inline_nested_header_braceless_bias = None;
            self.else_if_break_depths.clear();
            self.preprocessor.split_else.reset();
            if let Some((base, delta)) = self.state.last_braceless_block()
                && base + delta == self.state.indent()
            {
                self.state.exit_braceless_block();
            }
        }
    }

    pub(super) fn should_indent_preprocessor_block(&mut self) -> bool {
        let block_is_indentable = self
            .preprocessor
            .indentable_blocks
            .pop_front()
            .unwrap_or(true);
        if self.preprocessor.indented_block_stack.last() == Some(&true) {
            return true;
        }
        self.options.indent_preproc_block
            && block_is_indentable
            && !self.line_adjuster.is_in_macro_block()
            && self.preprocessor_region_allows_block_indent(self.preprocessor_region(false))
    }

    pub(super) fn branch_snapshot(&self) -> PreprocessorBranchState {
        PreprocessorBranchState {
            state: self.state.clone(),
            command_state: self.command_state.clone(),
            stack_state: self.stack_state.clone(),
            frame_stack: self.frame_stack.clone(),
            line_state: self.line_state.clone(),
            run_in_state: self.run_in_state.clone(),
            line_adjuster: self.line_adjuster.clone(),
            previous_pre_adjust_line: self.previous_pre_adjust_line.clone(),
            pending_member_spacing: self.pending_member_spacing,
            previous: self.previous,
            literal_line: self.literal_line.clone(),
            continuation_indent: self.continuation_indent.clone(),
            first_body_indent_spaces: None,
            restore_body_indent: false,
            objc: self.objc.clone(),
            switch_case_layout: self.switch_case_layout.clone(),
            in_class_base_clause: self.in_class_base_clause,
            split_class_export_pending_base: self.split_class_export_pending_base,
            preprocessor_split_else: self.preprocessor.split_else,
            template_declaration: self.template_declaration,
            else_if_break_depths: self.else_if_break_depths.clone(),
            compound_literal: self.compound_literal.clone(),
            pending_braceless_block_bias: self.pending_braceless_block_bias,
            inline_nested_header_braceless_bias: self.inline_nested_header_braceless_bias,
            header_paren: self.header_paren.clone(),
            inline_array: self.inline_array.clone(),
            pending_extern: self.pending_extern,
            cpp_extern_c_brace: self.cpp_extern_c_brace,
        }
    }

    pub(super) fn restore_branch_snapshot(&mut self, snapshot: PreprocessorBranchState) {
        let active_split_else = self.preprocessor.split_else;
        let PreprocessorBranchState {
            state,
            command_state,
            stack_state,
            frame_stack,
            line_state,
            run_in_state,
            line_adjuster,
            previous_pre_adjust_line,
            pending_member_spacing,
            previous,
            literal_line,
            continuation_indent,
            first_body_indent_spaces: _,
            restore_body_indent: _,
            objc,
            switch_case_layout,
            in_class_base_clause,
            split_class_export_pending_base,
            preprocessor_split_else,
            template_declaration,
            else_if_break_depths,
            compound_literal,
            pending_braceless_block_bias,
            inline_nested_header_braceless_bias,
            header_paren,
            inline_array,
            pending_extern,
            cpp_extern_c_brace,
        } = snapshot;
        self.state = state;
        self.command_state = command_state;
        self.stack_state = stack_state;
        self.frame_stack = frame_stack;
        self.line_state = line_state;
        self.run_in_state = run_in_state;
        self.line_adjuster = line_adjuster;
        self.previous_pre_adjust_line = previous_pre_adjust_line;
        self.pending_member_spacing = pending_member_spacing;
        self.previous = previous;
        self.literal_line = literal_line;
        self.continuation_indent = continuation_indent;
        self.objc = objc;
        self.switch_case_layout = switch_case_layout;
        self.in_class_base_clause = in_class_base_clause;
        self.split_class_export_pending_base = split_class_export_pending_base;
        self.preprocessor.split_else =
            preprocessor_split_else.after_branch_restore(active_split_else);
        self.template_declaration = template_declaration;
        self.else_if_break_depths = else_if_break_depths;
        self.compound_literal = compound_literal;
        self.pending_braceless_block_bias = pending_braceless_block_bias;
        self.inline_nested_header_braceless_bias = inline_nested_header_braceless_bias;
        self.header_paren = header_paren;
        self.inline_array = inline_array;
        self.pending_extern = pending_extern;
        self.cpp_extern_c_brace = cpp_extern_c_brace;
    }
}

#[cfg(test)]
mod tests {
    use super::{FormatEngine, FormatterBraceType, PreprocessorRegion};
    use crate::config::FormatOptions;

    #[test]
    fn classifies_regions_from_brace_ownership() {
        use FormatterBraceType::{Array, Command, Definition, Enum, Namespace, Struct};
        use PreprocessorRegion::{
            Block, EnumList, FieldDeclarationList, MacroBody, Namespace as NamespaceRegion,
            TopLevel, Unknown,
        };

        for (stack, expected) in [
            (Vec::new(), TopLevel),
            (vec![Namespace], NamespaceRegion),
            (vec![Command], Block),
            (vec![Definition], Block),
            (vec![Struct], FieldDeclarationList),
            (vec![Enum], EnumList),
            (vec![Array], Unknown),
        ] {
            assert_eq!(
                FormatEngine::preprocessor_region_from_brace_stack(&stack, false),
                expected
            );
        }
        assert_eq!(
            FormatEngine::preprocessor_region_from_brace_stack(&[Struct], true),
            MacroBody
        );
    }

    #[test]
    fn block_indent_gate_uses_region() {
        let options = FormatOptions::default();
        let mut formatter = FormatEngine::new(&options);

        assert!(formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::TopLevel));
        formatter
            .stack_state
            .brace_type_stack
            .push(FormatterBraceType::Definition);
        assert!(!formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::Block));
        assert!(!formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::Unknown));
    }
}
