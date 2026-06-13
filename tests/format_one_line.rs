#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::api::format_bytes;
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

#[test]
fn keep_one_line_block_pads_nested_logical_operator() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("void f(){return (a&&b)||c;}\n", &options),
        "void f() {return (a && b) || c;}\n"
    );
}

#[test]
fn keep_one_line_block_does_not_pad_call_braced_argument() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("void f(){call({1,2}, other);}\n", &options),
        "void f() {call({1, 2}, other);}\n"
    );
}

#[test]
fn keep_one_line_switch_does_not_pad_case_label_colons() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void f(){switch(x){case 1:a();break;default:b();}}\n",
            &options,
        ),
        "void f() {switch (x) {case 1: a(); break; default: b();}}\n"
    );
}

#[test]
fn keep_one_line_statements_does_not_force_space_after_case_label_colon() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            fixture!(
                "void f(int x){",
                "switch(x){",
                "case 1:do_a();",
                "default:do_b();",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(int x) {",
            "    switch(x) {",
            "    case 1:do_a();",
            "    default:do_b();",
            "    }",
            "}",
        )
    );
}

#[test]
fn one_line_operator_index_body_splits_with_body_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "class Vector",
                "{",
                "    T &operator[](size_t i) { CHECK_AT(i < size_t(d.size)); detach(); return begin()[i]; }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class Vector",
            "{",
            "    T &operator[](size_t i) {",
            "        CHECK_AT(i < size_t(d.size));",
            "        detach();",
            "        return begin()[i];",
            "    }",
            "};",
        )
    );
}

#[test]
fn operator_overload_one_line_block_breaks_by_default() {
    assert_eq!(
        format_c(
            "class S {\n    S operator+(int i) { return *this; }\n    S operator+=(int i) { return *this; }\n    S operator==(int i) { return *this; }\n    S operator[](int i) { return *this; }\n    S operator()(int i) { return *this; }\n};\n",
            &FormatOptions::default(),
        ),
        "class S {\n    S operator+(int i) {\n        return *this;\n    }\n    S operator+=(int i) {\n        return *this;\n    }\n    S operator==(int i) {\n        return *this;\n    }\n    S operator[](int i) {\n        return *this;\n    }\n    S operator()(int i) {\n        return *this;\n    }\n};\n",
    );
}

#[test]
fn operator_assign_overload_does_not_cascade_indent() {
    assert_eq!(
        format_c(
            "class S {\n    S& operator+=(int i) { x += i; return *this; }\n    S& operator-=(int i) { x -= i; return *this; }\n};\n",
            &FormatOptions::default(),
        ),
        "class S {\n    S& operator+=(int i) {\n        x += i;\n        return *this;\n    }\n    S& operator-=(int i) {\n        x -= i;\n        return *this;\n    }\n};\n",
    );
}

#[test]
fn keeps_empty_one_line_blocks_by_default() {
    let actual = format(fixture!("void empty(){}", "class C{};", "if(x){}"));

    assert_eq!(
        actual,
        fixture!("void empty() {}", "class C {};", "if (x) {}")
    );
}
#[test]
fn keeps_one_line_blocks_when_requested() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    let actual = format_with(
        fixture!("int f(int x){", "if(x){/* keep */return 1;}", "return 0;}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) {/* keep */return 1;}",
            "    return 0;",
            "}",
        )
    );
}
#[test]
fn keep_one_line_statements_keeps_outer_inline_header_at_outer_indent() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(alpha) if(beta) one(); else two();",
            "}",
        )
    );
}

#[test]
fn keeps_one_line_statements_when_requested() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;
    let actual = format_with(fixture!("int f(){", "a(); b();", "c();", "}"), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    a(); b();", "    c();", "}")
    );
}
#[test]
fn keep_one_line_statements_keeps_switch_label_lines() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;
    options.indent_switches = true;
    let actual = format_with(
        fixture!(
            "void f(int x) {",
            "switch (x) {",
            "case 0: case 1:",
            "break;",
            "default: abort();",
            "case 2:",
            "break;",
            "}",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "        case 0: case 1:",
            "            break;",
            "        default: abort();",
            "        case 2:",
            "            break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn case_line_marker_compact_action_braces_stay_on_one_line() {
    assert_eq!(
        format_c(
            fixture!(
                "void f(int value)",
                "{",
                "    switch (value)",
                "    {",
                "    case 1: #line 10 \"generated\"",
                "        {{ p = q; }{ done(); } break; }",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f(int value)",
            "{",
            "    switch (value)",
            "    {",
            "    case 1:",
            "#line 10 \"generated\"",
            "        {{ p = q; }{ done(); } break; }",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_one_line_headers_splits_control_bodies() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let actual = format_with(
        fixture!(
            "int f(int x){",
            "if(x) return 1;",
            "while(x) x--;",
            "do x++; while(x);",
            "if(x) /* keep */ return 2;",
            "if(strcmp(s,\"//\")==0) return 3;",
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
            "    while (x)",
            "        x--;",
            "    do",
            "        x++;",
            "    while (x);",
            "    if (x) /* keep */",
            "        return 2;",
            "    if (strcmp(s, \"//\") == 0)",
            "        return 3;",
            "}",
        )
    );
}
#[test]
fn break_one_line_headers_takes_precedence_over_keep_one_line_statements() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("void run(){\nif(ready) work();\n}\n", &options,),
        fixture!("void run() {", "    if(ready)", "        work();", "}")
    );
}

#[test]
fn break_one_line_headers_overrides_kept_one_line_blocks() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("void run(){\nif(ready) { work(); }\n}\n", &options),
        fixture!(
            "void run() {",
            "    if(ready) {",
            "        work();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_one_line_headers_overrides_added_one_line_braces() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.add_one_line_braces = true;
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("void run(){\nif(ready) work();\n}\n", &options),
        fixture!(
            "void run() {",
            "    if(ready) {",
            "        work();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_one_line_headers_breaks_following_closing_header() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    options.break_one_line_headers = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) { one(); } else { two(); }\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(alpha) {",
            "        one();",
            "    }",
            "    else {",
            "        two();",
            "    }",
            "}",
        )
    );
}

#[test]
fn keep_one_line_statements_respects_broken_closing_header_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) { one(); } else { two(); }\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha) { one(); }",
            "    else { two(); }",
            "}",
        )
    );
}

#[test]
fn default_style_keeps_empty_do_while_on_one_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(void)",
                "{",
                "\tdo { } while (0);",
                "\tcontext(do { } while (0));",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run(void)",
            "{",
            "    do { } while (0);",
            "    context(do { } while (0));",
            "}",
        )
    );
}

#[test]
fn linux_style_keeps_empty_do_while_on_one_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n    do { } while(value < limit);\n}\n",
            &options,
        ),
        "void helper(void)\n{\n    do { } while(value < limit);\n}\n"
    );
}

#[test]
fn allman_style_breaks_empty_do_while_before_while() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n    do { } while(value < limit);\n}\n",
            &options,
        ),
        "void helper(void)\n{\n    do { }\n    while(value < limit);\n}\n"
    );
}

#[test]
fn allman_detaches_while_after_empty_do_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void run(void)", "{", "\tdo { } while (0);", "}"),
            &options,
        ),
        fixture!("void run(void)", "{", "    do { }", "    while (0);", "}",)
    );
}

#[test]
fn default_style_generated_do_body_detaches_while() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(void)",
                "{",
                "\tdo {",
                "\t\tcall();",
                "\t} while (--count);",
                "\tdo { call(); } while (--count);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run(void)",
            "{",
            "    do {",
            "        call();",
            "    } while (--count);",
            "    do {",
            "        call();",
            "    }",
            "    while (--count);",
            "}",
        )
    );
}

#[test]
fn one_true_brace_style_keeps_generated_do_while_together() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=1tbs".to_string(),
            "--keep-one-line-blocks".to_string(),
            "--keep-one-line-statements".to_string(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void run(){\ndo one(); while(alpha);\n}\n", &options),
        fixture!("void run()", "{", "    do { one(); } while(alpha);", "}",)
    );
}

#[test]
fn lisp_explicit_block_keep_overrides_added_brace_breaking() {
    for add_option in ["--add-braces", "--add-one-line-braces"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(
            &mut options,
            &[
                "--style=lisp".to_string(),
                add_option.to_string(),
                "--keep-one-line-blocks".to_string(),
            ],
        )
        .expect("valid options");

        assert_eq!(
            format_c("void run(){\nif(alpha) one(); else two();\n}\n", &options,),
            fixture!(
                "void run() {",
                "    if(alpha) { one(); }",
                "    else { two(); } }",
            )
        );
    }
}

#[test]
fn break_one_line_headers_recognizes_if_constexpr_body() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("void run(){\nif constexpr(ready) work();\n}\n", &options,),
        fixture!(
            "void run() {",
            "    if constexpr(ready)",
            "        work();",
            "}",
        )
    );
}

#[test]
fn break_one_line_headers_in_pico_splits_only_run_in_switch_header() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.break_one_line_headers = true;

    // Breaking a run-in switch header does not rewrite its case statements.
    assert_eq!(
        format_c(
            "void run(){\nswitch(value){case 1: one(); break;}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{   switch(value)",
            "    {   case 1: one(); break;} }",
        )
    );
}

#[test]
fn break_one_line_headers_still_applies_when_lambda_prevents_added_braces() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.break_one_line_headers = true;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) call([]{ return 1; });\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(ready)",
            "        call([] { return 1; });",
            "}",
        )
    );
}

#[test]
fn preserves_inner_spacing_of_kept_one_line_blocks() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;

    assert_eq!(
        format_c(fixture!("void f(void){a();}"), &options),
        fixture!("void f(void) {a();}")
    );
    assert_eq!(
        format_c(fixture!("void f(void){ a(); }"), &options),
        fixture!("void f(void) { a(); }")
    );
    assert_eq!(
        format_c(fixture!("void f(void){a(); }"), &options),
        fixture!("void f(void) {a(); }")
    );
    assert_eq!(
        format_c(fixture!("void f(void){ }"), &options),
        fixture!("void f(void) { }")
    );
}
#[test]
fn preserves_inner_spacing_of_nested_kept_one_line_blocks() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(fixture!("void f(void){a();b();if(x){y();z();}}"), &options),
        fixture!("void f(void) {a(); b(); if(x) {y(); z();}}")
    );
}
#[test]
fn pads_space_after_closing_brace_before_word_in_kept_blocks() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(fixture!("void f(void){if(x){a();}else{b();}}"), &options),
        fixture!("void f(void) {if(x) {a();} else {b();}}")
    );
    assert_eq!(
        format_c(fixture!("void f(void){do{a();}while(x);}"), &options),
        fixture!("void f(void) {do {a();} while(x);}")
    );
    assert_eq!(
        format_c(fixture!("void f(void){struct{int x;}v;}"), &options),
        fixture!("void f(void) {struct {int x;} v;}")
    );
    assert_eq!(
        format_c(fixture!("void f(void){if(x){a();}  else{b();}}"), &options),
        fixture!("void f(void) {if(x) {a();}  else {b();}}")
    );
}
#[test]
fn while_returns_to_do_level_after_kept_one_line_body() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    let source = fixture!(
        "void run(void)",
        "{",
        "    do",
        "    { beta(); count++; }",
        "    while (count < 10); // note",
        "}",
    );
    assert_eq!(format_c(source, &options), source);
}

#[test]
fn macro_before_one_line_block_stays_separate_by_default() {
    let source = fixture!(
        "void f()",
        "{",
        "    MACRO_SETUP",
        "    { begin(); return VALUE; }",
        "    MACRO_BREAK",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn comment_only_one_line_blocks_stay_inline_by_default() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (x) {/* comment */}",
        "",
        "    if (x) {;/* comment */}",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// Ref-qualified members use the same one-line policy as other definitions.
#[test]
fn ref_qualified_member_uses_function_block_layout() {
    assert_eq!(
        format_c(
            fixture!(
                "class Test {",
                "    int lvalue() & { call(); return 0; }",
                "    int rvalue() && { call(); return 0; }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class Test {",
            "    int lvalue() & {",
            "        call();",
            "        return 0;",
            "    }",
            "    int rvalue() && {",
            "        call();",
            "        return 0;",
            "    }",
            "};",
        )
    );
}

#[test]
fn keep_one_line_statements_still_splits_when_block_is_broken_open() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("void f() { a(); b(); }\n", &options),
        "void f() {\n    a();\n    b();\n}\n",
    );
}

#[test]
fn keep_one_line_statements_splits_members_when_struct_block_expands() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("struct S { int a; int b; };\n", &options),
        "struct S {\n    int a;\n    int b;\n};\n",
    );
}

#[test]
fn default_style_breaks_else_when_one_line_block_expands() {
    assert_eq!(
        format_c(
            "void f() {\n    if(x){y();}else{v();}\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    if(x) {\n        y();\n    }\n    else {\n        v();\n    }\n}\n",
    );
}

#[test]
fn keep_one_line_statements_keeps_do_while_attached_when_block_expands() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("void f() {\n    do{step();}while(cond);\n}\n", &options,),
        "void f() {\n    do {\n        step();\n    } while(cond);\n}\n",
    );
}

#[test]
fn keep_one_line_blocks_pads_space_before_brace_after_bare_control_header() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["-O".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f() {\n    if (a){g();}\n    else{h();}\n    do{x++;} while (a);\n    try{risky();} catch (...){handle();}\n}\n",
            &options,
        ),
        "void f() {\n    if (a) {g();}\n    else {h();}\n    do {x++;}\n    while (a);\n    try {risky();}\n    catch (...) {handle();}\n}\n",
    );
}

#[test]
fn keep_one_line_blocks_keeps_struct_trailing_declarator_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["-O".to_owned()]).expect("valid options");
    let source = "struct S {int m; double n;} s = {1,2.0};\n";

    assert_eq!(format_c(source, &options), source);
}

// Blocks containing case labels are always split; one-line options cannot create partial layouts.
#[test]
fn one_line_block_options_split_blocks_with_case_labels() {
    let source = "void g() {switch (k){case 1:do_a();break;default:do_b();}}\n";
    let expected = "void g() {\n    switch (k) {\n    case 1:\n        do_a();\n        break;\n    default:\n        do_b();\n    }\n}\n";

    for argument in ["-O", "--add-one-line-braces"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[argument.to_owned()]).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn keep_one_line_statements_attached_default_after_case_block_uses_case_indent() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){switch(value){case 1:{step();break;}default:stop();}}\n",
            &options,
        ),
        "void run() {\n    switch(value) {\n    case 1: {\n        step();\n        break;\n    } default:stop();\n    }\n}\n",
    );
}

#[test]
fn keep_one_line_pads_space_before_brace_after_else_following_inline_block() {
    let mut options = FormatOptions::default();
    let args = ["-O", "-o"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void f() {\n    if (a){g();} else{h();}\n}\n", &options,),
        "void f() {\n    if (a) {g();} else {h();}\n}\n",
    );
}

#[test]
fn keep_one_line_blocks_breaks_else_after_inline_block_without_keep_statements() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["-O".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f() {\n    if (a){g();} else{h();}\n}\n", &options,),
        "void f() {\n    if (a) {g();}\n    else {h();}\n}\n",
    );
}

#[test]
fn keep_one_line_statements_keeps_do_while_and_try_catch_inline() {
    let mut options = FormatOptions::default();
    let args = ["-O", "-o"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source =
        "void f() {\n    do {x++;} while (a);\n    try {risky();} catch (...) {handle();}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn keep_one_line_blocks_spaces_trailing_declarator_after_aggregate() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;

    assert_eq!(
        format_c("struct M{int a;}x;\n", &options),
        "struct M {int a;} x;\n",
    );
}

#[test]
fn keep_one_line_blocks_preserves_class_brace_with_access_modifier() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;

    assert_eq!(
        format_c("class M{public:int a;};\n", &options),
        "class M {public:int a;};\n",
    );
}

#[test]
fn keep_one_line_statements_unindents_access_label_with_attached_member() {
    let mut options = FormatOptions::default();
    let args = ["--style=kr", "--keep-one-line-statements"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("class Map{public:T key;U val;};\n", &options),
        "class Map\n{\npublic:T key;\n    U val;\n};\n",
    );
}

// Pico splits connecting headers only when the enclosing block spans lines.
#[test]
fn pico_breaks_else_after_one_line_block_in_multiline_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f()\n{\nif (a){g();} else{h();}\n}\n", &options,),
        "void f()\n{   if (a) {g();}\n    else {h();} }\n",
    );
}

#[test]
fn pico_breaks_do_while_after_one_line_block_in_multiline_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f()\n{\ndo{x++;} while (a);\n}\n", &options),
        "void f()\n{   do {x++;}\n    while (a); }\n",
    );
}

#[test]
fn pico_breaks_catch_after_one_line_block_in_multiline_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f()\n{\ntry{risky();} catch (...){handle();}\n}\n",
            &options,
        ),
        "void f()\n{   try {risky();}\n    catch (...) {handle();} }\n",
    );
}

#[test]
fn pico_keeps_else_attached_in_one_line_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f(){ if(a){g();} else{h();} }\n", &options),
        "void f() { if(a) {g();} else {h();} }\n",
    );
}

#[test]
fn keep_one_line_blocks_attaches_pointer_declarator_after_aggregate() {
    let mut options = FormatOptions::default();
    options.break_one_line_blocks = false;
    let source = "struct node { int val; } *head = NULL;\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pico_attaches_pointer_declarator_after_aggregate() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");
    let source = "union U { int a; } *p;\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn keep_one_line_statements_keeps_statement_after_closing_brace_attached() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c("int main() { if(x) { a(); } b(); }\n", &options,),
        "int main() {\n    if(x) {\n        a();\n    } b();\n}\n",
    );
}

#[test]
fn lisp_keeps_statement_after_closing_brace_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=lisp".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int main() { for(int i=0;i<n;i++) { g(i); } return 0; }\n",
            &options,
        ),
        "int main() {\n    for(int i=0; i<n; i++) {\n        g(i); } return 0; }\n",
    );
}

#[test]
fn keep_one_line_blocks_keeps_class_with_defaulted_member() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--keep-one-line-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "class Vec { int a; Vec()=default; int b; };\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn run_in_styles_honor_explicit_one_line_header_breaks() {
    let source = "void f(){\nif(a) run();\n}\n";

    let mut pico = FormatOptions::default();
    let pico_args = ["--break-one-line-headers", "--style=pico"].map(str::to_owned);
    apply_command_line_args(&mut pico, &pico_args).expect("valid options");
    assert_eq!(
        format_c(source, &pico),
        "void f()\n{   if(a)\n        run(); }\n",
    );

    let mut lisp = FormatOptions::default();
    let lisp_args = ["--break-one-line-headers", "--style=lisp"].map(str::to_owned);
    apply_command_line_args(&mut lisp, &lisp_args).expect("valid options");
    assert_eq!(
        format_c(source, &lisp),
        "void f() {\n    if(a)\n        run(); }\n",
    );
}

#[test]
fn for_header_semicolons_do_not_suppress_run_in_header_breaks() {
    let source = "void f(){\nfor(;;) run();\n}\n";

    let mut pico = FormatOptions::default();
    let pico_args = ["--break-one-line-headers", "--style=pico"].map(str::to_owned);
    apply_command_line_args(&mut pico, &pico_args).expect("valid options");
    assert_eq!(
        format_c(source, &pico),
        "void f()\n{   for(;;)\n        run(); }\n",
    );

    let mut lisp = FormatOptions::default();
    let lisp_args = ["--break-one-line-headers", "--style=lisp"].map(str::to_owned);
    apply_command_line_args(&mut lisp, &lisp_args).expect("valid options");
    assert_eq!(
        format_c(source, &lisp),
        "void f() {\n    for(;;)\n        run(); }\n",
    );
}

#[test]
fn break_one_line_headers_breaks_nested_header_inside_kept_block() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=lisp",
        "--remove-braces",
        "--break-one-line-headers",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "void run() {\n    if (alpha) {\n        if (beta)\n            one(); }\n    else\n        two();  }\n";

    assert_eq!(
        format_c(
            "void run()\n{\n    if (alpha) { if (beta) one(); } else { two(); }\n}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn pico_header_break_keeps_brace_line_comment_with_header() {
    let mut options = FormatOptions::default();
    let args = ["--break-one-line-headers", "--style=pico"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void f(){\nif(a) { // note\nrun();\n}\n}\n", &options,),
        "void f()\n{   if(a)   // note\n    {   run(); } }\n",
    );
}

#[test]
fn run_in_header_break_keeps_multi_statement_lines() {
    let source = "void f(){\nif(a) run(); else stop();\ndo wait(); while(a);\n}\n";
    let cases = [
        (
            "--style=pico",
            "void f()\n{   if(a) run(); else stop();\n    do wait(); while(a); }\n",
        ),
        (
            "--style=lisp",
            "void f() {\n    if(a) run(); else stop();\n    do wait(); while(a); }\n",
        ),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        let args = ["--break-one-line-headers", style].map(str::to_owned);
        apply_command_line_args(&mut options, &args).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn pico_header_break_keeps_run_in_one_line_blocks() {
    let mut options = FormatOptions::default();
    let args = ["--break-one-line-headers", "--style=pico"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(){\nif(a) { run(); } else { stop(); }\n}\n",
            &options,
        ),
        "void f()\n{   if(a)\n    {   run(); }\n    else\n    {   stop(); } }\n",
    );
}

#[test]
fn statement_keep_is_independent_of_enclosing_one_line_block() {
    let mut options = FormatOptions::default();
    let args = ["--keep-one-line-statements", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(){if(a) one(); else two(); for(;;) step();}\n",
            &options,
        ),
        "void f() {\n    if(a) one();\n    else two();\n    for(;;) step();\n}\n",
    );
}

#[test]
fn explicit_statement_keep_preserves_multi_statement_header_line() {
    let mut options = FormatOptions::default();
    let args = ["--keep-one-line-statements", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void f(){\nif(a) run(); else stop();\n}\n", &options,),
        "void f() {\n    if(a) run(); else stop();\n}\n",
    );
}

#[test]
fn run_in_header_break_overrides_explicit_statement_keep() {
    let source = "void f(){\nif(a) run();\n}\n";
    let expected = "void f()\n{   if(a)\n        run(); }\n";
    let cases = [
        [
            "--keep-one-line-statements",
            "--break-one-line-headers",
            "--style=pico",
        ],
        [
            "--break-one-line-headers",
            "--style=pico",
            "--keep-one-line-statements",
        ],
    ];

    for arguments in cases {
        let mut options = FormatOptions::default();
        let arguments = arguments.map(str::to_owned);
        apply_command_line_args(&mut options, &arguments).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn lisp_add_one_line_braces_does_not_keep_existing_one_line_block() {
    let source = "struct S {\nint get() const { return value; }\n};\n";
    let mut added = FormatOptions::default();
    let added_args = ["--style=lisp", "--add-one-line-braces"].map(str::to_owned);
    apply_command_line_args(&mut added, &added_args).expect("valid options");
    assert_eq!(
        format_c(source, &added),
        "struct S {\n    int get() const {\n        return value; } };\n",
    );

    let kept = "struct S {\n    int get() const { return value; } };\n";
    let cases: &[&[&str]] = &[
        &["--style=lisp", "--keep-one-line-blocks"],
        &[
            "--style=lisp",
            "--add-one-line-braces",
            "--keep-one-line-blocks",
        ],
        &[
            "--style=lisp",
            "--keep-one-line-blocks",
            "--add-one-line-braces",
        ],
    ];
    for arguments in cases {
        let mut options = FormatOptions::default();
        let arguments: Vec<_> = arguments.iter().map(|value| (*value).to_owned()).collect();
        apply_command_line_args(&mut options, &arguments).expect("valid options");
        assert_eq!(format_c(source, &options), kept);
    }
}

#[test]
fn lisp_breaks_closing_header_after_run_in_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=lisp".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int f(int x)\n{\nif (x) {\ng();\n} else {\nh();\n}\n}\n",
            &options,
        ),
        "int f(int x) {\n    if (x) {\n        g(); }\n    else {\n        h(); } }\n",
    );
    assert_eq!(
        format_c("int f()\n{\ndo {\ng();\n} while (x);\n}\n", &options),
        "int f() {\n    do {\n        g(); }\n    while (x); }\n",
    );
    assert_eq!(
        format_c(
            "void f()\n{\ntry {\ng();\n} catch (...) {\nh();\n}\n}\n",
            &options,
        ),
        "void f() {\n    try {\n        g(); }\n    catch (...) {\n        h(); } }\n",
    );
}

#[test]
fn break_one_line_headers_keeps_unterminated_body_indent_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=java", "--pad-oper", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "while(x)\n    switchyenum\n    helper\n";

    assert_eq!(
        format_c("while(x) switchyenum\nhelper\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn break_one_line_headers_keeps_embedded_else_marker_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let expected = "|#else^~do\n#else\n        enumy+=autoresult\n";

    assert_eq!(
        format_c("|#else^~do#else\nenumy+=autoresult\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn break_one_line_headers_keeps_malformed_else_preprocessor_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=java", "--pad-oper", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "else\n#else\nconst\n";

    assert_eq!(format_c("else#else\nconst\n", &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn break_one_line_headers_clears_malformed_else_if_preprocessor_body_after_semicolon() {
    let mut options = FormatOptions::default();
    let args = ["--style=java", "--pad-oper", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected =
        "a % * #if A & else\n#if A\n    classbreak#endif\n    void.10struct;\n/ int\n}\n";

    assert_eq!(
        format_c(
            "a % * #if A\n&else#if A\nclassbreak#endif\nvoid.10struct;/int}\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn break_one_line_headers_keeps_malformed_else_if_preprocessor_idempotent() {
    let mut options = FormatOptions::default();
    let args = ["--style=java", "--pad-oper", "--break-one-line-headers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let expected = "else\n#if A\n    classbreak#endif\n";

    assert_eq!(
        format_c("else#if A\nclassbreak#endif\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn break_one_line_headers_splits_statement_after_header_block_comment() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("if(b) /* c */ x;\n", &options),
        "if(b) /* c */\n    x;\n",
    );
}

#[test]
fn add_one_line_braces_indents_closing_whiles_of_same_line_do_chain() {
    let mut options = FormatOptions::default();
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("do do x; while(a); while(b);\n", &options),
        "do do { x; }\n    while(a);\nwhile(b);\n",
    );
}

#[test]
fn break_one_line_headers_splits_same_line_header_chain() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("if(b) if(c) x;\n", &options),
        "if(b)\n    if(c)\n        x;\n",
    );
    assert_eq!(
        format_c("if(b) while(c) x;\n", &options),
        "if(b)\n    while(c)\n        x;\n",
    );
}

#[test]
fn break_one_line_headers_splits_same_line_do_chain() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;

    assert_eq!(
        format_c("do do x; while(a); while(b);\n", &options),
        "do\n    do\n        x;\n    while(a);\nwhile(b);\n",
    );
}

#[test]
fn run_in_styles_keep_one_line_do_chain_at_source_indent() {
    let input = "do do do x; while(a); while(b); while(c);\n";

    for args in [
        ["--style=pico", "--remove-braces"],
        ["--style=lisp", "--indent=force-tab"],
    ] {
        let mut options = FormatOptions::default();
        let args = args.map(str::to_owned);
        apply_command_line_args(&mut options, &args).expect("valid options");

        assert_eq!(format_c(input, &options), input);
    }
}

#[test]
fn break_one_line_headers_malformed_embedded_else_body_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = "*#else\n(}  namespace,[\nyenum  #else\n42 #if Aswitchwhiletry #else::;alpha /* block */\n;\t(42\n#else=ydefault\n,#endif\nreturn0// line-+&&::#endif\nbetawhile#if A/break}\t==while  #define X(x) \\ helper  !\nhelperwhileelse\n=:\nclass\nalpha:=\n!=Config\tcall(x} zswitchdefault~switch)helperelse\ntry0structNULLdefault#define X(x) \\\telse>=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_one_line_headers_malformed_operator_body_after_embedded_if_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = "returnclass\nItemcontinue#else}\n->{xswitch#else:\nenum\n-[!)continueelse+\n#if A-={\tbetay*\n=\n<=<=\n;-defaultconstexpr\n0\n!\n,\n!defaultalpha->defaultenumbreak\t*\tdo,\nfor}tryItem\n->\tfor\nz!\t<=continue->+,1\nhelper\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_one_line_headers_malformed_paren_before_embedded_if_body_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = "!/* block */elsecatch(#define X(x) \\// line\n42\n!\nclass)\n#define X(x) \\::returnwhile&&1::NULL\n==\nelsez\n]/\tfor=*NULL\t&&-beta{\nItemNULL #endif->enum\nalpha] x,betaswitch->\nalphaz#elsetrystruct;\t,/* block */ continue=x\nelse{ x\n(\nhelperclasscatchbetabeta!={z\n{\n#if Adefaultreturn ->\n;::\ndoconstexprcatch42 0\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_one_line_headers_malformed_preprocessor_block_body_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = ">==else#endifItem+%struct*structwhilewhilewhilevoidcall/* block */ifclassbeta%<=>#if Agamma<=#if A{!void/%autovoid\ncase:value<alphaelse<try>=~>result#endifbetagamma)elseconstexpr10betaresult}return~#define X(x) \\\nconstexpr%=default&  +];\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_header_orphan_else_in_unclosed_paren_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let first = format_c("(\nelse;c\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_one_line_headers_malformed_catch_word_continuation_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = " ::  (\nhelper\nz{   ;#elseswitch   || ==break   }break\t&&%  auto==   !=\tstruct)constexprdo\ncontinue\tconstexpr:\n-\t{\thelperdo\n-> #else!<=enum\n}!=\t-helper  (\t% %while\ny\t1\n+   /* block */\telse ;catch catchItem enumstructstruct\n?=::   do\nz\t%\nelse\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn unmatched_close_after_commented_header_keeps_physical_row() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let source = b"/**/for\n)d";

    assert_eq!(
        format_bytes(source, &options).expect("format bytes"),
        source,
    );
}

#[test]
fn break_one_line_headers_malformed_else_branch_indent_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = "catch  xcatch <=break}]alphaNULLenumelseconstexprcontinue{/[default\n<=else!;\n==do(betastruct(\n#endif  1break\t#else\n/\n)class+~:\n0!=doNULLcontinue\n=struct-\n>=  /* block */for\n#endif switch&&break\n]class  )#else\n-structenumifdo)dodefaultx\ncatchdodefault-enum==x\t42default\t==\t!=// line\t-/-\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn break_one_line_headers_malformed_inline_else_preprocessor_is_idempotent() {
    let mut options = FormatOptions::default();
    options.break_one_line_headers = true;
    let input = "struct-><=do#elsebreak!=gamma%else<!#if AItemcatch,~->\ntry!::\n\ntry\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn bare_top_level_one_line_block_is_kept_at_file_start() {
    let options = FormatOptions::default();

    for source in [
        "{ int a; }\n",
        "{int a;}\n",
        "{ 1, { 2, 3 } };\n",
        "{{int a;}}\n",
    ] {
        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn bare_top_level_one_line_block_is_kept_after_comment_and_preprocessor() {
    let options = FormatOptions::default();

    for source in [
        "// c\n{ int a; }\n",
        "#include <a.h>\n{ int a; }\n",
        "\n{ int a; }\n",
    ] {
        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn consecutive_bare_top_level_one_line_blocks_are_kept() {
    let options = FormatOptions::default();

    for source in [
        "{ int a; }\n{ int b; }\n",
        "{ int a; } { int b; }\n",
        "{\n}\n{ int a; }\n",
    ] {
        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn bare_top_level_one_line_block_breaks_after_statement() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c("int x;\n{ int a; }\n", &options),
        "int x;\n{\n    int a;\n}\n",
    );
    assert_eq!(
        format_c("{ int a; } int x;\n{ int b; }\n", &options),
        "{ int a; } int x;\n{\n    int b;\n}\n",
    );
}

#[test]
fn bare_one_line_block_inside_bare_block_is_kept_and_indented() {
    assert_eq!(
        format_c("{\n{ int a; }\n}\n", &FormatOptions::default()),
        "{\n    { int a; }\n}\n",
    );
}

#[test]
fn header_one_line_block_inside_bare_block_still_breaks() {
    assert_eq!(
        format_c("{\nif (x) { y(); }\n}\n", &FormatOptions::default(),),
        "{\n    if (x) {\n        y();\n    }\n}\n",
    );
}

#[test]
fn bare_top_level_open_brace_run_keeps_one_line() {
    let options = FormatOptions::default();

    for (source, expected) in [
        ("{{\n", "{   {\n"),
        ("{{{\n", "{   {   {\n"),
        ("{{{{\n", "{   {   {   {\n"),
    ] {
        assert_eq!(format_c(source, &options), expected);
    }
}
