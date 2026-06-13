#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, IndentStyle, apply_command_line_args};

#[test]
fn horstmann_normalizes_run_in_body_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(fixture!("void run()", "{     if (ready())"), &options,),
        fixture!("void run()", "{   if (ready())")
    );
    assert_eq!(
        format_c(fixture!("void run()", "{ if (ready())"), &options),
        fixture!("void run()", "{   if (ready())")
    );
}

#[test]
fn attach_and_one_true_brace_styles_place_definition_and_closing_braces() {
    let mut attach = FormatOptions::default();
    attach.brace_style = BraceStyle::Attach;
    let attach_actual = format_with(fixture!("int f(){if(x){y();}else{z();}}"), &attach);
    assert_eq!(
        attach_actual,
        fixture!(
            "int f() {",
            "    if (x) {",
            "        y();",
            "    } else {",
            "        z();",
            "    }",
            "}",
        )
    );

    let mut linux = FormatOptions::default();
    linux.brace_style = BraceStyle::OneTrueBrace;
    let linux_actual = format_with(fixture!("int f(){if(x){y();}else{z();}}"), &linux);
    assert_eq!(
        linux_actual,
        fixture!(
            "int f()",
            "{",
            "    if (x) {",
            "        y();",
            "    } else {",
            "        z();",
            "    }",
            "}",
        )
    );

    let mut stroustrup = linux.clone();
    stroustrup.break_closing_braces = true;
    let stroustrup_actual = format_with(fixture!("int f(){if(x){y();}else{z();}}"), &stroustrup);
    assert_eq!(
        stroustrup_actual,
        fixture!(
            "int f()",
            "{",
            "    if (x) {",
            "        y();",
            "    }",
            "    else {",
            "        z();",
            "    }",
            "}",
        )
    );
}

#[test]
fn webkit_style_indents_class_members_and_breaks_definition_braces() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::WebKit;
    let actual = format_with(
        fixture!(
            "namespace N{class C{public:void m(){if(x){y();}}};}",
            "int f(){return 0;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace N {",
            "class C {",
            "public:",
            "    void m()",
            "    {",
            "        if (x) {",
            "            y();",
            "        }",
            "    }",
            "};",
            "}",
            "int f()",
            "{",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn brace_style_variants_place_function_and_control_braces() {
    let source = fixture!("int f(){if(x){call();}}");

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_switches = true;
    assert_eq!(
        format_c(source, &whitesmith),
        fixture!(
            "int f()",
            "    {",
            "    if(x)",
            "        {",
            "        call();",
            "        }",
            "    }",
        )
    );

    let mut vtk = FormatOptions::default();
    vtk.brace_style = BraceStyle::Vtk;
    assert_eq!(
        format_c(source, &vtk),
        fixture!(
            "int f()",
            "{",
            "    if(x)",
            "        {",
            "        call();",
            "        }",
            "}",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    assert_eq!(
        format_c(source, &ratliff),
        fixture!(
            "int f() {",
            "    if(x) {",
            "        call();",
            "        }",
            "    }",
        )
    );

    let mut gnu = FormatOptions::default();
    gnu.brace_style = BraceStyle::Gnu;
    gnu.indent_blocks = true;
    assert_eq!(
        format_c(source, &gnu),
        fixture!(
            "int f()",
            "{",
            "    if(x)",
            "        {",
            "            call();",
            "        }",
            "}",
        )
    );

    let mut google = FormatOptions::default();
    google.brace_style = BraceStyle::Attach;
    assert_eq!(
        format_c(source, &google),
        fixture!("int f() {", "    if(x) {", "        call();", "    }", "}",)
    );

    let mut pico = FormatOptions::default();
    pico.brace_style = BraceStyle::Pico;
    pico.break_one_line_blocks = false;
    pico.break_one_line_statements = false;
    assert_eq!(
        format_c(source, &pico),
        fixture!("int f() {if(x) {call();}}")
    );

    let mut lisp = FormatOptions::default();
    lisp.brace_style = BraceStyle::Lisp;
    lisp.break_one_line_statements = false;
    assert_eq!(
        format_c(source, &lisp),
        fixture!("int f() {", "    if(x) {", "        call(); } }",)
    );

    let mut horstmann = FormatOptions::default();
    horstmann.brace_style = BraceStyle::Horstmann;
    horstmann.indent_switches = true;
    assert_eq!(
        format_c(source, &horstmann),
        fixture!("int f()", "{   if(x)", "    {   call();", "    }", "}",)
    );
}

#[test]
fn whitesmith_else_block_uses_whitesmith_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid Whitesmith style");

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "if (ready) {",
                "work();",
                "} else {",
                "stop();",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    if (ready)",
            "        {",
            "        work();",
            "        }",
            "    else",
            "        {",
            "        stop();",
            "        }",
            "    }",
        ),
    );
}

#[test]
fn vtk_else_block_uses_vtk_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid VTK style");

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "if (ready) {",
                "work();",
                "} else {",
                "stop();",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if (ready)",
            "        {",
            "        work();",
            "        }",
            "    else",
            "        {",
            "        stop();",
            "        }",
            "}",
        ),
    );
}

#[test]
fn whitesmith_add_braces_indents_nested_inline_header_blocks() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    if(alpha) if(beta)",
            "            {",
            "            one();",
            "            }",
            "        else",
            "            {",
            "            two();",
            "            }",
            "    }",
        )
    );
}

#[test]
fn vtk_add_braces_indents_nested_inline_header_blocks() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha) if(beta)",
            "            {",
            "            one();",
            "            }",
            "        else",
            "            {",
            "            two();",
            "            }",
            "}",
        )
    );
}

#[test]
fn ratliff_add_braces_aligns_nested_inline_header_closers() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;
    options.indent_classes = true;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(alpha) if(beta) {",
            "            one();",
            "            }",
            "        else {",
            "            two();",
            "            }",
            "    }",
        )
    );
}

#[test]
fn run_in_styles_keep_nested_inline_headers_at_body_indent() {
    let source = "void run(){\nif(alpha) if(beta) one(); else two();\n}\n";

    let mut pico = FormatOptions::default();
    pico.brace_style = BraceStyle::Pico;
    pico.break_one_line_blocks = false;
    pico.break_one_line_statements = false;
    pico.indent_switches = true;
    let pico_expected = fixture!("void run()", "{   if(alpha) if(beta) one(); else two(); }",);

    let mut lisp = FormatOptions::default();
    lisp.brace_style = BraceStyle::Lisp;
    lisp.break_one_line_statements = false;
    let lisp_expected = fixture!(
        "void run() {",
        "    if(alpha) if(beta) one(); else two(); }",
    );

    // No-op one-line options cannot shift a complete nested-header row.
    for configure in [
        |options: &mut FormatOptions| options.remove_braces = true,
        |options: &mut FormatOptions| options.break_one_line_headers = true,
        |options: &mut FormatOptions| options.break_one_line_blocks = false,
    ] {
        let mut options = pico.clone();
        configure(&mut options);
        assert_eq!(format_c(source, &options), pico_expected);

        let mut options = lisp.clone();
        configure(&mut options);
        assert_eq!(format_c(source, &options), lisp_expected);
    }
}

#[test]
fn gnu_add_braces_indents_nested_inline_else_block() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if(alpha) if(beta)",
            "            {",
            "                one();",
            "            }",
            "        else",
            "            {",
            "                two();",
            "            }",
            "}",
        )
    );
}

#[test]
fn whitesmith_keep_one_line_statements_keeps_case_action_with_label() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nswitch(value){case 1: one(); break;}\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    switch(value)",
            "        {",
            "        case 1: one();",
            "            break;",
            "        }",
            "    }",
        )
    );
}

#[test]
fn indent_braces_places_added_one_line_braces_at_control_body_indent() {
    let source = "void run(){\nif(ready)\nwork();\n}\n";

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_classes = true;
    whitesmith.indent_switches = true;
    whitesmith.add_braces = true;
    whitesmith.add_one_line_braces = true;
    assert_eq!(
        format_c(source, &whitesmith),
        fixture!(
            "void run()",
            "    {",
            "    if(ready)",
            "        { work(); }",
            "    }",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    ratliff.indent_classes = true;
    ratliff.add_braces = true;
    ratliff.add_one_line_braces = true;
    assert_eq!(
        format_c(source, &ratliff),
        fixture!(
            "void run() {",
            "    if(ready)",
            "        { work(); }",
            "    }",
        )
    );
}

#[test]
fn add_braces_keeps_following_same_line_header_outside_added_block() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    options.add_braces = true;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nif(value)one();else two();for(int i=0;i<value;i++)step();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "    {",
            "    if(value)",
            "        {",
            "        one();",
            "        }",
            "    else",
            "        {",
            "        two();",
            "        } for(int i=0; i<value; i++)",
            "        {",
            "        step();",
            "        }",
            "    }",
        )
    );
}

#[test]
fn gnu_add_one_line_braces_uses_control_block_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("void run(){\nif(ready)\nwork();\n}\n", &options),
        fixture!(
            "void run()",
            "{",
            "    if(ready)",
            "        { work(); }",
            "}",
        )
    );
}

#[test]
fn pico_add_braces_aligns_dangling_else_with_inner_header() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{   if(alpha) if(beta) { one(); }",
            "        else { two(); } }",
        )
    );
}

#[test]
fn pico_add_braces_keeps_same_line_control_blocks_together() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.indent_switches = true;
    options.add_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nfor(int i=0;i<2;i++) one(); while(ready) two(); do three(); while(done);\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{   for(int i=0; i<2; i++) { one(); } while(ready) { two(); } do { three(); }",
            "    while(done); }",
        )
    );
}

#[test]
fn pico_add_one_line_braces_starts_split_body_at_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("void run(){\nif(ready)\nwork();\n}\n", &options),
        fixture!("void run()", "{   if(ready)", "    {   work(); } }")
    );
}

#[test]
fn pico_remove_braces_preserves_opening_brace_gaps() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Pico;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;
    options.remove_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) { work(); } else { stop(); }\n}\n",
            &options,
        ),
        fixture!("void run()", "{   if(ready)  work();   else  stop();  }")
    );
}

#[test]
fn lisp_remove_braces_preserves_closing_brace_gap() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_statements = false;
    options.remove_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) { work(); } else { stop(); }\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(ready)",
            "        work();   else",
            "        stop();  }",
        )
    );
}

#[test]
fn lisp_kept_one_line_block_breaks_following_else() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_blocks = false;
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run(){\nif(ready) { work(); } else { stop(); }\n}\n",
            &options,
        ),
        fixture!(
            "void run() {",
            "    if(ready) { work(); }",
            "    else { stop(); } }",
        )
    );
}

#[test]
fn lisp_add_one_line_braces_uses_lisp_block_layout() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Lisp;
    options.break_one_line_statements = false;
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c("void run(){\nif(ready) work();\n}\n", &options),
        fixture!("void run() {", "    if(ready) {", "        work(); } }")
    );
}

#[test]
fn horstmann_add_one_line_braces_keeps_function_closer_at_definition_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.indent_switches = true;
    options.add_braces = true;
    options.add_one_line_braces = true;

    assert_eq!(
        format_c(
            "void run(){\nif(alpha) if(beta) one(); else two();\n}\n",
            &options,
        ),
        fixture!(
            "void run()",
            "{   if(alpha) if(beta) { one(); }",
            "        else { two(); }",
            "}",
        )
    );
}

#[test]
fn horstmann_tabs_align_run_in_access_label_to_tab_stop() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Horstmann;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    options.indent_classes = true;

    assert_eq!(
        format_c("class Item{\npublic:\nint value;\n};\n", &options),
        fixture!("class Item", "{\tpublic:", "\t\tint value;", "};")
    );
}

#[test]
fn horstmann_force_tabs_uses_configured_indent_and_tab_widths() {
    let source = fixture!(
        "void run(void)",
        "{",
        "    if (alpha)",
        "    {",
        "        work();",
        "    }",
        "}",
    );

    let mut narrow = FormatOptions::default();
    narrow.brace_style = BraceStyle::Horstmann;
    narrow.indent_style = IndentStyle::ForceTabs;
    narrow.indent_width = 4;
    narrow.tab_width = 8;
    assert_eq!(
        format_c(source, &narrow),
        fixture!(
            "void run(void)",
            "{   if (alpha)",
            "    {   work();",
            "    }",
            "}",
        )
    );

    let mut wide = FormatOptions::default();
    wide.brace_style = BraceStyle::Horstmann;
    wide.indent_style = IndentStyle::ForceTabs;
    wide.indent_width = 8;
    wide.tab_width = 4;
    assert_eq!(
        format_c(source, &wide),
        fixture!(
            "void run(void)",
            "{\t\tif (alpha)",
            "\t\t{\t\twork();",
            "\t\t}",
            "}",
        )
    );
}

#[test]
fn pico_and_lisp_pad_aggregate_closing_braces() {
    let mut pico = FormatOptions::default();
    pico.brace_style = BraceStyle::Pico;
    pico.break_one_line_blocks = false;
    pico.break_one_line_statements = false;

    assert_eq!(
        format_c(fixture!("int arr[]={1,2,3};"), &pico),
        fixture!("int arr[]= {1,2,3 };")
    );
    assert_eq!(
        format_c(fixture!("int m[2][2]={{1,2},{3,4}};"), &pico),
        fixture!("int m[2][2]= {{1,2 },{3,4 } };")
    );
    assert_eq!(
        format_c(fixture!("int a[]={};"), &pico),
        fixture!("int a[]= { };")
    );
    assert_eq!(
        format_c(fixture!("enum E{A,B};"), &pico),
        fixture!("enum E {A,B };")
    );
    assert_eq!(
        format_c(fixture!("void f(void){int a[]={1,2};}"), &pico),
        fixture!("void f(void) {int a[]= {1,2 };}")
    );

    let mut lisp = FormatOptions::default();
    lisp.brace_style = BraceStyle::Lisp;
    lisp.break_one_line_statements = false;

    assert_eq!(
        format_c(fixture!("int arr[]={1,2,3};"), &lisp),
        fixture!("int arr[]= {1,2,3 };")
    );
    assert_eq!(
        format_c(fixture!("int m[2][2]={{1,2},{3,4}};"), &lisp),
        fixture!("int m[2][2]= {{1,2 },{3,4 } };")
    );
}

#[test]
fn allman_attach_horstmann_and_ratliff_leave_aggregate_closing_braces_unpadded() {
    for style in [
        BraceStyle::Allman,
        BraceStyle::Attach,
        BraceStyle::Horstmann,
        BraceStyle::Ratliff,
    ] {
        let mut options = FormatOptions::default();
        options.brace_style = style;
        assert_eq!(
            format_c(fixture!("int a[]={1,2};"), &options),
            fixture!("int a[]= {1,2};"),
            "style {style:?} must not pad the array closing brace"
        );
    }
}
