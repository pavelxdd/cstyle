#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

fn non_whitespace_without_braces(source: &str) -> String {
    source
        .chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '{' && *ch != '}')
        .collect()
}

#[test]
fn add_braces_wraps_unbraced_control_statements() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x) return 1; else return 0;",
            "for(i=0;i<x;i++) sum+=i;",
            "while(x) x--;",
            "do x++; while(x<3);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "    {",
            "        return 1;",
            "    }",
            "    else",
            "    {",
            "        return 0;",
            "    }",
            "    for (i = 0; i < x; i++)",
            "    {",
            "        sum += i;",
            "    }",
            "    while (x)",
            "    {",
            "        x--;",
            "    }",
            "    do",
            "    {",
            "        x++;",
            "    }",
            "    while (x < 3);",
            "}",
        )
    );
}

#[test]
fn add_braces_preserves_source_spacing_in_wrapped_body() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    assert_eq!(
        format_c("int f(){if(x) value[ i ]=1;}\n", &options),
        fixture!(
            "int f() {",
            "    if(x) {",
            "        value[ i ]=1;",
            "    }",
            "}",
        )
    );
    assert_eq!(
        format_c("int f(){if(x) call() ;}\n", &options),
        fixture!("int f() {", "    if(x) {", "        call() ;", "    }", "}",)
    );
}

#[test]
fn add_braces_run_in_keeps_nested_if_one_line_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--add-braces".to_owned(),
            "--style=run-in".to_owned(),
            "--keep-one-line-blocks".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{",
                "    if (isBar1)",
                "        if (isBar2) return true;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo()",
            "{   if (isBar1)",
            "        if (isBar2) { return true; }",
            "}",
        )
    );
}

#[test]
fn add_braces_wraps_cross_line_statement_after_header_comments() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "if(x) // keep",
            "y();",
            "while(x)",
            "x--;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (x) // keep",
            "    {",
            "        y();",
            "    }",
            "    while (x)",
            "    {",
            "        x--;",
            "    }",
            "}",
        )
    );
}

#[test]
fn add_braces_does_not_wrap_preprocessor_following_header() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let actual = format_with(
        fixture!("void f(){", "for(;;)", "#if A", "z();", "#endif", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for (;;)",
            "#if A",
            "        z();",
            "#endif",
            "}",
        )
    );
}

#[test]
fn allman_add_braces_keeps_dangling_else_with_inner_header() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.brace_style = BraceStyle::Allman;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha) if(beta)",
            "        {",
            "            one();",
            "        }",
            "        else",
            "        {",
            "            two();",
            "        }",
            "}",
        )
    );
}

#[test]
fn add_braces_wraps_if_constexpr_body() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("void run(){ if constexpr(ready) work(); }\n", &options),
        fixture!(
            "void run() {",
            "    if constexpr(ready) {",
            "        work();",
            "    }",
            "}",
        )
    );
}

#[test]
fn add_braces_skips_empty_bodies_and_outer_nested_headers() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let actual = format_with(
        fixture!("int f(int x){", "if(x);", "if(x) if(y) y();", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x);",
            "    if (x) if (y)",
            "        {",
            "            y();",
            "        }",
            "}",
        )
    );
}

#[test]
fn add_braces_wraps_commented_body_and_ignores_comment_markers_in_strings() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x) /* keep */ y();",
            "if(strcmp(s,\"/*\")==0) return 1;",
            "if(strcmp(s,\"//\")==0) return 2;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) /* keep */",
            "    {",
            "        y();",
            "    }",
            "    if (strcmp(s, \"/*\") == 0)",
            "    {",
            "        return 1;",
            "    }",
            "    if (strcmp(s, \"//\") == 0)",
            "    {",
            "        return 2;",
            "    }",
            "}",
        )
    );
}

#[test]
fn remove_braces_unwraps_cross_line_single_statement_blocks() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x)",
            "{",
            "return 1;",
            "}",
            "else",
            "{",
            "return 0;",
            "}",
            "for(i=0;i<x;i++)",
            "{",
            "sum+=i;",
            "}",
            "while(x)",
            "{",
            "x--;",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "        return 1;",
            "    else",
            "        return 0;",
            "    for (i = 0; i < x; i++)",
            "        sum += i;",
            "    while (x)",
            "        x--;",
            "}",
        )
    );
}

#[test]
fn remove_braces_unwraps_if_constexpr_body() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;

    assert_eq!(
        format_c(
            fixture!("void run(){", "if constexpr(ready) {", "work();", "}", "}",),
            &options,
        ),
        fixture!(
            "void run() {",
            "    if constexpr(ready)",
            "        work();",
            "}",
        )
    );
}

#[test]
fn remove_braces_preserves_lambda_body() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) call([]{ return 1; });\n}\n",
            &options
        ),
        fixture!("void run() {", "    if(ready) call([] { return 1; });", "}",)
    );
}

#[test]
fn remove_braces_preserves_comment_after_removed_opening_brace() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;

    assert_eq!(
        format_c(
            fixture!("void run(){", "if(ready) { // note", "work();", "}", "}",),
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(ready)   // note",
            "        work();",
            "}",
        )
    );
}

#[test]
fn remove_braces_keeps_cross_line_guarded_blocks() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x)",
            "{",
            "// keep",
            "return 1;",
            "}",
            "if(x)",
            "{",
            "if(y) y();",
            "}",
            "if(x)",
            "{",
            "a();",
            "b();",
            "}",
            "if(x)",
            "{",
            "return 2",
            "}",
            "if(x)",
            "{",
            "return 3;",
            "// keep close",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "    {",
            "// keep",
            "        return 1;",
            "    }",
            "    if (x)",
            "    {",
            "        if (y) y();",
            "    }",
            "    if (x)",
            "    {",
            "        a();",
            "        b();",
            "    }",
            "    if (x)",
            "    {",
            "        return 2",
            "    }",
            "    if (x)",
            "    {",
            "        return 3;",
            "// keep close",
            "    }",
            "}",
        )
    );
}

#[test]
fn remove_braces_skips_comments_nested_headers_and_multi_statement_blocks() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x){/* keep */return 1;}",
            "if(x){if(y) y();}",
            "if(x){a(); b();}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "    {",
            "        /* keep */return 1;",
            "    }",
            "    if (x)",
            "    {",
            "        if (y) y();",
            "    }",
            "    if (x)",
            "    {",
            "        a();",
            "        b();",
            "    }",
            "}",
        )
    );
}

#[test]
fn keeps_switch_case_labels_on_separate_lines_under_add_braces() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_switches = true;
    options.add_braces = true;
    options.break_one_line_headers = true;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.break_after_logical = true;
    options.max_continuation_indent = 80;
    options.attach_return_type = true;
    options.attach_return_type_decl = true;
    let actual = format_with(
        fixture!(
            "void f(int x){",
            "    switch (x) {",
            "        case A:",
            "            write_log(LOG_DEBUG, ctx->log, 0,",
            "                      \"message\");",
            "            break;",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch (x) {",
            "        case A:",
            "            write_log(LOG_DEBUG, ctx->log, 0,",
            "                      \"message\");",
            "            break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn keeps_header_comments_on_their_own_indented_lines_when_adding_braces() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_col1_comments = true;
    let actual = format_with(
        fixture!(
            "void f(int x){",
            "if(x)",
            "/* run-in before statement */",
            "return;",
            "if(x)",
            "// line before else",
            "return;",
            "else",
            "return;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    if (x)",
            "        /* run-in before statement */",
            "    {",
            "        return;",
            "    }",
            "    if (x)",
            "        // line before else",
            "    {",
            "        return;",
            "    } else {",
            "        return;",
            "    }",
            "}",
        )
    );
}

#[test]
fn add_braces_preserves_original_tokens_except_inserted_braces() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let source = fixture!("int f(){if(x)return x;else return 0;}");
    let actual = format_with(source, &options);

    assert_eq!(
        non_whitespace_without_braces(&actual),
        non_whitespace_without_braces(source)
    );
}

#[test]
fn remove_braces_preserves_original_tokens_except_deleted_braces() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    let source = fixture!("int f(){if(x){return x;}else{return 0;}}");
    let actual = format_with(source, &options);

    assert_eq!(
        non_whitespace_without_braces(&actual),
        non_whitespace_without_braces(source)
    );
}

#[test]
fn add_one_line_braces_breaks_else_after_rewritten_block() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.add_one_line_braces = true;
    let actual = format_with(
        fixture!("int f(int x){", "if(x) return 1; else return 0;", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) { return 1; }",
            "    else { return 0; }",
            "}",
        )
    );
}

#[test]
fn add_one_line_braces_preserves_split_header_line() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("void run(){\nif(ready)\nwork();\n}\n", &options),
        fixture!("void run() {", "    if(ready)", "    { work(); }", "}",)
    );
}

#[test]
fn add_one_line_braces_keeps_outer_inline_header_at_outer_indent() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(alpha) if(beta) { one(); }",
            "        else { two(); }",
            "}",
        )
    );
}

#[test]
fn add_one_line_braces_recognizes_if_constexpr_body() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("void run(){\nif constexpr(ready)\nwork();\n}\n", &options),
        fixture!(
            "void run() {",
            "    if constexpr(ready)",
            "    { work(); }",
            "}",
        )
    );
}

#[test]
fn remove_braces_unwraps_one_line_control_statements() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x){return 1;} else {return 0;}",
            "for(i=0;i<x;i++){sum+=i;}",
            "while(x){x--;}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "        return 1;",
            "    else",
            "        return 0;",
            "    for (i = 0; i < x; i++)",
            "        sum += i;",
            "    while (x)",
            "        x--;",
            "}",
        )
    );
}

#[test]
fn remove_braces_keeps_one_line_body_with_else_when_requested() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) { work(); } else { stop(); }\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(ready)",
            "        work();   else",
            "        stop();",
            "}",
        )
    );
}

#[test]
fn add_braces_skips_multi_line_conditional_body() {
    let mut options = FormatOptions::default();
    let args = ["--style=kr", "--indent=tab", "--add-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source =
        "void f()\n{\n\tif(a && b)\n\t\tcall(x,\n\t\t     y);\n\telse {\n\t\tother();\n\t}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn add_braces_takes_precedence_over_remove_braces() {
    let mut options = FormatOptions::default();
    let args = ["--style=1tbs", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("int f() { if(x) g(); else h(); }\n", &options),
        "int f()\n{\n    if(x) {\n        g();\n    } else {\n        h();\n    }\n}\n",
    );
}

#[test]
fn lisp_cross_line_removed_brace_closing_gap_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=lisp", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void run() {\n    if (alpha)\n        one();   else\n        two();  }\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha) {\n        one(); } else {\n        two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn lisp_removed_brace_closing_gap_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=lisp", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void run() {\n    if (alpha)\n        one();   else\n        two();  }\n";

    assert_eq!(
        format_c(
            "void run(){\nif (alpha) { one(); } else { two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn pico_cross_line_removed_braces_preserve_body_boundaries() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void run()\n{   if (alpha)\n        one();\n    else\n        two();  }\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha)\n    { one(); }\n    else\n    { two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn run_in_remove_braces_honors_requested_header_breaks() {
    let source = "void run()\n{\n    if (alpha) { one(); } else { two(); }\n}\n";
    let cases = [
        (
            "--style=pico",
            "void run()\n{   if (alpha)\n        one();\n    else\n        two();  }\n",
        ),
        (
            "--style=lisp",
            "void run() {\n    if (alpha)\n        one();\n    else\n        two();  }\n",
        ),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        let args = [style, "--remove-braces", "--break-one-line-headers"].map(str::to_owned);
        apply_command_line_args(&mut options, &args).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
        assert_eq!(format_c(expected, &options), expected);
    }
}

#[test]
fn pico_remove_braces_never_merges_else_with_its_body() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=pico",
        "--remove-braces",
        "--break-one-line-headers",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void f(){if(a){one();}else{two();}}\n", &options),
        "void f() {if(a) {one();} else {two();}}\n",
    );
}

#[test]
fn add_braces_attaches_open_brace_to_separated_header() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(format_c("if(b)\nx;\n", &options), "if(b) {\n    x;\n}\n");
    assert_eq!(
        format_c("while(a)\nx;\n", &options),
        "while(a) {\n    x;\n}\n",
    );
}

#[test]
fn add_braces_attaches_open_brace_before_header_line_comment() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b) // c\nx;\n", &options),
        "if(b) { // c\n    x;\n}\n",
    );
}

#[test]
fn add_braces_keeps_open_brace_on_body_line_after_blank_or_comment_line() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b)\n\nx;\n", &options),
        "if(b)\n\n{\n    x;\n}\n",
    );
    assert_eq!(
        format_c("if(b)\n// c\nx;\n", &options),
        "if(b)\n// c\n{\n    x;\n}\n",
    );
}

#[test]
fn add_braces_attaches_brace_after_do_header_and_breaks_closing_while() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("do x; while(a);\n", &options),
        "do {\n    x;\n}\nwhile(a);\n",
    );
}

#[test]
fn add_braces_breaks_else_from_closing_brace_in_default_style() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b) x; else y;\n", &options),
        "if(b) {\n    x;\n}\nelse {\n    y;\n}\n",
    );
}

#[test]
fn add_braces_keeps_closing_while_attached_with_keep_one_line_statements() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("do x; while(a);\n", &options),
        "do {\n    x;\n} while(a);\n",
    );
}

#[test]
fn add_braces_indents_block_by_same_line_header_nesting() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b) if(c) x; else y;\n", &options),
        "if(b) if(c) {\n        x;\n    }\n    else {\n        y;\n    }\n",
    );
}

#[test]
fn add_braces_indents_nested_same_line_do_chain() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("do do x; while(a); while(b);\n", &options),
        "do do {\n        x;\n    }\n    while(a);\nwhile(b);\n",
    );
}

#[test]
fn add_braces_nested_same_line_do_else_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void f()\n{\n    if (x) if (y) while (z) do\n                {\n                    a++;\n                }\n                while (b);\n        else\n        {\n            c();\n        }\n}\n";

    assert_eq!(
        format_c(
            "void f(){ if(x) if(y) while(z) do a++; while(b); else c(); }\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn add_braces_nested_same_line_if_else_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void f()\n{\n    if (a) for (;;) while (b) if (c)\n                {\n                    d();\n                }\n                else\n                {\n                    e();\n                }\n}\n";

    assert_eq!(
        format_c(
            "void f(){ if(a) for(;;) while(b) if(c) d(); else e(); }\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn add_braces_wraps_statement_after_header_block_comment() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b) /* c */ x;\n", &options),
        "if(b) { /* c */\n    x;\n}\n",
    );
}

#[test]
fn removed_brace_keeps_following_same_line_statement_when_requested() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    options.break_one_line_statements = false;
    let expected = "void run()\n{\n    if (alpha)\n        one();   two();\n}\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha) { one(); } two();\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn cross_line_removed_brace_keeps_closing_header_gap_idempotent() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    options.break_one_line_statements = false;
    let expected = "void run()\n{\n    if (alpha)\n        one();   else\n        two();\n}\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha) {\n        one(); } else {\n        two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn removed_brace_kept_else_is_idempotent() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    options.break_one_line_statements = false;
    let expected = "void run()\n{\n    if (alpha)\n        one();   else\n        two();\n}\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha) { one(); } else { two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn remove_braces_keeps_block_comment_after_changed_spacing_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "x } beta/* block */value\n";

    assert_eq!(format_c("x}beta/* block */value\n", &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn add_braces_malformed_do_scope_word_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "else<%break~>catchwhile,do::trycatch;<=[int#endif;case||callfor}// linebreak\n/* block */Item!switch{,return&&->int*casecontinuethrow\n";

    let first = format_c(source, &options);
    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn add_braces_malformed_preprocessor_after_operator_block_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "<=%{call#if A{\t}alphagamma&&try!=}default continue 11NULL?-:!= [autoNULL!!Item/\n?if||case?\nswitch  continuecatch\n#if Abeta]\n* case+struct!  namespace=\nconstexpr!=  NULLclass  defaulttry\ncontinue\t#else\t}structnamespaceclasshelper<=/* block */#if A<=gammaenum\tnamespaceyclass!:\n#if A\n/* block */\t==class\n";

    let first = format_c(source, &options);
    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn add_braces_keeps_unknown_preprocessor_after_generated_block_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "try#define X(x) \\ {struct\n{\n    = ;\n    #elsehelperenum /,  <= if &&\n        try  #elseclass & beta:: &&\n        ~returnhelper &&\n";

    assert_eq!(
        format_c(
            "try#define X(x) \\{struct{ \n= ;#elsehelperenum/,  <=if&&\ntry  #elseclass&beta::&&\n~returnhelper&&\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn pico_pads_adjacent_close_number_before_trailing_comment_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "breakx...+[!=ifzcontinue[yNULL#endif:!=? auto-NULL&&&try* } 1beta]-> /* block */ // line=\n";

    assert_eq!(
        format_c(
            "breakx...+[!=ifzcontinue[yNULL#endif:!=? auto-NULL&&&try*}1beta]->/* block */// line=\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn add_braces_keeps_embedded_branch_line_unindented_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "{\n#if A#else&structreturn( != enum0alpha]\t#else~ >=::\n";

    assert_eq!(
        format_c(
            "{#if A#else&structreturn(!=enum0alpha]\t#else~>=::\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn add_braces_moves_trailing_comment_into_block() {
    let mut options = FormatOptions::default();
    options.add_braces = true;

    assert_eq!(
        format_c("if(b) x; // c\n", &options),
        "if(b) {\n    x;    // c\n}\n",
    );
}

#[test]
fn add_braces_trailing_comment_spacing_is_idempotent() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    let first = format_c("if(b) x; // c\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn add_braces_malformed_operator_after_embedded_endif_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "case||/ /\tconstexpr/* block */break\n>=namespace\nreturn\n/{enum*\tx\nbreak<===\n&&,/||\nConfig::return\nautogammaifConfig\nclass!y\nstruct{  if\n-\nConfigz  ->if  enumxenum/\n#if A\n==  :[+->catch#endif\ntry\n<=#endifdo?\n(||\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_scope_close_word_after_close_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "continue(  catch\t#endif#define X(x) \\\nforstruct\t-\n;\tbeta\t#if A   ]   [switch gammabeta   ::try\t]  do[  -if}:: }\telse1\nconstexpr\n?  continue/* block */\t-\nnamespace\n<=42x  !\n/* block */switch\t||)#define X(x) \\ /* block */switch,\n#if Atry\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_semicolon_before_dot_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "forstruct:catch0/result:elseItembreak::-42call #elsecase\t|1&&  /&!;.dostructdefault}-::alphaenumdoreturntrywhileConfiggammacase~tryConfig!=[continue;||,}xresult*do:default=\twhilecase;alpha<42returnx\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_run_in_semicolon_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "1y<=>alpha||<=\ndefault#endif][#endif==*constexprswitch||gammaItem;42beta)gamma1:result!continuex?Itemdoif]if!=value?)namespace1throwvalue/!=  ||helperstructdefault{call>=;enum&&namespacevaluethrowenumstructvalue\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_embedded_if_after_blank_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = ":+switchautocatchintgamma.<=,gamma+||Item#if A*>=>12#endifalphaItem,20namespacebreak>result-><=  betacontinuenamespacewhile&&&/* block */]int#define X(x) \\for+voidcall:do{class>4Itemresult17#if Abeta/elseif}&&return#if A/* block */25)/* block */throw>=  do// line8+throw}||gammaautogammaforbreakcasebetacontinue/* block *//* block */(31doint32==// linedogamma)  /40::,class#if A\n\nifalpha||helperfor=auto)default|continuethrowwhilecatch9default\tItem*gamma<=int  enumbetadefaultnamespacevalue<=>>=#endif|ItemItem%gammaenumcontinue>catchcatchcontinueenum9+elsewhilevalueconstexpr~>6(value  throw<=continuecallstruct42case<breakcasetryconstexpr::&>; /* block */call<=>41\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_operator_after_close_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "switch(zthrowswitch>=[}[1&&>alpha?namespace\tbeta==else?}&whilehelper=Item*]1},1if[value\n\n<>=<\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_line_comment_after_embedded_preprocessor_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "call enumelseswitch!=continuestructy/* block */#if A;enum,.}constexpr!=alpha// linevaluealpha/* block */returny\tif=continue{caseNULL<{\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_semicolon_before_spaceship_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "||zcasenamespaceelse4242call/z;<=>=.break#endif\t,!gamma{[value/* block */resultx.gammastruct!=>=>|>fortrygammaItem[42->call=ybreak/Itemresult;{==elsenamespacealphaenum<=>\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_brace_after_inline_preprocessor_header_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "\nnamespace\nautoif\t}\tx#define X(x) \\\nbreak!%alphaauto   /\tdefault\t?()auto\nConfig\t=else\nreturn ->\t+: do\n~#if Acontinue{ %\n#define X(x) \\namespace\t#define X(x) \\<=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_pad_add_braces_malformed_preprocessor_after_semicolon_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=allman",
        "--pad-oper",
        "--pad-header",
        "--add-braces",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = "constexpr\n;#endifcatch\tConfigauto\treturnenum;/* block */[  ydo->Config* <=\n{\nreturnconstexpr+1~z#if A,\nswitch<=auto\nbeta,\n;#if Ahelper// line/42  ,#define X(x) \\returnauto42\nenumConfig\ncontinuegammabeta\n==\nbetahelper+\n~\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pico_remove_braces_malformed_colon_block_after_embedded_define_is_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--remove-braces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let input = " \t(\t Config{\t #define X(x) \\ 42 switch\t/ /\t::\n) &\n\telseswitch\t #else .\t alpha[  )\n\tConfig   if\nbeta\t )\t #else\n\t(\n~\n\tdo\n\tbeta  . \tif  <= \ttry\n\t[\n )\tNULL&&\n(\n:\t {42   // comment\n\t<=\t=   else\tvalue   #define X(x) \\ 42   #else  throw\t while\n namespace\t 0\ncatch\n\t:   throw != Item\tresult\n\t/\n {\t|  42#define X(x) \\/* block */\t #define X(x) \\ 42 \tbeta\t&|\nif]\n <=>  % .==\t<=>\n1\n -> \t#else\tauto   >=\n >  gamma value  struct Config\t throw\nNULL\t ]\n ! \treturn \t42 gamma\tConfig,\n -\n\t>\t<= struct / result \t1   constexpr}   !=   <=>0\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}
