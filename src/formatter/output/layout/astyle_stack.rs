//! astyle's continuation indent stack, replayed over a statement's
//! published lines.
//!
//! astyle indents a continuation line at the top of a stack of registered
//! indents: an opening paren registers the column of what follows it, an
//! assignment the column of its value, `return` the column of the returned
//! value; a closing paren drops what its paren registered. A registered
//! column past the maximum falls back to two levels past the line's own
//! continuation, and a paren at a line end registers one continuation
//! level past the indent before.

use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::syntax::language::is_header;

/// Statements longer than this are left to the engine.
const MAX_REPLAYED_TOKENS: usize = 4000;

struct Replay {
    stack: Vec<usize>,
    sizes: Vec<usize>,
    parens: Vec<usize>,
    paren_statements: Vec<bool>,
    continuation: bool,
    depth: usize,
    line_space: usize,
    assigned_this_line: bool,
    header_paren: Option<usize>,
}

impl FormatEngine<'_> {
    /// A line starting inside parentheses stands at the top of astyle's
    /// continuation stack.
    pub(super) fn stacked_argument_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let group = groups.enclosing(first)?;
        if groups.get(group).delimiter != Delimiter::Paren
            || matches!(tokens[first], Token::Symbol('['))
            || self
                .tree
                .previous_code_token(groups.get(group).open)
                .is_some_and(|before| matches!(tokens[before], Token::Symbol(']')))
            || tokens[groups.get(group).open..first]
                .iter()
                .any(|token| matches!(token, Token::Symbol('{' | '}')))
        {
            return None;
        }
        // Only a compound literal's braces may follow in the parens.
        let close = groups.get(group).close.unwrap_or(tokens.len());
        if (first..close).any(|index| {
            let brace = match tokens[index] {
                Token::Symbol('{') => groups.opened_at(index),
                Token::Symbol('}') => groups.closed_at(index),
                _ => return false,
            };
            brace.is_none_or(|brace| {
                self.tree.blocks.kind(brace) != Some(BlockKind::CompoundLiteral)
            })
        }) {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// The indent astyle's continuation stack gives a part of a line that
    /// the code length split, starting at `first`.
    pub(crate) fn split_part_stack_indent(&self, first: usize) -> Option<usize> {
        self.stacked_argument_indent(first)
            .or_else(|| self.stacked_bracket_row_indent(first))
            .or_else(|| self.stacked_return_indent(first))
            .or_else(|| self.stacked_closing_paren_indent(first))
            .or_else(|| self.stacked_assignment_indent(first))
            .or_else(|| self.stacked_declarator_indent(first))
    }

    /// A declarator after a `,` ending its statement's first line stands at
    /// the top of astyle's continuation stack.
    fn stacked_declarator_indent(&self, first: usize) -> Option<usize> {
        let comma = self.tree.previous_code_token(first)?;
        let group = self.tree.groups.enclosing(first);
        if !matches!(self.tree.tokens[comma], Token::Symbol(','))
            || self.tree.groups.enclosing(comma) != group
            || group.is_some_and(|group| match self.tree.blocks.kind(group) {
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block) => false,
                // Enumerators are no declarators.
                Some(BlockKind::Aggregate) => self.tree.blocks.owner(group).is_none_or(|owner| {
                    self.tree.tokens[owner..self.tree.groups.get(group).open]
                        .iter()
                        .any(|token| matches!(token, Token::Word(word) if word == "enum"))
                }),
                _ => true,
            })
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// Whether `block` is a bare block opened right after a directive, which
    /// astyle takes for an array brace.
    pub(crate) fn is_directive_block(&self, block: GroupId) -> bool {
        let open = self.tree.groups.get(block).open;
        self.tree.blocks.kind(block) == Some(BlockKind::Block)
            && self.tree.tokens[self
                .tree
                .previous_code_token(open)
                .map_or(0, |before| before + 1)..open]
                .iter()
                .any(|token| matches!(token, Token::Preprocessor(_)))
    }

    /// A line inside brackets whose `[` ends its line stands at the top of
    /// astyle's continuation stack: a continuation level past the indent
    /// before.
    pub(super) fn stacked_bracket_row_indent(&self, first: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let group = groups.enclosing(first)?;
        let open = groups.get(group).open;
        if groups.get(group).delimiter != Delimiter::Bracket
            || matches!(self.tree.tokens[first], Token::Symbol(']'))
            || self
                .output
                .line_with_token(open)
                .and_then(|line| self.output.line_tokens(line))
                .is_none_or(|span| span.last != open)
            || self
                .tree
                .previous_code_token(open)
                .is_none_or(|before| !matches!(self.tree.tokens[before], Token::Word(_)))
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// A `)` starting its line stands where astyle saved its paren: the
    /// paren's column when code follows it, else the indent before.
    pub(super) fn stacked_closing_paren_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let group = self.tree.groups.closed_at(first)?;
        if !matches!(tokens[first], Token::Symbol(')'))
            || self
                .tree
                .previous_code_token(self.tree.groups.get(group).open)
                .is_some_and(|before| matches!(tokens[before], Token::Symbol(']')))
            || tokens[self.tree.groups.get(group).open..first]
                .iter()
                .any(|token| matches!(token, Token::Symbol('{' | '}')))
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// A line continuing a `return` statement outside its parentheses
    /// stands at the top of astyle's continuation stack.
    pub(super) fn stacked_return_indent(&self, first: usize) -> Option<usize> {
        let start = self.stack_statement_start(first)?;
        if !matches!(&self.tree.tokens[start], Token::Word(word) if word == "return")
            || self.tree.groups.enclosing(first) != self.tree.groups.enclosing(start)
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// A line continuing an assignment outside its parentheses stands at
    /// the top of astyle's continuation stack.
    pub(super) fn stacked_assignment_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let start = self.stack_statement_start(first)?;
        let group = groups.enclosing(first);
        if groups.enclosing(start) != group
            || group.is_some_and(|group| {
                !matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                )
            })
            || !self.options.indent_after_parens
                && tokens[start..first]
                    .iter()
                    .any(|token| matches!(token, Token::Preprocessor(_)))
            || !(start..first).any(|index| {
                groups.enclosing(index) == group
                    && matches!(&tokens[index], Token::Operator(operator)
                        if operator.ends_with('=')
                                && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">=")
                            || self.options.indent_after_parens
                                && matches!(operator.as_str(), "<<" | ">>"))
            })
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// The indent astyle's continuation stack gives the line starting at
    /// `first`, replayed from the start of its statement.
    fn astyle_stack_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let closes_stacked_brace = self.closes_stacked_initializer(first);
        if matches!(tokens[first], Token::Symbol(']' | '{' | ','))
            || matches!(tokens[first], Token::Symbol('}')) && !closes_stacked_brace
            || tokens[first..]
                .iter()
                .enumerate()
                .take_while(|&(offset, token)| {
                    !matches!(token, Token::Newline)
                        && self.tree.groups.enclosing(first).is_none_or(|group| {
                            self.tree
                                .groups
                                .get(group)
                                .close
                                .is_none_or(|close| first + offset < close)
                        })
                })
                // The block brace after a closing paren opens no group of
                // the line.
                .any(|(offset, token)| {
                    matches!(token, Token::Symbol('{' | '}'))
                        && !(offset == 0 && closes_stacked_brace)
                        && !(next_code_token(tokens, first + 1) == Some(first + offset)
                            && (self.paren_ends_its_line(first)
                                || self.options.indent_after_parens
                                    && matches!(tokens[first], Token::Symbol(')'))))
                })
        {
            return None;
        }
        let start = self.stack_statement_start(first)?;
        // Lambda bodies continue their statements past astyle's stack.
        if let Some(group) = self.tree.groups.enclosing(start)
            && self.tree.groups.ancestors(group).any(|id| {
                self.tree.blocks.kind(id) == Some(BlockKind::Lambda)
                    || (self.tree.blocks.kind(id) == Some(BlockKind::FunctionBody)
                        && self.header_holds_braces(self.tree.groups.get(id).open))
            })
        {
            return None;
        }
        if first - start > MAX_REPLAYED_TOKENS {
            return None;
        }
        let start_line = self.output.line_with_token(start)?;
        // The start leads its line, after at most `}` and `else`.
        let mut line_first = next_code_token(tokens, self.output.line_tokens(start_line)?.first)?;
        while line_first < start
            && (matches!(tokens[line_first], Token::Symbol('}'))
                || matches!(&tokens[line_first], Token::Word(word) if word == "else"))
        {
            line_first = next_code_token(tokens, line_first + 1)?;
        }
        // A body run in after its header on the line continues a level in,
        // as astyle indents that body.
        let mut header_levels = 0;
        while line_first < start
            && matches!(&tokens[line_first], Token::Word(word) if matches!(word.as_str(), "if" | "while" | "for"))
            && let Some(open) = next_code_token(tokens, line_first + 1)
            && matches!(tokens[open], Token::Symbol('('))
            && let Some(close) = self
                .tree
                .groups
                .opened_at(open)
                .and_then(|group| self.tree.groups.get(group).close)
            && close < start
            && self.output.line_with_token(close) == Some(start_line)
        {
            header_levels += 1;
            line_first = next_code_token(tokens, close + 1)?;
        }
        if line_first != start {
            return None;
        }
        // astyle takes a bare block brace after a directive for an array
        // brace and registers no assignment in it.
        let directive_block = self
            .tree
            .groups
            .enclosing(start)
            .is_some_and(|block| self.is_directive_block(block));
        // Only ternary arms are known to follow its parentheses there.
        let ternary_arm = matches!(tokens[first], Token::Symbol('?' | ':'))
            || matches!(&tokens[first], Token::Operator(operator) if matches!(operator.as_str(), "?" | ":"));
        if directive_block && !ternary_arm {
            return None;
        }
        let block_lead = self.output.lead_width(start_line, self.options.tab_width);
        // astyle registers no `=` of a designator in an initializer.
        let in_initializer =
            directive_block
                || self.tree.groups.enclosing(start).is_some_and(|group| {
                    self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
                });
        let mut replay = Replay {
            stack: Vec::new(),
            sizes: Vec::new(),
            parens: Vec::new(),
            paren_statements: Vec::new(),
            continuation: false,
            depth: 0,
            line_space: 0,
            assigned_this_line: false,
            header_paren: None,
        };
        let mut line = None;
        let mut saw_question = false;
        let mut index = start;
        while index < first {
            let token = &tokens[index];
            if !is_code_token(token) {
                index += 1;
                continue;
            }
            let token_line = self.output.line_with_token(index)?;
            let starts_line = line != Some(token_line);
            if starts_line {
                line = Some(token_line);
                replay.line_space = replay.stack.last().copied().unwrap_or(0);
                replay.assigned_this_line = false;
            }
            let relative = |index: usize| -> Option<usize> {
                self.token_column(index)?.checked_sub(block_lead)
            };
            let next_on_line = self
                .next_code_token_before(index, first)
                .filter(|&next| self.output.line_with_token(next) == Some(token_line));
            match token {
                Token::Symbol('(' | '[') => {
                    if replay.depth == 0 {
                        replay.paren_statements.push(replay.continuation);
                        replay.continuation = true;
                    }
                    replay.depth += 1;
                    replay.sizes.push(replay.stack.len());
                    self.register(&mut replay, index, next_on_line, true, &relative)?;
                }
                Token::Symbol(')' | ']') => {
                    replay.depth = replay.depth.checked_sub(1)?;
                    if replay.depth == 0 {
                        replay.continuation = replay.paren_statements.pop()?;
                    }
                    let size = replay.sizes.pop()?;
                    replay.stack.truncate(size);
                    let popped = replay.parens.pop()?;
                    if starts_line {
                        replay.line_space = popped;
                    }
                }
                Token::Symbol(',') => {
                    if replay.depth > 0 {
                        let size = *replay.sizes.last()?;
                        replay.stack.truncate(size + 1);
                    } else if next_on_line.is_none() && !replay.continuation {
                        // A `,` ending the statement's first line registers
                        // the line's second word.
                        if token_line != start_line {
                            return None;
                        }
                        let text = self.output.as_slice()[token_line].trim_start();
                        replay.stack.push(second_word_column(text, relative(index)?));
                        replay.continuation = true;
                    }
                }
                Token::Symbol('?') => saw_question = true,
                Token::Operator(operator) if operator == "?" => saw_question = true,
                Token::Symbol(':') if saw_question => {}
                Token::Symbol(';') if replay.depth > 0 => {
                    let size = *replay.sizes.last()?;
                    replay.stack.truncate(size + 1);
                }
                // Indenting after parens stacks an initializer brace as a
                // paren.
                Token::Symbol('{') if self.indents_initializer_brace(index) => {
                    replay.depth += 1;
                    replay.sizes.push(replay.stack.len());
                    self.register(&mut replay, index, next_on_line, true, &relative)?;
                }
                Token::Symbol('}')
                    if self
                        .tree
                        .groups
                        .closed_at(index)
                        .is_some_and(|group| self.indents_initializer_brace(self.tree.groups.get(group).open)) =>
                {
                    replay.depth = replay.depth.checked_sub(1)?;
                    let size = replay.sizes.pop()?;
                    replay.stack.truncate(size);
                    let popped = replay.parens.pop()?;
                    if starts_line {
                        replay.line_space = popped;
                    }
                }
                Token::Symbol('{' | '}' | ';' | ':') => return None,
                _ if index == start && is_control_keyword(token) => {
                    replay.header_paren = next_code_token(tokens, index + 1);
                }
                // The first shift outside parens registers its column, or,
                // when indenting after parens, a continuation level.
                Token::Operator(operator)
                    if matches!(operator.as_str(), "<<" | ">>") && replay.depth == 0 =>
                {
                    if !self.options.indent_after_parens {
                        // A line leading with a shift continues the chain its
                        // own way; others stack at the first shift.
                        if matches!(&tokens[first], Token::Operator(shift) if matches!(shift.as_str(), "<<" | ">>"))
                        {
                            return None;
                        }
                        if replay.stack.is_empty() {
                            let mut column = relative(index)?;
                            if column > self.options.max_continuation_indent {
                                column = 2 * self.options.indent_width + replay.line_space;
                            }
                            replay.stack.push(column);
                            replay.continuation = true;
                        }
                    } else if replay.stack.is_empty() {
                        self.register(&mut replay, index, next_on_line, false, &relative)?;
                        replay.continuation = true;
                    }
                }
                Token::Operator(operator)
                    if operator == "=" && in_initializer && replay.depth == 0 => {}
                Token::Operator(operator)
                    if operator.ends_with('=')
                        && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">=") =>
                {
                    let previous = self.tree.previous_code_token(index)?;
                    if operator == "="
                        && !matches!(tokens[previous], Token::Symbol(']'))
                        && self.statement_ends_with_comma(index, token_line, first)
                    {
                        // astyle registers again at each such line and
                        // drifts pointer declarators; the first holds.
                        if !replay.assigned_this_line && !replay.continuation {
                            replay.assigned_this_line = true;
                            let indent = match tokens[previous] {
                                Token::Word(_) | Token::Number(_) => relative(previous)?,
                                _ => replay.line_space,
                            };
                            replay.stack.push(indent);
                            replay.continuation = true;
                        }
                    } else {
                        self.register(&mut replay, index, next_on_line, false, &relative)?;
                        replay.continuation = true;
                    }
                }
                Token::Word(word) if word == "return" => {
                    self.register(&mut replay, index, next_on_line, false, &relative)?;
                    replay.continuation = true;
                }
                Token::Word(word) if word == "new" => {
                    if replay.continuation
                        && self
                            .tree
                            .previous_code_token(index)
                            .is_some_and(|previous| matches!(&tokens[previous], Token::Operator(operator) if operator == "="))
                        && let Some(top) = replay.stack.last_mut()
                    {
                        *top = 0;
                    }
                }
                Token::Word(word)
                    if is_header(word)
                        || matches!(
                            word.as_str(),
                            "operator" | "template" | "case" | "default" | "else" | "do"
                        ) =>
                {
                    return None;
                }
                Token::StringLiteral(text) | Token::CharLiteral(text) if !literal_closed(text) => {
                    return None;
                }
                _ => {}
            }
            index += 1;
        }
        // A `)` starting its line takes the indent its paren saved.
        let top = if matches!(tokens[first], Token::Symbol(')')) || closes_stacked_brace {
            replay.parens.last().copied()?
        } else {
            replay.stack.last().copied()?
        };
        Some(
            block_lead
                + top
                + header_levels * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// Whether the `{` at `open` opens an initializer that indenting after
    /// parens stacks like a paren: one with code after it on its line.
    fn indents_initializer_brace(&self, open: usize) -> bool {
        self.options.indent_after_parens
            && self.tree.groups.opened_at(open).is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
            })
            && self
                .output
                .line_with_token(open)
                .and_then(|line| self.output.line_tokens(line))
                .is_some_and(|span| span.last != open)
    }

    /// The row of an initializer that indenting after parens stacks.
    pub(super) fn stacked_initializer_row_indent(&self, first: usize) -> Option<usize> {
        if self.closes_stacked_initializer(first) {
            return self.astyle_stack_indent(first);
        }
        let group = self.tree.groups.enclosing(first)?;
        if !self.indents_initializer_brace(self.tree.groups.get(group).open)
            || matches!(self.tree.tokens[first], Token::Symbol('{' | '}'))
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// Whether `close` is the `}` of an initializer stacked like a paren.
    fn closes_stacked_initializer(&self, close: usize) -> bool {
        matches!(self.tree.tokens[close], Token::Symbol('}'))
            && self.tree.groups.closed_at(close).is_some_and(|group| {
                self.indents_initializer_brace(self.tree.groups.get(group).open)
            })
    }

    /// Whether `close` is a `)` whose `(` ends its line.
    fn paren_ends_its_line(&self, close: usize) -> bool {
        matches!(self.tree.tokens[close], Token::Symbol(')'))
            && self.tree.groups.closed_at(close).is_some_and(|group| {
                let open = self.tree.groups.get(group).open;
                self.output
                    .line_with_token(open)
                    .and_then(|line| self.output.line_tokens(line))
                    .is_some_and(|span| span.last == open)
            })
    }

    /// Whether the declaration before the body opening at `open` holds
    /// braces, such as `= {}` default arguments, which leave astyle in an
    /// array state for the body.
    fn header_holds_braces(&self, open: usize) -> bool {
        let tokens = &self.tree.tokens;
        let mut index = open;
        while let Some(before) = self.tree.previous_code_token(index) {
            if self.tree.groups.enclosing(before).is_none()
                && matches!(tokens[before], Token::Symbol(';' | '}'))
            {
                break;
            }
            if matches!(tokens[before], Token::Symbol('{')) {
                return true;
            }
            index = before;
        }
        false
    }

    /// Pushes the indent astyle registers at `index`: the column of the
    /// code after it on its line, or a continuation level past the indent
    /// before when it ends the line.
    fn register(
        &self,
        replay: &mut Replay,
        index: usize,
        next_on_line: Option<usize>,
        paren: bool,
        relative: &dyn Fn(usize) -> Option<usize>,
    ) -> Option<()> {
        let indent_width = self.options.indent_width;
        let max = self.options.max_continuation_indent;
        let Some(next) = next_on_line.filter(|_| !self.options.indent_after_parens) else {
            let previous = replay.stack.last().copied().unwrap_or(replay.line_space);
            let mut indent = self.options.continuation_indent * indent_width + previous;
            if indent > max {
                indent = 2 * indent_width + replay.line_space;
            }
            replay.stack.push(indent);
            if paren {
                replay.parens.push(previous);
            }
            return Some(());
        };
        if paren {
            replay.parens.push(relative(index)?);
        }
        let mut indent = relative(next)?;
        // The paren right after a control header holds at least the
        // minimum conditional indent.
        let min_conditional = min_conditional_indent_spaces(self.options);
        if paren && replay.header_paren == Some(index) && indent < min_conditional {
            indent = min_conditional + replay.line_space;
        }
        if indent > max {
            indent = 2 * indent_width + replay.line_space;
        }
        if let Some(&top) = replay.stack.last() {
            indent = indent.max(top);
        }
        replay.stack.push(indent);
        Some(())
    }

    fn next_code_token_before(&self, index: usize, end: usize) -> Option<usize> {
        next_code_token(&self.tree.tokens, index + 1).filter(|&next| next < end)
    }

    /// Whether the line holding the `=` at `assign` ends at a `,` with no
    /// paren left open after the `=`.
    fn statement_ends_with_comma(&self, assign: usize, line: usize, end: usize) -> bool {
        let tokens = &self.tree.tokens;
        let mut depth = 0isize;
        let mut last = None;
        let mut index = assign + 1;
        while index < end {
            if is_code_token(&tokens[index]) {
                if self.output.line_with_token(index) != Some(line) {
                    break;
                }
                match tokens[index] {
                    Token::Symbol('(') => depth += 1,
                    Token::Symbol(')') => depth -= 1,
                    _ => {}
                }
                last = Some(index);
            }
            index += 1;
        }
        depth <= 0 && last.is_some_and(|last| matches!(tokens[last], Token::Symbol(',')))
    }

    /// The first token of the statement holding `index`, past parens,
    /// brackets, and initializer braces around it.
    fn stack_statement_start(&self, index: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let is_block = |group: GroupId| {
            groups.get(group).delimiter == Delimiter::Brace
                && !matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
        };
        let mut start = index;
        while let Some(before) = self.tree.previous_code_token(start) {
            // A directive outside parentheses ends what astyle stacks when
            // the lines before it carry the indent of their block.
            if self.options.indent_preproc_block
                && tokens[before + 1..start]
                    .iter()
                    .enumerate()
                    .any(|(offset, token)| {
                        matches!(token, Token::Preprocessor(_))
                            && groups
                                .enclosing(before + 1 + offset)
                                .is_none_or(|group| groups.get(group).delimiter == Delimiter::Brace)
                    })
            {
                break;
            }
            if let Some(closed) = groups.closed_at(before) {
                let header = self
                    .tree
                    .previous_code_token(groups.get(closed).open)
                    .is_some_and(|keyword| is_control_keyword(&tokens[keyword]));
                if is_block(closed) || header {
                    break;
                }
                start = groups.get(closed).open;
                continue;
            }
            // astyle stacks nothing for the braces of an array whose `{`
            // ends its line: each row starts afresh.
            let row_array = |group: GroupId| {
                self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
                    && self
                        .output
                        .line_with_token(groups.get(group).open)
                        .is_some_and(|line| {
                            self.output
                                .line_tokens(line)
                                .is_some_and(|span| span.last == groups.get(group).open)
                        })
            };
            // Each enumerator starts its own statement.
            let enum_body = |group: GroupId| {
                self.tree.blocks.kind(group) == Some(BlockKind::Aggregate)
                    && self.tree.blocks.owner(group).is_some_and(|owner| {
                        tokens[owner..groups.get(group).open]
                            .iter()
                            .any(|token| matches!(token, Token::Word(word) if word == "enum"))
                    })
            };
            if groups.opened_at(before).is_some_and(|group| is_block(group) || row_array(group))
                || matches!(tokens[before], Token::Symbol(','))
                    && groups
                        .enclosing(before)
                        .is_some_and(|group| row_array(group) || enum_body(group))
                // The `;` of a `for` header stays inside its statement.
                || matches!(tokens[before], Token::Symbol(';'))
                    && groups
                        .enclosing(before)
                        .is_none_or(|group| groups.get(group).delimiter == Delimiter::Brace)
                || matches!(&tokens[before], Token::Word(word) if word == "else" || word == "do")
                || self.ends_case_label(before)
                || self.ends_user_label(before)
            {
                break;
            }
            start = before;
        }
        (start != index).then_some(start)
    }

    /// Whether the token `colon` is the `:` ending a statement label.
    pub(crate) fn ends_user_label(&self, colon: usize) -> bool {
        let tokens = &self.tree.tokens;
        matches!(tokens[colon], Token::Symbol(':'))
            && self
                .tree
                .groups
                .enclosing(colon)
                .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Brace)
            && self.tree.previous_code_token(colon).is_some_and(|label| {
                matches!(&tokens[label], Token::Word(word)
                    if !matches!(word.as_str(), "default" | "public" | "private" | "protected"))
                    && self.tree.previous_code_token(label).is_none_or(|previous| {
                        matches!(tokens[previous], Token::Symbol(';' | '{' | '}' | ':'))
                    })
            })
    }

    /// Whether the token `colon` is the `:` ending a `case` or `default`
    /// label.
    pub(crate) fn ends_case_label(&self, colon: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        if !matches!(tokens[colon], Token::Symbol(':')) {
            return false;
        }
        let group = groups.enclosing(colon);
        let mut index = colon;
        while let Some(before) = self.tree.previous_code_token(index) {
            if groups.enclosing(before) != group {
                let Some(closed) = groups.closed_at(before) else {
                    return false;
                };
                index = groups.get(closed).open;
                continue;
            }
            match &tokens[before] {
                Token::Symbol(';' | '{' | '}' | '?') => return false,
                Token::Word(word) if word == "case" || word == "default" => {
                    return self
                        .tree
                        .previous_code_token(before)
                        .is_none_or(|previous| {
                            matches!(tokens[previous], Token::Symbol(';' | '{' | '}' | ':'))
                        });
                }
                _ => index = before,
            }
        }
        false
    }
}

/// Whether a string or character literal ends at its closing quote.
pub(super) fn literal_closed(text: &str) -> bool {
    let Some(quote) = text
        .chars()
        .last()
        .filter(|quote| matches!(quote, '"' | '\''))
    else {
        return false;
    };
    let body = &text[..text.len() - 1];
    let Some(open) = body.find(quote) else {
        return false;
    };
    let escapes = body[open + 1..]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count();
    escapes % 2 == 0
}

fn is_control_keyword(token: &Token) -> bool {
    matches!(token, Token::Word(word) if matches!(word.as_str(), "if" | "while" | "for" | "switch"))
}

/// The column astyle registers for a `,` at `comma` ending `line`: its second
/// word, past a first word of at least three characters.
fn second_word_column(line: &str, comma: usize) -> usize {
    let name_char = |ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.');
    if !line.starts_with(name_char) {
        return 0;
    }
    let first_end = line.find(|ch: char| !name_char(ch)).unwrap_or(line.len());
    let after = first_end + 1;
    if after >= comma || after < 4 {
        return 0;
    }
    match line[after..].find(|ch: char| !matches!(ch, ' ' | '\t')) {
        Some(offset) if after + offset < comma => after + offset,
        _ => 0,
    }
}
