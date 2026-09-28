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

use crate::formatter::lexer::{Token, token_text, tokenize};
use blocks::{Blocks, is_code_token, next_code_token};
use functions::Functions;
use groups::Groups;

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

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct SourceTree {
    pub(crate) tokens: Vec<Token>,
    pub(crate) groups: Groups,
    pub(crate) blocks: Blocks,
    pub(crate) functions: Functions,
}

impl SourceTree {
    pub(crate) fn build(tokens: &[Token]) -> Self {
        let groups = Groups::build(tokens);
        let mut blocks = Blocks::build(tokens, &groups);
        let functions = Functions::build(tokens, &groups, &blocks);
        blocks.mark_function_bodies(functions.heads().iter().filter_map(|head| head.body));
        Self {
            tokens: tokens.to_vec(),
            groups,
            blocks,
            functions,
        }
    }

    /// Byte offset in the formatted `line` of the source token `target`, for
    /// a line whose code tokens are the source code tokens from `first` on.
    /// `None` when the line's tokens do not follow the source up to `target`.
    pub(crate) fn token_offset_in_line(
        &self,
        line: &str,
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

    /// Last code token before `index`.
    pub(crate) fn previous_code_token(&self, index: usize) -> Option<usize> {
        blocks::previous_code_token(&self.tokens, index)
    }
}
