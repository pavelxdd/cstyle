use crate::config::{BraceStyle, PointerAlign};
use crate::formatter::braces::classification::contains_one_line_block;
use crate::formatter::braces::rewrite::is_defer_header;
use crate::formatter::constructs::headers::{
    is_add_braces_header, is_attachable_closing_header, is_header,
};
use crate::formatter::constructs::switch_cases;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::frame::BracelessHeaderFrame;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{
    has_unclosed_delimiter_after, trailing_comment_split_limit,
};
use crate::formatter::tokens::pointers::resolved_pointer_align;
use crate::source::lex::leading_identifier;

impl FormatEngine<'_> {
    pub(crate) fn push_word(&mut self, word: &str, next: Option<&Token>) {
        self.break_line_before_word(word);
        let previous_non_ws_char = self.layout.command_state.previous_non_ws_char;
        self.record_braceless_header_frames(word, next);
        self.align_else_after_braceless_body(word);
        self.align_while_after_do(word);
        self.update_word_state(word, next);
        self.update_command_word(word, next);
        self.push_word_text(word, next, previous_non_ws_char);
    }

    fn break_line_before_word(&mut self, word: &str) {
        if self.options.break_one_line_headers
            && !self.one_line_block_mode
            && matches!(word, "else" | "while")
            && self.current.trim_end().ends_with(';')
            && !is_header(self.options, leading_identifier(self.current.trim_start()))
        {
            self.finish_line();
        }
        if self.options.brace_style == BraceStyle::Whitesmith
            && !self.one_line_block_mode
            && !self.current_line_has_class_initializer_colon
            && !self.layout.line_state.ternary_colon
            && !self.layout.objc.message_active
            && !self.current.trim_start().starts_with("@interface ")
            && !has_unclosed_delimiter_after(self.current.trim_end(), "[", "]")
            && self.current.trim_end().ends_with(':')
            && !self.current.trim_end().ends_with("::")
            && !self.current.contains('?')
            && switch_cases::find_case_colon(self.current.trim_end()).is_none()
            && !self.current.trim_start().starts_with('#')
            && self
                .current
                .trim_end()
                .trim_end_matches(':')
                .trim_end()
                .contains(|ch: char| {
                    !(ch.is_ascii_alphanumeric() || ch == '_' || ch.is_whitespace())
                })
        {
            let return_continuation = (!self.unmatched_closing_brace_recovery
                && self.current.trim_start().starts_with("return "))
            .then(|| self.current_line_indent_spaces() + "return ".len());
            self.finish_line();
            if let Some(spaces) = return_continuation {
                self.layout.continuation_indent.set_next_line_spaces(spaces);
            }
            self.previous_was_newline = true;
        }
        if self.current.trim().is_empty()
            && self.layout.nesting.has_question_in_current_brace()
            && matches!(
                word,
                "break" | "case" | "co_return" | "continue" | "default" | "goto" | "return"
            )
        {
            let closed_questions = self.layout.nesting.truncate_questions_to_brace_scope();
            for _ in 0..closed_questions {
                self.layout.frame_stack.pop_active_ternary();
            }
            self.layout.indentation.clear_continuation_indents();
            self.layout.nesting.clear_continuation_indents();
        }
    }

    fn record_braceless_header_frames(&mut self, word: &str, next: Option<&Token>) {
        let mut previous_header = self.layout.command_state.current_header.clone();
        if previous_header
            .as_deref()
            .is_some_and(|header| matches!(header, "for" | "while" | "switch" | "catch"))
            && self
                .current
                .trim()
                .eq(previous_header.as_deref().unwrap_or_default())
            && !matches!(next, Some(Token::Symbol('(')))
        {
            self.layout.command_state.current_header = None;
            previous_header = None;
        }
        if is_header(self.options, word)
            && word != "else"
            && self.layout.nesting.paren_depth == 0
            && self.current_is_blank()
            && let Some(frame) = self.layout.frame_stack.active_header()
            && !self.layout.frame_stack.active_brace().is_some_and(|brace| {
                brace.header.as_deref() == Some(frame.header.as_str())
                    && brace.header_indent_column == frame.line_indent_spaces
            })
            && is_add_braces_header(self.options, &frame.header)
            && previous_header
                .as_deref()
                .is_none_or(|header| header == frame.header)
            && !(frame.header == "do" && word == "while")
            && !(frame.header == "else" && word == "if")
            && !is_defer_header(&frame.header)
        {
            self.layout
                .frame_stack
                .push_braceless_header(BracelessHeaderFrame {
                    header: frame.header.clone(),
                    header_indent_spaces: frame.line_indent_spaces,
                    can_match_else: frame.header == "if",
                });
        }
        if is_header(self.options, word)
            && previous_header
                .as_deref()
                .is_some_and(|header| is_add_braces_header(self.options, header))
            && !matches!(previous_header.as_deref(), Some("else") if word == "if")
            && !previous_header.as_deref().is_some_and(is_defer_header)
            && self.layout.nesting.paren_depth == 0
            && !self.current_is_blank()
        {
            let nested_parent_indent = self.layout.inline_nested_header_braceless_bias;
            let mut header_indent = nested_parent_indent.unwrap_or_else(|| {
                self.layout
                    .continuation_indent
                    .next_line_indent
                    .unwrap_or_else(|| {
                        if previous_header.as_deref() == Some("else")
                            && let Some(previous) = self.output.last()
                            && previous.trim() == "else"
                        {
                            leading_visual_width(previous, self.options.tab_width)
                                / self.options.indent_width
                                + 1
                        } else {
                            self.layout.indentation.indent()
                        }
                    })
            });
            if previous_header.as_deref() == Some("if") && nested_parent_indent.is_none() {
                header_indent = header_indent.min(self.layout.indentation.indent());
            }
            self.layout.inline_nested_header_braceless_bias = Some(header_indent + 1);
            self.layout
                .frame_stack
                .push_braceless_header(BracelessHeaderFrame {
                    header: previous_header.clone().unwrap_or_default(),
                    header_indent_spaces: header_indent * self.options.indent_width,
                    can_match_else: previous_header.as_deref() == Some("if"),
                });
        }
    }

    fn align_else_after_braceless_body(&mut self, word: &str) {
        if word == "else"
            && self.current_is_blank()
            && self.output.last().is_some_and(|line| {
                line[..trailing_comment_split_limit(line)]
                    .trim_end()
                    .ends_with(';')
            })
        {
            while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && self.layout.indentation.indent() == base + delta
                && !self.braceless_header_accepts_else(base)
            {
                self.layout.indentation.exit_braceless_block();
            }
            if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && self.layout.indentation.indent() == base + delta
            {
                self.layout.continuation_indent.set_next_line_level(
                    base + self.layout.line_adjuster.total_case_unindent_depth(),
                );
            }

            let mut index = self.output.len() - 1;
            let last_trimmed = self.output[index].trim_start();
            let last_is_same_line_if = (last_trimmed.starts_with("if")
                || last_trimmed.starts_with("else if"))
                && last_trimmed.ends_with(';');
            if !last_is_same_line_if {
                while index > 0 {
                    let above = self.output[index - 1].trim_end();
                    let above_code = above[..trailing_comment_split_limit(above)].trim_end();
                    if above_code.ends_with(';')
                        || above_code.ends_with('{')
                        || above_code.ends_with('}')
                        || above_code.ends_with(')')
                        || above_code.ends_with("*/")
                    {
                        break;
                    }
                    index -= 1;
                }
            }
            while index > 0 {
                let trimmed = self.output[index].trim_start();
                if !(trimmed.starts_with('?') || trimmed.starts_with(':')) {
                    break;
                }
                index -= 1;
                while index > 0 && self.output[index].trim().is_empty() {
                    index -= 1;
                }
            }
            let body_line = &self.output[index];
            let previous_indent = leading_visual_width(body_line, self.options.tab_width);
            if previous_indent >= self.options.indent_width
                && previous_indent.is_multiple_of(self.options.indent_width)
            {
                let previous_level = previous_indent / self.options.indent_width;
                let body_trimmed = body_line.trim_start();
                let same_line_if_body = (body_trimmed.starts_with("if")
                    || body_trimmed.starts_with("else if"))
                    && body_trimmed.ends_with(';');
                let previous_body_follows_compound_condition = self.output[..index]
                    .iter()
                    .rev()
                    .take(4)
                    .any(|line| line.trim_start().starts_with("})"));
                let mut match_level = previous_level.saturating_sub(usize::from(
                    !same_line_if_body && !previous_body_follows_compound_condition,
                ));
                if !same_line_if_body && !previous_body_follows_compound_condition {
                    match_level = self.enclosing_if_level(index, previous_level, match_level);
                }
                self.layout.continuation_indent.set_next_line_level(
                    match_level + self.layout.line_adjuster.total_case_unindent_depth(),
                );
            }
            let matching_if_indent = self
                .layout
                .frame_stack
                .active_header()
                .filter(|frame| frame.header == "if")
                .map(|frame| frame.line_indent_spaces)
                .or_else(|| {
                    self.layout
                        .frame_stack
                        .take_matching_braceless_else_indent()
                });
            if let Some(indent_spaces) = matching_if_indent {
                let level = indent_spaces / self.options.indent_width;
                self.layout.continuation_indent.next_line_indent = Some(level);
                self.layout.continuation_indent.next_line_indent_spaces =
                    (indent_spaces != level * self.options.indent_width).then_some(indent_spaces);
                self.layout.inline_nested_header_braceless_bias = Some(level);
            }
        } else if word == "else"
            && self.current_is_blank()
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
            && self.layout.nesting.last_closed_brace_header.as_deref() == Some("if")
            && self.output.last().is_some_and(|line| {
                line[..trailing_comment_split_limit(line)]
                    .trim_end()
                    .ends_with('}')
            })
            && let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
        {
            self.layout
                .continuation_indent
                .set_next_line_level(base + self.layout.line_adjuster.total_case_unindent_depth());
        }
    }

    fn align_while_after_do(&mut self, word: &str) {
        if word == "while" && self.current_is_blank() {
            if self.output.last().is_some_and(|line| {
                line[..trailing_comment_split_limit(line)]
                    .trim_end()
                    .ends_with(';')
            }) {
                self.match_closing_while_to_braceless_do();
            } else if matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            ) && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")
                && let Some(previous) = self.output.last()
                && previous[..trailing_comment_split_limit(previous)].trim() == "}"
            {
                let closing_brace_indent = leading_visual_width(previous, self.options.tab_width)
                    / self.options.indent_width;
                self.layout.continuation_indent.set_next_line_level(
                    if matches!(
                        self.options.brace_style,
                        BraceStyle::Gnu | BraceStyle::Whitesmith | BraceStyle::Vtk
                    ) {
                        closing_brace_indent.saturating_sub(1)
                    } else {
                        closing_brace_indent
                    },
                );
            }
        }
    }

    fn push_word_text(
        &mut self,
        word: &str,
        next: Option<&Token>,
        previous_non_ws_char: Option<char>,
    ) {
        if previous_non_ws_char == Some('}')
            && !self.one_line_block_mode
            && (self.options.break_one_line_statements
                || (self.options.brace_style == BraceStyle::Pico
                    && !(word == "while" && self.options.attach_closing_while)))
            && contains_one_line_block(self.current.trim())
            && (matches!(word, "else" | "catch" | "@catch" | "__finally" | "__except")
                || (word == "while"
                    && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")))
        {
            self.finish_line();
        }
        let attached_closing_header = self.try_attach_leading_closing_header(word);
        let closing_header_after_brace = previous_non_ws_char == Some('}')
            && (is_attachable_closing_header(word)
                || (word == "while"
                    && self.layout.nesting.last_closed_brace_header.as_deref() == Some("do")));
        let aggregate_declarator_after_brace = previous_non_ws_char == Some('}')
            && matches!(
                self.layout.nesting.last_closed_brace_type,
                Some(BraceType::Struct | BraceType::Union | BraceType::Enum | BraceType::Class)
            );
        let is_word_operator = matches!(word, "and" | "or");
        let current_ends_pointer_operator = self.current.trim_end().ends_with(['*', '&', '^']);
        let attaches_after_pointer_array_const = word == "const"
            && self.layout.previous == PreviousToken::Operator
            && self.current.trim_end().ends_with('*')
            && matches!(next, Some(Token::Symbol('[')))
            && matches!(self.options.pointer_align, PointerAlign::Name);
        let current_without_references = self.current.trim_end().trim_end_matches('&').trim_end();
        let pointer_name_aligns_mixed_declarator = self.options.pointer_align == PointerAlign::Name
            && current_without_references.ends_with(['*', '^']);
        let trailing_operator = if self.current.trim_end().ends_with('&') {
            "&"
        } else if self.current.trim_end().ends_with('^') {
            "^"
        } else {
            "*"
        };
        let attaches_after_name_aligned_pointer = self.layout.previous == PreviousToken::Operator
            && current_ends_pointer_operator
            && (resolved_pointer_align(self.options, trailing_operator) == PointerAlign::Name
                || pointer_name_aligns_mixed_declarator)
            && self.current_paren_context_is_declaration()
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_none_or(str::is_empty);
        let attaches_after_literal_operator_name = self.layout.previous == PreviousToken::Literal
            && word.starts_with('_')
            && self.current.trim_end().ends_with("operator\"\"")
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_none_or(str::is_empty);
        let attaches_after_adjacent_string_literal_macro = self.layout.previous
            == PreviousToken::Literal
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_none_or(str::is_empty);
        if word == "noexcept"
            && self.layout.previous == PreviousToken::CloseParen
            && !self.current.ends_with([' ', '\t'])
        {
            self.emit_source_space();
        }
        if self.current_ends_cast() {
            if self.options.pad_parens_outside || self.space_after_cast {
                self.emit_source_space_or_ensure();
            }
        } else if !attached_closing_header
            && self.current.trim_end().ends_with('}')
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|gap| !gap.is_empty())
        {
            self.emit_source_space();
        } else if !attached_closing_header
            && !attaches_after_pointer_array_const
            && !attaches_after_name_aligned_pointer
            && !attaches_after_literal_operator_name
            && !attaches_after_adjacent_string_literal_macro
            && self.layout.previous.needs_space_before_word()
        {
            if matches!(
                self.layout.previous,
                PreviousToken::Word | PreviousToken::Literal
            ) || (is_word_operator && self.options.pad_operators)
            {
                self.emit_source_space_or_ensure();
            } else if !self.previous_was_newline {
                self.emit_source_space();
            }
        } else if !attached_closing_header
            && (closing_header_after_brace || aggregate_declarator_after_brace)
            && !self.previous_was_newline
        {
            self.emit_source_space_or_ensure();
        } else if !attached_closing_header && is_word_operator && self.options.pad_operators {
            self.ensure_space();
        }
        if attaches_after_pointer_array_const
            || attaches_after_name_aligned_pointer
            || attaches_after_literal_operator_name
        {
            self.trim_current_end();
        }
        if self.layout.previous == PreviousToken::Comma
            && self.options.pad_commas
            && !self.current.ends_with([' ', '\t'])
        {
            self.ensure_space();
        }
        self.current.push_str(word);
        self.space_after_cast = false;
        if is_word_operator
            && self.options.pad_operators
            && !matches!(next, Some(Token::Comment(_, _)))
        {
            self.ensure_space();
        }
        self.layout.previous = PreviousToken::Word;
        self.previous_was_newline = false;
    }

    fn update_word_state(&mut self, word: &str, next: Option<&Token>) {
        match word {
            "extern" if matches!(next, Some(Token::StringLiteral(literal)) if literal == "\"C\"") =>
            {
                self.pending_extern = true;
            }
            "case" => {
                self.reindent_trailing_comment(LineKind::SwitchLabel);
            }
            "default" if matches!(next, Some(Token::Symbol(':'))) => {
                self.reindent_trailing_comment(LineKind::SwitchLabel);
            }
            _ => {}
        }
    }
}
