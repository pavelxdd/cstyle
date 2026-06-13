use super::FormatEngine;
use super::brace_classification::is_class_like_brace_type;
use super::columns::visual_width_from;
use super::frame::{DeclarationFrame, PointerRole};
use super::language;

use super::language::{
    is_macro_like_word, is_non_type_keyword, is_pointer_type_word, is_type_like_pointer_word,
};
use super::line_scan::last_unmatched_open_delimiter;
use super::line_scan::trailing_matching_parens;

use super::return_types::is_return_type_line;
use super::state::{FormatterBraceType, PreviousToken};
use super::syntax::{
    function_head_has_assignment, function_name_start, scoped_name_is_constructor,
};
use super::token::Token;
use crate::config::{PointerAlign, ReferenceAlign};
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct PointerRunState {
    pub(super) trailing_ws: Option<String>,
    pub(super) next_is_name_like: bool,
    pub(super) followed_by_reference: bool,
    pub(super) reference_has_name: bool,
    pub(super) followed_by_comment: bool,
    pub(super) star_count: usize,
    pub(super) gap_before_column: Option<usize>,
}

pub(super) fn pointer_next_is_name_like(next: Option<&Token>) -> bool {
    matches!(
        next,
        Some(Token::Word(_) | Token::Number(_) | Token::Symbol('(' | '['))
    )
}

impl FormatEngine<'_> {
    fn pointer_role(&self, operator: &str, next: Option<&Token>) -> PointerRole {
        if self.is_function_pointer_parameter_continuation()
            || (operator == "*" && matches!(next, Some(Token::Symbol(')'))))
        {
            PointerRole::FunctionPointer
        } else if self.looks_like_pointer_declaration_context() {
            if operator.contains('&') {
                PointerRole::DeclarationReference
            } else {
                PointerRole::DeclarationPointer
            }
        } else if self.is_unary_pointer_operator() {
            PointerRole::UnaryOperator
        } else if self.current.trim_end().ends_with(')')
            && is_type_like_pointer_word(trailing_word(self.current.trim_end_matches(')')))
        {
            PointerRole::CastTypeGroup
        } else {
            PointerRole::BinaryOperator
        }
    }

    fn record_declaration_frame_for_pointer(&mut self, operator: &str, next: Option<&Token>) {
        self.frame_stack.push_declaration(DeclarationFrame {
            pointer_role: self.pointer_role(operator, next),
            continuation_anchor_column: None,
            closing_anchor_column: None,
            is_typedef: self.current.trim_start().starts_with("typedef "),
        });
    }

    pub(super) fn is_rvalue_reference_like(&self, next: Option<&Token>) -> bool {
        if self.current.trim_end().ends_with('*') {
            return true;
        }
        if matches!(next, Some(Token::Symbol('[')))
            && trailing_word(&self.current) == language::AUTO
        {
            return true;
        }
        if matches!(next, Some(Token::Symbol('('))) {
            let current = self.current.trim_end();
            if current
                .rfind("operator ")
                .is_some_and(|operator| !current[operator + "operator ".len()..].trim().is_empty())
            {
                return true;
            }
        }
        if matches!(next, None | Some(Token::Newline))
            && self.looks_like_pointer_declaration_context()
        {
            return true;
        }
        let is_trailing_return_reference = matches!(next, Some(Token::Symbol(';' | '{')))
            && self
                .current
                .rsplit([';', '{', '}'])
                .next()
                .is_some_and(|statement| statement.contains("->"));
        if self.pointer_in_template_type_context(next)
            || matches!(next, Some(Token::Symbol(')')))
                && (self.current_in_cast_type_group()
                    || self.current_in_parenthesized_type_operand())
        {
            return true;
        }
        if !is_trailing_return_reference
            && !matches!(next, Some(Token::Word(_)) | Some(Token::Symbol(')')))
        {
            return false;
        }
        if is_trailing_return_reference {
            return true;
        }
        let previous_word = trailing_word(&self.current);
        if (self.command_state.current_header.is_some() && previous_word != language::AUTO)
            || (self.stack_state.paren_depth > 0
                && self
                    .stack_state
                    .brace_type_stack
                    .last()
                    .is_some_and(|brace_type| *brace_type == FormatterBraceType::Command))
        {
            return false;
        }
        previous_word == language::AUTO
            || self.current.trim_end().ends_with('>')
            || self.looks_like_pointer_declaration_context()
    }

    pub(super) fn is_pointer_like(
        &self,
        operator: &str,
        next: Option<&Token>,
        next_is_adjacent: bool,
        following_operator: Option<&str>,
    ) -> bool {
        if !matches!(operator, "*" | "&" | "^") {
            return false;
        }
        if let Some(Token::Word(word)) = next
            && matches!(
                word.as_str(),
                "sizeof" | "return" | "case" | "new" | "delete" | "throw"
            )
        {
            return false;
        }
        if self.template_close_before_current || self.pointer_in_template_type_context(next) {
            return true;
        }
        if matches!(next, Some(Token::Symbol('['))) && self.looks_like_pointer_declaration_context()
        {
            return true;
        }
        if matches!(operator, "&" | "*")
            && matches!(next, Some(Token::Word(_)))
            && self.current_paren_started_by_catch()
        {
            return true;
        }
        if self.frame_stack.bracket_depth() > 0
            && matches!(
                self.previous,
                PreviousToken::Word
                    | PreviousToken::Literal
                    | PreviousToken::CloseParen
                    | PreviousToken::CloseBracket
            )
        {
            return false;
        }
        if matches!(operator, "*" | "&")
            && self.previous == PreviousToken::Operator
            && self.is_unary_pointer_operator()
            && !self.current.trim_end().ends_with(['*', '&', '^', ':'])
        {
            return false;
        }
        if self.current.trim_end().ends_with(['*', '&', '^']) {
            return true;
        }
        if self.current.trim_end().ends_with('}')
            && matches!(
                self.stack_state.last_closed_brace_type,
                Some(
                    FormatterBraceType::Class
                        | FormatterBraceType::Struct
                        | FormatterBraceType::Union
                        | FormatterBraceType::Enum
                        | FormatterBraceType::Interface
                )
            )
            && matches!(next, Some(Token::Word(_)) | Some(Token::Symbol('(')))
        {
            return true;
        }
        if self.current_in_objc_method_type_group() {
            return true;
        }
        if matches!(operator, "*" | "&" | "^")
            && matches!(next, Some(Token::Symbol(')')))
            && (self.current_in_cast_type_group() || self.current_in_parenthesized_type_operand())
        {
            return true;
        }
        if operator == "*" && self.current.trim_end().ends_with(')') {
            return self.current_ends_type_group();
        }
        if matches!(operator, "*")
            && matches!(next, Some(Token::Operator(next_operator)) if next_operator == "*")
        {
            if next_is_adjacent {
                if self.current.trim().is_empty() {
                    return self.is_function_declaration_parameter_continuation();
                }
                if self.stack_state.paren_depth > 0
                    && !self.current_paren_context_is_declaration()
                    && !self.current_in_objc_method_type_group()
                    && !self.is_function_declaration_parameter_continuation()
                {
                    return false;
                }
                return !trailing_word(&self.current)
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_digit());
            }
            return is_type_like_pointer_word(trailing_word(&self.current));
        }
        if self.current.trim_end().ends_with('(') {
            return matches!(next, Some(Token::Word(_)) | Some(Token::Symbol(')')))
                || matches!(
                    next,
                    Some(Token::Operator(next_operator))
                        if matches!(next_operator.as_str(), "*" | "&" | "&&" | "^")
                ) && (self.looks_like_pointer_declaration_context()
                    || self
                        .current
                        .rfind('(')
                        .map(|open| trailing_word(self.current[..open].trim_end()))
                        .is_some_and(is_type_like_pointer_word));
        }
        if self.current.trim_end().ends_with("::") {
            return matches!(next, Some(Token::Word(_)) | Some(Token::Symbol(')')))
                || matches!(
                    next,
                    Some(Token::Operator(next_operator))
                        if matches!(next_operator.as_str(), "&" | "&&")
                );
        }
        if operator == "*"
            && matches!(next, Some(Token::Word(word)) if word == "const")
            && (self.looks_like_pointer_declaration_context()
                || (self.current.trim_start().starts_with('(')
                    && self
                        .current
                        .split_whitespace()
                        .any(|word| matches!(word, "struct" | "union" | "enum"))))
        {
            return true;
        }
        if operator == "*"
            && matches!(next, Some(Token::Operator(next_operator)) if next_operator == ">")
            && (is_type_like_pointer_word(trailing_word(&self.current))
                || self.current_ends_named_cast_type_argument())
        {
            return true;
        }
        if self.in_initializer_brace()
            && matches!(next, Some(Token::Word(_)))
            && trailing_word(&self.current)
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            return false;
        }
        if self.stack_state.paren_depth > 0
            && matches!(next, Some(Token::Word(_)))
            && trailing_word(&self.current)
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            if self.looks_like_pointer_declaration_context() {
                return true;
            }
            if matches!(next, Some(Token::Word(word)) if is_macro_like_word(word)) {
                return false;
            }
            if let Some(following_operator) = following_operator
                && !matches!(following_operator, "*" | "&")
            {
                return matches!(following_operator, "=" | ":");
            }
            let previous_word = trailing_word(&self.current);
            if is_pointer_type_word(previous_word) && !is_macro_like_word(previous_word) {
                return true;
            }
        }
        if matches!(next, Some(Token::Comment(_, _))) {
            return is_type_like_pointer_word(trailing_word(&self.current))
                || self.looks_like_pointer_declaration_context();
        }
        if matches!(next, Some(Token::Symbol('('))) {
            return is_pointer_type_word(trailing_word(&self.current))
                || self.looks_like_pointer_declaration_context();
        }
        let next_can_follow_pointer = match next {
            None | Some(Token::Newline) => true,
            Some(Token::Word(_)) | Some(Token::Symbol(')' | ',')) => true,
            Some(Token::Operator(op)) if matches!(op.as_str(), "*" | "&" | "&&" | "^" | "=") => {
                true
            }
            _ => false,
        };
        if !next_can_follow_pointer {
            return false;
        }
        if operator == "&"
            && self.stack_state.paren_depth > 0
            && matches!(next, Some(Token::Word(_)))
            && !self.current_paren_context_is_declaration()
            && !is_pointer_type_word(trailing_word(&self.current))
        {
            return false;
        }
        if operator == "*" && self.current.trim_end().ends_with(')') {
            return self.current_ends_type_group();
        }
        is_pointer_type_word(trailing_word(&self.current))
            || self.looks_like_pointer_declaration_context()
    }

    fn pointer_in_template_type_context(&self, next: Option<&Token>) -> bool {
        if self.line_state.template_angle_depth == 0 {
            return false;
        }
        match next {
            Some(Token::Operator(operator)) if operator.starts_with('>') => true,
            Some(Token::Symbol(',')) => true,
            Some(Token::Word(word))
                if matches!(word.as_str(), "const" | "volatile" | "restrict") =>
            {
                true
            }
            Some(Token::Word(_)) if self.current.trim_start().starts_with("template") => {
                let segment = self
                    .current
                    .rsplit(['<', ','])
                    .next()
                    .unwrap_or_default()
                    .trim();
                is_pointer_declaration_segment(segment)
            }
            _ => false,
        }
    }

    pub(super) fn current_ends_named_cast_type_argument(&self) -> bool {
        let current = self.current.trim_end();
        let Some(open) = current.rfind('<') else {
            return false;
        };
        if current[open + 1..].contains('>') {
            return false;
        }
        matches!(
            trailing_word(current[..open].trim_end()),
            "static_cast" | "const_cast" | "dynamic_cast" | "reinterpret_cast"
        )
    }

    pub(super) fn current_ends_type_group(&self) -> bool {
        let current = self.current.trim_end();
        let Some((open, close)) = trailing_matching_parens(current) else {
            return false;
        };
        if close + 1 != current.len() {
            return false;
        }
        let before = current[..open].trim_end();
        let name = trailing_word(before);
        if name.is_empty()
            || !(matches!(
                name,
                "decltype" | "typeof" | "typeof_unqual" | "__typeof__" | "_Atomic" | "_BitInt"
            ) || is_macro_like_word(name))
        {
            return false;
        }
        let segment = before
            .rfind(['(', ',', ';', '{', '}'])
            .map_or(before, |index| &before[index + 1..])
            .trim();
        !segment.chars().any(|ch| {
            matches!(
                ch,
                '=' | '+' | '-' | '/' | '%' | '?' | '!' | '|' | '<' | '>'
            )
        }) && !matches!(segment.split_whitespace().next(), Some("return" | "case"))
    }

    pub(super) fn looks_like_pointer_declaration_context(&self) -> bool {
        let current = self.current.trim_end();
        if current.is_empty() {
            return false;
        }
        let mut segment_start = 0usize;
        let mut saved_starts: Vec<usize> = Vec::new();
        let mut angle_depth = 0u32;
        for (index, ch) in current.char_indices() {
            match ch {
                '(' => {
                    saved_starts.push(segment_start);
                    segment_start = index + ch.len_utf8();
                }
                ')' => {
                    if let Some(prev) = saved_starts.pop() {
                        segment_start = prev;
                    }
                }
                '<' => angle_depth += 1,
                '>' => angle_depth = angle_depth.saturating_sub(1),
                ',' | ';' | '{' | '}' if saved_starts.is_empty() && angle_depth == 0 => {
                    segment_start = index + ch.len_utf8();
                }
                _ => {}
            }
        }
        let segment_text = strip_balanced_parens(current[segment_start..].trim());
        let segment = segment_text.trim();
        if segment.is_empty() || !is_pointer_declaration_segment(segment) {
            return false;
        }
        if self.stack_state.paren_depth == 0 {
            return true;
        }
        self.current_paren_context_is_declaration()
            || self.current_paren_context_is_constructor_declaration(segment)
            || self.current_paren_context_has_attached_return_type()
            || self.is_function_declaration_parameter_continuation()
            || (self.is_function_pointer_parameter_continuation()
                && segment
                    .split_whitespace()
                    .any(|word| is_type_like_pointer_word(word) && !is_macro_like_word(word)))
            || segment
                .split_whitespace()
                .any(|word| is_pointer_type_word(word) && !is_macro_like_word(word))
    }

    pub(super) fn is_function_declaration_parameter_continuation(&self) -> bool {
        for line in self.output.iter().rev().take(8) {
            let trimmed = line.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
            if !has_unclosed_balanced_delimiter(trimmed, "(", ")") {
                continue;
            }
            let Some(open) = trimmed.find('(') else {
                continue;
            };
            let before = trimmed[..open].trim_end();
            if before.is_empty() || before.contains('=') {
                return false;
            }
            let Some(name_start) = function_name_start(before) else {
                return false;
            };
            let return_type = before[..name_start].trim_end();
            let name = before[name_start..].trim_start();
            return !return_type.is_empty() && !name.is_empty() && !self.is_header(name);
        }
        false
    }

    pub(super) fn is_function_pointer_parameter_continuation(&self) -> bool {
        self.output.iter().rev().take(4).any(|line| {
            let trimmed = line.trim_end();
            trimmed.contains("(*") && !trimmed.ends_with(';') && !trimmed.ends_with('}')
        })
    }

    pub(super) fn current_paren_context_is_declaration(&self) -> bool {
        if self.current_paren_is_lambda_parameter_list() {
            return true;
        }
        match last_unmatched_open_delimiter(&self.current) {
            Some(('(', open)) => {
                let before = self.current[..open].trim_end();
                if !before.is_empty() {
                    return self.paren_head_is_declaration(before);
                }
                // The enclosing open paren starts this line; its head is the
                // preceding output line.
                self.output
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| self.paren_head_is_declaration(line.trim_end()))
            }
            Some(_) => false,
            None => {
                // Continuation line of a multi-line parameter list: the enclosing
                // open paren and its function head live on an earlier output line.
                let Some(before) = self.enclosing_open_paren_head() else {
                    return false;
                };
                let before = before.trim_end();
                self.paren_head_is_declaration(before)
                    || (!before.is_empty()
                        && !function_head_has_assignment(before)
                        && scoped_name_is_constructor(before))
            }
        }
    }

    pub(super) fn paren_head_is_declaration(&self, before: &str) -> bool {
        if before.is_empty() || before.contains('?') || function_head_has_assignment(before) {
            return false;
        }
        if let Some((open, close)) = trailing_matching_parens(before)
            && close + 1 == before.len()
        {
            let declarator = before[open + 1..close].trim_start();
            if declarator.starts_with(['*', '&', '^'])
                || declarator.contains("::*")
                || declarator.contains(":: *")
            {
                return true;
            }
        }
        let Some(name_start) = function_name_start(before) else {
            return false;
        };
        let return_type = before[..name_start].trim_end();
        let name = before[name_start..].trim_start();
        if name.is_empty() || self.is_header(name) || is_non_type_keyword(name) {
            return false;
        }
        if return_type.is_empty() {
            return self
                .stack_state
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| is_class_like_brace_type(*brace_type));
        }
        if return_type.contains(['.', '[', ']']) {
            return false;
        }
        let last_type_word = return_type
            .rsplit(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty());
        !last_type_word.is_some_and(is_non_type_keyword)
    }

    fn enclosing_open_paren_head(&self) -> Option<String> {
        for line in self.output.iter().rev().take(8) {
            let trimmed = line.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
            if !has_unclosed_balanced_delimiter(trimmed, "(", ")") {
                continue;
            }
            let open = trimmed.find('(')?;
            return Some(trimmed[..open].trim_end().to_string());
        }
        None
    }

    pub(super) fn current_paren_is_lambda_parameter_list(&self) -> bool {
        let Some(open) = self.current.rfind('(') else {
            return false;
        };
        let mut before = self.current[..open].trim_end();
        if before.ends_with('>') {
            let mut depth = 0usize;
            let mut template_open = None;
            for (index, ch) in before.char_indices().rev() {
                match ch {
                    '>' => depth += 1,
                    '<' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            template_open = Some(index);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(template_open) = template_open else {
                return false;
            };
            before = before[..template_open].trim_end();
        }
        if !before.ends_with(']') {
            return false;
        }
        let bytes = before.as_bytes();
        let mut depth = 0i32;
        let mut start = None;
        for index in (0..bytes.len()).rev() {
            match bytes[index] {
                b']' => depth += 1,
                b'[' => {
                    depth -= 1;
                    if depth == 0 {
                        start = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(start) = start else {
            return false;
        };
        let prefix = before[..start].trim_end();
        !prefix.ends_with(|ch: char| is_identifier_continue(ch) || matches!(ch, ')' | ']'))
    }

    pub(super) fn current_paren_context_is_constructor_declaration(&self, segment: &str) -> bool {
        if !is_pointer_declaration_segment(segment) {
            return false;
        }
        let Some(open) = self.current.rfind('(') else {
            return false;
        };
        let before = self.current[..open].trim_end();
        !before.is_empty()
            && !function_head_has_assignment(before)
            && scoped_name_is_constructor(before)
    }

    pub(super) fn current_paren_context_has_attached_return_type(&self) -> bool {
        let Some(open) = self.current.rfind('(') else {
            return false;
        };
        let before = self.current[..open].trim_end();
        if before.is_empty() || function_head_has_assignment(before) || self.is_header(before) {
            return false;
        }
        if !matches!(function_name_start(before), Some(0)) {
            return false;
        }
        self.output
            .last()
            .is_some_and(|line| is_return_type_line(line.trim()))
    }

    pub(super) fn push_pointer_run(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        next_is_adjacent: bool,
    ) {
        let continues_sequence = matches!(
            next,
            Some(Token::Operator(next_operator)) if next_operator == operator
        );
        if continues_sequence && !next_is_adjacent {
            self.record_declaration_frame_for_pointer(operator, next);
            if operator == "&"
                && matches!(
                    self.resolved_pointer_align(operator),
                    PointerAlign::Type | PointerAlign::Name
                )
            {
                self.trim_current_end_horizontal_space();
            } else {
                self.emit_source_space();
            }
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            return;
        }
        if continues_sequence && self.pointer_run.star_count > 1 {
            self.skip_adjacent_pointer_operators = self.pointer_run.star_count - 1;
            let sequence = operator.repeat(self.pointer_run.star_count);
            self.push_pointer_or_reference(&sequence, next, next_is_adjacent);
        } else {
            self.push_pointer_or_reference(operator, next, next_is_adjacent);
        }
    }

    pub(super) fn push_pointer_or_reference(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        next_is_adjacent: bool,
    ) {
        let followed_by_reference = self.pointer_run.followed_by_reference
            || matches!(
                next,
                Some(Token::Operator(next_operator)) if matches!(next_operator.as_str(), "&" | "&&")
            );
        if self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && self.is_function_declaration_parameter_continuation()
        {
            self.record_declaration_frame_for_pointer(operator, next);
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            return;
        }
        if self.current.trim_end().ends_with('(') && !followed_by_reference {
            self.record_declaration_frame_for_pointer(operator, next);
            self.current.push_str(operator);
            self.emit_trailing_source_space();
            return;
        }
        let is_after_scope_resolution = self.current.trim_end().ends_with(':');
        if is_after_scope_resolution && operator == "*" {
            self.record_declaration_frame_for_pointer(operator, next);
            match self.resolved_pointer_align(operator) {
                PointerAlign::None => {
                    self.current.push_str(operator);
                    self.emit_trailing_source_space();
                }
                PointerAlign::Type => {
                    self.trim_current_end_horizontal_space();
                    self.current.push_str(operator);
                    if !followed_by_reference {
                        let gap = self.consolidated_pointer_gap();
                        self.current.push_str(&gap);
                    }
                }
                PointerAlign::Middle => {
                    self.trim_current_end_horizontal_space();
                    self.current.push_str(operator);
                    if !followed_by_reference {
                        let mut gap = self
                            .token_input
                            .previous_input_whitespace
                            .clone()
                            .unwrap_or_default();
                        gap.push_str(self.pointer_run.trailing_ws.as_deref().unwrap_or_default());
                        if gap.is_empty() {
                            gap.push(' ');
                        }
                        self.current.push_str(&gap);
                    }
                }
                PointerAlign::Name => {
                    self.trim_current_end_horizontal_space();
                    self.current.push_str(operator);
                }
            }
            return;
        }
        if operator.starts_with('&') && self.current.trim_end().ends_with('*') {
            self.record_declaration_frame_for_pointer(operator, next);
            let align = self.resolved_pointer_align(operator);
            self.trim_current_end();
            if operator == "&" && self.options.pointer_align == PointerAlign::Name {
                self.current.push('&');
                return;
            }
            match align {
                PointerAlign::None => {
                    if let Some(gap) = self.token_input.previous_input_whitespace.clone() {
                        self.current.push_str(&gap);
                    }
                    self.current.push_str(operator);
                    self.emit_trailing_source_space();
                }
                PointerAlign::Type => {
                    self.current.push_str(operator);
                    let gap = self.consolidated_pointer_gap();
                    self.current.push_str(&gap);
                }
                PointerAlign::Middle
                    if operator == "&" && self.options.pointer_align == PointerAlign::Middle =>
                {
                    self.current.push('&');
                    let gap = self.consolidated_pointer_gap();
                    self.current.push_str(&gap);
                }
                PointerAlign::Middle => {
                    let (before, after) = self.middle_pointer_gaps();
                    self.current.push_str(&before);
                    self.current.push_str(operator);
                    self.current.push_str(&after);
                }
                PointerAlign::Name => {
                    let gap = self.consolidated_pointer_gap();
                    self.current.push_str(&gap);
                    self.current.push_str(operator);
                }
            }
            return;
        }
        if operator == "*"
            && self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && self
                .output
                .last()
                .is_some_and(|line| line.trim_end().ends_with("*)"))
        {
            self.current.push_str(operator);
            return;
        }
        self.record_declaration_frame_for_pointer(operator, next);
        let align = self.resolved_pointer_align(operator);
        if matches!(next, None | Some(Token::Newline)) {
            match align {
                PointerAlign::None => {
                    self.emit_source_space();
                    self.current.push_str(operator);
                }
                PointerAlign::Type => {
                    self.trim_current_end_horizontal_space();
                    self.current.push_str(operator);
                }
                PointerAlign::Middle | PointerAlign::Name => {
                    self.emit_source_space_or_ensure();
                    self.current.push_str(operator);
                }
            }
            return;
        }

        match align {
            PointerAlign::Type => {
                if self.previous == PreviousToken::Comma {
                    if self
                        .token_input
                        .previous_input_whitespace
                        .as_ref()
                        .is_some_and(|whitespace| !whitespace.is_empty())
                    {
                        self.emit_source_space();
                    } else if self.options.pad_commas || self.options.pad_operators {
                        self.ensure_space();
                    }
                    self.current.push_str(operator);
                    if let Some(gap) = self.pointer_run.trailing_ws.clone()
                        && !gap.is_empty()
                    {
                        self.current.push_str(&gap);
                    } else if !matches!(next, Some(Token::Symbol(')' | ','))) {
                        self.ensure_space();
                    }
                } else {
                    self.trim_current_end();
                    self.current.push_str(operator);
                    if self.pointer_run.next_is_name_like
                        || self.pointer_run.followed_by_comment
                        || matches!(
                            next,
                            Some(Token::Operator(operator)) if operator == "="
                        )
                        || matches!(next, Some(Token::Comment(_, _)))
                    {
                        let gap = self.consolidated_pointer_gap();
                        self.current.push_str(&gap);
                    } else if !matches!(next, Some(Token::Symbol(')' | ','))) {
                        self.ensure_space();
                    }
                }
            }
            PointerAlign::Middle => {
                let closes_unnamed_type = matches!(
                    next,
                    None | Some(Token::Newline) | Some(Token::Symbol(')' | ','))
                ) || matches!(
                    next,
                    Some(Token::Operator(next_operator)) if next_operator.starts_with('>')
                );
                if closes_unnamed_type {
                    self.trim_current_end();
                    self.ensure_space();
                    self.current.push_str(operator);
                    if let Some(gap) = self.pointer_run.trailing_ws.clone() {
                        if self.options.convert_tabs && gap.contains('\t') {
                            let after_column =
                                self.token_input.token_source_column + self.pointer_run.star_count;
                            let width = visual_width_from(
                                &gap,
                                after_column,
                                self.options.tab_width.max(1),
                            );
                            self.current.push_str(&" ".repeat(width));
                        } else {
                            self.current.push_str(&gap);
                        }
                    }
                } else if is_after_scope_resolution {
                    if followed_by_reference {
                        self.ensure_space();
                    }
                    self.current.push_str(operator);
                    self.ensure_space();
                } else if self.current.trim().is_empty()
                    && self.token_input.token_begins_source_line
                {
                    self.current.push_str(operator);
                    if !matches!(next, Some(Token::Symbol(')' | ','))) {
                        self.ensure_space();
                    }
                } else {
                    self.trim_current_end();
                    let (before, after) = if matches!(next, Some(Token::Comment(_, _)))
                        && !self.options.convert_tabs
                    {
                        let before = self
                            .token_input
                            .previous_input_whitespace
                            .clone()
                            .unwrap_or_default();
                        let after = self.pointer_run.trailing_ws.clone().unwrap_or_default();
                        (
                            if before.is_empty() {
                                " ".to_string()
                            } else {
                                before
                            },
                            if after.is_empty() {
                                " ".to_string()
                            } else {
                                after
                            },
                        )
                    } else {
                        self.middle_pointer_gaps()
                    };
                    self.current.push_str(&before);
                    self.current.push_str(operator);
                    self.current.push_str(&after);
                }
            }
            PointerAlign::Name => {
                if matches!(next, Some(Token::Comment(_, _))) {
                    self.emit_source_space_or_ensure();
                } else if self.current.trim_end().ends_with('&')
                    && (operator == "*"
                        || operator == "&"
                            && (!self.token_input.previous_input_was_adjacent
                                || self
                                    .token_input
                                    .previous_input_whitespace
                                    .as_ref()
                                    .is_some_and(|whitespace| !whitespace.is_empty())))
                {
                    self.trim_current_end();
                    self.ensure_space();
                } else if matches!(
                    next,
                    Some(Token::Operator(next_operator)) if next_operator == "&"
                ) && !self.current.trim_end().ends_with('(')
                    && !is_after_scope_resolution
                    && !self.looks_like_pointer_declaration_context()
                {
                    self.trim_current_end();
                } else if (self.current.trim_end().ends_with('(') || is_after_scope_resolution)
                    && followed_by_reference
                {
                    self.ensure_space();
                } else if self.current.trim_end().ends_with(operator)
                    && self
                        .token_input
                        .previous_input_whitespace
                        .as_ref()
                        .is_some_and(|whitespace| !whitespace.is_empty())
                {
                    self.trim_current_end();
                    let gap = self.consolidated_pointer_gap();
                    self.current.push_str(&gap);
                } else if !is_after_scope_resolution
                    && !self.current.ends_with('(')
                    && !self.current.trim_end().ends_with('*')
                    && !self.current.trim_end().ends_with('&')
                    && !self.current.trim_end().ends_with('^')
                {
                    if matches!(next, Some(Token::Operator(next_operator)) if next_operator == "=")
                    {
                        self.trim_current_end();
                        let gap = self.consolidated_pointer_gap();
                        let before_len = gap.chars().count().saturating_sub(1).max(1);
                        self.current.push_str(&" ".repeat(before_len));
                        self.current.push_str(operator);
                        self.ensure_space();
                        return;
                    } else if followed_by_reference && !self.pointer_run.reference_has_name {
                        self.trim_current_end_horizontal_space();
                    } else if self.pointer_run.next_is_name_like {
                        self.trim_current_end();
                        let gap = if matches!(next, Some(Token::Symbol('('))) {
                            match self.token_input.previous_input_whitespace.as_deref() {
                                Some(gap) if !gap.is_empty() => gap.to_string(),
                                _ => " ".to_string(),
                            }
                        } else {
                            self.consolidated_pointer_gap()
                        };
                        self.current.push_str(&gap);
                    } else {
                        self.ensure_space();
                    }
                }
                self.current.push_str(operator);
                if matches!(next, Some(Token::Comment(_, _))) {
                    self.emit_trailing_source_space();
                }
                if matches!(next, Some(Token::Operator(next_operator)) if next_operator == operator)
                {
                    self.trim_current_end();
                }
                if matches!(next, Some(Token::Symbol('(')))
                    && self.function_pointer_parameter_keeps_space_before_name_group()
                {
                    if self.function_pointer_parameter_name_group_uses_space() {
                        self.ensure_space();
                    } else {
                        self.emit_trailing_source_space();
                    }
                }
            }
            PointerAlign::None => {
                self.emit_source_space();
                self.current.push_str(operator);
                self.emit_trailing_source_space();
            }
        }
        if matches!(next, Some(Token::Symbol(')')))
            && self.options.convert_tabs
            && let Some(gap) = self.pointer_run.trailing_ws.clone()
            && gap.contains('\t')
        {
            self.trim_current_end_horizontal_space();
            let after_column = self.token_input.token_source_column + self.pointer_run.star_count;
            let width = visual_width_from(&gap, after_column, self.options.tab_width.max(1));
            self.current.push_str(&" ".repeat(width));
        }
        if is_after_scope_resolution && !next_is_adjacent {
            self.ensure_space();
        }
    }

    pub(super) fn function_pointer_parameter_keeps_space_before_name_group(&self) -> bool {
        if !self.current_paren_context_is_declaration()
            && !self.is_function_declaration_parameter_continuation()
        {
            return false;
        }
        self.function_pointer_parameter_type_words()
            .is_some_and(|words| {
                words.iter().any(|word| {
                    matches!(*word, "struct" | "union" | "enum" | "const" | "volatile")
                        || is_type_like_pointer_word(word)
                        || word.chars().next().is_some_and(is_identifier_start)
                })
            })
    }

    fn function_pointer_parameter_name_group_uses_space(&self) -> bool {
        self.options.pad_operators
            && self
                .function_pointer_parameter_type_words()
                .is_some_and(|words| !(words.len() == 1 && words[0] == "void"))
    }

    fn function_pointer_parameter_type_words(&self) -> Option<Vec<&str>> {
        let current = self.current.trim_end();
        if !current.ends_with(['*', '&', '^']) {
            return None;
        }
        let segment = current
            .rfind(['(', ',', ';', '{', '}'])
            .map_or(current, |index| &current[index + 1..])
            .trim_end();
        let before_operator = segment.trim_end_matches(['*', '&', '^']);
        Some(
            before_operator
                .split(|ch: char| !is_identifier_continue(ch))
                .filter(|word| !word.is_empty())
                .collect(),
        )
    }

    fn middle_pointer_gaps(&self) -> (String, String) {
        let before = self
            .token_input
            .previous_input_whitespace
            .as_deref()
            .unwrap_or("");
        let after = self.pointer_run.trailing_ws.as_deref().unwrap_or("");
        if self.options.convert_tabs
            && (before.contains('\t') || after.contains('\t'))
            && let Some(before_column) = self.pointer_run.gap_before_column
        {
            let before_width = self
                .token_input
                .token_source_column
                .saturating_sub(before_column);
            let after_column = self.token_input.token_source_column + self.pointer_run.star_count;
            let after_width = visual_width_from(after, after_column, self.options.tab_width.max(1));
            let gap = (before_width + after_width).max(2);
            let before_pad = gap.div_ceil(2);
            return (" ".repeat(before_pad), " ".repeat(gap - before_pad));
        }
        if (before.contains('\t') || after.contains('\t')) && !self.options.convert_tabs {
            let mut gap: Vec<char> = before.chars().chain(after.chars()).collect();
            while gap.len() < 2 {
                gap.push(' ');
            }
            let before_pad = gap.len().div_ceil(2);
            return (
                gap[..before_pad].iter().collect(),
                gap[before_pad..].iter().collect(),
            );
        }
        let gap = (before.chars().count() + after.chars().count()).max(2);
        let before_pad = gap.div_ceil(2);
        (" ".repeat(before_pad), " ".repeat(gap - before_pad))
    }

    pub(super) fn consolidated_pointer_gap(&self) -> String {
        let before = self
            .token_input
            .previous_input_whitespace
            .as_deref()
            .unwrap_or("");
        let after = self.pointer_run.trailing_ws.as_deref().unwrap_or("");
        if before == " " && after == " " {
            return " ".to_string();
        }
        if before.is_empty() && after.is_empty() {
            return " ".to_string();
        }
        if self.options.convert_tabs
            && (before.contains('\t') || after.contains('\t'))
            && let Some(before_column) = self.pointer_run.gap_before_column
        {
            let tab_width = self.options.tab_width.max(1);
            let before_width = self
                .token_input
                .token_source_column
                .saturating_sub(before_column);
            let after_column = self.token_input.token_source_column + self.pointer_run.star_count;
            let after_width = visual_width_from(after, after_column, tab_width);
            return " ".repeat(before_width + after_width);
        }
        format!("{before}{after}")
    }

    pub(super) fn resolved_pointer_align(&self, operator: &str) -> PointerAlign {
        if operator.starts_with('&') {
            match self.options.reference_align {
                ReferenceAlign::None => PointerAlign::None,
                ReferenceAlign::Type => PointerAlign::Type,
                ReferenceAlign::Middle => PointerAlign::Middle,
                ReferenceAlign::Name => PointerAlign::Name,
                ReferenceAlign::SameAsPointer => self.options.pointer_align,
            }
        } else {
            self.options.pointer_align
        }
    }

    pub(super) fn is_unary_pointer_operator(&self) -> bool {
        if self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && self.previous == PreviousToken::CloseParen
            && self
                .output
                .last()
                .is_some_and(|line| line.trim_end().ends_with("*)"))
        {
            return true;
        }
        if matches!(
            trailing_word(&self.current),
            "return" | "case" | "else" | "delete" | "do"
        ) {
            return true;
        }
        match self.previous {
            PreviousToken::Word
            | PreviousToken::Literal
            | PreviousToken::CloseParen
            | PreviousToken::CloseBracket => false,
            PreviousToken::Operator => {
                let current = self.current.trim_end();
                current.ends_with('=')
                    || current.ends_with('+')
                    || current.ends_with('-')
                    || current.ends_with('!')
                    || current.ends_with('~')
                    || current.ends_with('*')
                    || current.ends_with('&')
                    || current.ends_with('^')
                    || current.ends_with('>')
                    || current.ends_with('<')
                    || current.ends_with('?')
                    || current.ends_with(':')
                    || current.ends_with("&&")
                    || current.ends_with("||")
            }
            _ => true,
        }
    }

    fn current_in_objc_method_type_group(&self) -> bool {
        if !(self.is_objc_method_line() || self.objc.method_continuation) {
            return false;
        }
        let current = self.current.trim_end();
        let Some(open) = current.rfind('(') else {
            return false;
        };
        current[open + 1..].find(')').is_none()
            && current[..open].rfind(':').is_some_and(|colon| colon < open)
    }

    pub(super) fn current_in_parenthesized_type_operand(&self) -> bool {
        let current = self.current.trim_end();
        let Some(open) = current.rfind('(') else {
            return false;
        };
        if current[open + 1..].contains(')')
            || !matches!(
                trailing_word(current[..open].trim_end()),
                "sizeof" | "alignof" | "_Alignof" | "typeid"
            )
        {
            return false;
        }
        is_pointer_declaration_segment(current[open + 1..].trim())
    }

    pub(super) fn current_in_cast_type_group(&self) -> bool {
        let current = self.current.trim_end();
        let Some(open) = current.rfind('(') else {
            return false;
        };
        if current[open + 1..].contains(')') {
            return false;
        }
        let before = current[..open].trim_end();
        if before.ends_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_' || ch == ')') {
            return false;
        }
        let segment = current[open + 1..].trim();
        !segment.is_empty()
            && segment
                .split_whitespace()
                .all(|word| word.chars().next().is_some_and(is_identifier_start))
    }
}

/// Drops C++ `[[...]]` attribute specifiers so a declaration prefixed by an
/// attribute is still classified by its type, not rejected for the brackets.
/// Single-bracket subscripts like `a[i]` are left intact.
fn strip_balanced_attributes(segment: &str) -> String {
    if !segment.contains("[[") {
        return segment.to_string();
    }
    let bytes = segment.as_bytes();
    let mut result = String::with_capacity(segment.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'[' && bytes.get(index + 1) == Some(&b'[') {
            let mut depth = 0usize;
            while index < bytes.len() {
                match bytes[index] {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            index += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
        } else {
            let ch = segment[index..]
                .chars()
                .next()
                .expect("byte index on char boundary");
            result.push(ch);
            index += ch.len_utf8();
        }
    }
    result
}

fn strip_balanced_angles(segment: &str) -> String {
    if !segment.contains('<') {
        return segment.to_string();
    }
    let mut depth: i32 = 0;
    for ch in segment.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return segment.to_string();
        }
    }
    if depth != 0 {
        return segment.to_string();
    }
    let mut result = String::with_capacity(segment.len());
    let mut depth = 0u32;
    for ch in segment.chars() {
        match ch {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

fn has_unclosed_balanced_delimiter(text: &str, open: &str, close: &str) -> bool {
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < text.len() {
        let rest = &text[index..];
        if rest.starts_with(open) {
            depth += 1;
            index += open.len();
        } else if rest.starts_with(close) {
            depth = depth.saturating_sub(1);
            index += close.len();
        } else if let Some(ch) = rest.chars().next() {
            index += ch.len_utf8();
        } else {
            break;
        }
    }
    depth > 0
}

fn strip_balanced_parens(segment: &str) -> String {
    let mut result = String::with_capacity(segment.len());
    let mut depth = 0u32;
    for ch in segment.chars() {
        match ch {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

pub(super) fn is_pointer_declaration_segment(segment: &str) -> bool {
    let stripped = strip_balanced_attributes(segment);
    let stripped = strip_balanced_angles(&stripped);
    let segment = stripped.as_str();
    if segment.chars().any(|ch| {
        matches!(
            ch,
            '=' | '+' | '-' | '/' | '%' | '?' | '!' | '~' | '|' | '^' | '<' | '>' | ']' | ')'
        )
    }) {
        return false;
    }
    if segment.contains(':') && segment.replace("::", "").contains(':') {
        return false;
    }
    let mut words = segment
        .split(|ch: char| !is_identifier_continue(ch))
        .filter(|word| !word.is_empty());
    let Some(first) = words.next() else {
        return false;
    };
    if first.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        return false;
    }
    !matches!(
        first,
        "return" | "case" | "sizeof" | "delete" | "new" | "throw" | "else"
    ) && !language::is_header(first)
}
