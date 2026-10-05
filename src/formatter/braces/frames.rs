use crate::config::BraceStyle;
use crate::formatter::constructs::labels;
use crate::formatter::constructs::switch_cases::find_case_colon;
use crate::formatter::engine::FormatEngine;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::BraceType;
use crate::formatter::state::frame::{BraceFrame, BraceSemanticKind};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan;
use crate::formatter::text::line_scan::{
    is_comment_only_line, preprocessor_directive, trailing_comment_split_limit,
};
use crate::source::lex::{is_word_char, leading_identifier};

fn case_label_token_offset(line: &str, header: &str) -> Option<usize> {
    let code = &line[..trailing_comment_split_limit(line)];
    // Labels that lead the line own what follows them all.
    let trimmed = code.trim_start();
    if (trimmed.starts_with("case") || trimmed.starts_with("default"))
        && code.matches(':').count() > 1
        && trimmed.starts_with(header)
    {
        // A statement label after them owns the block itself.
        let mut rest = code;
        while let Some(colon) = find_case_colon(rest) {
            rest = &rest[colon + 1..];
        }
        if rest.trim_end().ends_with(':') && !rest.contains("::") {
            return None;
        }
        return Some(code.len() - trimmed.len());
    }
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

/// Whether `prefix`, the text before a label, is the brace closing the case
/// before.
fn brace_led(prefix: &str) -> bool {
    let prefix = prefix.trim();
    !prefix.is_empty() && prefix.chars().all(|ch| ch == '}')
}

fn leading_case_label_count(line: &str) -> usize {
    let mut rest = line;
    let mut count = 0;
    while let Some(colon) = find_case_colon(rest) {
        count += 1;
        rest = &rest[colon + 1..];
    }
    count
}

impl FormatEngine<'_> {
    fn brace_semantic_kind(
        &self,
        brace_type: BraceType,
        opens_lambda_body: bool,
    ) -> BraceSemanticKind {
        if opens_lambda_body {
            return BraceSemanticKind::Lambda;
        }
        if brace_type == BraceType::Command
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
        {
            return BraceSemanticKind::Array;
        }
        match brace_type {
            BraceType::Command => BraceSemanticKind::Command,
            BraceType::Definition => BraceSemanticKind::Definition,
            BraceType::Array => BraceSemanticKind::Array,
            BraceType::CompoundLiteral => BraceSemanticKind::CompoundLiteral,
            BraceType::Initializer => BraceSemanticKind::Initializer,
            BraceType::DeferArray => BraceSemanticKind::DeferArray,
            BraceType::Class
            | BraceType::Interface
            | BraceType::Struct
            | BraceType::Union
            | BraceType::Enum => BraceSemanticKind::Aggregate,
            BraceType::Namespace => BraceSemanticKind::Namespace,
            BraceType::Extern => BraceSemanticKind::Extern,
            BraceType::NonStatement => BraceSemanticKind::NonStatement,
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
            .scoped()
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
        brace_type: BraceType,
        opens_lambda_body: bool,
        lambda_header_indent: Option<usize>,
        class_base: bool,
    ) {
        let current_line_indent = if self.current_is_blank()
            && brace_header.is_some()
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|line| line.trim_start().starts_with('#'))
        {
            self.layout
                .continuation_indent
                .next_line_indent_spaces
                .or_else(|| {
                    self.layout
                        .continuation_indent
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
                    self.layout
                        .frame_stack
                        .active_header()
                        .filter(|frame| frame.header == *header)
                })
                .map_or(current_line_indent, |frame| frame.line_indent_spaces)
        } else if let Some(lambda_header_indent) = lambda_header_indent {
            lambda_header_indent
        } else if brace_type == BraceType::Definition {
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
                    .scoped()
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
            // A statement label after switch labels owns the block.
            let mut line = line.trim_start();
            while let Some(colon) = find_case_colon(line) {
                let rest = line[colon + 1..].trim_start();
                if rest.is_empty() {
                    break;
                }
                line = rest;
            }
            labels::line_kind(line, &self.options.access_labels) == LineKind::Label
        };
        let case_header = brace_header
            .filter(|header| matches!(header.as_str(), "case" | "default"))
            .filter(|header| {
                if current_label() {
                    return false;
                }
                let current = self.current.trim_start();
                if !current.is_empty() && !is_comment_only_line(current) {
                    // A statement kept after the label owns no brace.
                    return case_label_token_offset(current, header).is_some()
                        && !current[..trailing_comment_split_limit(current)]
                            .trim_end()
                            .ends_with(';');
                }
                self.has_pending_case_label_brace()
                    || self
                        .output
                        .scoped()
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
        let case_header_pending = if case_header
            .is_some_and(|header| case_label_token_offset(&self.current, header).is_some())
        {
            // Labels kept on one line take one line.
            if self.options.break_one_line_statements {
                leading_case_label_count(&self.current).max(1)
            } else {
                1
            }
        } else {
            0
        };
        let case_separated_by_preprocessor = case_block
            && self.current.trim().is_empty()
            && self.output.last_line_outside_comment().is_some_and(|line| {
                preprocessor_directive(line.trim_start())
                    .is_some_and(|directive| !is_conditional_preprocessor(directive))
            });
        let label_block =
            semantic_kind == BraceSemanticKind::Command && case_header.is_none() && current_label();
        let semantic_header = brace_header.and_then(|header| {
            self.layout
                .frame_stack
                .active_header()
                .filter(|frame| frame.header == *header)
        });
        // A switch label still leading the line opens its body first; VTK
        // counts its switch brace's level in place of it.
        let pending_case_label = usize::from(
            self.options.brace_style != BraceStyle::Vtk && find_case_colon(&self.current).is_some(),
        );
        let label_owner_column = label_block.then(|| {
            (self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal)
                + pending_case_label)
                * self.options.indent_width
        });
        let case_owner_column = case_header.and_then(|header| {
            case_label_token_offset(&self.current, header)
                .map(|offset| {
                    // VTK's and Whitesmith's labels stand at their switch's brace.
                    let vtk_switch_brace = matches!(
                        self.options.brace_style,
                        BraceStyle::Vtk | BraceStyle::Whitesmith
                    )
                    .then(|| self.layout.frame_stack.active_brace())
                    .flatten()
                    .filter(|frame| frame.header.as_deref() == Some("switch"))
                    .map(|frame| frame.sibling_indent_column);
                    let base = if let Some(column) = vtk_switch_brace
                        && self.preprocessor.split_else.extra_levels == 0
                    {
                        column
                    } else if self.preprocessor.split_else.extra_levels == 0 {
                        self.layout
                            .indentation
                            .line_indent(LineKind::SwitchLabel, self.options)
                            * self.options.indent_width
                    } else {
                        let owner_depth =
                            1 + self.preprocessor.split_else.extra_levels.saturating_sub(
                                self.layout.line_adjuster.next_line_case_unindent_depth(),
                            );
                        self.current_line_indent_spaces()
                            .saturating_sub(owner_depth * self.options.indent_width)
                    };
                    // A label after the brace closing the case before stands
                    // at the line's own column.
                    if brace_led(&self.current[..offset]) {
                        base
                    } else {
                        base + visual_width_from(
                            &self.current[..offset],
                            base,
                            self.options.tab_width,
                        )
                    }
                })
                .or_else(|| {
                    self.output.scoped().iter().rev().find_map(|line| {
                        case_label_token_offset(line, header).map(|offset| {
                            if brace_led(&line[..offset]) {
                                // Styles that indent braces put the closing one
                                // a level past its label.
                                let indented_braces = matches!(
                                    self.options.brace_style,
                                    BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
                                );
                                leading_visual_width(line, self.options.tab_width).saturating_sub(
                                    usize::from(indented_braces) * self.options.indent_width,
                                )
                            } else {
                                visual_width_from(&line[..offset], 0, self.options.tab_width)
                            }
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
        self.layout.frame_stack.push_brace(BraceFrame {
            semantic_kind,
            brace_type,
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

    pub(crate) fn update_current_brace_indent_columns(&mut self, body: usize, sibling: usize) {
        if let Some(frame) = self.layout.frame_stack.active_brace_mut() {
            frame.body_indent_column = body;
            frame.sibling_indent_column = sibling;
        }
    }

    pub(super) fn update_current_brace_indent_from_last_output_line(&mut self) {
        let Some(mut line) = self.output.last() else {
            return;
        };
        // A header continued onto the brace's line, as a max-code-length
        // split leaves it, stands where its first line does.
        if let Some(header) = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Command)
            .and_then(|frame| frame.header.as_deref())
        {
            let starts_header = |line: &str| {
                let code = line.trim_start().trim_start_matches('}').trim_start();
                let code = code
                    .strip_prefix("else")
                    .filter(|_| header != "else")
                    .map_or(code, str::trim_start);
                code.strip_prefix(header).is_some_and(|rest| {
                    !rest.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_')
                })
            };
            let code = line[..trailing_comment_split_limit(line)].trim();
            if code != "{"
                && !starts_header(line)
                && let Some(header_line) = self
                    .output
                    .iter()
                    .rev()
                    .skip(1)
                    .take(8)
                    .take_while(|line| {
                        !line[..trailing_comment_split_limit(line)]
                            .trim_end()
                            .ends_with([';', '{', '}'])
                    })
                    .find(|line| starts_header(line))
            {
                line = header_line;
            }
        }
        let code = line[..trailing_comment_split_limit(line)].trim();
        if self
            .layout
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
            .layout
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.label_block || frame.case_block)
        {
            return;
        }
        if self.layout.frame_stack.active_brace().is_some_and(|frame| {
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
        let vtk_constructor_lambda = self
            .layout
            .frame_stack
            .active_constructor_initializer()
            .is_some();
        let body_uses_brace_column = code == "{"
            && self.layout.frame_stack.active_brace().is_some_and(|frame| {
                self.options.brace_style == BraceStyle::Whitesmith
                    || self.options.brace_style == BraceStyle::Vtk
                        && (frame.semantic_kind == BraceSemanticKind::Command
                            // A file-scope brace stays in column one, off
                            // its rows.
                            || matches!(
                                frame.semantic_kind,
                                BraceSemanticKind::Array | BraceSemanticKind::Initializer
                            ) && sibling > 0
                            || frame.semantic_kind == BraceSemanticKind::Lambda
                            && (frame.header_indent_column > 0 || vtk_constructor_lambda)
                            || self.should_indent_brace_line(frame.brace_type))
                    || self.options.brace_style == BraceStyle::Ratliff
                        && matches!(
                            frame.semantic_kind,
                            BraceSemanticKind::Array | BraceSemanticKind::Initializer
                        )
            });
        // Ratliff closes the aggregate a declaration defines at its body:
        // an initializer opened on that line nests from the declaration.
        let sibling = if self.options.brace_style == BraceStyle::Ratliff
            && code.starts_with('}')
            && code.ends_with('{')
            && self.layout.frame_stack.active_brace().is_some_and(|frame| {
                matches!(
                    frame.semantic_kind,
                    BraceSemanticKind::Array | BraceSemanticKind::Initializer
                )
            }) {
            sibling.saturating_sub(self.options.indent_width)
        } else {
            sibling
        };
        let body = if body_uses_brace_column {
            sibling
        } else {
            sibling + self.options.indent_width
        };
        self.update_current_brace_indent_columns(body, sibling);
    }

    pub(super) fn exit_brace_state(&mut self) {
        self.pop_brace_state();
        // A split else never reaches past the function it is in.
        if self
            .current
            .active_token()
            .and_then(|brace| self.tree.groups.closed_at(brace))
            .is_some_and(|block| self.tree.blocks.kind(block) == Some(BlockKind::FunctionBody))
        {
            self.end_preprocessor_split_else();
            // The engine agrees that no function or statement block is open.
            if !self
                .layout
                .nesting
                .brace_type_stack
                .iter()
                .any(|brace_type| matches!(brace_type, BraceType::Definition | BraceType::Command))
            {
                self.output.set_scope_start(self.output.len());
            }
        }
    }

    fn pop_brace_state(&mut self) {
        let closes_scope = self.layout.nesting.has_active_brace_scope();
        self.layout.indentation.exit_block();
        if closes_scope {
            let bracket_depth = self.layout.indentation.bracket_depth();
            self.inline_array.initializer_designator_bracket_depth = 0;
            self.layout.frame_stack.truncate_brackets(bracket_depth);
            self.layout.objc.message_active = self.layout.frame_stack.has_objc_alignment_bracket();
            if !self.layout.objc.message_active {
                self.layout.objc.message_pending_align = false;
                self.layout.objc.message_align = None;
            }
        }
        let recovery = self.layout.nesting.exit_brace();
        for _ in 0..recovery.parens {
            self.layout.frame_stack.pop_delimiter(self.output.len());
        }
        for _ in 0..recovery.questions {
            self.layout.frame_stack.pop_active_ternary();
        }
        if closes_scope {
            self.layout.frame_stack.pop_brace();
        }
    }

    pub(super) fn mark_closed_brace_output_position(&mut self) {
        self.layout
            .frame_stack
            .mark_last_closed_brace_output_position(self.output.len());
    }

    pub(super) fn current_open_brace_is_lambda_body(&self) -> bool {
        self.layout
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
    }
}
