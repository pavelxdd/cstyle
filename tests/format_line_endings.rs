#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::format_with;
use cstyle::api::format_bytes;
use cstyle::config::{BraceStyle, FormatOptions, LineEnding, apply_command_line_args};

#[test]
fn writes_configured_line_break_between_formatted_lines() {
    let mut options = FormatOptions::default();
    options.line_ending = LineEnding::Crlf;
    let actual = format_with(fixture!("int main(){return 0;}"), &options);

    assert_eq!(
        actual,
        fixture!("int main()\r", "{\r", "    return 0;\r", "}\r")
    );
}

#[test]
fn cr_line_endings_do_not_double_after_brace_style_postprocess() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;

    assert_eq!(
        format_bytes(b"int a;\r", &options).expect("format bytes"),
        b"int a;\r"
    );
    assert_eq!(
        format_bytes(b"void f()\r{\rint a;\r}\r", &options).expect("format bytes"),
        b"void f()\r{\r    int a;\r}\r"
    );
}

#[test]
fn missing_final_line_break_preserves_unterminated_preprocessor_string_whitespace() {
    let source = b"#define TEXT \"open\\\nbody   ";
    let actual = format_bytes(source, &FormatOptions::default()).expect("format bytes");

    assert_eq!(actual, source);
}

#[test]
fn missing_final_line_break_preserves_unterminated_preprocessor_raw_literal_whitespace() {
    let source = b"#define TEXT R\"(\nbody   ";
    let actual = format_bytes(source, &FormatOptions::default()).expect("format bytes");

    assert_eq!(actual, source);
}

#[test]
fn case_label_comment_body_keeps_missing_final_line_break() {
    let actual = format_bytes(
        b"void f(int x) {\n    switch (x) {\n    case A: // comment\nvalue();\n    }\n}",
        &FormatOptions::default(),
    )
    .expect("format bytes");

    assert_eq!(
        actual,
        b"void f(int x) {\n    switch (x) {\n    case A: // comment\n        value();\n    }\n}"
    );
}

#[test]
fn unterminated_raw_string_preserves_trailing_whitespace_at_eof() {
    let source = b"const char* text=R\"tag(\nvalue  ";
    let actual = format_bytes(source, &FormatOptions::default()).expect("format bytes");

    assert_eq!(actual, source);
}

#[test]
fn delete_empty_lines_keeps_final_line_break_in_unclosed_block() {
    let mut options = FormatOptions::default();
    let args = ["--delete-empty-lines", "--break-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_bytes(b"\nvoid foo()\n{\n    bar()\n    {\n\n}\n", &options,).expect("format bytes"),
        b"\nvoid foo()\n{\n    bar()\n    {\n    }\n",
    );
}

#[test]
fn form_feed_line_is_preserved_under_options_none() {
    let source = b"\nint value;\n\x0c\nvoid foo(void)\n{\n}\n";

    assert_eq!(
        format_bytes(source, &FormatOptions::default()).expect("format bytes"),
        source,
    );
}
