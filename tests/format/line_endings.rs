use crate::common::format_with;
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

#[test]
fn an_8_bit_source_measures_a_byte_past_ascii_as_its_utf8_width() {
    // An 8-bit source is read as Latin-1, and columns count the UTF-8
    // width of the text read: two for each byte past ASCII.
    let source = b"void f(void)\n{\n    call(first_value, \"\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\", second_value);\n}\n";
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--max-code-length=50".to_owned()])
        .expect("valid options");
    assert_eq!(
        format_bytes(source, &options).expect("format bytes"),
        b"void f(void)\n{\n    call(first_value, \"\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\xe9\",\n         second_value);\n}\n",
    );
}

#[test]
fn blanks_outside_ascii_stay_as_text() {
    // Only ASCII blanks part C tokens; any other blank is text the
    // formatter keeps, in UTF-8 and in 8-bit sources alike, and text after
    // a statement on its line starts one of its own.
    let source = "void f(void)\n{\n    x = 1;\u{a0}\n    \u{a0}y = 2;\n    w = 3;\u{3000}\n}\n";
    let expected =
        "void f(void)\n{\n    x = 1;\n    \u{a0}\n    \u{a0}y = 2;\n    w = 3;\n    \u{3000}\n}\n";
    assert_eq!(
        format_bytes(source.as_bytes(), &FormatOptions::default()).expect("format bytes"),
        expected.as_bytes(),
    );
    let source = b"void f(void)\n{\n    x = 1;\xa0\n    \xa0y = 2;\n    z = \x85;\n}\n";
    assert_eq!(
        format_bytes(source, &FormatOptions::default()).expect("format bytes"),
        b"void f(void)\n{\n    x = 1;\n    \xa0\n    \xa0y = 2;\n    z = \x85;\n}\n",
    );
}
