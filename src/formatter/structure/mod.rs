//! Structure of the token stream, computed once before formatting.
//!
//! Layout rules ask this tree what a token belongs to (which bracket group,
//! block, or function head) instead of re-parsing the text of emitted output
//! lines. Output lines record the source tokens they hold
//! ([`TokenSpan`]), which links them back to the tree.

// Layout rules move onto the tree stage by stage; parts of the API have no
// caller outside the tests yet.
#![allow(dead_code)]

pub(crate) mod blocks;
pub(crate) mod functions;
pub(crate) mod groups;
pub(crate) mod statements;

use crate::formatter::lexer::{Token, token_text, tokenize};
use crate::formatter::text::line_scan::preprocessor_directive;
use crate::formatter::text::line_view::LineView;
use blocks::{BlockKind, Blocks, is_code_token, next_code_token, previous_code_token};
use functions::Functions;
use groups::Groups;
use statements::Statements;
use std::rc::Rc;

/// First and last code token of an output line, as token indices.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct TokenSpan {
    pub(crate) first: usize,
    pub(crate) last: usize,
}

impl TokenSpan {
    pub(crate) fn contains(self, index: usize) -> bool {
        (self.first..=self.last).contains(&index)
    }
}

/// Block comments with text on an output line, as token indices.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub(crate) struct LineComments {
    /// Comment whose text starts the line.
    pub(crate) lead: Option<usize>,
    /// Last comment with text on the line.
    pub(crate) last: Option<usize>,
}

impl LineComments {
    pub(crate) fn mentions(self, token: usize) -> bool {
        self.lead == Some(token) || self.last == Some(token)
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct SourceTree {
    /// The tokens the tree describes, shared with the formatting pass.
    pub(crate) tokens: Rc<Vec<Token>>,
    pub(crate) groups: Groups,
    pub(crate) blocks: Blocks,
    pub(crate) functions: Functions,
    pub(crate) statements: Statements,
    /// Indices of the directive tokens, in order.
    directives: Vec<usize>,
    code_look_back: CodeLookBack,
}

/// How many tokens back a code token is looked for before the looks back
/// are read.
const NEAR_CODE_TOKENS: usize = 8;

/// The last two looks back for a code token, the latest first, kept to
/// read on from: a start among the tokens one passed has its answer, and
/// one past them reads back only to them. They take no part in what the
/// tree is.
#[derive(Debug, Clone, Default)]
struct CodeLookBack(std::cell::Cell<[Option<CodeLook>; 2]>);

/// A look back's start and the code token it found before it.
type CodeLook = (usize, Option<usize>);

impl PartialEq for CodeLookBack {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for CodeLookBack {}

impl SourceTree {
    pub(crate) fn build(tokens: &[Token]) -> Self {
        Self::build_shared(Rc::new(tokens.to_vec()))
    }

    pub(crate) fn build_shared(shared: Rc<Vec<Token>>) -> Self {
        Self::build_within(shared, None)
    }

    /// The tree of a block taken out of its context, whose first brace
    /// group is of `root` kind when given.
    pub(crate) fn build_block(tokens: &[Token], root: Option<BlockKind>) -> Self {
        Self::build_within(Rc::new(tokens.to_vec()), root)
    }

    fn build_within(shared: Rc<Vec<Token>>, root: Option<BlockKind>) -> Self {
        let tokens = shared.as_slice();
        let groups = Groups::build(tokens);
        let mut blocks = Blocks::build_within(tokens, &groups, root);
        let functions = Functions::build(tokens, &groups, &blocks);
        blocks.mark_function_bodies(functions.heads().iter().filter_map(|head| head.body));
        let statements = Statements::build(tokens, &groups, &blocks);
        let directives = tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| matches!(token, Token::Preprocessor(_)))
            .map(|(index, _)| index)
            .collect();
        Self {
            tokens: shared,
            groups,
            blocks,
            functions,
            statements,
            directives,
            code_look_back: CodeLookBack::default(),
        }
    }

    /// Byte offset in the formatted `line` of the source token `target`, for
    /// a line whose code tokens are the source code tokens from `first` on.
    /// `None` when the line's tokens do not follow the source up to `target`.
    pub(crate) fn token_offset_in_line(
        &self,
        line: &LineView<'_>,
        first: usize,
        target: usize,
    ) -> Option<usize> {
        let mut offset = 0;
        let mut source = first;
        for token in tokenize(line) {
            let text = token_text(&token);
            if is_code_token(&token) {
                let source_index = next_code_token(&self.tokens, source)?;
                if text != token_text(&self.tokens[source_index]) {
                    return None;
                }
                if source_index == target {
                    return Some(offset);
                }
                source = source_index + 1;
            }
            offset += text.len();
        }
        None
    }

    /// The code token before `index` as a compiler taking the first branch
    /// of each conditional reads the tokens: a later branch follows the code
    /// before its `#if`, and the code after `#endif` follows the end of the
    /// first branch.
    pub(crate) fn previous_code_token_in_first_branches(&self, index: usize) -> Option<usize> {
        let mut before = index.min(self.tokens.len());
        loop {
            let previous = previous_code_token(&self.tokens, before)?;
            let start = self
                .directives
                .partition_point(|&directive| directive <= previous);
            // Most code has no directive between a token and the one before.
            if self
                .directives
                .get(start)
                .is_none_or(|&directive| directive >= before)
            {
                return Some(previous);
            }
            let end = self
                .directives
                .partition_point(|&directive| directive < before);
            // The last conditional directive between them decides.
            let jump = self.directives[start..end]
                .iter()
                .rev()
                .find_map(|&directive| {
                    let Token::Preprocessor(text) = &self.tokens[directive] else {
                        return None;
                    };
                    match preprocessor_directive(&text.text) {
                        Some("endif") => Some(self.first_branch_end_of(directive)),
                        Some("else" | "elif" | "elifdef" | "elifndef") => {
                            Some(self.conditional_start_of(directive))
                        }
                        _ => None,
                    }
                });
            match jump {
                Some(Some(target)) if target < before => before = target,
                _ => return Some(previous),
            }
        }
    }

    /// The directive ending the first branch of the conditional `endif`
    /// closes, when it has more than one branch.
    fn first_branch_end_of(&self, endif: usize) -> Option<usize> {
        let at = self
            .directives
            .partition_point(|&directive| directive < endif);
        let mut depth = 0usize;
        let mut first_branch_end = None;
        for &directive in self.directives[..at].iter().rev() {
            let Token::Preprocessor(text) = &self.tokens[directive] else {
                continue;
            };
            match preprocessor_directive(&text.text) {
                Some("endif") => depth += 1,
                Some("if" | "ifdef" | "ifndef") if depth == 0 => return first_branch_end,
                Some("if" | "ifdef" | "ifndef") => depth -= 1,
                Some("else" | "elif" | "elifdef" | "elifndef") if depth == 0 => {
                    first_branch_end = Some(directive);
                }
                _ => {}
            }
        }
        None
    }

    /// The `#if` of the conditional a branch directive `branch` continues.
    fn conditional_start_of(&self, branch: usize) -> Option<usize> {
        let at = self
            .directives
            .partition_point(|&directive| directive < branch);
        let mut depth = 0usize;
        for &directive in self.directives[..at].iter().rev() {
            let Token::Preprocessor(text) = &self.tokens[directive] else {
                continue;
            };
            match preprocessor_directive(&text.text) {
                Some("endif") => depth += 1,
                Some("if" | "ifdef" | "ifndef") if depth == 0 => return Some(directive),
                Some("if" | "ifdef" | "ifndef") => depth -= 1,
                _ => {}
            }
        }
        None
    }

    /// Whether `token` stands in a branch after the first of a conditional
    /// that closes before `later`: a compiler taking first branches reads
    /// nothing of it before `later`.
    pub(crate) fn in_later_branch_closing_before(&self, token: usize, later: usize) -> bool {
        let start = self
            .directives
            .partition_point(|&directive| directive < token);
        let end = self
            .directives
            .partition_point(|&directive| directive < later);
        let mut depth = 0usize;
        for &directive in &self.directives[start..end] {
            let Token::Preprocessor(text) = &self.tokens[directive] else {
                continue;
            };
            match preprocessor_directive(&text.text) {
                Some("if" | "ifdef" | "ifndef") => depth += 1,
                Some("endif") if depth > 0 => depth -= 1,
                Some("endif")
                    if self
                        .first_branch_end_of(directive)
                        .is_some_and(|first_end| first_end < token) =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

    /// Whether a directive token lies in `range`.
    pub(crate) fn has_directive_in(&self, range: std::ops::Range<usize>) -> bool {
        let at = self
            .directives
            .partition_point(|&index| index < range.start);
        self.directives
            .get(at)
            .is_some_and(|&index| index < range.end)
    }

    /// Whether an `#else` or `#elif` in `range` switches away from a branch
    /// whose `#if` comes before the range.
    pub(crate) fn switches_outer_branch_in(&self, range: std::ops::Range<usize>) -> bool {
        let at = self
            .directives
            .partition_point(|&index| index < range.start);
        let mut depth = 0usize;
        for &index in self.directives[at..]
            .iter()
            .take_while(|&&index| index < range.end)
        {
            let Token::Preprocessor(directive) = &self.tokens[index] else {
                continue;
            };
            match preprocessor_directive(&directive.text) {
                Some("if" | "ifdef" | "ifndef") => depth += 1,
                Some("endif") => depth = depth.saturating_sub(1),
                Some("else" | "elif" | "elifdef" | "elifndef") if depth == 0 => return true,
                _ => {}
            }
        }
        false
    }

    /// Last code token before `index`.
    #[inline]
    pub(crate) fn previous_code_token(&self, index: usize) -> Option<usize> {
        if index > self.tokens.len() {
            return None;
        }
        // Most code tokens stand a few tokens back; only longer walks read
        // on from the looks.
        let near = index.saturating_sub(NEAR_CODE_TOKENS);
        if let Some(found) = (near..index)
            .rev()
            .find(|&token| is_code_token(&self.tokens[token]))
        {
            return Some(found);
        }
        if near == 0 {
            return None;
        }
        self.previous_code_token_far(near)
    }

    #[inline(never)]
    fn previous_code_token_far(&self, index: usize) -> Option<usize> {
        let mut looks = self.code_look_back.0.get();
        let mut base: Option<usize> = None;
        for (slot, &(before, found)) in looks
            .iter()
            .enumerate()
            .filter_map(|(slot, look)| look.as_ref().map(|look| (slot, look)))
        {
            if index <= before && found.is_none_or(|found| found < index) {
                return found;
            }
            if before < index
                && base.is_none_or(|base| looks[base].is_some_and(|(far, _)| far < before))
            {
                base = Some(slot);
            }
        }
        let found = match base.and_then(|base| looks[base]) {
            Some((before, found)) => (before..index)
                .rev()
                .find(|&token| is_code_token(&self.tokens[token]))
                .or(found),
            None => blocks::previous_code_token(&self.tokens, index),
        };
        let replaced = base.unwrap_or(1);
        looks[replaced] = Some((index, found));
        looks.swap(0, replaced);
        self.code_look_back.0.set(looks);
        found
    }
}
