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
use crate::formatter::constructs::headers::line_is_control_body_header;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, token_text};
use crate::formatter::structure::blocks::{BlockKind, is_code_token, next_code_token};
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::text::columns::visual_width_from;
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::trim::Trimmed;

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
                // The rest of a comment a directive line opened stays as
                // written, as the directive does.
                let continues_directive_comment =
                    self.comment_opener(index).is_some_and(|opener| {
                        opener != index && self.output.trimmed(opener).starts_with('#')
                    });
                if self.output.is_verbatim(index)
                    || continues_directive_comment
                    || line.is_empty()
                    || !self.options.indent_preproc_define
                        && directive.is_some()
                        && !indented_conditional
                    || self.continues_column_one_comment(index)
                {
                    return None;
                }
                // A comment row whose own whitespace holds a tab keeps it past
                // the indent of the comment's first line, or of the block
                // whose brace the comment follows.
                if let Some(opener) = self.comment_opener(index)
                    && opener != index
                    && line[..line.len() - text.len()].contains('\t')
                {
                    let mut width = self.output.lead_width(opener, tab_width);
                    let follows_block_brace =
                        self.output.code_trimmed_of(&self.output[opener]).trimmed() == "{";
                    if follows_block_brace {
                        width += indent_width;
                    }
                    if line.starts_with(&" ".repeat(width)) {
                        let tab_columns = if follows_block_brace {
                            match self.output_indent_style {
                                IndentStyle::ForceTabs => width / tab_width * tab_width,
                                _ => width / indent_width * indent_width,
                            }
                        } else {
                            self.tab_columns(opener, width)
                        };
                        return Some((width, tab_columns));
                    }
                }
                let width = self.output.lead_width(index, tab_width);
                let tab_columns = match self.output_indent_style {
                    // A standalone comment's rows keep the tabs of its first
                    // line and align past them in spaces.
                    IndentStyle::ForceTabs => {
                        self.comment_opener(index)
                            .filter(|&opener| !self.comment_row_indents_with_tab(opener, index))
                            .and_then(|opener| self.comment_rows_tab_base(opener, index))
                            .map_or(width, |base| base.min(width))
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
        let runs_body_in = matches!(
            self.options.brace_style,
            BraceStyle::Horstmann | BraceStyle::Pico
        );
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
            let line = &self.output[index];
            let text = if line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
                .contains('\t')
                && line.starts_with(&" ".repeat(width))
            {
                &line[width..]
            } else {
                line.trim_start_matches([' ', '\t'])
            };
            let text = match text.strip_prefix('{') {
                // A run-in block brace keeps the style's fill before its
                // text; an initializer row's spaces align its values.
                Some(rest)
                    if rest.starts_with("  ")
                        && !rest.trimmed().is_empty()
                        // A comment keeps the gap it had after a brace but
                        // where the style runs the block's body in.
                        && (runs_body_in && self.starts_with_block_brace(index)
                            || !rest.trimmed_start().starts_with("/*")
                                && !rest.trimmed_start().starts_with("//"))
                        && (self.starts_with_block_brace(index)
                            || self.output.line_tokens(index).is_none()
                            || rest.trimmed_start().starts_with('{')
                            || self.brace_runs_in_to_rows_below(index)) =>
                {
                    let body = rest.trimmed_start();
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

    /// Whether the directive on output line `index` stands among the
    /// arguments of parens or brackets opened above it, past a control
    /// header's condition, whose directives keep structural tabs.
    fn directive_among_arguments(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        let tokens = &self.tree.tokens;
        (index + 1..self.output.len())
            .find_map(|row| self.output.line_tokens(row))
            .and_then(|span| groups.enclosing(span.first))
            .is_some_and(|group| {
                let group = groups.get(group);
                matches!(group.delimiter, Delimiter::Paren | Delimiter::Bracket)
                    && self
                        .output
                        .line_with_token(group.open)
                        .is_some_and(|line| line < index)
                    && !self
                        .tree
                        .previous_code_token(group.open)
                        .is_some_and(|before| {
                            matches!(&tokens[before], Token::Word(word)
                            if matches!(word.as_str(), "if" | "while" | "for" | "switch"))
                        })
            })
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

    /// Whether output line `index` leads with a brace whose elements go on
    /// in rows below it: its fill runs the first element in like a block's.
    fn brace_runs_in_to_rows_below(&self, index: usize) -> bool {
        let groups = &self.tree.groups;
        self.output
            .line_tokens(index)
            .and_then(|span| groups.opened_at(span.first))
            .and_then(|group| groups.get(group).close)
            .is_some_and(|close| self.output.line_with_token(close) != Some(index))
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
        // A row closing a comment before code aligns as the comment's rows.
        let closes_comment = self.continues_comment(index);
        if let Some(span) = self.output.line_tokens(index)
            && !closes_comment
        {
            return self.token_statement_indent_width(span.first, index);
        }
        let text = self.output.trimmed(index);
        // A line the tree does not cover that a brace leads, as one around a
        // statement braces were added to, opens a block.
        if text.starts_with('{') && !closes_comment {
            return None;
        }
        if let Some(opener) = self.comment_opener(index) {
            // A trailing comment's rows align in spaces; a standalone
            // comment's rows keep the tabs of its first line.
            let trailing =
                self.line_holds_code(opener) || !self.output.trimmed(opener).starts_with("/*");
            // The rows of a comment after a block's brace stand in the block.
            if trailing && self.output.code_trimmed_of(&self.output[opener]).trimmed() == "{" {
                return Some(
                    self.output.lead_width(opener, self.options.tab_width)
                        + self.options.indent_width,
                );
            }
            if trailing {
                return Some(0);
            }
            if self.comment_row_indents_with_tab(opener, index) {
                return None;
            }
            let opener_width = self.output.lead_width(opener, self.options.tab_width);
            return Some(self.tab_columns(opener, opener_width));
        }
        if text.starts_with("/*") || text.starts_with("//") {
            // A comment opening a block's body stands at its level.
            let opens_block = (0..index)
                .rev()
                .find(|&previous| self.output.line_tokens(previous).is_some())
                .is_some_and(|previous| {
                    self.output
                        .code_trimmed_of(&self.output[previous])
                        .ends_with('{')
                });
            if opens_block {
                return None;
            }
            // A comment at the row of a list before it stands with that row.
            let tab_width = self.options.tab_width;
            if let Some(previous) = (0..index)
                .rev()
                .find(|&previous| self.output.line_tokens(previous).is_some())
                && self.output.lead_width(previous, tab_width)
                    == self.output.lead_width(index, tab_width)
                && self.output.line_tokens(previous).is_some_and(|span| {
                    let groups = &self.tree.groups;
                    let list = match self.tree.tokens[span.first] {
                        Token::Symbol('}') => groups.closed_at(span.first),
                        _ => groups.enclosing(span.first),
                    };
                    list.is_some_and(|list| {
                        matches!(
                            self.tree.blocks.kind(list),
                            Some(BlockKind::Initializer | BlockKind::CompoundLiteral)
                        )
                    })
                })
            {
                return Some(
                    self.tab_columns(previous, self.output.lead_width(previous, tab_width)),
                );
            }
            // A comment inside a statement continues it.
            let next =
                (index + 1..self.output.len()).find_map(|next| self.output.line_tokens(next))?;
            return self.token_statement_indent_width(next.first, index);
        }
        if text.starts_with('#') {
            // A directive among a statement's arguments aligns as their rows
            // do.
            if !self.directive_among_arguments(index) {
                return None;
            }
            let next =
                (index + 1..self.output.len()).find_map(|next| self.output.line_tokens(next))?;
            return self.token_statement_indent_width(next.first, index);
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
        // Parens the replacement on the directive line leaves open make its
        // rows continue that line's statement.
        let open_on_directive = define_replacement(self.output.code(directive))
            .filter(|text| text.matches('{').count() <= text.matches('}').count())
            .map_or(0, |text| {
                text.matches('(').count() as isize - text.matches(')').count() as isize
            });
        let mut statement = if open_on_directive > 0 {
            directive
        } else {
            directive + 1
        };
        let mut depth = open_on_directive.max(0);
        for row in directive + 1..index {
            let code = self.output.code_before_comment(row).trimmed_end();
            let code = code.strip_suffix('\\').unwrap_or(code).trimmed_end();
            depth += code.matches('(').count() as isize - code.matches(')').count() as isize;
            // An initializer's brace opens rows that align.
            let opens_block = code.strip_suffix('{').is_some_and(|head| {
                head.trimmed_end()
                    .strip_suffix('=')
                    .is_none_or(|before| before.ends_with_any(b"=!<>"))
            });
            // So does a control header for the body after it, once its
            // parens close.
            let opens_body = matches!(code.trimmed_start(), "do" | "else")
                || code.ends_with(')')
                    && line_is_control_body_header(self.row_code(statement.max(directive + 1)));
            if depth <= 0
                && (code.is_empty() || code.ends_with_any(b";}") || opens_block || opens_body)
            {
                statement = row + 1;
            }
        }
        if statement == directive {
            return self.output.lead_width(directive, self.options.tab_width);
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

    /// The code of macro body row `row`, past its indent and before its
    /// continuation backslash.
    fn row_code(&self, row: usize) -> &str {
        let code = self.output.code_before_comment(row).trimmed();
        code.strip_suffix('\\').unwrap_or(code).trimmed()
    }

    /// The column up to which the rows of the block comment opened on line
    /// `opener` indent with tabs: the comment's indent when it stands alone,
    /// the block body's when it runs in after the block's brace.
    fn comment_rows_tab_base(&self, opener: usize, index: usize) -> Option<usize> {
        let tab_width = self.options.tab_width.max(1);
        if self.output.line_tokens(opener).is_none() {
            return self
                .output
                .trimmed(opener)
                .starts_with("/*")
                .then(|| self.output.lead_width(opener, tab_width));
        }
        if !self.row_code(opener).ends_with('{') {
            return None;
        }
        let body = (index + 1..self.output.len())
            .find(|&line| self.comment_opener(line) != Some(opener))?;
        self.output.line_tokens(body)?;
        let lead = self.output.lead_width(body, tab_width);
        (lead > self.output.lead_width(opener, tab_width)).then_some(lead)
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
        if self.options.convert_tabs {
            return false;
        }
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
        // A comment run in after code on its source line opens at a column
        // its rows' whitespace reaches; only a tab past it indents a row.
        let line_start = tokens[..comment]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |newline| newline + 1);
        let run_in = tokens[line_start..comment]
            .iter()
            .any(|token| !matches!(token, Token::Whitespace(_)));
        let tab_width = self.options.tab_width.max(1);
        let opener_column = run_in.then(|| {
            let before: String = tokens[line_start..comment]
                .iter()
                .map(|token| token_text(token))
                .collect();
            visual_width_from(&before, 0, tab_width)
        });
        text.split('\n').nth(index - opener).is_some_and(|row| {
            let whitespace = &row[..row.len() - row.trim_start_matches([' ', '\t']).len()];
            let Some(opener_column) = opener_column else {
                return whitespace
                    .strip_prefix(opener_whitespace)
                    .is_some_and(|relative| relative.contains('\t'));
            };
            let mut column = 0;
            let past_opener = whitespace
                .char_indices()
                .find_map(|(at, ch)| {
                    if column >= opener_column {
                        return Some(at);
                    }
                    column = if ch == '\t' {
                        (column / tab_width + 1) * tab_width
                    } else {
                        column + 1
                    };
                    None
                })
                .map_or("", |at| &whitespace[at..]);
            past_opener.contains('\t')
        })
    }

    /// Whether output line `index` starts inside a block comment opened on
    /// a line before it.
    fn continues_comment(&self, index: usize) -> bool {
        self.output.lead_comment(index).is_some() && self.output.comment_start_index(index) != index
    }

    /// Whether output line `index` holds a token other than a comment.
    fn line_holds_code(&self, index: usize) -> bool {
        self.output.line_tokens(index).is_some_and(|span| {
            self.tree.tokens[span.first..=span.last]
                .iter()
                .any(|token| !matches!(token, Token::Comment(..) | Token::Whitespace(_)))
        })
    }

    /// The line opening the block comment that output line `index`, holding
    /// no code, continues.
    fn comment_opener(&self, index: usize) -> Option<usize> {
        // The row ending a comment holds that comment's token, and the code
        // after it.
        if !self.continues_comment(index) && self.line_holds_code(index) {
            return None;
        }
        let opener = self.output.comment_start_index(index);
        if opener != index {
            // A comment run in after a brace records no lead on its first
            // line; the row that opens it is the one holding its `/*`.
            if !self.output[opener].contains("/*") {
                return self.open_comment_line(opener).or(Some(opener));
            }
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
            // A block whose body runs on from a brace that ends a header,
            // as `enum e { A,`, closes aligned past the header's indent.
            if matches!(self.tree.tokens[first], Token::Symbol('}'))
                && let Some(group) = groups.closed_at(first)
            {
                let open = groups.get(group).open;
                let line = self.output.line_with_token(open)?;
                let tokens = &self.tree.tokens;
                let runs_on = next_code_token(tokens, open + 1).is_some_and(|next| {
                    next != first && self.output.line_with_token(next) == Some(line)
                });
                let leads_line = self
                    .output
                    .line_tokens(line)
                    .is_some_and(|span| next_code_token(tokens, span.first) == Some(open));
                if runs_on && !leads_line {
                    return Some(self.output.lead_width(line, self.options.tab_width));
                }
            }
            return None;
        }
        // Styles that indent braces indent an initializer's brace opening
        // its line, or one closing it, as a level.
        let closes_compound_literal = matches!(self.tree.tokens[first], Token::Symbol('}'))
            && groups.closed_at(first).is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::CompoundLiteral | BlockKind::Initializer)
                )
            });
        // A brace closing a list opened after code continues the statement
        // past its indent, as its elements run on from that brace do,
        // unless the list opens with a nested brace that holds its elements
        // a level in.
        let closes_after_code = closes_compound_literal
            && groups.closed_at(first).is_some_and(|group| {
                let open = groups.get(group).open;
                let next = next_code_token(&self.tree.tokens, open + 1);
                !next.is_some_and(|next| matches!(self.tree.tokens[next], Token::Symbol('{')))
                    && self.output.line_with_token(open).is_some_and(|line| {
                        self.output
                            .line_tokens(line)
                            .is_some_and(|span| span.first != open)
                            && next
                                .is_some_and(|next| self.output.line_with_token(next) == Some(line))
                    })
            });
        if matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
        ) && (matches!(self.tree.tokens[first], Token::Symbol('{'))
            || closes_compound_literal && !closes_after_code)
        {
            return None;
        }
        // A macro call after a control header, as `if (a) FAIL(x)`, is the
        // header's body: the line after it stands at a level.
        if self.follows_header_macro_call(first) {
            return None;
        }
        // A row a closing paren leads continues the statement its paren
        // opened in.
        let from = match self.tree.tokens[first] {
            Token::Symbol(')' | ']') => groups
                .closed_at(first)
                .map_or(first, |group| groups.get(group).open),
            _ => first,
        };
        let start = self.statement_start(from);
        let line = self.output.line_with_token(start)?;
        // A case label's rows take levels.
        if line >= index || matches!(&self.tree.tokens[start], Token::Word(word) if word == "case")
        {
            return None;
        }
        // An initializer list that opens with a nested brace, as
        // `{ { 1, 2 },`, holds its elements a level in.
        if matches!(self.tree.tokens[start], Token::Symbol('{'))
            && groups.enclosing(first) == groups.enclosing(start)
            && self.tree.previous_code_token(start).is_some_and(|open| {
                matches!(self.tree.tokens[open], Token::Symbol('{'))
                    && groups.opened_at(open).is_some_and(|group| {
                        self.tree.blocks.kind(group) == Some(BlockKind::Initializer)
                    })
            })
        {
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
        // A statement on its braceless header's line, as `if (a) f(x,`,
        // stands at the header's body level.
        let opens_header = matches!(&tokens[start], Token::Word(word)
            if matches!(word.as_str(), "if" | "for" | "while" | "switch" | "do" | "foreach"));
        let after_header = !opens_header
            && self.tree.previous_code_token(start).is_some_and(|previous| {
            self.output.line_with_token(previous) == Some(line)
                && (self.closes_control_condition(previous)
                    || matches!(&tokens[previous], Token::Word(word) if matches!(word.as_str(), "else" | "do")))
        });
        // A declaration a `struct` or `union` leads, its first line ending
        // in a name or `*`, continues as the block that keyword would open.
        let ends_in_name = self.output.line_tokens(line).is_some_and(|span| {
            (span.first..=span.last)
                .rev()
                .find(|&token| is_code_token(&tokens[token]))
                .is_some_and(|token| match &tokens[token] {
                    Token::Word(_) | Token::Symbol('*') => true,
                    Token::Operator(op) => op == "*",
                    _ => false,
                })
        });
        let record_led = ends_in_name
            && groups.enclosing(first) == groups.enclosing(start)
            && tokens[start..]
            .iter()
            .filter(|token| {
                !matches!(token, Token::Whitespace(_) | Token::Newline | Token::Comment(..))
            })
            .find(|token| {
                !matches!(token, Token::Word(word) if matches!(
                    word.as_str(),
                    "static" | "extern" | "const" | "volatile" | "typedef" | "register"
                ))
            })
            .is_some_and(|token| matches!(token, Token::Word(word) if matches!(word.as_str(), "struct" | "union")));
        // A class head's continued rows, its base list, stand a level in.
        let class_head = matches!(&tokens[start], Token::Word(word)
            if matches!(word.as_str(), "class" | "struct" | "union"))
            && (first..tokens.len())
                .filter(|&token| is_code_token(&tokens[token]))
                .take_while(|&token| !matches!(tokens[token], Token::Symbol(';')))
                .find(|&token| matches!(tokens[token], Token::Symbol('{')))
                .and_then(|brace| groups.opened_at(brace))
                .is_some_and(|body| {
                    groups.get(body).parent == groups.enclosing(first)
                        && self.tree.blocks.kind(body) == Some(BlockKind::Aggregate)
                });
        Some(
            lead + usize::from(after_head_colon || after_header || record_led || class_head)
                * self.options.indent_width,
        )
    }

    /// For a code line the tree does not cover, such as a row of a macro
    /// body, the statement starts on the first line after one that ends
    /// with `;`, `{`, or `}`, or after the directive.
    fn untracked_statement_indent_width(&self, index: usize) -> Option<usize> {
        let ends_statement = |line: usize| {
            let code = self.output.code_trimmed(line);
            let code = code.strip_suffix('\\').unwrap_or(code).trimmed_end();
            code.is_empty()
                || code.starts_with('#')
                || code.ends_with_any(b";{}")
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
            // A later branch of a conditional continues the code before its
            // `#if`, and the code after `#endif` its first branch, whatever
            // statement the tokens in order start there.
            let previous = self.tree.previous_code_token_in_first_branches(start);
            let in_order = previous == self.tree.previous_code_token(start);
            if in_order && statements.starts_block_statement(start)
                || statements.braceless_header(start).is_some()
                || initializer_colon(start)
            {
                return start;
            }
            let Some(previous) = previous else {
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
                // An Objective-C directive such as `@property` starts anew,
                // and so does what follows an `@end`.
                || matches!(tokens[start], Token::Symbol('@'))
                || matches!(&tokens[previous], Token::Word(word) if word == "end")
                    && self
                        .tree
                        .previous_code_token(previous)
                        .is_some_and(|at| matches!(tokens[at], Token::Symbol('@')))
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
    pub(crate) fn ends_access_label(&self, index: usize) -> bool {
        let access = |label: usize| {
            matches!(&self.tree.tokens[label], Token::Word(word)
                    if matches!(word.as_str(), "public" | "protected" | "private")
                        || self.options.access_labels.iter().any(|access| access == word))
        };
        matches!(self.tree.tokens[index], Token::Symbol(':'))
            && self.tree.previous_code_token(index).is_some_and(|label| {
                access(label)
                    // Qt's `public slots:`
                    || matches!(&self.tree.tokens[label], Token::Word(word) if word == "slots")
                        && self.tree.previous_code_token(label).is_some_and(access)
            })
    }
}

/// Whether `code` is a `case`, `default`, or plain label ending its line.
fn ends_label(code: &str) -> bool {
    let Some(label) = code.strip_suffix(':') else {
        return false;
    };
    let label = label.trimmed();
    label.starts_with("case ")
        || label == "default"
        || !label.is_empty() && label.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// The replacement text on a `#define` line: what follows the macro's name
/// and parameters.
fn define_replacement(line: &str) -> Option<&str> {
    let rest = line.trimmed_start().strip_prefix('#')?.trimmed_start();
    let rest = rest.strip_prefix("define")?;
    let rest = rest.trimmed_start();
    let name_end = rest
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(rest.len());
    let rest = &rest[name_end..];
    if let Some(parameters) = rest.strip_prefix('(') {
        let close = parameters.find(')')?;
        return Some(&parameters[close + 1..]);
    }
    Some(rest)
}
