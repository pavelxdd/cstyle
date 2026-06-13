use super::FormatEngine;
use super::columns::{leading_visual_width, visual_width_from};
use super::frame::{BraceFrame, BraceSemanticKind};
use super::indentation::LineKind;
use super::labels;

use super::line_scan;
use super::line_scan::is_comment_only_line;
use super::line_scan::trailing_comment_split_limit;
use super::preprocessor::{is_conditional_preprocessor, preprocessor_directive};
use super::state::FormatterBraceType;
use crate::config::BraceStyle;
use crate::source::lex::is_word_char;
use crate::source::lex::leading_identifier;

fn case_label_token_offset(line: &str, header: &str) -> Option<usize> {
    let code = &line[..trailing_comment_split_limit(line)];
    code.match_indices(header)
        .filter_map(|(offset, _)| {
            let before = code[..offset].chars().next_back();
            let after = &code[offset + header.len()..];
            let boundary = before.is_none_or(|ch| !is_word_char(ch));
            let suffix_matches = if header == "case" {
                after.starts_with(char::is_whitespace)
            } else {
                after.trim_start().starts_with(':')
            };
            (boundary && suffix_matches).then_some(offset)
        })
        .last()
}

impl FormatEngine<'_> {
    fn brace_semantic_kind(
        &self,
        brace_type: FormatterBraceType,
        opens_lambda_body: bool,
    ) -> BraceSemanticKind {
        if opens_lambda_body {
            return BraceSemanticKind::Lambda;
        }
        if brace_type == FormatterBraceType::Command
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
        {
            return BraceSemanticKind::Array;
        }
        match brace_type {
            FormatterBraceType::Command => BraceSemanticKind::Command,
            FormatterBraceType::Definition => BraceSemanticKind::Definition,
            FormatterBraceType::Array => BraceSemanticKind::Array,
            FormatterBraceType::CompoundLiteral => BraceSemanticKind::CompoundLiteral,
            FormatterBraceType::Init => BraceSemanticKind::Initializer,
            FormatterBraceType::DeferArray => BraceSemanticKind::DeferArray,
            FormatterBraceType::Class
            | FormatterBraceType::Interface
            | FormatterBraceType::Struct
            | FormatterBraceType::Union
            | FormatterBraceType::Enum => BraceSemanticKind::Aggregate,
            FormatterBraceType::Namespace => BraceSemanticKind::Namespace,
            FormatterBraceType::Extern => BraceSemanticKind::Extern,
            FormatterBraceType::NonStatement => BraceSemanticKind::NonStatement,
        }
    }

    fn split_definition_header_indent_spaces(&self) -> Option<usize> {
        let current = &self.current[..trailing_comment_split_limit(&self.current)];
        let (mut pending_closes, _) = line_scan::line_paren_imbalance(current);
        if pending_closes == 0 {
            return None;
        }
        for previous in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(32)
        {
            let code = &previous[..trailing_comment_split_limit(previous)];
            if code.trim_end().ends_with([';', '{', '}']) {
                return None;
            }
            let (closes, opens) = line_scan::line_paren_imbalance(code);
            pending_closes += closes;
            let matched = pending_closes.min(opens.len());
            pending_closes -= matched;
            if matched > 0 && pending_closes == 0 {
                return Some(leading_visual_width(previous, self.options.tab_width));
            }
        }
        None
    }

    pub(super) fn push_brace_frame(
        &mut self,
        brace_header: Option<&String>,
        brace_type: FormatterBraceType,
        opens_lambda_body: bool,
        lambda_header_indent: Option<usize>,
        class_base: bool,
    ) {
        let current_line_indent = if self.current_is_blank()
            && brace_header.is_some()
            && self
                .output
                .last_non_empty_line()
                .is_some_and(|line| line.trim_start().starts_with('#'))
        {
            self.continuation_indent
                .next_line_indent_spaces
                .or_else(|| {
                    self.continuation_indent
                        .next_line_indent
                        .map(|level| level * self.options.indent_width)
                })
                .unwrap_or_else(|| self.current_line_indent_spaces())
        } else {
            self.current_line_indent_spaces()
        };
        let closes_interrupted_comment = self
            .current
            .find("*/")
            .is_some_and(|close| self.current[..close].rfind("/*").is_none());
        let line_indent = if closes_interrupted_comment {
            brace_header
                .and_then(|header| {
                    self.frame_stack
                        .active_header()
                        .filter(|frame| frame.header == *header)
                })
                .map_or(current_line_indent, |frame| frame.line_indent_spaces)
        } else if let Some(lambda_header_indent) = lambda_header_indent {
            lambda_header_indent
        } else if brace_type == FormatterBraceType::Definition {
            self.output_objc_method_header_indent_spaces()
                .or_else(|| self.split_definition_header_indent_spaces())
                .unwrap_or(current_line_indent)
        } else {
            current_line_indent
        };
        let header_candidate = self
            .current
            .trim_start()
            .strip_prefix('}')
            .map_or(self.current.trim_start(), str::trim_start);
        let split_header = !header_candidate.is_empty()
            && brace_header.is_some_and(|header| {
                leading_identifier(header_candidate) != header
                    && !(header == "if" && header_candidate.starts_with("else if"))
            });
        let semantic_kind = self.brace_semantic_kind(brace_type, opens_lambda_body);
        let current_label = || {
            let line = if self.current.trim().is_empty() {
                self.output
                    .iter()
                    .rev()
                    .find(|line| {
                        let trimmed = line.trim_start();
                        !trimmed.is_empty() && !is_comment_only_line(trimmed)
                    })
                    .map(String::as_str)
                    .unwrap_or_default()
            } else {
                self.current.as_str()
            };
            labels::line_kind(line.trim_start(), &self.options.access_labels) == LineKind::Label
        };
        let case_header = brace_header
            .filter(|header| matches!(header.as_str(), "case" | "default"))
            .filter(|header| {
                if current_label() {
                    return false;
                }
                let current = self.current.trim_start();
                if !current.is_empty() && !is_comment_only_line(current) {
                    return case_label_token_offset(current, header).is_some();
                }
                self.has_pending_case_label_brace()
                    || self
                        .output
                        .iter()
                        .rev()
                        .find(|line| {
                            let trimmed = line.trim_start();
                            !trimmed.is_empty()
                                && !trimmed.starts_with('#')
                                && !is_comment_only_line(trimmed)
                        })
                        .is_some_and(|line| case_label_token_offset(line, header).is_some())
            });
        let case_block = semantic_kind == BraceSemanticKind::Command && case_header.is_some();
        let case_header_pending = case_header
            .is_some_and(|header| case_label_token_offset(&self.current, header).is_some());
        let case_separated_by_preprocessor = case_block
            && self.current.trim().is_empty()
            && self.output.last_non_empty_line().is_some_and(|line| {
                preprocessor_directive(line.trim_start())
                    .is_some_and(|directive| !is_conditional_preprocessor(directive))
            });
        let label_block =
            semantic_kind == BraceSemanticKind::Command && case_header.is_none() && current_label();
        let semantic_header = brace_header.and_then(|header| {
            self.frame_stack
                .active_header()
                .filter(|frame| frame.header == *header)
        });
        let label_owner_column = label_block.then(|| {
            (self.state.line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal))
                * self.options.indent_width
        });
        let case_owner_column = case_header.and_then(|header| {
            case_label_token_offset(&self.current, header)
                .map(|offset| {
                    let base = if self.preprocessor.split_else.extra_levels == 0 {
                        self.state.line_indent(LineKind::SwitchLabel, self.options)
                            * self.options.indent_width
                    } else {
                        let owner_depth = 1 + self
                            .preprocessor
                            .split_else
                            .extra_levels
                            .saturating_sub(self.line_adjuster.next_line_case_unindent_depth());
                        self.current_line_indent_spaces()
                            .saturating_sub(owner_depth * self.options.indent_width)
                    };
                    base + visual_width_from(&self.current[..offset], base, self.options.tab_width)
                })
                .or_else(|| {
                    self.output.iter().rev().find_map(|line| {
                        case_label_token_offset(line, header).map(|offset| {
                            visual_width_from(&line[..offset], 0, self.options.tab_width)
                        })
                    })
                })
        });
        let owner_column = label_owner_column.or(case_owner_column);
        let header_indent_column = owner_column.unwrap_or_else(|| {
            semantic_header.map_or(line_indent, |frame| frame.line_indent_spaces)
        });
        let (body_indent_column, sibling_indent_column) = if let Some(owner) = label_owner_column {
            let body = owner + self.options.indent_width;
            let sibling = if matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            ) {
                body
            } else {
                owner
            };
            (body, sibling)
        } else if let Some(owner) = case_owner_column {
            let case_indent = usize::from(self.options.indent_cases) * self.options.indent_width;
            let preprocessor_indent =
                usize::from(case_separated_by_preprocessor) * self.options.indent_width;
            let body = owner + self.options.indent_width + case_indent + preprocessor_indent;
            let sibling = if matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            ) {
                body
            } else {
                owner + case_indent + preprocessor_indent
            };
            (body, sibling)
        } else if semantic_kind == BraceSemanticKind::Command {
            semantic_header.map_or(
                (line_indent + self.options.indent_width, line_indent),
                |frame| (frame.body_indent_spaces, frame.line_indent_spaces),
            )
        } else {
            (line_indent + self.options.indent_width, line_indent)
        };
        self.frame_stack.push_brace(BraceFrame {
            semantic_kind,
            formatter_type: brace_type,
            header: if label_block {
                None
            } else {
                brace_header.cloned()
            },
            label_block,
            case_block,
            case_header_pending,
            nested_case_label: false,
            class_base,
            header_indent_column,
            body_indent_column,
            sibling_indent_column,
            split_header,
            close_output_line: None,
            close_ends_output_line: false,
        });
    }

    pub(super) fn update_current_brace_indent_columns(&mut self, body: usize, sibling: usize) {
        if let Some(frame) = self.frame_stack.active_brace_mut() {
            frame.body_indent_column = body;
            frame.sibling_indent_column = sibling;
        }
    }

    pub(super) fn update_current_brace_indent_from_last_output_line(&mut self) {
        let Some(line) = self.output.last() else {
            return;
        };
        let code = line[..trailing_comment_split_limit(line)].trim();
        if self
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.header.is_some())
            && code
                .find("*/")
                .is_some_and(|close| code[..close].rfind("/*").is_none())
        {
            return;
        }
        let sibling = leading_visual_width(line, self.options.tab_width);
        if self
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.label_block || frame.case_block)
        {
            return;
        }
        if self.frame_stack.active_brace().is_some_and(|frame| {
            code != "{"
                && ((frame.semantic_kind == BraceSemanticKind::Definition
                    && frame.sibling_indent_column < sibling)
                    || (frame.semantic_kind == BraceSemanticKind::Lambda
                        && frame.sibling_indent_column != sibling)
                    || (frame.semantic_kind == BraceSemanticKind::Command
                        && frame.split_header
                        && frame.header.is_some()))
        }) {
            return;
        }
        let vtk_constructor_lambda = self.frame_stack.active_constructor_initializer().is_some();
        let body_uses_brace_column = code == "{"
            && self.frame_stack.active_brace().is_some_and(|frame| {
                self.options.brace_style == BraceStyle::Whitesmith
                    || self.options.brace_style == BraceStyle::Vtk
                        && (matches!(
                            frame.semantic_kind,
                            BraceSemanticKind::Command
                                | BraceSemanticKind::Array
                                | BraceSemanticKind::Initializer
                        ) || frame.semantic_kind == BraceSemanticKind::Lambda
                            && (frame.header_indent_column > 0 || vtk_constructor_lambda))
                    || self.options.brace_style == BraceStyle::Ratliff
                        && matches!(
                            frame.semantic_kind,
                            BraceSemanticKind::Array | BraceSemanticKind::Initializer
                        )
            });
        let body = if body_uses_brace_column {
            sibling
        } else {
            sibling + self.options.indent_width
        };
        self.update_current_brace_indent_columns(body, sibling);
    }

    pub(super) fn exit_brace_state(&mut self) {
        let closes_scope = self.stack_state.has_active_brace_scope();
        self.state.exit_block();
        if closes_scope {
            let bracket_depth = self.state.bracket_depth();
            self.inline_array.initializer_designator_bracket_depth = 0;
            self.frame_stack.truncate_brackets(bracket_depth);
            self.objc.message_active = self.frame_stack.has_objc_alignment_bracket();
            if !self.objc.message_active {
                self.objc.message_pending_align = false;
                self.objc.message_align = None;
            }
        }
        let recovery = self.stack_state.exit_brace();
        for _ in 0..recovery.parens {
            self.frame_stack.pop_delimiter(self.output.len());
        }
        for _ in 0..recovery.questions {
            self.frame_stack.pop_active_ternary();
        }
        if closes_scope {
            self.frame_stack.pop_brace();
        }
    }

    pub(super) fn mark_closed_brace_output_position(&mut self) {
        self.frame_stack
            .mark_last_closed_brace_output_position(self.output.len());
    }

    pub(super) fn current_open_brace_is_lambda_body(&self) -> bool {
        self.frame_stack
            .active_brace()
            .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
    }
}
