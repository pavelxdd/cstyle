//! Indents anchored in the structure tree.
//!
//! Layout heuristics read the text of earlier output lines, so comments,
//! continuation lines, and directives in between can mislead them. These
//! rules run last and take a statement's indent from lines that the tree
//! links it to: the `if` of an `else`, the header of a braceless body, the
//! previous statement of the block, and the block's opening.

use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::output::model::LineLayout;
use crate::formatter::state::BraceType;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::{BlockKind, next_code_token};
use crate::formatter::structure::groups::GroupId;

impl FormatEngine<'_> {
    pub(crate) fn apply_tree_anchor_layout(
        &self,
        line: &str,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind != LineKind::Normal || line.trim_start().starts_with('#') {
            return layout;
        }
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return layout;
        };
        if let Some(spaces) = self
            .else_matching_if_indent(first)
            .or_else(|| self.braceless_body_indent(first))
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        let structural = layout.indent * self.options.indent_width;
        let sibling = self.sibling_statement_column(first);
        let block = self.block_body_column(first);
        if sibling == Some(structural) || block == Some(structural) {
            // A heuristic moved the line off a structural level that the
            // tree confirms; an anchor off that level is itself misplaced.
            layout.exact_indent_spaces = Some(structural);
        }
        layout
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
    fn case_unindent_spaces(&self) -> usize {
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
            return None;
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
        let tokens = &self.tree.tokens;
        let is_first = self.tree.statements.block_opening(first).is_some();
        if !is_first && self.tree.statements.previous_sibling(first).is_none() {
            return None;
        }
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
            let header_line = self.line_led_by(header?)?;
            self.output.lead_width(header_line, self.options.tab_width) + width
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
