//! Preprocessor directives, macro bodies, and backslash-continued lines.

use crate::formatter::text::line_scan::has_hash_outside_literals;

use crate::formatter::braces::classification::ExternCGuard;
use crate::formatter::braces::initializers::InlineArrayState;
use crate::formatter::constructs::headers::HeaderParenState;
use crate::formatter::engine::{FormatEngine, LayoutState};
use crate::formatter::lexer::Token;
use crate::formatter::state::frame::{BraceSemanticKind, ParenRole};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::blocks::next_code_token;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::{line_comment_split_limit, preprocessor_directive};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start, trailing_word};
use std::collections::VecDeque;

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct PreprocessorState {
    /// Token of the directive being pushed.
    pub(crate) active_directive: Option<usize>,
    pub(crate) branch_stack: Vec<PreprocessorBranchState>,
    pub(crate) indented_block_stack: Vec<bool>,
    /// The statement continuation each open conditional set aside, which
    /// its `#endif` resumes.
    pub(crate) suspended_continuations: Vec<Option<usize>>,
    /// Whether each open conditional is a file scope block indenting the
    /// lines of a paren group it opened in.
    pub(crate) group_blocks: Vec<bool>,
    /// Output lines such blocks indented.
    pub(crate) group_block_rows: Vec<usize>,
    pub(crate) indentable_blocks: VecDeque<bool>,
    pub(crate) split_else: PreprocessorSplitElseState,
    pub(crate) may_have_preprocessor: bool,
    pub(crate) last_output_was_preprocessor: bool,
}

pub(crate) mod backslash_bodies;
pub(crate) mod layout;
mod macro_definitions;
pub(crate) mod macro_invocations;

pub(crate) fn indent_off_follows_code(tokens: &[Token]) -> bool {
    let mut seen_code = false;
    for token in tokens {
        match token {
            Token::Whitespace(_) | Token::Newline => {}
            Token::Comment(_, comment) if comment.contains("*INDENT-OFF*") => return seen_code,
            Token::Comment(_, _) => {}
            _ => seen_code = true,
        }
    }
    false
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub(crate) struct PreprocessorSplitElseState {
    pub(crate) extra_indent: bool,
    pub(crate) extra_levels: usize,
    pub(crate) trigger_output_len: Option<usize>,
    pub(crate) pending_body: bool,
    pub(crate) clear_pending_after_brace: bool,
    pub(crate) closing_brace_has_else: bool,
    pub(crate) comment_body_indent_spaces: Option<usize>,
    pub(crate) body_braceless: bool,
    pub(crate) brace_indent: usize,
    pub(crate) after_line: bool,
}

impl PreprocessorSplitElseState {
    fn is_active(self) -> bool {
        self.extra_indent
            || self.pending_body
            || self.clear_pending_after_brace
            || self.extra_levels > 0
    }

    pub(crate) fn reset(&mut self) {
        let brace_indent = self.brace_indent;
        *self = Self {
            brace_indent,
            ..Self::default()
        };
    }

    fn after_branch_restore(mut self, active: Self) -> Self {
        if active.extra_levels == 0 && !active.pending_body {
            self.reset();
            return self;
        }
        if active.extra_levels > self.extra_levels {
            self.extra_indent = true;
            self.extra_levels = active.extra_levels;
            self.trigger_output_len = active.trigger_output_len;
            self.body_braceless = active.body_braceless;
            self.brace_indent = active.brace_indent;
        }
        self.pending_body |= active.pending_body;
        self.clear_pending_after_brace |= active.clear_pending_after_brace;
        self.closing_brace_has_else |= active.closing_brace_has_else;
        if self.comment_body_indent_spaces.is_none() {
            self.comment_body_indent_spaces = active.comment_body_indent_spaces;
        }
        self
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct PreprocessorBranchState {
    pub(crate) layout: LayoutState,
    pub(crate) first_body_indent_spaces: Option<usize>,
    pub(crate) restore_body_indent: bool,
    preprocessor_split_else: PreprocessorSplitElseState,
    pub(crate) header_paren: HeaderParenState,
    pub(crate) inline_array: InlineArrayState,
    pub(crate) pending_extern: bool,
    pub(crate) extern_c_guard: ExternCGuard,
    /// The state at the end of the first branch, which astyle continues
    /// from after `#endif`.
    first_branch_end: Option<Box<PreprocessorBranchState>>,
    /// The token index of the directive that opened the conditional.
    directive: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum PreprocessorLineIndent {
    Level(usize),
    Exact {
        structural_level: usize,
        spaces: usize,
    },
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PreprocessorRegion {
    TopLevel,
    Namespace,
    Block,
    FieldDeclarationList,
    EnumList,
    MacroBody,
    Unknown,
}

/// Per-directive facts that each physical line of a preprocessor directive uses.
#[derive(Clone, Copy)]
struct PreprocessorLineParts<'a> {
    opaque_literal_line_ranges: &'a [(usize, usize)],
    branch_separator_after_else: bool,
    indent_continued_conditional: bool,
    /// The directive is a conditional that indenting preprocessor blocks
    /// indents: its continued lines stand a level past it.
    indent_continued_block_conditional: bool,
    /// astyle reads `#error`, `#warning`, and `#line` as comments and
    /// starts each of their continued lines where the directive starts.
    comment_directive: bool,
}

impl FormatEngine<'_> {
    pub(crate) fn preprocessor_region(&self, in_macro_body: bool) -> PreprocessorRegion {
        let region = Self::preprocessor_region_from_brace_stack(
            &self.layout.nesting.brace_type_stack,
            in_macro_body,
        );
        if region == PreprocessorRegion::TopLevel
            && self
                .layout
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.semantic_kind == BraceSemanticKind::Namespace)
        {
            PreprocessorRegion::Namespace
        } else {
            region
        }
    }

    fn preprocessor_region_from_brace_stack(
        brace_type_stack: &[BraceType],
        in_macro_body: bool,
    ) -> PreprocessorRegion {
        if in_macro_body {
            return PreprocessorRegion::MacroBody;
        }
        match brace_type_stack.last().copied() {
            None => PreprocessorRegion::TopLevel,
            Some(BraceType::Namespace) => PreprocessorRegion::Namespace,
            Some(BraceType::Command | BraceType::Definition) => PreprocessorRegion::Block,
            Some(
                BraceType::Class | BraceType::Interface | BraceType::Struct | BraceType::Union,
            ) => PreprocessorRegion::FieldDeclarationList,
            Some(BraceType::Enum) => PreprocessorRegion::EnumList,
            Some(_) => PreprocessorRegion::Unknown,
        }
    }

    fn preprocessor_region_allows_block_indent(&self, region: PreprocessorRegion) -> bool {
        match region {
            PreprocessorRegion::TopLevel => self.layout.indentation.indent() == 0,
            PreprocessorRegion::Namespace => !self.options.indent_namespaces,
            _ => false,
        }
    }
}

pub(crate) fn output_has_active_preprocessor_branch(output: &[String]) -> bool {
    output
        .iter()
        .rev()
        .filter(|line| !line.trimmed().is_empty())
        .find_map(|line| {
            let trimmed = line.trimmed_start();
            if trimmed.starts_with("#endif") {
                return Some(false);
            }
            (trimmed.starts_with("#if")
                || trimmed.starts_with("#else")
                || trimmed.starts_with("#elif"))
            .then_some(true)
        })
        .unwrap_or(false)
}

fn is_ndef_preprocessor_statement(line: &str, directive: &str) -> bool {
    match directive {
        "ifndef" => true,
        "if" => preprocessor_condition(line).is_some_and(is_not_defined_condition),
        _ => false,
    }
}

pub(crate) fn preprocessor_block_indentability(tokens: &[Token]) -> VecDeque<bool> {
    // Each conditional, with the `#endif` that closes it.
    let mut conditionals: Vec<(usize, Option<usize>)> = Vec::new();
    let mut open_stack: Vec<usize> = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let Token::Preprocessor(preprocessor) = token else {
            continue;
        };
        match preprocessor_directive(&preprocessor.text) {
            Some("if" | "ifdef" | "ifndef") => {
                open_stack.push(conditionals.len());
                conditionals.push((index, None));
            }
            Some("endif") => {
                if let Some(open) = open_stack.pop() {
                    conditionals[open].1 = Some(index);
                }
            }
            _ => {}
        }
    }
    let breakers = (conditionals.len() > 1).then(|| BlockBreakers::new(tokens));
    let mut result = VecDeque::new();
    for (position, &(index, endif)) in conditionals.iter().enumerate() {
        result.push_back(endif.is_some_and(|endif| match &breakers {
            Some(breakers) if position > 0 => breakers.indentable(index, endif),
            _ => is_indentable_preprocessor_block(tokens, index, position == 0),
        }));
    }
    result
}

/// Where the tokens stand that keep a conditional block from indenting, so
/// that a block after the file's first is judged without reading it the
/// way [`is_indentable_preprocessor_block`] does.
struct BlockBreakers {
    /// `#if` and `#elif` lines whose continued lines leave a paren open.
    unbalanced_conditions: Vec<usize>,
    /// `{`, `}` and `:`.
    braces_and_colons: Vec<usize>,
    /// `#define`s continued past their line.
    continued_defines: Vec<usize>,
    /// `endif` words with a `#` after them on their line.
    endif_words: Vec<usize>,
    /// Parens, with the count of those open after each.
    parens: Vec<(usize, isize)>,
    /// Newlines, with the count of parens open at each and the next newline
    /// at which the count differs.
    newlines: Vec<(usize, isize, usize)>,
}

impl BlockBreakers {
    fn new(tokens: &[Token]) -> Self {
        let mut breakers = Self {
            unbalanced_conditions: Vec::new(),
            braces_and_colons: Vec::new(),
            continued_defines: Vec::new(),
            endif_words: Vec::new(),
            parens: Vec::new(),
            newlines: Vec::new(),
        };
        let mut open_parens = 0isize;
        let mut line_endif_words = Vec::new();
        for (index, token) in tokens.iter().enumerate() {
            match token {
                Token::Preprocessor(preprocessor) => {
                    line_endif_words.clear();
                    let line = &preprocessor.text;
                    match preprocessor_directive(line) {
                        Some("if" | "elif")
                            if line.lines().nth(1).is_some()
                                && line.lines().any(|part| {
                                    part.matches('(').count() != part.matches(')').count()
                                }) =>
                        {
                            breakers.unbalanced_conditions.push(index);
                        }
                        Some("define") if define_continues(line) => {
                            breakers.continued_defines.push(index);
                        }
                        _ => {}
                    }
                }
                Token::Word(word) if word == "endif" => line_endif_words.push(index),
                Token::Symbol('#') => breakers.endif_words.append(&mut line_endif_words),
                Token::Symbol('{' | '}' | ':') => breakers.braces_and_colons.push(index),
                Token::Symbol(paren @ ('(' | ')')) => {
                    open_parens += if *paren == '(' { 1 } else { -1 };
                    breakers.parens.push((index, open_parens));
                }
                Token::Newline => {
                    line_endif_words.clear();
                    breakers.newlines.push((index, open_parens, 0));
                }
                _ => {}
            }
        }
        let mut change = breakers.newlines.len();
        for at in (0..breakers.newlines.len()).rev() {
            if breakers
                .newlines
                .get(at + 1)
                .is_some_and(|next| next.1 != breakers.newlines[at].1)
            {
                change = at + 1;
            }
            breakers.newlines[at].2 = change;
        }
        breakers
    }

    /// The first of `indices` after `start`.
    fn first_after(indices: &[usize], start: usize) -> Option<usize> {
        indices
            .get(indices.partition_point(|&index| index <= start))
            .copied()
    }

    /// The count of parens open before the token `index`.
    fn open_parens_before(&self, index: usize) -> isize {
        let before = self.parens.partition_point(|&(at, _)| at < index);
        before.checked_sub(1).map_or(0, |last| self.parens[last].1)
    }

    /// `is_indentable_preprocessor_block` of the block from the
    /// conditional at `start` to its `#endif` at `endif`, not the file's
    /// first.
    fn indentable(&self, start: usize, endif: usize) -> bool {
        let inside = |index: Option<usize>| index.is_some_and(|index| index < endif);
        if inside(
            self.unbalanced_conditions
                .get(
                    self.unbalanced_conditions
                        .partition_point(|&index| index < start),
                )
                .copied(),
        ) || inside(Self::first_after(&self.braces_and_colons, start))
        {
            return false;
        }
        // A continued define breaks the block unless an `endif` word with a
        // `#` after it comes first.
        if let Some(define) = Self::first_after(&self.continued_defines, start)
            && define < endif
            && Self::first_after(&self.endif_words, start).is_none_or(|word| word > define)
        {
            return false;
        }
        // Every newline in the block, and its end, sees the parens open
        // that were open at its start.
        let open = self.open_parens_before(start);
        let first = self.newlines.partition_point(|&(at, _, _)| at <= start);
        let differing = match self.newlines.get(first) {
            Some(&(_, count, change)) if count == open => change,
            _ => first,
        };
        !inside(self.newlines.get(differing).map(|&(at, _, _)| at))
            && self.open_parens_before(endif) == open
    }
}

fn is_indentable_preprocessor_block(
    tokens: &[Token],
    start: usize,
    is_first_conditional: bool,
) -> bool {
    // astyle reads `#error`, `#warning`, and `#line` as comments; one that
    // opens a block on the first line of the file leaves it unindented.
    if start == 0
        && tokens
            .iter()
            .skip(start + 1)
            .find(|token| !matches!(token, Token::Whitespace(_) | Token::Newline))
            .is_some_and(|token| {
                matches!(token, Token::Preprocessor(preprocessor)
                if matches!(
                    preprocessor_directive(&preprocessor.text),
                    Some("error" | "warning" | "line" | "region" | "endregion")
                ))
            })
    {
        return false;
    }
    let mut depth = 0usize;
    let mut paren_depth = 0isize;
    let mut saw_conditional = false;
    let mut potential_header_guard = false;
    let mut potential_header_guard_define = false;
    let mut saw_endif_hash_line = false;
    let mut block_end = None;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token {
            Token::Preprocessor(preprocessor) => {
                let line = &preprocessor.text;
                // A condition whose parentheses span its continued lines
                // leaves astyle unbalanced: no block around it indents.
                if preprocessor_directive(line)
                    .is_some_and(|directive| matches!(directive, "if" | "elif"))
                    && line.lines().nth(1).is_some()
                    && line
                        .lines()
                        .any(|part| part.matches('(').count() != part.matches(')').count())
                {
                    return false;
                }
                match preprocessor_directive(line) {
                    Some(directive @ ("if" | "ifdef" | "ifndef")) => {
                        depth += 1;
                        saw_conditional = true;
                        if is_first_conditional
                            && depth == 1
                            && is_ndef_preprocessor_statement(line, directive)
                        {
                            potential_header_guard = true;
                        }
                    }
                    Some("endif") => {
                        depth = depth.saturating_sub(1);
                        if saw_conditional && depth == 0 {
                            if paren_depth != 0 {
                                return false;
                            }
                            block_end = Some(index + 1);
                            break;
                        }
                    }
                    Some("define")
                        if depth > 0
                            && define_continues(line)
                            && (potential_header_guard || !saw_endif_hash_line) =>
                    {
                        return false;
                    }
                    Some("define") if potential_header_guard && depth == 1 => {
                        potential_header_guard_define = true;
                    }
                    _ => {}
                }
            }
            Token::Word(word)
                if depth > 0 && word == "endif" && line_has_hash_after(tokens, index) =>
            {
                saw_endif_hash_line = true;
            }
            Token::Symbol('{' | '}') if depth > 0 => return false,
            Token::Symbol('(') if depth > 0 => paren_depth += 1,
            Token::Symbol(')') if depth > 0 => paren_depth -= 1,
            Token::Symbol(':') if depth > 0 => return false,
            Token::Newline if depth > 0 && paren_depth != 0 => return false,
            _ => {}
        }
    }
    let Some(block_end) = block_end else {
        return false;
    };
    if is_first_conditional
        && potential_header_guard_define
        && !preprocessor_block_followed_by_code(tokens, block_end)
    {
        return false;
    }
    true
}

/// Whether a `#define` goes on past its line: astyle reads only a
/// backslash ending it, not a comment it opens.
fn define_continues(line: &str) -> bool {
    line.lines().nth(1).is_some()
        && line
            .lines()
            .next()
            .is_some_and(|first| first.ends_with('\\'))
}

fn line_has_hash_after(tokens: &[Token], start: usize) -> bool {
    for token in tokens.iter().skip(start + 1) {
        match token {
            Token::Newline | Token::Preprocessor(_) => return false,
            Token::Symbol('#') => return true,
            _ => {}
        }
    }
    false
}

fn preprocessor_block_followed_by_code(tokens: &[Token], start: usize) -> bool {
    tokens.iter().skip(start).any(|token| {
        !matches!(
            token,
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _)
        )
    })
}

fn collapse_pound_whitespace(line: &str) -> String {
    let Some(rest) = line.strip_prefix('#') else {
        return line.to_string();
    };
    let trimmed = rest.trimmed_start();
    if trimmed.len() == rest.len() {
        line.to_string()
    } else {
        format!("#{trimmed}")
    }
}

fn preprocessor_condition(line: &str) -> Option<&str> {
    let line = line.trimmed_start().strip_prefix('#')?.trimmed_start();
    let condition = line.strip_prefix("if")?;
    Some(condition.trimmed_start())
}

fn is_not_defined_condition(condition: &str) -> bool {
    let condition = condition.trimmed_start();
    let Some(rest) = condition.strip_prefix('!') else {
        return false;
    };
    let Some(rest) = rest.trimmed_start().strip_prefix("defined") else {
        return false;
    };
    if rest.chars().next().is_some_and(is_identifier_continue) {
        return false;
    }
    let rest = rest.trimmed_start();
    if let Some(inner) = rest.strip_prefix('(') {
        let name = inner.trimmed_start();
        let name_end = name
            .char_indices()
            .take_while(|(_, ch)| is_identifier_continue(*ch))
            .map(|(index, ch)| index + ch.len_utf8())
            .last()
            .unwrap_or(0);
        return name[..name_end]
            .chars()
            .next()
            .is_some_and(is_identifier_start)
            && name[name_end..].trimmed_start().starts_with(')');
    }
    rest.chars().next().is_some_and(is_identifier_start)
}

pub(crate) fn is_cplusplus_conditional(line: &str) -> bool {
    match preprocessor_directive(line) {
        Some("ifdef") => preprocessor_directive_argument(line) == Some("__cplusplus"),
        Some("if") => {
            let Some(condition) = preprocessor_condition(line) else {
                return false;
            };
            let Some(rest) = condition.trimmed_start().strip_prefix("defined") else {
                return false;
            };
            if rest.chars().next().is_some_and(is_identifier_continue) {
                return false;
            }
            let Some(inner) = rest.trimmed_start().strip_prefix('(') else {
                return false;
            };
            let name = inner.trimmed_start();
            name.strip_prefix("__cplusplus")
                .is_some_and(|tail| !tail.chars().next().is_some_and(is_identifier_continue))
        }
        _ => false,
    }
}

pub(crate) fn is_conditional_preprocessor(directive: &str) -> bool {
    matches!(
        directive,
        "if" | "ifdef" | "ifndef" | "elif" | "elifdef" | "elifndef" | "else" | "endif"
    )
}

pub(crate) fn is_known_preprocessor_directive(directive: &str) -> bool {
    is_conditional_preprocessor(directive)
        || matches!(
            directive,
            "define"
                | "include"
                | "include_next"
                | "import"
                | "line"
                | "error"
                | "warning"
                | "pragma"
                | "undef"
                | "region"
                | "endregion"
        )
}

fn is_always_indented_preprocessor_line(line: &str, directive: &str) -> bool {
    matches!(directive, "region" | "endregion")
        || (directive == "pragma"
            && preprocessor_directive_argument(line)
                .is_some_and(|argument| matches!(argument, "omp" | "region" | "endregion")))
}

fn is_bare_macro_invocation(trimmed: &str) -> bool {
    !trimmed.is_empty()
        && trimmed.chars().any(|ch| ch.is_ascii_alphabetic())
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn preprocessor_directive_argument(line: &str) -> Option<&str> {
    let mut parts = line.trimmed_start().strip_prefix('#')?.split_whitespace();
    parts.next()?;
    parts.next()
}

impl FormatEngine<'_> {
    pub(crate) fn preprocessor_split_else_active(&self) -> bool {
        self.preprocessor.split_else.is_active()
    }

    fn mark_preprocessor_split_else_after_chain(&mut self) {
        if self.preprocessor_split_else_active() {
            self.preprocessor.split_else.after_line = true;
        }
    }

    fn preprocessor_line_follows_split_else_output(&self) -> bool {
        let mut saw_preprocessor = false;
        for line in self.output.scoped().iter().rev().take(8) {
            let trimmed = line.trimmed();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('#') {
                if preprocessor_directive(trimmed)
                    .is_some_and(|directive| directive == "else" || directive.starts_with("elif"))
                {
                    return false;
                }
                saw_preprocessor = true;
                continue;
            }
            return saw_preprocessor && (trimmed.ends_with("} else") || trimmed == "else");
        }
        false
    }

    pub(crate) fn push_preprocessor(
        &mut self,
        line: &str,
        opaque_literal_line_ranges: &[(usize, usize)],
    ) {
        self.finish_line();
        let directive = line.lines().next().and_then(preprocessor_directive);
        let header_before_preprocessor = self.output.last_non_empty_scoped().map(|line| {
            (
                trailing_word(line.trimmed_end()).to_string(),
                leading_visual_width(line, self.options.tab_width),
            )
        });
        let indent_continued_conditional = self.options.indent_preproc_conditional
            && directive.is_some_and(is_conditional_preprocessor);
        let indent_continued_block_conditional = self.options.indent_preproc_block
            && directive.is_some_and(|directive| {
                matches!(directive, "if" | "ifdef" | "ifndef") || directive.starts_with("elif")
            });
        let branch_separator =
            directive.is_some_and(|directive| directive == "else" || directive.starts_with("elif"));
        if !branch_separator
            && self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| {
                    let trimmed = line.trimmed();
                    !trimmed.is_empty() && !is_bare_macro_invocation(trimmed)
                })
                .is_some_and(|line| {
                    let trimmed = line.trimmed();
                    trimmed == "else" || trimmed.ends_with("} else")
                })
        {
            self.preprocessor.split_else.pending_body = true;
        }
        let branch_separator_after_else = branch_separator
            && (self.layout.command_state.current_header.as_deref() == Some("else")
                || self.output.last_non_empty_scoped().is_some_and(|line| {
                    let trimmed = line.trimmed();
                    trimmed == "else" || trimmed.ends_with("} else")
                }));
        if branch_separator_after_else {
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.continuation_indent.clear_next_line();
            self.layout.pending_braceless_block_bias = None;
            self.layout.inline_nested_header_braceless_bias = None;
            self.layout.else_if_break_depths.clear();
            self.preprocessor.split_else.reset();
            if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && base + delta == self.layout.indentation.indent()
            {
                self.layout.indentation.exit_braceless_block();
            }
        } else if self.layout.command_state.current_header.is_some()
            && directive.is_some_and(is_known_preprocessor_directive)
        {
            self.layout.command_state.preprocessor_after_header = true;
        } else if self.layout.command_state.current_header.is_some() && directive.is_some() {
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.continuation_indent.clear_next_line();
        }
        let is_define = directive.is_some_and(|directive| directive == "define");

        let parts: Vec<&str> = line.lines().collect();
        let continued_define_contains_directive = is_define
            && parts.iter().skip(1).any(|part| {
                let part = part.trimmed_end();
                preprocessor_directive(part).is_some_and(is_known_preprocessor_directive)
                    && !part.ends_with('\\')
            });
        // Lines a block comment carries past an unbroken directive continue
        // no define body.
        let continued_by_backslashes = parts
            .first()
            .is_some_and(|part| part.trimmed_end().ends_with('\\'));
        if self.options.indent_preproc_define
            && is_define
            && parts.len() > 1
            && continued_by_backslashes
            && !continued_define_contains_directive
            && opaque_literal_line_ranges.is_empty()
        {
            self.push_multiline_define(&parts);
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            self.preprocessor.last_output_was_preprocessor = true;
            return;
        }

        let mut continued_line_comment = false;
        let mut open_paren_columns = Vec::new();
        for (index, part) in parts.iter().enumerate() {
            let backslash_continued = index > 0 && parts[index - 1].trimmed_end().ends_with('\\');
            self.push_preprocessor_part(
                index,
                part,
                backslash_continued,
                &PreprocessorLineParts {
                    opaque_literal_line_ranges,
                    branch_separator_after_else,
                    indent_continued_conditional,
                    indent_continued_block_conditional,
                    comment_directive: matches!(directive, Some("error" | "warning" | "line")),
                },
                &mut continued_line_comment,
                &mut open_paren_columns,
            );
        }
        if is_define && !line.trimmed_end().ends_with('\\') {
            if !(is_define && self.preprocessor_split_else_active()) {
                self.layout.continuation_indent.clear_next_line();
                self.layout
                    .continuation_indent
                    .next_input_line_continuation_indent = None;
            }
            self.layout.nesting.clear_continuation_indents();
            self.layout.frame_stack.clear_stream_frames();
            self.layout.frame_stack.clear_logical_frames();
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
        }
        if directive == Some("endif")
            && let Some(previous) = self.output.last()
            && has_hash_outside_literals(previous)
            && !previous.trimmed_start().starts_with('#')
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces =
                Some(leading_visual_width(previous, self.options.tab_width));
        }
        if directive == Some("else")
            && let Some((word, indent)) = header_before_preprocessor
            && word == "do"
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(indent + self.options.indent_width * 2);
        }
        if is_define && parts.len() > 1 && self.preprocessor_split_else_active()
            || self.preprocessor_line_follows_split_else_output()
        {
            self.mark_preprocessor_split_else_after_chain();
        } else {
            self.preprocessor.split_else.after_line = false;
        }
        if line.trimmed_end().ends_with("&&")
            && let Some(previous) = self.output.last()
        {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width,
            );
        }
        self.layout.previous = PreviousToken::Other;
        self.previous_was_newline = false;
        self.preprocessor.last_output_was_preprocessor = true;
    }

    fn push_preprocessor_part(
        &mut self,
        index: usize,
        part: &str,
        backslash_continued: bool,
        parts: &PreprocessorLineParts<'_>,
        continued_line_comment: &mut bool,
        open_paren_columns: &mut Vec<usize>,
    ) {
        let PreprocessorLineParts {
            opaque_literal_line_ranges,
            branch_separator_after_else,
            indent_continued_conditional,
            indent_continued_block_conditional,
            comment_directive,
        } = *parts;
        if comment_directive && index > 0 {
            // A directive an indented preprocessor block moves takes its
            // lines along.
            let prefix = self
                .output
                .last()
                .map(|row| row[..row.len() - row.trimmed_start().len()].to_string())
                .unwrap_or_default();
            self.adjust_and_publish_line(format!("{prefix}{}", part.trimmed()));
            return;
        }
        let line_is_continued_comment = *continued_line_comment;
        let is_opaque_literal_line = opaque_literal_line_ranges
            .iter()
            .any(|&(start, end)| start <= index && index <= end);
        let is_opaque_literal_continuation = is_opaque_literal_line && index > 0;
        let part = if is_opaque_literal_line {
            part
        } else {
            part.trimmed_end()
        };
        // On a line a backslash continues, a `#` before an unknown name
        // stringizes a macro parameter.
        let directive = (!line_is_continued_comment && !is_opaque_literal_continuation)
            .then(|| preprocessor_directive(part))
            .flatten()
            .filter(|&name| !backslash_continued || is_known_preprocessor_directive(name));
        let part_known_directive = directive.is_some_and(is_known_preprocessor_directive);
        let part_is_define = directive == Some("define");
        let opening_indentable = if matches!(directive, Some("if" | "ifdef" | "ifndef")) {
            Some(self.should_indent_preprocessor_block())
        } else {
            None
        };
        let in_indentable_block = match opening_indentable {
            Some(indentable) => indentable,
            None => self.preprocessor.indented_block_stack.last() == Some(&true),
        };
        // astyle writes the directives it reads as comments as they stand.
        let collapse = self.options.indent_preproc_block
            && directive.is_some_and(|directive| {
                !matches!(
                    directive,
                    "error" | "warning" | "line" | "region" | "endregion"
                )
            })
            && in_indentable_block;
        if index == 0 && self.take_block_spacing_blank(part) {
            self.push_empty_line();
        }
        let force_unindented_branch_separator = branch_separator_after_else && index == 0;
        let indent = if force_unindented_branch_separator {
            None
        } else if index > 0 && indent_continued_conditional {
            Some(self.conditional_continuation_indent(open_paren_columns))
        } else if index > 0
            && (indent_continued_block_conditional
                || !backslash_continued && self.preprocessor.group_blocks.contains(&true))
            && self.preprocessor.indented_block_stack.last() == Some(&true)
        {
            Some(PreprocessorLineIndent::Level(
                self.preprocessor_base_level(),
            ))
        } else if directive == Some("endif")
            && self.preprocessor.branch_stack.is_empty()
            && self.token_input.token_source_line_indent > 0
        {
            Some(PreprocessorLineIndent::Exact {
                structural_level: 0,
                spaces: self.token_input.token_source_line_indent,
            })
        } else {
            self.preprocessor_line_indent(part, part_is_define, index, opening_indentable)
        };
        let indented_continuation = index > 0 && indent.is_some();
        let output_line = if let Some(indent) = indent {
            let prefix = match indent {
                PreprocessorLineIndent::Level(level) => self.options.indent_prefix(level),
                PreprocessorLineIndent::Exact {
                    structural_level,
                    spaces,
                } => self
                    .options
                    .continuation_indent_prefix(structural_level, spaces),
            };
            let body = if collapse {
                collapse_pound_whitespace(part.trimmed_start())
            } else {
                part.trimmed_start().to_string()
            };
            format!("{prefix}{body}")
        } else if collapse {
            let leading = &part[..part.len() - part.trimmed_start().len()];
            format!(
                "{leading}{}",
                collapse_pound_whitespace(part.trimmed_start())
            )
        } else if force_unindented_branch_separator
            || line_is_continued_comment
            || (directive.is_some()
                && (part_known_directive || self.token_input.token_source_line_indent == 0))
        {
            part.trimmed_start().to_string()
        } else if directive.is_some() {
            format!(
                "{}{}",
                " ".repeat(self.token_input.token_source_line_indent),
                part.trimmed_start()
            )
        } else {
            part.to_string()
        };
        if is_opaque_literal_line {
            let structural_start = output_line.len();
            self.adjust_and_publish_raw_literal_line(output_line, structural_start);
        } else {
            self.adjust_and_publish_line(output_line);
            if indented_continuation {
                self.output.mark_last_indented_directive_continuation();
            }
            // astyle reads the directive's own line apart from the code its
            // continued lines are indented as.
            if indent_continued_conditional
                && index > 0
                && let Some(published) = self.output.last()
            {
                let after_parens = self.options.indent_after_parens.then(|| {
                    leading_visual_width(published, self.options.tab_width)
                        + self.options.indent_width
                });
                track_open_paren_columns(
                    published,
                    self.options.tab_width,
                    after_parens,
                    open_paren_columns,
                );
            }
        }
        if !line_is_continued_comment && !is_opaque_literal_continuation {
            self.update_preprocessor_state(
                part,
                opening_indentable,
                index == 0 && branch_separator_after_else,
            );
        }
        let line_ends_with_backslash = part.trimmed_end().ends_with('\\');
        *continued_line_comment = if line_is_continued_comment {
            line_ends_with_backslash
        } else if line_ends_with_backslash && !is_opaque_literal_line {
            let comment_start = line_comment_split_limit(part);
            comment_start < part.len() && part[comment_start..].trimmed_start().starts_with("//")
        } else {
            false
        };
    }

    pub(crate) fn preprocessor_base_level(&self) -> usize {
        self.preprocessor
            .indented_block_stack
            .iter()
            .filter(|&&indented| indented)
            .count()
    }

    fn preprocessor_line_indent(
        &self,
        line: &str,
        is_define: bool,
        define_part_index: usize,
        opening_indentable: Option<bool>,
    ) -> Option<PreprocessorLineIndent> {
        if self.options.indent_preproc_define && is_define {
            let continuation = if define_part_index == 0 {
                0
            } else {
                self.options.continuation_indent
            };
            return Some(PreprocessorLineIndent::Level(
                self.preprocessor_base_level() + continuation,
            ));
        }

        let directive = preprocessor_directive(line)?;
        if is_always_indented_preprocessor_line(line, directive) {
            return Some(PreprocessorLineIndent::Level(
                self.layout.indentation.indent(),
            ));
        }
        if self.layout.line_adjuster.is_in_macro_block() {
            return None;
        }
        if self.options.indent_preproc_conditional
            && is_conditional_preprocessor(directive)
            && self
                .layout
                .frame_stack
                .active_delimiter()
                .is_some_and(|frame| frame.role == ParenRole::Header)
            && let Some(header) = self.layout.frame_stack.active_header()
        {
            // Parens indenting after them leave the directive at the
            // header's own level.
            let spaces = if self.options.indent_after_parens {
                header
                    .body_indent_spaces
                    .saturating_sub(self.options.indent_width)
            } else {
                // astyle takes the header's level off the condition's column.
                self.layout
                    .continuation_indent
                    .next_line_indent_spaces
                    .map_or(header.body_indent_spaces, |spaces| {
                        spaces.saturating_sub(self.options.indent_width)
                    })
            };
            return Some(PreprocessorLineIndent::Exact {
                structural_level: spaces / self.options.indent_width.max(1),
                spaces,
            });
        }
        if self.options.indent_preproc_block && is_conditional_preprocessor(directive) {
            match directive {
                "if" | "ifdef" | "ifndef" => {
                    // A file scope block opening in parens stands at the
                    // margin.
                    if self.layout.indentation.indent() == 0
                        && self.layout.nesting.paren_depth > 0
                        && opening_indentable == Some(true)
                    {
                        return Some(PreprocessorLineIndent::Level(0));
                    }
                    if self.preprocessor.indented_block_stack.last() == Some(&true) {
                        return Some(PreprocessorLineIndent::Level(
                            self.layout.indentation.indent(),
                        ));
                    }
                    // An indented block's opening stands at its level, not
                    // at a continuation.
                    if opening_indentable == Some(true) {
                        return Some(PreprocessorLineIndent::Level(
                            self.layout.indentation.indent(),
                        ));
                    }
                }
                _ => {
                    if self.preprocessor.indented_block_stack.last() == Some(&true) {
                        return Some(PreprocessorLineIndent::Level(
                            self.layout.indentation.indent().saturating_sub(1),
                        ));
                    }
                }
            }
        }
        if self.options.indent_preproc_conditional && is_conditional_preprocessor(directive) {
            if matches!(directive, "if" | "ifdef" | "ifndef")
                || self
                    .layout
                    .indentation
                    .current_preprocessor_indent()
                    .is_some()
            {
                return Some(
                    self.current_preprocessor_indent(matches!(
                        directive,
                        "if" | "ifdef" | "ifndef"
                    )),
                );
            }
            return None;
        }
        if self.options.indent_preproc_block
            && !is_conditional_preprocessor(directive)
            && self
                .preprocessor
                .indented_block_stack
                .iter()
                .any(|&indented| indented)
        {
            return Some(PreprocessorLineIndent::Level(
                self.layout.indentation.indent(),
            ));
        }
        if line.trimmed_start().matches('#').count() > 1
            && self.layout.indentation.indent() > 0
            && self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| line.trimmed() == ";")
        {
            return Some(PreprocessorLineIndent::Level(
                self.layout.indentation.indent(),
            ));
        }
        None
    }

    /// The indent of a conditional directive: an opening one stands at the
    /// code, the others at the directive that opened them.
    fn current_preprocessor_indent(&self, opening: bool) -> PreprocessorLineIndent {
        if opening && let Some(spaces) = self.break_else_if_directive_column() {
            return PreprocessorLineIndent::Exact {
                structural_level: spaces / self.options.indent_width.max(1),
                spaces,
            };
        }
        if let Some(spaces) = self.direct_switch_body_indent_spaces() {
            return PreprocessorLineIndent::Exact {
                structural_level: spaces / self.options.indent_width.max(1),
                spaces,
            };
        }
        // In the block a case label opens, directives stand at its body.
        if let Some(spaces) = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.case_block)
            .map(|frame| {
                // The line adjuster takes the case unindent of the line off.
                frame.body_indent_column
                    + self.layout.line_adjuster.case_unindent_depth_for_line("#")
                        * self.options.indent_width
            })
        {
            return PreprocessorLineIndent::Exact {
                structural_level: spaces / self.options.indent_width.max(1),
                spaces,
            };
        }
        if let Some(case_label_column) = self.active_case_label_indent_spaces() {
            return PreprocessorLineIndent::Exact {
                structural_level: self.layout.indentation.indent() + 1,
                spaces: case_label_column + self.options.indent_width,
            };
        }
        if let Some(indent) = self
            .layout
            .indentation
            .current_preprocessor_indent()
            .filter(|_| !opening)
        {
            if let Some(spaces) = indent.spaces {
                return PreprocessorLineIndent::Exact {
                    structural_level: indent.level,
                    spaces,
                };
            }
            return PreprocessorLineIndent::Level(indent.level);
        }
        if let Some(spaces) = self.preprocessor_continuation_spaces() {
            PreprocessorLineIndent::Exact {
                structural_level: self.layout.indentation.indent(),
                spaces,
            }
        } else {
            // A block nested in a case body stands past the case's own level.
            PreprocessorLineIndent::Level(
                self.layout.indentation.indent()
                    + self.case_body_indent_extra(LineKind::Normal)
                    + self.split_else_directive_extra(),
            )
        }
    }

    /// astyle indents a conditional's continued lines as code: after a paren
    /// the directive left open, else where a statement line would stand.
    fn conditional_continuation_indent(
        &self,
        open_paren_columns: &[usize],
    ) -> PreprocessorLineIndent {
        let structural_level = self.layout.indentation.indent();
        if let Some(&spaces) = open_paren_columns.last() {
            return PreprocessorLineIndent::Exact {
                structural_level,
                spaces,
            };
        }
        if let Some(spaces) = self.layout.continuation_indent.next_line_indent_spaces {
            return PreprocessorLineIndent::Exact {
                structural_level,
                spaces: spaces.max(
                    self.layout
                        .nesting
                        .current_continuation_indent_spaces()
                        .unwrap_or(0),
                ),
            };
        }
        self.current_preprocessor_indent(false)
    }

    /// The column of a directive inside a continued statement: in the
    /// condition of a control header astyle takes the header's level back off.
    fn preprocessor_continuation_spaces(&self) -> Option<usize> {
        // A paren opened past the pending column holds the next line.
        let spaces = self
            .layout
            .continuation_indent
            .next_line_indent_spaces?
            .max(
                self.layout
                    .nesting
                    .current_continuation_indent_spaces()
                    .unwrap_or(0),
            );
        let in_header_condition = self.header_paren.depth.is_some()
            && matches!(
                self.layout.command_state.current_header.as_deref(),
                Some("if" | "while" | "for")
            );
        Some(if in_header_condition {
            spaces.saturating_sub(self.options.indent_width)
        } else {
            spaces
        })
    }

    /// A body an `else` split across directives opens stands a level deeper.
    fn split_else_directive_extra(&self) -> usize {
        // The brace closing the body ended the chain unless an `else` goes on.
        let body_closed = self.preprocessor.split_else.clear_pending_after_brace
            && !self.preprocessor.split_else.closing_brace_has_else;
        if self.preprocessor.split_else.extra_indent && !body_closed {
            self.preprocessor.split_else.extra_levels
        } else {
            0
        }
    }

    fn update_preprocessor_state(
        &mut self,
        line: &str,
        opening_indentable: Option<bool>,
        branch_separator_after_else: bool,
    ) {
        match preprocessor_directive(line) {
            Some("if" | "ifdef" | "ifndef") => {
                let should_indent_block =
                    opening_indentable.unwrap_or_else(|| self.should_indent_preprocessor_block());
                let mut suspended = None;
                let mut group_block = false;
                if should_indent_block {
                    // At file scope astyle indents a block's lines by the
                    // block alone, a statement they continue or not.
                    let file_scope = self.layout.indentation.indent() == 0;
                    group_block = file_scope;
                    self.layout.indentation.enter_block();
                    if file_scope {
                        suspended = self
                            .layout
                            .continuation_indent
                            .next_line_indent_spaces
                            .take();
                    } else if let Some(spaces) = self
                        .layout
                        .continuation_indent
                        .next_line_indent_spaces
                        .as_mut()
                    {
                        *spaces += self.options.indent_width;
                    }
                }
                let spaces = self
                    .options
                    .indent_preproc_conditional
                    .then(|| self.break_else_if_directive_column())
                    .flatten()
                    .or_else(|| self.preprocessor_continuation_spaces());
                self.layout.indentation.push_preprocessor_indent(
                    self.layout.indentation.indent()
                        + self.case_body_indent_extra(LineKind::Normal)
                        + self.split_else_directive_extra(),
                    spaces,
                );
                self.preprocessor
                    .indented_block_stack
                    .push(should_indent_block);
                self.preprocessor.suspended_continuations.push(suspended);
                self.preprocessor.group_blocks.push(group_block);
                self.preprocessor.branch_stack.push(self.branch_snapshot());
            }
            Some("else" | "elif" | "elifdef" | "elifndef") => {
                if self
                    .preprocessor
                    .branch_stack
                    .last()
                    .is_some_and(|branch| branch.first_branch_end.is_none())
                {
                    let end = Box::new(self.branch_snapshot());
                    if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
                        branch.first_branch_end = Some(end);
                    }
                }
                if let Some(snapshot) = self.preprocessor.branch_stack.last().cloned() {
                    self.restore_branch_snapshot(snapshot);
                    if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
                        branch.restore_body_indent = branch.first_body_indent_spaces.is_some();
                    }
                    self.close_braceless_bodies_kept_for_else();
                }
            }
            Some("endif") => {
                // Branches that leave different braces open continue from the
                // first, as astyle does.
                if let Some(end) = self
                    .preprocessor
                    .branch_stack
                    .pop()
                    .and_then(|branch| branch.first_branch_end)
                    && end.layout.nesting.brace_type_stack.len()
                        != self.layout.nesting.brace_type_stack.len()
                {
                    self.restore_branch_snapshot(*end);
                }
                self.layout.indentation.pop_preprocessor_indent();
                let suspended = self.preprocessor.suspended_continuations.pop().flatten();
                self.preprocessor.group_blocks.pop();
                if self.preprocessor.indented_block_stack.pop() == Some(true) {
                    self.layout.indentation.exit_block();
                    if suspended.is_some() {
                        self.layout.continuation_indent.next_line_indent_spaces = suspended;
                    } else if let Some(spaces) = self
                        .layout
                        .continuation_indent
                        .next_line_indent_spaces
                        .as_mut()
                    {
                        *spaces = spaces.saturating_sub(self.options.indent_width);
                    }
                }
            }
            _ => {}
        }
        if branch_separator_after_else {
            self.layout.command_state.current_header = None;
            self.layout.command_state.preprocessor_after_header = false;
            self.layout.continuation_indent.clear_next_line();
            self.layout.pending_braceless_block_bias = None;
            self.layout.inline_nested_header_braceless_bias = None;
            self.layout.else_if_break_depths.clear();
            self.preprocessor.split_else.reset();
            if let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && base + delta == self.layout.indentation.indent()
            {
                self.layout.indentation.exit_braceless_block();
            }
        }
    }

    fn should_indent_preprocessor_block(&mut self) -> bool {
        let block_is_indentable = self
            .preprocessor
            .indentable_blocks
            .pop_front()
            .unwrap_or(true);
        if self.preprocessor.indented_block_stack.last() == Some(&true) {
            return true;
        }
        self.options.indent_preproc_block
            && block_is_indentable
            && !self.layout.line_adjuster.is_in_macro_block()
            && self.preprocessor_region_allows_block_indent(self.preprocessor_region(false))
    }

    pub(crate) fn branch_snapshot(&self) -> PreprocessorBranchState {
        PreprocessorBranchState {
            layout: self.layout.clone(),
            first_body_indent_spaces: None,
            restore_body_indent: false,
            preprocessor_split_else: self.preprocessor.split_else,
            header_paren: self.header_paren.clone(),
            inline_array: self.inline_array.clone(),
            pending_extern: self.pending_extern,
            extern_c_guard: self.extern_c_guard,
            first_branch_end: None,
            directive: self.preprocessor.active_directive,
        }
    }

    pub(crate) fn restore_branch_snapshot(&mut self, snapshot: PreprocessorBranchState) {
        let active_split_else = self.preprocessor.split_else;
        let PreprocessorBranchState {
            layout,
            first_body_indent_spaces: _,
            restore_body_indent: _,
            preprocessor_split_else,
            header_paren,
            inline_array,
            pending_extern,
            extern_c_guard,
            first_branch_end: _,
            directive: _,
        } = snapshot;
        self.layout = layout;
        self.preprocessor.split_else =
            preprocessor_split_else.after_branch_restore(active_split_else);
        self.header_paren = header_paren;
        self.inline_array = inline_array;
        self.pending_extern = pending_extern;
        self.extern_c_guard = extern_c_guard;
    }
}

/// Pushes the column after each paren `line` opens, or `after_parens` when
/// lines indent after parens, and pops one per paren it closes, outside
/// literals and comments.
fn track_open_paren_columns(
    line: &str,
    tab_width: usize,
    after_parens: Option<usize>,
    columns: &mut Vec<usize>,
) {
    let mut column = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        column = if ch == '\t' {
            column + tab_width.max(1) - column % tab_width.max(1)
        } else {
            column + 1
        };
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '/' if matches!(chars.peek(), Some('/' | '*')) => break,
            '(' => columns.push(after_parens.unwrap_or(column)),
            ')' => {
                columns.pop();
            }
            _ => {}
        }
    }
}

impl FormatEngine<'_> {
    /// A first branch that goes on with an `else` keeps the chain's levels;
    /// a later branch that does not leaves the chain.
    fn close_braceless_bodies_kept_for_else(&mut self) {
        let tokens = &self.tree.tokens;
        let starts_with_else = |directive: usize| {
            next_code_token(tokens, directive + 1)
                .is_some_and(|index| matches!(&tokens[index], Token::Word(word) if word == "else"))
        };
        let Some(directive) = self.preprocessor.active_directive else {
            return;
        };
        let Some(opening) = self
            .preprocessor
            .branch_stack
            .last()
            .and_then(|branch| branch.directive)
        else {
            return;
        };
        if starts_with_else(directive) || !starts_with_else(opening) {
            return;
        }
        while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
            && self.layout.indentation.indent() == base + delta
        {
            self.layout.indentation.exit_braceless_block();
        }
        self.unwind_else_if_break_depths();
        if let Some(branch) = self.preprocessor.branch_stack.last_mut() {
            branch.restore_body_indent = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockBreakers, BraceType, FormatEngine, PreprocessorRegion};
    use crate::config::FormatOptions;
    use crate::formatter::lexer::{Token, tokenize};
    use crate::formatter::text::line_scan::preprocessor_directive;

    #[test]
    fn block_breakers_judge_blocks_as_reading_them_does() {
        let sources = [
            "#if A\n#if B\nint a;\n#endif\n#endif\n",
            "#if A\nint a;\n#endif\n#if B\nf(a,\n  b);\n#endif\n#ifdef C\nf(a, b);\n#endif\n",
            "x\n#if A\n#if B\n{\n#endif\nint c;\n#endif\n#if D\nlabel:\n#endif\n",
            "#if A\n#if (B && \\\n  C)\nint a;\n#endif\n#endif\n#if X\n#elif (Y || \\\n Z)\n#endif\n",
            "#if A\n#if B\n#define M(x) \\\n  x\n#endif\n#endif\n#if C\nendif #\n#define N \\\n  1\n#endif\n",
            "#if A\nf(\n#if B\na)\n#endif\n#endif\n#if C\n(\n#endif\n)\n#if D\nint d;\n#endif\n",
            "#if A\n#if B\n(a)\n#endif\n)\n#if C\n((\n))\n#endif\n#if E\nint e;\n#endif\n#endif\n",
        ];
        for source in sources {
            let tokens = tokenize(source);
            let breakers = BlockBreakers::new(&tokens);
            let mut open = Vec::new();
            for (index, token) in tokens.iter().enumerate() {
                let Token::Preprocessor(preprocessor) = token else {
                    continue;
                };
                match preprocessor_directive(&preprocessor.text) {
                    Some("if" | "ifdef" | "ifndef") => open.push(index),
                    Some("endif") => {
                        let start = open.pop().expect("open conditional");
                        assert_eq!(
                            breakers.indentable(start, index),
                            super::is_indentable_preprocessor_block(&tokens, start, false),
                            "block at {start} in {source:?}"
                        );
                    }
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn classifies_regions_from_brace_ownership() {
        use BraceType::{Array, Command, Definition, Enum, Namespace, Struct};
        use PreprocessorRegion::{
            Block, EnumList, FieldDeclarationList, MacroBody, Namespace as NamespaceRegion,
            TopLevel, Unknown,
        };

        for (stack, expected) in [
            (Vec::new(), TopLevel),
            (vec![Namespace], NamespaceRegion),
            (vec![Command], Block),
            (vec![Definition], Block),
            (vec![Struct], FieldDeclarationList),
            (vec![Enum], EnumList),
            (vec![Array], Unknown),
        ] {
            assert_eq!(
                FormatEngine::preprocessor_region_from_brace_stack(&stack, false),
                expected
            );
        }
        assert_eq!(
            FormatEngine::preprocessor_region_from_brace_stack(&[Struct], true),
            MacroBody
        );
    }

    #[test]
    fn block_indent_gate_uses_region() {
        let options = FormatOptions::default();
        let mut formatter = FormatEngine::new(&options);

        assert!(formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::TopLevel));
        formatter
            .layout
            .nesting
            .brace_type_stack
            .push(BraceType::Definition);
        assert!(!formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::Block));
        assert!(!formatter.preprocessor_region_allows_block_indent(PreprocessorRegion::Unknown));
    }
}
