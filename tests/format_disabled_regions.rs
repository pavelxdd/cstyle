#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c};
use cstyle::config::{FormatOptions, apply_command_line_args};

const KR_C_ARGS: &[&str] = &[
    "--style=kr",
    "--mode=c",
    "--indent=spaces=4",
    "--indent-switches",
    "--indent-preprocessor",
    "--indent-preproc-define",
    "--indent-col1-comments",
    "--pad-oper",
    "--pad-comma",
    "--pad-header",
    "--unpad-paren",
    "--break-one-line-headers",
    "--keep-one-line-blocks",
    "--keep-one-line-statements",
    "--align-pointer=name",
    "--align-reference=name",
    "--min-conditional-indent=0",
    "--attach-closing-while",
    "--attach-return-type",
    "--attach-return-type-decl",
    "--convert-tabs",
    "--max-continuation-indent=80",
    "--max-code-length=100",
    "--break-after-logical",
];

#[test]
fn keeps_disabled_regions_unchanged() {
    let source = fixture!(
        "int f(){",
        "// *INDENT-OFF*",
        "  if(x){return 1;}",
        "// *INDENT-ON*",
        "return 0;}",
    );
    let actual = format(source);
    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "// *INDENT-OFF*",
            "  if(x){return 1;}",
            "// *INDENT-ON*",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn horstmann_does_not_run_in_indent_off_marker() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!(
        "void run(void){",
        "/* *INDENT-OFF* */",
        "// note",
        "  call();",
        "/* *INDENT-ON* */",
        "other();",
        "}",
    );
    let expected = fixture!(
        "void run(void)",
        "{",
        "/* *INDENT-OFF* */",
        "// note",
        "  call();",
        "/* *INDENT-ON* */",
        "    other();",
        "}",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn kr_c_options_preserve_indented_disabled_format_markers_inside_struct() {
    let mut options = FormatOptions::default();
    let args = KR_C_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    apply_command_line_args(&mut options, &args).expect("valid C options");
    let source = fixture!(
        "struct Item {",
        "    int value;",
        "    // *INDENT-OFF*",
        "    bool (*check)(void *);",
        "    // *INDENT-ON*",
        "};",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn disabled_regions_preserve_original_text_between_markers() {
    let source = fixture!(
        "int f(){// *INDENT-OFF*",
        "  if(x){return 1;}",
        "// *INDENT-ON*",
        "return 0;}",
    );
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f() // *INDENT-OFF*",
            "{",
            "    if (x)",
            "    {",
            "        return 1;",
            "    }",
            "// *INDENT-ON*",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn disabled_regions_ignore_braces_in_comments_and_strings_for_state() {
    let actual = format(fixture!(
        "int f(){",
        "/* *INDENT-OFF* */",
        "char*s=\"{\";",
        "/* { */",
        "/* *INDENT-ON* */",
        "return 0;",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "/* *INDENT-OFF* */",
            "char*s=\"{\";",
            "/* { */",
            "/* *INDENT-ON* */",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn resets_formatter_state_after_disabled_block_comments() {
    let actual = format(fixture!(
        "int f(){",
        "/* *INDENT-OFF* */",
        "if(x){return 1;}",
        "/* *INDENT-ON* */",
        "if(x){return 2;}",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "/* *INDENT-OFF* */",
            "if(x){return 1;}",
            "/* *INDENT-ON* */",
            "    if (x)",
            "    {",
            "        return 2;",
            "    }",
            "}",
        )
    );
}

#[test]
fn inline_indent_off_marker_does_not_disable_formatting() {
    let actual = format(fixture!(
        "int f(){// *INDENT-OFF*",
        "  if(x){return 1;}",
        "// *INDENT-ON*",
        "return 0;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f() // *INDENT-OFF*",
            "{",
            "    if (x)",
            "    {",
            "        return 1;",
            "    }",
            "// *INDENT-ON*",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn disabled_regions_preserve_unclosed_block_indentation_for_following_lines() {
    let actual = format(fixture!(
        "int f(){",
        "/* *INDENT-OFF* */",
        "if(x){",
        "raw();",
        "/* *INDENT-ON* */",
        "return 1;",
        "}",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "/* *INDENT-OFF* */",
            "if(x){",
            "raw();",
            "/* *INDENT-ON* */",
            "        return 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn disabled_regions_preserve_preprocessor_indentation_for_following_lines() {
    let actual = format(fixture!(
        "int f(){",
        "/* *INDENT-OFF* */",
        "#if A",
        "if(a){",
        "/* *INDENT-ON* */",
        "return 1;",
        "#endif",
        "}",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "/* *INDENT-OFF* */",
            "#if A",
            "if(a){",
            "/* *INDENT-ON* */",
            "        return 1;",
            "#endif",
            "    }",
            "}",
        )
    );
}

#[test]
fn disabled_line_markers_stop_indent_preproc_block_changes() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let source = fixture!(
        "#ifdef PLATFORM",
        "#define CALL __call",
        "// *INDENT-OFF*",
        "#else",
        "#define CALL",
        "#endif",
        "// *INDENT-ON*",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "#ifdef PLATFORM",
            "    #define CALL __call",
            "// *INDENT-OFF*",
            "#else",
            "#define CALL",
            "#endif",
            "// *INDENT-ON*",
        ),
    );
}

#[test]
fn disabled_line_markers_preserve_define_body_indentation_until_enabled() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!(
        "// *INDENT-OFF*",
        "#define DECLARE_NUMBER(name) \\",
        "        API_EXTERN extern ::core::Number SETTING(name)",
        "// *INDENT-ON*",
        "#define DECLARE_TEXT(name) \\",
        "        API_EXTERN extern ::text::String SETTING(name)",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "// *INDENT-OFF*",
            "#define DECLARE_NUMBER(name) \\",
            "        API_EXTERN extern ::core::Number SETTING(name)",
            "// *INDENT-ON*",
            "#define DECLARE_TEXT(name) \\",
            "    API_EXTERN extern ::text::String SETTING(name)",
        ),
    );
}

#[test]
fn disabled_block_markers_preserve_define_body_indentation_until_enabled() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!(
        "/*INDENT-OFF*/",
        "#define DECLARE_NUMBER(name) \\",
        "        API_EXTERN extern ::core::Number SETTING(name)",
        "/*INDENT-ON*/",
        "#define DECLARE_TEXT(name) \\",
        "        API_EXTERN extern ::text::String SETTING(name)",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "/*INDENT-OFF*/",
            "#define DECLARE_NUMBER(name) \\",
            "        API_EXTERN extern ::core::Number SETTING(name)",
            "/*INDENT-ON*/",
            "#define DECLARE_TEXT(name) \\",
            "    API_EXTERN extern ::text::String SETTING(name)",
        ),
    );
}

#[test]
fn pico_disabled_region_preserves_body_and_marker_boundaries() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n// *INDENT-OFF*\nif(alpha){call();}\n// *INDENT-ON*\nif(beta){call();}\n}\n",
            &options,
        ),
        "void run()\n{\n// *INDENT-OFF*\nif(alpha){call();}\n// *INDENT-ON*\n    if(beta) {call();} }\n",
    );
}
