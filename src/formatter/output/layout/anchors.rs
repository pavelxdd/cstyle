//! Indents anchored in the structure tree.
//!
//! Layout heuristics read the text of earlier output lines, so comments,
//! continuation lines, and directives in between can mislead them. These
//! rules run last and take a statement's indent from lines that the tree
//! links it to: the `if` of an `else`, the header of a braceless body, the
//! previous statement of the block, and the block's opening.

use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, token_text};
use crate::formatter::output::model::LineLayout;
use crate::formatter::state::BraceType;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::text::columns::visual_width_from;
use crate::formatter::text::line_scan::preprocessor_directive;

impl FormatEngine<'_> {
    pub(crate) fn apply_tree_anchor_layout(
        &self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::SwitchLabel
            && let Some(first) = self.output.pending_tokens().map(|span| span.first)
            && let Some(spaces) = self.sibling_case_label_column(first)
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        if layout.line_kind != LineKind::Normal || line.trim_start().starts_with('#') {
            return layout;
        }
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return layout;
        };
        if let Some(spaces) = self
            .else_matching_if_indent(first)
            .or_else(|| self.braceless_body_indent(first))
            .or_else(|| self.split_else_if_indent(first))
            .or_else(|| self.statement_expression_indent(first))
            .or_else(|| self.return_value_indent(first))
            .or_else(|| self.ternary_arm_in_parens_indent(first))
            .or_else(|| self.assigned_string_continuation_indent(first))
            .or_else(|| self.comment_interrupted_continuation_indent(first))
            .or_else(|| self.initializer_row_indent(first))
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        let structural = layout.indent * self.options.indent_width;
        let sibling = self.sibling_statement_column(first);
        let block = self.block_body_column(first);
        let brace = self
            .closing_brace_indent(first)
            .or_else(|| self.opening_brace_indent(first));
        if sibling == Some(structural) || block == Some(structural) || brace == Some(structural) {
            // A heuristic moved the line off a structural level that the
            // tree confirms; an anchor off that level is itself misplaced.
            layout.exact_indent_spaces = Some(structural);
        } else if matches!(self.tree.tokens[first], Token::Symbol('{'))
            && sibling == Some(structural + self.options.indent_width)
        {
            // An indented block brace stands a level past its statement's.
            layout.exact_indent_spaces = sibling;
        }
        layout
    }

    /// An initializer element starting a line after the `,` that ends the
    /// element before it takes that element's column, when that element
    /// starts its own line; a value after a designator's `=` takes the
    /// designator's.
    fn initializer_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer) {
            return None;
        }
        let comma = self.tree.previous_code_token(first)?;
        if groups.enclosing(comma) != Some(group) {
            return None;
        }
        // A value after a designator's `=` at a line end stays at the
        // designator: astyle continues no `=` inside an initializer.
        if matches!(&tokens[comma], Token::Operator(operator) if operator == "=") {
            let line = self.output.line_with_token(comma)?;
            return (self.output.line_tokens(line)?.last == comma).then(|| {
                self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
            });
        }
        if !matches!(tokens[comma], Token::Symbol(',')) {
            return None;
        }
        // The engine places continuation rows from the row's own layout;
        // only a row that ends on its line, or at a designator's `=` whose
        // value line follows the row, moves alone.
        let close = groups.get(group).close?;
        let end = (first..close)
            .find(|&index| {
                matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group)
            })
            .unwrap_or(close);
        if let Some(newline) = (first..end).find(|&index| matches!(tokens[index], Token::Newline))
            && !self.tree.previous_code_token(newline).is_some_and(|last| {
                matches!(&tokens[last], Token::Operator(operator) if operator == "=")
                    && groups.enclosing(last) == Some(group)
            })
        {
            return None;
        }
        let open = groups.get(group).open;
        let separator = (open..comma).rev().find(|&index| {
            index == open
                || (matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group))
        })?;
        let element = next_code_token(tokens, separator + 1)?;
        // Styles that indent initializer braces move brace rows alone.
        let is_brace = |index: usize| matches!(tokens[index], Token::Symbol('{'));
        if is_brace(element) != is_brace(first) {
            return None;
        }
        let line = self.output.line_with_token(element)?;
        (self.output.line_tokens(line)?.first == element).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    /// Visual column of the code token `token` on its output line, found
    /// by walking the line's token texts in order.
    fn token_column(&self, token: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let line = self.output.line_with_token(token)?;
        let span = self.output.line_tokens(line)?;
        let text = &self.output.as_slice()[line];
        let mut position = 0;
        for (index, piece_token) in tokens.iter().enumerate().take(token + 1).skip(span.first) {
            if !is_code_token(piece_token) {
                continue;
            }
            let piece = token_text(piece_token);
            let offset = text.get(position..)?.find(piece.as_str())?;
            if index == token {
                return Some(visual_width_from(
                    &text[..position + offset],
                    0,
                    self.options.tab_width,
                ));
            }
            position += offset + piece.len();
        }
        None
    }

    /// A continuation line inside parentheses that comments separate from
    /// the code before it lays out as if they were not there: at the line
    /// continuing the same parentheses above, or at the parentheses'
    /// content, past a control condition's floor.
    fn comment_interrupted_continuation_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        if !self.continues_parentheses_past_comments(first) {
            return None;
        }
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        let previous_line = self.output.line_with_token(previous)?;
        let previous_first = self.output.line_tokens(previous_line)?.first;
        let column = if groups.enclosing(previous_first) == Some(group) {
            self.output
                .lead_width(previous_line, self.options.tab_width)
        } else {
            let open = groups.get(group).open;
            let content = self.token_column(next_code_token(tokens, open + 1)?)?;
            let condition = self.tree.previous_code_token(open).is_some_and(|keyword| {
                matches!(&tokens[keyword], Token::Word(word)
                    if matches!(word.as_str(), "if" | "while" | "for" | "switch"))
            });
            match self.control_condition_header_indent().filter(|_| condition) {
                Some(header) => content.max(header + min_conditional_indent_spaces(self.options)),
                None => content,
            }
        };
        Some(column + self.case_unindent_spaces())
    }

    /// A string literal continuing an assigned value's adjacent literals
    /// from the line before stands at the value's first token.
    fn assigned_string_continuation_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let is_string = |index: usize| matches!(tokens[index], Token::StringLiteral(_));
        let previous = self.tree.previous_code_token(first)?;
        if !is_string(first) || !is_string(previous) {
            return None;
        }
        // Only statements align to the value: astyle continues no `=`
        // inside an initializer, and declarations at file scope keep their
        // continuation indent.
        let group = Some(groups.enclosing(first)?);
        if group.is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer)) {
            return None;
        }
        let mut index = previous;
        let assign = loop {
            index = self.tree.previous_code_token(index)?;
            if groups.enclosing(index) != group {
                return None;
            }
            match &tokens[index] {
                Token::Operator(operator) if operator == "=" => break index,
                Token::StringLiteral(_) | Token::Word(_) => {}
                _ => return None,
            }
        };
        // The value's line is published: the literal continues it.
        let value = next_code_token(tokens, assign + 1)?;
        Some(self.token_column(value)? + self.case_unindent_spaces())
    }

    /// Whether the code token `first` continues a control condition or a
    /// ternary inside parentheses past standalone comments or blank lines:
    /// astyle lays out other arguments after a comment its own way.
    fn continues_parentheses_past_comments(&self, first: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let Some(previous) = self.tree.previous_code_token(first) else {
            return false;
        };
        // Standalone comments or blank lines: tokens on lines of their own.
        let between: Vec<&Token> = tokens[previous + 1..first]
            .iter()
            .filter(|token| !matches!(token, Token::Whitespace(_)))
            .collect();
        let standalone_comment = between.windows(2).any(|pair| {
            matches!(pair[0], Token::Newline)
                && matches!(pair[1], Token::Comment(_, _) | Token::Newline)
        });
        let Some(group) = groups.enclosing(first) else {
            return false;
        };
        standalone_comment
            && groups.get(group).delimiter == Delimiter::Paren
            && groups.enclosing(previous) == Some(group)
            && (self.control_condition_of(first).is_some()
                || matches!(tokens[previous], Token::Symbol('?' | ':')))
    }

    /// A ternary arm starting a line after a `:` inside parentheses takes
    /// the column of the parentheses' content.
    fn ternary_arm_in_parens_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        if groups.get(group).delimiter != Delimiter::Paren {
            return None;
        }
        let colon = self.tree.previous_code_token(first)?;
        let open = groups.get(group).open;
        if !matches!(tokens[colon], Token::Symbol(':'))
            || groups.enclosing(colon) != Some(group)
            || !(open + 1..colon).any(|index| {
                matches!(tokens[index], Token::Symbol('?'))
                    && groups.enclosing(index) == Some(group)
            })
        {
            return None;
        }
        // A control condition keeps its own floor.
        if self.control_condition_header_indent().is_some() {
            return None;
        }
        // An arm under a line continuing the same parentheses takes its
        // column, as capped continuations do.
        let previous_line = self.output.line_with_token(colon)?;
        let previous_first = self.output.line_tokens(previous_line)?.first;
        if groups.enclosing(previous_first) == Some(group) {
            return Some(
                self.output
                    .lead_width(previous_line, self.options.tab_width)
                    + self.case_unindent_spaces(),
            );
        }
        let content = next_code_token(tokens, open + 1)?;
        Some(self.token_column(content)? + self.case_unindent_spaces())
    }

    /// A `return` value starting a line after the keyword takes one level
    /// past the keyword's line, past comments and directives between.
    fn return_value_indent(&self, first: usize) -> Option<usize> {
        let keyword = self.tree.previous_code_token(first)?;
        if !matches!(&self.tree.tokens[keyword], Token::Word(word) if word == "return") {
            return None;
        }
        let line = self.line_led_by(keyword)?;
        (self.output.line_tokens(line)?.last == keyword).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces()
        })
    }

    /// Column of the case label before the one at `first` in the same
    /// switch body, from the line it leads.
    fn sibling_case_label_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let is_label = |index: usize| matches!(&tokens[index], Token::Word(word) if matches!(word.as_str(), "case" | "default"));
        if !is_label(first) {
            return None;
        }
        let body = groups.enclosing(first)?;
        let open = groups.get(body).open;
        let earlier = (open + 1..first).rev().find(|&index| {
            is_label(index)
                && groups.enclosing(index) == Some(body)
                && self.tree.statements.starts_block_statement(index)
        });
        let Some(earlier) = earlier else {
            return self.first_case_label_column(body);
        };
        let line = self.output.line_with_token(earlier)?;
        (self.output.line_tokens(line)?.first == earlier).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.layout.line_adjuster.pending_case_unindent() * self.options.indent_width
        })
    }

    /// Column of the first case label of the switch body `body`: its brace
    /// line, or the `switch` line for an attached brace, one level in
    /// where the style indents switches.
    fn first_case_label_column(&self, body: GroupId) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let open = self.tree.groups.get(body).open;
        let owner = self.tree.blocks.owner(body)?;
        // Indented switch braces take their labels after layout.
        if !matches!(&tokens[owner], Token::Word(word) if word == "switch")
            || self.should_indent_brace_line(BraceType::Command)
        {
            return None;
        }
        let brace_line = self.output.line_with_token(open)?;
        let line = if self.output.line_tokens(brace_line)?.first == open {
            brace_line
        } else {
            self.line_led_by(owner)?
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + (usize::from(self.options.indent_switches)
                    + self.layout.line_adjuster.pending_case_unindent())
                    * self.options.indent_width,
        )
    }

    /// Standalone comments right before the statement on output line `line`
    /// take its indent, as astyle indents a comment to the code it precedes;
    /// a comment in column one stays there unless the style indents those.
    /// Case labels wait for [`Self::align_comments_before_case_labels`].
    pub(crate) fn align_comments_before_statement(&mut self, line: usize) {
        self.align_comments_before(line, false);
    }

    /// Comments before `case` and `default` labels take the label's final
    /// indent: switch layouts move label lines after they are published.
    pub(crate) fn align_comments_before_case_labels(&mut self) {
        for line in 0..self.output.len() {
            self.align_comments_before(line, true);
        }
    }

    fn align_comments_before(&mut self, line: usize, case_labels: bool) {
        let tokens = &self.tree.tokens;
        let Some(first) = self.output.line_tokens(line).map(|span| span.first) else {
            return;
        };
        let statements = &self.tree.statements;
        // A continuation inside parentheses takes the comments inside them.
        let continues_parens = self.continues_parentheses_past_comments(first);
        if !statements.starts_block_statement(first)
            && statements.braceless_header(first).is_none()
            && !continues_parens
        {
            return;
        }
        let is_case_label = matches!(&tokens[first], Token::Word(word) if matches!(word.as_str(), "case" | "default"));
        if is_case_label != case_labels {
            return;
        }
        match &tokens[first] {
            Token::Symbol('{' | '}') => return,
            Token::Word(word) if word == "else" => return,
            _ if !is_case_label
                && next_code_token(tokens, first + 1)
                    .is_some_and(|next| matches!(tokens[next], Token::Symbol(':'))) =>
            {
                return;
            }
            _ => {}
        }
        // The code line may close a comment itself; a label line opening
        // with one keeps the comments before it.
        if self.output.comment_start_index(line) != line
            || (is_case_label && self.output.trimmed(line).starts_with("/*"))
        {
            return;
        }
        let code = &self.output.as_slice()[line];
        let prefix = code[..code.len() - code.trim_start().len()].to_string();
        let tab_width = self.options.tab_width.max(1);
        let mut end = line;
        while end > 0 {
            let index = end - 1;
            let text = self.output.trimmed(index);
            if text.is_empty() {
                end = index;
                continue;
            }
            if self.output.line_tokens(index).is_some() {
                break;
            }
            let start = self.output.comment_start_index(index);
            let opener = self.output.trimmed(start);
            if self.output.line_tokens(start).is_some()
                || !(opener.starts_with("//") || opener.starts_with("/*"))
                || opener.contains("*INDENT-")
                || self.comment_runs_into_brace(start)
            {
                break;
            }
            let lead = self.output.lead_width(start, tab_width);
            let opener_line = &self.output.as_slice()[start];
            let opener_prefix = &opener_line[..opener_line.len() - opener_line.trim_start().len()];
            if opener_prefix != prefix && (lead != 0 || self.options.indent_col1_comments) {
                for offset in start..=index {
                    let relative = self
                        .output
                        .lead_width(offset, tab_width)
                        .saturating_sub(lead);
                    let text = self.output.trimmed(offset).to_string();
                    let line = &mut self.output.range_mut(offset..offset + 1)[0];
                    *line = format!("{prefix}{}{text}", " ".repeat(relative));
                }
            }
            end = start;
        }
    }

    /// Whether `line`, being laid out, starts with a statement label such
    /// as `again:`, as the tree reads it: no `case`, `default`, or `::`.
    pub(crate) fn pending_line_is_label(&self, line: &str) -> bool {
        let tokens = &self.tree.tokens;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        let is_colon = |index: Option<usize>| {
            index.is_some_and(|index| matches!(tokens[index], Token::Symbol(':')))
        };
        let colon = next_code_token(tokens, first + 1);
        // The line may hold only what follows the label.
        matches!(&tokens[first], Token::Word(word)
        if !matches!(word.as_str(), "case" | "default")
            && line.trim_start().strip_prefix(word.as_str()).is_some_and(|rest| {
                rest.trim_start().starts_with(':')
            }))
            && is_colon(colon)
            && !is_colon(colon.and_then(|colon| next_code_token(tokens, colon + 1)))
    }

    /// Whether the line being laid out starts inside parentheses or
    /// brackets, as the tree reads it: a continuation, never a body.
    pub(crate) fn pending_line_in_parens(&self) -> bool {
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.enclosing(span.first))
            .is_some_and(|group| groups.get(group).delimiter != Delimiter::Brace)
    }

    /// Whether the line being laid out starts an argument with a unary
    /// operator, as in `&x` after a comma inside parentheses.
    pub(crate) fn pending_line_starts_unary_operator(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        matches!(tokens[first], Token::Operator(_))
            && self.pending_line_in_parens()
            && self
                .tree
                .previous_code_token(first)
                .is_some_and(|previous| matches!(tokens[previous], Token::Symbol(',' | '(')))
    }

    /// Whether the line being laid out starts in the same parentheses, as
    /// the tree reads them, as the last code line before it: braces aside.
    pub(crate) fn pending_line_in_previous_line_group(&self) -> bool {
        let groups = &self.tree.groups;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return true;
        };
        let Some(previous) = (0..self.output.len())
            .rev()
            .find_map(|line| self.output.line_tokens(line))
        else {
            return true;
        };
        let parens = |index: usize| {
            groups
                .enclosing(index)
                .filter(|&group| groups.get(group).delimiter != Delimiter::Brace)
        };
        parens(first) == parens(previous.first)
    }

    /// Whether the line being laid out is a row of an initializer brace
    /// opened on an earlier line: astyle aligns no continuation to a `=`
    /// there.
    pub(crate) fn pending_row_in_initializer_brace(&self) -> bool {
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.enclosing(span.first))
            .filter(|&group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer))
            .and_then(|group| self.output.line_with_token(groups.get(group).open))
            .is_some()
    }

    /// Leading width of the line holding the control header whose condition
    /// the line being laid out continues, from the structure tree.
    pub(crate) fn control_condition_header_indent(&self) -> Option<usize> {
        let first = self.output.pending_tokens()?.first;
        let keyword = self.control_condition_of(first)?;
        let line = self.line_led_by(keyword)?;
        Some(self.output.lead_width(line, self.options.tab_width))
    }

    /// The control keyword whose condition holds the token `index`, through
    /// any parentheses nested in it.
    fn control_condition_of(&self, index: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let condition = groups
            .ancestors(groups.enclosing(index)?)
            .take_while(|&id| groups.get(id).delimiter == Delimiter::Paren)
            .last()?;
        let keyword = self.tree.previous_code_token(groups.get(condition).open)?;
        matches!(&tokens[keyword], Token::Word(word)
            if matches!(word.as_str(), "if" | "for" | "while" | "switch"))
        .then_some(keyword)
    }

    /// Whether the code before the line being laid out ends with the
    /// condition of a control statement, as the tree reads it; `None` when
    /// the line recorded no tokens.
    pub(crate) fn previous_code_closes_control_condition(&self) -> Option<bool> {
        let tokens = &self.tree.tokens;
        let first = self.output.pending_tokens()?.first;
        let previous = self.tree.previous_code_token(first)?;
        if !matches!(tokens[previous], Token::Symbol(')')) {
            return Some(false);
        }
        let group = self.tree.groups.closed_at(previous)?;
        let open = self.tree.groups.get(group).open;
        Some(self.tree.previous_code_token(open).is_some_and(|keyword| {
            matches!(&tokens[keyword], Token::Word(word)
                if matches!(word.as_str(), "if" | "for" | "while" | "switch" | "foreach" | "constexpr"))
        }))
    }

    /// Whether the last non-empty output line starts with a `}` that closes
    /// a block inside another block, from the structure tree, or, for a line
    /// that recorded no tokens, from the engine's open blocks.
    pub(crate) fn last_line_closes_nested_block(&self) -> bool {
        let Some(span) = self
            .output
            .last_non_empty_index()
            .and_then(|index| self.output.line_tokens(index))
        else {
            return !self.layout.nesting.brace_type_stack.is_empty();
        };
        let groups = &self.tree.groups;
        matches!(self.tree.tokens[span.first], Token::Symbol('}'))
            && groups
                .closed_at(span.first)
                .and_then(|group| groups.get(group).parent)
                .is_some_and(|parent| self.tree.blocks.kind(parent).is_some())
    }

    /// Case-block unindent the line being laid out will lose; indents from
    /// published lines have lost theirs already.
    pub(crate) fn case_unindent_spaces(&self) -> usize {
        self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
    }

    /// An `else` starting a line, alone or after the `}` closing the `if`
    /// body, takes the indent of the line holding its `if`.
    fn else_matching_if_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let is_else = |index: usize| matches!(&tokens[index], Token::Word(word) if word == "else");
        let else_token = if is_else(first) {
            first
        } else if matches!(tokens[first], Token::Symbol('}')) {
            let next = next_code_token(tokens, first + 1)?;
            let span = self.output.pending_tokens()?;
            (is_else(next) && span.contains(next)).then_some(next)?
        } else {
            return None;
        };
        let if_line = self.line_led_by(self.tree.statements.if_of_else(else_token)?)?;
        Some(self.output.lead_width(if_line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// A `}` closing a statement block lines up with the line holding its
    /// `{`.
    fn closing_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(tokens[first], Token::Symbol('}')) {
            return None;
        }
        let groups = &self.tree.groups;
        let group = groups.closed_at(first)?;
        if !matches!(
            self.tree.blocks.kind(group),
            Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
        ) {
            return None;
        }
        let open = groups.get(group).open;
        let owner = self.tree.blocks.owner(group)?;
        if matches!(&tokens[owner], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
        {
            return None;
        }
        // A brace pair split across conditional branches closes elsewhere.
        if tokens[open..first].iter().any(|token| {
            matches!(token, Token::Preprocessor(directive)
                if matches!(preprocessor_directive(&directive.text), Some("else" | "elif")))
        }) {
            return None;
        }
        let open_line = self.output.line_with_token(open)?;
        let statements = &self.tree.statements;
        let open_line_first = self.output.line_tokens(open_line)?.first;
        // Code after the `{` on its line makes no block layout to follow.
        if open_line_first != open
            && next_code_token(tokens, open + 1)
                .is_some_and(|next| self.output.line_with_token(next) == Some(open_line))
        {
            return None;
        }
        let brace_type = if self.tree.blocks.kind(group) == Some(BlockKind::FunctionBody) {
            BraceType::Definition
        } else {
            BraceType::Command
        };
        // A style indenting braces closes an attached block at its body.
        let mut extra = 0;
        let line = if open_line_first == open {
            open_line
        } else {
            if self.should_indent_brace_line(brace_type) {
                extra = self.options.indent_width;
            }
            // The brace's line starts the statement, or a continuation of the
            // header does and the header's line does.
            if statements.starts_block_statement(open_line_first)
                || statements.braceless_header(open_line_first).is_some()
            {
                open_line
            } else if let Some(keyword) = self.header_keyword_before(open) {
                let line = self.line_led_by(keyword)?;
                // `else while (x) {` nests two headers on one line.
                if !matches!(&tokens[keyword], Token::Word(word) if word == "if")
                    && self
                        .tree
                        .previous_code_token(keyword)
                        .is_some_and(|previous| {
                            matches!(&tokens[previous], Token::Word(word) if word == "else")
                                && self.output.line_with_token(previous) == Some(line)
                        })
                {
                    extra += self.options.indent_width;
                }
                line
            } else {
                let mut owner = owner;
                while matches!(&tokens[owner], Token::Word(word) if word == "else") {
                    let next = next_code_token(tokens, owner + 1)?;
                    if next == open {
                        break;
                    }
                    // `else while (x) {` nests two headers on one line.
                    if !matches!(&tokens[next], Token::Word(word) if word == "if")
                        && self.output.line_with_token(owner) == self.output.line_with_token(next)
                    {
                        extra += self.options.indent_width;
                    }
                    owner = next;
                }
                self.line_led_by(owner)?
            }
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + extra
                + self.case_unindent_spaces(),
        )
    }

    /// A `{` on its own line after a control header stands at the header's
    /// line, one level in where the style indents blocks.
    fn opening_brace_indent(&self, first: usize) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol('{')) {
            return None;
        }
        let group = self.tree.groups.opened_at(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Control) {
            return None;
        }
        // Indented braces and a body split off its header by a blank line
        // move lines after layout.
        if self.should_indent_brace_line(BraceType::Command)
            || self.tree.statements.in_else_body_after_blank_line(first)
        {
            return None;
        }
        let (line, levels) = self.header_chain_before(first)?;
        let extra = if self.options.indent_blocks {
            self.options.indent_width
        } else {
            0
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + (levels - 1) * self.options.indent_width
                + extra
                + self.case_unindent_spaces(),
        )
    }

    /// Statements of a GNU statement expression `({ ... })` take one level
    /// past the line holding its `(`, and a `}` starting a line closes at
    /// that line.
    fn statement_expression_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let (group, extra) = if matches!(tokens[first], Token::Symbol('}')) {
            (groups.closed_at(first)?, 0)
        } else {
            if !self.tree.statements.starts_block_statement(first) {
                return None;
            }
            (groups.enclosing(first)?, self.options.indent_width)
        };
        if self.tree.blocks.kind(group) != Some(BlockKind::StatementExpression) {
            return None;
        }
        // The `(` line: styles that break or indent braces put the `{` back.
        let paren = self.tree.previous_code_token(groups.get(group).open)?;
        let line = self.output.line_with_token(paren)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + extra
                + self.case_unindent_spaces(),
        )
    }

    /// The line leading the header chain that owns the `{` at `open`, and
    /// how many levels the chain nests on that line: `else if` is one
    /// header.
    fn header_chain_before(&self, open: usize) -> Option<(usize, usize)> {
        let tokens = &self.tree.tokens;
        let is_word = |index: usize, words: &[&str]| matches!(&tokens[index], Token::Word(word) if words.contains(&word.as_str()));
        let mut inner = self.header_keyword_before(open).or_else(|| {
            self.tree
                .previous_code_token(open)
                .filter(|&previous| is_word(previous, &["do", "else", "try"]))
        })?;
        let mut levels = 1;
        while let Some(outer) = self.tree.statements.braceless_header(inner)
            && self.output.line_with_token(outer) == self.output.line_with_token(inner)
            && !(is_word(inner, &["if"]) && is_word(outer, &["else"]))
        {
            levels += 1;
            inner = outer;
        }
        Some((self.line_led_by(inner)?, levels))
    }

    /// The control keyword whose condition closes right before the `{` at
    /// `open`, as in `if (x) {`.
    fn header_keyword_before(&self, open: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let close = self.tree.previous_code_token(open)?;
        let condition = groups.closed_at(close)?;
        let keyword = self.tree.previous_code_token(groups.get(condition).open)?;
        matches!(&self.tree.tokens[keyword], Token::Word(word)
            if matches!(word.as_str(), "if" | "for" | "while" | "switch" | "foreach"))
        .then_some(keyword)
    }

    /// An `if` that a line break separates from its `else` nests one level
    /// past the `else` line, as a body would.
    fn split_else_if_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if self.options.no_indent_if_after_else
            || !matches!(&tokens[first], Token::Word(word) if word == "if")
        {
            return None;
        }
        let keyword = self.tree.previous_code_token(first)?;
        if !matches!(&tokens[keyword], Token::Word(word) if word == "else") {
            return None;
        }
        let line = self.line_led_by(keyword)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// A braceless body starting a line takes one level past the line
    /// holding its header.
    fn braceless_body_indent(&self, first: usize) -> Option<usize> {
        // Added braces make the body a block.
        if self.options.add_braces || self.options.add_one_line_braces {
            return None;
        }
        let tokens = &self.tree.tokens;
        let header = self.tree.statements.braceless_header(first)?;
        // astyle loses track of a body after a block in its header, such as
        // a lambda in the condition.
        if tokens[header..first]
            .iter()
            .any(|token| matches!(token, Token::Symbol('{')))
        {
            return None;
        }
        let line = self.line_led_by(header)?;
        // `else while (x)` nests two headers on one line; `else if` is one.
        let nested = !matches!(&tokens[header], Token::Word(word) if word == "if")
            && self
                .tree
                .previous_code_token(header)
                .is_some_and(|previous| {
                    matches!(&tokens[previous], Token::Word(word) if word == "else")
                        && self.output.line_with_token(previous) == Some(line)
                });
        let levels = 1 + usize::from(nested);
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + levels * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The output line holding the token `token` at its start, after at
    /// most `}` and `else`.
    fn line_led_by(&self, token: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let line = self.output.line_with_token(token)?;
        let first = self.output.line_tokens(line)?.first;
        let mut index = token;
        while index != first {
            index = self.tree.previous_code_token(index)?;
            if !matches!(&tokens[index], Token::Symbol('}'))
                && !matches!(&tokens[index], Token::Word(word) if word == "else")
            {
                return None;
            }
        }
        Some(line)
    }

    /// Column of the statement starting at `first` after its previous
    /// statement in the block, from the line holding that statement:
    /// comments, continuation lines, and bodies in between do not count.
    /// Where the style indents a block's brace, a brace sibling stands one
    /// level past the statement column, and a brace line's column is the
    /// style's to tell.
    fn sibling_statement_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let sibling = self.tree.statements.previous_sibling(first)?;
        let line = self.output.line_with_token(sibling)?;
        if self.output.line_tokens(line)?.first != sibling {
            return None;
        }
        let column =
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces();
        let is_brace = |index: usize| matches!(tokens[index], Token::Symbol('{'));
        let indented_braces = self.should_indent_brace_line(BraceType::Command);
        if is_brace(first) && indented_braces {
            // A brace after a statement stands a level in; after a block's
            // brace, at that brace.
            return Some(if is_brace(sibling) {
                column
            } else {
                column + self.options.indent_width
            });
        }
        if is_brace(sibling) && indented_braces {
            column.checked_sub(self.options.indent_width)
        } else {
            Some(column)
        }
    }

    /// Column of the statements of the block holding the statement that
    /// starts at `first`, from the block's opening: the brace line plus the
    /// style's body offset, or, for an attached brace, the line of the
    /// block's header (past a leading `}` and `else`) plus one level.
    /// Function, control, and plain blocks only: switch bodies, case
    /// blocks, and lambdas follow astyle's own layout.
    fn block_body_column(&self, first: usize) -> Option<usize> {
        let is_first = self.tree.statements.block_opening(first).is_some();
        if !is_first && self.tree.statements.previous_sibling(first).is_none() {
            return None;
        }
        self.enclosing_block_body_column(first)
    }

    /// Column of the statements of the block that holds the statement
    /// after a label on the line being laid out.
    pub(crate) fn label_statement_column(&self) -> Option<usize> {
        let first = self.output.pending_tokens()?.first;
        if !self.tree.statements.starts_block_statement(first) {
            return None;
        }
        self.enclosing_block_body_column(first)
            .or_else(|| self.earlier_statement_column(first))
    }

    /// Column of the nearest earlier statement of the block holding the
    /// statement at `first`, from the line it starts, labels aside.
    fn earlier_statement_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let block = groups.enclosing(first)?;
        let open = groups.get(block).open;
        let earlier = (open + 1..first).rev().find(|&index| {
            self.tree.statements.starts_block_statement(index)
                && groups.enclosing(index) == Some(block)
                && !next_code_token(tokens, index + 1)
                    .is_some_and(|next| matches!(tokens[next], Token::Symbol(':')))
        })?;
        let line = self.output.line_with_token(earlier)?;
        (self.output.line_tokens(line)?.first == earlier).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    fn enclosing_block_body_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if matches!(tokens[first], Token::Symbol('{'))
            && self.should_indent_brace_line(BraceType::Command)
        {
            return None;
        }
        let block = self.enclosing_block(first)?;
        if !matches!(
            self.tree.blocks.kind(block),
            Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
        ) {
            return None;
        }
        if self.tree.blocks.owner(block).is_some_and(|owner| {
            matches!(&tokens[owner], Token::Word(word)
                if matches!(word.as_str(), "switch" | "case" | "default"))
        }) {
            return None;
        }
        let open = self.tree.groups.get(block).open;
        let width = self.options.indent_width;
        let brace_line = self.output.line_with_token(open)?;
        let column = if self.output.line_tokens(brace_line)?.first == open {
            let indented_brace = self.options.indent_braces
                && self
                    .layout
                    .nesting
                    .brace_type_stack
                    .last()
                    .is_some_and(|&brace_type| self.should_indent_brace_line(brace_type));
            self.output.lead_width(brace_line, self.options.tab_width)
                + if indented_brace { 0 } else { width }
        } else {
            let mut header = self.tree.blocks.owner(block);
            while let Some(index) = header
                && (matches!(tokens[index], Token::Symbol('}'))
                    || matches!(&tokens[index], Token::Word(word) if word == "else"))
            {
                header = next_code_token(tokens, index + 1);
            }
            // The block belongs to the innermost header of a braceless
            // chain; headers sharing its line, as in `if (x) do {`, each
            // nest one level.
            if let Some((line, levels)) = self.header_chain_before(open) {
                self.output.lead_width(line, self.options.tab_width) + levels * width
            } else {
                let header_line = self.line_led_by(header?)?;
                self.output.lead_width(header_line, self.options.tab_width) + width
            }
        };
        Some(column + self.case_unindent_spaces())
    }

    /// The brace block directly holding the statement that starts at
    /// `first`.
    fn enclosing_block(&self, first: usize) -> Option<GroupId> {
        let groups = &self.tree.groups;
        let group = if matches!(self.tree.tokens[first], Token::Symbol('{')) {
            groups.get(groups.opened_at(first)?).parent?
        } else {
            groups.enclosing(first)?
        };
        (self.tree.blocks.kind(group).is_some()).then_some(group)
    }
}
