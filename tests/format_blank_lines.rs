#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, LineBetweenMembers, apply_command_line_args};

#[test]
fn break_blocks_delete_empty_lines_reindents_block_comment_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--break-blocks".to_owned(),
            "--delete-empty-lines".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{",
                "    bar();",
                "",
                "/*//BEGIN debug",
                "    trace();",
                "//END debug*/",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo()",
            "{",
            "    bar();",
            "    /*//BEGIN debug",
            "        trace();",
            "    //END debug*/",
            "}",
        )
    );
}

#[test]
fn delete_empty_lines_keeps_blank_between_leading_line_comments() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=run-in".to_owned(),
            "--break-blocks=all".to_owned(),
            "--delete-empty-lines".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{",
                "    // comment1",
                "",
                "    // comment2",
                "    if (isFoo)",
                "        bar();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo()",
            "{   // comment1",
            "",
            "    // comment2",
            "    if (isFoo)",
            "        bar();",
            "}",
        )
    );
}

#[test]
fn break_blocks_does_not_insert_blank_after_preprocessor_conditional() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--break-blocks".to_owned()]).expect("valid options");
    let source = fixture!(
        "",
        "void foo()",
        "{",
        "    if (isFoo)",
        "    {",
        "        bar1();",
        "    }",
        "",
        "#if 0",
        "    bar2();",
        "    bar3();",
        "#endif",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn default_preserves_leading_trailing_and_consecutive_blank_lines() {
    let source = fixture!("", "", "a;", "", "", "", "b;", "", "",);
    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn blank_line_inside_continuation_keeps_statement_indent() {
    let actual = format_c(
        fixture!("int value = a", "", "+ b", ";"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!("int value = a", "", "            + b", "            ;")
    );
}

#[test]
fn delete_empty_lines_removes_blank_lines_inside_command_blocks() {
    let mut options = FormatOptions::default();
    options.delete_empty_lines = true;
    let actual = format_with(
        fixture!(
            "",
            "",
            "int f(){",
            "",
            "",
            "int x;",
            "",
            "",
            "return 0;",
            "}",
            "",
            "",
            "int g(){",
            "#if A",
            "",
            "",
            "return 1;",
            "#endif",
            "}",
            "",
            "",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "",
            "",
            "int f()",
            "{",
            "    int x;",
            "    return 0;",
            "}",
            "",
            "",
            "int g()",
            "{",
            "#if A",
            "    return 1;",
            "#endif",
            "}",
            "",
            "",
        )
    );
}
#[test]
fn delete_empty_lines_preserves_top_level_blank_runs() {
    let mut options = FormatOptions::default();
    options.delete_empty_lines = true;
    let source = fixture!(
        "int alpha;",
        "",
        "",
        "if (alpha) {",
        "    work();",
        "}",
        "",
        "",
        "int beta;",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn delete_empty_lines_preserves_namespace_blanks_but_removes_function_blanks() {
    let mut options = FormatOptions::default();
    options.delete_empty_lines = true;
    let source = fixture!(
        "namespace N {",
        "",
        "int x;",
        "",
        "}",
        "void f() {",
        "",
        "int y;",
        "",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "namespace N {",
            "",
            "int x;",
            "",
            "}",
            "void f() {",
            "    int y;",
            "}",
        )
    );
}
#[test]
fn delete_empty_lines_keeps_repeated_blank_lines_in_protected_contexts() {
    let mut options = FormatOptions::default();
    options.delete_empty_lines = true;
    let actual = format_with(
        fixture!(
            "namespace N {",
            "",
            "",
            "int n;",
            "}",
            "struct S{",
            "",
            "",
            "int x;",
            "};",
            "enum E{",
            "",
            "",
            "A",
            "};",
            "int a[]={",
            "",
            "",
            "1,",
            "2",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace N",
            "{",
            "",
            "",
            "int n;",
            "}",
            "struct S",
            "{",
            "",
            "",
            "    int x;",
            "};",
            "enum E {",
            "",
            "",
            "    A",
            "};",
            "int a[] =",
            "{",
            "",
            "",
            "    1,",
            "    2",
            "};",
        )
    );
}
#[test]
fn delete_empty_lines_removes_blank_lines_around_comments_inside_command_blocks() {
    let mut options = FormatOptions::default();
    options.delete_empty_lines = true;

    assert_eq!(
        format_with(
            fixture!(
                "void f(){",
                "alpha();",
                "",
                "/* note */",
                "",
                "beta();",
                "}"
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    alpha();",
            "    /* note */",
            "    beta();",
            "}",
        )
    );
}
#[test]
fn line_between_members_separates_methods_but_not_field_group() {
    let mut options = FormatOptions::default();
    options.line_between_members = LineBetweenMembers::Members;
    let actual = format_c(
        fixture!(
            "struct Item {",
            "    int alpha;",
            "    int beta;",
            "    void first();",
            "    void second();",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct Item {",
            "    int alpha;",
            "    int beta;",
            "",
            "    void first();",
            "",
            "    void second();",
            "};",
        )
    );
}

#[test]
fn line_between_members_all_separates_field_group() {
    let mut options = FormatOptions::default();
    options.line_between_members = LineBetweenMembers::All;
    let actual = format_c(
        fixture!("struct Item {", "    int alpha;", "    int beta;", "};"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("struct Item {", "    int alpha;", "", "    int beta;", "};")
    );
}

#[test]
fn line_between_members_separates_top_level_functions() {
    let mut options = FormatOptions::default();
    options.line_between_members = LineBetweenMembers::Members;
    let actual = format_c(
        fixture!(
            "void first()",
            "{",
            "    alpha();",
            "}",
            "void second()",
            "{",
            "    beta();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void first()",
            "{",
            "    alpha();",
            "}",
            "",
            "void second()",
            "{",
            "    beta();",
            "}",
        )
    );
}

#[test]
fn line_between_members_survives_delete_empty_lines_and_break_blocks() {
    let mut options = FormatOptions::default();
    options.line_between_members = LineBetweenMembers::Members;
    options.delete_empty_lines = true;
    options.break_blocks = true;
    let actual = format_c(
        fixture!(
            "class Item {",
            "public:",
            "    void first();",
            "    void second();",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item {",
            "public:",
            "    void first();",
            "",
            "    void second();",
            "};",
        )
    );
}

#[test]
fn line_between_members_does_not_pad_access_labels_or_closing_braces() {
    let mut options = FormatOptions::default();
    options.line_between_members = LineBetweenMembers::All;
    let actual = format_c(
        fixture!(
            "class Outer {",
            "public:",
            "    int alpha;",
            "private:",
            "    int beta;",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Outer {",
            "public:",
            "    int alpha;",
            "private:",
            "    int beta;",
            "};",
        )
    );
}

#[test]
fn blank_line_between_split_switch_labels_is_preserved() {
    let actual = format(fixture!(
        "int f(int x){switch(x){case 1:return 1;",
        "",
        "default:return 0;}}",
    ));
    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "        return 1;",
            "",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_blank_line_between_header_and_own_line_brace() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("if (x)", "", "{", "    call();", "}"), &options),
        fixture!("if (x)", "", "{", "    call();", "}")
    );
    assert_eq!(
        format_c(fixture!("int f()", "", "{", "    call();", "}"), &options),
        fixture!("int f()", "", "{", "    call();", "}")
    );
    assert_eq!(
        format_c(fixture!("namespace N", "", "{", "int a;", "}"), &options),
        fixture!("namespace N", "", "{", "int a;", "}")
    );
}
#[test]
fn preserves_multiple_blank_lines_before_own_line_brace() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("if (x)", "", "", "{", "    call();", "}"),
            &options
        ),
        fixture!("if (x)", "", "", "{", "    call();", "}")
    );
}

fn break_blocks_options() -> FormatOptions {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.break_blocks = true;
    options
}

#[test]
fn break_blocks_prepends_blank_before_top_level_header() {
    let options = break_blocks_options();

    assert_eq!(
        format_c(fixture!("if (alpha) {", "    work();", "}"), &options),
        fixture!("", "if (alpha)", "{", "    work();", "}"),
    );
}

#[test]
fn break_blocks_keeps_leading_line_comment_attached_to_top_level_header() {
    let options = break_blocks_options();

    assert_eq!(
        format_c(
            fixture!("// note", "if (alpha) {", "    work();", "}"),
            &options,
        ),
        fixture!("// note", "if (alpha)", "{", "    work();", "}"),
    );
}

#[test]
fn break_blocks_prepends_blank_before_column_one_comment_header_in_block() {
    let options = break_blocks_options();

    assert_eq!(
        format_c(
            fixture!(
                "void helper(void) {",
                "    setup();",
                "// note",
                "    if (alpha) {",
                "        work();",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void helper(void)",
            "{",
            "    setup();",
            "",
            "// note",
            "    if (alpha) {",
            "        work();",
            "    }",
            "}",
        ),
    );
}

#[test]
fn break_blocks_appends_blank_before_statement_after_top_level_header_block() {
    let options = break_blocks_options();

    assert_eq!(
        format_c(
            fixture!("if (alpha) {", "    work();", "}", "done();"),
            &options,
        ),
        fixture!("", "if (alpha)", "{", "    work();", "}", "", "done();"),
    );
}

#[test]
fn break_blocks_appends_blank_before_preprocessor_after_header_block() {
    let options = break_blocks_options();

    assert_eq!(
        format_c(
            fixture!("#if ENABLED", "if (alpha) {", "    work();", "}", "#endif"),
            &options,
        ),
        fixture!(
            "#if ENABLED",
            "",
            "if (alpha)",
            "{",
            "    work();",
            "}",
            "",
            "#endif",
        ),
    );
}

#[test]
fn break_blocks_surrounds_block_and_following_header() {
    let mut options = break_blocks_options();
    options.delete_empty_lines = true;
    let source = fixture!(
        "void helper(void) {",
        "    if (alpha) {",
        "        first();",
        "    }",
        "    middle();",
        "    if (beta) {",
        "        second();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    if (alpha) {",
            "        first();",
            "    }",
            "",
            "    middle();",
            "",
            "    if (beta) {",
            "        second();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_blocks_surrounds_nested_header_after_statement() {
    let options = break_blocks_options();
    let source = fixture!(
        "void helper(void) {",
        "    for (int i = 0; i < n; i++) {",
        "        Item *it = lookup(i);",
        "        if (it->valid) {",
        "            process(it);",
        "        }",
        "        cleanup(it);",
        "    }",
        "    finalize();",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    for (int i = 0; i < n; i++) {",
            "        Item *it = lookup(i);",
            "",
            "        if (it->valid) {",
            "            process(it);",
            "        }",
            "",
            "        cleanup(it);",
            "    }",
            "",
            "    finalize();",
            "}",
        )
    );
}

#[test]
fn break_blocks_keeps_do_while_tail_attached() {
    let options = break_blocks_options();
    let source = fixture!(
        "void helper(void) {",
        "    do {",
        "        step();",
        "    } while (alpha);",
        "    int beta = 1;",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    do {",
            "        step();",
            "    } while (alpha);",
            "",
            "    int beta = 1;",
            "}",
        )
    );
}

#[test]
fn break_blocks_separates_switch_labels() {
    let options = break_blocks_options();
    let source = fixture!(
        "void helper(int alpha) {",
        "    switch (alpha) {",
        "    case 1:",
        "        first();",
        "        break;",
        "    case 2:",
        "        second();",
        "        break;",
        "    default:",
        "        other();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(int alpha)",
            "{",
            "    switch (alpha) {",
            "    case 1:",
            "        first();",
            "        break;",
            "",
            "    case 2:",
            "        second();",
            "        break;",
            "",
            "    default:",
            "        other();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_closing_header_blocks_separates_else_branch() {
    let mut options = break_blocks_options();
    options.break_closing_header_blocks = true;
    let source = fixture!(
        "void helper(void) {",
        "    if (alpha) {",
        "        first();",
        "    } else {",
        "        second();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    if (alpha) {",
            "        first();",
            "",
            "    } else {",
            "        second();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_all_blocks_keeps_outer_else_at_outer_header_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.break_blocks = true;
    options.break_closing_header_blocks = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha){if(beta){one();}else{two();}}else{three();}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha)",
            "    {",
            "        if(beta)",
            "        {",
            "            one();",
            "        }",
            "",
            "        else",
            "        {",
            "            two();",
            "        }",
            "    }",
            "",
            "    else",
            "    {",
            "        three();",
            "    }",
            "}",
        )
    );
}

#[test]
fn vtk_break_all_blocks_keeps_closing_header_body_at_brace_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;
    options.break_blocks = true;
    options.break_closing_header_blocks = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha){if(beta){one();}else{two();}}else{three();}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha)",
            "        {",
            "        if(beta)",
            "            {",
            "            one();",
            "            }",
            "",
            "        else",
            "            {",
            "            two();",
            "            }",
            "        }",
            "",
            "    else",
            "        {",
            "        three();",
            "        }",
            "}",
        )
    );
}

#[test]
fn pico_break_blocks_does_not_separate_do_from_closing_while() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.break_blocks = true;
    options.break_closing_header_blocks = true;

    assert_eq!(
        format_c("void run(){\ndo{one();}while(ready);\n}\n", &options),
        fixture!("void run()", "{   do {one();}", "    while(ready); }")
    );
}

#[test]
fn pico_break_all_blocks_starts_after_first_run_in_block() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.break_blocks = true;
    options.break_closing_header_blocks = true;

    assert_eq!(
        format_c("void run(){\nif(ready){one();}else{two();}\n}\n", &options,),
        fixture!(
            "void run()",
            "{   if(ready) {one();}",
            "",
            "    else {two();} }",
        )
    );
}

#[test]
fn break_blocks_does_not_indent_a_following_do_header() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.break_blocks = true;
    options.break_closing_header_blocks = true;

    assert_eq!(
        format_c(
            "void run(){if(ready){one();}else{two();}do{step();}while(ready);}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(ready)",
            "    {",
            "        one();",
            "    }",
            "",
            "    else",
            "    {",
            "        two();",
            "    }",
            "",
            "    do",
            "    {",
            "        step();",
            "    }",
            "    while(ready);",
            "}",
        )
    );
}

#[test]
fn break_blocks_prepends_blank_before_commented_header() {
    let options = break_blocks_options();
    let source = fixture!(
        "void helper(void) {",
        "    setup();",
        "    // explain",
        "    if (alpha) {",
        "        work();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    setup();",
            "",
            "    // explain",
            "    if (alpha) {",
            "        work();",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_blocks_blank_after_do_while_with_trailing_comment() {
    let mut options = break_blocks_options();
    options.brace_style = BraceStyle::None;
    options.attach_closing_while = true;
    let source = fixture!(
        "void run(void)",
        "{",
        "    alpha();",
        "    do",
        "    {",
        "        beta();",
        "    }",
        "    while (count < 10); // note",
        "    gamma();",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run(void)",
            "{",
            "    alpha();",
            "",
            "    do",
            "    {",
            "        beta();",
            "    } while (count < 10); // note",
            "",
            "    gamma();",
            "}",
        )
    );
}

#[test]
fn break_blocks_blank_after_one_line_header_with_trailing_comment() {
    let mut options = break_blocks_options();
    options.brace_style = BraceStyle::None;
    let source = fixture!(
        "void run(void)",
        "{",
        "    if (alpha) work(); // note",
        "    next();",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run(void)",
            "{",
            "    if (alpha) work(); // note",
            "",
            "    next();",
            "}",
        )
    );
}

#[test]
fn break_blocks_appends_blank_after_braceless_one_line_for() {
    let mut options = FormatOptions::default();
    let args = ["--style=kr", "--break-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void g() {\n    int a = 1;\n    for(int i=0;i<n;i++) f(i);\n    int b = 2;\n}\n",
            &options,
        ),
        "void g()\n{\n    int a = 1;\n\n    for(int i=0; i<n; i++) f(i);\n\n    int b = 2;\n}\n",
    );
}

#[test]
fn break_blocks_appends_blank_after_for_in_broken_one_line_block() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--break-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "int main(){ int a=1; for(int i=0;i<n;i++) f(i); return a; }\n",
            &options,
        ),
        "int main()\n{\n    int a=1;\n\n    for(int i=0; i<n; i++) f(i);\n\n    return a;\n}\n",
    );
}

#[test]
fn lisp_break_all_blocks_keeps_blank_lines_between_blocks() {
    let mut options = FormatOptions::default();
    let args = ["--style=lisp", "--break-blocks=all"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(){if(a){one();}else{two();}do{step();}while(a);}\n",
            &options,
        ),
        "void f() {\n    if(a) {\n        one(); }\n\n    else {\n        two(); } do {\n        step(); }\n    while(a); }\n",
    );
}

#[test]
fn break_blocks_surrounds_braceless_body() {
    let options = break_blocks_options();
    let source = fixture!(
        "void helper(void) {",
        "    prepare();",
        "    if (alpha)",
        "        work();",
        "    done();",
        "}",
    );
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void helper(void)",
            "{",
            "    prepare();",
            "",
            "    if (alpha)",
            "        work();",
            "",
            "    done();",
            "}",
        )
    );
}

#[test]
fn empty_line_fill_uses_previous_output_indent() {
    let mut options = FormatOptions::default();
    options.empty_line_fill = true;
    let actual = format_with(
        fixture!(
            "int f(){",
            "if(x){",
            "",
            "return 1;}",
            "struct S{",
            "",
            "int x;};}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (x)",
            "    {",
            "    ",
            "        return 1;",
            "    }",
            "    struct S",
            "    {",
            "    ",
            "        int x;",
            "    };",
            "}",
        )
    );
}

#[test]
fn empty_line_fill_uses_configured_macro_block_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--fill-empty-lines".to_owned(),
            "--macro-block=REGION_BEGIN:REGION_END".to_owned(),
        ],
    )
    .expect("valid empty-line and macro-block options");

    assert_eq!(
        format_c(
            fixture!(
                "REGION_BEGIN(Item, Base)",
                "    ENTRY(Item::First)",
                "",
                "",
                "    ENTRY(Item::Second)",
                "REGION_END()",
            ),
            &options,
        ),
        fixture!(
            "REGION_BEGIN(Item, Base)",
            "    ENTRY(Item::First)",
            "    ",
            "    ",
            "    ENTRY(Item::Second)",
            "REGION_END()",
        ),
    );
}

#[test]
fn indent_preproc_block_does_not_fill_empty_lines_without_option() {
    let mut options = FormatOptions::default();
    options.indent_preproc_block = true;

    assert_eq!(
        format_c(
            fixture!(
                "#ifdef ALPHA",
                "    #define FIRST",
                "    ",
                "    #define SECOND",
                "#endif",
            ),
            &options,
        ),
        fixture!(
            "#ifdef ALPHA",
            "    #define FIRST",
            "",
            "    #define SECOND",
            "#endif",
        ),
    );
}

#[test]
fn empty_line_fill_uses_enum_member_indent() {
    let mut options = FormatOptions::default();
    options.empty_line_fill = true;

    assert_eq!(
        format_c(
            "\nenum\n{\n    FIRST,\n    SECOND,\n\n    THIRD,\n",
            &options,
        ),
        "\nenum\n{\n    FIRST,\n    SECOND,\n    \n    THIRD,\n",
    );
}

#[test]
fn case_empty_line_fill_uses_pre_adjustment_indent() {
    let mut options = FormatOptions::default();
    options.empty_line_fill = true;
    let actual = format_with(
        fixture!("int f(int x){switch(x){case 1:{", "", "return 1;", "}}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "    {",
            "    ",
            "        return 1;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn blank_line_stops_split_else_extra_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.add_braces = true;
    options.break_one_line_headers = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "if(a){",
            "work_a();",
            "}else",
            "#endif",
            "",
            "{",
            "work_b();",
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
            "    if (a) {",
            "        work_a();",
            "    } else",
            "#endif",
            "",
            "    {",
            "        work_b();",
            "    }",
            "}",
        )
    );
}
#[test]
fn delete_empty_lines_removes_single_blank_inside_command_blocks() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--delete-empty-lines".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    call();\n\n    // note\n    done();\n}\n",
            &options,
        ),
        "\nvoid foo()\n{\n    call();\n    // note\n    done();\n}\n"
    );
}
#[test]
fn kr_function_brace_after_blank_line_stays_split() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--delete-empty-lines".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void foo()\n\n{\n    if (isFoo)\n\n    {\n        bar = 1;\n    }\n}\n",
            &options,
        ),
        "void foo()\n\n{\n    if (isFoo) {\n        bar = 1;\n    }\n}\n",
    );
}
#[test]
fn java_function_brace_after_blank_line_stays_split() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--delete-empty-lines".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void foo()\n\n{\n    if (isFoo)\n\n    {\n        bar = 1;\n    }\n}\n",
            &options,
        ),
        "void foo()\n\n{\n    if (isFoo) {\n        bar = 1;\n    }\n}\n",
    );
}

#[test]
fn break_blocks_inserts_blank_before_preprocessor_guarded_header() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--break-blocks=all".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void run(){\n#if A\nif(a){call();}\n#endif\n}\n", &options,),
        "void run()\n{\n#if A\n\n    if(a)\n    {\n        call();\n    }\n\n#endif\n}\n",
    );
}

#[test]
fn break_blocks_inserts_blank_before_guarded_header_leading_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--break-blocks=all".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n#if A\n// note\nif(a){call();}\n#endif\n}\n",
            &options,
        ),
        "void run()\n{\n#if A\n\n// note\n    if(a)\n    {\n        call();\n    }\n\n#endif\n}\n",
    );
}

#[test]
fn break_blocks_inserts_blank_before_guarded_header_block_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--break-blocks=all".to_owned(),
            "--delete-empty-lines".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n#if A\nfirst();\n#else\n/* note */\nwhile(b){call();}\n#endif\n}\n",
            &options,
        ),
        "void run()\n{\n#if A\n    first();\n#else\n\n    /* note */\n    while(b)\n    {\n        call();\n    }\n\n#endif\n}\n",
    );
}

#[test]
fn break_blocks_inserts_blank_before_preprocessor_guarded_case() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--break-blocks=all".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){switch(value){\n#if A\ncase 1:break;\n#endif\n}}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n#if A\n\n    case 1:\n        break;\n#endif\n    }\n}\n",
    );
}

#[test]
fn break_blocks_run_in_switch_branches_separate_each_case() {
    let source = "void run(int value){switch(value){\n#if A\ncase 1:first();break;\n#else\ncase 2:second();break;\n#endif\ndefault:break;}}\n";
    let cases = [
        (
            "pico",
            "void run(int value)\n{   switch(value)\n    {\n#if A\n\n        case 1:first(); break;\n#else\n\n        case 2:second(); break;\n#endif\n\n        default:break; } }\n",
        ),
        (
            "lisp",
            "void run(int value) {\n    switch(value) {\n#if A\n\n    case 1:first(); break;\n#else\n\n    case 2:second(); break;\n#endif\n\n    default:break; } }\n",
        ),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        apply_command_line_args(
            &mut options,
            &[format!("--style={style}"), "--break-blocks=all".to_owned()],
        )
        .expect("valid options");

        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn break_blocks_delete_empty_lines_removes_blanks_between_comments() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--break-blocks".to_owned(),
            "--delete-empty-lines".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n// first\n\n// second\n\n/* third */\n\ncall();\n}\n",
            &options,
        ),
        "void run()\n{\n// first\n// second\n    /* third */\n    call();\n}\n",
    );
}

#[test]
fn break_blocks_does_not_cross_indent_on_marker() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--break-blocks".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n// *INDENT-OFF*\nif(alpha){call();}\n// *INDENT-ON*\nif(beta){call();}\n}\n",
            &options,
        ),
        "void run()\n{\n// *INDENT-OFF*\nif(alpha){call();}\n// *INDENT-ON*\n    if(beta)\n    {\n        call();\n    }\n}\n",
    );
}
