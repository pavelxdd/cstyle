//! C, C++, and Objective-C formatter.
//!
//! [`format()`] normalizes line endings and tabs, tokenizes the source
//! ([`lexer`]), and feeds the tokens through [`engine::FormatEngine`]. The
//! engine classifies token roles ([`syntax`]), builds each output line with
//! the per-token handlers ([`tokens`]) and the brace, construct,
//! preprocessor, and continuation rules, and emits finished lines through
//! [`output`]. Brace styles that reshape whole lines are applied last by
//! [`braces::postprocess`].

use crate::config::{BraceStyle, FormatOptions, IndentStyle};
use crate::formatter::braces::postprocess::postprocess_brace_style;
use crate::formatter::constructs::class_declarations;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, tokenize_owned};
use crate::formatter::output::finish::EmptyFillSource;
use crate::formatter::text::line_scan::preprocessor_directive;
use crate::formatter::text::tabs;
use crate::formatter::text::trim::Trimmed;
use crate::source::line_endings;

mod braces;
mod constructs;
mod continuation;
mod engine;
mod index_hash;
mod lexer;
mod output;
mod preprocessor;
mod state;
mod structure;
mod syntax;
mod text;
mod tokens;

#[cfg(test)]
mod tests;

pub(crate) fn format(source: &str, options: &FormatOptions) -> String {
    format_owned(source.to_owned(), options)
}

/// `format` of a source it takes, which becomes the text its tokens share.
pub(crate) fn format_owned(source: String, options: &FormatOptions) -> String {
    let normalized = match line_endings::normalize(&source) {
        std::borrow::Cow::Owned(normalized) => Some(normalized),
        std::borrow::Cow::Borrowed(_) => None,
    };
    let source = normalized.unwrap_or(source);
    let source = if options.convert_tabs && source.contains('\t') {
        let converted = tabs::source_to_spaces(&source, options.tab_width);
        drop(source);
        converted
    } else {
        source
    };
    let may_have_backslash_body = source.contains('\\');
    let may_have_swig = source.contains('%');
    let may_have_hash = source.contains('#');
    let may_have_noexcept = source.contains("noexcept");
    let tokens = tokenize_owned(source);
    // Tab-indented styles are laid out in spaces and get their tabs once the
    // output is finished.
    let spaced_options = (options.indent_style != IndentStyle::Spaces).then(|| {
        let mut spaced = options.clone();
        spaced.indent_style = IndentStyle::Spaces;
        spaced
    });
    let mut engine = FormatEngine::new(spaced_options.as_ref().unwrap_or(options));
    engine.output_indent_style = options.indent_style;
    engine
        .layout
        .line_adjuster
        .set_tab_conversion_enabled(false);
    if !case_adjustments_needed_for_tokens(&tokens, options) {
        engine
            .layout
            .line_adjuster
            .set_case_processing_enabled(false);
    }
    if !line_observer_needed_for_tokens(&tokens, options) {
        engine.layout.line_adjuster.set_line_observe_enabled(false);
    }
    engine.set_may_have_backslash_body(may_have_backslash_body);
    engine.set_may_have_swig(may_have_swig);
    engine.may_have_noexcept = may_have_noexcept;
    engine.may_have_class_base_access = class_declarations::has_base_access_token(&tokens);
    engine.preprocessor.may_have_preprocessor = may_have_hash
        || tokens
            .iter()
            .any(|token| matches!(token, Token::Preprocessor(_)));
    let (output, fill_sources) = engine.format_owned(tokens).finish();
    let output = postprocess_brace_style(output, options);
    if options.empty_line_fill
        && matches!(
            options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp | BraceStyle::Horstmann
        )
    {
        refill_empty_lines(&output, options.line_break(), &fill_sources)
    } else {
        output
    }
}

/// Brace styles that move lines after the output is finished leave filled
/// empty lines with the indent of a line that moved: they take the indent of
/// the line now before them in the state they were filled from.
fn refill_empty_lines(output: &str, line_break: &str, sources: &[EmptyFillSource]) -> String {
    let lines: Vec<&str> = output.split(line_break).collect();
    let blank_lines = lines
        .iter()
        .filter(|line| line.trimmed().is_empty())
        .count();
    // The split leaves an empty line after the final line break.
    let aligned = blank_lines == sources.len() + usize::from(output.ends_with(line_break));
    let mut sources = sources.iter();
    // A state's lead is none after a block comment: the fill the comment
    // left stands, as the lines it spans move with it.
    let mut root = Some("");
    let mut active: Vec<Option<&str>> = Vec::new();
    let mut waiting: Vec<Option<&str>> = Vec::new();
    let mut conditionals: Vec<(usize, usize)> = Vec::new();
    let mut refilled = Vec::with_capacity(lines.len());
    // The rest of a block comment keeps the indent of its first line.
    let mut in_block_comment = false;
    let mut continues_directive = false;
    for line in lines {
        let in_directive = continues_directive;
        continues_directive = !in_block_comment
            && (in_directive || line.trimmed_start().starts_with('#'))
            && line.trimmed_end().ends_with('\\');
        if in_directive && aligned {
            refilled.push(line);
            if line.trimmed().is_empty() {
                sources.next();
            }
            continue;
        }
        if line.trimmed().is_empty() {
            let source = sources.next();
            let lead = match source {
                _ if !aligned => *active.last().unwrap_or(&root),
                Some(EmptyFillSource::Root) => root,
                Some(EmptyFillSource::Branch) => active.last().copied().unwrap_or(root),
                Some(EmptyFillSource::Kept) | None => None,
            }
            .unwrap_or(line);
            // A line filled from a state takes that state's lead even when
            // the lead was empty before lines moved.
            let filled_from_state = aligned
                && matches!(
                    source,
                    Some(EmptyFillSource::Root | EmptyFillSource::Branch)
                );
            let keep = line.is_empty() && !filled_from_state || line.contains('\u{c}');
            refilled.push(if keep { line } else { lead });
            continue;
        }
        let state = active.last_mut().unwrap_or(&mut root);
        let code = line.trimmed_start();
        if in_block_comment {
            if aligned {
                *state = None;
            }
        } else if !code.starts_with('#') {
            *state = Some(&line[..line.len() - code.len()]);
        }
        if aligned && !in_block_comment {
            match preprocessor_directive(code) {
                Some("if" | "ifdef" | "ifndef") => {
                    waiting.push(*state);
                    conditionals.push((waiting.len() - 1, active.len()));
                }
                Some("else") => active.extend(waiting.pop()),
                Some("elif") => active.extend(waiting.last().copied()),
                Some("endif") => {
                    if let Some((waiting_len, active_len)) = conditionals.pop() {
                        waiting.truncate(waiting_len);
                        active.truncate(active_len);
                    }
                }
                _ => {}
            }
        }
        in_block_comment = ends_inside_block_comment(line, in_block_comment);
        refilled.push(line);
    }
    refilled.join(line_break)
}

/// Whether a block comment is open at the end of `line`, given whether one
/// was open at its start.
pub(crate) fn ends_inside_block_comment(line: &str, mut in_block_comment: bool) -> bool {
    let mut quote = None;
    let mut escaped = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
            }
        } else if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
        } else if ch == '/' && chars.peek() == Some(&'/') {
            break;
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
        } else if matches!(ch, '"' | '\'') {
            quote = Some(ch);
        }
    }
    in_block_comment
}

fn case_adjustments_needed_for_tokens(tokens: &[Token], options: &FormatOptions) -> bool {
    if options.convert_tabs || options.indent_switches || options.indent_cases {
        return true;
    }
    tokens.iter().any(|token| match token {
        Token::Word(word) => matches!(word.as_str(), "switch" | "case" | "default"),
        Token::Comment(_, _)
        | Token::StringLiteral(_)
        | Token::CharLiteral(_)
        | Token::Preprocessor(_)
        | Token::RawLine(_) => true,
        _ => false,
    })
}

fn line_observer_needed_for_tokens(tokens: &[Token], options: &FormatOptions) -> bool {
    if options.indent_switches || options.indent_cases {
        return true;
    }
    tokens.iter().any(|token| match token {
        Token::Word(word) => {
            matches!(word.as_str(), "switch" | "case" | "default")
                || options.access_labels.iter().any(|label| label == word)
        }
        Token::Symbol(':') | Token::RawLine(_) => true,
        _ => false,
    })
}
