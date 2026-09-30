//! Indents anchored in the structure tree.
//!
//! Layout heuristics read the text of earlier output lines, so comments,
//! continuation lines, and directives in between can mislead them. These
//! rules run last and take a statement's indent from lines that the tree
//! links it to: the `if` of an `else`, the header of a braceless body, the
//! previous statement of the block, and the block's opening.

use super::astyle_stack::literal_closed;
use crate::config::BraceStyle;
use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{CommentKind, Token, token_text};
use crate::formatter::output::model::LineLayout;
use crate::formatter::state::BraceType;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::syntax::language::is_header;
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
        // Tokens that the engine moved across lines map to no line of theirs.
        if !line
            .trim_start()
            .starts_with(token_text(&self.tree.tokens[first]).as_str())
        {
            return layout;
        }
        if let Some(spaces) = self
            .else_matching_if_indent(first)
            .or_else(|| self.braceless_body_indent(first))
            .or_else(|| self.do_while_indent(first))
            .or_else(|| self.split_else_block_statement_indent(first))
            .or_else(|| self.statement_after_split_else_indent(first))
            .or_else(|| self.dangling_else_block_indent(first))
            .or_else(|| self.split_else_if_indent(first))
            .or_else(|| self.statement_expression_indent(first))
            .or_else(|| self.return_value_indent(first))
            .or_else(|| self.ternary_arm_in_parens_indent(first))
            .or_else(|| self.assigned_string_continuation_indent(first))
            .or_else(|| self.comment_interrupted_continuation_indent(first))
            .or_else(|| self.argument_after_interruption_indent(first))
            .or_else(|| self.string_concatenation_indent(first))
            .or_else(|| self.stacked_argument_indent(first))
            .or_else(|| self.stacked_return_indent(first))
            .or_else(|| self.stacked_closing_paren_indent(first))
            .or_else(|| self.logical_operand_in_parens_indent(first))
            .or_else(|| self.logical_chain_operand_indent(first))
            .or_else(|| self.declarator_after_comma_indent(first))
            .or_else(|| self.leading_semicolon_indent(first))
            .or_else(|| self.assignment_continuation_indent(first))
            .or_else(|| self.assigned_operand_indent(first))
            .or_else(|| self.leading_ternary_in_condition_indent(first))
            .or_else(|| self.leading_ternary_in_argument_indent(first))
            .or_else(|| self.ternary_second_arm_indent(first))
            .or_else(|| self.leading_logical_in_parens_indent(first))
            .or_else(|| self.enum_value_after_split_member_indent(first))
            .or_else(|| self.vtk_initializer_first_element_indent(first))
            .or_else(|| self.whitesmith_brace_row_indent(first))
            .or_else(|| self.initializer_row_indent(first))
            .or_else(|| self.initializer_closing_brace_indent(first))
            .or_else(|| self.nested_initializer_closing_brace_indent(first))
            .or_else(|| self.indented_block_brace_indent(first))
            .or_else(|| self.broken_control_brace_indent(first))
            .or_else(|| self.gnu_else_brace_indent(first))
            .or_else(|| self.whitesmith_bare_block_brace_indent(first))
            .or_else(|| self.whitesmith_macro_block_brace_indent(first))
            .or_else(|| self.whitesmith_function_brace_indent(first))
            .or_else(|| self.vtk_anonymous_member_aggregate_brace_indent(first))
            .or_else(|| self.statement_after_case_block_indent(first))
            .or_else(|| self.first_statement_after_case_label_indent(first))
            .or_else(|| self.statement_after_labeled_statement_indent(first))
            .or_else(|| self.case_block_statement_indent(first))
            .or_else(|| self.block_closing_brace_indent(first))
            .or_else(|| self.assigned_value_in_case_block_indent(first))
            .or_else(|| self.argument_after_assigned_call_paren_indent(first))
            .or_else(|| self.assigned_value_in_call_indent(first))
            .or_else(|| self.condition_after_split_header_paren_indent(first))
            .or_else(|| self.closing_paren_indent(first))
            .or_else(|| self.parameter_line_indent(first))
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        // The engine loses the level of an `if` chain that is the body of a
        // braceless header once the chain opens a braced block.
        if self.inside_braced_chain_of_braceless_body(first)
            && let Some(spaces) = self.closing_brace_indent(first)
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

    /// Whitesmith indents a brace row a level past the plain element row
    /// before it, as it indents braces past their statements.
    fn whitesmith_brace_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Whitesmith
            || !matches!(tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let group = groups.enclosing(first)?;
        let comma = self.tree.previous_code_token(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || !matches!(tokens[comma], Token::Symbol(','))
            || groups.enclosing(comma) != Some(group)
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
        let line = self.output.line_with_token(element)?;
        if matches!(tokens[element], Token::Symbol('{'))
            || self.output.line_tokens(line)?.first != element
        {
            return None;
        }
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
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
        let separated = (first..close).find(|&index| {
            matches!(tokens[index], Token::Symbol(',')) && groups.enclosing(index) == Some(group)
        });
        // Styles that indent initializer braces lay out a last brace row
        // their own way.
        if separated.is_none()
            && matches!(tokens[first], Token::Symbol('{'))
            && self.should_indent_brace_line(BraceType::Initializer)
            && !(self.options.brace_style == BraceStyle::Vtk
                && self.in_code(first)
                && (open_of_row_before(tokens, groups, group, comma)
                    .is_some_and(|element| matches!(tokens[element], Token::Symbol('{')))))
        {
            return None;
        }
        let end = separated.unwrap_or_else(|| {
            self.tree
                .previous_code_token(close)
                .map_or(close, |last| last + 1)
        });
        // A row after a comment line moves alone: the engine lays it out
        // from the comment.
        let after_comment = tokens[comma..first]
            .iter()
            .any(|token| matches!(token, Token::Comment(..)));
        if !after_comment
            && let Some(newline) =
                (first..end).find(|&index| matches!(tokens[index], Token::Newline))
            && !self.tree.previous_code_token(newline).is_some_and(|last| {
                matches!(&tokens[last], Token::Operator(operator) if operator == "=")
                    && groups.enclosing(last) == Some(group)
                    // A nested initializer whose `{` ends the line of a row
                    // lays out its rows from that line; the engine places
                    // the row itself only after a comment, or where the
                    // style indents the nested closing brace.
                    || (tokens[comma..first].iter().any(|token| matches!(token, Token::Comment(..)))
                        || matches!(
                            self.options.brace_style,
                            BraceStyle::Whitesmith | BraceStyle::Ratliff | BraceStyle::Vtk
                        ))
                        && groups.opened_at(last).is_some_and(|nested| {
                        self.tree.blocks.kind(nested) == Some(BlockKind::Initializer)
                            && groups.get(nested).close.is_some_and(|close| {
                                self.tree.previous_code_token(end) == Some(close)
                            })
                    })
            })
        {
            return None;
        }
        let open = groups.get(group).open;
        // astyle continues a sign that opens the first element through the
        // rows after it; the engine follows that.
        if next_code_token(tokens, open + 1).is_some_and(|element| {
            matches!(&tokens[element], Token::Operator(operator) if operator == "-" || operator == "+")
        }) {
            return None;
        }
        let separator = (open..comma).rev().find(|&index| {
            index == open
                || (matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group))
        })?;
        let element = next_code_token(tokens, separator + 1)?;
        // Styles that indent initializer braces move brace rows alone, a
        // level past the rows around them.
        let is_brace = |index: usize| matches!(tokens[index], Token::Symbol('{'));
        let mut brace_offset = 0;
        if is_brace(element) != is_brace(first)
            && self.should_indent_brace_line(BraceType::Initializer)
        {
            // Only a brace row that spans lines stands apart.
            let element_line = self.output.line_with_token(element);
            if !is_brace(element)
                || !element_line.is_some_and(|line| {
                    self.output
                        .line_tokens(line)
                        .is_some_and(|span| span.first == element)
                })
                || groups
                    .opened_at(element)
                    .and_then(|nested| groups.get(nested).close)
                    .is_none_or(|close| self.output.line_with_token(close) == element_line)
            {
                return None;
            }
            brace_offset = self.options.indent_width;
        }
        let line = self.output.line_with_token(element)?;
        // A row of several elements places the next row from its first.
        let row = self.output.line_tokens(line)?.first;
        if !self.output.as_slice()[line]
            .trim_start()
            .starts_with(token_text(&tokens[row]).as_str())
        {
            return None;
        }
        if row != element
            && !(groups.enclosing(row) == Some(group)
                && self.tree.previous_code_token(row).is_some_and(|before| {
                    before == open
                        || matches!(tokens[before], Token::Symbol(','))
                            && groups.enclosing(before) == Some(group)
                }))
        {
            return None;
        }
        (self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
            .checked_sub(brace_offset)
    }

    /// VTK places the first element of an assigned initializer whose `{`
    /// starts its line a level past the `{`.
    fn vtk_initializer_first_element_indent(&self, first: usize) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Vtk {
            return None;
        }
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        let nested = groups
            .enclosing(open)
            .is_some_and(|outer| self.tree.blocks.kind(outer) == Some(BlockKind::Initializer));
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || self.tree.previous_code_token(first) != Some(open)
            || matches!(self.tree.tokens[first], Token::Symbol('}'))
            || !nested
                && !self.tree.previous_code_token(open).is_some_and(|assign| {
                    matches!(&self.tree.tokens[assign], Token::Operator(operator) if operator == "=")
                })
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_tokens(line)?.first != open
            || !self.output.as_slice()[line].trim_start().starts_with('{')
        {
            return None;
        }
        // The engine places the first element itself when it is no brace
        // row in code, or follows a blank line at file scope.
        let outermost = groups
            .ancestors(group)
            .filter(|&id| self.tree.blocks.kind(id) == Some(BlockKind::Initializer))
            .last()?;
        let in_code = groups.ancestors(outermost).skip(1).any(|id| {
            matches!(
                self.tree.blocks.kind(id),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        });
        if !nested {
            let brace_row = matches!(self.tree.tokens[first], Token::Symbol('{'));
            // At file scope a blank line before the first element leaves it
            // to the engine.
            let after_blank_line = self.tree.tokens[open + 1..first]
                .iter()
                .filter(|token| !matches!(token, Token::Whitespace(_)))
                .collect::<Vec<_>>()
                .windows(2)
                .any(|pair| matches!(pair[0], Token::Newline) && matches!(pair[1], Token::Newline));
            if in_code && !brace_row || !in_code && after_blank_line {
                return None;
            }
        }
        // Rows of a nested brace row start a level past its `{` only at file
        // scope, where the `{` stands at the element column.
        let offset = if nested {
            let outermost = groups
                .ancestors(group)
                .filter(|&id| self.tree.blocks.kind(id) == Some(BlockKind::Initializer))
                .last()?;
            let in_code = groups.ancestors(outermost).skip(1).any(|id| {
                matches!(
                    self.tree.blocks.kind(id),
                    Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                )
            });
            let outer_line = self.output.line_with_token(groups.get(outermost).open)?;
            let element_column = self.output.lead_width(outer_line, self.options.tab_width)
                + self.options.indent_width;
            if !in_code && self.output.lead_width(line, self.options.tab_width) == element_column {
                self.options.indent_width
            } else {
                0
            }
        } else {
            self.options.indent_width
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + offset
                + self.case_unindent_spaces(),
        )
    }

    /// Whitesmith and Ratliff indent the `}` closing an initializer element
    /// whose `{` ends the element's line one level past that line; VTK does
    /// so from the second level of nesting.
    fn nested_initializer_closing_brace_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let depth = match self.options.brace_style {
            BraceStyle::Whitesmith | BraceStyle::Ratliff => 1,
            BraceStyle::Vtk => 2,
            _ => return None,
        };
        if !matches!(self.tree.tokens[first], Token::Symbol('}')) {
            return None;
        }
        let group = groups.closed_at(first)?;
        let open = groups.get(group).open;
        if groups
            .ancestors(group)
            .take(depth + 1)
            .filter(|&id| self.tree.blocks.kind(id) == Some(BlockKind::Initializer))
            .count()
            != depth + 1
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        let span = self.output.line_tokens(line)?;
        (span.first != open && span.last == open).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces()
        })
    }

    /// A `}` closing an initializer whose `{` starts its own line stands at
    /// that line.
    fn initializer_closing_brace_indent(&self, first: usize) -> Option<usize> {
        // VTK, GNU and Horstmann close some initializers their own way.
        if matches!(
            self.options.brace_style,
            BraceStyle::Vtk | BraceStyle::Gnu | BraceStyle::Horstmann
        ) || !matches!(self.tree.tokens[first], Token::Symbol('}'))
        {
            return None;
        }
        let group = self.tree.groups.closed_at(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer) {
            return None;
        }
        let open = self.tree.groups.get(group).open;
        let line = self.output.line_with_token(open)?;
        // A brace attached to the line before after layout closes its way.
        (self.output.line_tokens(line)?.first == open
            && self.output.as_slice()[line].trim_start().starts_with('{'))
        .then(|| self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// The line after a statement's first line holding assignments stands
    /// where astyle's continuation stack leaves it: each `=` stacks the
    /// column of its value or, past the maximum, two levels, never below
    /// the indent before; a trailing `=` stacks a continuation level more.
    fn assignment_continuation_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let is_assignment = |index: usize| {
            matches!(&tokens[index], Token::Operator(operator)
                if operator.ends_with('=')
                    && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">="))
        };
        if matches!(tokens[first], Token::Symbol('{' | '}')) {
            return None;
        }
        let last = self.tree.previous_code_token(first)?;
        if matches!(tokens[last], Token::Symbol(',' | ';' | '{' | '}')) {
            return None;
        }
        let group = groups.enclosing(first);
        if let Some(group) = group
            && !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        {
            return None;
        }
        let line = self.output.line_with_token(last)?;
        let start = next_code_token(tokens, self.output.line_tokens(line)?.first)?;
        if groups.enclosing(start) != group
            || self
                .tree
                .previous_code_token(start)
                .is_some_and(|before| !matches!(tokens[before], Token::Symbol(';' | '{' | '}')))
            || tokens[start..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let lead = self.output.lead_width(line, self.options.tab_width);
        let max = self.options.max_continuation_indent;
        let mut stack: Vec<usize> = Vec::new();
        let mut index = start;
        loop {
            match &tokens[index] {
                Token::Word(word) if word == "return" || word == "template" || is_header(word) => {
                    return None;
                }
                Token::Operator(operator) if matches!(operator.as_str(), "<<" | ">>") => {
                    return None;
                }
                Token::StringLiteral(text) | Token::CharLiteral(text) if !literal_closed(text) => {
                    return None;
                }
                _ if is_assignment(index) => {
                    let indent = if index == last || self.options.indent_after_parens {
                        let previous = stack.last().copied().unwrap_or(0);
                        let indent =
                            self.options.continuation_indent * self.options.indent_width + previous;
                        if indent > max {
                            2 * self.options.indent_width
                        } else {
                            indent
                        }
                    } else {
                        let value = next_code_token(tokens, index + 1)?;
                        if matches!(tokens[value], Token::Symbol('{'))
                            || matches!(&tokens[value], Token::Word(word) if word == "new")
                        {
                            return None;
                        }
                        let mut indent = self.token_column(value)?.checked_sub(lead)?;
                        if indent > max {
                            indent = 2 * self.options.indent_width;
                        }
                        indent.max(stack.last().copied().unwrap_or(0))
                    };
                    stack.push(indent);
                }
                _ => {}
            }
            if index == last {
                break;
            }
            index = match groups.opened_at(index) {
                Some(opened) => groups.get(opened).close?,
                None => index,
            };
            if index >= last && index != last {
                return None;
            }
            if index == last {
                break;
            }
            index = next_code_token(tokens, index + 1)?;
            if index > last {
                return None;
            }
        }
        let top = stack.last().copied()?;
        Some(lead + top + self.case_unindent_spaces())
    }

    /// The first statement of a case starting a line after its label's
    /// line takes one level past the label.
    fn first_statement_after_case_label_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if matches!(tokens[first], Token::Symbol('{' | '}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
        {
            return None;
        }
        let colon = self.tree.previous_code_token(first)?;
        if !matches!(tokens[colon], Token::Symbol(':')) {
            return None;
        }
        let line = self.output.line_with_token(colon)?;
        let span = self.output.line_tokens(line)?;
        let label = next_code_token(tokens, span.first)?;
        let last = (label..=span.last)
            .rev()
            .find(|&index| is_code_token(&tokens[index]))?;
        if last != colon
            || !matches!(&tokens[label], Token::Word(word) if word == "case" || word == "default")
            || groups.enclosing(label) != groups.enclosing(first)
            || !self.in_switch_body(first)
            || tokens[colon..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.options.indent_width)
    }

    /// A statement after a labeled statement, which links to no sibling,
    /// stands at the statements of its block.
    fn statement_after_labeled_statement_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let statements = &self.tree.statements;
        if !statements.starts_block_statement(first)
            || statements.previous_sibling(first).is_some()
            || statements.block_opening(first).is_some()
            || matches!(tokens[first], Token::Symbol('{' | '}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let group = groups.enclosing(first);
        let statement_before = |end: usize| {
            (0..end).rev().find(|&index| {
                statements.starts_block_statement(index) && groups.enclosing(index) == group
            })
        };
        let is_label = |index: usize| {
            matches!(&tokens[index], Token::Word(word)
                if !matches!(word.as_str(), "case" | "default"))
                && next_code_token(tokens, index + 1)
                    .is_some_and(|colon| matches!(tokens[colon], Token::Symbol(':')))
        };
        let previous = statement_before(first)?;
        let label = if is_label(previous) {
            previous
        } else {
            let label = statement_before(previous)?;
            let colon = next_code_token(tokens, label + 1)?;
            if !is_label(label) || next_code_token(tokens, colon + 1) != Some(previous) {
                return None;
            }
            label
        };
        if tokens[label..first]
            .iter()
            .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        self.enclosing_block_body_column(first)
    }

    /// Whether the innermost group around `index` is a `switch` body.
    fn in_switch_body(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        groups.enclosing(index).is_some_and(|body| {
            groups.get(body).delimiter == Delimiter::Brace
                && self
                    .tree
                    .previous_code_token(groups.get(body).open)
                    .and_then(|close| groups.closed_at(close))
                    .and_then(|condition| self.tree.previous_code_token(groups.get(condition).open))
                    .is_some_and(
                        |keyword| matches!(&tokens[keyword], Token::Word(word) if word == "switch"),
                    )
        })
    }

    /// A `;` starting a line that ends a statement continued over lines
    /// stands at the continuation line before it.
    fn leading_semicolon_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        if !matches!(tokens[first], Token::Symbol(';')) {
            return None;
        }
        let group = groups.enclosing(first);
        if let Some(group) = group
            && !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        {
            return None;
        }
        let previous = self.tree.previous_code_token(first)?;
        if matches!(tokens[previous], Token::Symbol(';' | '{' | '}' | ':'))
            || tokens[previous..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let line = self.output.line_with_token(previous)?;
        let line_first = next_code_token(tokens, self.output.line_tokens(line)?.first)?;
        if groups.enclosing(line_first) != group
            || self
                .tree
                .previous_code_token(line_first)
                .is_none_or(|before| matches!(tokens[before], Token::Symbol(';' | '{' | '}')))
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// A declarator after a `,` of a statement whose first line holds an
    /// `=` and ends at a `,` stands where astyle registers that `=`: at
    /// the word before it, or at the value after an array's `]`.
    fn declarator_after_comma_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let group = groups.enclosing(first);
        let comma = self.tree.previous_code_token(first)?;
        if !matches!(tokens[comma], Token::Symbol(',')) || groups.enclosing(comma) != group {
            return None;
        }
        if let Some(group) = group
            && !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        {
            return None;
        }
        let mut start = comma;
        while let Some(mut before) = self.tree.previous_code_token(start) {
            if let Some(closed) = groups.closed_at(before)
                && (groups.get(closed).delimiter != Delimiter::Brace
                    || self.tree.blocks.kind(closed) == Some(BlockKind::Initializer))
            {
                before = groups.get(closed).open;
            } else if matches!(tokens[before], Token::Symbol(';' | '{' | '}' | ':')) {
                break;
            }
            if groups.enclosing(before) != group {
                break;
            }
            start = before;
        }
        if matches!(&tokens[start], Token::Word(word) if word == "return" || is_header(word))
            || tokens[start..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let start_line = self.output.line_with_token(start)?;
        if self.output.line_tokens(start_line)?.first != start {
            return None;
        }
        // astyle stacks an indent at each line holding an `=` and ending at
        // a `,`, drifting pointer declarators a column per line; only the
        // statement's first line holds here.
        let span = self.output.line_tokens(start_line)?;
        let line_end = (start..=span.last)
            .rev()
            .find(|&index| is_code_token(&tokens[index]))?;
        if !matches!(tokens[line_end], Token::Symbol(',')) || groups.enclosing(line_end) != group {
            return None;
        }
        let mut index = start;
        let assign = loop {
            if index >= line_end {
                // No `=`: the `,` ending the line registers the second word,
                // and that indent holds for the lines after.
                let second = next_code_token(tokens, start + 1)?;
                let second_line = self.output.line_with_token(comma)?;
                return (second_line != start_line
                    && self.tree.previous_code_token(start).is_none_or(|before| {
                        matches!(tokens[before], Token::Symbol(';' | '{' | '}'))
                    })
                    && matches!(&tokens[start], Token::Word(word)
                        if !matches!(word.as_str(), "class" | "struct" | "union" | "enum"))
                    && !tokens[start..line_end]
                        .iter()
                        .any(|token| matches!(token, Token::Symbol(':')))
                    && matches!(tokens[second], Token::Word(_))
                    && second < line_end)
                    .then(|| self.token_column(second))
                    .flatten()
                    .map(|column| column + self.case_unindent_spaces());
            }
            match &tokens[index] {
                Token::Operator(operator) if operator == "=" => break index,
                Token::Symbol('(') => return None,
                _ => {}
            }
            index = match groups.opened_at(index) {
                Some(opened) => groups.get(opened).close?,
                None => index,
            };
            index = next_code_token(tokens, index + 1)?;
        };
        // astyle drops the indent of an `=` whose value starts with `new`.
        if next_code_token(tokens, assign + 1)
            .is_some_and(|value| matches!(&tokens[value], Token::Word(word) if word == "new"))
        {
            return None;
        }
        let before = self.tree.previous_code_token(assign)?;
        let target = match &tokens[before] {
            Token::Symbol(']') => next_code_token(tokens, assign + 1)?,
            Token::Word(_) => before,
            _ => return None,
        };
        Some(self.token_column(target)? + self.case_unindent_spaces())
    }

    /// Visual column of the code token `token` on its output line, found
    /// by walking the line's token texts in order.
    pub(super) fn token_column(&self, token: usize) -> Option<usize> {
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

    /// An argument after a comma keeps the argument column past a trailing
    /// block comment that runs onto later lines or past directives: those
    /// lines break the engine's continuation.
    fn argument_after_interruption_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        if !matches!(tokens[previous], Token::Symbol(','))
            || groups.enclosing(previous) != Some(group)
            || groups.get(group).delimiter != Delimiter::Paren
        {
            return None;
        }
        let comment =
            (previous + 1..first).find(|&index| !matches!(tokens[index], Token::Whitespace(_)))?;
        let directive =
            (comment..first).any(|index| matches!(tokens[index], Token::Preprocessor(_)));
        if !directive && !matches!(&tokens[comment], Token::Comment(_, text) if text.contains('\n'))
        {
            return None;
        }
        let previous_line = self.output.line_with_token(previous)?;
        let previous_first = self.output.line_tokens(previous_line)?.first;
        let column = if groups.enclosing(previous_first) == Some(group) {
            self.output
                .lead_width(previous_line, self.options.tab_width)
        } else {
            // Past the maximum continuation indent astyle falls back to
            // the engine's indent.
            let open = groups.get(group).open;
            let content = self.token_column(next_code_token(tokens, open + 1)?)?;
            let open_line = self.output.line_with_token(open)?;
            let open_lead = self.output.lead_width(open_line, self.options.tab_width);
            if content >= open_lead + self.options.max_continuation_indent {
                return None;
            }
            content
        };
        Some(column + self.case_unindent_spaces())
    }

    /// An operand after a trailing `&&` or `||` inside nested parentheses
    /// aligns with the first operand after their `(`.
    fn logical_operand_in_parens_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        if !matches!(&tokens[previous], Token::Operator(operator) if operator == "&&" || operator == "||")
            || groups.enclosing(previous) != Some(group)
            || groups.get(group).delimiter != Delimiter::Paren
        {
            return None;
        }
        let open = groups.get(group).open;
        let before = self.tree.previous_code_token(open)?;
        let condition = matches!(&tokens[before], Token::Word(word)
            if matches!(word.as_str(), "if" | "while" | "switch"));
        let nested = matches!(&tokens[before], Token::Symbol('(') | Token::Operator(_))
            || matches!(&tokens[before], Token::Word(word) if word == "return");
        if !condition && !nested {
            return None;
        }
        let content = next_code_token(tokens, open + 1)?;
        let open_line = self.output.line_with_token(open)?;
        let content_line = self.output.line_with_token(content)?;
        if content_line != open_line {
            // Operands after one that starts its line past a directive
            // stand at that operand.
            return (condition
                && self.output.line_tokens(content_line)?.first == content
                && (open + 1..content)
                    .any(|index| matches!(tokens[index], Token::Preprocessor(_))))
            .then(|| {
                self.output.lead_width(content_line, self.options.tab_width)
                    + self.case_unindent_spaces()
            });
        }
        let mut column = self.token_column(content)?;
        let open_lead = self.output.lead_width(open_line, self.options.tab_width);
        if column >= open_lead + self.options.max_continuation_indent {
            return None;
        }
        // A control condition's lines keep at least its continuation indent,
        // in groups opened on the header's line.
        let on_header_line = groups
            .ancestors(group)
            .find(|&id| {
                self.tree
                    .previous_code_token(groups.get(id).open)
                    .is_some_and(|keyword| {
                        matches!(&tokens[keyword], Token::Word(word)
                        if matches!(word.as_str(), "if" | "while" | "switch"))
                    })
            })
            .and_then(|condition| self.output.line_with_token(groups.get(condition).open))
            .is_some_and(|header_line| header_line == open_line);
        if on_header_line && let Some(header) = self.control_condition_header_indent() {
            column = column.max(header + min_conditional_indent_spaces(self.options));
        }
        Some(column + self.case_unindent_spaces())
    }

    /// An operand after a trailing `&&` or `||` outside parentheses lines
    /// up with the operand before it in the same chain.
    fn logical_chain_operand_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let is_logical = |index: usize| matches!(&tokens[index], Token::Operator(operator) if operator == "&&" || operator == "||");
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        if !is_logical(previous)
            || groups.enclosing(previous) != Some(group)
            || !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        {
            return None;
        }
        if let Some(value) = self.returned_value_past_comments(first) {
            return Some(self.token_column(value)? + self.case_unindent_spaces());
        }
        // After a keyword, the first operand's line lays out the second;
        // only operands after it anchor the next.
        let mut start = None;
        let mut index = previous;
        while let Some(mut before) = self.tree.previous_code_token(index) {
            if is_logical(before) && groups.enclosing(before) == Some(group) {
                let start = start?;
                let line = self.output.line_with_token(start)?;
                return (self.output.line_tokens(line)?.first == start).then(|| {
                    self.output.lead_width(line, self.options.tab_width)
                        + self.case_unindent_spaces()
                });
            }
            // Operands of an assigned value stand at the value.
            if matches!(&tokens[before], Token::Operator(operator)
                    if operator.ends_with('=') && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">="))
                && groups.enclosing(before) == Some(group)
            {
                return Some(self.token_column(start?)? + self.case_unindent_spaces());
            }
            // So do those of a parenthesized returned value one space past
            // the keyword; the engine lays out the others.
            if matches!(&tokens[before], Token::Word(word) if word == "return")
                && let Some(start) = start
                && start == before + 2
                && matches!(tokens[start], Token::Symbol('('))
                && matches!(&tokens[before + 1], Token::Whitespace(space) if space == " ")
            {
                return Some(self.token_column(start)? + self.case_unindent_spaces());
            }
            if groups.enclosing(before) != Some(group)
                || matches!(
                    tokens[before],
                    Token::Symbol(';' | ',' | '{' | '}' | '?' | ':')
                )
                || matches!(&tokens[before], Token::Word(word) if word == "return")
            {
                break;
            }
            if let Some(closed) = groups.closed_at(before) {
                before = groups.get(closed).open;
            }
            start = Some(before);
            index = before;
        }
        None
    }

    /// An operand after a trailing arithmetic or bitwise operator of an
    /// assigned value stands at the value.
    fn assigned_operand_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        if !matches!(&tokens[previous], Token::Operator(operator)
                if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%" | "|" | "&" | "^" | "<<" | ">>"))
            || groups.enclosing(previous) != Some(group)
            || !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
            || !self
                .tree
                .previous_code_token(previous)
                .is_some_and(|operand| {
                    matches!(
                        tokens[operand],
                        Token::Word(_) | Token::Number(_) | Token::Symbol(')' | ']')
                    )
                })
        {
            return None;
        }
        // The first `=` of a chained assignment holds the value.
        let mut assign = None;
        let mut index = previous;
        while let Some(before) = self.tree.previous_code_token(index) {
            if groups.enclosing(before) == Some(group) {
                match &tokens[before] {
                    Token::Operator(operator) if operator == "=" => assign = Some(before),
                    Token::Symbol(';' | ',' | '{' | '}' | '?' | ':') => break,
                    Token::Word(word) if word == "return" => return None,
                    _ => {}
                }
            }
            index = before;
        }
        let assign = assign?;
        let value = next_code_token(tokens, assign + 1)?;
        let line = self.output.line_with_token(value)?;
        if self.output.line_with_token(assign)? != line {
            return None;
        }
        Some(self.token_column(value)? + self.case_unindent_spaces())
    }

    /// The first token of a returned value whose `&&` or `||` operand at
    /// `first` follows a comment: the comment breaks the engine's
    /// continuation, and the operand stands at the value.
    fn returned_value_past_comments(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let previous = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first);
        if !matches!(&tokens[previous], Token::Operator(operator) if operator == "&&" || operator == "||")
            || groups.enclosing(previous) != group
            || !tokens[previous..first]
                .iter()
                .any(|token| matches!(token, Token::Comment(..)))
        {
            return None;
        }
        let mut value = previous;
        loop {
            let mut before = self.tree.previous_code_token(value)?;
            if matches!(&tokens[before], Token::Word(word) if word == "return") {
                return Some(value);
            }
            if groups.enclosing(before) != group
                || matches!(tokens[before], Token::Symbol(';' | '{' | '}'))
            {
                return None;
            }
            if let Some(closed) = groups.closed_at(before) {
                before = groups.get(closed).open;
            }
            value = before;
        }
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

    /// The arm after a `:` ending its line stands at the first arm when that
    /// arm starts its own line after the `?`.
    fn ternary_second_arm_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let colon = self.tree.previous_code_token(first)?;
        // The `:` ends a published line.
        if !matches!(tokens[colon], Token::Symbol(':')) {
            return None;
        }
        self.output.line_with_token(colon)?;
        let group = groups.enclosing(colon);
        let question = (0..colon)
            .rev()
            .filter(|&index| groups.enclosing(index) == group)
            .take_while(|&index| !matches!(tokens[index], Token::Symbol(';' | '{' | '}' | ',')))
            .find(|&index| matches!(tokens[index], Token::Symbol('?' | ':')))?;
        if !matches!(tokens[question], Token::Symbol('?')) {
            return None;
        }
        let arm = next_code_token(tokens, question + 1)?;
        let line = self.output.line_with_token(arm)?;
        (self.output.line_tokens(line)?.first == arm
            && self.output.line_with_token(question) != Some(line))
        .then(|| self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// A `?` or `:` leading a line inside the arguments of a call stands at
    /// the start of its argument.
    fn leading_ternary_in_argument_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(tokens[first], Token::Symbol('?' | ':')) {
            return None;
        }
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if groups.get(group).delimiter != Delimiter::Paren
            || !self.tree.previous_code_token(open).is_some_and(
                |callee| matches!(&tokens[callee], Token::Word(word) if !is_header(word)),
            )
        {
            return None;
        }
        let separator = (open..first).rev().find(|&index| {
            index == open
                || matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group)
        })?;
        let argument = next_code_token(tokens, separator + 1)?;
        if argument == first
            || (argument..first).any(|index| {
                matches!(tokens[index], Token::Symbol('?' | ':'))
                    && groups.enclosing(index) == Some(group)
            })
        {
            return None;
        }
        let line = self.output.line_with_token(argument)?;
        (self.output.line_tokens(line)?.first == argument).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    /// A `?` or `:` leading a line inside parentheses nested in a control
    /// condition aligns with the parentheses' first operand, but never
    /// before the condition's continuation indent.
    fn leading_ternary_in_condition_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(tokens[first], Token::Symbol('?' | ':')) {
            return None;
        }
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if groups.get(group).delimiter != Delimiter::Paren
            || self.control_condition_of(first).is_none()
            || self.control_condition_of(open).is_none()
        {
            return None;
        }
        let content = next_code_token(tokens, open + 1)?;
        if self.output.line_with_token(content)? != self.output.line_with_token(open)? {
            return None;
        }
        let header = self.control_condition_header_indent()?;
        let column = self
            .token_column(content)?
            .max(header + min_conditional_indent_spaces(self.options));
        Some(column + self.case_unindent_spaces())
    }

    /// An enum member's `=` leading a line after a macro call stays at the
    /// member's column, as astyle lays it out.
    fn enum_value_after_split_member_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(&tokens[first], Token::Operator(operator) if operator == "=") {
            return None;
        }
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if self.tree.blocks.kind(group) != Some(BlockKind::Aggregate)
            || !self.tree.blocks.owner(group).is_some_and(|owner| {
                tokens[owner..open]
                    .iter()
                    .any(|token| matches!(token, Token::Word(word) if word == "enum"))
            })
        {
            return None;
        }
        // Only after a macro call such as a deprecation marker.
        let previous = self.tree.previous_code_token(first)?;
        if !matches!(tokens[previous], Token::Symbol(')')) {
            return None;
        }
        let separator = (open..first).rev().find(|&index| {
            index == open
                || matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group)
        })?;
        let member = next_code_token(tokens, separator + 1)?;
        let line = self.output.line_with_token(member)?;
        (self.output.line_tokens(line)?.first == member).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    /// A `&&` or `||` leading a line inside parentheses aligns with the
    /// parentheses' first operand, but never before a control condition's
    /// continuation indent.
    fn leading_logical_in_parens_indent(&self, first: usize) -> Option<usize> {
        if self.options.indent_after_parens {
            return None;
        }
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(&tokens[first], Token::Operator(operator) if operator == "&&" || operator == "||")
        {
            return None;
        }
        let group = groups.enclosing(first)?;
        if groups.get(group).delimiter != Delimiter::Paren {
            return None;
        }
        let open = groups.get(group).open;
        let content = next_code_token(tokens, open + 1)?;
        let open_line = self.output.line_with_token(open)?;
        if self.output.line_with_token(content)? != open_line {
            return None;
        }
        let mut column = self.token_column(content)?;
        let open_lead = self.output.lead_width(open_line, self.options.tab_width);
        if column >= open_lead + self.options.max_continuation_indent {
            return None;
        }
        if self.control_condition_of(first).is_some()
            && let Some(header) = self.control_condition_header_indent()
        {
            column = column.max(header + min_conditional_indent_spaces(self.options));
        }
        Some(column + self.case_unindent_spaces())
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
        if !matches!(&tokens[owner], Token::Word(word) if word == "switch") {
            return None;
        }
        let brace_line = self.output.line_with_token(open)?;
        // Indented switch braces take their labels after layout, except a
        // brace on its own line, whose column the labels share.
        if self.should_indent_brace_line(BraceType::Command)
            && !(self.options.brace_style == BraceStyle::Vtk
                && !self.options.indent_switches
                && self.layout.line_adjuster.pending_case_unindent() == 0
                && self.output.line_tokens(brace_line)?.first == open)
        {
            return None;
        }
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
        let continues_parens = self.continues_parentheses_past_comments(first)
            || self.returned_value_past_comments(first).is_some();
        let groups = &self.tree.groups;
        let initializer_element = groups.enclosing(first).is_some_and(|group| {
            self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
                && self
                    .tree
                    .previous_code_token(first)
                    .is_some_and(|previous| {
                        matches!(tokens[previous], Token::Symbol(','))
                            && groups.enclosing(previous) == Some(group)
                    })
        });
        let is_else = matches!(&tokens[first], Token::Word(word) if word == "else");
        // An arm after a ternary `?` or `:` ending the line before.
        let ternary_arm = self
            .tree
            .previous_code_token(first)
            .is_some_and(|previous| {
                matches!(tokens[previous], Token::Symbol('?' | ':'))
                    && (0..previous)
                        .rev()
                        .filter(|&index| groups.enclosing(index) == groups.enclosing(previous))
                        .take_while(|&index| {
                            !matches!(tokens[index], Token::Symbol(';' | '{' | '}'))
                        })
                        .any(|index| matches!(tokens[index], Token::Symbol('?')))
                    || matches!(tokens[previous], Token::Symbol('?'))
            });
        // A value after a trailing `=` or `return`.
        let value_after_line_end = self
            .tree
            .previous_code_token(first)
            .is_some_and(|previous| {
                groups.enclosing(previous) == groups.enclosing(first)
                    && (matches!(&tokens[previous], Token::Operator(operator) if operator == "=")
                        || matches!(&tokens[previous], Token::Word(word) if word == "return"))
            });
        // A line inside parentheses: comments before it share its level of
        // the continuation.
        let in_parens = groups
            .enclosing(first)
            .is_some_and(|group| groups.get(group).delimiter == Delimiter::Paren)
            && !matches!(tokens[first], Token::Symbol(')' | '['))
            && !tokens[first..]
                .iter()
                .take_while(|token| !matches!(token, Token::Newline))
                .any(|token| matches!(token, Token::Symbol('{' | '}')));
        if !statements.starts_block_statement(first)
            && !in_parens
            && !initializer_element
            && !is_else
            && !ternary_arm
            && !value_after_line_end
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
            Token::Symbol('{')
                if initializer_element
                    && (self.options.brace_style == BraceStyle::Vtk && !self.in_code(first)
                        || !self.should_indent_brace_line(BraceType::Initializer)) => {}
            Token::Symbol('{' | '}') => return,
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
            // A comment that starts the source line keeps column one.
            let source_column_one = self
                .output
                .comment_token(start)
                .map_or(lead == 0, |comment| {
                    comment == 0 || matches!(self.tree.tokens[comment - 1], Token::Newline)
                });
            if opener_prefix != prefix && (!source_column_one || self.options.indent_col1_comments)
            {
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

    /// Whether the line being laid out continues a statement of a block
    /// at the block's own level, as the value after a trailing `=`.
    pub(crate) fn pending_line_continues_statement(&self) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        if !is_code_token(&tokens[first])
            || matches!(tokens[first], Token::Preprocessor(_))
            || self.tree.statements.starts_block_statement(first)
            || groups
                .enclosing(first)
                .is_none_or(|group| groups.get(group).delimiter != Delimiter::Brace)
        {
            return false;
        }
        self.tree
            .previous_code_token(first)
            .is_some_and(|previous| matches!(&tokens[previous], Token::Operator(_)))
    }

    /// Whether the token `index` sits in a function body.
    fn in_code(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        groups.enclosing(index).is_some_and(|group| {
            groups.ancestors(group).any(|id| {
                matches!(
                    self.tree.blocks.kind(id),
                    Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                )
            })
        })
    }

    /// Whether the statement of the line being laid out sits directly in a
    /// nested `{ ... }` compound statement.
    pub(crate) fn pending_line_in_plain_block(&self) -> bool {
        let groups = &self.tree.groups;
        self.output
            .pending_tokens()
            .and_then(|span| groups.enclosing(span.first))
            .and_then(|group| {
                groups
                    .ancestors(group)
                    .find(|&id| groups.get(id).delimiter == Delimiter::Brace)
            })
            .is_some_and(|block| self.tree.blocks.kind(block) == Some(BlockKind::Block))
    }

    /// Whether the line being laid out starts an argument of a call after
    /// the `,` ending the argument before it.
    pub(crate) fn pending_line_starts_call_argument(&self) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        let Some(group) = groups.enclosing(first) else {
            return false;
        };
        groups.get(group).delimiter == Delimiter::Paren
            && !self.tree.functions.is_parameter_list(group)
            && self
                .tree
                .previous_code_token(first)
                .is_some_and(|previous| matches!(tokens[previous], Token::Symbol(',')))
            && self
                .tree
                .previous_code_token(groups.get(group).open)
                .is_some_and(|callee| matches!(tokens[callee], Token::Word(_)))
    }

    /// Whether the line being laid out starts an initializer element after
    /// a comma.
    pub(crate) fn pending_line_starts_initializer_element(&self) -> bool {
        let groups = &self.tree.groups;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        let Some(group) = groups.enclosing(first) else {
            return false;
        };
        self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
            && self
                .tree
                .previous_code_token(first)
                .is_some_and(|previous| {
                    matches!(self.tree.tokens[previous], Token::Symbol(','))
                        && groups.enclosing(previous) == Some(group)
                })
    }

    /// Whether the line being laid out starts outside every block.
    pub(crate) fn pending_line_at_file_scope(&self) -> bool {
        self.output
            .pending_tokens()
            .is_some_and(|span| self.tree.groups.enclosing(span.first).is_none())
    }

    /// Whether the line being laid out starts an enum member after a comma.
    pub(crate) fn pending_line_starts_enum_member(&self) -> bool {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let Some(first) = self.output.pending_tokens().map(|span| span.first) else {
            return false;
        };
        let Some(group) = groups.enclosing(first) else {
            return false;
        };
        let open = groups.get(group).open;
        self.tree.blocks.kind(group) == Some(BlockKind::Aggregate)
            && self.tree.blocks.owner(group).is_some_and(|owner| {
                tokens[owner..open]
                    .iter()
                    .any(|token| matches!(token, Token::Word(word) if word == "enum"))
            })
            && self
                .tree
                .previous_code_token(first)
                .is_some_and(|previous| {
                    matches!(tokens[previous], Token::Symbol(','))
                        && groups.enclosing(previous) == Some(group)
                })
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
        let Some(previous) = self.tree.previous_code_token(first) else {
            return Some(false);
        };
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
        let if_token = self.tree.statements.if_of_else(else_token)?;
        // A `}` closing a block nested in a braceless `if` body keeps the
        // `else` of that `if` at the brace.
        if first != else_token && self.is_dangling_else(else_token) {
            return None;
        }
        let if_line = self.line_led_by(if_token)?;
        Some(self.output.lead_width(if_line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Whether the `}` right before the `else` at `else_token` closes a
    /// block other than the body of the `else`'s `if`.
    fn is_dangling_else(&self, else_token: usize) -> bool {
        let Some(if_token) = self.tree.statements.if_of_else(else_token) else {
            return false;
        };
        self.tree
            .previous_code_token(else_token)
            .and_then(|close| self.tree.groups.closed_at(close))
            .is_some_and(|group| {
                let open = self.tree.groups.get(group).open;
                self.tree
                    .previous_code_token(open)
                    .and_then(|close| self.tree.groups.closed_at(close))
                    .is_none_or(|condition| {
                        self.tree
                            .previous_code_token(self.tree.groups.get(condition).open)
                            != Some(if_token)
                    })
            })
    }

    /// The statements and the `}` of the block of a dangling `else` stand
    /// by the line of its `if`.
    fn dangling_else_block_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let (block, extra) = if matches!(tokens[first], Token::Symbol('}')) {
            (groups.closed_at(first)?, 0)
        } else if let Some(open) = self.tree.statements.block_opening(first) {
            (groups.opened_at(open)?, self.options.indent_width)
        } else if self.tree.statements.starts_block_statement(first) {
            (groups.enclosing(first)?, self.options.indent_width)
        } else {
            return None;
        };
        let else_token = self.tree.previous_code_token(groups.get(block).open)?;
        if !matches!(&tokens[else_token], Token::Word(word) if word == "else")
            || !self.is_dangling_else(else_token)
        {
            return None;
        }
        let line = self.line_led_by(self.tree.statements.if_of_else(else_token)?)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + extra
                + self.case_unindent_spaces(),
        )
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

    /// Whether the `}` at `first` closes a block inside an `if` chain that
    /// is itself the braceless body of a control statement.
    fn inside_braced_chain_of_braceless_body(&self, first: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let statements = &self.tree.statements;
        let is_word =
            |index: usize, text: &str| matches!(&tokens[index], Token::Word(word) if word == text);
        let Some(group) = groups.closed_at(first) else {
            return false;
        };
        groups.ancestors(group).any(|id| {
            if self.tree.blocks.kind(id) != Some(BlockKind::Control) {
                return false;
            }
            let Some(mut head) = self.tree.blocks.owner(id) else {
                return false;
            };
            loop {
                if is_word(head, "else") {
                    match statements.if_of_else(head) {
                        Some(keyword) => head = keyword,
                        None => return false,
                    }
                } else if let Some(previous) = self
                    .tree
                    .previous_code_token(head)
                    .filter(|&previous| is_word(head, "if") && is_word(previous, "else"))
                {
                    head = previous;
                } else {
                    break;
                }
            }
            is_word(head, "if") && statements.braceless_header(head).is_some()
        })
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

    /// A `{` on its own line after a control header stands a level past the
    /// header's line in styles that indent block braces.
    fn indented_block_brace_indent(&self, first: usize) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) || !matches!(self.tree.tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let group = self.tree.groups.opened_at(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Control)
            || !self.should_indent_brace_line(BraceType::Command)
            || self.tree.statements.in_else_body_after_blank_line(first)
        {
            return None;
        }
        let (line, levels) = self.header_chain_before(first)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + levels * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// GNU indents the `{` of an `else` block starting its line one level
    /// past the `else`.
    fn gnu_else_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if self.options.brace_style != BraceStyle::Gnu
            || !matches!(tokens[first], Token::Symbol('{'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
        {
            return None;
        }
        let keyword = self.tree.previous_code_token(first)?;
        if !matches!(&tokens[keyword], Token::Word(word) if word == "else")
            || self
                .tree
                .groups
                .opened_at(first)
                .and_then(|group| self.tree.blocks.kind(group))
                != Some(BlockKind::Control)
        {
            return None;
        }
        let line = self.line_led_by(keyword)?;
        Some(self.output.lead_width(line, self.options.tab_width) + self.options.indent_width)
    }

    /// A control block's `{` starting its line stands at its header where
    /// the style does not indent braces.
    fn broken_control_brace_indent(&self, first: usize) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol('{'))
            || !matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::None
                    | BraceStyle::Attach
                    | BraceStyle::OneTrueBrace
            )
            || self.options.indent_braces
            || self.options.indent_blocks
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
        {
            return None;
        }
        let group = self.tree.groups.opened_at(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Control) {
            return None;
        }
        self.opening_brace_indent(first)
    }

    /// VTK indents the braces and members of a struct, union, or enum that
    /// is a member of a union one level past its keyword.
    fn vtk_anonymous_member_aggregate_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Vtk {
            return None;
        }
        let group = match tokens[first] {
            Token::Symbol('{') => groups.opened_at(first)?,
            Token::Symbol('}') => groups.closed_at(first)?,
            // Members start after the `{` or after a `;` or `,` of the body.
            _ => groups.enclosing(first).filter(|&group| {
                self.tree
                    .previous_code_token(first)
                    .is_some_and(|previous| {
                        previous == groups.get(group).open
                            || matches!(tokens[previous], Token::Symbol(';' | ','))
                                && groups.enclosing(previous) == Some(group)
                    })
            })?,
        };
        let open = groups.get(group).open;
        let is_aggregate_keyword = |index: usize| matches!(&tokens[index], Token::Word(word) if matches!(word.as_str(), "struct" | "union" | "enum"));
        let mut keyword = self.tree.previous_code_token(open)?;
        if !is_aggregate_keyword(keyword) {
            keyword = self.tree.previous_code_token(keyword)?;
        }
        let outer = groups.enclosing(open)?;
        let outer_open = groups.get(outer).open;
        if self.tree.blocks.kind(group) != Some(BlockKind::Aggregate)
            || !is_aggregate_keyword(keyword)
            || self.tree.blocks.kind(outer) != Some(BlockKind::Aggregate)
            || !self.tree.blocks.owner(outer).is_some_and(|start| {
                tokens[start..outer_open]
                    .iter()
                    .any(|token| matches!(token, Token::Word(word) if word == "union"))
            })
        {
            return None;
        }
        let line = self.output.line_with_token(keyword)?;
        (self.output.line_tokens(line)?.first == keyword).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces()
        })
    }

    /// Whitesmith indents the `{` of a nested compound statement one level
    /// past the statements of the block holding it, whose `{` stands at
    /// them.
    fn whitesmith_bare_block_brace_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Whitesmith
            || !matches!(self.tree.tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let block = groups.opened_at(first)?;
        let parent = groups.get(block).parent?;
        if self.tree.blocks.kind(block) != Some(BlockKind::Block)
            || !matches!(
                self.tree.blocks.kind(parent),
                Some(BlockKind::Control | BlockKind::Block)
            )
            || self.tree.blocks.owner(parent).is_some_and(|owner| {
                matches!(&self.tree.tokens[owner], Token::Word(word)
                    if matches!(word.as_str(), "switch" | "case" | "default"))
            })
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let parent_open = groups.get(parent).open;
        let line = self.output.line_with_token(parent_open)?;
        (self.output.line_tokens(line)?.first == parent_open).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces()
        })
    }

    /// A statement after an `if` chain whose `else` a directive splits from
    /// its body stands at that chain: the engine keeps the level of the
    /// body.
    fn statement_after_split_else_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if matches!(tokens[first], Token::Symbol('{' | '}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let sibling = self.tree.statements.previous_sibling(first)?;
        let split = (sibling..first).any(|index| {
            matches!(&tokens[index], Token::Word(word) if word == "else")
                && tokens[index + 1..]
                    .iter()
                    .find(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
                    .is_some_and(|token| matches!(token, Token::Preprocessor(_)))
        });
        if !split {
            return None;
        }
        self.sibling_statement_column(first)
    }

    /// Statements of a block whose `{` follows its `else` past a directive
    /// stand at the block's body column: the engine keeps the level of the
    /// branch before the directive.
    fn split_else_block_statement_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        // The `{` itself stands at its `else`, a level in where the style
        // indents block braces.
        if matches!(tokens[first], Token::Symbol('{'))
            && self.layout.line_adjuster.total_case_unindent_depth() == 0
            && let Some(keyword) = self.tree.previous_code_token(first)
            && matches!(&tokens[keyword], Token::Word(word) if word == "else")
            && tokens[keyword..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            let offset = if matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Gnu | BraceStyle::Ratliff
            ) {
                self.options.indent_width
            } else {
                0
            };
            let line = self.line_led_by(keyword)?;
            return Some(self.output.lead_width(line, self.options.tab_width) + offset);
        }
        if matches!(tokens[first], Token::Symbol('{' | '}'))
            || !self.tree.statements.starts_block_statement(first)
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let block = self.enclosing_block(first)?;
        let open = self.tree.groups.get(block).open;
        let keyword = self.tree.previous_code_token(open)?;
        if !matches!(&tokens[keyword], Token::Word(word) if word == "else")
            || !tokens[keyword..open]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        self.enclosing_block_body_column(first)
    }

    /// The `while` of a `do` block starting its line stands at the `do`.
    fn do_while_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(&tokens[first], Token::Word(word) if word == "while") {
            return None;
        }
        let close = self.tree.previous_code_token(first)?;
        let block = self.tree.groups.closed_at(close)?;
        let owner = self.tree.blocks.owner(block)?;
        if !matches!(&tokens[owner], Token::Word(word) if word == "do")
            || tokens[close..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let line = self.line_led_by(owner)?;
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Whitesmith indents the `{` of a function body starting its line one
    /// level past the first line of the function's head.
    fn whitesmith_function_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Whitesmith
            || !matches!(tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let body = groups.opened_at(first)?;
        if self.tree.blocks.kind(body) != Some(BlockKind::FunctionBody)
            || groups.get(body).parent.is_some()
        {
            return None;
        }
        let mut start = first;
        while let Some(before) = self.tree.previous_code_token(start) {
            if let Some(closed) = groups.closed_at(before) {
                if groups.get(closed).delimiter == Delimiter::Brace {
                    break;
                }
                start = groups.get(closed).open;
                continue;
            }
            if matches!(tokens[before], Token::Symbol(';' | '}' | '{')) {
                break;
            }
            start = before;
        }
        if start == first
            || tokens[start..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let line = self.output.line_with_token(start)?;
        (self.output.line_tokens(line)?.first == start).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.options.indent_width
        })
    }

    /// Whitesmith indents the `{` of a statement-like macro's block one
    /// level past the macro, as it does for control headers.
    fn whitesmith_macro_block_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if self.options.brace_style != BraceStyle::Whitesmith
            || !matches!(tokens[first], Token::Symbol('{'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let block = self.tree.groups.opened_at(first)?;
        let owner = self.tree.blocks.owner(block)?;
        // A brace after a bare word is an array brace to astyle.
        if !self
            .tree
            .previous_code_token(first)
            .is_some_and(|close| matches!(tokens[close], Token::Symbol(')')))
            || tokens[owner..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        if self.tree.blocks.kind(block) != Some(BlockKind::Control)
            || !matches!(&tokens[owner], Token::Word(word)
                if !is_header(word) && !matches!(word.as_str(), "else" | "do" | "try"))
            || !self.tree.groups.get(block).parent.is_some_and(|parent| {
                matches!(
                    self.tree.blocks.kind(parent),
                    Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                )
            })
        {
            return None;
        }
        let line = self.output.line_with_token(owner)?;
        (self.output.line_tokens(line)?.first == owner).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces()
        })
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
        // An `else` block belongs to the line of its `if`: a `}` before the
        // `else` may close a block nested deeper.
        if is_word(inner, &["else"])
            && let Some(if_token) = self.tree.statements.if_of_else(inner)
        {
            inner = if_token;
        }
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
        // `for (...) if (...)` nests a header per braceless header on the
        // line.
        let mut outer = header;
        let mut chained = 0;
        while let Some(enclosing) = self.tree.statements.braceless_header(outer)
            && self.output.line_with_token(enclosing) == self.output.line_with_token(outer)
            && !matches!(&tokens[enclosing], Token::Word(word) if word == "else")
        {
            outer = enclosing;
            chained += 1;
        }
        let header = outer;
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
        let levels = 1 + chained + usize::from(nested);
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

    /// Lines of a parameter list after a directive stand at the first
    /// parameter when it follows the `(`, up to the maximum continuation
    /// indent.
    fn parameter_line_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let group = groups.enclosing(first)?;
        let previous = self.tree.previous_code_token(first)?;
        if !self.tree.functions.is_parameter_list(group)
            || self.options.indent_after_parens
            || !tokens[previous + 1..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        let open = groups.get(group).open;
        let content = next_code_token(&self.tree.tokens, open + 1)?;
        let line = self.output.line_with_token(open)?;
        if content == first {
            return None;
        }
        if self.output.line_with_token(content)? != line {
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + self.options.indent_width
                    + self.case_unindent_spaces(),
            );
        }
        let column = self.token_column(content)?;
        (column
            <= self.output.lead_width(line, self.options.tab_width)
                + self.options.max_continuation_indent)
            .then(|| column + self.case_unindent_spaces())
    }

    /// A `)` starting a line stands at its `(` when the `(` has content
    /// after it on its line.
    fn closing_paren_indent(&self, first: usize) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol(')'))
            || self.tree.statements.in_split_else_body(first)
        {
            return None;
        }
        let group = self.tree.groups.closed_at(first)?;
        let open = self.tree.groups.get(group).open;
        let content = next_code_token(&self.tree.tokens, open + 1)?;
        if content == first
            || self.output.line_with_token(content)? != self.output.line_with_token(open)?
        {
            return None;
        }
        Some(self.token_column(open)? + self.case_unindent_spaces())
    }

    /// The first operand of a control condition whose `(` ends the header
    /// line stands a level past the header.
    fn condition_after_split_header_paren_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let open = self.tree.previous_code_token(first)?;
        let group = groups.opened_at(open)?;
        let keyword = self.tree.previous_code_token(open)?;
        if groups.get(group).delimiter != Delimiter::Paren
            || !matches!(&tokens[keyword], Token::Word(word) if matches!(word.as_str(), "if" | "while" | "switch"))
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_tokens(line)?.first != keyword {
            return None;
        }
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// A string literal that continues the literal ending the line before,
    /// past a comment, stands at that literal when it starts its line.
    fn string_concatenation_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let previous = self.tree.previous_code_token(first)?;
        if !matches!(tokens[first], Token::StringLiteral(_))
            || !matches!(tokens[previous], Token::StringLiteral(_))
            || self.tree.groups.enclosing(first) != self.tree.groups.enclosing(previous)
            || !tokens[previous + 1..first]
                .iter()
                .any(|token| matches!(token, Token::Comment(CommentKind::Block, text) if text.contains('\n')))
        {
            return None;
        }
        let line = self.output.line_with_token(previous)?;
        (self.output.line_tokens(line)?.first == previous).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    /// A value after a trailing `=` inside the parentheses of a call stands
    /// a level past the parentheses' first token.
    fn assigned_value_in_call_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let assign = self.tree.previous_code_token(first)?;
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if !matches!(&tokens[assign], Token::Operator(operator) if operator == "=")
            || groups.enclosing(assign) != Some(group)
            || groups.get(group).delimiter != Delimiter::Paren
            || !self.tree.previous_code_token(open).is_some_and(
                |callee| matches!(&tokens[callee], Token::Word(word) if !is_header(word)),
            )
        {
            return None;
        }
        let content = next_code_token(tokens, open + 1)?;
        let line = self.output.line_with_token(assign)?;
        if self.output.line_with_token(content)? != line
            || self.output.line_tokens(line)?.last != assign
        {
            return None;
        }
        Some(self.token_column(content)? + self.options.indent_width + self.case_unindent_spaces())
    }

    /// An argument after the `(` that ends the line of a call assigned
    /// with `=` stands a level past the call.
    fn argument_after_assigned_call_paren_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let open = self.tree.previous_code_token(first)?;
        let group = groups.opened_at(open)?;
        let callee = self.tree.previous_code_token(open)?;
        let assign = self.tree.previous_code_token(callee)?;
        if groups.get(group).delimiter != Delimiter::Paren
            || groups.enclosing(first) != Some(group)
            || !matches!(tokens[callee], Token::Word(_))
            || !matches!(&tokens[assign], Token::Operator(operator) if operator == "=")
            || tokens[open + 1..first]
                .iter()
                .any(|token| matches!(token, Token::Comment(..)))
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_with_token(assign)? != line {
            return None;
        }
        let column = self.token_column(callee)?
            + self.options.continuation_indent * self.options.indent_width;
        if column
            > self.output.lead_width(line, self.options.tab_width)
                + self.options.max_continuation_indent
        {
            return None;
        }
        Some(column + self.case_unindent_spaces())
    }

    /// A value after a trailing `=` of a statement inside a switch stands
    /// a level past the statement.
    fn assigned_value_in_case_block_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let assign = self.tree.previous_code_token(first)?;
        let block = groups.enclosing(first)?;
        if !matches!(&tokens[assign], Token::Operator(operator) if operator == "=")
            || matches!(tokens[first], Token::Symbol('{'))
            || groups.enclosing(assign) != Some(block)
            || groups.get(block).delimiter != Delimiter::Brace
            || !groups.ancestors(block).any(|id| {
                matches!(
                    self.tree
                        .previous_code_token(groups.get(id).open)
                        .map(|index| &tokens[index]),
                    Some(Token::Symbol(':'))
                ) || self.tree.blocks.owner(id).is_some_and(
                    |owner| matches!(&tokens[owner], Token::Word(word) if word == "switch"),
                )
            })
            || !matches!(
                self.tree.blocks.kind(block),
                Some(BlockKind::Control | BlockKind::Block) | None
            )
        {
            return None;
        }
        let line = self.output.line_with_token(assign)?;
        let start = self.output.line_tokens(line)?.first;
        if !self.tree.statements.starts_block_statement(start)
            && self.tree.statements.braceless_header(start).is_none()
        {
            return None;
        }
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The `}` of a block whose `{` starts its line stands at the `{`.
    fn block_closing_brace_indent(&self, first: usize) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol('}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
        {
            return None;
        }
        let block = self.tree.groups.closed_at(first)?;
        if !matches!(
            self.tree.blocks.kind(block),
            Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
        ) {
            return None;
        }
        let open = self.tree.groups.get(block).open;
        let tokens = &self.tree.tokens;
        // A `{` right after a directive stands where the branches leave it.
        if self.tree.previous_code_token(open).is_some_and(|header| {
            tokens[header + 1..open]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
        }) {
            return None;
        }
        // Branches with unbalanced braces and labels inside leave the
        // pairing of the braces to the engine.
        let mut branches: Vec<isize> = Vec::new();
        for (offset, token) in tokens[open + 1..first].iter().enumerate() {
            match token {
                Token::Preprocessor(directive) => match preprocessor_directive(&directive.text) {
                    Some("if" | "ifdef" | "ifndef") => branches.push(0),
                    Some("else" | "elif" | "elifdef" | "elifndef" | "endif") => {
                        if branches.pop().is_none_or(|depth| depth != 0) {
                            return None;
                        }
                        if !matches!(preprocessor_directive(&directive.text), Some("endif")) {
                            branches.push(0);
                        }
                    }
                    _ => {}
                },
                Token::Symbol('{') => {
                    if let Some(depth) = branches.last_mut() {
                        *depth += 1;
                    }
                }
                Token::Symbol('}') => {
                    if let Some(depth) = branches.last_mut() {
                        *depth -= 1;
                    }
                }
                Token::Word(word)
                    if matches!(word.as_str(), "case" | "default")
                        && self.tree.groups.enclosing(open + 1 + offset) == Some(block) =>
                {
                    return None;
                }
                _ => {}
            }
        }
        if !branches.is_empty() {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        let line_first = self.output.line_tokens(line)?.first;
        if line_first != open {
            // An attached `{` closes at its header's line where the style
            // aligns closing braces with headers.
            let groups = &self.tree.groups;
            let mut header = self.tree.previous_code_token(open)?;
            if let Some(condition) = groups.closed_at(header) {
                header = self.tree.previous_code_token(groups.get(condition).open)?;
            }
            if !matches!(&tokens[header], Token::Word(word)
                if matches!(word.as_str(), "if" | "else" | "for" | "while" | "switch" | "do"))
            {
                return None;
            }
            let is_word = |index: usize, text: &str| matches!(&tokens[index], Token::Word(word) if word == text);
            if is_word(header, "if")
                && let Some(before) = self.tree.previous_code_token(header)
                && is_word(before, "else")
            {
                header = before;
            }
            if is_word(header, "else")
                && let Some(before) = self.tree.previous_code_token(header)
                && matches!(tokens[before], Token::Symbol('}'))
            {
                header = before;
            }
            if header != line_first
                || !matches!(
                    self.options.brace_style,
                    BraceStyle::None
                        | BraceStyle::Allman
                        | BraceStyle::Attach
                        | BraceStyle::OneTrueBrace
                        | BraceStyle::WebKit
                )
                || self.options.indent_blocks
                || self.options.indent_braces
                || self.tree.groups.enclosing(line_first) != self.tree.groups.enclosing(open)
            {
                return None;
            }
            return Some(self.output.lead_width(line, self.options.tab_width));
        }
        if !self.output.as_slice()[line].trim_start().starts_with('{') {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width))
    }

    /// Statements of a case block whose brace is attached to its label
    /// stand a level past the label, or at the statement before them.
    fn case_block_statement_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        // Ratliff indents the closing brace and the code around it its own
        // way.
        if self.options.indent_cases || self.options.brace_style == BraceStyle::Ratliff {
            return None;
        }
        // A `}` with more code after it takes the layout of that code.
        let closing = groups.closed_at(first).filter(|_| {
            next_code_token(tokens, first + 1).is_none_or(|next| {
                matches!(tokens[next], Token::Symbol(';' | ','))
                    || self
                        .output
                        .pending_tokens()
                        .is_some_and(|span| next > span.last)
            })
        });
        if groups.closed_at(first).is_some() && closing.is_none() {
            return None;
        }
        if closing.is_none()
            && (!self.tree.statements.starts_block_statement(first)
                || matches!(tokens[first], Token::Symbol('{' | '}')))
        {
            return None;
        }
        let block = closing.or_else(|| groups.enclosing(first))?;
        let open = groups.get(block).open;
        let colon = self.tree.previous_code_token(open)?;
        if groups.get(block).delimiter != Delimiter::Brace
            || !matches!(tokens[colon], Token::Symbol(':'))
        {
            return None;
        }
        let label = self.tree.blocks.owner(block).and_then(|owner| {
            (0..=owner).rev().take_while(|&index| !matches!(tokens[index], Token::Symbol(';' | '{' | '}'))).find(|&index| {
                matches!(&tokens[index], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
            })
        })?;
        let line = self.output.line_with_token(label)?;
        if self.output.line_tokens(line)?.first != label
            || self.output.line_with_token(open) != Some(line)
        {
            return None;
        }
        let unindent =
            self.layout.line_adjuster.next_line_case_unindent_depth() * self.options.indent_width;
        if closing.is_some() {
            return Some(self.output.lead_width(line, self.options.tab_width) + unindent);
        }
        if self.tree.statements.block_opening(first) == Some(open) {
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + self.options.indent_width
                    + self.layout.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width,
            );
        }
        self.sibling_statement_column(first)
    }

    /// A statement after a case block whose brace is attached to its label
    /// stands at the label.
    fn statement_after_case_block_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let open = self.tree.statements.previous_sibling(first)?;
        let colon = self.tree.previous_code_token(open)?;
        if !matches!(tokens[open], Token::Symbol('{'))
            || !matches!(tokens[colon], Token::Symbol(':'))
        {
            return None;
        }
        let label = (0..colon)
            .rev()
            .take_while(|&index| !matches!(tokens[index], Token::Symbol(';' | '{' | '}')))
            .find(|&index| self.tree.statements.starts_block_statement(index))?;
        if !matches!(&tokens[label], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
        {
            return None;
        }
        let line = self.output.line_with_token(label)?;
        if self.output.line_tokens(line)?.first != label
            || self
                .output
                .line_with_token(open)
                .is_some_and(|brace| brace != line)
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
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
            let brace_type = if self.tree.blocks.kind(block) == Some(BlockKind::FunctionBody) {
                BraceType::Definition
            } else {
                BraceType::Command
            };
            let indented_brace = (self.options.indent_braces
                || matches!(
                    self.options.brace_style,
                    BraceStyle::Whitesmith | BraceStyle::Vtk
                ))
                && self.should_indent_brace_line(brace_type)
                || self.options.brace_style == BraceStyle::Ratliff
                    && brace_type == BraceType::Command;
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

/// First token of the element that ends at the `,` at `comma` in the
/// initializer `group`.
fn open_of_row_before(
    tokens: &[Token],
    groups: &crate::formatter::structure::groups::Groups,
    group: GroupId,
    comma: usize,
) -> Option<usize> {
    let open = groups.get(group).open;
    let separator = (open..comma).rev().find(|&index| {
        index == open
            || matches!(tokens[index], Token::Symbol(',')) && groups.enclosing(index) == Some(group)
    })?;
    next_code_token(tokens, separator + 1)
}
