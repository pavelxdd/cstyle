//! Shared fixtures compile into every integration test target; a helper is
//! unused in some targets by construction.
#![allow(dead_code)]
#![allow(clippy::field_reassign_with_default)]

use cstyle::api;
use cstyle::config::{BraceStyle, FormatOptions, PointerAlign};

#[allow(unused_macros)]
macro_rules! fixture {
    ($($line:expr),+ $(,)?) => {
        concat!($($line, "\n"),+)
    };
}

pub fn format(source: &str) -> String {
    format_with(source, &FormatOptions::default())
}

pub fn format_c(source: &str, options: &FormatOptions) -> String {
    api::format(source, options)
}

pub fn format_with(source: &str, options: &FormatOptions) -> String {
    let mut options = options.clone();
    if options.brace_style == BraceStyle::None {
        options.brace_style = BraceStyle::Allman;
    }
    if !has_padding_option(&options) {
        options.pad_operators = true;
        options.pad_commas = true;
        options.pad_header = true;
        if options.pointer_align == PointerAlign::None {
            options.pointer_align = PointerAlign::Name;
        }
    }
    api::format(source, &options)
}

fn has_padding_option(options: &FormatOptions) -> bool {
    options.pad_operators
        || options.pad_commas
        || options.pad_parens_outside
        || options.pad_first_paren_outside
        || options.pad_parens_inside
        || options.pad_header
        || options.unpad_parens
}
