use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::{BlockKind, next_code_token};
use crate::formatter::structure::groups::GroupId;
use crate::formatter::text::trim::Trimmed;

impl FormatEngine<'_> {
    /// Blocks opened back to back, as `{{{{`, and closed together, as
    /// `}}}}` or `};};};}`, nest one level: the inner blocks hold nothing
    /// but each other, so their braces share the outer block's lines.
    pub(crate) fn fuse_adjacent_braces(&mut self) {
        let groups = &self.tree.groups;
        let mut chains = Vec::new();
        for id in groups.ids() {
            let fused_into_parent = groups
                .get(id)
                .parent
                .and_then(|parent| self.fused_child(parent))
                == Some(id);
            if fused_into_parent {
                continue;
            }
            let mut chain = vec![id];
            while let Some(child) = self.fused_child(*chain.last().expect("chain starts non-empty"))
            {
                chain.push(child);
            }
            if chain.len() > 1 {
                chains.push(chain);
            }
        }
        // Later chains first: lines before them keep their indices.
        for chain in chains.iter().rev() {
            self.fuse_chain(chain);
        }
    }

    /// A statement expression's `{` that a brace style broke off its `(`
    /// rejoins it: `({` is one opener of the expression.
    pub(crate) fn attach_statement_expression_braces(&mut self) {
        let groups = &self.tree.groups;
        let opens: Vec<usize> = groups
            .ids()
            .filter(|&id| self.tree.blocks.kind(id) == Some(BlockKind::StatementExpression))
            .map(|id| groups.get(id).open)
            .collect();
        for &open in opens.iter().rev() {
            let Some(line) = self.output.line_with_token(open) else {
                continue;
            };
            let Some(paren) = self.tree.previous_code_token(open) else {
                continue;
            };
            if line > 0
                && self.output.trimmed(line) == "{"
                && self.output.line_with_token(paren) == Some(line - 1)
                && self.output.code_trimmed(line - 1).ends_with('(')
                && !self.output.is_verbatim(line - 1)
            {
                self.output.join_into(line - 1, line, "");
            }
        }
    }

    /// The only statement of the block `parent` when it is a plain block
    /// opened right after the parent's `{` on the same line and closed
    /// right before the parent's `}`, with nothing but `;` in between.
    fn fused_child(&self, parent: GroupId) -> Option<GroupId> {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let blocks = &self.tree.blocks;
        if !matches!(
            blocks.kind(parent),
            Some(
                BlockKind::FunctionBody
                    | BlockKind::Control
                    | BlockKind::Block
                    | BlockKind::StatementExpression
            )
        ) {
            return None;
        }
        let outer = groups.get(parent);
        let outer_close = outer.close?;
        let open = next_code_token(tokens, outer.open + 1)?;
        if !tokens[outer.open + 1..open]
            .iter()
            .all(|token| matches!(token, Token::Whitespace(_)))
        {
            return None;
        }
        let child = groups.opened_at(open)?;
        if blocks.kind(child) != Some(BlockKind::Block) {
            return None;
        }
        let close = groups.get(child).close?;
        tokens[close + 1..outer_close]
            .iter()
            .all(|token| matches!(token, Token::Whitespace(_) | Token::Symbol(';')))
            .then_some(child)
    }

    fn fuse_chain(&mut self, chain: &[GroupId]) {
        let groups = &self.tree.groups;
        let opens: Vec<usize> = chain.iter().map(|&id| groups.get(id).open).collect();
        let Some(closes) = chain
            .iter()
            .map(|&id| groups.get(id).close)
            .collect::<Option<Vec<usize>>>()
        else {
            return;
        };
        let Some(open_lines) = opens
            .iter()
            .map(|&open| self.output.line_with_token(open))
            .collect::<Option<Vec<usize>>>()
        else {
            return;
        };
        let (Some(first_close_line), Some(last_close_line)) = (
            self.output.line_with_token(closes[closes.len() - 1]),
            self.output.line_with_token(closes[0]),
        ) else {
            return;
        };
        let top = open_lines[0];
        let innermost = open_lines[open_lines.len() - 1];
        // The layout gives each inner brace its own line and each closer
        // its own line; anything else is left alone.
        let inner_opens_alone = open_lines[1..]
            .iter()
            .enumerate()
            .all(|(offset, &line)| line == top + offset + 1 && self.output.trimmed(line) == "{");
        let closers_alone = (first_close_line..=last_close_line).all(|line| {
            let text = self.output.trimmed(line);
            !text.is_empty() && text.chars().all(|ch| matches!(ch, '}' | ';'))
        });
        let tab_width = self.options.tab_width;
        let shift = self
            .output
            .lead_width(innermost, tab_width)
            .saturating_sub(self.output.lead_width(top, tab_width));
        if !inner_opens_alone || !closers_alone || innermost >= first_close_line || shift == 0 {
            return;
        }
        let close_indent = self.output.lead_width(last_close_line, tab_width);
        for _ in first_close_line..last_close_line {
            self.output
                .join_into(first_close_line, first_close_line + 1, "");
        }
        let closers = self.output.trimmed(first_close_line).to_string();
        self.output.set(
            first_close_line,
            format!("{}{closers}", " ".repeat(close_indent)),
        );
        for line in innermost + 1..first_close_line {
            let text = &self.output.as_slice()[line];
            let lead = text.len() - text.trim_start_matches(' ').len();
            if self.output.is_verbatim(line) || text.trimmed().is_empty() || lead < shift {
                continue;
            }
            let text = text[shift..].to_string();
            self.output.set(line, text);
        }
        for pair in opens.windows(2) {
            let adjacent = pair[1] == pair[0] + 1;
            self.output
                .join_into(top, top + 1, if adjacent { "" } else { " " });
        }
    }
}
