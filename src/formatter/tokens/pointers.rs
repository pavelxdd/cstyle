use crate::config::{FormatOptions, PointerAlign, ReferenceAlign};
use crate::formatter::braces::classification::is_class_like_brace_type;
use crate::formatter::constructs::headers::is_header;
use crate::formatter::constructs::switch_cases::{is_case_label_start, is_default_label_start};
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::frame::{DeclarationFrame, PointerRole};
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::structure::blocks::next_code_token;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::language::{
    is_macro_like_word, is_non_type_keyword, is_pointer_type_word, is_type_like_pointer_word,
};
use crate::formatter::syntax::{
    function_head_has_assignment, function_name_start, language, scoped_name_is_constructor,
};
use crate::formatter::text::columns::visual_width_from;
use crate::formatter::text::line_scan::{
    last_unmatched_open_delimiter, trailing_comment_split_limit, trailing_matching_parens,
};
use crate::formatter::tokens::operators::{
    head_ends_assignment_operator, head_ends_binary_operator,
};
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct PointerRunState {
    pub(crate) trailing_ws: Option<String>,
    pub(crate) next_is_name_like: bool,
    pub(crate) followed_by_reference: bool,
    pub(crate) reference_has_name: bool,
    pub(crate) followed_by_comment: bool,
    pub(crate) star_count: usize,
    pub(crate) gap_before_column: Option<usize>,
    pub(super) skip_adjacent_pointer_operators: usize,
    pub(crate) template_close_before_current: bool,
    /// A `*` aligned off its name before a parenthesized declarator, as in
    /// `char* (*get)(void)`, keeps a space before the paren.
    pub(crate) spaces_declarator_group: bool,
}

pub(crate) fn pointer_next_is_name_like(next: Option<&Token>) -> bool {
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
        self.layout.frame_stack.push_declaration(DeclarationFrame {
            pointer_role: self.pointer_role(operator, next),
            continuation_anchor_column: None,
            closing_anchor_column: None,
            is_typedef: self.current.trim_start().starts_with("typedef "),
        });
    }

    pub(super) fn is_rvalue_reference_like(&self, next: Option<&Token>) -> bool {
        if self.continues_operator_expression()
            && matches!(
                self.layout.previous,
                PreviousToken::Word
                    | PreviousToken::Literal
                    | PreviousToken::CloseParen
                    | PreviousToken::CloseBracket
            )
        {
            return false;
        }
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
        if (self.layout.command_state.current_header.is_some() && previous_word != language::AUTO)
            || (self.layout.nesting.paren_depth > 0
                && self
                    .layout
                    .nesting
                    .brace_type_stack
                    .last()
                    .is_some_and(|brace_type| *brace_type == BraceType::Command))
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
        // Right after `[` a `*` or `&` starts an index expression.
        if self.layout.previous == PreviousToken::OpenBracket && matches!(operator, "*" | "&") {
            return false;
        }
        // An increment or decrement after a logical operator is dereferenced.
        if self.layout.previous == PreviousToken::Operator
            && (self.current.trim_end().ends_with("&&") || self.current.trim_end().ends_with("||"))
            && matches!(next, Some(Token::Operator(step)) if step == "++" || step == "--")
        {
            return false;
        }
        // A run of stars right before `)` ends a type, as in `(u8**)`.
        if operator == "*"
            && matches!(self.layout.previous, PreviousToken::Word)
            && (matches!(next, Some(Token::Symbol(')')))
                || matches!(next, Some(Token::Operator(next)) if next == "*")
                    && self.current.active_token().is_some_and(|index| {
                        let tokens = &self.tree.tokens;
                        tokens
                            .get(index + 1..)
                            .and_then(|rest| {
                                rest.iter().find(|token| {
                                    !matches!(token, Token::Whitespace(_))
                                        && !matches!(token, Token::Operator(star) if star == "*")
                                })
                            })
                            .is_some_and(|token| matches!(token, Token::Symbol(')')))
                    }))
        {
            return true;
        }
        if self.continues_operator_expression()
            && matches!(
                self.layout.previous,
                PreviousToken::Word
                    | PreviousToken::Literal
                    | PreviousToken::CloseParen
                    | PreviousToken::CloseBracket
            )
        {
            return false;
        }
        if matches!(self.layout.previous, PreviousToken::Literal)
            || (self
                .current
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_digit() || ch == '\'' || ch == '"')
                && !self.looks_like_pointer_declaration_context())
        {
            return false;
        }
        if let Some(Token::Word(word)) = next
            && matches!(
                word.as_str(),
                "sizeof" | "return" | "case" | "new" | "delete" | "throw"
            )
            // A C name such as `new` ends a declarator.
            && !(matches!(word.as_str(), "new" | "delete")
                && self
                    .current
                    .active_token()
                    .filter(|&index| index < self.tree.tokens.len())
                    .and_then(|index| next_code_token(&self.tree.tokens, index + 1))
                    .and_then(|name| next_code_token(&self.tree.tokens, name + 1))
                    .is_some_and(|after| {
                        matches!(self.tree.tokens[after], Token::Symbol(';' | ',' | ')' | '['))
                            || matches!(&self.tree.tokens[after], Token::Operator(operator) if operator == "=")
                    }))
        {
            return false;
        }
        if self.pointer_run.template_close_before_current
            || self.pointer_in_template_type_context(next)
        {
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
        if self.layout.frame_stack.bracket_depth() > 0
            && matches!(
                self.layout.previous,
                PreviousToken::Word
                    | PreviousToken::Literal
                    | PreviousToken::CloseParen
                    | PreviousToken::CloseBracket
            )
        {
            return false;
        }
        if matches!(operator, "*" | "&")
            && self.layout.previous == PreviousToken::Operator
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
                self.layout.nesting.last_closed_brace_type,
                Some(
                    BraceType::Class
                        | BraceType::Struct
                        | BraceType::Union
                        | BraceType::Enum
                        | BraceType::Interface
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
                if self.layout.nesting.paren_depth > 0
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
        if self.layout.nesting.paren_depth > 0
            && matches!(next, Some(Token::Word(_)))
            && trailing_word(&self.current)
                .chars()
                .next()
                .is_some_and(is_identifier_start)
        {
            // A constant-like name after the operator is an operand.
            if matches!(next, Some(Token::Word(word)) if is_macro_like_word(word)) {
                return false;
            }
            if self.looks_like_pointer_declaration_context() {
                return true;
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
            && self.layout.nesting.paren_depth > 0
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

    fn continues_operator_expression(&self) -> bool {
        self.output.last_line_outside_comment().is_some_and(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            if head_ends_assignment_operator(code) {
                return true;
            }
            // `>=` is always a comparison; a trailing `>` whose line has a
            // `<` is usually a template close, not an expression break.
            if code.ends_with(">=") {
                return true;
            }
            if code.ends_with('>') && code.contains('<') {
                return false;
            }
            let trimmed = code.trim_start();
            let ends_single_word_label = trimmed.split_once(':').is_some_and(|(label, rest)| {
                rest.is_empty()
                    && !label.is_empty()
                    && label.trim_end().chars().all(is_identifier_continue)
            });
            let ends_ternary_colon = code.ends_with(':')
                && !is_case_label_start(trimmed)
                && !is_default_label_start(trimmed)
                && (self.layout.frame_stack.last_ternary_with_colon().is_some()
                    || !ends_single_word_label);
            head_ends_binary_operator(code)
                || ["==", "!=", "<=", "<", ">", "&&", "||", "?"]
                    .iter()
                    .any(|operator| code.ends_with(operator))
                || ends_ternary_colon
        })
    }

    fn pointer_in_template_type_context(&self, next: Option<&Token>) -> bool {
        if self.layout.line_state.template_angle_depth == 0 {
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

    fn current_ends_named_cast_type_argument(&self) -> bool {
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

    fn current_ends_type_group(&self) -> bool {
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

    pub(crate) fn looks_like_pointer_declaration_context(&self) -> bool {
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
                    if let Some(previous) = saved_starts.pop() {
                        segment_start = previous;
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
        if self.layout.nesting.paren_depth == 0 {
            return true;
        }
        self.current_paren_context_is_declaration()
            || self.current_paren_context_is_constructor_declaration(segment)
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
        if let Some(declaration) = self.tree_declaration_context() {
            return declaration;
        }
        for line in self.output.scoped().iter().rev().take(8) {
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
            return !return_type.is_empty() && !name.is_empty() && !is_header(self.options, name);
        }
        false
    }

    fn is_function_pointer_parameter_continuation(&self) -> bool {
        self.output.scoped().iter().rev().take(4).any(|line| {
            let trimmed = line.trim_end();
            trimmed.contains("(*") && !trimmed.ends_with(';') && !trimmed.ends_with('}')
        })
    }

    /// What the structure tree says about the parentheses around the token
    /// being pushed: `Some(true)` in a parameter list or a pointer
    /// declarator, `Some(false)` in other parentheses directly at
    /// declaration scope, and `None` where the tree does not decide (inside
    /// function bodies, in nested groups, or with no token being pushed).
    pub(super) fn tree_declaration_context(&self) -> Option<bool> {
        let tree = &self.tree;
        let group = tree.groups.enclosing(self.current.active_token()?)?;
        if tree.groups.get(group).delimiter != Delimiter::Paren {
            return None;
        }
        if tree.functions.is_parameter_list(group) || tree.functions.is_declarator(group) {
            return Some(true);
        }
        let at_declaration_scope = tree.groups.get(group).parent.is_none_or(|parent| {
            tree.groups.get(parent).delimiter == Delimiter::Brace
                && BlockKind::is_declaration_scope(tree.blocks.kind(parent))
        });
        at_declaration_scope.then_some(false)
    }

    /// A line continuing parentheses opened on an earlier line inside a
    /// function body holds an expression.
    pub(super) fn continues_expression_parens(&self) -> bool {
        if crate::formatter::text::line_scan::unmatched_open_paren_column(self.current.as_str())
            .is_some()
        {
            return false;
        }
        let tree = &self.tree;
        let Some(token) = self
            .current
            .active_token()
            .filter(|&token| token < tree.tokens.len())
        else {
            return false;
        };
        let Some(group) = tree.groups.enclosing(token) else {
            return false;
        };
        if tree.groups.get(group).delimiter != Delimiter::Paren
            || tree.functions.is_parameter_list(group)
            || tree.functions.is_declarator(group)
        {
            return false;
        }
        let mut parent = tree.groups.get(group).parent;
        while let Some(index) = parent {
            if tree.groups.get(index).delimiter == Delimiter::Brace {
                return !BlockKind::is_declaration_scope(tree.blocks.kind(index));
            }
            parent = tree.groups.get(index).parent;
        }
        false
    }

    pub(super) fn current_paren_context_is_declaration(&self) -> bool {
        if self.current_paren_is_lambda_parameter_list() {
            return true;
        }
        if let Some(declaration) = self.tree_declaration_context() {
            return declaration;
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
                    .scoped()
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
                self.paren_head_is_declaration(before)
                    || (!before.is_empty()
                        && !function_head_has_assignment(before)
                        && scoped_name_is_constructor(before))
            }
        }
    }

    pub(super) fn paren_head_is_declaration(&self, before: &str) -> bool {
        // A head continued past an escaped newline still names the function.
        let before = before
            .trim_end()
            .strip_suffix('\\')
            .map_or(before, str::trim_end);
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
        if name.is_empty() || is_header(self.options, name) || is_non_type_keyword(name) {
            return false;
        }
        if return_type.is_empty() {
            return self
                .layout
                .nesting
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| is_class_like_brace_type(*brace_type));
        }
        // A head inside an open paren is a call's argument.
        if return_type.contains(['.', '[', ']'])
            || has_unclosed_balanced_delimiter(return_type, "(", ")")
        {
            return false;
        }
        let last_type_word = return_type
            .rsplit(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty());
        !last_type_word.is_some_and(is_non_type_keyword)
    }

    /// Head text of the open paren that encloses the current parameter
    /// continuation line.
    fn enclosing_open_paren_head(&self) -> Option<&str> {
        for line in self.output.scoped().iter().rev().take(8) {
            let trimmed = line.trim_end();
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
            if !has_unclosed_balanced_delimiter(trimmed, "(", ")") {
                continue;
            }
            let open = trimmed.find('(')?;
            return Some(trimmed[..open].trim_end());
        }
        None
    }

    pub(crate) fn current_paren_is_lambda_parameter_list(&self) -> bool {
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

    fn current_paren_context_is_constructor_declaration(&self, segment: &str) -> bool {
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
                    resolved_pointer_align(self.options, operator),
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
        // The rest of a run whose first star went out alone joins it.
        if continues_sequence
            && self.pointer_run.star_count > 1
            && self.current.trim_end().ends_with(operator)
        {
            self.pointer_run.skip_adjacent_pointer_operators = self.pointer_run.star_count - 1;
            self.trim_current_end_horizontal_space();
            self.current
                .push_str(&operator.repeat(self.pointer_run.star_count));
            return;
        }
        if continues_sequence && self.pointer_run.star_count > 1 {
            self.pointer_run.skip_adjacent_pointer_operators = self.pointer_run.star_count - 1;
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
            match resolved_pointer_align(self.options, operator) {
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
            let align = resolved_pointer_align(self.options, operator);
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
        let align = resolved_pointer_align(self.options, operator);
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
                self.push_type_aligned_pointer(operator, next);
                self.pointer_run.spaces_declarator_group =
                    self.pointer_spaces_declarator_group(operator, next);
            }
            PointerAlign::Middle => {
                self.push_middle_aligned_pointer(
                    operator,
                    next,
                    followed_by_reference,
                    is_after_scope_resolution,
                );
                self.pointer_run.spaces_declarator_group =
                    self.pointer_spaces_declarator_group(operator, next);
            }
            PointerAlign::Name => {
                if self.push_name_aligned_pointer(
                    operator,
                    next,
                    followed_by_reference,
                    is_after_scope_resolution,
                ) {
                    return;
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

    /// Whether the `*` just pushed starts a parenthesized declarator a space
    /// parts from it. astyle leaves the star of a `struct` type and of a
    /// conversion operator where it was.
    fn pointer_spaces_declarator_group(&self, operator: &str, next: Option<&Token>) -> bool {
        let current = self.current.trim_end();
        let Some(before) = current.strip_suffix(operator) else {
            return false;
        };
        let words = before
            .split(|ch: char| !is_identifier_continue(ch))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        matches!(next, Some(Token::Symbol('(')))
            && !words.iter().rev().take(2).any(|word| {
                *word == "operator"
                        // In a statement astyle leaves a tagged type's star
                        // where it is.
                        || matches!(*word, "struct" | "union" | "enum" | "class")
                            && self.in_statement_brace()
            })
    }

    /// In a statement astyle leaves a tagged type's star before a declarator
    /// group as written.
    fn push_tagged_type_star_before_group(&mut self, operator: &str, next: Option<&Token>) -> bool {
        let tagged = self
            .current
            .trim_end()
            .split(|ch: char| !is_identifier_continue(ch))
            .filter(|word| !word.is_empty())
            .rev()
            .take(2)
            .any(|word| matches!(word, "struct" | "union" | "enum" | "class"));
        if !(tagged && matches!(next, Some(Token::Symbol('('))) && self.in_statement_brace()) {
            return false;
        }
        self.emit_source_space();
        self.current.push_str(operator);
        self.emit_trailing_source_space();
        true
    }

    /// Whether the code is a statement or a parameter rather than a member
    /// or a declaration at file scope.
    fn in_statement_brace(&self) -> bool {
        self.layout.nesting.paren_depth > 0
            || !matches!(
                self.layout.nesting.brace_type_stack.last(),
                None | Some(
                    BraceType::Struct | BraceType::Class | BraceType::Union | BraceType::Interface
                )
            )
    }

    fn push_type_aligned_pointer(&mut self, operator: &str, next: Option<&Token>) {
        if self.layout.previous == PreviousToken::Comma {
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
            if self.push_tagged_type_star_before_group(operator, next) {
                return;
            }
            if matches!(next, Some(Token::Symbol('('))) && {
                self.current.push_str(operator);
                let multiply = self.function_pointer_parameter_keeps_space_before_name_group()
                    && self.function_pointer_star_follows_no_pointer_type();
                self.current.truncate(self.current.len() - operator.len());
                multiply
            } {
                if self.options.pad_operators {
                    self.ensure_space();
                    self.current.push_str(operator);
                    self.ensure_space();
                } else {
                    self.emit_source_space();
                    self.current.push_str(operator);
                    self.emit_trailing_source_space();
                }
                return;
            }
            self.trim_current_end();
            self.current.push_str(operator);
            if self.pointer_run.next_is_name_like
                || self.pointer_run.followed_by_comment
                || matches!(
                    next,
                    Some(Token::Operator(operator)) if operator == "="
                )
                || matches!(next, Some(Token::Comment(_, _)))
                || matches!(next, Some(Token::Symbol('(')))
                    && self.pointer_spaces_declarator_group(operator, next)
            {
                let gap = self.consolidated_pointer_gap();
                self.current.push_str(&gap);
            } else if !matches!(next, Some(Token::Symbol(')' | ','))) {
                self.ensure_space();
            }
        }
    }

    fn push_middle_aligned_pointer(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        followed_by_reference: bool,
        is_after_scope_resolution: bool,
    ) {
        let closes_unnamed_type = matches!(
            next,
            None | Some(Token::Newline) | Some(Token::Symbol(')' | ','))
        ) || matches!(
            next,
            Some(Token::Operator(next_operator)) if next_operator.starts_with('>')
        );
        if self.push_tagged_type_star_before_group(operator, next) {
            return;
        }
        if closes_unnamed_type {
            self.trim_current_end();
            self.ensure_space();
            self.current.push_str(operator);
            if let Some(gap) = self.pointer_run.trailing_ws.clone() {
                if self.options.convert_tabs && gap.contains('\t') {
                    let after_column =
                        self.token_input.token_source_column + self.pointer_run.star_count;
                    let width =
                        visual_width_from(&gap, after_column, self.options.tab_width.max(1));
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
        } else if self.current.trim().is_empty() && self.token_input.token_begins_source_line {
            self.current.push_str(operator);
            if !matches!(next, Some(Token::Symbol(')' | ','))) {
                self.ensure_space();
            }
        } else {
            self.trim_current_end();
            let (before, after) =
                if matches!(next, Some(Token::Comment(_, _))) && !self.options.convert_tabs {
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

    fn push_name_aligned_pointer(
        &mut self,
        operator: &str,
        next: Option<&Token>,
        followed_by_reference: bool,
        is_after_scope_resolution: bool,
    ) -> bool {
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
            if matches!(next, Some(Token::Operator(next_operator)) if next_operator == "=") {
                self.trim_current_end();
                let gap = self.consolidated_pointer_gap();
                let before_len = gap.chars().count().saturating_sub(1).max(1);
                self.current.push_str(&" ".repeat(before_len));
                self.current.push_str(operator);
                self.ensure_space();
                return true;
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
        if matches!(next, Some(Token::Operator(next_operator)) if next_operator == operator) {
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
        false
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

    /// Padded operators space a `*` before `(` unless the word before it
    /// names a pointer type the way astyle knows them: `char`, `int`,
    /// `void`, or a `name_t`.
    fn function_pointer_parameter_name_group_uses_space(&self) -> bool {
        self.options.pad_operators && self.function_pointer_star_follows_no_pointer_type()
    }

    fn function_pointer_star_follows_no_pointer_type(&self) -> bool {
        self.function_pointer_parameter_type_words()
            .is_some_and(|words| {
                words.last().is_some_and(|word| {
                    !(matches!(*word, "char" | "int" | "void" | "INT" | "VOID")
                        || word.len() >= 6 && word.ends_with("_t"))
                })
            })
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

    fn consolidated_pointer_gap(&self) -> String {
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

    pub(super) fn is_unary_pointer_operator(&self) -> bool {
        if self.current.trim().is_empty()
            && self.token_input.token_begins_source_line
            && self.layout.previous == PreviousToken::CloseParen
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
        match self.layout.previous {
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
        if !(self.is_objc_method_line() || self.layout.objc.method_continuation) {
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

pub(crate) fn is_pointer_declaration_segment(segment: &str) -> bool {
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

pub(super) fn resolved_pointer_align(options: &FormatOptions, operator: &str) -> PointerAlign {
    if operator.starts_with('&') {
        match options.reference_align {
            ReferenceAlign::None => PointerAlign::None,
            ReferenceAlign::Type => PointerAlign::Type,
            ReferenceAlign::Middle => PointerAlign::Middle,
            ReferenceAlign::Name => PointerAlign::Name,
            ReferenceAlign::SameAsPointer => options.pointer_align,
        }
    } else {
        options.pointer_align
    }
}
