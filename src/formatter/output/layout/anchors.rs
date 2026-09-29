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
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::text::line_scan::preprocessor_directive;

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
            .or_else(|| self.statement_expression_indent(first))
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
        if !statements.starts_block_statement(first) && statements.braceless_header(first).is_none()
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
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let first = self.output.pending_tokens()?.first;
        let condition = groups
            .ancestors(groups.enclosing(first)?)
            .take_while(|&id| groups.get(id).delimiter == Delimiter::Paren)
            .last()?;
        let keyword = self.tree.previous_code_token(groups.get(condition).open)?;
        if !matches!(&tokens[keyword], Token::Word(word)
            if matches!(word.as_str(), "if" | "for" | "while" | "switch"))
        {
            return None;
        }
        let line = self.line_led_by(keyword)?;
        Some(self.output.lead_width(line, self.options.tab_width))
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
        // Case blocks move lines after layout; their closers stay with it.
        if self.case_unindent_spaces() > 0 {
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
        Some(self.output.lead_width(line, self.options.tab_width) + extra)
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
