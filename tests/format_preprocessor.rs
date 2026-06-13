#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{
    BraceStyle, FormatOptions, IndentStyle, MinConditionalIndent, apply_command_line_args,
};

const ONE_TRUE_BRACE_C_ARGS: &[&str] = &[
    "--style=1tbs",
    "--mode=c",
    "--indent=spaces=4",
    "--indent-switches",
    "--indent-preprocessor",
    "--indent-preproc-define",
    "--indent-col1-comments",
    "--add-braces",
    "--pad-oper",
    "--pad-comma",
    "--pad-header",
    "--unpad-paren",
    "--break-one-line-headers",
    "--break-after-logical",
    "--align-pointer=name",
    "--align-reference=name",
    "--attach-closing-while",
    "--attach-return-type",
    "--attach-return-type-decl",
    "--min-conditional-indent=0",
    "--max-continuation-indent=80",
    "--max-code-length=109",
    "--convert-tabs",
];

fn one_true_brace_c_options() -> FormatOptions {
    let mut options = FormatOptions::default();
    let args = ONE_TRUE_BRACE_C_ARGS
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect::<Vec<_>>();
    apply_command_line_args(&mut options, &args).expect("valid C options");
    options
}

#[test]
fn inline_preprocessor_after_opening_brace_stays_at_column_zero() {
    assert_eq!(
        format_c(
            "void run(){#if READY\nstep();\n#endif\n}\n",
            &FormatOptions::default(),
        ),
        "void run() {\n#if READY\n    step();\n#endif\n}\n",
    );
}

#[test]
fn pico_keeps_closing_brace_after_preprocessor_on_own_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run() {\n#if READY\nif (ready) { step(); }\n#endif\n}\n",
            &options,
        ),
        "void run()\n{\n#if READY\n    if (ready) { step(); }\n#endif\n}\n",
    );
}

#[test]
fn keeps_embedded_define_with_brace_and_else_marker_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let expected = "autotry#define X(x) \\ {\t#elsecatch;\n#else\n";

    assert_eq!(
        format_c("autotry#define X(x) \\{\t#elsecatch;#else\n", &options,),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn keeps_embedded_define_with_malformed_else_continuation_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let expected =
        "autotry#define X(x) \\ {\t#elsecatch;\n#elsehelperenum/,  <=if&&\n    try  #else\n";

    assert_eq!(
        format_c(
            "autotry#define X(x) \\{\t#elsecatch;#elsehelperenum/,  <=if&&\ntry  #else\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn allman_keeps_malformed_define_after_bracket_brace_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let expected = "xconstexpr  \t0z((do\n} Config=!!=/<=||10&[+=#if A[ {#define X(x) \\<&&\n";

    assert_eq!(
        format_c(
            "xconstexpr  \t0z((do}Config=!!=/<=||10&[+=#if A[{#define X(x) \\<&&\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn gnu_keeps_close_word_after_malformed_define_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected =
        "+  !do&)(&#define X(x) \\...defaulttry->==xNULL[1\n    call\n} x0->]  gamma>=classy>\n";

    assert_eq!(
        format_c(
            "+  !do&)(&#define X(x) \\...defaulttry->==xNULL[1\ncall}x0->]  gamma>=classy>\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn indent_preproc_define_preserves_source_indented_initializer_closer() {
    let source = fixture!(
        "#define ITEM(name, value) \\",
        "    static struct item item_##name \\",
        "        = { \\",
        "            .name = (name), \\",
        "            .value = (value), \\",
        "          }",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn indent_preproc_define_preserves_source_indented_closing_parenthesis() {
    let source = fixture!(
        "#define FLAGS (FLAG_ALPHA \\",
        "               | FLAG_BETA \\",
        "              )",
        "",
        "#define VERSIONS ((1u << VERSION_ALPHA) | \\",
        "                  (1u << VERSION_BETA) | \\",
        "                  (1u << VERSION_GAMMA))",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn indent_preproc_define_preserves_source_indented_ternary_arm() {
    let source = fixture!(
        "#define ITEM(args) \\",
        "    static const Item item = { \\",
        "        .value = ((const void *)(args) != nullptr) \\",
        "                  ? (int)(sizeof(args) - 1) : -1, \\",
        "    };",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn conditional_branch_else_if_keeps_following_sibling_indent() {
    let actual = format_c(
        fixture!(
            "void f(int z){",
            "  if(z==0){",
            "    int k;",
            "    for(k=0; k<3; k++){",
            "      if(ok(k)){",
            "        done();",
            "        break;",
            "      }",
            "    }",
            "    if(k>=3){",
            "      fail();",
            "    }",
            "#ifdef A",
            "  }else if(z==1){",
            "    one();",
            "#endif",
            "  }else if(z==2){",
            "    two();",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int z) {",
            "    if(z==0) {",
            "        int k;",
            "        for(k=0; k<3; k++) {",
            "            if(ok(k)) {",
            "                done();",
            "                break;",
            "            }",
            "        }",
            "        if(k>=3) {",
            "            fail();",
            "        }",
            "#ifdef A",
            "    } else if(z==1) {",
            "        one();",
            "#endif",
            "    } else if(z==2) {",
            "        two();",
            "    }",
            "}",
        )
    );
}

#[test]
fn preprocessor_else_branch_does_not_take_split_else_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "#if A",
        "    if (x)",
        "    {",
        "        a();",
        "    }",
        "    else",
        "#else",
        "    unused(event);",
        "#endif",
        "    {",
        "        b();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn line_after_preprocessor_branch_in_block_uses_block_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f() {",
                "#ifdef A",
                "  if (ready()) return;",
                "#endif",
                "  auto value = T();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f() {",
            "#ifdef A",
            "    if (ready()) return;",
            "#endif",
            "    auto value = T();",
            "}",
        )
    );
}

#[test]
fn braceless_if_global_call_keeps_else_indent_after_preprocessor_branch() {
    let source = fixture!(
        "void g()",
        "{",
        "#if A",
        "    if (x)",
        "    {",
        "        a();",
        "    }",
        "    else",
        "    {",
        "        b();",
        "    }",
        "#else",
        "    c();",
        "#endif",
        "}",
        "",
        "void f()",
        "{",
        "#if A",
        "    if (x)",
        "        ::g();",
        "    else",
        "    {",
        "        b();",
        "    }",
        "#else",
        "    c();",
        "#endif",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preprocessor_split_else_if_keeps_parent_if_indent() {
    let source = fixture!(
        "void f(void){",
        "  if( a ){",
        "    one();",
        "#ifdef DEBUG",
        "  }else if( b ){",
        "    two();",
        "#endif",
        "#ifdef EXTRA",
        "  }else if( c ){",
        "    three();",
        "#endif",
        "  }else if( d ){",
        "    four();",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(void) {",
            "    if( a ) {",
            "        one();",
            "#ifdef DEBUG",
            "    } else if( b ) {",
            "        two();",
            "#endif",
            "#ifdef EXTRA",
            "    } else if( c ) {",
            "        three();",
            "#endif",
            "    } else if( d ) {",
            "        four();",
            "    }",
            "}",
        )
    );
}

#[test]
fn top_level_comment_and_declaration_after_preprocessor_stay_unindented() {
    let source = "#if A\n#  define F_OPEN(name, mode) \\\n     fopen((name), (mode), \"x\")\n#endif\n\n#if B\n#  define OS_CODE 1\n#endif\n\n/* comment */\n#ifndef C\n   DECL value;\n#endif\n";

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        "#if A\n#  define F_OPEN(name, mode) \\\n     fopen((name), (mode), \"x\")\n#endif\n\n#if B\n#  define OS_CODE 1\n#endif\n\n/* comment */\n#ifndef C\nDECL value;\n#endif\n"
    );
}

#[test]
fn define_body_unmatched_close_brace_stays_at_body_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_preproc_define = true;

    assert_eq!(
        format_c(
            "#define BLOCK_END \\\n    restore(state); \\\n} while (0)\n",
            &options,
        ),
        "#define BLOCK_END \\\n    restore(state); \\\n    } while (0)\n"
    );
}

#[test]
fn preprocessor_call_continuation_over_max_uses_block_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "#if X",
                "    {",
                "        Type value = VeryLongFunctionNameThatExceedsLimit(first,",
                "            MACRO(\"value\"), false);",
                "    }",
                "#endif",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "#if X",
            "    {",
            "        Type value = VeryLongFunctionNameThatExceedsLimit(first,",
            "                MACRO(\"value\"), false);",
            "    }",
            "#endif",
            "}",
        )
    );
}

#[test]
fn conditional_branch_multiline_header_keeps_closing_paren_indent() {
    let actual = format_c(
        fixture!(
            "int f(void) {",
            "#if defined(PLATFORM)",
            "  if( first()",
            "   || second()",
            "   || third()",
            "  ){",
            "    return 1;",
            "  }",
            "#endif",
            "  return 0;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(void) {",
            "#if defined(PLATFORM)",
            "    if( first()",
            "            || second()",
            "            || third()",
            "      ) {",
            "        return 1;",
            "    }",
            "#endif",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn indent_preproc_block_indents_namespace_members_without_namespace_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "namespace sample{",
            "#if ENABLED",
            "void run();",
            "#else",
            "void other();",
            "#endif",
            "void tail();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace sample",
            "{",
            "#if ENABLED",
            "    void run();",
            "#else",
            "    void other();",
            "#endif",
            "void tail();",
            "}",
        )
    );
}

#[test]
fn indent_preproc_block_does_not_unindent_nested_namespace_members() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    options.indent_namespaces = true;
    let actual = format_with(
        fixture!(
            "namespace outer{",
            "namespace inner{",
            "#if ENABLED",
            "int value;",
            "#endif",
            "}",
            "}",
        ),
        &options,
    );

    // Preprocessor block indentation is additive to namespace indentation.
    assert_eq!(
        actual,
        fixture!(
            "namespace outer",
            "{",
            "    namespace inner",
            "    {",
            "#if ENABLED",
            "        int value;",
            "#endif",
            "    }",
            "}",
        )
    );
}

#[test]
fn whitesmith_indent_preproc_block_indents_namespace_members() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let actual = format_with(
        fixture!(
            "namespace sample{",
            "#if ENABLED",
            "void run();",
            "#endif",
            "void tail();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace sample",
            "{",
            "#if ENABLED",
            "    void run();",
            "#endif",
            "void tail();",
            "}",
        )
    );
}

#[test]
fn bare_less_than_in_if_zero_uses_structural_indent() {
    let source = fixture!(
        "class C",
        "{",
        "#if 0",
        "    <   // fake",
        "    // more",
        "#endif",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn indent_preproc_block_keeps_guard_with_trailing_line_comment_unindented() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let source = fixture!(
        "#ifndef SAMPLE_H",
        "#define SAMPLE_H",
        "int value;",
        "#endif",
        "// trailing comment",
    );
    let actual = format_with(source, &options);

    // Trailing comment kind does not change include-guard classification.
    assert_eq!(actual, source);
}

#[test]
fn indent_preproc_block_column_one_comment_does_not_disable_code_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!("#if ENABLED", "// comment", "int value;", "#endif",),
        &options,
    );

    // Column-one comments do not suppress code indentation.
    assert_eq!(
        actual,
        fixture!("#if ENABLED", "// comment", "    int value;", "#endif",)
    );
}

#[test]
fn indent_preproc_block_preserves_column_one_comment_in_namespace() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "namespace alpha{",
            "#if ENABLED",
            "// comment",
            "int value;",
            "#endif",
            "}",
        ),
        &options,
    );

    // Column-one comments move only when `indent_col1_comments` is enabled.
    assert_eq!(
        actual,
        fixture!(
            "namespace alpha",
            "{",
            "#if ENABLED",
            "// comment",
            "    int value;",
            "#endif",
            "}",
        )
    );
}

#[test]
fn indent_preproc_block_handles_c23_branch_separators() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#ifdef ALPHA",
            "int alpha;",
            "#elifdef BETA",
            "int beta;",
            "#elifndef GAMMA",
            "int other;",
            "#endif",
        ),
        &options,
    );

    // C23 branch separators use the active directive column.
    assert_eq!(
        actual,
        fixture!(
            "#ifdef ALPHA",
            "    int alpha;",
            "#elifdef BETA",
            "    int beta;",
            "#elifndef GAMMA",
            "    int other;",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_define_keeps_case_at_switch_level() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!(
        "#define BODY(x) \\",
        "switch(x){ \\",
        "case 1:{return 1;} \\",
        "}",
    );
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "#define BODY(x) \\",
            "    switch(x){ \\",
            "    case 1:{return 1;} \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_with_indent_switches_keeps_outermost_case_at_switch_level() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_switches = true;
    options.pad_operators = true;
    let source = fixture!(
        "#define HANDLE(value) \\",
        "switch (value) { \\",
        "case 1: \\",
        "result = alpha; \\",
        "break; \\",
        "default: \\",
        "result = beta; \\",
        "}",
    );
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "#define HANDLE(value) \\",
            "    switch (value) { \\",
            "    case 1: \\",
            "        result = alpha; \\",
            "        break; \\",
            "    default: \\",
            "        result = beta; \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_with_indent_switches_indents_nested_case() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_switches = true;
    options.pad_operators = true;
    let source = fixture!(
        "#define HANDLE(value) \\",
        "{ \\",
        "switch (value) { \\",
        "case 1: \\",
        "result = alpha; \\",
        "} \\",
        "}",
    );
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "#define HANDLE(value) \\",
            "    { \\",
            "        switch (value) { \\",
            "            case 1: \\",
            "                result = alpha; \\",
            "        } \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_preserves_body_content_without_padding() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.pad_operators = true;
    let source = fixture!("#define ADD(a, b) \\", "    result=a+b;",);
    let actual = format_with(source, &options);

    assert_eq!(actual, fixture!("#define ADD(a, b) \\", "    result=a+b;",));
}

#[test]
fn indent_preproc_define_keeps_one_line_do_while_body() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!("#define ONCE(x) \\", "    do { x; } while (0)",);
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!("#define ONCE(x) \\", "    do { x; } while (0)",)
    );
}

#[test]
fn indent_preproc_define_indents_nested_braces_and_braceless_headers() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!(
        "#define LOOP \\",
        "while(a){ \\",
        "if(b) \\",
        "c(); \\",
        "}",
    );
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "#define LOOP \\",
            "    while(a){ \\",
            "        if(b) \\",
            "            c(); \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_statement_expression_body_keeps_block_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.pad_operators = true;
    // Each statement resets continuation alignment to the block body.
    let source = fixture!(
        "#define M(ID, EXPR) \\",
        "    (__extension__({ \\",
        "        uint32_t v = (uint32_t)(EXPR); \\",
        "        touch((unsigned)(ID)); \\",
        "        v; \\",
        "    }))",
    );
    let actual = format_with(source, &options);

    assert_eq!(actual, source);
}

#[test]
fn indent_preproc_define_keeps_designated_initializer_rows_aligned() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    // Each designated initializer row uses the macro-body column.
    let source = fixture!(
        "#define ITEM(name, value) \\",
        "    { \\",
        "        .name = (name), \\",
        "        .value = (value), \\",
        "        .help = get_help(name), \\",
        "    }",
    );
    let actual = format_with(source, &options);

    assert_eq!(actual, source);
}

#[test]
fn indent_preproc_define_aligns_assignment_continuation() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!("#define SUM \\", "    value = alpha + \\", "    beta;",);
    let actual = format_with(source, &options);

    assert_eq!(
        actual,
        fixture!(
            "#define SUM \\",
            "    value = alpha + \\",
            "            beta;",
        )
    );
}

#[test]
fn preprocessor_branches_preserve_standalone_call_closer_alignment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(){",
            "if(x){",
            "call(",
            "\"one\"",
            "#if A",
            "\"two\"",
            "#else",
            "\"three\"",
            "#endif",
            "\"four\"",
            ");",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (x) {",
            "        call(",
            "            \"one\"",
            "#if A",
            "            \"two\"",
            "#else",
            "            \"three\"",
            "#endif",
            "            \"four\"",
            "        );",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_preserves_continuation_space_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int f(){return sum(alpha,",
            "#if A",
            "beta,",
            "#else",
            "gamma,",
            "#endif",
            "delta);}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return sum(alpha,",
            "               #if A",
            "               beta,",
            "               #else",
            "               gamma,",
            "               #endif",
            "               delta);",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_uses_control_header_owner() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "void run(){",
            "if(alpha&&",
            "#if ENABLED",
            "beta&&",
            "#endif",
            "gamma){",
            "call();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "{",
            "    if (alpha &&",
            "        #if ENABLED",
            "            beta &&",
            "        #endif",
            "            gamma)",
            "    {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_keeps_structural_tabs_in_control_header() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    let actual = format_with(
        fixture!(
            "void run(){",
            "if(alpha&&",
            "#if ENABLED",
            "beta&&",
            "#endif",
            "gamma){",
            "call();",
            "}",
            "}",
        ),
        &options,
    );

    // Directives inside continued headers retain configured structural tabs.
    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "{",
            "\tif (alpha &&",
            "\t\t#if ENABLED",
            "\t        beta &&",
            "\t\t#endif",
            "\t        gamma)",
            "\t{",
            "\t\tcall();",
            "\t}",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_with_force_tabs_uses_configured_prefix() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int f(){return sum(alpha,",
            "#if A",
            "beta,",
            "#endif",
            "delta);}",
        ),
        &options,
    );

    let lines: Vec<&str> = actual.lines().collect();
    assert_eq!(lines[2], "\treturn sum(alpha,");
    assert_eq!(lines[3], "\t\t\t   #if A");
    assert_eq!(lines[5], "\t\t\t   #endif");
}

#[test]
fn multiline_define_preserves_following_conditional_state() {
    let source = fixture!(
        "#define INC(x) \\",
        "    ((x)+1)",
        "#if ENABLED",
        "int y=INC(1);",
        "#endif",
    );
    let actual = format(source);
    assert_eq!(
        actual,
        fixture!(
            "#define INC(x) \\",
            "    ((x)+1)",
            "#if ENABLED",
            "int y = INC(1);",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_define_aligns_expression_continuations() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(fixture!("#define FLAGS (FLAG_A \\", "| FLAG_B)",), &options);

    assert_eq!(
        actual,
        fixture!("#define FLAGS (FLAG_A \\", "               | FLAG_B)",)
    );
}

#[test]
fn indent_preproc_define_aligns_flag_continuations_after_open_paren() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;

    assert_eq!(
        format_c(
            fixture!(
                concat!("#define OPTION_FLAGS (FIRST_FLAG| ", r"\"),
                concat!("          SECOND_FLAG| ", r"\"),
                "          THIRD_FLAG)",
            ),
            &options,
        ),
        fixture!(
            concat!("#define OPTION_FLAGS (FIRST_FLAG| ", r"\"),
            concat!("                      SECOND_FLAG| ", r"\"),
            "                      THIRD_FLAG)",
        ),
    );
}

#[test]
fn indent_preproc_define_preserves_expression_operator_spacing() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "#define ALIGN_PTR(value, align) \\",
            "    (byte *)(((uintptr_t)(value) + ((uintptr_t)align - 1))&~((uintptr_t)align - 1))",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define ALIGN_PTR(value, align) \\",
            "    (byte *)(((uintptr_t)(value) + ((uintptr_t)align - 1))&~((uintptr_t)align - 1))",
        )
    );
}

#[test]
fn indent_preproc_define_preserves_parenthesized_expression_continuations() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "#define SPECIAL(x) \\",
            "    (((x)->ready || (x)->done) \\",
            "     && !check(x) && !(x)->file)",
            "#define SIZE(x) \\",
            "    (in_memory(x) ? (long)((x)->last - (x)->pos) : \\",
            "     ((x)->file_last - (x)->file_pos))",
            "#define ADD(x) \\",
            "    (((x)[0] << 8) \\",
            "     + ((x)[1]))",
            "#define SET(block, data, n) \\",
            "    (block[n] = \\",
            "                (uint32_t)data[n * 4] | \\",
            "                ((uint32_t)data[n * 4 + 1] << 8))",
            "#define MIN_SIZE \\",
            "    align((sizeof(item_t) + 2 * sizeof(extra_t)), \\",
            "          ALIGNMENT)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define SPECIAL(x) \\",
            "    (((x)->ready || (x)->done) \\",
            "     && !check(x) && !(x)->file)",
            "#define SIZE(x) \\",
            "    (in_memory(x) ? (long)((x)->last - (x)->pos) : \\",
            "     ((x)->file_last - (x)->file_pos))",
            "#define ADD(x) \\",
            "    (((x)[0] << 8) \\",
            "     + ((x)[1]))",
            "#define SET(block, data, n) \\",
            "    (block[n] = \\",
            "                (uint32_t)data[n * 4] | \\",
            "                ((uint32_t)data[n * 4 + 1] << 8))",
            "#define MIN_SIZE \\",
            "    align((sizeof(item_t) + 2 * sizeof(extra_t)), \\",
            "          ALIGNMENT)",
        )
    );
}

#[test]
fn indent_preproc_define_aligns_call_continuations() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.max_continuation_indent = 80;
    let actual = format_with(
        fixture!(
            "#define DEBUG6(level, fmt,                                      \\",
            "    a1, a2, a3, a4, a5, a6) call(level, fmt, \\",
            "                                  a1, a2, a3, a4, a5, a6)",
            "",
            "#define TRACE8(level, state, err, fmt,                                  \\",
            "    a1, a2, a3, a4, a5, a6, a7, a8) if ((state)->flags & level) call_core(state, err, \\",
            "            fmt, \\",
            "            a1, a2, a3, a4, a5, a6, a7, a8)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define DEBUG6(level, fmt,                                      \\",
            "               a1, a2, a3, a4, a5, a6) call(level, fmt, \\",
            "                                            a1, a2, a3, a4, a5, a6)",
            "",
            "#define TRACE8(level, state, err, fmt,                                  \\",
            "               a1, a2, a3, a4, a5, a6, a7, a8) if ((state)->flags & level) call_core(state, err, \\",
            "                           fmt, \\",
            "                           a1, a2, a3, a4, a5, a6, a7, a8)",
        )
    );
}

#[test]
fn default_indents_region_and_openmp_directives() {
    let actual = format_with(
        fixture!(
            "void f(void){",
            "#region A",
            "#pragma omp parallel",
            "#pragma region B",
            "int x;",
            "#pragma endregion",
            "#endregion",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    #region A",
            "    #pragma omp parallel",
            "    #pragma region B",
            "    int x;",
            "    #pragma endregion",
            "    #endregion",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_indents_directives_inside_function() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#if ENABLED",
            "return 1;",
            "#else",
            "return 0;",
            "#endif",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    #if ENABLED",
            "    return 1;",
            "    #else",
            "    return 0;",
            "    #endif",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    options.indent_style = IndentStyle::Tabs;
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    let actual = format_with(
        fixture!(
            "void run(void){",
            "if(ready){",
            "#if ENABLED",
            "call();",
            "#else",
            "other();",
            "#endif",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run(void)",
            "\t{",
            "\tif (ready)",
            "\t\t{",
            "\t\t#if ENABLED",
            "\t\tcall();",
            "\t\t#else",
            "\t\tother();",
            "\t\t#endif",
            "\t\t}",
            "\t}",
        )
    );
}

#[test]
fn indent_preproc_conditional_normalizes_continued_directive_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "void run(void){",
            concat!("#if defined(ALPHA) && ", r"\"),
            "        defined(BETA)",
            "call();",
            "#endif",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run(void)",
            "{",
            concat!("    #if defined(ALPHA) && ", r"\"),
            "    defined(BETA)",
            "    call();",
            "    #endif",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_uses_indented_case_body_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    options.indent_switches = true;
    let actual = format_with(
        fixture!(
            "void run(int value){",
            "switch(value){",
            "case 1:",
            "#if ENABLED",
            "call();",
            "#else",
            "other();",
            "#endif",
            "break;",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run(int value)",
            "{",
            "    switch (value)",
            "    {",
            "        case 1:",
            "            #if ENABLED",
            "            call();",
            "            #else",
            "            other();",
            "            #endif",
            "            break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_with_vtk_keeps_split_else_brace_tabs() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;
    options.indent_style = IndentStyle::Tabs;
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "void run(void){",
            "if(ready){",
            "call();",
            "}else",
            "#if ENABLED",
            "other();",
            "#else",
            "last();",
            "#endif",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run(void)",
            "{",
            "\tif (ready)",
            "\t\t{",
            "\t\tcall();",
            "\t\t}",
            "\telse",
            "\t#if ENABLED",
            "\t\tother();",
            "\t#else",
            "\t\tlast();",
            "\t#endif",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_indents_c23_branch_separators() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#ifdef A",
            "return 1;",
            "#elifdef B",
            "return 2;",
            "#elifndef C",
            "return 3;",
            "#endif",
            "}",
        ),
        &options,
    );

    // C23 branch separators use the active conditional column.
    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    #ifdef A",
            "    return 1;",
            "    #elifdef B",
            "    return 2;",
            "    #elifndef C",
            "    return 3;",
            "    #endif",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_recognizes_endif_with_attached_comment() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#ifndef VALUE//note",
            "return 1;",
            "#endif//note",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    #ifndef VALUE//note",
            "    return 1;",
            "    #endif//note",
            "}",
        )
    );
}

#[test]
fn indent_preproc_conditional_preserves_else_endif_without_opener() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "int data[] =",
            "{",
            "// #ifdef ALPHA",
            "1,",
            "#else",
            "2,",
            "#endif",
            "3,",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int data[] =",
            "{",
            "// #ifdef ALPHA",
            "    1,",
            "#else",
            "    2,",
            "#endif",
            "    3,",
            "};",
        )
    );
}

#[test]
fn indent_preproc_define_indents_multiline_body() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#define INC(x) \\",
            "((x)+1)",
            "return INC(1);}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "#define INC(x) \\",
            "    ((x)+1)",
            "    return INC(1);",
            "}",
        )
    );
}

#[test]
fn indent_preproc_define_uses_one_body_level_with_zero_continuation_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.continuation_indent = 0;
    let actual = format_c(
        fixture!(concat!("#define VALUE ", r"\"), "alpha + beta"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(concat!("#define VALUE ", r"\"), "    alpha + beta")
    );
}

#[test]
fn indent_preproc_define_uses_one_body_level_with_two_continuation_indents() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.continuation_indent = 2;
    let actual = format_c(
        fixture!(concat!("#define VALUE ", r"\"), "alpha + beta"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(concat!("#define VALUE ", r"\"), "    alpha + beta")
    );
}

#[test]
fn indent_preproc_define_expression_uses_structural_tab_prefix() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define VALUE(alpha, beta) ", r"\"),
            concat!("(alpha + ", r"\"),
            "beta)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define VALUE(alpha, beta) ", r"\"),
            concat!("\t(alpha + ", r"\"),
            "\t beta)",
        )
    );
}

#[test]
fn convert_tabs_with_tab_indent_expands_preprocessor_continuation_tab() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    options.indent_style = IndentStyle::Tabs;

    assert_eq!(
        format_c(
            fixture!(
                "#define CALL(value) \\",
                "\tcall(value)",
                "void run()",
                "{",
                "CALL(1);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "#define CALL(value) \\",
            "    call(value)",
            "void run()",
            "{",
            "\tCALL(1);",
            "}",
        ),
    );
}

#[test]
fn indent_preproc_define_call_alignment_uses_visual_tab_columns() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY ", r"\"),
            concat!("call(alpha, ", r"\"),
            "beta)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY ", r"\"),
            concat!("\tcall(alpha, ", r"\"),
            "\t     beta)",
        )
    );
}

#[test]
fn indent_preproc_define_assignment_alignment_uses_visual_tab_columns() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define TOTAL ", r"\"),
            concat!("result = alpha + ", r"\"),
            "beta;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define TOTAL ", r"\"),
            concat!("\tresult = alpha + ", r"\"),
            "\t         beta;",
        )
    );
}

#[test]
fn indent_preproc_define_string_call_alignment_uses_visual_tab_columns() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define TEXT(value) ", r"\"),
            concat!("call(\"{ not a block }\", ", r"\"),
            "value)",
        ),
        &options,
    );

    // Generated exact-column rows retain their structural tab prefix.
    assert_eq!(
        actual,
        fixture!(
            concat!("#define TEXT(value) ", r"\"),
            concat!("\tcall(\"{ not a block }\", ", r"\"),
            "\t     value)",
        )
    );
}

#[test]
fn indent_preproc_define_plain_struct_declaration_uses_assignment_continuation() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define ITEM(name) ", r"\"),
            concat!("static const struct Entry value ", r"\"),
            "= { name, 0 }",
        ),
        &options,
    );

    // Leading type spelling does not change assignment-continuation depth.
    assert_eq!(
        actual,
        fixture!(
            concat!("#define ITEM(name) ", r"\"),
            concat!("    static const struct Entry value ", r"\"),
            "        = { name, 0 }",
        )
    );
}

#[test]
fn indent_preproc_define_block_comment_keeps_structural_tabs() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ ", r"\"),
            concat!("/* first ", r"\"),
            concat!(" * second ", r"\"),
            concat!(" */ ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("\t{ ", r"\"),
            concat!("\t\t/* first ", r"\"),
            concat!("\t\t * second ", r"\"),
            concat!("\t\t */ ", r"\"),
            "\t}",
        )
    );
}

#[test]
fn indent_preproc_define_indents_open_block_comment_and_following_brace() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;

    assert_eq!(
        format_c(
            "#define APPLY(X,Y) \\\n{                  \\\n    /* first line\n     * second line */  \\\n    {                      \\\n",
            &options,
        ),
        "#define APPLY(X,Y) \\\n    {                  \\\n        /* first line\n         * second line */  \\\n        {                      \\\n",
    );
}

#[test]
fn indent_preproc_define_indents_directive_inside_replacement_list() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;

    assert_eq!(
        format_c(
            fixture!(
                concat!("#define APPLY ", r"\"),
                concat!("#ifdef ENABLED ", r"\"),
            ),
            &options,
        ),
        fixture!(
            concat!("#define APPLY ", r"\"),
            concat!("    #ifdef ENABLED ", r"\"),
        ),
    );
}

#[test]
fn indent_preproc_define_indents_comment_and_call_continuations_without_leaking_state() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#define BODY(x) \\",
            "/* keep */ \\",
            "sum(x, \\",
            "1)",
            "return BODY(1);}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "#define BODY(x) \\",
            "    /* keep */ \\",
            "    sum(x, \\",
            "        1)",
            "    return BODY(1);",
            "}",
        )
    );
}

#[test]
fn multiline_define_preserves_comments_and_backslashes() {
    let actual = format(fixture!(
        "#define BODY(x) \\",
        "/* keep */ \\",
        "do { call(x); } while (0)",
        "int y=1;",
    ));

    assert_eq!(
        actual,
        fixture!(
            "#define BODY(x) \\",
            "/* keep */ \\",
            "do { call(x); } while (0)",
            "int y = 1;",
        )
    );
}

#[test]
fn multiline_define_comment_continues_until_block_comment_closes() {
    let actual = format(fixture!(
        "#define BODY /* start",
        "still comment */",
        "int y=1;",
    ));

    assert_eq!(
        actual,
        fixture!("#define BODY /* start", "still comment */", "int y = 1;",)
    );
}

#[test]
fn terminated_multiline_define_comment_does_not_continue_definition() {
    let actual = format(fixture!("#define BODY /* done */", "int y=1;",));

    assert_eq!(actual, fixture!("#define BODY /* done */", "int y = 1;",));
}

#[test]
fn indent_preproc_define_comment_continuation_does_not_leak_state() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "#define BODY \\",
            "/* start",
            "still comment */",
            "return 0;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "#define BODY \\",
            "    /* start",
            "    still comment */",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn indent_preproc_define_with_whitesmith_indents_physical_braces() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_switches = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ ", r"\"),
            concat!("if (ready(value)) { ", r"\"),
            concat!("call(value); ", r"\"),
            concat!("} ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("        { ", r"\"),
            concat!("        if (ready(value)) { ", r"\"),
            concat!("            call(value); ", r"\"),
            concat!("            } ", r"\"),
            "        }",
        )
    );
}

#[test]
fn indent_preproc_define_with_ratliff_indents_physical_braces() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ ", r"\"),
            concat!("call(value); ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("        { ", r"\"),
            concat!("        call(value); ", r"\"),
            "        }",
        )
    );
}

#[test]
fn indent_preproc_define_with_vtk_indents_command_closing_brace() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Vtk;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) do { ", r"\"),
            concat!("call(value); ", r"\"),
            "} while (0)",
        ),
        &options,
    );

    // VTK command closers use the command-body column at every scope depth.
    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) do { ", r"\"),
            concat!("        call(value); ", r"\"),
            "        } while (0)",
        )
    );
}

#[test]
fn indent_preproc_define_with_gnu_indents_command_blocks() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) do { ", r"\"),
            concat!("call(value); ", r"\"),
            "} while (0)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) do { ", r"\"),
            concat!("            call(value); ", r"\"),
            "        } while (0)",
        )
    );
}

#[test]
fn indent_preproc_define_with_vtk_uses_command_body_columns_for_nested_switch() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Vtk;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ ", r"\"),
            concat!("switch (value) { ", r"\"),
            concat!("case 1: ", r"\"),
            concat!("call(); ", r"\"),
            concat!("break; ", r"\"),
            concat!("} ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    { ", r"\"),
            concat!("        switch (value) { ", r"\"),
            concat!("            case 1: ", r"\"),
            concat!("                call(); ", r"\"),
            concat!("                break; ", r"\"),
            concat!("            } ", r"\"),
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_nested_call_restores_outer_argument_anchor() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY ", r"\"),
            concat!("outer(alpha, inner(beta, ", r"\"),
            concat!("gamma), ", r"\"),
            "delta)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY ", r"\"),
            concat!("    outer(alpha, inner(beta, ", r"\"),
            concat!("                       gamma), ", r"\"),
            "          delta)",
        )
    );
}

#[test]
fn indent_preproc_define_closing_paren_uses_opening_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define FLAGS (ALPHA ", r"\"),
            concat!("| BETA ", r"\"),
            ")",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define FLAGS (ALPHA ", r"\"),
            concat!("               | BETA ", r"\"),
            "              )",
        )
    );
}

#[test]
fn indent_preproc_define_sibling_parens_restore_outer_anchor() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define BITS ((1 << A) | ", r"\"),
            concat!("(1 << B) | ", r"\"),
            "(1 << C))",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define BITS ((1 << A) | ", r"\"),
            concat!("              (1 << B) | ", r"\"),
            "              (1 << C))",
        )
    );
}

#[test]
fn indent_preproc_define_assignment_initializer_closer_uses_brace_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("= { ", r"\"),
            concat!("alpha, ", r"\"),
            concat!("beta, ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("        = { ", r"\"),
            concat!("            alpha, ", r"\"),
            concat!("            beta, ", r"\"),
            "          }",
        )
    );
}

#[test]
fn indent_preproc_define_assignment_initializer_keeps_macro_body_tab_ownership() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("= { ", r"\"),
            concat!("alpha, ", r"\"),
            concat!("beta, ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("\t    = { ", r"\"),
            concat!("\t        alpha, ", r"\"),
            concat!("\t        beta, ", r"\"),
            "\t      }",
        )
    );
}

#[test]
fn indent_preproc_define_with_whitesmith_keeps_initializer_closer_macro_tab() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.indent_style = IndentStyle::Tabs;
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_switches = true;
    let actual = format_c(
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("= { ", r"\"),
            concat!("alpha, ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define VALUES ", r"\"),
            concat!("\t    = { ", r"\"),
            concat!("\t        alpha, ", r"\"),
            "\t      }",
        )
    );
}

#[test]
fn indent_preproc_define_blank_continuation_uses_current_block_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ ", r"\"),
            r"\",
            concat!("call(value); ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    { ", r"\"),
            concat!("        ", r"\"),
            concat!("        call(value); ", r"\"),
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_inline_comment_preserves_relative_source_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("call(value); /* first ", r"\"),
            concat!(" * second */ ", r"\"),
            "other();",
        ),
        &options,
    );

    // Comment continuations keep their relative column within the shifted macro body.
    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    call(value); /* first ", r"\"),
            concat!("     * second */ ", r"\"),
            "    other();",
        )
    );
}

#[test]
fn indent_preproc_define_does_not_indent_following_column_one_comment() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = fixture!(
        concat!("#define APPLY(value) ", r"\"),
        "call(value)",
        "// note",
        "int value;",
    );
    let actual = format_c(source, &options);

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            "    call(value)",
            "// note",
            "int value;",
        )
    );
}

#[test]
fn indent_preproc_define_with_whitesmith_uses_case_body_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_switches = true;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("switch (value) { ", r"\"),
            concat!("case 1: { ", r"\"),
            concat!("call(); ", r"\"),
            concat!("break; ", r"\"),
            concat!("} ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    switch (value) { ", r"\"),
            concat!("    case 1: { ", r"\"),
            concat!("        call(); ", r"\"),
            concat!("        break; ", r"\"),
            concat!("        } ", r"\"),
            "        }",
        )
    );
}

#[test]
fn indent_preproc_define_with_vtk_uses_body_column_for_case_block_closer() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Vtk;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("switch (value) { ", r"\"),
            concat!("case 1: { ", r"\"),
            concat!("call(); ", r"\"),
            concat!("break; ", r"\"),
            concat!("} ", r"\"),
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    switch (value) { ", r"\"),
            concat!("    case 1: { ", r"\"),
            concat!("        call(); ", r"\"),
            concat!("        break; ", r"\"),
            concat!("        } ", r"\"),
            "        }",
        )
    );
}

#[test]
fn indent_preproc_define_with_vtk_keeps_adjacent_blocks_at_outer_brace_column() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.brace_style = BraceStyle::Vtk;
    let actual = format_c(
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("{ { ", r"\"),
            concat!("call(value); ", r"\"),
            "} }",
        ),
        &options,
    );

    // Adjacent nested blocks use the outer brace column on their shared row.
    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    { { ", r"\"),
            concat!("            call(value); ", r"\"),
            "        } }",
        )
    );
}

#[test]
fn min_conditional_indent_does_not_preserve_define_source_indent() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let actual = format_c(
        fixture!(
            concat!("    #define APPLY(value) ", r"\"),
            concat!("        call(value, ", r"\"),
            "             other)",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            concat!("#define APPLY(value) ", r"\"),
            concat!("    call(value, ", r"\"),
            "         other)",
        )
    );
}

#[test]
fn indent_preproc_define_indents_struct_brace_body() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            "#define ATOMIC(type) struct { \\",
            "_Atomic(type) value; \\",
            "char pad[PAD(type)]; \\",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define ATOMIC(type) struct { \\",
            "        _Atomic(type) value; \\",
            "        char pad[PAD(type)]; \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_define_keeps_in_function_directive_at_column_zero() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    int x = 1;",
            "#define M(a) do { \\",
            "g(a); \\",
            "} while (0)",
            "    M(1);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    int x = 1;",
            "#define M(a) do { \\",
            "        g(a); \\",
            "    } while (0)",
            "    M(1);",
            "}",
        )
    );
}

#[test]
fn indent_preproc_define_indents_assignment_continuation() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_c(
        fixture!(
            "#define FUNC(name, func) \\",
            "static const struct Item CONCAT(func) \\",
            "= { .name = name, .func = func }",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define FUNC(name, func) \\",
            "    static const struct Item CONCAT(func) \\",
            "        = { .name = name, .func = func }",
        )
    );
}

#[test]
fn indent_preproc_define_formats_body_without_leaking_state() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "#define BODY(x) \\",
            "do { \\",
            "call(x); \\",
            "} while (0)",
            "int y=1;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define BODY(x) \\",
            "    do { \\",
            "        call(x); \\",
            "    } while (0)",
            "int y = 1;",
        )
    );
}

#[test]
fn indent_preproc_define_keeps_default_parameter_ternary_at_body_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_preproc_define = true;
    let actual = format_with(
        fixture!(
            "#define MERGE(value, previous, default) \\",
            "if (value == UNSET) { \\",
            "value = (previous == UNSET) ? default : previous; \\",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#define MERGE(value, previous, default) \\",
            "    if (value == UNSET) { \\",
            "    value = (previous == UNSET) ? default : previous; \\",
            "    }",
        )
    );
}

#[test]
fn indent_preproc_conditional_preserves_split_else_body_across_multiline_define() {
    let mut options = FormatOptions::default();
    options.indent_preproc_conditional = true;
    let actual = format_with(
        fixture!(
            "void run(void){",
            "if(ready){call();}else",
            "#if ENABLED",
            concat!("#define APPLY(value) ", r"\"),
            "call(value)",
            "APPLY(1);",
            "#else",
            "other();",
            "#endif",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run(void)",
            "{",
            "    if (ready)",
            "    {",
            "        call();",
            "    }",
            "    else",
            "    #if ENABLED",
            concat!("#define APPLY(value) ", r"\"),
            "call(value)",
            "        APPLY(1);",
            "    #else",
            "        other();",
            "    #endif",
            "}",
        )
    );
}

#[test]
fn preprocessor_with_gnu_preserves_column_one_block_comment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let actual = format_with(
        fixture!("#if ENABLED", "/* comment */", "int value;", "#endif"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("#if ENABLED", "/* comment */", "int value;", "#endif")
    );
}

#[test]
fn preprocessor_branches_restore_structural_indentation() {
    let actual = format(fixture!(
        "void f(){",
        "#if A",
        "if(a){",
        "return 1;",
        "#else",
        "if(b){",
        "return 2;",
        "}",
        "#endif",
        "return 0;",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "#if A",
            "    if (a)",
            "    {",
            "        return 1;",
            "#else",
            "    if (b)",
            "    {",
            "        return 2;",
            "    }",
            "#endif",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn preprocessor_split_else_if_indents_following_if_as_else_body() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n#if B\n    if (b)\n    {\n        return false;\n    }\n#endif\n    return true;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n#if B\n        if (b)\n        {\n            return false;\n        }\n#endif\n    return true;\n}\n",
    );
}

#[test]
fn preprocessor_split_bare_else_multiline_call_keeps_single_continuation_level() {
    assert_eq!(
        format_c(
            "void f()\n{\n#ifdef A\n    if (a)\n        call(x);\n    else\n#endif\n    call(\n        x,\n        y\n    );\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n#ifdef A\n    if (a)\n        call(x);\n    else\n#endif\n        call(\n            x,\n            y\n        );\n}\n",
    );
}

#[test]
fn preprocessor_split_bare_else_if_indents_following_if_as_else_body() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#ifdef A\n    if (a)\n        return true;\n    else\n#endif\n    if (b)\n        return false;\n    else\n        return true;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#ifdef A\n    if (a)\n        return true;\n    else\n#endif\n        if (b)\n            return false;\n        else\n            return true;\n}\n",
    );
}

#[test]
fn split_else_body_if_wraps_condition_from_its_own_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n#ifdef MODE1\n  if (a)\n    {\n      x ();\n    }\n  else\n#endif\n  if (has_supported_memory (obj) &&\n      state->format_id != INVALID_FORMAT_KEY)\n    {\n      y ();\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n#ifdef MODE1\n    if (a)\n    {\n        x ();\n    }\n    else\n#endif\n        if (has_supported_memory (obj) &&\n                state->format_id != INVALID_FORMAT_KEY)\n        {\n            y ();\n        }\n}\n",
    );
}

#[test]
fn split_else_body_chained_else_if_keeps_extra_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n#ifdef MODE1\n  if (a)\n    {\n      x ();\n    }\n  else\n#endif\n  if (b)\n    {\n      y ();\n    }\n  else if (c)\n    {\n      z ();\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n#ifdef MODE1\n    if (a)\n    {\n        x ();\n    }\n    else\n#endif\n        if (b)\n        {\n            y ();\n        }\n        else if (c)\n        {\n            z ();\n        }\n}\n",
    );
}

#[test]
fn preprocessor_split_else_if_chain_nests_each_following_branch() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n#if B\n    if (b)\n    {\n        return true;\n    } else\n#endif\n#if C\n    if (c)\n    {\n        return true;\n    }\n#endif\n    return false;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n#if B\n        if (b)\n        {\n            return true;\n        } else\n#endif\n#if C\n            if (c)\n            {\n                return true;\n            }\n#endif\n    return false;\n}\n",
    );
}

#[test]
fn preprocessor_split_else_indents_nested_headers() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.add_braces = true;
    options.break_one_line_headers = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "#if USE_A",
            "if(a){",
            "work_a();",
            "}else",
            "#endif",
            "#if USE_B",
            "if(b){",
            "work_b();",
            "}else",
            "#endif",
            "{",
            "work_c();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "#if USE_A",
            "    if (a) {",
            "        work_a();",
            "    } else",
            "#endif",
            "#if USE_B",
            "        if (b) {",
            "            work_b();",
            "        } else",
            "#endif",
            "        {",
            "            work_c();",
            "        }",
            "}",
        )
    );
}

#[test]
fn preprocessor_split_else_keeps_single_brace_at_else_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.add_braces = true;
    options.break_one_line_headers = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "#if A",
            "if (x) {",
            "a();",
            "} else",
            "#endif",
            "{",
            "b();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "#if A",
            "    if (x) {",
            "        a();",
            "    } else",
            "#endif",
            "    {",
            "        b();",
            "    }",
            "}",
        )
    );
}

#[test]
fn preprocessor_split_else_keeps_braceless_body_extra_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.add_braces = true;
    options.break_one_line_headers = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "#if GUARD",
            "    if (alpha)",
            "        beta = (beta == 1) ? 0 : beta;",
            "    else",
            "#endif",
            "        beta = (beta == 2) ? 0 : beta;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "#if GUARD",
            "    if (alpha) {",
            "        beta = (beta == 1) ? 0 : beta;",
            "    } else",
            "#endif",
            "        beta = (beta == 2) ? 0 : beta;",
            "}",
        )
    );
}

#[test]
fn inline_new_call_after_split_else_preprocessor_aligns_to_open_paren() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "#ifdef X",
                "    if (x)",
                "        other = new Control(a, b, c,",
                "                            d,",
                "                            e);",
                "    else",
                "#endif",
                "    value = new WidgetControl(this, id, current,",
                "                              position, size,",
                "                              style);",
                "",
                "#ifdef Y",
                "    if (y)",
                "        other = new Control(a, b, c,",
                "                            d,",
                "                            e);",
                "    else",
                "#endif",
                "    value = new ExtremelyVeryVeryLongWidgetControlName(this, id, current,",
                "                                                       position, size,",
                "                                                       style);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "#ifdef X",
            "    if (x)",
            "        other = new Control(a, b, c,",
            "                            d,",
            "                            e);",
            "    else",
            "#endif",
            "        value = new WidgetControl(this, id, current,",
            "                                  position, size,",
            "                                  style);",
            "",
            "#ifdef Y",
            "    if (y)",
            "        other = new Control(a, b, c,",
            "                            d,",
            "                            e);",
            "    else",
            "#endif",
            "        value = new ExtremelyVeryVeryLongWidgetControlName(this, id, current,",
            "                position, size,",
            "                style);",
            "}",
        )
    );
}

#[test]
fn preprocessor_elif_branches_restore_structural_indentation() {
    let actual = format(fixture!(
        "void f(){",
        "#if A",
        "if(a){",
        "return 1;",
        "#elif B",
        "if(b){",
        "return 2;",
        "#else",
        "if(c){",
        "return 3;",
        "}",
        "#endif",
        "return 0;",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "#if A",
            "    if (a)",
            "    {",
            "        return 1;",
            "#elif B",
            "    if (b)",
            "    {",
            "        return 2;",
            "#else",
            "    if (c)",
            "    {",
            "        return 3;",
            "    }",
            "#endif",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn indent_preproc_block_does_not_treat_later_ifndef_as_include_guard() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#if FEATURE",
            "int feature;",
            "#endif",
            "#ifndef HEADER_H",
            "#define HEADER_H",
            "int guarded;",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#if FEATURE",
            "    int feature;",
            "#endif",
            "#ifndef HEADER_H",
            "    #define HEADER_H",
            "    int guarded;",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_preserves_if_not_defined_include_guard() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#if !defined(HEADER_H)",
            "#define HEADER_H",
            "int guarded;",
            "#if FEATURE",
            "int nested;",
            "#endif",
            "#define BODY \\",
            "    do { call(); } while (0)",
            "#endif",
            "#if FEATURE",
            "int feature;",
            "struct S{int x;};",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#if !defined(HEADER_H)",
            "#define HEADER_H",
            "int guarded;",
            "#if FEATURE",
            "    int nested;",
            "#endif",
            "#define BODY \\",
            "    do { call(); } while (0)",
            "#endif",
            "#if FEATURE",
            "int feature;",
            "struct S",
            "{",
            "    int x;",
            "};",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_preserves_spaced_if_not_defined_include_guard() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#if ! defined HEADER_H",
            "#define HEADER_H",
            "int guarded;",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#if ! defined HEADER_H",
            "#define HEADER_H",
            "int guarded;",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_indents_if_not_defined_without_define() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!("#if ! defined HEADER_H", "int guarded;", "#endif",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("#if ! defined HEADER_H", "    int guarded;", "#endif",)
    );
}

#[test]
fn indent_preproc_block_does_not_treat_defined_prefix_identifiers_as_include_guards() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#if !definedness",
            "int value;",
            "#endif",
            "#if !defined_value",
            "int other;",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#if !definedness",
            "    int value;",
            "#endif",
            "#if !defined_value",
            "    int other;",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_indents_nested_conditionals_progressively() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#ifdef ALPHA",
            "#ifdef BETA",
            "const int value = 5;",
            "#endif",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#ifdef ALPHA",
            "    #ifdef BETA",
            "        const int value = 5;",
            "    #endif",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_indents_consecutive_non_guard_blocks() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#ifdef ALPHA",
            "#define VALUE 1",
            "#endif",
            "#ifndef ALPHA",
            "#define VALUE 2",
            "#endif",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#ifdef ALPHA",
            "    #define VALUE 1",
            "#endif",
            "#ifndef ALPHA",
            "    #define VALUE 2",
            "#endif",
        )
    );
}

#[test]
fn indent_preproc_block_collapses_padded_pound_directives() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#ifdef ALPHA",
            "#  define VALUE 1",
            "#  endif",
            "int after;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#ifdef ALPHA",
            "    #define VALUE 1",
            "#endif",
            "int after;",
        )
    );
}

#[test]
fn indent_preproc_block_normalizes_nested_padded_pound_directives() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;

    assert_eq!(
        format_c(
            fixture!(
                "#if defined(ALPHA)",
                "#   if defined(BETA) && defined(GAMMA)",
                "#       include <item.h>",
                "#   endif",
                "#endif",
            ),
            &options,
        ),
        fixture!(
            "#if defined(ALPHA)",
            "    #if defined(BETA) && defined(GAMMA)",
            "        #include <item.h>",
            "    #endif",
            "#endif",
        ),
    );
}

#[test]
fn indent_preproc_block_preserves_padded_pound_outside_block() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;

    assert_eq!(
        format_c(
            fixture!(
                "#if defined(ALPHA)",
                "#   include <item.h>",
                "#endif",
                "#    define VALUE",
            ),
            &options,
        ),
        fixture!(
            "#if defined(ALPHA)",
            "    #include <item.h>",
            "#endif",
            "#    define VALUE",
        ),
    );
}

#[test]
fn indent_preproc_block_skips_block_with_multiline_unbalanced_paren() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!(
            "#ifdef ALPHA",
            "int helper(int alpha,",
            "           int beta);",
            "#endif",
            "int after;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "#ifdef ALPHA",
            "int helper(int alpha,",
            "           int beta);",
            "#endif",
            "int after;",
        )
    );
}

#[test]
fn indent_preproc_block_skips_block_containing_braces() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    options.pad_operators = true;
    let actual = format_with(fixture!("#if A", "void f(){x=1;}", "#endif"), &options);

    assert_eq!(
        actual,
        fixture!("#if A", "void f()", "{", "    x = 1;", "}", "#endif",)
    );
}

#[test]
fn indent_preproc_block_indents_guard_when_code_follows() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;
    let actual = format_with(
        fixture!("#ifndef GUARD", "#define GUARD", "#endif", "int after;",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("#ifndef GUARD", "    #define GUARD", "#endif", "int after;",)
    );
}
#[test]
fn indent_preprocessor_keeps_for_update_continuation_in_define() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--indent-preprocessor",
        "--indent-preproc-define",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\n#define each(node, head) \\\n    for ((node) = first(head); \\\n         (node); \\\n         (node) = ((node) == (head)->last ? nullptr : (node)->next))\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn indent_preprocessor_keeps_function_parameter_continuation_in_define() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--indent-preprocessor",
        "--indent-preproc-define",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\n#define WRAP(func, name, type) \\\n    static inline bool name(const char *str, type *val, \\\n                            int base, type min, type max) { \\\n        return func(str, val, base, min, max); \\\n    }\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn preprocessor_conditional_after_unfinished_define_is_not_define_body() {
    let mut options = FormatOptions::default();
    let args = ["--indent-preproc-define", "--indent-preproc-cond"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\n#ifdef _WIN32\n#define Is_Bar(arg,P,b) \\\n#endif\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn indent_preproc_block_keeps_define_body_at_block_level_after_endif_like_line() {
    let mut options = FormatOptions::default();
    let args = ["--indent-preproc-block", "--indent-preproc-define"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "\n#ifdef ALPHA\nendif #ifclude <item.h>\n#define VALUE(a,b) \\\n|| call((a), (b))\n#endif\n#define VALUE\n",
            &options,
        ),
        "\n#ifdef ALPHA\n    endif #ifclude <item.h>\n    #define VALUE(a,b) \\\n    || call((a), (b))\n#endif\n#define VALUE\n",
    );
}

#[test]
fn preprocessor_inside_one_line_block_breaks_to_own_line() {
    assert_eq!(
        format_c(
            "\nvoid Foo()\n{\n    { #define }\n    x = 1;\n    y = 2;\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid Foo()\n{\n    {\n#define \n    }\n    x = 1;\n    y = 2;\n}\n",
    );
}

#[test]
fn blank_line_after_continued_define_ends_define_body() {
    let mut options = FormatOptions::default();
    options.indent_preproc_define = true;
    let source = "\n#define TRACE_ONE(value)            \\\n    if (enabled)                    \\\n        call(value);                \\\n\n#define TRACE_TWO(value)            \\\n    if (ready)                      \\\n        call(value);                \\\n\n";

    assert_eq!(format_c(source, &options), source);
}

// Mutually exclusive branches restore brace depth independently.
#[test]
fn preprocessor_branch_brace_imbalance_keeps_later_braces_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int opt)\n{\n\tif (opt & A) {\n\t\tif (cond)\n\t\t\treturn;\n#ifdef X\n\t} else if (opt & B) {\n\t\treturn;\n\t} else {\n\t\treturn;\n\t}\n#else\n\t} else if (other)\n\t\treturn;\n#endif\n\n\tif (opt & C) {\n\t\twork();\n\t} else {\n\t\tdone();\n\t}\n}\n",
            &options,
        ),
        "void f(int opt)\n{\n    if (opt & A) {\n        if (cond)\n            return;\n#ifdef X\n    } else if (opt & B) {\n        return;\n    } else {\n        return;\n    }\n#else\n    } else if (other)\n        return;\n#endif\n\n    if (opt & C) {\n        work();\n    } else {\n        done();\n    }\n}\n",
    );
}

#[test]
fn preprocessor_else_call_argument_uses_then_branch_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    CHECK_EQ(output,\n#ifdef MODE_USE_WRAPPED_TEXT\n             LiteralString(\"1234\")\n#else\n             \"1234\"\n#endif\n            );\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    CHECK_EQ(output,\n#ifdef MODE_USE_WRAPPED_TEXT\n             LiteralString(\"1234\")\n#else\n             \"1234\"\n#endif\n            );\n}\n",
    );
}
#[test]
fn preprocessor_split_else_comment_body_keeps_statement_at_comment_indent() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n\n    // no\n    return false;\n\n    return false;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n\n        // no\n        return false;\n\n    return false;\n}\n",
    );
}
#[test]
fn preprocessor_split_else_chain_keeps_pending_body_after_nested_endifs() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#if B\n    if (b)\n    {\n        return true;\n    } else\n#endif\n#endif\n\n    // next\n#if C\n    if (c)\n    {\n        return true;\n    }\n#endif\n    return false;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#if B\n        if (b)\n        {\n            return true;\n        } else\n#endif\n#endif\n\n            // next\n#if C\n            if (c)\n            {\n                return true;\n            }\n#endif\n    return false;\n}\n",
    );
}
#[test]
fn preprocessor_split_else_if_else_body_skips_blank_line() {
    assert_eq!(
        format_c(
            "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n\n#if B\n    if (b)\n    {\n        return false;\n    }\n#endif\n    return true;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n#if A\n    if (a)\n    {\n        return true;\n    } else\n#endif\n\n#if B\n        if (b)\n        {\n            return false;\n        }\n#endif\n    return true;\n}\n",
    );
}
#[test]
fn split_else_chain_level_survives_branch_brace() {
    assert_eq!(
        format_c(
            r#"void f (void)
{
#ifdef A

BEGIN_DIAGNOSTIC_SCOPE

    if (cond_a)
    {
        if (x)
        {
            xa ();
        }
        else
        {
            xb ();
        }
    }
    else

END_DIAGNOSTIC_SCOPE

#endif
#ifdef B
    if (cond_b)
    {
        yb ();
    }
    else
#endif
        fallback ();
}
"#,
            &FormatOptions::default(),
        ),
        r#"void f (void)
{
#ifdef A

    BEGIN_DIAGNOSTIC_SCOPE

    if (cond_a)
    {
        if (x)
        {
            xa ();
        }
        else
        {
            xb ();
        }
    }
    else

        END_DIAGNOSTIC_SCOPE

#endif
#ifdef B
        if (cond_b)
        {
            yb ();
        }
        else
#endif
            fallback ();
}
"#,
    );
}

#[test]
fn default_malformed_embedded_preprocessor_trailing_tab_is_idempotent() {
    let options = FormatOptions::default();
    let input = ")\thelperzauto:(y\n-?1\nclasselse?\nalpha1Config\n){ break=:->\t#if A\t};\ndefault\n/helper\t!\nif :0Config(ItemnamespaceItemcase)  }for+/* block */;\n!=\n=default(  ->:for-==callalpha->)\ngammaenum\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_close_word_after_embedded_endif_is_idempotent() {
    let options = FormatOptions::default();
    let input = "alphaclass  42~\nnamespaceConfigautoItembreak#endif?{42||\nswitch-\t}namespace==\n-\nalphaconstexpr}{defaultautoifgammaswitch\t%for&&\ncatchautoz\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_define_with_inline_backslash_does_not_indent_next_bracket() {
    let options = FormatOptions::default();
    let input = "~:!return\n// line=Config\tcall||enumNULL::\n!=ifgamma>=struct\t,[auto  continue\n*\n!Config\n0helper->enum\ty\n==+namespace<=->\nconstexpr%>=>=casefor\nItemclass:{\t]}#define X(x) \\ >=alphacontinueyconstexprwhile\n]\nstructzwhile returnstruct))\nelse({try->  continue:\n? (\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn gnu_malformed_embedded_preprocessor_initializer_before_blank_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let first = format_c("x#if A<=>{a\n\nauto==={value<-\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_embedded_define_before_line_comment_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "gamma#define X(x) \\.13|2->alpha&>auto30; ??24&&catchenumfor{// linez\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_colon_run_in_preprocessor_block_is_idempotent() {
    let options = FormatOptions::default();
    let input = ":+gamma+||Item#if A*>=>12#endifalphaItem,20namespacebreak>result-><=  betacontinuenamespacewhile&&&/* block */]int#define X(x) \\for+voidcall:do{class>4Itemresult17#if Abeta/elseif}&&return#if A/* block */25\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_word_after_embedded_define_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "gamma/default~breakfor1  enum,while\ngamma#define X(x) \\)#else{ ->return\n-Config%%else=}continue)alpha\nwhile-enumbeta~\nautocontinue\nelse\t==call#define X(x) \\\n// line%\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_embedded_define_line_comment_body_indent_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "\n->{||/* block */while\t+#define X(x) \\// line!  ::\n1*try>=\n,default\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_embedded_define_adjacent_braces_are_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let first = format_c(
        "->&&%\tcase0+#define X(x) \\#if A:{{result}constexpr\n",
        &options,
    );

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_define_brace_after_semicolon_operator_body_is_idempotent() {
    let options = FormatOptions::default();
    let first = format_c("x;#define X(x) \\y%{\n<=z\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_embedded_define_after_blank_clears_colon_indent() {
    let options = FormatOptions::default();
    let input = "||&  do\nbetavalue:/ifItem>-try {#endif]{NULLdefault<=>||[>=::.(if42helper// line\n\n>42%betaelse42valuez#define X(x) \\beta#elseconstexprenum,while#if A\tbreak.0!catch\n\n::\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_brace_after_inline_preprocessor_header_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "\nnamespace\nautoif\t}\tx#define X(x) \\\nbreak!%alphaauto   /\tdefault\t?()auto\nConfig\t=else\nreturn ->\t+: do\n~#if Acontinue{ %\n#define X(x) \\namespace\t#define X(x) \\<=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn gnu_malformed_brace_after_inline_preprocessor_header_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let input = "\nnamespace\nautoif\t}\tx#define X(x) \\\nbreak!%alphaauto   /\tdefault\t?()auto\nConfig\t=else\nreturn ->\t+: do\n~#if Acontinue{ %\n#define X(x) \\namespace\t#define X(x) \\<=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn horstmann_malformed_brace_after_inline_preprocessor_header_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let input = "\nnamespace\nautoif\t}\tx#define X(x) \\\nbreak!%alphaauto   /\tdefault\t?()auto\nConfig\t=else\nreturn ->\t+: do\n~#if Acontinue{ %\n#define X(x) \\namespace\t#define X(x) \\<=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_brace_after_inline_preprocessor_header_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "\nnamespace\nautoif\t}\tx#define X(x) \\\nbreak!%alphaauto   /\tdefault\t?()auto\nConfig\t=else\nreturn ->\t+: do\n~#if Acontinue{ %\n#define X(x) \\namespace\t#define X(x) \\<=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_preprocessor_word_before_identifier_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "!Config(  enum,/case\nzcontinuedefault!=&& if{catch/<==\ncase\n&&Item==while)\n;gammabreakxreturn)switchalpha\tdefault\n}#elsealpha\n#elseautohelper]\nConfig\n[#endifz\nbreakdefaultif\treturn\ncall\n%\t~gammabetawhileItem\n[\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_colon_block_after_embedded_define_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = " \t(\t Config{\t #define X(x) \\ 42 switch\t/ /\t::\n) &\n\telseswitch\t #else .\t alpha[  )\n\tConfig   if\nbeta\t )\t #else\n\t(\n~\n\tdo\n\tbeta  . \tif  <= \ttry\n\t[\n )\tNULL&&\n(\n:\t {42   // comment\n\t<=\t=   else\tvalue   #define X(x) \\ 42   #else  throw\t while\n namespace\t 0\ncatch\n\t:   throw != Item\tresult\n\t/\n {\t|  42#define X(x) \\/* block */\t #define X(x) \\ 42 \tbeta\t&|\nif]\n <=>  % .==\t<=>\n1\n -> \t#else\tauto   >=\n >  gamma value  struct Config\t throw\nNULL\t ]\n ! \treturn \t42 gamma\tConfig,\n -\n\t>\t<= struct / result \t1   constexpr}   !=   <=>0\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn gnu_malformed_define_else_run_in_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let input = "yconstexpr\ncontinuehelper\ttrycall->alphaelse\n&&\n==\t&&for#endifswitch\nstruct\n[namespace-callfor\ndefault\n(catch\n]\ncontinue\t<=Item\n||if!=Config\n{\tdo#if A(::[return#define X(x) \\{~\thelperconstexpr{!while/* block */\n;[for/#if A\n#elsehelperalpha#define X(x) \\~constexpr!try  switch\t// linealpha\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn gnu_malformed_colon_block_after_embedded_define_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let input = " \t(\t Config{\t #define X(x) \\ 42 switch\t/ /\t::\n) &\n\telseswitch\t #else .\t alpha[  )\n\tConfig   if\nbeta\t )\t #else\n\t(\n~\n\tdo\n\tbeta  . \tif  <= \ttry\n\t[\n )\tNULL&&\n(\n:\t {42   // comment\n\t<=\t=   else\tvalue   #define X(x) \\ 42   #else  throw\t while\n namespace\t 0\ncatch\n\t:   throw != Item\tresult\n\t/\n {\t|  42#define X(x) \\/* block */\t #define X(x) \\ 42 \tbeta\t&|\nif]\n <=>  % .==\t<=>\n1\n -> \t#else\tauto   >=\n >  gamma value  struct Config\t throw\nNULL\t ]\n ! \treturn \t42 gamma\tConfig,\n -\n\t>\t<= struct / result \t1   constexpr}   !=   <=>0\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_trailing_header_brace_after_preprocessor_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = " throw   #else \t)\n: \tstruct\n\t!\treturn \tcase\t 42 \t:\t>namespace  42   <=>\n struct   >\n <=>value  ;  x\t> constexpr\n\tvalue  for Config \t< \t1try\t#else   0 \tresult  (\t // comment  |\tswitch  =)\telse   #if A\t ~\n\tnamespace \t#if A\n42\t== \tNULL-   y\n|!\n Config\n< throw   do \twhile\t 42  #endify\n ||#define X(x) \\/* block */\n\t; >= break\n -> if\n\t~\t :\n /\t /\n Item\n\tcatch\tthrow\n#define X(x) \\ 42\t try\t~\n/* block */\n #endif\tvalue\n42   return \t#if A  #define X(x) \\ 42\n %\nzwhile \t{\n #if A   try \treturn   try(\n ||\n\tauto\n call   <do\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_close_before_colon_preprocessor_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "catch=(do default)trydo&&1\nItemdefault )default else}\n:#if A?;[\nx*struct>=::::\ntry\t}y0\n=\n:elsecall#elsecase+;\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn preprocessor_branch_after_macro_string_call_keeps_branch_body_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(void) {\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(void) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
        ));
    }

    input.push_str("  if(show) {\n#if defined(A)\n    print(out, \"a-\" VALUE_ONE \".\"\n          VALUE_TWO \" (%s)\\n\", value);\n#elif defined(B)\n    print(out, \"b-\" VALUE_THREE \" (%s)\\n\", value);\n#elif defined(C)\n    print(out, \"c-\" VALUE_FOUR \" (%s)\\n\", value);\n#endif\n  } else\n\n    if(next) done();\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let arg = " ".repeat((depth + 2) * 4 + "print(".len());
    expected.push_str(&format!(
        "{outer}if(show) {{\n#if defined(A)\n{body}print(out, \"a-\" VALUE_ONE \".\"\n{arg}VALUE_TWO \" (%s)\\n\", value);\n#elif defined(B)\n{body}print(out, \"b-\" VALUE_THREE \" (%s)\\n\", value);\n#elif defined(C)\n{body}print(out, \"c-\" VALUE_FOUR \" (%s)\\n\", value);\n#endif\n{outer}}} else\n\n{body}if(next) done();\n}}\n"
    ));

    assert_eq!(format_c(&input, &FormatOptions::default()), expected);
}

#[test]
fn guarded_nested_condition_after_long_split_else_keeps_preprocessor_body_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int c) {\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
        ));
    }

    input.push_str("#ifndef FEATURE\n  if( (c=='o'\n        && (same(arg, \"output\")==0\n            || same(arg, \"once\")==0))\n        || (c=='e' && same(arg, \"excel\")==0)\n        || (c=='w' && same(arg, \"www\")==0)\n  ) {\n    done();\n  } else\n#endif\n\n    if(next) done();\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let branch = " ".repeat((depth + 3) * 4);
    let inner = " ".repeat((depth + 4) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let close = " ".repeat((depth + 1) * 4 + "if".len());
    expected.push_str(&format!(
        "#ifndef FEATURE\n{outer}if( (c=='o'\n{branch}&& (same(arg, \"output\")==0\n{inner}|| same(arg, \"once\")==0))\n{branch}|| (c=='e' && same(arg, \"excel\")==0)\n{branch}|| (c=='w' && same(arg, \"www\")==0)\n{close}) {{\n{body}done();\n{outer}}} else\n#endif\n\n{body}if(next) done();\n}}\n"
    ));

    assert_eq!(format_c(&input, &FormatOptions::default()), expected);
}

#[test]
fn grouped_condition_after_long_split_else_keeps_group_and_body_indent() {
    let depth = 8;
    let mut input =
        String::from("void f(int c) {\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int c) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
        ));
    }

    input.push_str("  if( c==1 && (same(arg, \"alpha\")==0\n        || same(arg, \"beta\")==0)\n  ) {\n    int value;\n    done();\n  } else\n\n    if(next) done();\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let group = " ".repeat((depth + 1) * 4 + "if( c==1 && (".len());
    let close = " ".repeat((depth + 1) * 4 + "if".len());
    expected.push_str(&format!(
        "{outer}if( c==1 && (same(arg, \"alpha\")==0\n{group}|| same(arg, \"beta\")==0)\n{close}) {{\n{body}int value;\n{body}done();\n{outer}}} else\n\n{body}if(next) done();\n}}\n"
    ));

    assert_eq!(format_c(&input, &FormatOptions::default()), expected);
}

#[test]
fn else_if_condition_after_long_split_else_aligns_to_condition_paren() {
    let depth = 8;
    let mut input =
        String::from("void f(int nArg) {\n#ifndef OMIT\n  if(a0){ one(); }else\n#endif\n\n");
    let mut expected = String::from(
        "void f(int nArg) {\n#ifndef OMIT\n    if(a0) {\n        one();\n    }\n    else\n#endif\n\n",
    );

    for index in 1..depth {
        input.push_str(&format!("  if(a{index}){{ one(); }}else\n\n"));
        let indent = " ".repeat((index + 1) * 4);
        expected.push_str(&format!(
            "{indent}if(a{index}) {{\n{indent}    one();\n{indent}}}\n{indent}else\n\n"
        ));
    }

    input.push_str("  if(a){\n    one();\n  } else if( nArg==3 && same(arg[1], \"close\")==0\n             && IsDigit(arg[2][0]) && arg[2][1]==0 ) {\n    done();\n  }\n}\n");
    let outer = " ".repeat((depth + 1) * 4);
    let body = " ".repeat((depth + 2) * 4);
    let condition = " ".repeat((depth + 1) * 4 + "} else if( ".len());
    expected.push_str(&format!(
        "{outer}if(a) {{\n{body}one();\n{outer}}} else if( nArg==3 && same(arg[1], \"close\")==0\n{condition}&& IsDigit(arg[2][0]) && arg[2][1]==0 ) {{\n{body}done();\n{outer}}}\n}}\n"
    ));

    assert_eq!(format_c(&input, &FormatOptions::default()), expected);
}

#[test]
fn sibling_preprocessor_block_comment_keeps_branch_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-preproc-block".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "namespace sample{\n#if ALPHA\n// first\nint value;\n#else\n/* second */\nint other;\n#endif\n}\n",
            &options,
        ),
        "namespace sample\n{\n#if ALPHA\n// first\n    int value;\n#else\n    /* second */\n    int other;\n#endif\n}\n",
    );
}

#[test]
fn run_in_styles_keep_multiline_define_splice_boundaries() {
    let source =
        "#define RUN(alpha) \\\ndo { \\\nif(alpha) { \\\ncall(); \\\n} \\\n} while(false)\n";

    for style in ["pico", "lisp"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn orphan_preprocessor_separators_do_not_create_branch_indent() {
    let source = "#else\nint alpha;\n#elif BETA\nint beta;\n#endif\nint gamma;\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// INTENTIONAL DIVERGENCE: A continued control condition does not replace a
// directive's structural owner with Linux's half-indent continuation column.
#[test]
fn linux_condition_preprocessor_directive_uses_structural_owner() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=linux".to_owned(),
            "--indent-preproc-cond".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){if(alpha&&\n#if ENABLED\nbeta\n#endif\ngamma){call();}}\n",
            &options,
        ),
        "void run()\n{\n    if(alpha&&\n        #if ENABLED\n       beta\n        #endif\n       gamma) {\n        call();\n    }\n}\n",
    );
}

#[test]
fn multiline_define_keeps_call_anchor_across_spliced_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent-preproc-define".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "#define VALUE(alpha) call(alpha, \\\nbeta \\\nint next;)\n",
            &options,
        ),
        "#define VALUE(alpha) call(alpha, \\\n                          beta \\\n                          int next;)\n",
    );
}

#[test]
fn multiline_define_call_anchor_includes_source_gap_after_opener() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent-preproc-define".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("#define VALUE(alpha) call( \\\nbeta)\n", &options,),
        "#define VALUE(alpha) call( \\\n                           beta)\n",
    );
}

#[test]
fn escaped_line_comment_in_define_continues_at_top_level_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "#define VALUE alpha + \\\n beta // comment \\\n continued\nint value;\n",
            &options,
        ),
        "#define VALUE alpha + \\\n beta // comment \\\ncontinued\nint value;\n",
    );
}
