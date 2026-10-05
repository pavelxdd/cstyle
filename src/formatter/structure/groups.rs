//! Bracket groups: matched `()`, `[]`, and `{}` pairs with parent links.

use crate::formatter::lexer::Token;
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::text::line_scan::preprocessor_directive;

/// A group's index, stored past zero so that an absent group costs no space
/// in the per-token tables.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub(crate) struct GroupId(std::num::NonZeroU32);

impl GroupId {
    fn new(index: usize) -> Self {
        let stored = u32::try_from(index + 1).expect("fewer than 2^32 groups");
        Self(std::num::NonZeroU32::new(stored).expect("stored past zero"))
    }

    pub(crate) fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum Delimiter {
    Paren,
    Bracket,
    Brace,
}

impl Delimiter {
    fn opened_by(token: &Token) -> Option<Self> {
        match token {
            Token::Symbol('(') => Some(Self::Paren),
            Token::Symbol('[') => Some(Self::Bracket),
            Token::Symbol('{') => Some(Self::Brace),
            _ => None,
        }
    }

    fn closed_by(token: &Token) -> Option<Self> {
        match token {
            Token::Symbol(')') => Some(Self::Paren),
            Token::Symbol(']') => Some(Self::Bracket),
            Token::Symbol('}') => Some(Self::Brace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Group {
    pub(crate) delimiter: Delimiter,
    /// Token index of the opening delimiter.
    pub(crate) open: usize,
    /// Token index of the matching closing delimiter; `None` when the source
    /// never closes the group.
    pub(crate) close: Option<usize>,
    pub(crate) parent: Option<GroupId>,
}

/// All bracket groups of a token stream.
///
/// Conditional preprocessor branches are parsed like a compiler that takes
/// the first branch: `#elif`/`#else` branches start from the nesting at
/// `#if`, and `#endif` continues with the nesting left by the first branch.
/// A closing delimiter never closes a group across an unclosed brace unless
/// it is itself a brace, so a stray `)` cannot end a block.
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct Groups {
    groups: Vec<Group>,
    /// Innermost group that contains each token; the delimiters of a group
    /// belong to its parent.
    enclosing: Vec<Option<GroupId>>,
    /// Group whose opening or closing delimiter each token is.
    delimits: Vec<Option<GroupId>>,
}

struct ConditionalFrame {
    stack_at_if: Vec<GroupId>,
    stack_after_first_branch: Option<Vec<GroupId>>,
}

impl Groups {
    pub(crate) fn build(tokens: &[Token]) -> Self {
        let mut groups = Self {
            groups: Vec::new(),
            enclosing: vec![None; tokens.len()],
            delimits: vec![None; tokens.len()],
        };
        let mut stack: Vec<GroupId> = Vec::new();
        let mut conditionals: Vec<ConditionalFrame> = Vec::new();
        for (index, token) in tokens.iter().enumerate() {
            groups.enclosing[index] = stack.last().copied();
            if let Token::Preprocessor(directive) = token {
                track_conditional(&directive.text, &mut stack, &mut conditionals);
            } else if let Some(delimiter) = Delimiter::opened_by(token) {
                let id = GroupId::new(groups.groups.len());
                groups.groups.push(Group {
                    delimiter,
                    open: index,
                    close: None,
                    parent: stack.last().copied(),
                });
                groups.delimits[index] = Some(id);
                stack.push(id);
            } else if let Some(delimiter) = Delimiter::closed_by(token)
                && let Some(depth) = groups.matching_open_depth(&stack, delimiter)
            {
                let id = stack[depth];
                stack.truncate(depth);
                groups.groups[id.index()].close = Some(index);
                groups.delimits[index] = Some(id);
                groups.enclosing[index] = groups.groups[id.index()].parent;
            }
        }
        groups
    }

    /// Stack depth of the group a closing `delimiter` ends, if any.
    fn matching_open_depth(&self, stack: &[GroupId], delimiter: Delimiter) -> Option<usize> {
        for (depth, id) in stack.iter().enumerate().rev() {
            let open = self.groups[id.index()].delimiter;
            if open == delimiter {
                return Some(depth);
            }
            if open == Delimiter::Brace {
                return None;
            }
        }
        None
    }

    pub(crate) fn get(&self, id: GroupId) -> &Group {
        &self.groups[id.index()]
    }

    pub(crate) fn len(&self) -> usize {
        self.groups.len()
    }

    pub(crate) fn ids(&self) -> impl Iterator<Item = GroupId> + '_ {
        (0..self.groups.len()).map(GroupId::new)
    }

    /// Innermost group containing the token at `index`, not counting the
    /// group the token opens or closes.
    pub(crate) fn enclosing(&self, index: usize) -> Option<GroupId> {
        self.enclosing.get(index).copied().flatten()
    }

    /// Group opened or closed by the token at `index`.
    pub(crate) fn delimited_by(&self, index: usize) -> Option<GroupId> {
        self.delimits.get(index).copied().flatten()
    }

    /// Group opened by the token at `index`.
    pub(crate) fn opened_at(&self, index: usize) -> Option<GroupId> {
        self.delimited_by(index)
            .filter(|&id| self.groups[id.index()].open == index)
    }

    /// Group closed by the token at `index`.
    pub(crate) fn closed_at(&self, index: usize) -> Option<GroupId> {
        self.delimited_by(index)
            .filter(|&id| self.groups[id.index()].close == Some(index))
    }

    /// The tokens before `index` that `group` holds itself, nearest first:
    /// a nested group is passed over from its closing to its opening.
    pub(crate) fn members_before(
        &self,
        group: Option<GroupId>,
        index: usize,
    ) -> impl Iterator<Item = usize> + '_ {
        let start = group.map_or(0, |id| self.groups[id.index()].open + 1);
        let mut next = index.max(start);
        std::iter::from_fn(move || {
            while next > start {
                next -= 1;
                let at = next;
                if self.enclosing(at) != group {
                    continue;
                }
                if let Some(closed) = self.closed_at(at) {
                    next = self.groups[closed.index()].open + 1;
                }
                return Some(at);
            }
            None
        })
    }

    /// `id` and its enclosing groups, innermost first.
    pub(crate) fn ancestors(&self, id: GroupId) -> impl Iterator<Item = GroupId> + '_ {
        std::iter::successors(Some(id), |&id| self.groups[id.index()].parent)
    }
}

fn track_conditional(
    text: &str,
    stack: &mut Vec<GroupId>,
    conditionals: &mut Vec<ConditionalFrame>,
) {
    let Some(directive) = preprocessor_directive(text) else {
        return;
    };
    if !is_conditional_preprocessor(directive) {
        return;
    }
    match directive {
        "if" | "ifdef" | "ifndef" => conditionals.push(ConditionalFrame {
            stack_at_if: stack.clone(),
            stack_after_first_branch: None,
        }),
        "endif" => {
            if let Some(frame) = conditionals.pop()
                && let Some(first_branch) = frame.stack_after_first_branch
            {
                *stack = first_branch;
            }
        }
        _ => {
            if let Some(frame) = conditionals.last_mut() {
                if frame.stack_after_first_branch.is_none() {
                    frame.stack_after_first_branch = Some(stack.clone());
                }
                stack.clone_from(&frame.stack_at_if);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Delimiter, Groups};
    use crate::formatter::lexer::{Token, tokenize};

    fn index_of(tokens: &[Token], symbol: char, nth: usize) -> usize {
        tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| matches!(token, Token::Symbol(ch) if *ch == symbol))
            .nth(nth)
            .map(|(index, _)| index)
            .expect("symbol present")
    }

    #[test]
    fn matches_nested_groups_and_links_parents() {
        let tokens = tokenize("int f(int a[2]) { g((a)); }\n");
        let groups = Groups::build(&tokens);
        let body = groups.opened_at(index_of(&tokens, '{', 0)).expect("body");
        let call = groups.opened_at(index_of(&tokens, '(', 1)).expect("call");
        let inner = groups.opened_at(index_of(&tokens, '(', 2)).expect("inner");

        assert_eq!(groups.len(), 5);
        assert_eq!(groups.get(body).close, Some(index_of(&tokens, '}', 0)));
        assert_eq!(groups.get(inner).delimiter, Delimiter::Paren);
        assert_eq!(
            groups.ancestors(inner).collect::<Vec<_>>(),
            [inner, call, body]
        );
        assert_eq!(groups.enclosing(index_of(&tokens, '(', 1)), Some(body));
        assert_eq!(groups.closed_at(index_of(&tokens, ')', 1)), Some(inner));
    }

    #[test]
    fn stray_closing_paren_does_not_close_a_block() {
        let tokens = tokenize("void f() { a ) b; }\n");
        let groups = Groups::build(&tokens);
        let body = groups.opened_at(index_of(&tokens, '{', 0)).expect("body");

        assert_eq!(groups.closed_at(index_of(&tokens, ')', 1)), None);
        assert_eq!(groups.get(body).close, Some(index_of(&tokens, '}', 0)));
    }

    #[test]
    fn closing_brace_ends_unclosed_inner_groups() {
        let tokens = tokenize("void f() { g(a; }\nint x;\n");
        let groups = Groups::build(&tokens);
        let body = groups.opened_at(index_of(&tokens, '{', 0)).expect("body");
        let call = groups.opened_at(index_of(&tokens, '(', 1)).expect("call");

        assert_eq!(groups.get(body).close, Some(index_of(&tokens, '}', 0)));
        assert_eq!(groups.get(call).close, None);
        assert_eq!(groups.enclosing(tokens.len() - 2), None);
    }

    #[test]
    fn else_branch_starts_from_the_nesting_at_if() {
        let source = "#if A\nif (x) {\n#else\nif (y) {\n#endif\n    z();\n}\nint w;\n";
        let tokens = tokenize(source);
        let groups = Groups::build(&tokens);
        let first = groups.opened_at(index_of(&tokens, '{', 0)).expect("first");
        let second = groups.opened_at(index_of(&tokens, '{', 1)).expect("second");
        let call = index_of(&tokens, '(', 2);

        assert_eq!(groups.get(second).parent, None);
        assert_eq!(groups.enclosing(call), Some(first));
        assert_eq!(groups.get(first).close, Some(index_of(&tokens, '}', 0)));
        assert_eq!(groups.get(second).close, None);
    }

    #[test]
    fn braces_inside_directives_and_literals_are_not_groups() {
        let tokens = tokenize("#define B {\nchar *s = \"{(\";\nchar c = '{';\n");
        assert_eq!(Groups::build(&tokens).len(), 0);
    }
}
