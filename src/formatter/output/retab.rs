//! Tab indentation for finished output.
//!
//! The engine lays out tab-indented styles in spaces, so a line's bytes are
//! its columns for every rule that measures earlier lines. Once the output
//! is complete, each line's indent turns into tabs the way astyle writes
//! them: with `--indent=tab`, a statement's own indent is tabs and a
//! continuation line aligns past it with spaces; with `--indent=force-tab`
//! all of it is tabs.

use crate::config::{BraceStyle, IndentStyle};
use crate::formatter::braces::postprocess::horstmann_run_in_fill;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::text::line_scan::trailing_comment_split_limit;

impl FormatEngine<'_> {
    pub(crate) fn retab_output(&mut self) {
        if self.output_indent_style == self.options.indent_style {
            return;
        }
        let mut output_options = self.options.clone();
        output_options.indent_style = self.output_indent_style;
        let tab_width = self.options.tab_width.max(1);
        let indent_width = self.options.indent_width.max(1);
        // Widths come from the untouched spaced lines: once a line holds
        // tabs, its bytes stop being columns.
        let rows: Vec<Option<(usize, usize)>> = (0..self.output.len())
            .map(|index| {
                let line = &self.output[index];
                let text = line.trim_start_matches([' ', '\t']);
                let directive = self.output.directive_of_continuation(index);
                // Indenting conditional directives indents their continued
                // lines too.
                let indented_conditional =
                    directive.is_some() && self.output.is_indented_directive_continuation(index);
                if self.output.is_verbatim(index)
                    || line.is_empty()
                    || !self.options.indent_preproc_define
                        && directive.is_some()
                        && !indented_conditional
                    || self.continues_column_one_comment(index)
                {
                    return None;
                }
                let width = self.output.lead_width(index, tab_width);
                let tab_columns = match self.output_indent_style {
                    // A standalone comment's rows keep the tabs of its first
                    // line and align past them in spaces.
                    IndentStyle::ForceTabs => {
                        self.comment_opener(index)
                            .filter(|&opener| {
                                self.output.line_tokens(opener).is_none()
                                    && self.output.trimmed(opener).starts_with("/*")
                                    && !self.comment_row_indents_with_tab(opener, index)
                            })
                            .map_or(width, |opener| {
                                self.output.lead_width(opener, tab_width).min(width)
                            })
                            / tab_width
                            * tab_width
                    }
                    _ if indented_conditional => width / indent_width * indent_width,
                    // A filled empty line holds its level's indent.
                    _ if text.is_empty() => width / indent_width * indent_width,
                    _ => self.tab_columns(index, width),
                };
                Some((width, tab_columns))
            })
            .collect();
        for (index, row) in rows.into_iter().enumerate() {
            let Some((width, tab_columns)) = row else {
                continue;
            };
            let tab_size = match self.output_indent_style {
                IndentStyle::ForceTabs => tab_width,
                _ => indent_width,
            };
            let prefix = format!(
                "{}{}",
                "\t".repeat(tab_columns / tab_size),
                " ".repeat(width - tab_columns)
            );
            let text = self.output[index].trim_start_matches([' ', '\t']);
            let text = match text.strip_prefix('{') {
                // A run-in block brace keeps the style's fill before its
                // text; an initializer row's spaces align its values.
                Some(rest)
                    if rest.starts_with("  ")
                        && !rest.trim().is_empty()
                        && (self.starts_with_block_brace(index)
                            || self.output.line_tokens(index).is_none()) =>
                {
                    let body = rest.trim_start();
                    let target = width + 1 + (rest.len() - body.len());
                    let fill = horstmann_run_in_fill(
                        &format!("{prefix}{{"),
                        &format!("{}{body}", " ".repeat(target)),
                        &output_options,
                    );
                    format!("{{{fill}{body}")
                }
                _ => text.to_string(),
            };
            self.output.set(index, format!("{prefix}{text}"));
        }
    }

    fn starts_with_block_brace(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        self.output
            .line_tokens(index)
            .and_then(|span| groups.opened_at(span.first))
            .is_some_and(|group| {
                !matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                )
            })
    }

    /// Columns of output line `index`'s indent, `width` wide, that tabs
    /// cover: all of a statement's own indent, and of a continuation, the
    /// indent of the line starting its statement.
    fn tab_columns(&self, index: usize, width: usize) -> usize {
        let indent_width = self.options.indent_width.max(1);
        let base = self
            .statement_indent_width(index)
            .map_or(width, |base| base.min(width));
        base / indent_width * indent_width
    }

    /// Indent width that tabs cover on output line `index`: that of the line
    /// where the statement it continues starts; `None` when the line starts
    /// a statement or stands at its own level.
    fn statement_indent_width(&self, index: usize) -> Option<usize> {
        if let Some(span) = self.output.line_tokens(index) {
            return self.token_statement_indent_width(span.first, index);
        }
        let text = self.output.trimmed(index);
        // A line the tree does not cover that a brace leads, as one around a
        // statement braces were added to, opens a block.
        if text.starts_with('{') {
            return None;
        }
        if let Some(opener) = self.comment_opener(index) {
            // A trailing comment's rows align in spaces; a standalone
            // comment's rows keep the tabs of its first line.
            let trailing = self.output.line_tokens(opener).is_some()
                || !self.output.trimmed(opener).starts_with("/*");
            if trailing {
                return Some(0);
            }
            let opener_width = self.output.lead_width(opener, self.options.tab_width);
            return Some(self.tab_columns(opener, opener_width));
        }
        if text.starts_with("/*") || text.starts_with("//") {
            // A comment inside a statement continues it.
            let next =
                (index + 1..self.output.len()).find_map(|next| self.output.line_tokens(next))?;
            return self.token_statement_indent_width(next.first, index);
        }
        if text.starts_with('#') {
            return None;
        }
        if let Some(directive) = self.output.directive_of_continuation(index) {
            return Some(self.macro_row_statement_width(directive, index));
        }
        self.untracked_statement_indent_width(index)
    }

    /// Indent width of the statement that row `index` of the macro body
    /// opened on line `directive` belongs to: a statement row indents in
    /// tabs, and the rows continuing it align past those in spaces.
    fn macro_row_statement_width(&self, directive: usize, index: usize) -> usize {
        let mut statement = directive + 1;
        let mut depth = 0isize;
        for row in directive + 1..index {
            let line = &self.output[row];
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let code = code.strip_suffix('\\').unwrap_or(code).trim_end();
            depth += code.matches('(').count() as isize - code.matches(')').count() as isize;
            // An initializer's brace opens rows that align.
            let opens_block = code.strip_suffix('{').is_some_and(|head| {
                head.trim_end()
                    .strip_suffix('=')
                    .is_none_or(|before| before.ends_with(['=', '!', '<', '>']))
            });
            if depth <= 0 && (code.is_empty() || code.ends_with([';', '}']) || opens_block) {
                statement = row + 1;
            }
        }
        let width = self.output.lead_width(statement, self.options.tab_width);
        let directive_code = self.output.code(directive);
        let directive_opens_block =
            directive_code.matches('{').count() > directive_code.matches('}').count();
        if statement == directive + 1 && !directive_opens_block {
            // The body starts a level past its directive.
            width.min(
                self.output.lead_width(directive, self.options.tab_width)
                    + self.options.indent_width,
            )
        } else {
            width
        }
    }

    /// Whether output line `index` continues a standalone comment that
    /// starts in column one: astyle keeps such a comment as written.
    fn continues_column_one_comment(&self, index: usize) -> bool {
        self.comment_opener(index).is_some_and(|opener| {
            self.output.line_tokens(opener).is_none() && self.output[opener].starts_with("/*")
        })
    }

    /// Whether the source row of the block comment opened on line `opener`
    /// that output line `index` holds indents past the comment with a tab.
    fn comment_row_indents_with_tab(&self, opener: usize, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        let Some(comment) = self.output.comment_token(opener) else {
            return false;
        };
        let Token::Comment(_, text) = &tokens[comment] else {
            return false;
        };
        let opener_whitespace = match comment.checked_sub(1).map(|before| &tokens[before]) {
            Some(Token::Whitespace(whitespace)) => whitespace.as_str(),
            _ => "",
        };
        text.split('\n').nth(index - opener).is_some_and(|row| {
            let whitespace = &row[..row.len() - row.trim_start_matches([' ', '\t']).len()];
            whitespace
                .strip_prefix(opener_whitespace)
                .is_some_and(|relative| relative.contains('\t'))
        })
    }

    /// The line opening the block comment that output line `index`, holding
    /// no code, continues.
    fn comment_opener(&self, index: usize) -> Option<usize> {
        if self.output.line_tokens(index).is_some() {
            return None;
        }
        let opener = self.output.comment_start_index(index);
        if opener != index {
            return Some(opener);
        }
        self.open_comment_line(index)
    }

    /// The line whose `/*` opens a block comment still open at the start of
    /// output line `index`.
    fn open_comment_line(&self, index: usize) -> Option<usize> {
        for line in (0..index).rev() {
            // A code line's own text, such as a string, opens no comment.
            let text = if self.output.line_tokens(line).is_some() {
                &self.output[line][self.output.code(line).len()..]
            } else {
                self.output[line].as_str()
            };
            if let Some(close) = text.rfind("*/") {
                return text[close..].contains("/*").then_some(line);
            }
            if text.contains("/*") {
                return Some(line);
            }
            if self.output.line_tokens(line).is_some() {
                return None;
            }
        }
        None
    }

    fn follows_header_macro_call(&self, first: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let Some(close) = self.tree.previous_code_token(first) else {
            return false;
        };
        if !matches!(tokens[close], Token::Symbol(')')) {
            return false;
        }
        let Some(call) = groups.closed_at(close) else {
            return false;
        };
        let Some(name) = self.tree.previous_code_token(groups.get(call).open) else {
            return false;
        };
        if !matches!(tokens[name], Token::Word(_)) {
            return false;
        }
        let Some(condition_close) = self.tree.previous_code_token(name) else {
            return false;
        };
        matches!(tokens[condition_close], Token::Symbol(')'))
            && groups.closed_at(condition_close).is_some_and(|condition| {
                self.tree
                    .previous_code_token(groups.get(condition).open)
                    .is_some_and(|keyword| {
                        matches!(&tokens[keyword], Token::Word(word)
                            if matches!(word.as_str(), "if" | "while" | "for"))
                    })
            })
    }

    fn token_statement_indent_width(&self, first: usize, index: usize) -> Option<usize> {
        // A block's braces stand at a level; an initializer's continue their
        // statement.
        let groups = &self.tree.groups;
        let block_brace = groups.delimited_by(first).is_some_and(|group| {
            !matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
            )
        });
        if matches!(self.tree.tokens[first], Token::Symbol('{' | '}')) && block_brace {
            return None;
        }
        // Styles that indent braces indent an initializer's brace opening
        // its line, or a compound literal's closing it, as a level.
        let closes_compound_literal = matches!(self.tree.tokens[first], Token::Symbol('}'))
            && groups.closed_at(first).is_some_and(|group| {
                self.tree.blocks.kind(group) == Some(BlockKind::CompoundLiteral)
            });
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
        ) && (matches!(self.tree.tokens[first], Token::Symbol('{')) || closes_compound_literal)
        {
            return None;
        }
        // A macro call after a control header, as `if (a) FAIL(x)`, is the
        // header's body: the line after it stands at a level.
        if self.follows_header_macro_call(first) {
            return None;
        }
        let start = self.statement_start(first);
        let line = self.output.line_with_token(start)?;
        if line >= index {
            return None;
        }
        let lead = self.output.lead_width(line, self.options.tab_width);
        // An initializer list that starts on its constructor's line still
        // stands one level in.
        let tokens = &self.tree.tokens;
        let after_head_colon = self.tree.previous_code_token(start).is_some_and(|colon| {
            matches!(tokens[colon], Token::Symbol(':'))
                && self.tree.previous_code_token(colon).is_some_and(|close| {
                    matches!(tokens[close], Token::Symbol(')'))
                        && self.output.line_with_token(close) == Some(line)
                })
        });
        Some(lead + usize::from(after_head_colon) * self.options.indent_width)
    }

    /// For a code line the tree does not cover, such as a row of a macro
    /// body, the statement starts on the first line after one that ends
    /// with `;`, `{`, or `}`, or after the directive.
    fn untracked_statement_indent_width(&self, index: usize) -> Option<usize> {
        let ends_statement = |line: usize| {
            let code = self.output.code_trimmed(line);
            let code = code.strip_suffix('\\').unwrap_or(code).trim_end();
            code.is_empty()
                || code.starts_with('#')
                || code.ends_with([';', '{', '}'])
                || ends_label(code)
        };
        let mut start = index;
        while start > 0 && !ends_statement(start - 1) {
            start -= 1;
        }
        (start < index).then(|| self.output.lead_width(start, self.options.tab_width))
    }

    /// First token of the statement that holds the token `index`: the walk
    /// leaves parentheses and brackets, then goes back past nested groups to
    /// a statement start the tree knows or to the token after a `;`, `{`,
    /// or `}` of the same level.
    fn statement_start(&self, index: usize) -> usize {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let statements = &self.tree.statements;
        let mut start = index;
        let mut level = groups.enclosing(index);
        while let Some(group) = level
            && groups.get(group).delimiter != Delimiter::Brace
        {
            start = groups.get(group).open;
            level = groups.get(group).parent;
        }
        // Outside statement blocks, a constructor's initializer list stands
        // at its own level.
        let statement_scope = level.is_some_and(|group| {
            matches!(
                self.tree.blocks.kind(group),
                Some(BlockKind::FunctionBody | BlockKind::Control | BlockKind::Block)
            )
        });
        let initializer_colon = |index: usize| {
            !statement_scope
                && matches!(tokens[index], Token::Symbol(':'))
                && self
                    .tree
                    .previous_code_token(index)
                    .is_some_and(|previous| matches!(tokens[previous], Token::Symbol(')')))
        };
        loop {
            if statements.starts_block_statement(start)
                || statements.braceless_header(start).is_some()
                || initializer_colon(start)
            {
                return start;
            }
            let Some(previous) = self.tree.previous_code_token(start) else {
                return start;
            };
            let closes_expression_brace = groups.closed_at(previous).is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::Initializer | BlockKind::CompoundLiteral | BlockKind::Lambda)
                )
            });
            if matches!(tokens[previous], Token::Symbol(';' | '{'))
                || matches!(tokens[previous], Token::Symbol('}')) && !closes_expression_brace
                || initializer_colon(previous)
                || self.ends_access_label(previous)
                || self.ends_case_label(previous)
                || self.ends_user_label(previous)
                // An Objective-C directive such as `@property` starts anew.
                || matches!(tokens[start], Token::Symbol('@'))
                || matches!(&tokens[previous], Token::Word(word) if matches!(word.as_str(), "else" | "do"))
                || self.closes_control_condition(previous)
            {
                return start;
            }
            start = groups
                .closed_at(previous)
                .map_or(previous, |group| groups.get(group).open);
        }
    }

    /// Whether the token `index` is the `)` of an `if`, `for`, `while`, or
    /// `switch` condition.
    fn closes_control_condition(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        groups.closed_at(index).is_some_and(|group| {
            self.tree
                .previous_code_token(groups.get(group).open)
                .is_some_and(|keyword| {
                    matches!(&self.tree.tokens[keyword], Token::Word(word)
                        if matches!(word.as_str(), "if" | "for" | "while" | "switch" | "foreach"))
                })
        })
    }

    /// Whether the token `index` is the `:` of an access label.
    fn ends_access_label(&self, index: usize) -> bool {
        matches!(self.tree.tokens[index], Token::Symbol(':'))
            && self.tree.previous_code_token(index).is_some_and(|label| {
                matches!(&self.tree.tokens[label], Token::Word(word)
                        if matches!(word.as_str(), "public" | "protected" | "private")
                            || self.options.access_labels.iter().any(|access| access == word))
            })
    }
}

/// Whether `code` is a `case`, `default`, or plain label ending its line.
fn ends_label(code: &str) -> bool {
    let Some(label) = code.strip_suffix(':') else {
        return false;
    };
    let label = label.trim();
    label.starts_with("case ")
        || label == "default"
        || !label.is_empty() && label.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}
