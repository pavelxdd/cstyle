#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

#[test]
fn unterminated_delimited_raw_string_uses_block_indent() {
    assert_eq!(
        format_c(
            fixture!("void run()", "{", "string value  =   R\"~(raw"),
            &FormatOptions::default(),
        ),
        fixture!("void run()", "{", "    string value  =   R\"~(raw")
    );
}

#[test]
fn malformed_raw_string_opener_is_preserved() {
    let source = fixture!("void run()", "{", "    string value = R\"raw");

    assert_eq!(format_c(&source, &FormatOptions::default()), source,);
}

#[test]
fn identifier_adjacent_string_literal_stays_glued() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void f() {", "\tcall(fd, MACRO\"=%d\\n\", x);", "}"),
            &options,
        ),
        fixture!("void f()", "{", "\tcall(fd, MACRO\"=%d\\n\", x);", "}")
    );
}

#[test]
fn cast_adjacent_string_literal_stays_glued() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
            "--unpad-paren".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void f() {", "\tcall(node, (Type*)\"name\", value);", "}",),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tcall(node, (Type*)\"name\", value);",
            "}"
        )
    );
}

#[test]
fn whitesmith_preserves_multiline_raw_string_body_columns() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    // raw query",
                "    output << R\"(",
                "first row",
                "second row",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    // raw query",
            "    output << R\"(",
            "first row",
            "second row",
        )
    );
}

#[test]
fn preserves_prefixed_raw_and_continued_literals() {
    let actual = format(fixture!(
        "auto s=R\"(if(x){y();}// keep)\";",
        "auto w=L\"x\"; auto u=u8\"x\"; char c=L'x';",
        "auto t=\"a\\",
        "b\";",
    ));

    assert_eq!(
        actual,
        fixture!(
            "auto s = R\"(if(x){y();}// keep)\";",
            "auto w = L\"x\";",
            "auto u = u8\"x\";",
            "char c = L'x';",
            "auto t = \"a\\",
            "b\";",
        )
    );
}

#[test]
fn convert_tabs_preserves_preprocessor_raw_literal_body() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    let source = "#define TEXT R\"(start   \nbody\t  \nend)\"\n";
    let actual = format_c(source, &options);

    assert_eq!(actual, source);
}

#[test]
fn keeps_digit_separators_inside_numbers() {
    let actual = format(fixture!("int f(){return 1'000+0xDEAD'BEEF;}"));
    assert_eq!(
        actual,
        fixture!("int f()", "{", "    return 1'000 + 0xDEAD'BEEF;", "}",)
    );
}

#[test]
fn pad_operators_preserves_leading_dot_float_adjacency_after_word() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(format_c("return.5;\n", &options), "return.5;\n");
}

#[test]
fn pad_operators_treats_leading_dot_float_as_one_numeric_token() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("double value=.5f;\n", &options),
        "double value = .5f;\n"
    );
}

#[test]
fn preprocessor_string_comment_marker_does_not_swallow_following_code() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("#define TEXT \"/*\"\nint value=1;\n", &options),
        "#define TEXT \"/*\"\nint value = 1;\n"
    );
}

#[test]
fn preprocessor_raw_string_comment_marker_does_not_swallow_following_code() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("#define TEXT R\"(/*)\"\nint value=1;\n", &options),
        "#define TEXT R\"(/*)\"\nint value = 1;\n"
    );
}

#[test]
fn distinguishes_char_literals_from_digit_separators() {
    let actual = format(fixture!("int f(){return 'a'+'\\'';}"));
    assert_eq!(
        actual,
        fixture!("int f()", "{", "    return 'a' + '\\'';", "}",)
    );
}

#[test]
fn preserves_continued_char_literals() {
    let actual = format(fixture!("int f(){char c='a\\", "b';return 0;}",));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    char c = 'a\\",
            "b';",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn preserves_continued_string_literals_and_following_comments() {
    let actual = format(fixture!(
        "int f(){const char*s=\"a\\",
        "b\"; // keep",
        "return 0;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    const char *s = \"a\\",
            "b\"; // keep",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn semicolon_in_string_with_trailing_comment_keeps_concat_continuation() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "static const char x[] =\n\t\"a;b\" /* c */\n\t\"de\"\n\t\"end\";\n",
            &options,
        ),
        "static const char x[] =\n    \"a;b\" /* c */\n    \"de\"\n    \"end\";\n",
    );
}

#[test]
fn string_literal_keywords_do_not_leak_indent_to_string_concat_rows() {
    let source = "const char* words =\n    \"alpha beta \"\n    \"struct switch template this throw true try typedef typeid \"\n    \"typename union unsigned \"\n    \"while\";\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn aligns_unclosed_string_continuation_lines() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(){",
            "value = \"alpha=",
            "        beta \"",
            "        \"gamma\";",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    value = \"alpha=",
            "            beta \"",
            "            \"gamma\";",
            "}",
        )
    );
}

#[test]
fn preserves_literals_with_comment_markers_and_tabs() {
    let actual = format(fixture!(
        "void f(){\tconst char*s=\"// not a comment\";\tconst char*t=\"a\tb\";char c='\\t';}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    const char *s = \"// not a comment\";",
            "    const char *t = \"a\tb\";",
            "    char c = '\\t';",
            "}",
        )
    );
}

#[test]
fn preserves_escaped_string_continuations() {
    let actual = format(fixture!("void f(){const char*s=\"a\\", "    b\";}",));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    const char *s = \"a\\",
            "    b\";",
            "}",
        )
    );
}

#[test]
fn unterminated_literals_stop_at_newline_without_corrupting_indent() {
    let actual = format(fixture!(
        "int f(){const char*s=\"unterminated",
        "return 0;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    const char *s = \"unterminated",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn escaped_backslashes_do_not_force_quote_continuation() {
    let actual = format(fixture!(
        "int f(){const char*s=\"two slash\\\\",
        "return 0;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    const char *s = \"two slash\\\\",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn string_literal_continuation_after_comment_and_preprocessor_keeps_assignment_column() {
    assert_eq!(
        format_c(
            "const char value[] = \"base:\"\n        // text\n\n#ifdef A\n\" a\"\n#endif\n;\n",
            &FormatOptions::default(),
        ),
        "const char value[] = \"base:\"\n                     // text\n\n#ifdef A\n                     \" a\"\n#endif\n                     ;\n",
    );
}

#[test]
fn equals_in_string_literal_does_not_trigger_parameter_default_continuation() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (!flag &&\n      (cmp (m[i], \"a=b\") == 0 ||\n       cmp (m[i], \"yy\") == 0)\n      ) {\n    go ();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (!flag &&\n            (cmp (m[i], \"a=b\") == 0 ||\n             cmp (m[i], \"yy\") == 0)\n       ) {\n        go ();\n    }\n}\n",
    );
}

#[test]
fn string_literal_adjacent_macro_word_keeps_no_space() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  call (\"%\"FMT, x);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    call (\"%\"FMT, x);\n}\n",
    );
}

#[test]
fn adjacent_string_array_element_ignores_using_inside_string_literal() {
    assert_eq!(
        format_c(
            "static Item items[] =\n  {\n    {\n      \"now fitting alpha into beta using gamma value here\",\n      \"this is a long string literal that goes well past the continuation indent limit for sure ok yes \"\n      \"and a second adjacent literal\",\n    },\n  };\n",
            &FormatOptions::default(),
        ),
        "static Item items[] =\n{\n    {\n        \"now fitting alpha into beta using gamma value here\",\n        \"this is a long string literal that goes well past the continuation indent limit for sure ok yes \"\n        \"and a second adjacent literal\",\n    },\n};\n",
    );
}

#[test]
fn escaped_multiline_string_keeps_embedded_quote_adjacency() {
    let source = "const char *z = \"\\\n AND (clean(text,'0123456790') test '*\"GENERIC_MARKER\"'),\\\n size-extent(clean(text, '\"GENERIC_MARKER\"0123456789')),\\\n\";\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn declaration_after_escaped_multiline_string_uses_body_indent() {
    assert_eq!(
        format_c(
            "void f(void){\n  static const char *a = \"\\\nSOURCE\\\n)\";\n  static const char *b =\n    \"SOURCE\";\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    static const char *a = \"\\\nSOURCE\\\n)\";\n    static const char *b =\n        \"SOURCE\";\n}\n",
    );
}

// Raw-string contents cannot change switch structure.
#[test]
fn multiline_raw_string_case_expression_keeps_switch_structure() {
    assert_eq!(
        format_c(
            "void f(int x){\nswitch(x){\ncase hash(R\"tag(a\nb\n)tag\"): call();\n}\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int x) {\n    switch(x) {\n    case hash(R\"tag(a\nb\n)tag\"):\n        call();\n    }\n}\n",
    );
}

#[test]
fn multiline_raw_string_case_block_keeps_following_switch_state() {
    assert_eq!(
        format_c(
            "void f(int x){\nswitch(x){\ncase hash(R\"tag(a\nb\n)tag\"): {\ncall();\n}\ncase 2: call();\n}\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int x) {\n    switch(x) {\n    case hash(R\"tag(a\nb\n)tag\"): {\n        call();\n    }\n    case 2:\n        call();\n    }\n}\n",
    );
}

#[test]
fn raw_string_colon_inside_case_expression_is_not_a_label_separator() {
    assert_eq!(
        format_c(
            fixture!("void f(int x){switch(x){case hash(R\"(a\":b)\"):call();}}"),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f(int x) {",
            "    switch(x) {",
            "    case hash(R\"(a\":b)\"):",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn convert_tabs_preserves_tabs_after_quotes_inside_raw_strings() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    let source = fixture!("const char* text=R\"tag(a\"\tb)tag\";");

    assert_eq!(format_c(source, &options), source);
}

// Case layout does not alter raw-string body indentation.
#[test]
fn case_adjustment_preserves_multiline_raw_string_body() {
    assert_eq!(
        format_c(
            "void f(int x){\nswitch(x){\ncase 1:\n{\nconst char* text=R\"tag(\n        raw\n    raw2\n)tag\";\ncall();\n}\n}\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int x) {\n    switch(x) {\n    case 1:\n    {\n        const char* text=R\"tag(\n        raw\n    raw2\n)tag\";\n        call();\n    }\n    }\n}\n",
    );
}

#[test]
fn multiline_raw_string_preserves_opening_line_content_whitespace() {
    let source = "const char* text=R\"tag(content   \nnext\n)tag\";\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multiline_raw_string_preserves_trailing_spaces_and_tabs() {
    let source = "const char* text=R\"tag(\nraw  \nvalue\t\nend   )tag\";\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn run_in_brace_styles_preserve_isolated_braces_inside_raw_strings() {
    let source = "const char* text=R\"tag(\n{\nraw\n}\n)tag\";\n";
    for style in ["horstmann", "pico", "lisp"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")]).expect("valid style");

        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn raw_opening_line_open_brace_does_not_change_later_if_else_structure() {
    assert_eq!(
        format_c(
            "void f(){\nif(x){\nconst char* text=R\"(a\" {\nbody\n)\";\ncall();\n}else{\nother();\n}\nafter();\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    if(x) {\n        const char* text=R\"(a\" {\nbody\n)\";\n        call();\n    } else {\n        other();\n    }\n    after();\n}\n",
    );
}

#[test]
fn raw_opening_line_close_brace_does_not_change_later_switch_structure() {
    assert_eq!(
        format_c(
            "void f(int x){\nswitch(x){\ncase 1:{\nconst char* text=R\"(a\" }\nbody\n)\";\ncall();\n}\ncase 2: other();\n}\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int x) {\n    switch(x) {\n    case 1: {\n        const char* text=R\"(a\" }\nbody\n)\";\n        call();\n    }\n    case 2:\n        other();\n    }\n}\n",
    );
}

#[test]
fn multiline_raw_string_code_like_lines_remain_opaque() {
    assert_eq!(
        format_c(
            "void f(){\nconst char* text=R\"tag(\n#if FLAG  \n{  \ncase 1:  \n} // tail  \n)tag\";\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    const char* text=R\"tag(\n#if FLAG  \n{  \ncase 1:  \n} // tail  \n)tag\";\n}\n",
    );
}

#[test]
fn macro_block_adjustment_preserves_multiline_raw_string_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--macro-block=BEGIN_BLOCK:END_BLOCK".to_owned()],
    )
    .expect("valid macro block");

    assert_eq!(
        format_c(
            "BEGIN_BLOCK()\nconst char* text=R\"tag(\nraw\n  raw2\n)tag\";\nEND_BLOCK()\n",
            &options,
        ),
        "BEGIN_BLOCK()\n    const char* text=R\"tag(\nraw\n  raw2\n)tag\";\nEND_BLOCK()\n",
    );
}

#[test]
fn convert_tabs_tracks_multiline_raw_string_boundaries() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;

    assert_eq!(
        format_c(
            "const char* text=R\"tag(\n\ta\"\tb\n\tc\n)tag\";\nint\tvalue;\n",
            &options,
        ),
        "const char* text=R\"tag(\n\ta\"\tb\n\tc\n)tag\";\nint value;\n",
    );
}

// Earlier raw literals cannot change later switch indentation.
#[test]
fn raw_string_before_switch_does_not_change_case_block_indent() {
    assert_eq!(
        format_c(
            "namespace\n{\nconst char* data = R\"x(\ntext\n)x\";\n}\n\nvoid f(int x)\n{\n    switch ( x )\n    {\n    case 1:\n#if A\n    case 2:\n#endif\n        {\n            int value = 1;\n        }\n        break;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "namespace\n{\nconst char* data = R\"x(\ntext\n)x\";\n}\n\nvoid f(int x)\n{\n    switch ( x )\n    {\n    case 1:\n#if A\n    case 2:\n#endif\n    {\n        int value = 1;\n    }\n    break;\n    }\n}\n",
    );
}

// A line break ends malformed string state; later lines are formatted uniformly.
#[test]
fn unterminated_string_does_not_leak_continuation_indent() {
    let source = "char* s = \"open;\nint y;\nint z=1;\nmore\";\nint after;\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn indented_statement_after_raw_string_in_preprocessor_keeps_source_indent() {
    let source = "#ifdef A\n    static const char* a = R\"x(\ntext\n)x\";\n\n    static const char* b = R\"x(\ntext\n)x\";\n#endif\n";
    let expected = "#ifdef A\nstatic const char* a = R\"x(\ntext\n)x\";\n\n    static const char* b = R\"x(\ntext\n)x\";\n#endif\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

#[test]
fn string_literal_space_before_line_continuation_is_preserved() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tcall(\n\t\t\"a \" \\\n\t\t\"b\");\n}\n",
            &options,
        ),
        "void f(void)\n{\n    call(\n        \"a \" \\\n        \"b\");\n}\n",
    );
}
#[test]
fn adjacent_string_literal_macro_keeps_source_spacing() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  s = \"Count: %\"VALUE_FORMAT\"\\n\";\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    s = \"Count: %\"VALUE_FORMAT\"\\n\";\n}\n",
    );
}

#[test]
fn pico_preserves_multiline_raw_literal_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){auto text=R\"tag(first\n// raw\n/* raw */\nlast)tag\";call();}\n",
            &options,
        ),
        "void run()\n{   auto text=R\"tag(first\n// raw\n/* raw */\nlast)tag\"; call(); }\n",
    );
}

#[test]
fn split_after_multiline_raw_literal_does_not_add_trailing_space() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){auto text=R\"tag(first\n// raw\n/* raw */\nlast)tag\";call();}\n",
            &options,
        ),
        "void run()\n{\n    auto text=R\"tag(first\n// raw\n/* raw */\nlast)tag\";\n    call();\n}\n",
    );
}

#[test]
fn raw_literal_preprocessor_text_does_not_open_conditional_region() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-preproc-block".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "#define TEXT_VALUE R\"tag(alpha && \\\n#if raw\n)tag\"\nint value;\n";

    assert_eq!(format_c(source, &options), source);
}
