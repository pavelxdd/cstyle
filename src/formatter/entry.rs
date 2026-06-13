use super::{
    FormatEngine,
    brace_postprocess::postprocess_brace_style,
    class_declarations, tabs,
    token::{Token, tokenize},
};
use crate::config::FormatOptions;
use crate::source::line_endings;

pub(crate) fn format_c(source: &str, options: &FormatOptions) -> String {
    let input = line_endings::normalize(source);
    let converted_source = options
        .convert_tabs
        .then(|| tabs::source_to_spaces(&input, options.tab_width));
    let source = converted_source.as_deref().unwrap_or(&input);
    let tokens = tokenize(source);
    let mut engine = FormatEngine::new(options);
    engine.line_adjuster.set_tab_conversion_enabled(false);
    if !case_adjustments_needed_for_tokens(&tokens, options) {
        engine.line_adjuster.set_case_processing_enabled(false);
    }
    if !line_observer_needed_for_tokens(&tokens, options) {
        engine.line_adjuster.set_line_observe_enabled(false);
    }
    engine.set_may_have_backslash_body(input.contains('\\'));
    engine.set_may_have_swig(input.contains('%'));
    engine.may_have_class_base_access = class_declarations::has_base_access_token(&tokens);
    engine.preprocessor.may_have_preprocessor = input.contains('#')
        || tokens
            .iter()
            .any(|token| matches!(token, Token::Preprocessor(_)));
    postprocess_brace_style(engine.format_into(&tokens).finish(), options)
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
