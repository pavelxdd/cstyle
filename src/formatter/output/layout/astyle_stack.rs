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

use crate::config::BraceStyle;
use crate::formatter::continuation::min_conditional_indent_spaces;
use crate::formatter::engine::{FormatEngine, ForwardFind};
use crate::formatter::lexer::Token;
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::{Delimiter, GroupId};
use crate::formatter::syntax::language::is_header;
use crate::formatter::text::trim::Trimmed;

/// Statements longer than this are left to the engine.
const MAX_REPLAYED_TOKENS: usize = 4000;

/// The braces of parens: the tokens' address, the parens, and their first
/// brace and last that is no compound literal's.
#[derive(Clone, Copy)]
pub(crate) struct ParenBraces {
    address: usize,
    group: GroupId,
    braces: (Option<usize>, Option<usize>),
}

/// A look back from a `:` for the `case` it may end: the tokens' address,
/// the `:`'s group, the tokens of that level it passed, and its answer.
#[derive(Clone, Copy)]
pub(crate) struct CaseLabelLook {
    address: usize,
    group: Option<GroupId>,
    low: usize,
    high: usize,
    answer: bool,
}

/// The tokens the last looks back for a statement start passed, all of
/// which look back to the same start: the tokens' address, the tokens in
/// order, and the start.
#[derive(Default)]
pub(crate) struct StackStartPath {
    address: usize,
    positions: Vec<u32>,
    start: usize,
    /// The tokens one look back passes, kept to reuse its buffer.
    passed: Vec<u32>,
}

/// The replay of a statement up to a line, kept so the next line of the
/// statement goes on from it: the tokens' address, the statement start, the
/// output version, the token reached, and the state there.
pub(crate) struct ReplayCache {
    address: usize,
    start: usize,
    version: u64,
    index: usize,
    replay: Replay,
    line: Option<usize>,
    saw_question: bool,
    /// Whether a shift outside parens stacked before `index`; a line
    /// leading with a shift ends the replay there.
    shift_stacked: bool,
}

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
        {
            return None;
        }
        // Only a compound literal's braces may follow in the parens.
        let (first_brace, last_other_brace) = self.paren_braces(group);
        if first_brace.is_some_and(|brace| brace < first)
            || last_other_brace.is_some_and(|brace| brace >= first)
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// The first brace in the parens `group` and the last that is no
    /// compound literal's.
    fn paren_braces(&self, group: GroupId) -> (Option<usize>, Option<usize>) {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let address = tokens.as_ptr() as usize;
        if let Some(cached) = self.paren_braces_cache.get()
            && (cached.address, cached.group) == (address, group)
        {
            return cached.braces;
        }
        let open = groups.get(group).open;
        let close = groups.get(group).close.unwrap_or(tokens.len());
        let mut braces = (None, None);
        for index in open..close {
            let brace = match tokens[index] {
                Token::Symbol('{') => groups.opened_at(index),
                Token::Symbol('}') => groups.closed_at(index),
                _ => continue,
            };
            braces.0.get_or_insert(index);
            if brace.is_none_or(|brace| {
                self.tree.blocks.kind(brace) != Some(BlockKind::CompoundLiteral)
            }) {
                braces.1 = Some(index);
            }
        }
        self.paren_braces_cache.set(Some(ParenBraces {
            address,
            group,
            braces,
        }));
        braces
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
            && self.tree.has_directive_in(
                self.tree
                    .previous_code_token(open)
                    .map_or(0, |before| before + 1)..open,
            )
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
            || !self.options.indent_after_parens && self.tree.has_directive_in(start..first)
            // A line past `#else` stands where the branch's `#if` left the stack.
            || self.tree.switches_outer_branch_in(start..first)
            || !self.stacks_assignment_before(start, first, group)
        {
            return None;
        }
        self.astyle_stack_indent(first)
    }

    /// Whether an assignment, or a shift when parens indent, stands at the
    /// level of `group` from `start` to `first`, `group` enclosing `start`.
    fn stacks_assignment_before(&self, start: usize, first: usize, group: Option<GroupId>) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        ForwardFind::first_in(
            &self.stacked_assignment_cache,
            tokens,
            start,
            first,
            |index| {
                groups.enclosing(index) == group
                    && matches!(&tokens[index], Token::Operator(operator)
                    if operator.ends_with('=')
                            && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">=")
                        || self.options.indent_after_parens
                            && matches!(operator.as_str(), "<<" | ">>"))
            },
        )
        .is_some()
    }

    /// Whether a brace stands from `first` to the end of its line or of the
    /// group enclosing it, a brace at `first` aside when it closes a stacked
    /// initializer.
    fn line_brace_after(&self, first: usize, closes_stacked_brace: bool) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let limit = groups
            .enclosing(first)
            .and_then(|group| groups.get(group).close)
            .unwrap_or(usize::MAX);
        let mut from = first;
        loop {
            let at = self.next_brace_or_newline(from);
            if at >= limit
                || tokens
                    .get(at)
                    .is_none_or(|token| matches!(token, Token::Newline))
            {
                return false;
            }
            // The block brace after a closing paren opens no group of the
            // line.
            if !(at == first && closes_stacked_brace)
                && !(next_code_token(tokens, first + 1) == Some(at)
                    && (self.paren_ends_its_line(first)
                        || self.options.indent_after_parens
                            && matches!(tokens[first], Token::Symbol(')'))))
            {
                return true;
            }
            from = at + 1;
        }
    }

    /// The first brace or newline token from `from` on, or the end of the
    /// tokens.
    fn next_brace_or_newline(&self, from: usize) -> usize {
        let tokens = &self.tree.tokens;
        let address = tokens.as_ptr() as usize;
        if let Some((cached_address, cached_from, stop)) = self.brace_or_newline_cache.get()
            && cached_address == address
            && (cached_from..=stop).contains(&from)
        {
            return stop;
        }
        let stop = tokens[from.min(tokens.len())..]
            .iter()
            .position(|token| matches!(token, Token::Symbol('{' | '}') | Token::Newline))
            .map_or(tokens.len(), |offset| from + offset);
        self.brace_or_newline_cache.set(Some((address, from, stop)));
        stop
    }

    /// The indent astyle's continuation stack gives the line starting at
    /// `first`, replayed from the start of its statement.
    fn astyle_stack_indent(&self, first: usize) -> Option<usize> {
        let tokens = &self.tree.tokens;
        let closes_stacked_brace = self.closes_stacked_initializer(first);
        if matches!(tokens[first], Token::Symbol(']' | '{' | ','))
            || matches!(tokens[first], Token::Symbol('}')) && !closes_stacked_brace
            || self.line_brace_after(first, closes_stacked_brace)
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
        // Indenting after parens, astyle measures a line run in after a
        // block brace from the brace.
        let column_base = self
            .run_in_brace_lead(start, start_line)
            .unwrap_or(block_lead);
        // astyle registers no `=` of a designator in an initializer.
        let in_initializer =
            directive_block
                || self.tree.groups.enclosing(start).is_some_and(|group| {
                    self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
                });
        let address = tokens.as_ptr() as usize;
        let version = self.output.version();
        let first_is_shift = matches!(&tokens[first], Token::Operator(shift) if matches!(shift.as_str(), "<<" | ">>"));
        // The tokens before an earlier line replay as they did then.
        let (mut replay, mut line, mut saw_question, mut index, mut shift_stacked) =
            match self.astyle_replay_cache.take().filter(|cached| {
                (cached.address, cached.start, cached.version) == (address, start, version)
                    && cached.index <= first
            }) {
                Some(cached) => (
                    cached.replay,
                    cached.line,
                    cached.saw_question,
                    cached.index,
                    cached.shift_stacked,
                ),
                None => (
                    Replay {
                        stack: Vec::new(),
                        sizes: Vec::new(),
                        parens: Vec::new(),
                        paren_statements: Vec::new(),
                        continuation: false,
                        depth: 0,
                        line_space: 0,
                        assigned_this_line: false,
                        header_paren: None,
                    },
                    None,
                    false,
                    start,
                    false,
                ),
            };
        if shift_stacked && first_is_shift {
            return None;
        }
        // The line found last and the tokens it settles the line of.
        let mut reach: Option<(usize, TokenSpan, usize)> = None;
        let mut line_of = |index: usize| -> Option<usize> {
            if let Some((line, span, next)) = reach
                && (span.first..next).contains(&index)
            {
                return (index <= span.last).then_some(line);
            }
            let line = self.output.line_with_token(index)?;
            reach = self
                .output
                .ordered_line_reach(line)
                .map(|(span, next)| (line, span, next));
            Some(line)
        };
        // A string continued over lines maps to the line it ends on and
        // stands on the line it starts on.
        let rows_before_end = |index: usize| match &tokens[index] {
            Token::StringLiteral(text) => text.matches('\n').count(),
            _ => 0,
        };
        while index < first {
            let token = &tokens[index];
            if !is_code_token(token) {
                index += 1;
                continue;
            }
            let token_line = line_of(index)?.checked_sub(rows_before_end(index))?;
            let starts_line = line != Some(token_line);
            if starts_line {
                line = Some(token_line);
                // A line an assignment leads, continuing the statement past
                // nothing registered, stands a continuation in.
                let leading_assignment = index != start
                    && matches!(token, Token::Operator(operator) if operator.ends_with('=')
                        && !matches!(operator.as_str(), "==" | "!=" | "<=" | ">="));
                replay.line_space = replay
                    .stack
                    .last()
                    .copied()
                    .unwrap_or(if leading_assignment {
                        self.options.continuation_indent * self.options.indent_width
                    } else {
                        0
                    });
                replay.assigned_this_line = false;
            }
            let relative = |index: usize| -> Option<usize> {
                self.token_column(index)?.checked_sub(column_base)
            };
            let next_on_line = self.next_code_token_before(index, first).filter(|&next| {
                line_of(next).and_then(|line| line.checked_sub(rows_before_end(next)))
                    == Some(token_line)
            });
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
                        let text = self.output.as_slice()[token_line].trimmed_start();
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
                        if first_is_shift {
                            return None;
                        }
                        shift_stacked = true;
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
                            "operator" | "case" | "default" | "else" | "do"
                        )
                        || word == "template" && template_keyword(tokens, index) =>
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
            replay.parens.last().copied()
        } else {
            replay.stack.last().copied()
        };
        self.astyle_replay_cache.set(Some(ReplayCache {
            address,
            start,
            version,
            index,
            replay,
            line,
            saw_question,
            shift_stacked,
        }));
        let top = top?;
        Some(
            block_lead
                + top
                + header_levels * self.options.indent_width
                + self.case_unindent_spaces(),
        )
    }

    /// The lead of the block brace alone on the line before `start_line`
    /// that a style running bodies in joins `start`'s line to.
    fn run_in_brace_lead(&self, start: usize, start_line: usize) -> Option<usize> {
        if !self.options.indent_after_parens
            || !matches!(
                self.options.brace_style,
                BraceStyle::Pico | BraceStyle::Horstmann
            )
        {
            return None;
        }
        let tokens = &self.tree.tokens;
        let brace = self.tree.previous_code_token(start)?;
        let brace_line = self.output.line_with_token(brace)?;
        (matches!(tokens[brace], Token::Symbol('{'))
            && brace_line + 1 == start_line
            && self.output.code_before_comment(brace_line).trimmed() == "{"
            && self.tree.groups.opened_at(brace).is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
                )
            }))
        .then(|| self.output.lead_width(brace_line, self.options.tab_width))
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
    #[inline(never)]
    pub(super) fn stacked_initializer_row_indent(&self, first: usize) -> Option<usize> {
        // Only indenting after parens stacks an initializer.
        if !self.options.indent_after_parens {
            return None;
        }
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
        let key = (tokens.as_ptr() as usize, open);
        if let Some((address, cached_open, holds)) = self.header_braces_cache.get()
            && (address, cached_open) == key
        {
            return holds;
        }
        // The header ends at its scope's statement before it; the brace of
        // that scope, as an `extern "C"` block's, is none of its own.
        let scope = self.tree.groups.enclosing(open);
        let mut index = open;
        let holds = loop {
            let Some(before) = self.tree.previous_code_token(index) else {
                break false;
            };
            if self.tree.groups.enclosing(before) == scope
                && matches!(tokens[before], Token::Symbol(';' | '}'))
            {
                break false;
            }
            if scope.is_some_and(|scope| self.tree.groups.get(scope).open == before) {
                break false;
            }
            if matches!(tokens[before], Token::Symbol('{')) {
                break true;
            }
            index = before;
        };
        self.header_braces_cache.set(Some((key.0, key.1, holds)));
        holds
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

    /// Whether `group` is a brace group other than an initializer's, which
    /// a statement does not continue past.
    fn is_stack_block(&self, group: GroupId) -> bool {
        self.tree.groups.get(group).delimiter == Delimiter::Brace
            && !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
            )
    }

    /// Whether the parens `closed` hold a control header's condition.
    fn closes_control_header(&self, closed: GroupId) -> bool {
        self.tree
            .previous_code_token(self.tree.groups.get(closed).open)
            .is_some_and(|keyword| is_control_keyword(&self.tree.tokens[keyword]))
    }

    /// Whether the code token `before` ends what a statement after it
    /// continues: a `;` or a brace of a block, a control header, or `else`
    /// and `do`.
    pub(super) fn ends_stack_statement(&self, before: usize) -> bool {
        let groups = &self.tree.groups;
        match &self.tree.tokens[before] {
            Token::Symbol(';') => groups
                .enclosing(before)
                .is_none_or(|group| groups.get(group).delimiter == Delimiter::Brace),
            Token::Symbol('{') => groups
                .opened_at(before)
                .is_some_and(|group| self.is_stack_block(group)),
            Token::Symbol(')' | ']' | '}') => groups.closed_at(before).is_some_and(|closed| {
                self.is_stack_block(closed) || self.closes_control_header(closed)
            }),
            Token::Word(word) => word == "else" || word == "do",
            _ => false,
        }
    }

    /// The first token of the statement holding `index`, past parens,
    /// brackets, and initializer braces around it.
    fn stack_statement_start(&self, index: usize) -> Option<usize> {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        let is_block = |group: GroupId| self.is_stack_block(group);
        // Most lines start a statement, where the look back below stops at
        // once.
        if self
            .tree
            .previous_code_token(index)
            .is_none_or(|before| self.ends_stack_statement(before))
        {
            return None;
        }
        let tokens_address = self.tree.tokens.as_ptr() as usize;
        // The look back depends on nothing but where it stands, so it ends
        // where the last one did once it reaches a token that one passed.
        let mut path = self.stack_start_cache.borrow_mut();
        if path.address != tokens_address {
            *path = StackStartPath {
                address: tokens_address,
                ..StackStartPath::default()
            };
        }
        let mut passed = std::mem::take(&mut path.passed);
        passed.clear();
        let mut start = index;
        let joined = loop {
            if path
                .positions
                .last()
                .is_some_and(|&last| start as u32 <= last)
                && let Ok(at) = path.positions.binary_search(&(start as u32))
            {
                break Some(at);
            }
            passed.push(start as u32);
            let Some(before) = self.tree.previous_code_token(start) else {
                break None;
            };
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
                break None;
            }
            if let Some(closed) = groups.closed_at(before) {
                if is_block(closed) || self.closes_control_header(closed) {
                    break None;
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
                || matches!(tokens[before], Token::Symbol(':'))
                    && (self.ends_case_label(before) || self.ends_user_label(before))
            {
                break None;
            }
            start = before;
        };
        match joined {
            Some(at) => {
                start = path.start;
                path.positions.truncate(at + 1);
            }
            None => {
                path.positions.clear();
                path.start = start;
            }
        }
        path.positions.extend(passed.iter().rev());
        path.passed = passed;
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
        // A look back reads the same from each token of its level it
        // passes, so a later one ends as an earlier one did there.
        let address = tokens.as_ptr() as usize;
        let cached = self
            .case_label_cache
            .get()
            .filter(|cached| (cached.address, cached.group) == (address, group));
        let mut index = colon;
        let answer = loop {
            if let Some(cached) = cached
                && (cached.low..=cached.high).contains(&index)
            {
                break cached.answer;
            }
            let Some(before) = self.tree.previous_code_token(index) else {
                break false;
            };
            if groups.enclosing(before) != group {
                let Some(closed) = groups.closed_at(before) else {
                    break false;
                };
                index = groups.get(closed).open;
                continue;
            }
            match &tokens[before] {
                Token::Symbol(';' | '{' | '}' | '?') => break false,
                Token::Word(word) if word == "case" || word == "default" => {
                    break self
                        .tree
                        .previous_code_token(before)
                        .is_none_or(|previous| {
                            matches!(tokens[previous], Token::Symbol(';' | '{' | '}' | ':'))
                        });
                }
                _ => index = before,
            }
        };
        let (low, high) = match cached {
            Some(cached) if (cached.low..=cached.high).contains(&index) => {
                (cached.low, cached.high.max(colon))
            }
            _ => (index, colon),
        };
        self.case_label_cache.set(Some(CaseLabelLook {
            address,
            group,
            low,
            high,
            answer,
        }));
        answer
    }
}

/// Whether a string or character literal ends at its closing quote.
pub(super) fn literal_closed(text: &str) -> bool {
    // The quotes and the backslash are ASCII, and no byte of a wider
    // character equals one.
    let Some((&quote, body)) = text
        .as_bytes()
        .split_last()
        .filter(|(quote, _)| matches!(quote, b'"' | b'\''))
    else {
        return false;
    };
    let Some(open) = body.iter().position(|&byte| byte == quote) else {
        return false;
    };
    let escapes = body[open + 1..]
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
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

/// Whether the `template` at `index` opens a template parameter list; a
/// name like any other otherwise.
fn template_keyword(tokens: &[Token], index: usize) -> bool {
    next_code_token(tokens, index + 1).is_some_and(
        |next| matches!(&tokens[next], Token::Operator(operator) if operator.starts_with('<')),
    )
}

#[cfg(test)]
mod tests {
    use super::literal_closed;

    fn closed_by_chars(text: &str) -> bool {
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

    #[test]
    fn reads_literal_ends_as_a_walk_over_characters_does() {
        let pieces = ["\"", "'", "\\", "a", "é", "L", "u8", " ", "\\\\"];
        let mut state = 0x2545_f491_u32;
        for _ in 0..20_000 {
            let mut text = String::new();
            for _ in 0..(state % 7) {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                text.push_str(pieces[state as usize % pieces.len()]);
            }
            state = state.wrapping_add(1);
            assert_eq!(literal_closed(&text), closed_by_chars(&text), "{text:?}");
        }
    }
}
