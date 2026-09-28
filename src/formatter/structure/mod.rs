//! Structure of the token stream, computed once before formatting.
//!
//! Layout rules ask this tree what a token belongs to (which bracket group,
//! later which block, statement, or function head) instead of re-parsing
//! the text of emitted output lines.

// Layout rules move onto the tree stage by stage; until the engine reads it,
// parts of the API have no caller outside the tests.
#![allow(dead_code)]

pub(crate) mod blocks;
pub(crate) mod functions;
pub(crate) mod groups;

use crate::formatter::lexer::Token;
use blocks::Blocks;
use functions::Functions;
use groups::Groups;

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct SourceTree {
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
            groups,
            blocks,
            functions,
        }
    }
}
