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
use crate::formatter::lexer::{Token, tokenize};
use crate::formatter::text::tabs;
use crate::source::line_endings;

mod braces;
mod constructs;
mod continuation;
mod engine;
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
    let input = line_endings::normalize(source);
    let converted_source = options
        .convert_tabs
        .then(|| tabs::source_to_spaces(&input, options.tab_width));
    let source = converted_source.as_deref().unwrap_or(&input);
    let tokens = tokenize(source);
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
    engine.set_may_have_backslash_body(input.contains('\\'));
    engine.set_may_have_swig(input.contains('%'));
    engine.may_have_class_base_access = class_declarations::has_base_access_token(&tokens);
    engine.preprocessor.may_have_preprocessor = input.contains('#')
        || tokens
            .iter()
            .any(|token| matches!(token, Token::Preprocessor(_)));
    let output = postprocess_brace_style(engine.format_into(&tokens).finish(), options);
    if options.empty_line_fill
        && matches!(
            options.brace_style,
            BraceStyle::Pico | BraceStyle::Lisp | BraceStyle::Horstmann
        )
    {
        refill_empty_lines(&output, options.line_break())
    } else {
        output
    }
}

/// Brace styles that move lines after the output is finished leave filled
/// empty lines with the indent of a line that moved: they take the indent of
/// the line now before them.
fn refill_empty_lines(output: &str, line_break: &str) -> String {
    let mut lead = "";
    let mut lines = Vec::new();
    // The rest of a block comment keeps the indent of its first line.
    let mut in_block_comment = false;
    for line in output.split(line_break) {
        if line.contains('\u{c}') {
            lines.push(line);
        } else if line.trim().is_empty() {
            lines.push(if line.is_empty() { "" } else { lead });
        } else {
            if !in_block_comment && !line.trim_start().starts_with('#') {
                lead = &line[..line.len() - line.trim_start().len()];
            }
            in_block_comment = ends_inside_block_comment(line, in_block_comment);
            lines.push(line);
        }
    }
    lines.join(line_break)
}

/// Whether a block comment is open at the end of `line`, given whether one
/// was open at its start.
fn ends_inside_block_comment(line: &str, mut in_block_comment: bool) -> bool {
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
