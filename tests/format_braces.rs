#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::api::format_bytes;
use cstyle::config::{
    BraceStyle, FormatOptions, IndentStyle, MinConditionalIndent, PointerAlign,
    apply_command_line_args,
};

#[test]
fn backslash_function_body_stays_split_and_compact() {
    assert_eq!(
        format_c(
            fixture!("template<typename T>", "void f() \\", "    { g(); }",),
            &FormatOptions::default(),
        ),
        fixture!("template<typename T>", "void f() \\", "{ g(); }",)
    );
}

#[test]
fn backslash_function_body_keeps_multiple_statements_compact() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename T>",
                "void f() \\",
                "    { T d; read(d); value = wrap(d); }",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename T>",
            "void f() \\",
            "{ T d; read(d); value = wrap(d); }",
        )
    );
}

#[test]
fn ratliff_separated_definition_brace_and_body_share_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!(
            "void run()",
            "/* head */",
            "{",
            "/* body */",
            "call();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "/* head */",
            "    {",
            "    /* body */",
            "    call();",
            "    }",
        )
    );
}

#[test]
fn whitesmith_separated_definition_brace_and_body_share_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let actual = format_c(
        fixture!(
            "void run()",
            "/* head */",
            "{",
            "/* body */",
            "call();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "/* head */",
            "    {",
            "    /* body */",
            "    call();",
            "    }",
        )
    );
}

#[test]
fn vtk_function_brace_indent_is_independent_of_extern_context() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!("extern \"C\"{", "void run(){", "call();", "}", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "extern \"C\" {",
            "    void run()",
            "    {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn vtk_class_method_brace_stays_at_method_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid VTK style");

    assert_eq!(
        format_c(
            fixture!(
                "class Item {",
                "public:",
                "void run() {",
                "if (ready) {",
                "step();",
                "}",
                "}",
                "};",
            ),
            &options,
        ),
        fixture!(
            "class Item",
            "{",
            "public:",
            "    void run()",
            "    {",
            "        if (ready)",
            "            {",
            "            step();",
            "            }",
            "    }",
            "};",
        ),
    );
}

#[test]
fn ratliff_nested_inline_else_braces_are_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");
    let first = format_c(
        fixture!(
            "void run(){",
            "if(alpha) if(beta){",
            "call();",
            "}else{",
            "other();",
            "}",
            "}",
        ),
        &options,
    );
    let second = format_c(&first, &options);

    assert_eq!(second, first);
}

#[test]
fn vtk_and_ratliff_indent_standalone_block_braces_from_the_body_column() {
    let source = fixture!("void run(){", "{", "int value=1;", "}", "call();", "}",);

    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "        {",
            "        int value=1;",
            "        }",
            "    call();",
            "}",
        )
    );

    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run() {",
            "        {",
            "        int value=1;",
            "        }",
            "    call();",
            "    }",
        )
    );
}

#[test]
fn gnu_standalone_block_closer_aligns_with_its_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!("void run(){", "{", "int value=1;", "}", "call();", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "{",
            "    {",
            "        int value=1;",
            "    }",
            "    call();",
            "}",
        )
    );
}

#[test]
fn whitesmith_treats_anonymous_namespace_as_namespace_scope() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(fixture!("namespace{", "int value;", "}"), &options),
        fixture!("namespace", "{", "int value;", "}")
    );
}

#[test]
fn whitesmith_treats_inline_namespace_as_namespace_scope() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("inline namespace Alpha{", "int value;", "}"),
            &options,
        ),
        fixture!("inline namespace Alpha", "{", "int value;", "}")
    );
}

#[test]
fn ratliff_treats_inline_namespace_as_namespace_scope() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("inline namespace Alpha{", "int value;", "}"),
            &options,
        ),
        fixture!("inline namespace Alpha {", "int value;", "}")
    );
}

#[test]
fn horstmann_does_not_run_in_anonymous_namespace_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(fixture!("namespace{", "int value;", "}"), &options),
        fixture!("namespace", "{", "int value;", "}")
    );
}

#[test]
fn java_attaches_function_brace_but_keeps_standalone_block_split() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(fixture!("void run()", "{", "    {"), &options),
        fixture!("void run() {", "    {")
    );
}

#[test]
fn java_keeps_comment_separated_standalone_brace_on_own_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    if (ready)",
                "    {",
                "        // note",
                "        {",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "    if (ready) {",
            "        // note",
            "        {",
        )
    );
}

#[test]
fn java_style_does_not_attach_body_block_comment_to_opening_brace() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;

    assert_eq!(
        format_c("// c\nvoid f(){/*x*/return;}\n", &options),
        "// c\nvoid f() {\n    /*x*/return;\n}\n"
    );
}

#[test]
fn breaks_multi_line_function_definition_brace_with_enum_return_type() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "enum result_t fn(",
            "    const Item *item) {",
            "    int ad;",
            "    return 0;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "enum result_t fn(",
            "    const Item *item)",
            "{",
            "    int ad;",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn breaks_struct_return_type_function_brace_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;

    assert_eq!(
        format_c(
            fixture!(
                "struct gamma {",
                "    int alpha;",
                "};",
                "",
                "struct gamma *",
                "make_gamma(const char *name, int value)",
                "{",
                "    int beta;",
                "}"
            ),
            &options
        ),
        fixture!(
            "struct gamma {",
            "    int alpha;",
            "};",
            "",
            "struct gamma *",
            "make_gamma(const char *name, int value)",
            "{",
            "    int beta;",
            "}"
        )
    );
}

#[test]
fn keeps_for_brace_attached_inside_bare_block_under_preprocessor_condition() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "    } else {",
            "",
            "#if (DBG)",
            "        {",
            "        item_t text[10];",
            "        str_t addr;",
            "",
            "        addr.data = text;",
            "",
            "        for (i = 0; i < n; i++) {",
            "            g(x);",
            "        }",
            "        }",
            "#endif",
            "    }",
            "}",
        ),
        &options,
    );

    assert!(
        actual.contains("for (i = 0; i < n; i++) {"),
        "for-brace must stay attached (consistent OneTrueBrace), got:\n{actual}"
    );
    assert!(
        !actual.contains("for (i = 0; i < n; i++)\n"),
        "for-brace must not break onto its own line, got:\n{actual}"
    );
}
#[test]
fn attaches_brace_after_multi_line_for_header_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    for (a = first();",
            "         a != end();",
            "         a = next(a))",
            "    {",
            "        body();",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    for (a = first();",
            "            a != end();",
            "            a = next(a)) {",
            "        body();",
            "    }",
            "}",
        )
    );
}
#[test]
fn keeps_block_brace_on_own_line_after_multi_line_preprocessor_condition() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    c = 1;",
            "#if (GUARD_A \\",
            "     && defined GUARD_B)",
            "    {",
            "        body();",
            "    }",
            "#endif",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    c = 1;",
            "#if (GUARD_A \\",
            "     && defined GUARD_B)",
            "    {",
            "        body();",
            "    }",
            "#endif",
            "}",
        )
    );
}
#[test]
fn indent_braces_indents_opening_closing_and_nested_brace_lines() {
    let mut options = FormatOptions::default();
    options.indent_braces = true;
    let actual = format_with(fixture!("int f(){if(x){return 1;}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "    {",
            "    if (x)",
            "        {",
            "        return 1;",
            "        }",
            "    }",
        )
    );
}
#[test]
fn indent_braces_handles_comment_adjacent_braces() {
    let mut options = FormatOptions::default();
    options.indent_braces = true;
    let actual = format_with(
        fixture!("int f(){", "if(x) // keep", "{return 1;}", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "    {",
            "    if (x) // keep",
            "        {",
            "        return 1;",
            "        }",
            "    }",
        )
    );
}
#[test]
fn trailing_comment_after_broken_brace_stays_with_block() {
    let line_comment = fixture!(
        "void run(void)",
        "{",
        "    if (alpha)",
        "    {   // note",
        "        work();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(line_comment, &FormatOptions::default()),
        line_comment
    );

    let block_comment = fixture!(
        "void run(void)",
        "{",
        "    if (alpha)",
        "    {   /* note */",
        "        work();",
        "    }",
        "}",
    );
    assert_eq!(
        format_c(block_comment, &FormatOptions::default()),
        block_comment
    );

    let mut horstmann = FormatOptions::default();
    horstmann.brace_style = BraceStyle::Horstmann;
    assert_eq!(
        format_c(line_comment, &horstmann),
        fixture!(
            "void run(void)",
            "{   if (alpha)",
            "    {   // note",
            "        work();",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_trailing_comment_column_when_breaking_brace() {
    let mut allman = FormatOptions::default();
    allman.brace_style = BraceStyle::Allman;

    assert_eq!(
        format_c(
            fixture!("void run() {    // note", "    work();", "}"),
            &allman
        ),
        fixture!("void run()      // note", "{", "    work();", "}")
    );
}
#[test]
fn default_brace_style_preserves_source_brace_placement() {
    let source = fixture!(
        "int a(){return 1;}",
        "int b()",
        "{",
        "return 2;",
        "}",
        "void f() { if (x) { g(); } }",
        "void h() {",
        "if (y) k();",
        "}",
        "int m(){a();b();}",
    );
    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "int a() {",
            "    return 1;",
            "}",
            "int b()",
            "{",
            "    return 2;",
            "}",
            "void f() {",
            "    if (x) {",
            "        g();",
            "    }",
            "}",
            "void h() {",
            "    if (y) k();",
            "}",
            "int m() {",
            "    a();",
            "    b();",
            "}",
        )
    );
}
#[test]
fn default_style_expands_compact_function_and_if_else_blocks() {
    let actual = format(fixture!(
        "int f(int x){if(x<10){x=x+1;}else{x=x-1;}return x;}"
    ));
    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x < 10)",
            "    {",
            "        x = x + 1;",
            "    }",
            "    else",
            "    {",
            "        x = x - 1;",
            "    }",
            "    return x;",
            "}",
        )
    );
}
#[test]
fn function_returning_pointer_uses_definition_block_layout() {
    assert_eq!(
        format(fixture!("int (*factory())(int){return nullptr;}")),
        fixture!("int (*factory())(int)", "{", "    return nullptr;", "}",)
    );
}

#[test]
fn attach_closing_while_keeps_do_while_on_closing_brace() {
    let mut options = FormatOptions::default();
    options.attach_closing_while = true;
    let actual = format_with(fixture!("void f(){do{x++;}while(x<3);}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    do",
            "    {",
            "        x++;",
            "    } while (x < 3);",
            "}",
        )
    );
}
#[test]
fn attach_brace_modifiers_attach_selected_block_types() {
    let mut options = FormatOptions::default();
    options.attach_extern_c = true;
    options.attach_namespace = true;
    options.attach_class = true;
    let actual = format_with(
        fixture!(
            "extern \"C\"{int f();}",
            "namespace N{int x;}",
            "class C{int x;};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "extern \"C\" {",
            "    int f();",
            "}",
            "namespace N {",
            "int x;",
            "}",
            "class C {",
            "    int x;",
            "};",
        )
    );
}

#[test]
fn attach_namespaces_attaches_nested_namespace_braces() {
    let mut options = FormatOptions::default();
    options.attach_namespace = true;

    assert_eq!(
        format_c(
            fixture!(
                "namespace Alpha",
                "{",
                "namespace Beta",
                "{",
                "namespace Gamma",
                "{",
                "}",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "namespace Alpha {",
            "namespace Beta {",
            "namespace Gamma {",
            "}",
            "}",
            "}",
        ),
    );
}

#[test]
fn attach_classes_attaches_nested_class_braces() {
    let mut options = FormatOptions::default();
    options.attach_class = true;

    assert_eq!(
        format_c(
            fixture!("class Alpha", "{", "class Beta", "{", "class Gamma", "{"),
            &options,
        ),
        fixture!("class Alpha {", "    class Beta {", "        class Gamma {",),
    );
}

#[test]
fn attach_inline_attaches_command_braces_inside_classes() {
    let mut options = FormatOptions::default();
    options.attach_inline = true;
    let actual = format_with(fixture!("class C{void f(){if(x){y();}}};"), &options);

    assert_eq!(
        actual,
        fixture!(
            "class C",
            "{",
            "    void f() {",
            "        if (x) {",
            "            y();",
            "        }",
            "    }",
            "};",
        )
    );
}

#[test]
fn attach_inlines_does_not_attach_nested_struct_brace() {
    let mut options = FormatOptions::default();
    options.attach_inline = true;
    let source = fixture!("class Item", "{", "    struct State", "    {",);

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn one_true_brace_style_breaks_definition_and_attaches_control_brace() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(fixture!("int f(int x){if(x){return 1;}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) {",
            "        return 1;",
            "    }",
            "}",
        )
    );
}
#[test]
fn attaches_closing_else_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!("int f(int x){if(x){return 1;}else{return 0;}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x) {",
            "        return 1;",
            "    } else {",
            "        return 0;",
            "    }",
            "}",
        )
    );
}
#[test]
fn attaches_try_catch_and_microsoft_try_handlers_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "void f(){try{a();}catch(e){b();}__try{c();}__finally{d();}__try{e();}__except(x){f();}}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    try {",
            "        a();",
            "    } catch (e) {",
            "        b();",
            "    }",
            "    __try {",
            "        c();",
            "    } __finally {",
            "        d();",
            "    }",
            "    __try {",
            "        e();",
            "    } __except (x) {",
            "        f();",
            "    }",
            "}",
        )
    );
}
#[test]
fn struct_cast_in_condition_keeps_control_brace_classification() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "if (outer) {",
            "if (item->len == 1 ||",
            "item->len > sizeof(((struct item *)nullptr)->name)) {",
            "log_error(ALPHA, beta, 0,",
            "\"text\");",
            "}",
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
            "    if (outer) {",
            "        if (item->len == 1 ||",
            "            item->len > sizeof(((struct item *)nullptr)->name)) {",
            "            log_error(ALPHA, beta, 0,",
            "                      \"text\");",
            "        }",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_source_spaces_before_attached_opening_brace() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f(void)  {", "    a();", "}"), &options),
        fixture!("void f(void)  {", "    a();", "}")
    );
    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    if (x)  {",
                "        a();",
                "    }",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    if (x)  {",
            "        a();",
            "    }",
            "}"
        )
    );
    assert_eq!(
        format_c(fixture!("void f(void){", "    a();", "}"), &options),
        fixture!("void f(void) {", "    a();", "}")
    );
}
#[test]
fn keeps_empty_block_on_its_own_source_line() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f()", "{}"), &options),
        fixture!("void f()", "{}")
    );
    assert_eq!(
        format_c(fixture!("if (x)", "{}"), &options),
        fixture!("if (x)", "{}")
    );
    assert_eq!(
        format_c(fixture!("class C", "{};"), &options),
        fixture!("class C", "{};")
    );
}
#[test]
fn keeps_attached_empty_block_attached() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f() {}"), &options),
        fixture!("void f() {}")
    );
}
#[test]
fn keeps_split_empty_block_split() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f()", "{", "}"), &options),
        fixture!("void f()", "{", "}")
    );
}

#[test]
fn multiline_function_definition_keeps_empty_body_at_definition_column() {
    let source = fixture!(
        "",
        "void call(const Type /*type*/,",
        "          Item* /*item*/,",
        "          const Config* /*config*/ = 0)",
        "{}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stroustrup_breaks_single_parameter_operator_overload_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=stroustrup".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("inline int operator~(int arg) {", "    return arg;", "}"),
            &options,
        ),
        fixture!("inline int operator~(int arg)", "{", "    return arg;", "}",)
    );
}

#[test]
fn allman_call_operator_definition_breaks_before_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("struct Item{int operator()(int value){return value;}};"),
            &options,
        ),
        fixture!(
            "struct Item",
            "{",
            "    int operator()(int value)",
            "    {",
            "        return value;",
            "    }",
            "};",
        )
    );
}

#[test]
fn keeps_split_function_signature_brace_broken() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("struct foo* bar", "(void)", "{", "}"), &options),
        fixture!("struct foo* bar", "(void)", "{", "}")
    );
    assert_eq!(
        format_c(fixture!("int bar", "(void)", "{", "}"), &options),
        fixture!("int bar", "(void)", "{", "}")
    );
}

#[test]
fn whitesmith_split_function_brace_uses_definition_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;

    assert_eq!(
        format_c(fixture!("int fn(", " int x)", "{", "}"), &options),
        fixture!("int fn(", "    int x)", "    {", "    }")
    );
}

#[test]
fn keeps_statement_expression_brace_unspaced_in_default_and_one_true_brace_styles() {
    let source = fixture!(
        "void f(void)",
        "{",
        "    call({",
        "        helper(value);",
        "    });",
        "}"
    );
    let default_options = FormatOptions::default();
    assert_eq!(format_c(source, &default_options), source);

    let mut one_true_brace_options = FormatOptions::default();
    one_true_brace_options.brace_style = BraceStyle::OneTrueBrace;
    assert_eq!(format_c(source, &one_true_brace_options), source);
}

#[test]
fn attaches_brace_to_methods_with_trailing_qualifiers() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "class Foo {",
            "public:",
            "bool isPressed() const {",
            "return p;",
            "}",
            "int plain() {",
            "return 0;",
            "}",
            "int inlined() const {return 1;}",
            "int multi() noexcept override {return 2;}",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Foo {",
            "public:",
            "    bool isPressed() const {",
            "        return p;",
            "    }",
            "    int plain() {",
            "        return 0;",
            "    }",
            "    int inlined() const {",
            "        return 1;",
            "    }",
            "    int multi() noexcept override {",
            "        return 2;",
            "    }",
            "};",
        )
    );
}

#[test]
fn attaches_brace_to_methods_with_trailing_macro_qualifier() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "class C {",
            "public:",
            "Result result() const METHOD_OVERRIDE {",
            "return Result(1, 0);",
            "}",
            "void handle(Event *event) METHOD_OVERRIDE {",
            "process(event);",
            "}",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class C {",
            "public:",
            "    Result result() const METHOD_OVERRIDE {",
            "        return Result(1, 0);",
            "    }",
            "    void handle(Event *event) METHOD_OVERRIDE {",
            "        process(event);",
            "    }",
            "};",
        )
    );
}

#[test]
fn kr_breaks_body_braces_for_repeated_macro_shaped_definitions() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source = fixture!(
        "RUN(Config, alpha)",
        "{",
        "\tint x = 1;",
        "}",
        "",
        "RUN(Config, beta)",
        "{",
        "\tint y = 1;",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn empty_blocks_are_idempotent() {
    let once = format(fixture!("void f(){}"));
    let twice = format(&once);

    assert_eq!(once, fixture!("void f() {}"));
    assert_eq!(twice, once);
}

#[test]
fn formats_simple_function() {
    let actual = format(fixture!("int main(){return 0;}"));
    assert_eq!(actual, fixture!("int main()", "{", "    return 0;", "}"));
}

#[test]
fn whitesmith_tabbed_definition_braces_keep_structural_tab_ownership() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_switches = true;
    options.indent_style = IndentStyle::Tabs;
    let actual = format_c(fixture!("void run(void){", "call();", "}"), &options);

    assert_eq!(
        actual,
        fixture!("void run(void)", "\t{", "\tcall();", "\t}")
    );
}

#[test]
fn indented_styles_keep_comment_separated_control_brace_and_body_together() {
    let source = fixture!(
        "void run() {",
        "if(ready)",
        "/* note */",
        "{",
        "work();",
        "}",
        "}",
    );

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_classes = true;
    whitesmith.indent_switches = true;
    assert_eq!(
        format_c(source, &whitesmith),
        fixture!(
            "void run()",
            "    {",
            "    if(ready)",
            "        /* note */",
            "        {",
            "        work();",
            "        }",
            "    }",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    ratliff.indent_classes = true;
    assert_eq!(
        format_c(source, &ratliff),
        fixture!(
            "void run() {",
            "    if(ready)",
            "        /* note */",
            "        {",
            "        work();",
            "        }",
            "    }",
        )
    );
}

#[test]
fn indented_styles_keep_preprocessor_separated_control_brace_and_body_together() {
    let source = fixture!(
        "void run() {",
        "if(ready)",
        "#if ENABLED",
        "{",
        "work();",
        "}",
        "#endif",
        "}",
    );

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_classes = true;
    whitesmith.indent_switches = true;
    assert_eq!(
        format_c(source, &whitesmith),
        fixture!(
            "void run()",
            "    {",
            "    if(ready)",
            "#if ENABLED",
            "        {",
            "        work();",
            "        }",
            "#endif",
            "    }",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    ratliff.indent_classes = true;
    assert_eq!(
        format_c(source, &ratliff),
        fixture!(
            "void run() {",
            "    if(ready)",
            "#if ENABLED",
            "        {",
            "        work();",
            "        }",
            "#endif",
            "    }",
        )
    );
}

#[test]
fn gnu_function_try_blocks_use_control_block_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;

    assert_eq!(
        format_c(
            fixture!(
                "void run() try",
                "{",
                "work();",
                "}",
                "catch(...)",
                "{",
                "stop();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() try",
            "    {",
            "        work();",
            "    }",
            "catch(...)",
            "    {",
            "        stop();",
            "    }",
        )
    );
}

#[test]
fn definition_brace_style_applies_after_requires_clause() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;

    assert_eq!(
        format_c(
            fixture!(
                "template<class T>",
                "int make(T value) requires Ready<T>",
                "{",
                "return value;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "template<class T>",
            "int make(T value) requires Ready<T>",
            "{",
            "    return value;",
            "}",
        )
    );
}

#[test]
fn ratliff_multiline_condition_closing_braces_follow_their_bodies() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;

    assert_eq!(
        format_c(
            fixture!(
                "bool ready(",
                "int left,",
                "int right)",
                "{",
                "if (",
                "left < right)",
                "{",
                "return true;",
                "}",
                "return false;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "bool ready(",
            "    int left,",
            "    int right) {",
            "    if (",
            "        left < right) {",
            "        return true;",
            "        }",
            "    return false;",
            "    }",
        )
    );
}

#[test]
fn brace_after_function_pointer_declarator_indents_its_body() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("int (*fp)(void)", "{", "return 0;", "}"), &options),
        fixture!("int (*fp)(void)", "{", "    return 0;", "}")
    );

    assert_eq!(
        format_c(
            fixture!(
                "int (*resolve(int a, void (*b)(int)))(int)",
                "{",
                "return 0;",
                "}"
            ),
            &options
        ),
        fixture!(
            "int (*resolve(int a, void (*b)(int)))(int)",
            "{",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn linux_style_one_line_function_keeps_brace_and_trailing_comment_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_bytes(b"void Foo() { // comment", &options).expect("format bytes"),
        b"void Foo(){ // comment",
    );
}

#[test]
fn one_line_function_opening_brace_keeps_trailing_line_comment_on_brace_line() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--add-braces",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
        "--align-reference=name",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--min-conditional-indent=0",
        "--max-continuation-indent=80",
        "--max-code-length=109",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_bytes(b"void Foo() { // comment", &options).expect("format bytes"),
        b"void Foo()\n{ // comment",
    );
}

#[test]
fn linux_style_breaks_one_line_function_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "extern inline int helper(int value)\t\t{ return value; }\n",
            &options,
        ),
        "extern inline int helper(int value)\n{\n    return value;\n}\n",
    );
}

#[test]
fn linux_style_keeps_else_if_opening_brace_attached_with_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");
    let source = "void helper(int value)\n{\n    if (value)\n        call();\n    else if (other) {\t/* insert */\n        call();\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_style_breaks_extern_inline_one_line_function_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "extern inline int helper(int value)\t\t{ return value; }\n",
            &options,
        ),
        "extern inline int helper(int value)\n{\n    return value;\n}\n"
    );
}

#[test]
fn stroustrup_and_mozilla_styles_attach_namespace_brace() {
    let source = "namespace alpha {\nint value;\n}\n";

    for style in ["stroustrup", "mozilla"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn ratliff_style_indents_operator_overload_inner_closing_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "struct Item {\n    Item& operator = (Item const* other)\n    {\n        if(other) {\n            call();\n        } else {\n            stop();\n        }\n        return *this;\n    }\n};\n",
            &options,
        ),
        "struct Item {\n    Item& operator = (Item const* other) {\n        if(other) {\n            call();\n            }\n        else {\n            stop();\n            }\n        return *this;\n        }\n    };\n"
    );
}

#[test]
fn indented_brace_styles_do_not_indent_namespace_braces() {
    let input = "namespace alpha {\nint value;\n}\n";
    let cases = [
        ("whitesmith", "namespace alpha\n{\nint value;\n}\n"),
        ("ratliff", "namespace alpha {\nint value;\n}\n"),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(format_c(input, &options), expected);
    }
}

#[test]
fn empty_block_at_line_start_uses_structural_indent() {
    assert_eq!(
        format_c(
            "int main()\n{\n    value;\n{}\n}\n",
            &FormatOptions::default(),
        ),
        fixture!("int main()", "{", "    value;", "    {}", "}")
    );
}

#[test]
fn break_style_keeps_eof_function_brace_with_trailing_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=break".to_owned()]).expect("valid options");

    assert_eq!(
        format_bytes(b"void Foo() { // comment", &options).expect("format bytes"),
        b"void Foo(){    // comment",
    );
}

#[test]
fn run_in_style_does_not_merge_brace_with_preprocessor_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");
    let source = fixture!(
        "",
        "void foo()",
        "{",
        "#ifdef FLAG",
        "    bar1();",
        "#else",
        "    bar2();",
        "#endif",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn default_style_preserves_source_run_in_braces_in_malformed_block() {
    let source = b"\nvoid Foo()\n{   do!\n    {   bar();\n    }[]\n}";

    assert_eq!(
        format_bytes(source, &FormatOptions::default()).expect("format bytes"),
        source,
    );
}

#[test]
fn default_style_keeps_already_attached_else() {
    let source = "void f()\n{\n    if (x) {\n        y();\n    } else {\n        v();\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn kr_breaks_file_scope_control_header_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("if (x) {\nfoo();\n}\n", &options),
        "if (x)\n{\n    foo();\n}\n",
    );
}

#[test]
fn kr_breaks_file_scope_switch_header_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "switch(x) {\ncase 1: foo(); break;\ndefault: bar();\n}\n",
            &options,
        ),
        "switch(x)\n{\ncase 1:\n    foo();\n    break;\ndefault:\n    bar();\n}\n",
    );
}

#[test]
fn whitesmith_indents_mid_case_body_block_brace_below_body_level() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void f(int x)\n{\nswitch(x)\n{\ncase 1:\nfoo();\n{\nbar();\n}\nbreak;\n}\n}\n",
            &options,
        ),
        "void f(int x)\n    {\n    switch(x)\n        {\n        case 1:\n            foo();\n                {\n                bar();\n                }\n            break;\n        }\n    }\n",
    );
}

#[test]
fn horstmann_does_not_run_in_one_line_namespace_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("namespace N { int z; }\n", &options),
        "namespace N\n{\nint z;\n}\n",
    );
}

#[test]
fn pico_does_not_run_in_nested_namespace_opening() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("namespace a { namespace b {\nint z;\n} }\n", &options,),
        "namespace a\n{\nnamespace b\n{\nint z; } }\n",
    );
}

#[test]
fn allman_breaks_one_line_member_inside_extern_linkage_block() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "extern \"C++\" {\nclass F\n{\npublic:\n    int g() const { return y; }\n};\n}\n",
            &options,
        ),
        "extern \"C++\" {\n    class F\n    {\n    public:\n        int g() const\n        {\n            return y;\n        }\n    };\n}\n",
    );
}

#[test]
fn whitesmith_indents_namespace_braces_with_indent_namespaces() {
    let mut options = FormatOptions::default();
    let args = ["--style=whitesmith", "--indent-namespaces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("namespace ns\n{\nint a;\n}\n", &options),
        "namespace ns\n    {\n    int a;\n    }\n",
    );
}

#[test]
fn ratliff_indents_closing_namespace_brace_with_indent_namespaces() {
    let mut options = FormatOptions::default();
    let args = ["--style=ratliff", "--indent-namespaces"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("namespace ns\n{\nint a;\n}\n", &options),
        "namespace ns {\n    int a;\n    }\n",
    );
}

#[test]
fn whitesmith_keeps_namespace_braces_unindented_without_indent_namespaces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let source = "namespace ns\n{\nint a;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_keeps_adjacent_nested_malformed_open_braces_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let expected = "^<ycontinueNULL+^<=*?->:x{{\n        0)?%&&*\n";

    assert_eq!(
        format_c("^<ycontinueNULL+^<=*?->:x{{0)?%&&*\n", &options),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn unpad_bare_block_after_operator_breaks_idempotently() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let expected = "xdefault%{#endif#if A;\n+=&&continue]returncasecase}                      // line...for42returnif42~zelseconstexpr\n";

    assert_eq!(
        format_c(
            "xdefault%{#endif#if A;+=&&continue]returncasecase}// line...for42returnif42~zelseconstexpr\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn bare_block_inside_namespace_is_not_indented() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c("namespace N {\n{ int a; }\n}\n", &options),
        "namespace N {\n{\n    int a;\n}\n}\n",
    );
    assert_eq!(
        format_c("namespace N {\nnamespace M {\n{ int a; }\n}\n}\n", &options,),
        "namespace N {\nnamespace M {\n{\n    int a;\n}\n}\n}\n",
    );
}

#[test]
fn bare_block_at_file_start_is_kept_verbatim() {
    let options = FormatOptions::default();

    assert_eq!(format_c("{a; b;}\n", &options), "{a; b;}\n");
    assert_eq!(
        format_c("{a; b;}\nint x;\n{c; d;}\n", &options),
        "{a; b;}\nint x;\n{\n    c;\n    d;\n}\n",
    );
}

#[test]
fn gnu_namespace_brace_breaks_from_attached_source() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("namespace{int value;}\n", &options),
        "namespace\n{\nint value;\n}\n",
    );
}

#[test]
fn gnu_breaks_malformed_namespace_brace_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected = "^#if A=-|+= {constexpr/* block */tryreturncatchnamespace)z;\nnamespace\n{\nconstexpr&continueif||\n";

    assert_eq!(
        format_c(
            "^#if A=-|+={constexpr/* block */tryreturncatchnamespace)z;namespace{constexpr&continueif||\n",
            &options,
        ),
        expected,
    );
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn whitesmith_malformed_continue_brace_indents_idempotently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "   #if A#else\ngammanamespaceenum NULL   %=0gamma  <=  if !constexpr<=-\nreturnconstexpr#define X(x) \\  *alpha\n->(\tcontinue{~\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_catch_word_before_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "do?}catchx\tcatch)  -x  {\n{  ~alphaautostructNULL{+#if A\t>=\t1NULL#else\tcase\n>=case\n=]&&\n}!=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_operator_brace_breaks_idempotently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "+::\nwhile   ydefault  !=Config\n=Config\n,if\t{  42\tnamespacezItem\t#else\n,z\nConfigdo<=z\n->;\n(constexpr\n<=\t{switch  42\t%struct? NULL\nfor\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn horstmann_malformed_catch_body_after_run_in_block_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let input = "%\tytry0&&-\n?NULL{Item(%\n==do  1=}::\n/* block */for/docall&&\ncatch\nConfig]\n// line#define X(x) \\\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_return_before_brace_after_catch_operator_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = ")alpha\nconstexprcatch<=elsecatch>=>=\nhelperItem\nclass\n{\ncallelse?42default\t||break[\ny\n||class{-\nalpha]try-/\nConfiggamma\nelse\nhelper<=]  default;>=\tcontinue~gamma for{#else::\t#endif==whilebreak=\nconstexpr\n0alphanamespace :: {::+\n||=1 catch&&\tstruct\n42namespace\treturn\n{\n]NULLswitch/* block */\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn horstmann_malformed_close_word_before_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let input = "325]+do#if Aauto*<=autobetadefault-29throw}enumvaluecase{returndefault#define X(x) \\result%#if A\n#if A20==voidbreak-for[alpha?else?whileenumItem*void(2126}int13forautoauto  gammacall/beta->class&&\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_trailing_close_word_aligns_idempotently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let first = format_c("!=x{!=y\n\n)z.}a\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_return_close_word_after_removed_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let first = format_c("alpha{void:return-}result)struct[int&&\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn gnu_headerless_inline_command_brace_keeps_trailing_body_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let first = format_bytes(b"g{:{0\nc", &options).expect("format bytes");

    assert_eq!(format_bytes(&first, &options).expect("format bytes"), first,);
}

#[test]
fn namespace_body_not_indented_after_broken_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("namespace N{\nint a;\n}\n", &options),
        "namespace N\n{\nint a;\n}\n",
    );
}

#[test]
fn backslash_continuation_keeps_brace_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("call(x) \\{ value; }\n", &options),
        "call(x) \\ { value; }\n",
    );
}

#[test]
fn backslash_continuation_keeps_brace_inline_without_close() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(format_c("alpha \\{beta\n", &options), "alpha \\ {beta\n",);
}
