//! Indents anchored in the structure tree.
//!
//! Layout heuristics read the text of earlier output lines, so comments,
//! continuation lines, and directives in between can mislead them. These
//! rules run last and take a statement's indent from lines that the tree
//! links it to: the `if` of an `else`, the header of a braceless body, the
//! previous statement of the block, and the block's opening.

use super::astyle_stack::literal_closed;
use crate::config::BraceStyle;
use crate::formatter::constructs::headers::starts_header_word;
use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::{FormatEngine, ForwardFind};
use crate::formatter::lexer::{CommentKind, Token, token_text};
use crate::formatter::output::model::LineLayout;
use crate::formatter::state::BraceType;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::syntax::language::{is_header, is_macro_like_word};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{ContainsAnyByte, preprocessor_directive};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;

/// A look back from a leading operator for its statement's assignment: the
/// tokens' address, the operator, its group, and the farthest `=` found, or
/// `None` when the look gave up.
pub(crate) type OperandAssignLook = (usize, usize, GroupId, Option<Option<usize>>);

/// The output line a token column walk read: the tokens' address, the
/// line, the output version, and the line text's address and length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TokenColumnKey {
    tokens: usize,
    line: usize,
    version: u64,
    text: (usize, usize),
}

/// The last token column walk: its line, the token it reached, the bytes
/// the token spans, and its visual column.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TokenColumnWalk {
    key: TokenColumnKey,
    token: usize,
    start: usize,
    end: usize,
    column: usize,
}

/// What a block's body takes its column from on the output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BodyAnchor {
    /// The lead of the line that the block's `{` starts.
    BraceLine(usize),
    /// The column past the block's header, when its line is known.
    Header(Option<usize>),
}

/// The last block body anchor read: the tokens' address, the block, the
/// output version then, the line of the `{`, and the anchor.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockBodyAnchor {
    address: usize,
    block: GroupId,
    version: u64,
    brace_line: usize,
    anchor: BodyAnchor,
}

/// The last look back from a member declarator's `,`: the tokens'
/// address, the `,`, the start of its declaration, and whether a `:` of the
/// body stands between them.
#[derive(Clone, Copy, Debug)]
pub(crate) struct MemberDeclaratorLook {
    address: usize,
    comma: usize,
    start: usize,
    colon: bool,
}

impl FormatEngine<'_> {
    /// The indent the syntax tree anchors the line starting at `first` to.
    pub(crate) fn tree_anchor_indent(&self, first: usize, line: &str) -> Option<usize> {
        // Most rules read a token of one kind at the line's start or right
        // before it; each runs only where that token is.
        let tokens = &self.tree.tokens;
        let token = &tokens[first];
        let previous_index = self.tree.previous_code_token(first);
        let previous = previous_index.map(|index| &tokens[index]);
        // Others read the group around the line's start.
        let group = self.tree.groups.enclosing(first);
        let delimiter = group.map(|group| self.tree.groups.get(group).delimiter);
        let in_parens = delimiter == Some(Delimiter::Paren);
        let in_initializer =
            group.is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer));
        // astyle's stack holds nothing for a line that starts a statement.
        let continues_stack =
            previous_index.is_some_and(|before| !self.ends_stack_statement(before));
        // Rules for a statement's place in its block read statements only.
        let starts_statement = self.tree.statements.starts_block_statement(first);
        let is_word =
            |token: &Token, wanted: &str| matches!(token, Token::Word(word) if word == wanted);
        let is_operator = |token: &Token, wanted: &str| matches!(token, Token::Operator(operator) if operator == wanted);
        let is_logical = |token: &Token| is_operator(token, "&&") || is_operator(token, "||");
        let opens_brace = matches!(token, Token::Symbol('{'));
        let closes_brace = matches!(token, Token::Symbol('}'));
        let brace = opens_brace || closes_brace;
        let leads_operator = matches!(token, Token::Operator(_));
        let string = matches!(token, Token::StringLiteral(_))
            && matches!(previous, Some(Token::StringLiteral(_)));
        let after_comma = matches!(previous, Some(Token::Symbol(',')));
        let after_colon = matches!(previous, Some(Token::Symbol(':')));
        let after_paren = matches!(previous, Some(Token::Symbol('(')));
        let after_assign = previous.is_some_and(|previous| is_operator(previous, "="));
        let after_logical = previous.is_some_and(is_logical);
        let parens_align = !self.options.indent_after_parens;
        let style = self.options.brace_style;
        let whitesmith = style == BraceStyle::Whitesmith;
        let vtk = style == BraceStyle::Vtk;
        let ratliff = style == BraceStyle::Ratliff;
        fn when(applies: bool, rule: impl FnOnce() -> Option<usize>) -> Option<usize> {
            if applies { rule() } else { None }
        }
        when(closes_brace || is_word(token, "else"), || {
            self.else_matching_if_indent(first)
        })
        .or_else(|| self.braceless_body_indent(first))
        .or_else(|| when(is_word(token, "while"), || self.do_while_indent(first)))
        .or_else(|| {
            when(!closes_brace && (opens_brace || starts_statement), || {
                self.split_else_block_statement_indent(first)
            })
        })
        .or_else(|| {
            when(!brace && starts_statement, || {
                self.statement_after_split_else_indent(first)
            })
        })
        .or_else(|| {
            when(closes_brace || starts_statement, || {
                self.dangling_else_block_indent(first)
            })
        })
        .or_else(|| when(is_word(token, "if"), || self.split_else_if_indent(first)))
        .or_else(|| {
            when(closes_brace || starts_statement, || {
                self.statement_expression_indent(first)
            })
        })
        .or_else(|| {
            when(
                previous.is_some_and(|previous| is_word(previous, "return")),
                || self.return_value_indent(first),
            )
        })
        .or_else(|| when(after_colon, || self.ternary_arm_in_parens_indent(first)))
        // An assignment registers a continuation level past its line
        // when parens indent after them.
        .or_else(|| {
            when(!parens_align && continues_stack, || {
                self.stacked_assignment_indent(first)
            })
        })
        .or_else(|| when(string, || self.assigned_string_continuation_indent(first)))
        .or_else(|| {
            when(in_parens, || {
                self.comment_interrupted_continuation_indent(first)
            })
        })
        .or_else(|| {
            when(after_comma && in_parens, || {
                self.argument_after_interruption_indent(first)
            })
        })
        .or_else(|| when(string, || self.string_concatenation_indent(first)))
        .or_else(|| when(in_parens, || self.stacked_argument_indent(first)))
        .or_else(|| {
            when(delimiter == Some(Delimiter::Bracket), || {
                self.stacked_bracket_row_indent(first)
            })
        })
        .or_else(|| self.stacked_initializer_row_indent(first))
        .or_else(|| when(continues_stack, || self.stacked_return_indent(first)))
        .or_else(|| {
            when(
                leads_operator && self.options.max_code_length.is_some(),
                || self.returned_operand_row_indent(first),
            )
        })
        .or_else(|| {
            when(matches!(token, Token::Symbol(')')), || {
                self.stacked_closing_paren_indent(first)
            })
        })
        .or_else(|| {
            when(parens_align && after_logical, || {
                self.logical_operand_in_parens_indent(first)
            })
        })
        .or_else(|| when(after_logical, || self.logical_chain_operand_indent(first)))
        .or_else(|| {
            when(matches!(token, Token::Word(_)) && after_comma, || {
                self.declarator_after_initializer_indent(first)
            })
        })
        .or_else(|| {
            when(opens_brace && (whitesmith || vtk), || {
                self.later_declarator_brace_indent(first)
            })
        })
        .or_else(|| when(after_comma, || self.declarator_after_comma_indent(first)))
        .or_else(|| {
            when(matches!(token, Token::Symbol(';')), || {
                self.leading_semicolon_indent(first)
            })
        })
        .or_else(|| {
            when(
                !brace
                    && previous.is_some_and(|previous| {
                        !matches!(previous, Token::Symbol(',' | ';' | '{' | '}'))
                    }),
                || self.assignment_continuation_indent(first),
            )
        })
        .or_else(|| {
            when(matches!(previous, Some(Token::Operator(_))), || {
                self.assigned_operand_indent(first)
            })
        })
        .or_else(|| {
            when(parens_align && leads_operator, || {
                self.leading_operator_assigned_value_indent(first)
            })
        })
        .or_else(|| {
            when(opens_brace || is_operator(token, "="), || {
                self.assignment_before_initializer_block_indent(first)
            })
        })
        .or_else(|| {
            when(is_operator(token, "="), || {
                self.leading_assignment_indent(first)
            })
        })
        .or_else(|| when(continues_stack, || self.stacked_assignment_indent(first)))
        .or_else(|| {
            when(matches!(token, Token::Symbol('?' | ':')), || {
                self.leading_ternary_in_condition_indent(first)
            })
        })
        .or_else(|| {
            when(matches!(token, Token::Symbol('?' | ':')), || {
                self.leading_ternary_in_argument_indent(first)
            })
        })
        .or_else(|| when(after_colon, || self.ternary_second_arm_indent(first)))
        .or_else(|| {
            when(parens_align && is_logical(token), || {
                self.leading_logical_in_parens_indent(first)
            })
        })
        .or_else(|| {
            when(is_operator(token, "="), || {
                self.enum_value_after_split_member_indent(first)
            })
        })
        .or_else(|| {
            when(opens_brace && (whitesmith || ratliff), || {
                self.indented_assigned_brace_row_indent(first)
            })
        })
        .or_else(|| {
            when(matches!(previous, Some(Token::Symbol(';'))), || {
                self.member_after_directive_indent(first)
            })
        })
        .or_else(|| {
            when(after_comma, || {
                self.member_declarator_after_comma_indent(first)
            })
        })
        .or_else(|| {
            when(whitesmith || ratliff || vtk, || {
                self.run_in_nested_array_row_indent(first)
            })
        })
        .or_else(|| when(vtk || whitesmith, || self.vtk_array_element_indent(first)))
        .or_else(|| when(vtk, || self.vtk_aggregate_array_row_indent(first)))
        .or_else(|| when(vtk, || self.vtk_initializer_first_element_indent(first)))
        .or_else(|| {
            when(opens_brace && (whitesmith || ratliff), || {
                self.whitesmith_brace_row_indent(first)
            })
        })
        .or_else(|| {
            when(opens_brace && ratliff, || {
                self.ratliff_first_brace_row_indent(first)
            })
        })
        .or_else(|| when(in_initializer, || self.initializer_row_indent(first)))
        // Horstmann and pico run the first row into its brace only later.
        .or_else(|| {
            when(
                matches!(style, BraceStyle::Horstmann | BraceStyle::Pico),
                || self.initializer_first_row_indent(first),
            )
        })
        .or_else(|| {
            when(!brace, || {
                self.initializer_element_continuation_indent(first)
            })
        })
        .or_else(|| {
            when(matches!(token, Token::Symbol(',')), || {
                self.initializer_leading_comma_indent(first)
            })
        })
        .or_else(|| {
            when(closes_brace, || {
                self.initializer_closing_brace_indent(first)
            })
        })
        .or_else(|| {
            when(closes_brace, || {
                self.nested_initializer_closing_brace_indent(first)
            })
        })
        .or_else(|| {
            when(opens_brace, || {
                self.one_line_control_block_indent(first, line)
                    .or_else(|| self.indented_block_brace_indent(first))
                    .or_else(|| self.broken_control_brace_indent(first))
                    .or_else(|| self.gnu_else_brace_indent(first))
                    .or_else(|| self.whitesmith_bare_block_brace_indent(first))
                    .or_else(|| self.whitesmith_macro_block_brace_indent(first))
                    .or_else(|| self.one_line_block_after_macro_indent(first))
                    .or_else(|| self.whitesmith_function_brace_indent(first))
            })
        })
        .or_else(|| when(vtk && brace, || self.vtk_knr_function_brace_indent(first)))
        .or_else(|| {
            when(vtk, || {
                self.vtk_anonymous_member_aggregate_brace_indent(first)
            })
        })
        .or_else(|| {
            when(starts_statement, || {
                self.statement_after_case_block_indent(first)
            })
        })
        .or_else(|| {
            when(after_colon && !brace, || {
                self.first_statement_after_case_label_indent(first)
            })
        })
        .or_else(|| {
            when(!brace && starts_statement, || {
                self.statement_after_labeled_statement_indent(first)
            })
        })
        .or_else(|| {
            when(
                !self.options.indent_cases && !ratliff && (closes_brace || starts_statement),
                || self.case_block_statement_indent(first),
            )
        })
        .or_else(|| {
            when((whitesmith || vtk) && !brace && starts_statement, || {
                self.broken_case_block_first_statement_indent(first)
            })
        })
        .or_else(|| {
            when(closes_brace, || {
                self.block_closing_brace_indent(first)
                    .or_else(|| self.case_block_closing_brace_indent(first))
            })
        })
        .or_else(|| {
            when(matches!(token, Token::Symbol(';')) || after_assign, || {
                self.assigned_value_in_case_block_indent(first)
            })
        })
        .or_else(|| {
            when(after_paren, || {
                self.argument_after_assigned_call_paren_indent(first)
            })
        })
        .or_else(|| when(after_assign, || self.assigned_value_in_call_indent(first)))
        .or_else(|| {
            when(after_paren, || {
                self.condition_after_split_header_paren_indent(first)
            })
        })
        .or_else(|| {
            when(matches!(token, Token::Symbol(')')), || {
                self.closing_paren_indent(first)
            })
        })
        .or_else(|| {
            previous_index.and_then(|previous| {
                when(parens_align, || self.parameter_line_indent(first, previous))
            })
        })
    }

    pub(crate) fn apply_tree_anchor_layout(
        &self,
        line: &LineView<'_>,
        mut layout: LineLayout,
    ) -> LineLayout {
        if layout.line_kind == LineKind::SwitchLabel
            && let Some(first) = self.output.pending_tokens().map(|span| span.first)
            && let Some(spaces) = self.sibling_case_label_column(first)
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        // A label split off the line of the bare label before it stands with
        // that label.
        if layout.line_kind == LineKind::SwitchLabel
            && self.output.pending_tokens().is_none()
            && let Some(previous) = self.output.last_non_empty_scoped()
            && {
                let code = self.output.code_trimmed_of(previous).trimmed_start();
                (code.starts_with("case ") || code.starts_with("default")) && code.ends_with(':')
            }
        {
            layout.exact_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width)
                    + self.layout.line_adjuster.pending_case_unindent() * self.options.indent_width,
            );
            return layout;
        }
        if layout.line_kind != LineKind::Normal || line.trimmed_start().starts_with('#') {
            return layout;
        }
        let first = self.output.pending_tokens().map(|span| span.first);
        // Tokens that the engine moved across lines map to no line of theirs,
        // and a brace the formatter added maps to no token at all.
        let Some(first) = first.filter(|&first| {
            line.trimmed_start()
                .starts_with(&*token_text(&self.tree.tokens[first]))
        }) else {
            if let Some(spaces) = self.added_closing_brace_indent(line) {
                layout.exact_indent_spaces = Some(spaces);
            }
            return layout;
        };
        if let Some(spaces) = self.tree_anchor_indent(first, line) {
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
        // The statements of such a block, and of blocks within an `else`
        // when else-if chains break, keep the column of the first one.
        if layout.exact_indent_spaces.is_none()
            && self.tree.statements.starts_block_statement(first)
            && self.tree.groups.enclosing(first).is_some_and(|group| {
                self.group_in_braced_chain_of_braceless_body(group)
                    || self.options.break_else_ifs
                        && self
                            .tree
                            .groups
                            .ancestors(group)
                            .any(|id| self.block_of_else(id))
            })
            && let Some(spaces) = self
                .sibling_statement_column(first)
                .or_else(|| self.block_body_column(first))
        {
            layout.exact_indent_spaces = Some(spaces);
            return layout;
        }
        let structural = layout.indent * self.options.indent_width;
        // Only a statement of a block has a sibling or a block column; the
        // block column is read only when the sibling leaves it a say.
        let starts_statement = self.tree.statements.starts_block_statement(first);
        let sibling = starts_statement
            .then(|| self.sibling_statement_column(first))
            .flatten();
        let block_cell = std::cell::OnceCell::new();
        let block = || {
            *block_cell.get_or_init(|| {
                starts_statement
                    .then(|| self.block_body_column(first))
                    .flatten()
            })
        };
        // Past a directive the engine keeps none of the levels it lost in
        // an `else` body split off by an empty line; the tree places them.
        if layout.exact_indent_spaces.is_none()
            && sibling.is_none()
            && block().is_none()
            && !matches!(self.tree.tokens[first], Token::Symbol('{' | '}'))
            && self.tree.statements.in_else_body_after_blank_line(first)
            && self.tree.statements.starts_block_statement(first)
            && let Some(column) = self.enclosing_block_body_column(first)
        {
            layout.exact_indent_spaces = Some(column);
            return layout;
        }
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
        ) && !matches!(self.tree.tokens[first], Token::Symbol('{' | '}'))
            && block().is_some()
            && (sibling.is_some() && sibling == block()
                && self
                    .tree
                    .groups
                    .enclosing(first)
                    .is_some_and(|group| self.group_in_braced_chain_of_braceless_body(group))
                // The engine loses levels in an `else` body split off by an
                // empty line; the tree places its blocks.
                || sibling.is_none_or(|sibling| Some(sibling) == block())
                    && self.tree.statements.in_else_body_after_blank_line(first)
                // A block's first statement stands at its brace.
                || self.options.brace_style != BraceStyle::Ratliff
                    && sibling.is_none()
                    && self.tree.statements.block_opening(first).is_some())
        {
            layout.exact_indent_spaces = block();
            return layout;
        }
        if sibling == Some(structural)
            || block() == Some(structural)
            || self
                .closing_brace_indent(first)
                .or_else(|| self.opening_brace_indent(first))
                == Some(structural)
        {
            // A heuristic moved the line off a structural level that the
            // tree confirms; an anchor off that level is itself misplaced.
            layout.exact_indent_spaces = Some(structural);
        } else if sibling.is_some()
            && sibling == block()
            && layout.exact_indent_spaces != sibling
            && (!matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
            ) || self.follows_added_one_line_block())
            && !matches!(self.tree.tokens[first], Token::Symbol('{' | '}'))
            && self.tree.statements.braceless_header(first).is_none()
            // Directives opening or closing a group leave the code where
            // they found it; alternative branches do not.
            && !self
                .tree
                .statements
                .previous_sibling(first)
                .is_some_and(|previous| {
                    // Groups opened and closed between the two hold both
                    // their branches.
                    let mut depth = 0usize;
                    self.tree.has_directive_in(previous..first)
                        && self.tree.tokens[previous..first].iter().any(|token| {
                        let Token::Preprocessor(directive) = token else {
                            return false;
                        };
                        match preprocessor_directive(&directive.text) {
                            Some("if" | "ifdef" | "ifndef") => depth += 1,
                            Some("endif") => depth = depth.saturating_sub(1),
                            Some("else" | "elif" | "elifdef" | "elifndef") => return depth == 0,
                            _ => {}
                        }
                        false
                    })
                })
        {
            // The statement's sibling and its block agree on a column the
            // engine missed, as after a braceless chain closed by `{}`.
            layout.exact_indent_spaces = block();
        } else if sibling.is_none()
            && block().is_some()
            && self
                .tree
                .statements
                .block_opening(first)
                .is_some_and(|open| {
                    self.brace_stands_at_its_header(open)
                        || self.brace_attached_to_control_header(open)
                        || self.tree.statements.branch_header_of_block(open).is_some()
                        || self.tree.statements.in_else_body_after_blank_line(open)
                })
            && !matches!(self.tree.tokens[first], Token::Symbol('{' | '}'))
            && Some(layout.exact_indent_spaces.unwrap_or(structural)) < block()
        {
            // The first statement of a block stands at its body, which the
            // engine may lose in a long `else if` chain.
            layout.exact_indent_spaces = block();
        } else if matches!(self.tree.tokens[first], Token::Symbol('{'))
            && sibling == Some(structural + self.options.indent_width)
        {
            // An indented block brace stands a level past its statement's.
            layout.exact_indent_spaces = sibling;
        }
        layout
    }

    /// Whether the `{` at `open` of a control block ends its header's line.
    fn brace_attached_to_control_header(&self, open: usize) -> bool {
        self.tree
            .groups
            .opened_at(open)
            .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Control))
            && self.output.line_with_token(open).is_some_and(|line| {
                self.output
                    .line_tokens(line)
                    .is_some_and(|span| span.first != open && span.last == open)
            })
    }

    /// Whether the `{` at `open` of a block in an `else` chain starts a line
    /// where its control header puts it.
    fn brace_stands_at_its_header(&self, open: usize) -> bool {
        let tokens = &self.tree.tokens;
        let is_else = |index: usize| matches!(&tokens[index], Token::Word(word) if word == "else");
        let Some(line) = self.output.line_with_token(open) else {
            return false;
        };
        let in_else_chain = self.tree.previous_code_token(open).is_some_and(is_else)
            || self
                .header_keyword_before(open)
                .is_some_and(|keyword| self.tree.previous_code_token(keyword).is_some_and(is_else));
        in_else_chain
            && self
                .output
                .line_tokens(line)
                .is_some_and(|span| span.first == open)
            && self
                .broken_control_brace_indent(open)
                .or_else(|| self.indented_block_brace_indent(open))
                .is_some_and(|spaces| {
                    spaces == self.output.lead_width(line, self.options.tab_width)
                })
    }

    /// Whitesmith indents a brace row a level past the plain element row
    /// before it, as it indents braces past their statements.
    #[inline(never)]
    fn whitesmith_brace_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Ratliff
        ) || !matches!(tokens[first], Token::Symbol('{'))
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

    /// Ratliff indents a first brace row a level past the rows of an
    /// initializer whose `{` ends a line.
    #[inline(never)]
    fn ratliff_first_brace_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Ratliff
            || !matches!(tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || self.tree.previous_code_token(first) != Some(open)
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        let span = self.output.line_tokens(line)?;
        if span.first == open || span.last != open {
            return None;
        }
        // An aggregate closed on the line stands at its body; a declaration
        // split over lines starts on its first.
        let lead = if matches!(tokens[span.first], Token::Symbol('}')) {
            self.output
                .lead_width(line, self.options.tab_width)
                .saturating_sub(self.options.indent_width)
        } else {
            let mut start = line;
            while start > 0 {
                let code = self.output.code(start - 1).trimmed_end();
                if code.is_empty()
                    || self.output.line_tokens(start - 1).is_none()
                    || code.ends_with_any(b";{},")
                    || code.trimmed_start().starts_with('#')
                {
                    break;
                }
                start -= 1;
            }
            self.output.lead_width(start, self.options.tab_width)
        };
        Some(lead + 2 * self.options.indent_width + self.case_unindent_spaces())
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
        // A last brace row after a brace row spanning lines stands at it.
        let after_split_brace_row = self.options.brace_style != BraceStyle::Vtk
            && open_of_row_before(tokens, groups, group, comma)
                .filter(|&element| matches!(tokens[element], Token::Symbol('{')))
                .and_then(|element| {
                    let close = groups.get(groups.opened_at(element)?).close?;
                    Some(
                        self.output.line_with_token(close)?
                            != self.output.line_with_token(element)?,
                    )
                })
                .unwrap_or(false);
        if separated.is_none()
            && matches!(tokens[first], Token::Symbol('{'))
            && self.should_indent_brace_line(BraceType::Initializer)
            && !after_split_brace_row
            && !(self.options.brace_style == BraceStyle::Vtk
                && open_of_row_before(tokens, groups, group, comma).is_some_and(|element| {
                    matches!(tokens[element], Token::Symbol('{'))
                        && (self.in_code(first)
                            || groups
                                .opened_at(element)
                                .and_then(|row| groups.get(row).close)
                                .is_some_and(|close| {
                                    self.output.line_with_token(close)
                                        == self.output.line_with_token(element)
                                }))
                }))
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
        // So does a row after a nested VTK group closed at its elements.
        // Whitesmith and Ratliff indent such a `}` on its own line past the
        // rows, which the engine would follow.
        let after_block_like_close = self
            .tree
            .previous_code_token(comma)
            .and_then(|close| groups.closed_at(close).map(|nested| (close, nested)))
            .is_some_and(|(close, nested)| match self.options.brace_style {
                BraceStyle::Vtk => self.vtk_nested_rows_like_blocks(nested),
                BraceStyle::Whitesmith | BraceStyle::Ratliff => {
                    self.tree.blocks.kind(nested) == Some(BlockKind::Initializer)
                        && self.output.line_with_token(close).is_some_and(|line| {
                            self.output
                                .line_tokens(line)
                                .is_some_and(|span| span.first == close)
                        })
                }
                _ => false,
            });
        // A brace row after a brace row starting its line stands at it.
        if !after_comment
            && matches!(tokens[first], Token::Symbol('{'))
            && let Some(element) = open_of_row_before(tokens, groups, group, comma)
            && matches!(tokens[element], Token::Symbol('{'))
            && let Some(line) = self.output.line_with_token(element)
            && self.output.line_tokens(line)?.first == element
        {
            return Some(
                self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces(),
            );
        }
        if !after_comment
            && !after_block_like_close
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
        // Elements run in after the group's brace stand at the first.
        if (row == open
            || matches!(&tokens[element], Token::Symbol('.'))
                && self.output.line_with_token(open) == Some(line))
            && next_code_token(tokens, open + 1) == Some(element)
        {
            return Some(self.token_column(element)? + self.case_unindent_spaces());
        }
        // The row may follow a comment on its line.
        let text = self.output.as_slice()[line].trimmed_start();
        let text = text
            .strip_prefix("/*")
            .and_then(|rest| rest.split_once("*/"))
            .map_or(text, |(_, rest)| rest.trimmed_start());
        if !text.starts_with(&*token_text(&tokens[row])) {
            return None;
        }
        // A `}, {` row closes the row before it on the element's line.
        let closes_row_before = matches!(tokens[row], Token::Symbol('}'))
            && groups
                .closed_at(row)
                .is_some_and(|closed| groups.get(closed).parent == Some(group))
            && next_code_token(tokens, row + 1)
                .is_some_and(|comma| next_code_token(tokens, comma + 1) == Some(element));
        if row != element
            && !closes_row_before
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

    /// VTK and whitesmith lay out an array initializer whose braces start their lines
    /// like blocks: the elements of a group stand at its `{`, and a nested
    /// `{` starting its line one level past them. At file scope the outer
    /// `{` stays in column one while its elements take the level of an
    /// indented brace. Aggregates of a `struct` keep their rows at the
    /// elements.
    fn vtk_array_element_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Vtk | BraceStyle::Whitesmith
        ) {
            return None;
        }
        let is_aggregate = |id: GroupId| {
            matches!(
                self.tree.blocks.kind(id),
                Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
            )
        };
        // A `}` starting its line closes at its `{` when that starts a line.
        if matches!(tokens[first], Token::Symbol('}')) {
            let closed = groups.closed_at(first)?;
            let open = groups.get(closed).open;
            let line = self.output.line_with_token(open)?;
            if !is_aggregate(closed)
                || self.output.line_tokens(line)?.first != open
                || self.vtk_array_outer(closed).is_none()
                || closed == self.vtk_array_outer(closed)?
            {
                return None;
            }
            return Some(
                self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces(),
            );
        }
        // A compound literal's `{` starting its line after its cast stands
        // a level past the line of the cast.
        let (group, cast_brace) = match groups.opened_at(first) {
            Some(literal)
                if is_aggregate(literal)
                    && self
                        .tree
                        .previous_code_token(first)
                        .is_some_and(|close| matches!(tokens[close], Token::Symbol(')'))) =>
            {
                (groups.enclosing(first)?, true)
            }
            _ => (groups.enclosing(first)?, false),
        };
        if !is_aggregate(group) {
            return None;
        }
        let open = groups.get(group).open;
        if !cast_brace {
            let previous = self.tree.previous_code_token(first)?;
            if !(previous == open
                || matches!(tokens[previous], Token::Symbol(','))
                    && groups.enclosing(previous) == Some(group))
            {
                return None;
            }
        }
        let outer = self.vtk_array_outer(group)?;
        if cast_brace {
            let close = self.tree.previous_code_token(first)?;
            let line = self.output.line_with_token(close)?;
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + self.options.indent_width
                    + self.case_unindent_spaces(),
            );
        }
        let open_line = self.output.line_with_token(open)?;
        if self.output.line_tokens(open_line)?.first != open
            || self.tree.has_directive_in(open..first)
        {
            return None;
        }
        let flush_outer =
            self.options.brace_style == BraceStyle::Vtk && group == outer && !self.in_code(open);
        let mut content = self.output.lead_width(open_line, self.options.tab_width);
        if flush_outer {
            content += self.options.indent_width;
        }
        // A `{` starting its line stands a level in; at file scope, a row
        // of the outer braces that starts with its elements stays at them.
        // A row led by a comment stays at the elements.
        let after_comment = tokens[..first]
            .iter()
            .rev()
            .find(|token| !matches!(token, Token::Whitespace(_)))
            .is_some_and(|token| matches!(token, Token::Comment(..)));
        let brace_alone = tokens[first + 1..]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_) | Token::Comment(..)))
            .is_none_or(|token| matches!(token, Token::Newline));
        // A row split over lines breaks after its `{`.
        let row_spans_lines = groups
            .opened_at(first)
            .and_then(|row| groups.get(row).close)
            .is_some_and(|close| {
                tokens[first..close]
                    .iter()
                    .any(|token| matches!(token, Token::Newline))
            });
        let indented = matches!(tokens[first], Token::Symbol('{'))
            && !after_comment
            && (brace_alone || row_spans_lines || !flush_outer);
        let spaces = if indented {
            content + self.options.indent_width
        } else {
            content
        };
        Some(spaces + self.case_unindent_spaces())
    }

    /// Styles that indent braces lay out the rows of an initializer whose
    /// attached `{` runs a nested `{` in as if the outer `{` stood on its
    /// own line: rows two levels past the declaration, the `}` one.
    #[inline(never)]
    fn run_in_nested_array_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Ratliff | BraceStyle::Vtk
        ) {
            return None;
        }
        let (group, levels) = if matches!(tokens[first], Token::Symbol('}')) {
            (groups.closed_at(first)?, 1)
        } else {
            let group = groups.enclosing(first)?;
            let previous = self.tree.previous_code_token(first)?;
            if !matches!(tokens[previous], Token::Symbol(','))
                || groups.enclosing(previous) != Some(group)
            {
                return None;
            }
            (group, 2)
        };
        let open = groups.get(group).open;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || groups
                .get(group)
                .parent
                .is_some_and(|parent| self.tree.blocks.kind(parent) == Some(BlockKind::Initializer))
            || (self.options.brace_style == BraceStyle::Vtk && !self.in_code(open))
            || !next_code_token(tokens, open + 1)
                .is_some_and(|element| matches!(tokens[element], Token::Symbol('{')))
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        let span = self.output.line_tokens(line)?;
        let element = next_code_token(tokens, open + 1)?;
        if span.first == open || self.output.line_with_token(element) != Some(line) {
            return None;
        }
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + levels * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// VTK leaves the rows of a file-scope initializer declared with an
    /// aggregate keyword, brace rows too, a level past its `{` line.
    #[inline(never)]
    fn vtk_aggregate_array_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Vtk
            || matches!(tokens[first], Token::Symbol('}'))
        {
            return None;
        }
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        let previous = self.tree.previous_code_token(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || groups.get(group).parent.is_some()
            || self.in_code(open)
            || self.vtk_array_outer(group).is_some()
            || !(previous == open
                || matches!(tokens[previous], Token::Symbol(','))
                    && groups.enclosing(previous) == Some(group))
            || !self.tree.previous_code_token(open).is_some_and(
                |before| matches!(&tokens[before], Token::Operator(operator) if operator == "="),
            )
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        Some(self.output.lead_width(line, self.options.tab_width) + self.options.indent_width)
    }

    /// Whether VTK lays out the nested initializer `group` like a block:
    /// in code, or in an array of no aggregate keyword.
    fn vtk_nested_rows_like_blocks(&self, group: GroupId) -> bool {
        let groups = &self.tree.groups;
        groups
            .get(group)
            .parent
            .is_some_and(|parent| self.tree.blocks.kind(parent) == Some(BlockKind::Initializer))
            && (self.in_code(groups.get(group).open) || self.vtk_array_outer(group).is_some())
    }

    /// The outermost initializer around `group` when it is the assigned
    /// initializer of a declaration without an aggregate keyword.
    fn vtk_array_outer(&self, group: GroupId) -> Option<GroupId> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let outer = groups
            .ancestors(group)
            .take_while(|&id| {
                matches!(
                    self.tree.blocks.kind(id),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
            })
            .last()?;
        let outer_open = groups.get(outer).open;
        if !self.tree.previous_code_token(outer_open).is_some_and(
            |before| matches!(&tokens[before], Token::Operator(operator) if operator == "="),
        ) {
            return None;
        }
        let mut start = outer_open;
        while let Some(before) = self.tree.previous_code_token(start) {
            if matches!(tokens[before], Token::Symbol(';' | '{' | '}')) {
                break;
            }
            if matches!(&tokens[before], Token::Word(word)
                if matches!(word.as_str(), "struct" | "union" | "class" | "enum"))
            {
                return None;
            }
            start = before;
        }
        Some(outer)
    }

    /// VTK places the first element of an assigned initializer whose `{`
    /// starts its line a level past the `{`.
    #[inline(never)]
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
            || !self.output.as_slice()[line]
                .trimmed_start()
                .starts_with('{')
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
    #[inline(never)]
    fn nested_initializer_closing_brace_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        if !matches!(self.tree.tokens[first], Token::Symbol('}')) {
            return None;
        }
        let group = groups.closed_at(first)?;
        let open = groups.get(group).open;
        // VTK closes a nested group at its elements in code and in arrays
        // of no aggregate keyword.
        let depth = match self.options.brace_style {
            BraceStyle::Whitesmith | BraceStyle::Ratliff => 1,
            BraceStyle::Vtk if self.vtk_nested_rows_like_blocks(group) => 1,
            BraceStyle::Vtk => 2,
            _ => return None,
        };
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
        // A group opened on a `}, {` row closes at that row.
        let row_extra = if matches!(self.tree.tokens[span.first], Token::Symbol('}')) {
            0
        } else {
            self.options.indent_width
        };
        (span.first != open && span.last == open).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + row_extra
                + self.case_unindent_spaces()
        })
    }

    /// A `}` closing an initializer whose `{` starts its own line stands at
    /// that line.
    #[inline(never)]
    fn initializer_closing_brace_indent(&self, first: usize) -> Option<usize> {
        // VTK, GNU and Horstmann close some initializers their own way;
        // VTK closes a nested row at its brace like a block.
        if !matches!(self.tree.tokens[first], Token::Symbol('}')) {
            return None;
        }
        let group = self.tree.groups.closed_at(first)?;
        if matches!(
            self.options.brace_style,
            BraceStyle::Gnu | BraceStyle::Horstmann
        ) || self.options.brace_style == BraceStyle::Vtk
            && !self.vtk_nested_rows_like_blocks(group)
        {
            return None;
        }
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer) {
            return None;
        }
        let open = self.tree.groups.get(group).open;
        let line = self.output.line_with_token(open)?;
        // A brace attached to the line before after layout closes its way.
        (self.output.line_tokens(line)?.first == open
            && self.output.as_slice()[line]
                .trimmed_start()
                .starts_with('{'))
        .then(|| self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// The line after a statement's first line holding assignments stands
    /// where astyle's continuation stack leaves it: each `=` stacks the
    /// column of its value or, past the maximum, two levels, never below
    /// the indent before; a trailing `=` stacks a continuation level more.
    #[inline(never)]
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
        let mut start = next_code_token(tokens, self.output.line_tokens(line)?.first)?;
        // The statement may run in after its headers.
        while matches!(&tokens[start], Token::Word(word) if matches!(word.as_str(), "if" | "while" | "for"))
            && let Some(open) = next_code_token(tokens, start + 1)
            && matches!(tokens[open], Token::Symbol('('))
            && let Some(close) = groups
                .opened_at(open)
                .and_then(|group| groups.get(group).close)
            && close < last
            && self.output.line_with_token(close) == Some(line)
        {
            start = next_code_token(tokens, close + 1)?;
        }
        if groups.enclosing(start) != group
            || self.tree.previous_code_token(start).is_some_and(|before| {
                !matches!(tokens[before], Token::Symbol(';' | '{' | '}'))
                    && !matches!(&tokens[before], Token::Word(word) if word == "else" || word == "do")
                    && !groups.closed_at(before).is_some_and(|condition| {
                        self.tree
                            .previous_code_token(groups.get(condition).open)
                            .is_some_and(|keyword| {
                                matches!(&tokens[keyword], Token::Word(word)
                                    if matches!(word.as_str(), "if" | "while" | "for"))
                            })
                    })
            })
            || self.tree.has_directive_in(start..first)
        {
            return None;
        }
        let lead = self.output.lead_width(line, self.options.tab_width);
        let max = self.options.max_continuation_indent;
        let mut stack: Vec<usize> = Vec::new();
        let mut index = start;
        loop {
            match &tokens[index] {
                Token::Word(word)
                    if word == "return"
                        || word == "template" && next_code_token(tokens, index + 1).is_some_and(|next| matches!(&tokens[next], Token::Operator(operator) if operator.starts_with('<')))
                        || is_header(word) =>
                {
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
        // A statement run in after its header continues a level in.
        Some(
            lead + top
                + self.run_in_header_levels(start) * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The first statement of a case starting a line after its label's
    /// line takes one level past the label.
    #[inline(never)]
    fn first_statement_after_case_label_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if matches!(tokens[first], Token::Symbol('{' | '}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
            || self.layout.line_adjuster.pending_case_unindent() > 0
        {
            return None;
        }
        let colon = self.tree.previous_code_token(first)?;
        if !matches!(tokens[colon], Token::Symbol(':')) {
            return None;
        }
        let colon_line = self.output.line_with_token(colon)?;
        let colon_span = self.output.line_tokens(colon_line)?;
        let last = (colon_span.first..=colon_span.last)
            .rev()
            .find(|&index| is_code_token(&tokens[index]))?;
        // A label continued over lines starts at its `case`.
        let mut label = colon;
        while let Some(before) = self.tree.previous_code_token(label) {
            if matches!(tokens[before], Token::Symbol(';' | '{' | '}' | ':')) {
                break;
            }
            label = before;
            if matches!(&tokens[label], Token::Word(word) if word == "case" || word == "default") {
                break;
            }
        }
        let line = self.output.line_with_token(label)?;
        if self.output.line_tokens(line)?.first != label {
            return None;
        }
        if last != colon
            || !matches!(&tokens[label], Token::Word(word) if word == "case" || word == "default")
            || groups.enclosing(label) != groups.enclosing(first)
            || !self.in_switch_body(first)
            || self.tree.has_directive_in(colon..first)
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.options.indent_width)
    }

    /// A statement after a labeled statement, which links to no sibling,
    /// stands at the statements of its block.
    #[inline(never)]
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
        // Directives inside the labeled statement's own blocks leave its
        // level alone.
        if (label..first).any(|index| {
            matches!(tokens[index], Token::Preprocessor(_)) && groups.enclosing(index) == group
        }) {
            return None;
        }
        let in_switch = group
            .and_then(|group| self.tree.blocks.owner(group))
            .is_some_and(|owner| match &tokens[owner] {
                Token::Word(word) if word == "switch" => true,
                // Horstmann runs case blocks in at their labels.
                Token::Word(word) if word == "case" || word == "default" => {
                    self.options.brace_style != BraceStyle::Horstmann
                }
                _ => false,
            });
        if !in_switch {
            return self.enclosing_block_body_column(first);
        }
        // In a switch or a case block, the statements of the case body
        // before the label.
        let mut index = label;
        loop {
            let before = statement_before(index)?;
            if matches!(&tokens[before], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
            {
                let line = self.line_led_by(before)?;
                return Some(
                    self.output.lead_width(line, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            if !is_label(before)
                && statements.braceless_header(before).is_none()
                && let Some(line) = self.line_led_by(before)
            {
                return Some(self.output.lead_width(line, self.options.tab_width));
            }
            index = before;
        }
    }

    /// An argument after a comma ending the line before stands at the first
    /// argument when that follows its paren. The stack replay places these
    /// unless the code length is limited.
    fn argument_after_comma_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        self.options.max_code_length?;
        let group = groups.enclosing(first)?;
        let previous = self.tree.previous_code_token(first)?;
        if groups.get(group).delimiter != Delimiter::Paren
            || !matches!(tokens[previous], Token::Symbol(','))
            || groups.enclosing(previous) != Some(group)
        {
            return None;
        }
        let open = groups.get(group).open;
        let argument = next_code_token(tokens, open + 1)?;
        let line = self.output.line_with_token(open)?;
        if self.output.line_with_token(argument)? != line || argument >= first {
            return None;
        }
        Some(self.token_column(argument)? + self.case_unindent_spaces())
    }

    /// A line that opens with a block comment and continues with a
    /// statement stands where the tree puts the statement: the engine lays
    /// such lines out as comments.
    pub(crate) fn comment_led_statement_line(&self, line: &str) -> Option<String> {
        let tokens = &self.tree.tokens;
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with("/*") {
            return None;
        }
        let code = self.output.pending_tokens().map(|span| span.first)?;
        // Only a comment that closes on the line, with the statement after it.
        let close = trimmed.find("*/")?;
        if !is_code_token(&tokens[code])
            || !trimmed[close + 2..]
                .trimmed_start()
                .starts_with(&*token_text(&tokens[code]))
            || matches!(&tokens[code], Token::Word(word) if word == "case" || word == "default")
        {
            return None;
        }
        let spaces = self
            .braceless_body_indent(code)
            .or_else(|| {
                self.tree
                    .statements
                    .starts_block_statement(code)
                    .then(|| self.sibling_statement_column(code))
                    .flatten()
            })
            .or_else(|| self.vtk_array_element_indent(code))
            .or_else(|| self.initializer_row_indent(code))
            .or_else(|| self.initializer_first_row_indent(code))
            .or_else(|| self.stacked_argument_indent(code))
            .or_else(|| self.argument_after_comma_column(code));
        let spaces = spaces?;
        let mut output = self
            .options
            .continuation_indent_prefix(spaces / self.options.indent_width.max(1), spaces);
        output.push_str(trimmed);
        Some(output)
    }

    /// An element continued over lines inside an initializer whose `{` has
    /// elements after it on its line stands at the first of them, as astyle
    /// registers that column for the brace.
    #[inline(never)]
    fn initializer_element_continuation_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || matches!(tokens[first], Token::Symbol('{' | '}'))
        {
            return None;
        }
        let open = groups.get(group).open;
        let previous = self.tree.previous_code_token(first)?;
        if previous == open
            || matches!(tokens[previous], Token::Symbol(','))
            || matches!(&tokens[previous], Token::Operator(operator) if operator == "=")
        {
            return None;
        }
        let element = next_code_token(tokens, open + 1)?;
        let line = self.output.line_with_token(open)?;
        if self.output.line_with_token(element) != Some(line) {
            return None;
        }
        Some(self.token_column(element)? + self.case_unindent_spaces())
    }

    /// A comma starting its line in an initializer stands at the line where
    /// the element it ends starts.
    #[inline(never)]
    fn initializer_leading_comma_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(tokens[first], Token::Symbol(',')) {
            return None;
        }
        let group = groups.enclosing(first)?;
        if !matches!(
            self.tree.blocks.kind(group),
            Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
        ) {
            return None;
        }
        let open = groups.get(group).open;
        let separator = (open..first).rev().find(|&index| {
            index == open
                || matches!(tokens[index], Token::Symbol(','))
                    && groups.enclosing(index) == Some(group)
        })?;
        let element = next_code_token(tokens, separator + 1)?;
        // Styles that indent brace rows keep the comma at the elements.
        if element >= first
            || matches!(tokens[element], Token::Symbol('{'))
                && self.should_indent_brace_line(BraceType::Initializer)
        {
            return None;
        }
        let line = self.output.line_with_token(element)?;
        let line_first = self.output.line_tokens(line)?.first;
        // Or at the line of elements holding it.
        if line_first != element
            && (line_first <= open || groups.enclosing(line_first) != Some(group))
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Whitesmith and ratliff indent an initializer `{` starting its line
    /// after an `=` that ends the line before, elements following it or not.
    #[inline(never)]
    fn indented_assigned_brace_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Ratliff
        ) || !matches!(tokens[first], Token::Symbol('{'))
            || !self
                .tree
                .groups
                .opened_at(first)
                .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer))
        {
            return None;
        }
        let assign = self.tree.previous_code_token(first)?;
        if !matches!(&tokens[assign], Token::Operator(operator) if operator == "=") {
            return None;
        }
        let assign_line = self.output.line_with_token(assign)?;
        if self.output.line_tokens(assign_line)?.last != assign {
            return None;
        }
        // The brace stands a level past the statement's first line.
        let mut start = assign;
        while let Some(before) = self.tree.previous_code_token(start) {
            // Past parens and a struct body the declaration defines.
            if let Some(group) = self.tree.groups.closed_at(before)
                && (self.tree.groups.get(group).delimiter != Delimiter::Brace
                    || self.tree.blocks.kind(group) == Some(BlockKind::Aggregate))
            {
                start = self.tree.groups.get(group).open;
                continue;
            }
            if matches!(tokens[before], Token::Symbol(';' | '{' | '}')) {
                break;
            }
            start = before;
        }
        let line = self.output.line_with_token(start)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// Declarators of a member continued after the `,` ending its first line
    /// stand at the member's second word, shifted as astyle counts the tabs
    /// before that comma.
    #[inline(never)]
    fn member_declarator_after_comma_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let body = groups.enclosing(first)?;
        let comma = self.tree.previous_code_token(first)?;
        if self.tree.blocks.kind(body) != Some(BlockKind::Aggregate)
            || !matches!(tokens[comma], Token::Symbol(','))
            || groups.enclosing(comma) != Some(body)
        {
            return None;
        }
        let open = groups.get(body).open;
        // Enumerators are no declarators.
        let mut head = open;
        while let Some(before) = self.tree.previous_code_token(head) {
            if matches!(tokens[before], Token::Symbol(';' | '{' | '}')) {
                break;
            }
            if matches!(&tokens[before], Token::Word(word) if word == "enum") {
                return None;
            }
            head = before;
        }
        // The look back depends on nothing but where it stands, so it ends
        // where the last one did once it reaches the `,` that one left.
        let address = tokens.as_ptr() as usize;
        let cached = self
            .member_declarator_cache
            .get()
            .filter(|look| look.address == address && look.comma <= comma);
        let mut start = comma;
        while let Some(before) = self.tree.previous_code_token(start) {
            if let Some(look) = cached
                && start == look.comma
            {
                start = look.start;
                break;
            }
            if before == open
                || matches!(tokens[before], Token::Symbol(';'))
                    && groups.enclosing(before) == Some(body)
            {
                break;
            }
            start = groups
                .closed_at(before)
                .map_or(before, |group| groups.get(group).open);
        }
        // Whether a `:` of the body stands before the `,`, read on from the
        // last look that started where this one does.
        let body_colon = |from: usize, to: usize| {
            (from..to).any(|index| {
                matches!(tokens[index], Token::Symbol(':')) && groups.enclosing(index) == Some(body)
            })
        };
        let colon = match cached.filter(|look| look.start == start) {
            Some(look) => look.colon || body_colon(look.comma, comma),
            None => body_colon(start, comma),
        };
        self.member_declarator_cache.set(Some(MemberDeclaratorLook {
            address,
            comma,
            start,
            colon,
        }));
        // A declarator row after a row that continues the declaration
        // stands at that row.
        if let Some(comma_line) = self.output.line_with_token(comma)
            && let Some(row_first) = self
                .output
                .line_tokens(comma_line)
                .and_then(|span| next_code_token(tokens, span.first))
            && row_first < comma
            && row_first > start
            && (matches!(&tokens[row_first], Token::Operator(operator)
                    if matches!(operator.as_str(), "*" | "**" | "&" | "&&"))
                || self
                    .tree
                    .previous_code_token(row_first)
                    .is_some_and(|before| {
                        matches!(tokens[before], Token::Symbol(','))
                            && groups.enclosing(before) == Some(body)
                    }))
            && !colon
        {
            return Some(
                self.output.lead_width(comma_line, self.options.tab_width)
                    + self.case_unindent_spaces(),
            );
        }
        let line = self.output.line_with_token(start)?;
        let span = self.output.line_tokens(line)?;
        let registering = (start..=span.last)
            .rev()
            .find(|&index| is_code_token(&tokens[index]))?;
        let second = next_code_token(tokens, start + 1)?;
        if span.first != start
            || !matches!(tokens[registering], Token::Symbol(','))
            || !matches!(&tokens[start], Token::Word(word)
                if !matches!(word.as_str(), "struct" | "union" | "class" | "enum"))
            || !matches!(tokens[second], Token::Word(_))
            || second >= registering
            || self.tree.has_directive_in(start..first)
        {
            return None;
        }
        let lead = self.output.lead_width(line, self.options.tab_width);
        // astyle registers no second word after a first one under three
        // characters.
        if token_text(&tokens[start]).len() < 3 {
            return Some(lead + self.case_unindent_spaces());
        }
        // astyle adds the widths its tabs gain before the comma.
        let text = &self.output.as_slice()[line];
        let comma_column = self.token_column(registering)?;
        let mut column = lead;
        let mut chars = 0;
        for ch in text.trimmed_start().chars() {
            if column >= comma_column {
                break;
            }
            column = if ch == '\t' {
                (column / self.options.tab_width + 1) * self.options.tab_width
            } else {
                column + 1
            };
            chars += 1;
        }
        let tab_gain = (comma_column - lead).saturating_sub(chars);
        Some(self.token_column(second)? + tab_gain + self.case_unindent_spaces())
    }

    /// A member of a struct body after a directive or a comment stands at
    /// the member before it.
    #[inline(never)]
    fn member_after_directive_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let body = groups.enclosing(first)?;
        let semicolon = self.tree.previous_code_token(first)?;
        if self.tree.blocks.kind(body) != Some(BlockKind::Aggregate)
            || !matches!(tokens[semicolon], Token::Symbol(';'))
            || groups.enclosing(semicolon) != Some(body)
        {
            return None;
        }
        let after_directive = tokens[semicolon..first].iter().any(|token| {
            matches!(token, Token::Preprocessor(directive)
            if preprocessor_directive(&directive.text).is_some_and(|name| {
                matches!(name, "if" | "ifdef" | "ifndef" | "elif" | "else" | "endif")
            }))
        });
        let after_comment = tokens[semicolon..first]
            .iter()
            .any(|token| matches!(token, Token::Comment(..)));
        // After a comment, only a nested aggregate member leads.
        let closes_aggregate = |index: usize| {
            groups
                .closed_at(index)
                .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Aggregate))
        };
        let last = self.tree.previous_code_token(semicolon)?;
        let after_nested_aggregate = closes_aggregate(last)
            || self
                .tree
                .previous_code_token(last)
                .is_some_and(closes_aggregate);
        if !(after_directive || after_comment && after_nested_aggregate) {
            return None;
        }
        let open = groups.get(body).open;
        let mut member = semicolon;
        while let Some(before) = self.tree.previous_code_token(member) {
            if before == open
                || matches!(tokens[before], Token::Symbol(';'))
                    && groups.enclosing(before) == Some(body)
            {
                break;
            }
            member = groups
                .closed_at(before)
                .map_or(before, |group| groups.get(group).open);
        }
        let line = self.output.line_with_token(member)?;
        if self.output.line_tokens(line)?.first != member {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// The first row of an initializer whose `{` ends a line stands a level
    /// past that line.
    fn initializer_first_row_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if self.tree.blocks.kind(group) != Some(BlockKind::Initializer)
            || self.tree.previous_code_token(first) != Some(open)
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        let span = self.output.line_tokens(line)?;
        // A brace after another on its line nests its rows past both.
        if span.last != open
            || self.tree.previous_code_token(open).is_some_and(|before| {
                matches!(self.tree.tokens[before], Token::Symbol('{'))
                    && self.output.line_with_token(before) == Some(line)
            })
        {
            return None;
        }
        // Rows stand at an indented brace starting its line, and at the
        // indented closing brace of a struct body before them.
        let indented_brace = match &self.tree.tokens[span.first] {
            Token::Symbol('{') => self.should_indent_brace_line(BraceType::Initializer),
            Token::Symbol('}') => matches!(
                self.options.brace_style,
                BraceStyle::Ratliff | BraceStyle::Whitesmith
            ),
            _ => false,
        };
        let level = if indented_brace {
            0
        } else {
            self.options.indent_width
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + level
                + self.case_unindent_spaces(),
        )
    }

    /// The `};` of a case block starting its line stands at the block's `{`
    /// when that starts a line.
    #[inline(never)]
    fn case_block_closing_brace_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        if !matches!(self.tree.tokens[first], Token::Symbol('}'))
            || !next_code_token(&self.tree.tokens, first + 1)
                .is_some_and(|next| matches!(self.tree.tokens[next], Token::Symbol(';')))
        {
            return None;
        }
        let block = groups.closed_at(first)?;
        let open = groups.get(block).open;
        // A block holding labels of its own closes as astyle's labels leave it.
        if self.tree.blocks.kind(block) != Some(BlockKind::Block)
            || !self.in_switch_body(open)
            || (open..first).any(|index| {
                groups.enclosing(index) == Some(block)
                    && matches!(&self.tree.tokens[index], Token::Word(word) if word == "case" || word == "default")
            })
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        (self.output.line_tokens(line)?.first == open).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces()
        })
    }

    /// Allman puts a control block's `{` on its own line at its header's
    /// line, wherever the tree anchored that line.
    pub(crate) fn align_allman_control_brace_to_header(&self, line: String) -> String {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        // Case bodies move their lines after publishing.
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman | BraceStyle::Pico | BraceStyle::Horstmann
        ) || self.options.indent_blocks
            || self.options.indent_braces
            || !(line.trimmed() == "{"
                || self.options.brace_style != BraceStyle::Allman
                    && line.trimmed_start().starts_with('{'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0
        {
            return line;
        }
        let Some(open) = self.output.pending_tokens().map(|span| span.first) else {
            return line;
        };
        if !groups
            .opened_at(open)
            .is_some_and(|block| self.tree.blocks.kind(block) == Some(BlockKind::Control))
        {
            return line;
        }
        let Some(mut header) = self.tree.previous_code_token(open) else {
            return line;
        };
        if let Some(condition) = groups.closed_at(header)
            && let Some(keyword) = self.tree.previous_code_token(groups.get(condition).open)
        {
            header = keyword;
        }
        if !matches!(&tokens[header], Token::Word(word)
            if matches!(word.as_str(), "if" | "for" | "while" | "switch"))
        {
            return line;
        }
        let Some(header_line) = self.output.line_with_token(header) else {
            return line;
        };
        if self
            .output
            .line_tokens(header_line)
            .is_none_or(|span| span.first != header)
        {
            return line;
        }
        let spaces = self.output.lead_width(header_line, self.options.tab_width);
        let current = leading_visual_width(&line, self.options.tab_width);
        // Run-in styles only lift a brace left behind its header.
        if current == spaces || self.options.brace_style != BraceStyle::Allman && current > spaces {
            return line;
        }
        format!("{}{}", " ".repeat(spaces), line.trimmed_start())
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
    #[inline(never)]
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
            || self.tree.has_directive_in(previous..first)
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

    /// A declarator after the `,` that follows an initialized declarator
    /// starting its line stands at that declarator.
    #[inline(never)]
    fn declarator_after_initializer_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        if !matches!(tokens[first], Token::Word(_)) {
            return None;
        }
        let comma = self.tree.previous_code_token(first)?;
        let close = self.tree.previous_code_token(comma)?;
        let initializer = groups.closed_at(close)?;
        if !matches!(tokens[comma], Token::Symbol(','))
            || groups.enclosing(comma) != groups.enclosing(first)
            || self.tree.blocks.kind(initializer) != Some(BlockKind::Initializer)
        {
            return None;
        }
        let assign = self
            .tree
            .previous_code_token(groups.get(initializer).open)?;
        let name = self.tree.previous_code_token(assign)?;
        if !matches!(&tokens[assign], Token::Operator(operator) if operator == "=")
            || !matches!(tokens[name], Token::Word(_))
        {
            return None;
        }
        let line = self.line_led_by(name)?;
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Styles that indent braces put the `{` of a later declarator's
    /// initializer, alone on its line, a level past the declarator.
    #[inline(never)]
    fn later_declarator_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !(self.options.brace_style == BraceStyle::Whitesmith
            || self.options.brace_style == BraceStyle::Vtk && self.in_code(first))
            || !matches!(tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let assign = self.tree.previous_code_token(first)?;
        let name = self.tree.previous_code_token(assign)?;
        let comma = self.tree.previous_code_token(name)?;
        if !matches!(&tokens[assign], Token::Operator(operator) if operator == "=")
            || !matches!(tokens[name], Token::Word(_))
            || !matches!(tokens[comma], Token::Symbol(','))
        {
            return None;
        }
        let line = self.line_led_by(name)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// A declarator after a `,` of a statement whose first line holds an
    /// `=` and ends at a `,` stands where astyle registers that `=`: at
    /// the word before it, or at the value after an array's `]`.
    #[inline(never)]
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
        // The look back depends on nothing but where it stands, so it ends
        // where the last one did once it reaches the `,` that one left.
        let address = tokens.as_ptr() as usize;
        let cached = self
            .declarator_start_cache
            .get()
            .filter(|&(cached_address, from, _)| cached_address == address && from <= comma);
        let mut start = comma;
        while let Some(mut before) = self.tree.previous_code_token(start) {
            if let Some((_, from, cached_start)) = cached
                && start == from
            {
                start = cached_start;
                break;
            }
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
        self.declarator_start_cache
            .set(Some((address, comma, start)));
        if matches!(&tokens[start], Token::Word(word) if word == "return" || is_header(word))
            || self.tree.has_directive_in(start..first)
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
            // A statement that registers nothing continues its lines one
            // level past its first.
            let registers = ForwardFind::first_in(
                &self.declarator_registers_cache,
                tokens,
                start,
                comma,
                |index| {
                    matches!(tokens[index], Token::Symbol('(' | '[' | '{' | '?'))
                        || matches!(&tokens[index], Token::Operator(operator)
                            if operator.ends_with('=') || operator == "?" || operator == "<<" || operator == ">>")
                },
            )
            .is_some();
            return (!registers
                && matches!(tokens[line_end], Token::Word(_))
                && matches!(&tokens[start], Token::Word(word) if !is_header(word) && word != "return"))
                .then(|| {
                    self.output.lead_width(start_line, self.options.tab_width)
                        + self.options.continuation_indent * self.options.indent_width
                        + self.case_unindent_spaces()
                });
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
        let key = TokenColumnKey {
            tokens: tokens.as_ptr() as usize,
            line,
            version: self.output.version(),
            text: (text.as_ptr() as usize, text.len()),
        };
        // A walk to a later token of the same line goes on from the last
        // one's token and column.
        let walked = self
            .token_column_cache
            .get()
            .filter(|walk| walk.key == key && (span.first..=token).contains(&walk.token));
        if let Some(walk) = walked
            && walk.token == token
        {
            return Some(walk.column);
        }
        let (mut position, from, (mut base, mut base_column)) = match walked {
            Some(walk) => (walk.end, walk.token + 1, (walk.start, walk.column)),
            None => (0, span.first, (0, 0)),
        };
        for (index, piece_token) in tokens.iter().enumerate().take(token + 1).skip(from) {
            if !is_code_token(piece_token) {
                continue;
            }
            let piece = token_text(piece_token);
            let rest = text.get(position..)?;
            // Tokens most often follow each other past blanks only; a code
            // token starts with no blank, so that is its first occurrence.
            let blanks = rest.len() - rest.trimmed_start().len();
            let offset = if !piece.is_empty() && rest[blanks..].starts_with(&*piece) {
                blanks
            } else {
                rest.find(&*piece)?
            };
            if index == token {
                let start = position + offset;
                base_column +=
                    visual_width_from(&text[base..start], base_column, self.options.tab_width);
                base = start;
                self.token_column_cache.set(Some(TokenColumnWalk {
                    key,
                    token,
                    start: base,
                    end: start + piece.len(),
                    column: base_column,
                }));
                return Some(base_column);
            }
            position += offset + piece.len();
        }
        None
    }

    /// A continuation line inside parentheses that comments separate from
    /// the code before it lays out as if they were not there: at the line
    /// continuing the same parentheses above, or at the parentheses'
    /// content, past a control condition's floor.
    #[inline(never)]
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
        // A row a file scope block indented stands apart from the group.
        let column = if groups.enclosing(previous_first) == Some(group)
            && !self.preprocessor.group_block_rows.contains(&previous_line)
        {
            self.output
                .lead_width(previous_line, self.options.tab_width)
        } else if self.options.indent_after_parens {
            return self.stacked_argument_indent(first);
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
    #[inline(never)]
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
        // A row a file scope block indented stands apart from the group.
        let column = if groups.enclosing(previous_first) == Some(group)
            && !self.preprocessor.group_block_rows.contains(&previous_line)
        {
            self.output
                .lead_width(previous_line, self.options.tab_width)
        } else if self.options.indent_after_parens {
            return self.stacked_argument_indent(first);
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
    #[inline(never)]
    fn logical_operand_in_parens_indent(&self, first: usize) -> Option<usize> {
        // Parens indenting after them align nothing.
        if self.options.indent_after_parens {
            return None;
        }
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
    #[inline(never)]
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
    /// A line leading with the `=` of a statement whose target fills the
    /// line before stands a continuation level past that line; an
    /// initializer's `= {` does not continue.
    /// A `=` leading its line before an initializer whose `{` ends a line
    /// continues nothing in astyle: it and a `{` alone after it stand at
    /// the statement.
    #[inline(never)]
    fn assignment_before_initializer_block_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let is_assignment =
            |index: usize| matches!(&tokens[index], Token::Operator(operator) if operator == "=");
        let (assign, open) = if is_assignment(first) {
            (first, next_code_token(tokens, first + 1)?)
        } else if matches!(tokens[first], Token::Symbol('{')) {
            (self.tree.previous_code_token(first)?, first)
        } else {
            return None;
        };
        if !is_assignment(assign)
            || !matches!(tokens[open], Token::Symbol('{'))
            || self
                .tree
                .groups
                .opened_at(open)
                .is_none_or(|group| self.tree.blocks.kind(group) != Some(BlockKind::Initializer))
            || !self
                .tree
                .groups
                .enclosing(assign)
                .is_none_or(|group| {
                    matches!(
                        self.tree.blocks.kind(group),
                        Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                    )
                })
            // The `{` ends its source line.
            || !next_code_token(tokens, open + 1).is_some_and(|next| {
                tokens[open + 1..next]
                    .iter()
                    .any(|token| matches!(token, Token::Newline))
            })
        {
            return None;
        }
        let statement_line = if first == assign {
            self.output
                .line_with_token(self.tree.previous_code_token(assign)?)?
        } else {
            let line = self.output.line_with_token(assign)?;
            if self.output.line_tokens(line)?.first != assign {
                return None;
            }
            self.output
                .line_with_token(self.tree.previous_code_token(assign)?)?
        };
        // Only a statement's declaration line leads it.
        let start = self.output.line_tokens(statement_line)?.first;
        if self
            .tree
            .previous_code_token(start)
            .is_some_and(|before| !matches!(tokens[before], Token::Symbol(';' | '{' | '}')))
        {
            return None;
        }
        Some(
            self.output
                .lead_width(statement_line, self.options.tab_width)
                + self.case_unindent_spaces(),
        )
    }

    /// Whether the line before is a one-line block that adding braces made.
    fn follows_added_one_line_block(&self) -> bool {
        (self.options.add_braces || self.options.add_one_line_braces)
            && self.output.last_non_empty().is_some_and(|line| {
                let code = self.output.code_of(line).trimmed();
                code.starts_with('{') && code.ends_with('}') && code.len() > 2
            })
    }

    /// Column of a directive opening a conditional within a block of an
    /// `else` when else-if chains break: that of the statement after it.
    pub(crate) fn break_else_if_directive_column(&self) -> Option<usize> {
        if !self.options.break_else_ifs {
            return None;
        }
        let tokens = &self.tree.tokens;
        let mut next = self.preprocessor.active_directive? + 1;
        let next = loop {
            let index = next_code_token(tokens, next)?;
            if !matches!(tokens[index], Token::Preprocessor(_)) {
                break index;
            }
            next = index + 1;
        };
        let group = self.tree.groups.enclosing(next)?;
        if !self.tree.statements.starts_block_statement(next)
            || !self
                .tree
                .groups
                .ancestors(group)
                .any(|id| self.block_of_else(id))
        {
            return None;
        }
        self.sibling_statement_column(next)
            .or_else(|| self.block_body_column(next))
    }

    #[inline(never)]
    fn leading_assignment_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        if !matches!(&tokens[first], Token::Operator(operator) if operator == "=")
            || next_code_token(tokens, first + 1)
                .is_some_and(|value| matches!(tokens[value], Token::Symbol('{')))
        {
            return None;
        }
        // A declaration at file scope stands like a statement.
        if let Some(group) = groups.enclosing(first)
            && !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        {
            return None;
        }
        let target = self.tree.previous_code_token(first)?;
        let line = self.output.line_with_token(target)?;
        let start = self.output.line_tokens(line)?.first;
        // A braceless body's statement starts its own statement too.
        if groups.enclosing(first).is_some()
            && !self.tree.statements.starts_block_statement(start)
            && self.tree.statements.braceless_header(start).is_none()
            || tokens[start..first]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_) | Token::Symbol(';')))
        {
            return None;
        }
        // astyle indents it one level whatever the continuation indent.
        Some(self.output.lead_width(line, self.options.tab_width) + self.options.indent_width)
    }

    /// A line leading with a binary operator in an assignment's value stands
    /// at the value when it follows the `=` on its line.
    #[inline(never)]
    fn leading_operator_assigned_value_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        // Indenting after parens indents the value, not aligns with it.
        if self.options.indent_after_parens
            || !matches!(&tokens[first], Token::Operator(operator)
                if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%" | "|" | "&" | "^" | "||" | "&&"))
        {
            return None;
        }
        let group = groups.enclosing(first)?;
        if !matches!(
            self.tree.blocks.kind(group),
            Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
        ) {
            return None;
        }
        // The look back depends on nothing but where it stands, so it ends
        // as the last one did once it reaches the token that one left from.
        let address = tokens.as_ptr() as usize;
        let cached = self
            .operand_assign_cache
            .get()
            .filter(|cached| (cached.0, cached.2) == (address, group) && cached.1 < first);
        let mut assign = None;
        let mut index = first;
        let open = groups.get(group).open;
        let found = loop {
            if let Some((_, from, _, found)) = cached
                && index == from
            {
                break found.map(|cached_assign| cached_assign.or(assign));
            }
            let Some(before) = self.tree.previous_code_token(index) else {
                break Some(assign);
            };
            // No token before the group's open brace is in the group.
            if before < open {
                break Some(assign);
            }
            if groups.enclosing(before) == Some(group) {
                match &tokens[before] {
                    Token::Operator(operator) if operator == "=" => assign = Some(before),
                    Token::Symbol(';' | '{' | '}') => break Some(assign),
                    Token::Symbol(':') if self.tree.statements.starts_block_statement(index) => {
                        break Some(assign);
                    }
                    Token::Symbol(',' | '?' | ':') | Token::Preprocessor(_) => break None,
                    Token::Word(word) if word == "return" || is_header(word) => break None,
                    _ => {}
                }
            }
            index = before;
        };
        self.operand_assign_cache
            .set(Some((address, first, group, found)));
        let assign = found??;
        let value = next_code_token(tokens, assign + 1)?;
        if self.output.line_with_token(assign)? != self.output.line_with_token(value)?
            || matches!(tokens[value], Token::Symbol('{'))
        {
            return None;
        }
        Some(self.token_column(value)? + self.case_unindent_spaces())
    }

    #[inline(never)]
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
        // The last `=` of a chained assignment holds the value.
        let mut assign = None;
        let mut index = previous;
        let open = groups.get(group).open;
        while let Some(before) = self.tree.previous_code_token(index) {
            if before < open {
                break;
            }
            if groups.enclosing(before) == Some(group) {
                match &tokens[before] {
                    Token::Operator(operator) if operator == "=" => {
                        assign = assign.or(Some(before));
                    }
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
    #[inline(never)]
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
        // The value's line is published: the literal continues it, a level
        // in when the statement runs in after a header.
        let value = next_code_token(tokens, assign + 1)?;
        Some(
            self.token_column(value)?
                + self.run_in_header_levels(assign) * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// How many control headers, as `if (x)`, precede on its line the
    /// statement holding the token `index`.
    pub(super) fn run_in_header_levels(&self, index: usize) -> usize {
        let tokens = &self.tree.tokens;
        let Some(line) = self.output.line_with_token(index) else {
            return 0;
        };
        let Some(mut cursor) = self
            .output
            .line_tokens(line)
            .and_then(|span| next_code_token(tokens, span.first))
        else {
            return 0;
        };
        let mut levels = 0;
        while cursor < index
            && matches!(&tokens[cursor], Token::Word(word) if matches!(word.as_str(), "if" | "while" | "for"))
            && let Some(open) = next_code_token(tokens, cursor + 1)
            && matches!(tokens[open], Token::Symbol('('))
            && let Some(close) = self
                .tree
                .groups
                .opened_at(open)
                .and_then(|group| self.tree.groups.get(group).close)
            && close < index
            && self.output.line_with_token(close) == Some(line)
            && let Some(next) = next_code_token(tokens, close + 1)
        {
            levels += 1;
            cursor = next;
        }
        levels
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
        let Some(group) = groups.enclosing(first) else {
            return false;
        };
        if groups.get(group).delimiter != Delimiter::Paren
            || groups.enclosing(previous) != Some(group)
        {
            return false;
        }
        // Standalone comments or blank lines: tokens on lines of their own.
        let mut between = tokens[previous + 1..first]
            .iter()
            .filter(|token| !matches!(token, Token::Whitespace(_)));
        let mut after_newline = false;
        let standalone_comment = between.any(|token| {
            let standalone =
                after_newline && matches!(token, Token::Comment(_, _) | Token::Newline);
            after_newline = matches!(token, Token::Newline);
            standalone
        });
        standalone_comment
            && (self.control_condition_of(first).is_some()
                || matches!(tokens[previous], Token::Symbol('?' | ':')))
    }

    /// A ternary arm starting a line after a `:` inside parentheses takes
    /// the column of the parentheses' content.
    #[inline(never)]
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
        // Parens indenting after them stack their own indent instead.
        if self.options.indent_after_parens {
            return self.stacked_argument_indent(first);
        }
        let content = next_code_token(tokens, open + 1)?;
        let column = self.token_column(content)?;
        // Parens past the continuation limit leave the arm to the stack.
        if column
            > self
                .output
                .lead_width(previous_line, self.options.tab_width)
                + self.options.max_continuation_indent
        {
            return None;
        }
        Some(column + self.case_unindent_spaces())
    }

    /// The arm after a `:` ending its line stands at the first arm when that
    /// arm starts its own line after the `?`.
    #[inline(never)]
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
        let question = groups
            .members_before(group, colon)
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
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
    /// Under a code length limit, where the stack replay stays off, a row
    /// led by a binary operator continuing a returned value stands at the
    /// value.
    #[inline(never)]
    fn returned_operand_row_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        self.options.max_code_length?;
        if !matches!(&tokens[first], Token::Operator(operator)
                if matches!(operator.as_str(), "+" | "-" | "*" | "/" | "%" | "|" | "&" | "^" | "||" | "&&" | "<<" | ">>"))
        {
            return None;
        }
        let keyword = self.operand_row_return(first)?;
        let value = next_code_token(tokens, keyword + 1)?;
        if self.output.line_with_token(value)? != self.output.line_with_token(keyword)? {
            return None;
        }
        Some(self.token_column(value)? + self.case_unindent_spaces())
    }

    /// The `return` the operand row at `first` continues the value of.
    fn operand_row_return(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        // The look back depends on nothing but where it stands, so it ends
        // where the last one did once it reaches the token that one left.
        let address = tokens.as_ptr() as usize;
        let cached = self
            .operand_return_cache
            .get()
            .filter(|&(cached_address, from, _)| cached_address == address && from <= first);
        let group = groups.enclosing(first);
        let mut index = first;
        let found = loop {
            if let Some((_, from, found)) = cached
                && index == from
            {
                break found;
            }
            let Some(before) = self.tree.previous_code_token(index) else {
                break None;
            };
            if let Some(closed) = groups.closed_at(before) {
                index = groups.get(closed).open;
                continue;
            }
            if groups.enclosing(before) != group {
                break None;
            }
            match &tokens[before] {
                Token::Word(word) if word == "return" => break Some(before),
                Token::Symbol(';' | '{' | '}' | ',' | '?' | ':') => break None,
                Token::Operator(operator)
                    if operator.ends_with('=')
                        && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">=") =>
                {
                    break None;
                }
                _ => index = before,
            }
        };
        self.operand_return_cache.set(Some((address, first, found)));
        found
    }

    fn return_value_indent(&self, first: usize) -> Option<usize> {
        let keyword = self.tree.previous_code_token(first)?;
        if !matches!(&self.tree.tokens[keyword], Token::Word(word) if word == "return") {
            return None;
        }
        let line = self.line_led_by(keyword)?;
        (self.output.line_tokens(line)?.last == keyword).then(|| {
            self.output.lead_width(line, self.options.tab_width)
                + self.options.continuation_indent * self.options.indent_width
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
        let earlier = self.case_labels_between(open, first).find(|&index| {
            groups.enclosing(index) == Some(body)
                && self.tree.statements.starts_block_statement(index)
        });
        let Some(earlier) = earlier else {
            return self.first_case_label_column(body);
        };
        let line = self.output.line_with_token(earlier)?;
        // A row a label was split off may span past its own label.
        let leads_with_label = {
            let code = self.output.code_trimmed(line);
            code.starts_with("case ") || code.starts_with("default")
        };
        (self.output.line_tokens(line)?.first == earlier || leads_with_label).then(|| {
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
        // A Whitesmith switch nested in a case block owns its body too.
        let owner = self
            .tree
            .previous_code_token(open)
            .filter(|_| self.options.brace_style == BraceStyle::Whitesmith)
            .and_then(|close| self.tree.groups.closed_at(close))
            .and_then(|condition| {
                self.tree
                    .previous_code_token(self.tree.groups.get(condition).open)
            })
            .filter(|&keyword| matches!(&tokens[keyword], Token::Word(word) if word == "switch"))
            .or_else(|| self.tree.blocks.owner(body))?;
        if !matches!(&tokens[owner], Token::Word(word) if word == "switch") {
            return None;
        }
        let brace_line = self.output.line_with_token(open)?;
        // Indented switch braces take their labels after layout, except a
        // brace on its own line, whose column the labels share.
        let brace_leads = self.output.line_tokens(brace_line)?.first == open;
        // Whitesmith labels share the column of the indented brace.
        let whitesmith = self.options.brace_style == BraceStyle::Whitesmith && brace_leads;
        if self.should_indent_brace_line(BraceType::Command)
            && !whitesmith
            && !(self.options.brace_style == BraceStyle::Vtk
                && !self.options.indent_switches
                && self.layout.line_adjuster.pending_case_unindent() == 0
                && brace_leads)
        {
            return None;
        }
        let line = if brace_leads {
            brace_line
        } else {
            self.line_led_by(owner)?
        };
        let switch_level = usize::from(self.options.indent_switches && !whitesmith);
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + (switch_level + self.layout.line_adjuster.pending_case_unindent())
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
        // Only comment lines above the line move: past blank lines and
        // directives, a line of code ends the walk below.
        if (0..line)
            .rev()
            .find(|&index| {
                let text = self.output.trimmed(index);
                !text.is_empty() && !text.starts_with('#')
            })
            .is_none_or(|index| self.output.line_tokens(index).is_some())
        {
            return;
        }
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
        let compound_literal_first_element = groups.enclosing(first).is_some_and(|group| {
            self.tree.blocks.kind(group) == Some(BlockKind::CompoundLiteral)
                && self.tree.previous_code_token(first) == Some(groups.get(group).open)
        });
        let is_else = matches!(&tokens[first], Token::Word(word) if word == "else");
        // An arm after a ternary `?` or `:` ending the line before.
        let ternary_arm = self
            .tree
            .previous_code_token(first)
            .is_some_and(|previous| {
                matches!(tokens[previous], Token::Symbol('?' | ':'))
                    && groups
                        .members_before(groups.enclosing(previous), previous)
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
        // A declarator continuing a struct member after a comma.
        let member_continuation = groups.enclosing(first).is_some_and(|group| {
            self.tree.blocks.kind(group) == Some(BlockKind::Aggregate)
                && self
                    .tree
                    .previous_code_token(first)
                    .is_some_and(|previous| {
                        matches!(tokens[previous], Token::Symbol(','))
                            && groups.enclosing(previous) == Some(group)
                    })
        });
        // Comments that end a block stand at its body column; comments
        // before an `else` whose `if` is the body of an `else` that a
        // directive splits stand at that outer `else`.
        let split_chain_column = is_else
            .then(|| self.split_chain_else_column(first))
            .flatten();
        let label_column = if case_labels {
            self.case_body_comment_column(first)
        } else {
            self.label_comment_column(first)
        };
        // VTK indents a file-scope brace row past the rows; comments before
        // it stay at the rows.
        let vtk_brace_row_column = (self.options.brace_style == BraceStyle::Vtk
            && initializer_element
            && matches!(tokens[first], Token::Symbol('{'))
            && !self.in_code(first))
        .then(|| {
            let open = groups.get(groups.enclosing(first)?).open;
            let open_line = self.output.line_with_token(open)?;
            let rows = self.output.lead_width(open_line, self.options.tab_width)
                + self.options.indent_width;
            (self.output.lead_width(line, self.options.tab_width) > rows).then_some(rows)
        })
        .flatten();
        let block_body_column = self
            .block_comment_column(first)
            .or(split_chain_column)
            .or(label_column)
            .or(vtk_brace_row_column);
        if !statements.starts_block_statement(first)
            && block_body_column.is_none()
            && !in_parens
            && !member_continuation
            && !initializer_element
            && !compound_literal_first_element
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
            Token::Symbol('}') if block_body_column.is_some() => {}
            Token::Symbol('{' | '}') => return,
            _ if !is_case_label
                && !member_continuation
                && label_column.is_none()
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
        let prefix = match block_body_column {
            Some(spaces) => self
                .options
                .continuation_indent_prefix(spaces / self.options.indent_width.max(1), spaces),
            None => code[..code.len() - code.trimmed_start().len()].to_string(),
        };
        let tab_width = self.options.tab_width.max(1);
        let mut end = line;
        while end > 0 {
            let index = end - 1;
            let text = self.output.trimmed(index);
            if text.is_empty() {
                end = index;
                continue;
            }
            // Directives change no indent: branches between the comments
            // and the code hold directives only. A label or an `else` may
            // move off the level of the code before.
            if (!is_case_label || label_column.is_some())
                && (!is_else || split_chain_column.is_some())
                && text.starts_with('#')
                && !text.ends_with('\\')
                && (index == 0 || !self.output.trimmed(index - 1).ends_with('\\'))
            {
                end = index;
                continue;
            }
            // Code after a comment on its line holds it there.
            if self.output.line_tokens(index).is_some()
                || crate::formatter::tokens::comments::text_follows_comment_close(text)
            {
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
            // A comment that trailed code in the source keeps its place
            // before a label.
            if is_case_label && self.output_comment_trailed_code(start) {
                break;
            }
            let lead = self.output.lead_width(start, tab_width);
            let opener_line = &self.output.as_slice()[start];
            let opener_prefix =
                &opener_line[..opener_line.len() - opener_line.trimmed_start().len()];
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
                    if text.is_empty() {
                        line.clear();
                        continue;
                    }
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
            && line.trimmed_start().strip_prefix(word.as_str()).is_some_and(|rest| {
                rest.trimmed_start().starts_with(':')
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
    /// A brace added to close a control statement's body stands where the
    /// line holding the statement's header and its `{` starts.
    fn added_closing_brace_indent(&self, line: &LineView<'_>) -> Option<usize> {
        if line.trimmed() != "}" || self.should_indent_brace_line(BraceType::Command) {
            return None;
        }
        let header = self.layout.nesting.last_closed_brace_header.as_deref()?;
        if !matches!(header, "if" | "else" | "for" | "while" | "do") {
            return None;
        }
        let (spaces, _, open) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        let open = open.strip_prefix('}').map_or(open, str::trim_start);
        let open = open
            .strip_prefix("else")
            .filter(|rest| header == "if" && rest.starts_with([' ', '\t']))
            .map_or(open, str::trim_start);
        (starts_header_word(open, header) && header_line_opens_its_block(&open[header.len()..]))
            .then(|| spaces + self.case_unindent_spaces())
    }

    pub(crate) fn case_unindent_spaces(&self) -> usize {
        self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
    }

    /// An `else` starting a line, alone or after the `}` closing the `if`
    /// body, takes the indent of the line holding its `if`.
    #[inline(never)]
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
    #[inline(never)]
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
        // GNU indents the block's braces a level past the `else`, Ratliff
        // its closing brace.
        let braces = if self.options.brace_style == BraceStyle::Gnu
            || self.options.brace_style == BraceStyle::Ratliff
                && matches!(tokens[first], Token::Symbol('}'))
        {
            self.options.indent_width
        } else {
            0
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + extra
                + braces
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
                if let Some(line) = self.line_led_by(keyword) {
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
                    // A header after braceless headers on its line, as in
                    // `} else for (x) if (y) {`, nests a level past each.
                    let (line, levels) = self.header_chain_before(open)?;
                    extra += (levels - 1) * self.options.indent_width;
                    line
                }
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
        self.tree
            .groups
            .closed_at(first)
            .is_some_and(|group| self.group_in_braced_chain_of_braceless_body(group))
    }

    /// Whether `group` is the block of an `else`, or of the `if` of an
    /// `else if`, which the block belongs to.
    fn block_of_else(&self, group: GroupId) -> bool {
        self.tree.blocks.kind(group) == Some(BlockKind::Control)
            && self.tree.blocks.owner(group).is_some_and(
                |head| matches!(&self.tree.tokens[head], Token::Word(word) if word == "else"),
            )
    }

    fn group_in_braced_chain_of_braceless_body(&self, group: GroupId) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let statements = &self.tree.statements;
        let is_word =
            |index: usize, text: &str| matches!(&tokens[index], Token::Word(word) if word == text);
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
    /// A one-line control block on its own line: astyle indents it a level
    /// in Ratliff and keeps it at the header in VTK, against how those
    /// styles place the braces of longer blocks.
    #[inline(never)]
    fn one_line_control_block_indent(&self, first: usize, line: &str) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol('{')) {
            return None;
        }
        let group = self.tree.groups.opened_at(first)?;
        let extra = match self.options.brace_style {
            BraceStyle::Ratliff => self.options.indent_width,
            // VTK indents the brace within a block other than a function's.
            BraceStyle::Vtk
                if self.tree.groups.ancestors(group).skip(1).any(|id| {
                    matches!(
                        self.tree.blocks.kind(id),
                        Some(BlockKind::Control | BlockKind::Block)
                    )
                }) =>
            {
                self.options.indent_width
            }
            BraceStyle::Vtk => 0,
            _ => return None,
        };
        let close = self.tree.groups.get(group).close?;
        let code = self.output.code_trimmed_of(line);
        if self.tree.blocks.kind(group) != Some(BlockKind::Control)
            || !code.ends_with('}')
            || self.tree.tokens[first..close]
                .iter()
                .any(|token| matches!(token, Token::Newline))
        {
            return None;
        }
        let (line, levels) = self.header_chain_before(first)?;
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + (levels - 1) * self.options.indent_width
                + extra
                + self.case_unindent_spaces(),
        )
    }

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
                && !self.header_keyword_before(first).is_some_and(|keyword| {
                    // A header nested in the body stands where layout put it.
                    self.tree.statements.in_else_body_after_blank_line(keyword)
                        && !self
                            .tree
                            .statements
                            .starts_else_body_after_blank_line(keyword)
                })
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
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Gnu
        ) || !matches!(self.tree.tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let group = self.tree.groups.opened_at(first)?;
        // A block after a group whose first branch ends with a control
        // header is that header's body to astyle.
        if self.options.brace_style == BraceStyle::Gnu
            && let Some(header) = self.tree.statements.branch_header_of_block(first)
        {
            let line = self.line_led_by(header)?;
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + self.options.indent_width
                    + self.case_unindent_spaces(),
            );
        }
        if self.tree.blocks.kind(group) != Some(BlockKind::Control)
            || (self.options.brace_style != BraceStyle::Gnu
                && !self.should_indent_brace_line(BraceType::Command))
        {
            return None;
        }
        // GNU braces nested deeper in such a body stand at their headers.
        if self.tree.statements.in_else_body_after_blank_line(first)
            && self.options.brace_style == BraceStyle::Gnu
            && self.header_keyword_before(first).is_none_or(|keyword| {
                self.tree
                    .statements
                    .starts_else_body_after_blank_line(keyword)
            })
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
    #[inline(never)]
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
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
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

    /// VTK indents the braces and members of a struct, union, or enum nested
    /// at any depth in a union one level past its keyword.
    #[inline(never)]
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
        let in_union = groups
            .ancestors(groups.enclosing(open)?)
            .take_while(|&outer| self.tree.blocks.kind(outer) == Some(BlockKind::Aggregate))
            .any(|outer| {
                let outer_open = groups.get(outer).open;
                self.tree.blocks.owner(outer).is_some_and(|start| {
                    tokens[start..outer_open]
                        .iter()
                        .any(|token| matches!(token, Token::Word(word) if word == "union"))
                })
            });
        if self.tree.blocks.kind(group) != Some(BlockKind::Aggregate)
            || !is_aggregate_keyword(keyword)
            || !in_union
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
    #[inline(never)]
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
    #[inline(never)]
    fn statement_after_split_else_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if matches!(tokens[first], Token::Symbol('{' | '}'))
            || self.layout.line_adjuster.total_case_unindent_depth() > 0
        {
            return None;
        }
        let sibling = self.tree.statements.previous_sibling(first)?;
        // The directive after such an `else` comes at the latest at `first`.
        if !self.tree.has_directive_in(sibling..first + 1)
            && !matches!(tokens[first], Token::Whitespace(_) | Token::Newline)
        {
            return None;
        }
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
    #[inline(never)]
    fn split_else_block_statement_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        // The `{` of an `else` stands at it, a level in where the style
        // indents block braces.
        if matches!(tokens[first], Token::Symbol('{'))
            && let Some(keyword) = self.tree.previous_code_token(first)
            && matches!(&tokens[keyword], Token::Word(word) if word == "else")
        {
            let offset = if matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Gnu | BraceStyle::Ratliff
            ) {
                self.options.indent_width
            } else {
                0
            };
            // A dangling `else` stands by the line of its `if`.
            let owner = if self.is_dangling_else(keyword) {
                self.tree.statements.if_of_else(keyword)?
            } else {
                keyword
            };
            let line = self.line_led_by(owner)?;
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + offset
                    + self.case_unindent_spaces(),
            );
        }
        // So does the `{` of a header the split `else` holds.
        if matches!(tokens[first], Token::Symbol('{'))
            && let Some(close) = self.tree.previous_code_token(first)
            && let Some(condition) = self.tree.groups.closed_at(close)
            && let Some(header) = self
                .tree
                .previous_code_token(self.tree.groups.get(condition).open)
            && matches!(&tokens[header], Token::Word(word) if matches!(word.as_str(), "if" | "while" | "for"))
            && let Some(keyword) = self.tree.previous_code_token(header)
            && matches!(&tokens[keyword], Token::Word(word) if word == "else")
            && self.tree.has_directive_in(keyword..header)
        {
            let offset = if matches!(
                self.options.brace_style,
                BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Gnu | BraceStyle::Ratliff
            ) {
                self.options.indent_width
            } else {
                0
            };
            let line = self.line_led_by(header)?;
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + offset
                    + self.case_unindent_spaces(),
            );
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
            || !self.tree.has_directive_in(keyword..open)
        {
            return None;
        }
        self.enclosing_block_body_column(first)
    }

    /// The `while` of a `do` block starting its line stands at the `do`.
    #[inline(never)]
    fn do_while_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(&tokens[first], Token::Word(word) if word == "while") {
            return None;
        }
        let close = self.tree.previous_code_token(first)?;
        if matches!(tokens[close], Token::Symbol(';')) {
            return self.braceless_do_while_indent(close);
        }
        let block = self.tree.groups.closed_at(close)?;
        // The `do` right before the block owns it, even as a braceless body.
        let owner = self
            .tree
            .previous_code_token(self.tree.groups.get(block).open)
            .filter(|&index| matches!(&tokens[index], Token::Word(word) if word == "do"))
            .or_else(|| self.tree.blocks.owner(block))?;
        if !matches!(&tokens[owner], Token::Word(word) if word == "do")
            || self.tree.has_directive_in(close..first)
        {
            return None;
        }
        let Some(line) = self.line_led_by(owner) else {
            // A `do` run in as the braceless body of a header on its line
            // closes at that body's level.
            let header = self.tree.statements.braceless_header(owner)?;
            let line = self
                .line_led_by(header)
                .filter(|&line| self.output.line_with_token(owner) == Some(line))?;
            return Some(
                self.output.lead_width(line, self.options.tab_width)
                    + self.options.indent_width
                    + self.case_unindent_spaces(),
            );
        };
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// The `while` after a braceless `do` body ending at `semicolon` stands
    /// at the `do`.
    fn braceless_do_while_indent(&self, semicolon: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let group = self.tree.groups.enclosing(semicolon);
        let do_token = self
            .tree
            .groups
            .members_before(group, semicolon)
            .take_while(|&index| !matches!(tokens[index], Token::Symbol(';' | '{' | '}')))
            .find_map(|index| self.tree.statements.braceless_header(index))
            .filter(|&header| matches!(&tokens[header], Token::Word(word) if word == "do"))?;
        if self.tree.has_directive_in(do_token..semicolon) {
            return None;
        }
        // Only a `do` leading its line, or run in after a `{`, places it;
        // one run in after an `else` stands at the else's body.
        let line = self.output.line_with_token(do_token)?;
        let first = self.output.line_tokens(line)?.first;
        if self
            .tree
            .previous_code_token(do_token)
            .is_some_and(|previous| {
                matches!(&tokens[previous], Token::Word(word) if word == "else")
                    && self.output.line_with_token(previous) == Some(line)
            })
        {
            return Some(
                self.output
                    .lead_width(self.line_led_by(do_token)?, self.options.tab_width)
                    + self.options.indent_width
                    + self.case_unindent_spaces(),
            );
        }
        if first != do_token
            && !(matches!(tokens[first], Token::Symbol('{'))
                && next_code_token(tokens, first + 1) == Some(do_token))
        {
            return None;
        }
        Some(self.token_column(do_token)? + self.case_unindent_spaces())
    }

    /// VTK keeps the braces of a file-scope function with K&R parameter
    /// declarations in column one, as for any function.
    #[inline(never)]
    fn vtk_knr_function_brace_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if self.options.brace_style != BraceStyle::Vtk {
            return None;
        }
        let body = match tokens[first] {
            Token::Symbol('{') => groups.opened_at(first)?,
            Token::Symbol('}') => groups.closed_at(first)?,
            _ => return None,
        };
        let open = groups.get(body).open;
        (self.tree.blocks.kind(body) == Some(BlockKind::FunctionBody)
            && groups.get(body).parent.is_none()
            && self
                .tree
                .previous_code_token(open)
                .is_some_and(|before| matches!(tokens[before], Token::Symbol(';'))))
        .then_some(0)
    }

    /// Whitesmith indents the `{` of a function body starting its line one
    /// level past the first line of the function's head.
    #[inline(never)]
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
        if start == first {
            return None;
        }
        // Headers split by directives start in column one at file scope.
        if self.tree.has_directive_in(start..first) {
            return Some(self.options.indent_width);
        }
        let line = self.output.line_with_token(start)?;
        (self.output.line_tokens(line)?.first == start).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.options.indent_width
        })
    }

    /// Styles that indent braces indent a one-line block kept after a
    /// line holding only a macro name one level past the macro, unless the
    /// block is empty.
    #[inline(never)]
    fn one_line_block_after_macro_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
        ) || !matches!(tokens[first], Token::Symbol('{'))
        {
            return None;
        }
        let block = self.tree.groups.opened_at(first)?;
        let close = self.tree.groups.get(block).close?;
        if tokens[first..close]
            .iter()
            .any(|token| matches!(token, Token::Newline))
            || !tokens[first + 1..close].iter().any(is_code_token)
        {
            return None;
        }
        let name = self.tree.previous_code_token(first)?;
        if !matches!(&tokens[name], Token::Word(word) if is_macro_like_word(word)) {
            return None;
        }
        let line = self.line_led_by(name)?;
        (self.output.line_tokens(line)?.last == name).then(|| {
            self.output.lead_width(line, self.options.tab_width) + self.options.indent_width
        })
    }

    /// Whitesmith indents the `{` of a statement-like macro's block one
    /// level past the macro, as it does for control headers.
    #[inline(never)]
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
            || self.tree.has_directive_in(owner..first)
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
    #[inline(never)]
    fn statement_expression_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let (group, extra) = if matches!(tokens[first], Token::Symbol('}')) {
            // Ratliff closes at the statements.
            let extra = usize::from(self.options.brace_style == BraceStyle::Ratliff)
                * self.options.indent_width;
            (groups.closed_at(first)?, extra)
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
    #[inline(never)]
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
    /// Whether the statement starting at `first` ends on a later source
    /// line than it starts.
    fn statement_spans_lines(&self, first: usize) -> bool {
        let tokens = &self.tree.tokens;
        let group = self.tree.groups.enclosing(first);
        tokens[first..]
            .iter()
            .enumerate()
            .take_while(|&(offset, token)| {
                !(matches!(token, Token::Symbol(';' | '{' | '}'))
                    && self.tree.groups.enclosing(first + offset) == group)
            })
            .any(|(_, token)| matches!(token, Token::Newline))
    }

    fn braceless_body_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let header = self.tree.statements.braceless_header(first)?;
        // Added braces make the body a block; an empty statement, a
        // nested header, a statement over several lines or one after a
        // directive gets none.
        if (self.options.add_braces || self.options.add_one_line_braces)
            && !matches!(tokens[first], Token::Symbol(';'))
            && !matches!(&tokens[first], Token::Word(word) if is_header(word))
            && !self.statement_spans_lines(first)
            && !self.tree.has_directive_in(header..first)
        {
            return None;
        }
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
        // A dangling `else` stands by the line of its `if`.
        let line = if matches!(&tokens[header], Token::Word(word) if word == "else")
            && self.is_dangling_else(header)
            && let Some(if_token) = self.tree.statements.if_of_else(header)
        {
            self.line_led_by(if_token)?
        } else {
            self.line_led_by(header)?
        };
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
    /// indent. `previous` is the code token before `first`.
    #[inline(never)]
    fn parameter_line_indent(&self, first: usize, previous: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        if self.options.indent_after_parens
            || !self.tree.has_directive_in(previous + 1..first)
            || !self.tree.functions.is_parameter_list(group)
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
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
    #[inline(never)]
    fn assigned_value_in_case_block_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let block = groups.enclosing(first)?;
        // A `;` on its own line ends the value and stands with it.
        let assign = if matches!(tokens[first], Token::Symbol(';')) {
            let mut index = first;
            loop {
                index = self.tree.previous_code_token(index)?;
                if groups.enclosing(index) != Some(block) {
                    index = groups.get(groups.closed_at(index)?).open;
                    continue;
                }
                match &tokens[index] {
                    Token::Operator(operator) if operator == "=" => break index,
                    Token::Symbol(';' | '{' | '}') => return None,
                    _ => {}
                }
            }
        } else {
            self.tree.previous_code_token(first)?
        };
        // Only a value split off its `=` takes the level.
        if matches!(tokens[first], Token::Symbol(';'))
            && next_code_token(tokens, assign + 1)
                .and_then(|next| self.output.line_with_token(next))
                == self.output.line_with_token(assign)
        {
            return None;
        }
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
                + self.options.continuation_indent * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The `}` of a block whose `{` starts its line stands at the `{`.
    #[inline(never)]
    fn block_closing_brace_indent(&self, first: usize) -> Option<usize> {
        if !matches!(self.tree.tokens[first], Token::Symbol('}')) {
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
        // Inside a case body only blocks within the case close here.
        if (self.layout.line_adjuster.total_case_unindent_depth() > 0
            || self.layout.line_adjuster.next_line_case_unindent_depth() > 0)
            && self.in_switch_body(open)
        {
            return None;
        }
        let tokens = &self.tree.tokens;
        // A `{` right after a directive stands where the branches leave it.
        if self
            .tree
            .previous_code_token(open)
            .is_some_and(|header| self.tree.has_directive_in(header + 1..open))
        {
            return None;
        }
        // Branches with unbalanced braces and labels inside leave the
        // pairing of the braces to the engine.
        if self
            .case_labels_between(open, first)
            .any(|word| self.tree.groups.enclosing(word) == Some(block))
        {
            return None;
        }
        let mut branches: Vec<isize> = Vec::new();
        let directives = if self.tree.has_directive_in(open + 1..first) {
            &tokens[open + 1..first]
        } else {
            &[]
        };
        for token in directives {
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
                _ => {}
            }
        }
        if !branches.is_empty() {
            return None;
        }
        let mut line = self.output.line_with_token(open)?;
        let mut line_first = self.output.line_tokens(line)?.first;
        if line_first != open {
            // An attached `{` closes at its header's line where the style
            // aligns closing braces with headers.
            let groups = &self.tree.groups;
            let mut header = self.tree.previous_code_token(open)?;
            if let Some(condition) = groups.closed_at(header) {
                header = self.tree.previous_code_token(groups.get(condition).open)?;
            }
            // A condition split over lines leaves the `{` on a later line;
            // its header may follow a `}` and an `else` on its own line.
            if let Some(header_line) = self.output.line_with_token(header)
                && header_line < line
                && let Some(header_line_first) =
                    self.output.line_tokens(header_line).map(|span| span.first)
                && tokens[header_line_first..header].iter().all(|token| {
                    matches!(token, Token::Symbol('}') | Token::Whitespace(_))
                        || matches!(token, Token::Word(word) if word == "else")
                })
            {
                line = header_line;
                line_first = header_line_first;
            }
            if !matches!(&tokens[header], Token::Word(word)
                if matches!(word.as_str(), "if" | "else" | "for" | "while" | "switch" | "do"))
            {
                return None;
            }
            let is_word = |index: usize, text: &str| matches!(&tokens[index], Token::Word(word) if word == text);
            let same_line = |a: usize, b: usize| {
                self.output.line_with_token(a).is_some()
                    && self.output.line_with_token(a) == self.output.line_with_token(b)
            };
            if is_word(header, "if")
                && let Some(before) = self.tree.previous_code_token(header)
                && is_word(before, "else")
                && same_line(before, header)
            {
                header = before;
            }
            if is_word(header, "else")
                && let Some(before) = self.tree.previous_code_token(header)
                && matches!(tokens[before], Token::Symbol('}'))
                && same_line(before, header)
            {
                header = before;
            }
            // A braceless body on its header's line closes a level past it:
            // `if (x) do {` ends at `do`'s level.
            let mut levels = 0;
            while header != line_first
                && let Some(outer) = self.tree.statements.braceless_header(header)
                && same_line(outer, header)
            {
                header = outer;
                levels += 1;
            }
            if levels > 0
                && header == line_first
                && matches!(
                    self.options.brace_style,
                    BraceStyle::None
                        | BraceStyle::Allman
                        | BraceStyle::Attach
                        | BraceStyle::OneTrueBrace
                        | BraceStyle::WebKit
                )
                && !self.options.indent_blocks
                && !self.options.indent_braces
            {
                return Some(
                    self.output.lead_width(line, self.options.tab_width)
                        + levels * self.options.indent_width
                        + self.case_unindent_spaces(),
                );
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
            return Some(
                self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces(),
            );
        }
        if !self.output.as_slice()[line]
            .trimmed_start()
            .starts_with('{')
        {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Styles that indent braces put the first statement of a case block
    /// whose `{` starts a line at the brace.
    #[inline(never)]
    fn broken_case_block_first_statement_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk
        ) || matches!(tokens[first], Token::Symbol('{' | '}'))
        {
            return None;
        }
        // Where the engine loses levels, in an `else` body split off by an
        // empty line, every statement of the block stands at its brace.
        let open = self.tree.statements.block_opening(first).or_else(|| {
            (self.tree.statements.starts_block_statement(first)
                && self.tree.statements.in_else_body_after_blank_line(first))
            .then(|| self.tree.groups.enclosing(first))
            .flatten()
            .map(|group| self.tree.groups.get(group).open)
        })?;
        let block = self.tree.groups.opened_at(open)?;
        let owner = self.tree.blocks.owner(block)?;
        if !matches!(&tokens[owner], Token::Word(word) if word == "case" || word == "default")
            || self
                .tree
                .previous_code_token(open)
                .is_none_or(|colon| !matches!(tokens[colon], Token::Symbol(':')))
        {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_tokens(line)?.first != open {
            return None;
        }
        Some(self.output.lead_width(line, self.options.tab_width) + self.case_unindent_spaces())
    }

    /// Statements of a case block whose brace is attached to its label
    /// stand a level past the label, or at the statement before them.
    #[inline(never)]
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
        // Labels sharing the line all lead to the block.
        let line_first = self.output.line_tokens(line)?.first;
        if !(line_first == label
            || matches!(&tokens[line_first], Token::Word(word) if matches!(word.as_str(), "case" | "default"))
                && !tokens[line_first..label]
                    .iter()
                    .any(|token| matches!(token, Token::Symbol(';' | '{' | '}'))))
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
    #[inline(never)]
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
        // Indented cases put what follows the block at the case body.
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + self.case_unindent_spaces()
                + usize::from(self.options.indent_cases) * self.options.indent_width,
        )
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
        // Statements kept on one line stand where the first of them does.
        let mut leader = sibling;
        while self.output.line_tokens(line)?.first != leader {
            leader = self.tree.statements.previous_sibling(leader)?;
            if self.output.line_with_token(leader) != Some(line) {
                return None;
            }
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
            .or_else(|| self.brace_line_body_column(first))
    }

    /// Comments that a directive separates from the case label at `first`
    /// stand a level past the label: astyle indents only a comment right
    /// before a label at the label.
    fn case_body_comment_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(&tokens[first], Token::Word(word) if word == "case" || word == "default")
            || !tokens[..first]
                .iter()
                .rev()
                .find(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
                .is_some_and(|token| matches!(token, Token::Preprocessor(_)))
        {
            return None;
        }
        Some(self.token_column(first)? + self.options.indent_width)
    }

    /// The column of the statements before the statement label at `first`
    /// in its block, which comments before the label keep.
    fn label_comment_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let statements = &self.tree.statements;
        if !matches!(&tokens[first], Token::Word(word) if !matches!(word.as_str(), "case" | "default"))
            || !next_code_token(tokens, first + 1)
                .is_some_and(|colon| matches!(tokens[colon], Token::Symbol(':')))
            || statements.braceless_header(first).is_some()
        {
            return None;
        }
        let block = groups.enclosing(first)?;
        let open = groups.get(block).open;
        let earlier = (open + 1..first).rev().find(|&index| {
            statements.starts_block_statement(index)
                && groups.enclosing(index) == Some(block)
                && statements.braceless_header(index).is_none()
                && !next_code_token(tokens, index + 1)
                    .is_some_and(|next| matches!(tokens[next], Token::Symbol(':')))
        })?;
        let line = self.line_led_by(earlier)?;
        Some(self.output.lead_width(line, self.options.tab_width))
    }

    /// The column of the outer `else` of the `else` at `first`, when that
    /// one's `if` forms the outer `else`'s body past a directive or a blank
    /// line.
    fn split_chain_else_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let if_token = self.tree.statements.if_of_else(first)?;
        let outer = self.tree.previous_code_token(if_token)?;
        if !matches!(&tokens[outer], Token::Word(word) if word == "else")
            || !self.tree.statements.in_split_else_body(if_token)
        {
            return None;
        }
        let line = self.line_led_by(outer)?;
        Some(self.output.lead_width(line, self.options.tab_width))
    }

    /// The body column of the block that the `}` at `first` closes: that of
    /// its first statement, or, when only comments stand in it, a level
    /// past its `{` starting a line.
    fn block_comment_column(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        if !matches!(tokens[first], Token::Symbol('}')) {
            return None;
        }
        let groups = &self.tree.groups;
        let block = groups.closed_at(first)?;
        let open = groups.get(block).open;
        if self.tree.previous_code_token(first) != Some(open) {
            if !matches!(
                self.tree.blocks.kind(block),
                Some(BlockKind::Control | BlockKind::Block | BlockKind::FunctionBody)
            ) || self.tree.blocks.owner(block).is_some_and(
                |owner| matches!(&tokens[owner], Token::Word(word) if word == "switch"),
            ) {
                return None;
            }
            let statement = next_code_token(tokens, open + 1)?;
            if !self.tree.statements.starts_block_statement(statement)
                || next_code_token(tokens, statement + 1)
                    .is_some_and(|next| matches!(tokens[next], Token::Symbol(':')))
            {
                return None;
            }
            let line = self.output.line_with_token(statement)?;
            if self.output.line_tokens(line)?.first == statement {
                return Some(self.output.lead_width(line, self.options.tab_width));
            }
            return self.token_column(statement);
        }
        if self.tree.blocks.kind(block) != Some(BlockKind::Control) {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_tokens(line)?.first != open {
            return None;
        }
        let offset = if self.should_indent_brace_line(BraceType::Command) {
            0
        } else {
            self.options.indent_width
        };
        Some(self.output.lead_width(line, self.options.tab_width) + offset)
    }

    /// The body column of the block holding `first` from its `{` when that
    /// starts a line.
    fn brace_line_body_column(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let block = groups.enclosing(first)?;
        let open = groups.get(block).open;
        if groups.get(block).delimiter != Delimiter::Brace {
            return None;
        }
        let line = self.output.line_with_token(open)?;
        if self.output.line_tokens(line)?.first != open {
            return None;
        }
        let offset = if self.should_indent_brace_line(BraceType::Command) {
            0
        } else {
            self.options.indent_width
        };
        Some(
            self.output.lead_width(line, self.options.tab_width)
                + offset
                + self.case_unindent_spaces(),
        )
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
        let column = match self.block_body_anchor(block)? {
            BodyAnchor::BraceLine(lead) => {
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
                lead + if indented_brace {
                    0
                } else {
                    self.options.indent_width
                }
            }
            BodyAnchor::Header(column) => column?,
        };
        Some(column + self.case_unindent_spaces())
    }

    /// What the output gives the body of `block` its column from: the
    /// lead of the `{` line when the `{` starts it, or else the column past
    /// the block's header. Function, control, and plain blocks only:
    /// switch bodies and case blocks follow astyle's own layout.
    fn block_body_anchor(&self, block: GroupId) -> Option<BodyAnchor> {
        let tokens = &self.tree.tokens;
        let open = self.tree.groups.get(block).open;
        let address = tokens.as_ptr() as usize;
        // The anchor reads no line past the `{` line, so it holds while no
        // line up to that one changes and later lines start past the `{`.
        let mut anchors = self.block_body_anchor_cache.borrow_mut();
        let slot = anchors
            .iter()
            .position(|cached| {
                cached.is_some_and(|cached| (cached.address, cached.block) == (address, block))
            })
            .unwrap_or(anchors.len() - 1);
        if let Some(cached) = anchors[slot]
            && (cached.address, cached.block) == (address, block)
            && self
                .output
                .lowest_change_since(cached.version)
                .is_some_and(|lowest| lowest > cached.brace_line)
            && self.output.lines_after_start_past(cached.brace_line, open)
        {
            bring_to_front(&mut anchors[..=slot]);
            return Some(cached.anchor);
        }
        // A block kept above is one of those.
        if anchors[slot].is_none_or(|cached| (cached.address, cached.block) != (address, block))
            && (!matches!(
                self.tree.blocks.kind(block),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            ) || self.tree.blocks.owner(block).is_some_and(|owner| {
                matches!(&tokens[owner], Token::Word(word)
                    if word == "switch" || word == "case" || word == "default")
            }))
        {
            return None;
        }
        let width = self.options.indent_width;
        let brace_line = self.output.line_with_token(open)?;
        let anchor = if self.output.line_tokens(brace_line)?.first == open {
            BodyAnchor::BraceLine(self.output.lead_width(brace_line, self.options.tab_width))
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
            BodyAnchor::Header(
                if let Some((line, levels)) = self.header_chain_before(open) {
                    Some(self.output.lead_width(line, self.options.tab_width) + levels * width)
                } else {
                    header
                        .and_then(|header| self.line_led_by(header))
                        .map(|header_line| {
                            self.output.lead_width(header_line, self.options.tab_width) + width
                        })
                },
            )
        };
        anchors[slot] = Some(BlockBodyAnchor {
            address,
            block,
            version: self.output.version(),
            brace_line,
            anchor,
        });
        bring_to_front(&mut anchors[..=slot]);
        Some(anchor)
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

/// Moves the last of `entries` to the front, keeping the order of the
/// others.
fn bring_to_front<T: Copy>(entries: &mut [T]) {
    if let Some(&last) = entries.last() {
        let mut carried = last;
        for entry in entries {
            carried = std::mem::replace(entry, carried);
        }
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

/// Whether the text after a header word is only the header's condition, if
/// any, and the `{` of its block: no further header shares the line.
fn header_line_opens_its_block(after_header: &str) -> bool {
    let rest = after_header.trim_start();
    let Some(condition) = rest.strip_prefix('(') else {
        return rest == "{";
    };
    let mut depth = 1usize;
    let mut quote = None;
    let mut escaped = false;
    for (index, ch) in condition.char_indices() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return condition[index + 1..].trim() == "{";
                }
            }
            _ => {}
        }
    }
    false
}
