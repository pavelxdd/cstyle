#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, IndentStyle, apply_command_line_args};

#[test]
fn default_normalizes_mixed_tab_block_comment_indent_in_nested_blocks() {
    let source = "void run()\n{\n\t/*\n\t * outer\n     */\n    if (ready)\n    {\n\t\t/*\n \t     * inner\n  \t     */\n";
    let expected = "void run()\n{\n    /*\n     * outer\n     */\n    if (ready)\n    {\n        /*\n         * inner\n         */\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

#[test]
fn allman_keeps_multiline_comments_below_broken_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "    /*",
                "     * function body",
                "     */",
                "    if (ready) {",
                "        /*",
                "         * control body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    /*",
            "     * function body",
            "     */",
            "    if (ready)",
            "    {",
            "        /*",
            "         * control body",
            "         */",
        )
    );
}

#[test]
fn allman_moves_run_in_multiline_comments_below_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{   /*",
                "     * function body",
                "     */",
                "    if (ready)",
                "    {   /*",
                "         * control body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    /*",
            "     * function body",
            "     */",
            "    if (ready)",
            "    {",
            "        /*",
            "         * control body",
            "         */",
        )
    );
}

#[test]
fn allman_moves_inline_brace_comments_to_body_lines() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(bool ready) /* header */",
                "{   /* function body */",
                "    if(ready)",
                "    {   /* control body */",
            ),
            &options,
        ),
        fixture!(
            "void run(bool ready) /* header */",
            "{",
            "    /* function body */",
            "    if(ready)",
            "    {",
            "        /* control body */",
        )
    );
}

#[test]
fn java_moves_run_in_multiline_comments_below_attached_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{   /*",
                "     * function body",
                "     */",
                "    if (ready)",
                "    {   /*",
                "         * control body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "    /*",
            "     * function body",
            "     */",
            "    if (ready) {",
            "        /*",
            "         * control body",
            "         */",
        )
    );
}

#[test]
fn java_keeps_control_brace_split_after_trailing_header_comments() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    if (ready) /* first */  // second",
                "    {",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "    if (ready) /* first */  // second",
            "    {",
        )
    );
}

#[test]
fn kr_moves_run_in_multiline_comments_below_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{   /*",
                "     * function body",
                "     */",
                "    if (ready)",
                "    {   /*",
                "         * control body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    /*",
            "     * function body",
            "     */",
            "    if (ready) {",
            "        /*",
            "         * control body",
            "         */",
        )
    );
}

#[test]
fn kr_moves_inline_brace_comments_to_body_lines() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(bool ready) /* header */",
                "{   /* function body */",
                "    if(ready)",
                "    {   /* control body */",
            ),
            &options,
        ),
        fixture!(
            "void run(bool ready) /* header */",
            "{",
            "    /* function body */",
            "    if(ready) {",
            "        /* control body */",
        )
    );
}

#[test]
fn java_moves_multiline_comment_below_control_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "    if (ready())   /* condition */ // trailing",
                "    {   /*",
                "         * body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "    if (ready())   /* condition */ // trailing",
            "    {",
            "        /*",
            "         * body",
            "         */",
        )
    );
}

#[test]
fn block_comment_between_control_header_and_brace_uses_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let actual = format_c(
        fixture!(
            "void run(){",
            "if(alpha)",
            "/* condition */",
            "{",
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
            "    if(alpha)",
            "        /* condition */",
            "    {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn whitesmith_comment_after_access_label_uses_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let actual = format_c(
        fixture!("class Item{", "public:", "/* note */", "int value;", "};",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item",
            "    {",
            "    public:",
            "        /* note */",
            "        int value;",
            "    };",
        )
    );
}

#[test]
fn whitesmith_moves_run_in_block_comment_to_block_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "bool ready()",
                "{   while (active)",
                "    {   /* first line",
                "         * second line",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "bool ready()",
            "    {",
            "    while (active)",
            "        {",
            "        /* first line",
            "         * second line",
            "         */",
        ),
    );
}

#[test]
fn vtk_moves_run_in_block_comment_to_block_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid VTK style");

    assert_eq!(
        format_c(
            fixture!(
                "bool ready()",
                "{   while (active)",
                "    {   /* first line",
                "         * second line",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "bool ready()",
            "{",
            "    while (active)",
            "        {",
            "        /* first line",
            "         * second line",
            "         */",
        ),
    );
}

#[test]
fn gnu_function_header_block_comment_stays_at_definition_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!("void run()", "/* note */", "{", "call();", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("void run()", "/* note */", "{", "    call();", "}")
    );
}

#[test]
fn gnu_indent_classes_keeps_member_block_comment_with_the_member() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=gnu".to_owned(), "--indent-classes".to_owned()],
    )
    .expect("valid options");
    let actual = format_c(
        fixture!("class Item{", "public:", "/* note */", "int value;", "};",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item",
            "{",
            "    public:",
            "        /* note */",
            "        int value;",
            "};",
        )
    );
}

#[test]
fn gnu_indent_classes_block_comment_after_line_comment_keeps_member_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=gnu".to_owned(), "--indent-classes".to_owned()],
    )
    .expect("valid options");
    let actual = format_c(
        fixture!(
            "class Item{",
            "public:",
            "// first",
            "/* second */",
            "int value;",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item",
            "{",
            "    public:",
            "// first",
            "        /* second */",
            "        int value;",
            "};",
        )
    );
}

#[test]
fn whitesmith_indent_namespaces_block_comment_uses_namespace_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=whitesmith".to_owned(),
            "--indent-namespaces".to_owned(),
        ],
    )
    .expect("valid options");
    let actual = format_c(
        fixture!("namespace Alpha{", "/* note */", "int value;", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace Alpha",
            "    {",
            "    /* note */",
            "    int value;",
            "    }",
        )
    );
}

#[test]
fn enum_value_line_comment_continuation_uses_value_column() {
    let actual = format_c(
        fixture!(
            "enum",
            "{",
            "    VALUE = 1  // first line",
            "               // second line",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "enum",
            "{",
            "    VALUE = 1  // first line",
            "            // second line",
            "};",
        )
    );
}

#[test]
fn split_one_line_statement_preserves_block_comment_gap() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    i = NUM_OF_PLATFORMS/*hack*/; break;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    i = NUM_OF_PLATFORMS/*hack*/;",
            "    break;",
            "}",
        )
    );
}

#[test]
fn line_comments_after_braceless_if_indent_as_body() {
    let source = fixture!(
        "void f()",
        "{",
        "    if ( a )",
        "        if ( b )",
        "            // one",
        "            // two",
        "            call();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn block_comment_with_case_labels_aligns_to_switch_case_level() {
    let source = fixture!(
        "void f(int x)",
        "{",
        "    switch ( x )",
        "    {",
        "    case 1:",
        "        break;",
        "    /*  case 2:",
        "        break;",
        "    */",
        "    default:",
        "        break;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn indent_classes_keeps_run_in_class_block_comment_body_indent() {
    let mut options = FormatOptions::default();
    options.indent_classes = true;
    let source = fixture!(
        "class Item",
        "{   /*enum Flags {",
        "           NoFlag = 0,",
        "           HasValue = 1",
        "         };*/",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn horstmann_moves_column_one_comment_to_nested_run_in_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(bool ready)",
                "{",
                "    if (ready)",
                "    {",
                "        // first",
                "/* column one */",
            ),
            &options,
        ),
        fixture!(
            "void run(bool ready)",
            "{   if (ready)",
            "    {   // first",
            "        /* column one */",
        )
    );
}

#[test]
fn horstmann_runs_leading_line_comments_into_nested_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(bool ready)",
                "{",
                "    // function body",
                "    if (ready)",
                "    {",
                "        // conditional body",
                "        while (active)",
                "        {",
                "            // loop body",
            ),
            &options,
        ),
        fixture!(
            "void run(bool ready)",
            "{   // function body",
            "    if (ready)",
            "    {   // conditional body",
            "        while (active)",
            "        {   // loop body",
        )
    );
}

#[test]
fn horstmann_runs_leading_multiline_comments_into_nested_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    /*",
                "     * function body",
                "     */",
                "    if (ready)",
                "    {",
                "        /*",
                "         * control body",
                "         */",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{   /*",
            "     * function body",
            "     */",
            "    if (ready)",
            "    {   /*",
            "         * control body",
            "         */",
        )
    );
}

#[test]
fn horstmann_runs_leading_inline_block_comments_into_nested_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(bool ready) /* header */",
                "{",
                "    /* function body */",
                "    if(ready)",
                "    {",
                "        /* control body */",
            ),
            &options,
        ),
        fixture!(
            "void run(bool ready) /* header */",
            "{   /* function body */",
            "    if(ready)",
            "    {   /* control body */",
        )
    );
}

#[test]
fn horstmann_runs_first_function_line_comment_into_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    // first",
                "    // second",
                "    if (ready())",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{   // first",
            "    // second",
            "    if (ready())",
        )
    );
}

#[test]
fn horstmann_runs_first_class_line_comment_in_and_keeps_access_label_flush() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()])
        .expect("valid run-in style");

    assert_eq!(
        format_c(
            fixture!("class Item", "{", "    // note", "    public:"),
            &options,
        ),
        fixture!("class Item", "{   // note", "public:"),
    );
}

#[test]
fn horstmann_runs_first_class_block_comment_in_and_keeps_access_label_flush() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()])
        .expect("valid run-in style");

    assert_eq!(
        format_c(
            fixture!("class Item", "{", "/* note */", "public:"),
            &options,
        ),
        fixture!("class Item", "{   /* note */", "public:"),
    );
}

#[test]
fn horstmann_runs_switch_line_comment_into_switch_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid Horstmann style");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    switch (value)",
                "    {",
                "        // note",
                "        case 1:",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{   switch (value)",
            "    {   // note",
            "        case 1:",
        ),
    );
}

#[test]
fn horstmann_runs_switch_block_comment_into_switch_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid Horstmann style");

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    switch (value)",
                "    {",
                "        /* note */",
                "        case 1:",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{   switch (value)",
            "    {   /* note */",
            "        case 1:",
        ),
    );
}

fn non_whitespace_without_comment_prefixes(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with('*') && !trimmed.starts_with("*/") {
                &trimmed[1..]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

#[test]
fn keeps_block_comment_prefix_indent_and_single_trailing_gap() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "    /* first",
            "     * second",
            "     */",
            "    x=1; /* trailing */",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    /* first",
            "     * second",
            "     */",
            "    x = 1; /* trailing */",
            "}",
        )
    );
}
#[test]
fn keeps_trailing_block_comment_attached_to_opening_brace() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "void f(void) {",
            "if (x) {",
            "a();",
            "} else { /* note */",
            "b();",
            "}",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    if (x) {",
            "        a();",
            "    } else { /* note */",
            "        b();",
            "    }",
            "}",
        )
    );
}

#[test]
fn preserves_multiple_trailing_comments_on_attached_brace_line() {
    let source = fixture!(
        "void f()",
        "{",
        "    if (x) { /* comment1 */  // comment2",
        "        call();",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn kr_attached_brace_keeps_space_before_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    if (a){ // TAG",
                "        inner();",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    if (a) { // TAG",
            "        inner();",
            "    }",
            "}",
        )
    );
}

#[test]
fn attached_block_comments_preserve_body_indent_when_moved_below_brace() {
    assert_eq!(
        format_c(
            fixture!(
                "void foo1(bool isFoo) { /* comment1 */",
                "    if(isFoo) { /* comment2 */",
                "        fooBar();",
                "    }",
                "}",
                "",
                "void foo2(bool isFoo) { /* comment3",
                "     *",
                "     */",
                "    if(isFoo) { /* comment4",
                "                 *",
                "                 */",
                "        fooBar();",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void foo1(bool isFoo) { /* comment1 */",
            "    if(isFoo) { /* comment2 */",
            "        fooBar();",
            "    }",
            "}",
            "",
            "void foo2(bool isFoo) {",
            "    /* comment3",
            "         *",
            "         */",
            "    if(isFoo) {",
            "        /* comment4",
            "                     *",
            "                     */",
            "        fooBar();",
            "    }",
            "}",
        )
    );
}

#[test]
fn allman_inline_block_comment_continuation_uses_opener_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("int f(){/* one", " * two", " */return 0;}"),
            &options,
        ),
        fixture!(
            "int f()",
            "{",
            "    /* one",
            "     * two",
            "     */return 0;",
            "}",
        )
    );
}

#[test]
fn run_in_block_comment_preserves_body_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{ if (x)",
                "  { /* comment1 */",
                "    call();",
                "  }",
                "  else",
                "  { /* comment2",
                "     *",
                "     */",
                "    call();",
                "  }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{   if (x)",
            "    {   /* comment1 */",
            "        call();",
            "    }",
            "    else",
            "    {   /* comment2",
            "         *",
            "         */",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn line_comment_text_inside_block_comment_uses_block_comment_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "\t/*",
                "// line-comment text",
                "",
                "\t*/",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run()",
            "{",
            "    /*",
            "    // line-comment text",
            "",
            "    */",
            "}",
        ),
    );
}

#[test]
fn closing_brace_after_block_comment_closer_moves_to_own_line() {
    assert_eq!(
        format_c(
            "void run()\n{\n    if (first) {\n/*      first line\n        second line\n*/  }\n\n    if (second) {\n/*      third line\n*/}\n}\n",
            &FormatOptions::default(),
        ),
        "void run()\n{\n    if (first) {\n        /*      first line\n                second line\n        */\n    }\n\n    if (second) {\n        /*      third line\n        */\n    }\n}\n",
    );
}

#[test]
fn keeps_trailing_comments_on_case_label_lines() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_switches = true;
    options.break_one_line_statements = false;
    let actual = format_with(
        fixture!(
            "void f(int x) {",
            "switch (x) {",
            "case 1: { /* one */",
            "break;",
            "}",
            "case 2: // two",
            "break;",
            "default:  alpha = 0; break;",
            "}",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch (x) {",
            "        case 1: { /* one */",
            "            break;",
            "        }",
            "        case 2: // two",
            "            break;",
            "        default:  alpha = 0; break;",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_case_comment_and_quote_state_across_lines() {
    let source = fixture!(
        "int f(int x){switch(x){case 1:{",
        "// first text",
        "/* multi",
        " * body",
        " */",
        "char *s = \"}\";",
        "return 1;",
        "}}}",
    );
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "    {",
            "// first text",
            "        /* multi",
            "         * body",
            "         */",
            "        char *s = \"}\";",
            "        return 1;",
            "    }",
            "    }",
            "}",
        )
    );
}
#[test]
fn reindents_block_comments_immediately_before_case_headers() {
    let source = fixture!(
        "int f(int x){switch(x){",
        "/* before */",
        "case 1:return 1;",
        "/* multi",
        " * before default",
        " */",
        "default:return 0;}}",
    );
    let actual = format(source);

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    /* before */",
            "    case 1:",
            "        return 1;",
            "    /* multi",
            "     * before default",
            "     */",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}

#[test]
fn block_comments_before_case_headers_use_case_label_indent() {
    let mut linux = FormatOptions::default();
    apply_command_line_args(
        &mut linux,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "int f(int value)",
                "{",
                "\tswitch (value) {",
                "\t/* first group */",
                "\t/* second line */",
                "\tcase A:",
                "\t\treturn 1;",
                "",
                "\t/* next group */",
                "\t/* next second */",
                "\tcase B:",
                "\t\treturn 2;",
                "\t}",
                "}",
            ),
            &linux,
        ),
        fixture!(
            "int f(int value)",
            "{",
            "    switch (value) {",
            "    /* first group */",
            "    /* second line */",
            "    case A:",
            "        return 1;",
            "",
            "    /* next group */",
            "    /* next second */",
            "    case B:",
            "        return 2;",
            "    }",
            "}",
        )
    );

    assert_eq!(
        format_c(
            fixture!(
                "void f(int value)",
                "{",
                "\tswitch (value) {",
                "\tcase A:",
                "\t\tbreak;",
                "",
                "\t/* note",
                "\t   more */",
                "\tcase B:",
                "\t\tbreak;",
                "\t}",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f(int value)",
            "{",
            "    switch (value) {",
            "    case A:",
            "        break;",
            "",
            "    /* note",
            "       more */",
            "    case B:",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn preserves_comments_strings_chars_and_preprocessor_lines() {
    let source = fixture!(
        "#define TEXT \"a{b}\"",
        "int f(){char c='}';// keep }",
        "return TEXT[0];}",
    );
    let actual = format(source);
    assert_eq!(
        actual,
        fixture!(
            "#define TEXT \"a{b}\"",
            "int f()",
            "{",
            "    char c = '}'; // keep }",
            "    return TEXT[0];",
            "}",
        )
    );
}
#[test]
fn keeps_trailing_comments_after_formatted_code() {
    let actual = format(fixture!(
        "int f(){int x=1;// keep",
        "x+=1; /* block */",
        "return x;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    int x = 1; // keep",
            "    x += 1; /* block */",
            "    return x;",
            "}",
        )
    );
}

#[test]
fn empty_constructor_block_keeps_trailing_comment_on_line() {
    let source = fixture!(
        "class Item",
        "{",
        "public:",
        "    Item() : value(value) {} // comment",
        "};",
        "",
        "Item() : value(value) {} // comment",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn empty_do_while_block_keeps_trailing_line_comment_attached() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tdo { } while (0); /* note */\n}\n",
            &options,
        ),
        "void f(void)\n{\n    do { } while (0); /* note */\n}\n",
    );
}

#[test]
fn expanding_one_line_block_moves_trailing_comment_with_three_space_gap() {
    assert_eq!(
        format_c(
            fixture!("int f(){return 0;}// tail"),
            &FormatOptions::default(),
        ),
        fixture!("int f() {", "    return 0;   // tail", "}")
    );
}

#[test]
fn breaks_one_line_block_and_moves_trailing_comment_to_statement() {
    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo()",
                "{",
                "    if (isFoo)",
                "    { bar(); } // comment",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "",
            "void foo()",
            "{",
            "    if (isFoo)",
            "    {",
            "        bar();    // comment",
            "    }",
            "}",
        )
    );
}

#[test]
fn java_style_brace_padding_reduces_spaces_before_trailing_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo() {",
                "    if (isBar1){\t// comment",
                "        bar1();",
                "    }",
                "    if (isBar2){   // comment",
                "        bar2();",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo() {",
            "    if (isBar1) {\t// comment",
            "        bar1();",
            "    }",
            "    if (isBar2) {  // comment",
            "        bar2();",
            "    }",
            "}",
        )
    );
}

#[test]
fn allman_pad_paren_keeps_single_gap_before_moved_brace_comments() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--pad-paren".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo() { // comment0",
                "    if ((isFoo())) { // comment1",
                "        bar(fooBar); // comment2",
                "    }",
                "}",
                "",
                "void foo2(){// comment0",
                "    if ((isFoo())){// comment1",
                "        bar(fooBar);// comment2",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo()   // comment0",
            "{",
            "    if ( ( isFoo() ) ) // comment1",
            "    {",
            "        bar ( fooBar ); // comment2",
            "    }",
            "}",
            "",
            "void foo2() // comment0",
            "{",
            "    if ( ( isFoo() ) ) // comment1",
            "    {",
            "        bar ( fooBar ); // comment2",
            "    }",
            "}",
        )
    );
}

#[test]
fn java_style_attaches_brace_before_header_block_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "void foo2(bool isFoo) /* comment0 */",
                "{   /* comment1",
                "     *",
                "     */",
                "    if(isFoo) /* comment2 */",
                "    {   /* comment3",
                "         *",
                "         */",
                "        bar();",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "void foo2(bool isFoo) { /* comment0 */",
            "    /* comment1",
            "     *",
            "     */",
            "    if(isFoo) { /* comment2 */",
            "        /* comment3",
            "         *",
            "         */",
            "        bar();",
            "    }",
            "}",
        )
    );
}

#[test]
fn run_in_style_does_not_merge_brace_with_column_one_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");
    let source = fixture!(
        "",
        "void foo()",
        "{",
        "// comment1",
        "// comment2",
        "    if (isFoo())",
        "        bar();",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn run_in_style_does_not_merge_brace_with_multiline_block_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "",
                "namespace name",
                "{",
                "    /**",
                "     * comment1",
                "     */",
                "    class Foo",
                "    {",
                "        /**",
                "         * comment2",
                "         */",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "",
            "namespace name",
            "{",
            "/**",
            " * comment1",
            " */",
            "class Foo",
            "{   /**",
            "     * comment2",
            "     */",
            "}",
            "}",
        )
    );
}

#[test]
fn multiline_block_comment_in_else_body_uses_body_indent() {
    assert_eq!(
        format_c(
            "void f() {\n  if (ready()) {\n    call();\n  } else {\n    /* comment\n    ** body\n    */\n    done();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    if (ready()) {\n        call();\n    } else {\n        /* comment\n        ** body\n        */\n        done();\n    }\n}\n",
    );
}

#[test]
fn split_one_line_case_body_preserves_trailing_comment_gap() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case A: b(); break;",
                "        default: break; // avoid compiler warning",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case A:",
            "        b();",
            "        break;",
            "    default:",
            "        break; // avoid compiler warning",
            "    }",
            "}",
        )
    );
}

#[test]
fn trailing_comment_closing_brace_text_does_not_close_block_early() {
    assert_eq!(
        format_c(
            fixture!("int f(){return 0;}// tail }"),
            &FormatOptions::default(),
        ),
        fixture!("int f() {", "    return 0;", "}// tail }")
    );
}

#[test]
fn keeps_trailing_comment_columns_after_padding_changes() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "int f(){",
            "int x=1;      // line",
            "x+=1;          /* block */",
            "if(x)",
            "{",
            "y();",
            "}    // block",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f() {",
            "    int x = 1;    // line",
            "    x += 1;        /* block */",
            "    if(x)",
            "    {",
            "        y();",
            "    }    // block",
            "}",
        )
    );
}
#[test]
fn keeps_initializer_table_trailing_comment_columns() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    let actual = format_c(
        fixture!(
            "static const item table[] = {",
            "    { alpha, beta },             /* first entry */",
            "    { gamma, delta },            /* second entry */",
            "    { NULL, NULL }               /* end */",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static const item table[] = {",
            "    { alpha, beta },             /* first entry */",
            "    { gamma, delta },            /* second entry */",
            "    { NULL, NULL }               /* end */",
            "};",
        )
    );
}

#[test]
fn trailing_comment_tab_gap_after_comma_is_not_doubled() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent=tab".to_owned()]).expect("valid options");
    let source = fixture!(
        "enum E {",
        "\tALPHA,\t/**< first */",
        "\tBETA,\t/**< second */",
        "};",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn top_level_block_comment_body_preserves_leading_tabs() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent=tab".to_owned()]).expect("valid options");
    let source = fixture!("/****", "\t This line.", "\t More text.", "****/", "int g;",);

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn keeps_column_one_comments_unindented_by_default() {
    let actual = format(fixture!(
        "int f(){",
        "// keep column one",
        "    // keep indented",
        "return 0;}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "// keep column one",
            "    // keep indented",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn default_snaps_near_column_one_comments_to_structural_columns() {
    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    // body",
                "// column one",
                " // near column one",
                "   // near body",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run()",
            "{",
            "    // body",
            "// column one",
            "// near column one",
            "    // near body",
            "}",
        ),
    );
}

#[test]
fn gnu_indent_col1_top_level_line_comments_stay_unindented() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=gnu".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(fixture!("// first", " // second", "int value;"), &options),
        fixture!("// first", "// second", "int value;"),
    );
}

#[test]
fn gnu_top_level_block_comment_after_line_comment_stays_unindented() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid GNU style");

    assert_eq!(
        format_c(fixture!("// line", " /* note */", "int value;"), &options),
        fixture!("// line", "/* note */", "int value;"),
    );
}

#[test]
fn indent_col1_comment_in_unindented_namespace_stays_at_column_one() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("namespace sample{", "// note", "int value;", "}"),
            &options,
        ),
        fixture!("namespace sample", "{", "// note", "int value;", "}"),
    );
}

#[test]
fn indent_col1_comment_in_extern_block_uses_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("extern \"C\"{", "// note", "void run(void);", "}"),
            &options,
        ),
        fixture!("extern \"C\" {", "    // note", "    void run(void);", "}",),
    );
}

#[test]
fn gnu_indent_col1_definition_comment_uses_header_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=gnu".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void run(void)", "// note", "{", "call();", "}"),
            &options,
        ),
        fixture!("void run(void)", "// note", "{", "    call();", "}"),
    );
}

#[test]
fn gnu_indent_col1_comment_after_access_label_uses_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=gnu".to_owned(),
            "--indent-classes".to_owned(),
            "--indent-modifiers".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("class Item{", "public:", "// note", "void run();", "};",),
            &options,
        ),
        fixture!(
            "class Item",
            "{",
            "    public:",
            "        // note",
            "        void run();",
            "};",
        ),
    );
}

#[test]
fn indent_col1_comment_after_case_label_uses_case_body_column_idempotently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!(
        "void run(int value){",
        "switch(value){",
        "case 1:",
        " // note",
        "call();",
        "}",
        "}",
    );
    let expected = fixture!(
        "void run(int value)",
        "{",
        "    switch(value)",
        "    {",
        "    case 1:",
        "        // note",
        "        call();",
        "    }",
        "}",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn indent_col1_comment_in_case_block_does_not_deepen_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=java".to_owned(),
            "--indent-cases".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(int value){",
                "switch(value){",
                "case 1:{",
                "// note",
                "call();",
                "}",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run(int value) {",
            "    switch(value) {",
            "    case 1: {",
            "            // note",
            "            call();",
            "        }",
            "    }",
            "}",
        ),
    );
}

#[test]
fn indent_col1_comment_in_nested_preprocessor_block_uses_block_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-preproc-block".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "#if ALPHA",
                "#if BETA",
                "// note",
                "int value;",
                "#endif",
                "#endif",
            ),
            &options,
        ),
        fixture!(
            "#if ALPHA",
            "    #if BETA",
            "        // note",
            "        int value;",
            "    #endif",
            "#endif",
        ),
    );
}

#[test]
fn indent_col1_comment_after_multiline_define_keeps_case_branch_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = "void run(int value){\nswitch(value){\ncase 1:\n#if ENABLED\n#define APPLY(arg) \\\ncall(arg)\n// note\nAPPLY(value);\n#endif\nbreak;\n}\n}\n";
    let expected = "void run(int value)\n{\n    switch(value)\n    {\n    case 1:\n#if ENABLED\n#define APPLY(arg) \\\ncall(arg)\n        // note\n        APPLY(value);\n#endif\n        break;\n    }\n}\n";

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn indent_col1_comment_in_split_else_branch_uses_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(void){\nif(ready){call();}else\n#if ENABLED\ncall();\n#else\n // note\nother();\n#endif\n}\n",
            &options,
        ),
        "void run(void)\n{\n    if(ready)\n    {\n        call();\n    }\n    else\n#if ENABLED\n        call();\n#else\n        // note\n        other();\n#endif\n}\n",
    );
}

#[test]
fn tab_indented_col1_comment_in_split_else_branch_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent=tab=4".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = "void run(void){\nif(ready){call();}else\n#if ENABLED\ncall();\n#else\n // note\nother();\n#endif\n}\n";
    let expected = "void run(void)\n{\n\tif(ready)\n\t{\n\t\tcall();\n\t}\n\telse\n#if ENABLED\n\t\tcall();\n#else\n\t\t// note\n\t\tother();\n#endif\n}\n";

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn indent_col1_comment_in_initializer_does_not_deepen_sibling() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=gnu".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "static Item items[]={",
                "{",
                "1,",
                " // note",
                "2,",
                "},",
                "};",
            ),
            &options,
        ),
        fixture!(
            "static Item items[]= {",
            "    {",
            "        1,",
            "        // note",
            "        2,",
            "    },",
            "};",
        ),
    );
}

#[test]
fn indent_col1_doc_comment_uses_class_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-classes".to_owned(),
            "--indent-modifiers".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!("class Item{", "/// note", "void run();", "};");
    let expected = fixture!(
        "class Item",
        "{",
        "        /// note",
        "        void run();",
        "};",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

// Run-in documentation comments use the same member column as later comments.
#[test]
fn horstmann_col1_doc_run_in_uses_member_column_consistently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent-classes".to_owned(),
            "--indent-modifiers".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!(
        "class Item{",
        "/// zero",
        " //! one",
        "  /// two",
        "void run();",
        "};",
    );
    let expected = fixture!(
        "class Item",
        "{       /// zero",
        "        //! one",
        "        /// two",
        "        void run();",
        "};",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn aggregate_doc_comment_uses_indented_member_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-classes".to_owned(),
            "--indent-modifiers".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("class Item{", "  /// note", "void run();", "};"),
            &options,
        ),
        fixture!(
            "class Item",
            "{",
            "        /// note",
            "        void run();",
            "};",
        ),
    );
}

#[test]
fn indent_col1_comment_in_formatted_preprocessor_block_uses_code_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-preproc-block".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!("#if ENABLED", "// note", "int value;", "#endif");
    let expected = fixture!("#if ENABLED", "    // note", "    int value;", "#endif",);

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn horstmann_run_in_line_comment_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent=spaces=2".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!("static Item items[]={", "{", "// note", "1,", "}", "};",);
    let expected = fixture!(
        "static Item items[]= {",
        "  { // note",
        "    1,",
        "  }",
        "};",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn horstmann_run_in_multiline_comment_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent=tab=4".to_owned(),
            "--indent-col1-comments".to_owned(),
        ],
    )
    .expect("valid options");
    let input = fixture!(
        "void run(void){",
        "/* note",
        " * body",
        " */",
        "call();",
        "}",
    );
    let expected = fixture!(
        "void run(void)",
        "{\t/* note",
        "\t * body",
        "\t */",
        "\tcall();",
        "}",
    );

    let first = format_c(input, &options);
    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), expected);
}

#[test]
fn indents_column_one_comments_when_requested() {
    let mut options = FormatOptions::default();
    options.indent_col1_comments = true;
    let actual = format_with(fixture!("int f(){", "// keep", "return 0;}",), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    // keep", "    return 0;", "}",)
    );
}
#[test]
fn keeps_comments_before_else_and_case_headers() {
    let actual = format(fixture!(
        "int f(int x){if(x){return 1;}",
        "// keep else",
        "else{return 0;}",
        "switch(x){",
        "// keep case",
        "case 1:return 1;}",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    if (x)",
            "    {",
            "        return 1;",
            "    }",
            "// keep else",
            "    else",
            "    {",
            "        return 0;",
            "    }",
            "    switch (x)",
            "    {",
            "// keep case",
            "    case 1:",
            "        return 1;",
            "    }",
            "}",
        )
    );
}
#[test]
fn keeps_column_one_comments_before_case_but_aligns_indented_ones() {
    let actual = format(fixture!(
        "int classify(int value){switch(value){",
        "case 1:return 1;",
        "// column one note",
        "case 2:return 2;",
        "    // indented note",
        "default:return 0;}}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int classify(int value)",
            "{",
            "    switch (value)",
            "    {",
            "    case 1:",
            "        return 1;",
            "// column one note",
            "    case 2:",
            "        return 2;",
            "    // indented note",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_escaped_line_comment_continuations() {
    let actual = format(fixture!(
        "int f(){int x=1; // first \\",
        " second",
        "return x;}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    int x = 1; // first \\",
            "    second",
            "    return x;",
            "}",
        )
    );
}
#[test]
fn preserves_block_comment_continuation_lines() {
    let actual = format(fixture!("int f(){/* one", " * two", " */return 0;}",));
    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    /* one",
            "     * two",
            "     */return 0;",
            "}",
        )
    );
}
#[test]
fn strips_block_comment_prefix_when_requested() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;
    let actual = format_with(
        fixture!("int f(){/* one", " * two", " plain", " */return 0;}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    /*  one",
            "        two",
            "        plain",
            "    */return 0;",
            "}",
        )
    );
}

#[test]
fn strip_comment_prefix_reindents_comments_at_each_scope() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;

    assert_eq!(
        format_c(
            "\nvoid foo(bool ready)\n{\n        /* first\n         *\n         */\n    if(ready) {\n                    /* second\n                     *\n                     */\n",
            &options,
        ),
        "\nvoid foo(bool ready)\n{\n    /*  first\n\n    */\n    if(ready) {\n        /*  second\n\n        */\n",
    );
}

#[test]
fn strip_comment_prefix_removes_banner_stars_and_trailing_spaces() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;

    assert_eq!(
        format_c(
            "\n/* * * * * * * * * * *\n * First banner line.     *\n * Second banner line.    *\n * * * * * * * * * * */\n",
            &options,
        ),
        "\n/* * * * * * * * * * *\n    First banner line.\n    Second banner line.\n * * * * * * * * * * */\n",
    );
}

#[test]
fn strip_comment_prefix_preserves_framed_body_indent_and_flushes_closer() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;

    assert_eq!(
        format_c(
            "\n/* * * * * * * * * *\n *   First owner\n *   Second owner\n * * * * * * * * *\n */\n",
            &options,
        ),
        "\n/* * * * * * * * * *\n     First owner\n     Second owner\n * * * * * * * * *\n*/\n",
    );
}

#[test]
fn strip_comment_prefix_preserves_tabs_in_documentation_comment() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;

    assert_eq!(
        format_c(
            "\n/*! \\brief\tUpdate the current value.\n * \\param\tvalue\n * \\return\tstatus\n */\n",
            &options,
        ),
        "\n/*! \\brief\tUpdate the current value.\n    \\param\tvalue\n    \\return\tstatus\n*/\n",
    );
}

#[test]
fn pads_leading_arithmetic_continuations_and_keeps_block_comment_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "value = (uint64_t)(",
            "       365 * year",
            "       +367 * month",
            "       +day",
            "/*",
            " * offset",
            " */",
            "       - 1);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    value = (uint64_t)(",
            "                365 * year",
            "                + 367 * month",
            "                + day",
            "                /*",
            "                 * offset",
            "                 */",
            "                - 1);",
            "}",
        )
    );
}
#[test]
fn preserves_continuation_after_line_comments_and_stream_operators() {
    let actual = format(fixture!(
        "void f(){",
        "int x = a + // keep",
        "b;",
        "cout << a",
        "<< b;",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    int x = a + // keep",
            "            b;",
            "    cout << a",
            "         << b;",
            "}",
        )
    );
}
#[test]
fn strip_comment_prefix_preserves_comment_text_except_prefix_markers() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;
    let source = fixture!("/*", " * alpha + beta", " * gamma - delta", " */",);
    let actual = format_with(source, &options);

    assert_eq!(
        non_whitespace_without_comment_prefixes(&actual),
        non_whitespace_without_comment_prefixes(source)
    );
}
#[test]
fn block_comment_continuation_shifts_with_opener_column() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("    /* alpha", "       beta */"), &options),
        fixture!("/* alpha", "   beta */")
    );
    assert_eq!(
        format_c(fixture!("        /* alpha", "    beta */"), &options),
        fixture!("/* alpha", "beta */")
    );
}
#[test]
fn block_comment_continuation_keeps_relative_indent_inside_block() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "  /* alpha",
                "      beta",
                "gamma */",
                "}",
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    /* alpha",
            "        beta",
            "    gamma */",
            "}",
        )
    );
}

#[test]
fn unterminated_block_comment_reindent_preserves_columns_with_scope_floor() {
    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n       /*   HEADER VALUES\n    first alpha, beta, gamma,\n             aligned delta, epsilon\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo()\n{\n    /*   HEADER VALUES\n    first alpha, beta, gamma,\n          aligned delta, epsilon\n",
    );
}

#[test]
fn code_after_block_comment_closer_stays_attached() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("int alpha; /* note", " more */ beta;"), &options),
        fixture!("int alpha; /* note", " more */ beta;")
    );
    assert_eq!(
        format_c(fixture!("/* note", " more */beta;"), &options),
        fixture!("/* note", " more */beta;")
    );
    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    /* note",
                "     more */ beta();",
                "}",
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    /* note",
            "     more */ beta();",
            "}",
        )
    );
}
#[test]
fn preserves_source_spacing_after_single_line_block_comment() {
    let options = FormatOptions::default();

    // No option governs the gap after a single-line block comment, so source
    // adjacency is preserved: no space when adjacent, the source gap otherwise.
    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    /*a*/b();",
                "    /*c*/ d();",
                "    e(/*x*/y);",
                "    /*p*//*q*/",
                "}",
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    /*a*/b();",
            "    /*c*/ d();",
            "    e(/*x*/y);",
            "    /*p*//*q*/",
            "}",
        )
    );
}
#[test]
fn strip_comment_prefix_aligns_body_to_one_indent() {
    let mut options = FormatOptions::default();
    options.strip_comment_prefix = true;

    assert_eq!(
        format_c(fixture!("/* alpha", " * beta", " gamma", " */"), &options),
        fixture!("/*  alpha", "    beta", "    gamma", "*/")
    );
}
#[test]
fn preserves_adjacent_trailing_comment_without_formatting_change() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "struct s {",
                "    int signal_pending;/**< note */",
                "    int loop;/* tag */",
                "};",
            ),
            &options
        ),
        fixture!(
            "struct s {",
            "    int signal_pending;/**< note */",
            "    int loop;/* tag */",
            "};",
        )
    );
}

#[test]
fn preserves_break_before_own_line_comment() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("void f() {", "alpha", "// note", "beta;", "}"),
            &options
        ),
        fixture!("void f() {", "    alpha", "// note", "    beta;", "}")
    );
}

#[test]
fn line_comment_list_items_keep_source_column() {
    let source = fixture!(
        "class Item {",
        "    // Intro (with examples):",
        "    // - first item.",
        "    //      detail.",
        "    void f();",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn line_comment_ending_colon_does_not_indent_next_statement() {
    let source = fixture!(
        "void f()",
        "{",
        "    // For compatibility, call",
        "    // helper():",
        "    Type value = source.get();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn fold_marker_line_comment_does_not_indent_next_member() {
    let source = fixture!(
        "class C",
        "{",
        "    // section {{{",
        "",
        "    // Add text.",
        "    void AddText();",
        "",
        "    // Add array.",
        "    void AddStyledText();",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn comment_after_macro_stream_chain_keeps_statement_column() {
    let source = fixture!(
        "#define ADD_ROW(a, b) \\",
        "    call(a) \\",
        "        << b",
        "",
        "// comment",
        "ADD_ROW(x, y);",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn call_chain_after_commented_chain_link_keeps_chain_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    service.addItem(Create(), Data().",
        "                    setName(\"Text\").",
        "                    adapt().location().",
        "                    //span().",
        "                    firstOption(true).optionalAction(true));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn call_argument_after_line_comment_keeps_comment_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    call(this, id, text,",
        "         pos, size,",
        "         0, nullptr,",
        "         // comment",
        "         FLAG);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn call_argument_after_line_comment_keeps_argument_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "bool f()",
                "{",
                "    return call",
                "           (",
                "             a,",
                "             // comment",
                "             b,",
                "             c",
                "           );",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "bool f()",
            "{",
            "    return call",
            "           (",
            "               a,",
            "               // comment",
            "               b,",
            "               c",
            "           );",
            "}",
        )
    );
}

#[test]
fn line_comment_after_trailing_comment_returns_to_statement_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case A:",
                "            {",
                "                if (x)",
                "                    break; // first",
                "                           // second",
                "",
                "                // block",
                "                call();",
                "            }",
                "            break;",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (x)",
            "    {",
            "    case A:",
            "    {",
            "        if (x)",
            "            break; // first",
            "        // second",
            "",
            "        // block",
            "        call();",
            "    }",
            "    break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn keeps_trailing_comment_attached_to_its_statement() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("void f() {", "alpha // note", "beta;", "}"),
            &options
        ),
        fixture!("void f() {", "    alpha // note", "    beta;", "}")
    );
}

#[test]
fn inline_block_comment_assignment_does_not_indent_next_statement() {
    let source = fixture!(
        "void f()",
        "{",
        "    void *actual = ValueType(ID).construct(storage, /*copy=*/0);",
        "    CHECK_EQ(actual, storage);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn shift_operator_inside_block_comment_does_not_indent_next_statement() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (x) {\n\t\t/* a\n\t\t * is 1 << 10 end */\n\t\tremainder *= 1000;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (x) {\n        /* a\n         * is 1 << 10 end */\n        remainder *= 1000;\n    }\n}\n",
    );
}

// Apostrophes inside block comments are not character-literal delimiters.
#[test]
fn block_comment_apostrophe_in_parens_does_not_indent_following_function() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "/*\n * If a record ends at the exact location of a\n * marker (special token that's a STOP_CHAR)\n * in the stream, silently consume the marker.\n */\nstatic void skip_marker(struct Data *data)\n{\n\tsize_t position;\n}\n",
            &options,
        ),
        "/*\n * If a record ends at the exact location of a\n * marker (special token that's a STOP_CHAR)\n * in the stream, silently consume the marker.\n */\nstatic void skip_marker(struct Data *data)\n{\n    size_t position;\n}\n",
    );
}

#[test]
fn multiline_block_comment_in_parameter_default_keeps_closing_paren() {
    let source = fixture!(
        "C::C(int a,",
        "     long flags /*= A |",
        "                   B",
        "                */)",
        "    : Base(flags)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multiline_block_comment_in_parameter_preserves_gap_before_closing_paren() {
    let source = fixture!(
        "int f(int value /* x",
        "                 y",
        "              */ );",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multiline_block_comment_in_declaration_keeps_semicolon() {
    let source = fixture!("int value /*", " * note", " */;");

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn trailing_comment_colon_does_not_indent_following_statement() {
    let source = fixture!(
        "void f()",
        "{",
        "    CHECK(one); // note:",
        "    CHECK(two);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn comment_after_braceless_body_after_split_function_header_uses_block_indent() {
    assert_eq!(
        format_c(
            "static const char *f(Scope *s, const char *p,\n                     const char *e) {\n  int i = 0;\n  while (check_value(s, p + i, e))\n    i++;\n  /* keep trying */\n  while (i >= 0) {\n    i--;\n  }\n}\n\nstatic void g(Scope *s, int i,\n              const char *e) {\n  if (i != value)\n    call(s, e);\n  /* else done */\n}\n",
            &FormatOptions::default(),
        ),
        "static const char *f(Scope *s, const char *p,\n                     const char *e) {\n    int i = 0;\n    while (check_value(s, p + i, e))\n        i++;\n    /* keep trying */\n    while (i >= 0) {\n        i--;\n    }\n}\n\nstatic void g(Scope *s, int i,\n              const char *e) {\n    if (i != value)\n        call(s, e);\n    /* else done */\n}\n",
    );
}

#[test]
fn first_block_comment_after_split_function_header_uses_block_indent() {
    assert_eq!(
        format_c(
            "static const char *read_text(Scope *s, const char *source,\n                              char *text) {\n  /* scans marks */\n  size_t len = strspn(source, MARKS);\n}\n",
            &FormatOptions::default(),
        ),
        "static const char *read_text(Scope *s, const char *source,\n                             char *text) {\n    /* scans marks */\n    size_t len = strspn(source, MARKS);\n}\n",
    );
}

#[test]
fn block_comment_after_closed_block_after_split_function_header_uses_block_indent() {
    assert_eq!(
        format_c(
            "void update(Scope *scope, Store *store,\n            unsigned old_len, unsigned new_len) {\n  if (new_len < old_len) {\n    swap_all(store, scope);\n    rebuilds(store, old_len, new_len);\n    swap_all(store, scope);\n  }\n  /* allocate new array */\n  new_data = update_data(scope, store, old_len, new_len);\n  /* allocation ok */\n  swap_all(store, scope);\n}\n",
            &FormatOptions::default(),
        ),
        "void update(Scope *scope, Store *store,\n            unsigned old_len, unsigned new_len) {\n    if (new_len < old_len) {\n        swap_all(store, scope);\n        rebuilds(store, old_len, new_len);\n        swap_all(store, scope);\n    }\n    /* allocate new array */\n    new_data = update_data(scope, store, old_len, new_len);\n    /* allocation ok */\n    swap_all(store, scope);\n}\n",
    );
}

#[test]
fn condition_continuation_after_trailing_block_comment_uses_conditional_indent() {
    assert_eq!(
        format_c(
            "void f() {\n  if (nums > 0 &&  /* grows array only if it gets more elements */\n      check(value, total)) {\n    done();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    if (nums > 0 &&  /* grows array only if it gets more elements */\n            check(value, total)) {\n        done();\n    }\n}\n",
    );
}

#[test]
fn block_comment_after_logical_operator_keeps_continuation_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tok = (flags & A) &&",
                "\t     /*",
                "\t      * first",
                "\t      */",
                "\t     !(flags & B) &&",
                "\t     value;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    ok = (flags & A) &&",
            "         /*",
            "          * first",
            "          */",
            "         !(flags & B) &&",
            "         value;",
            "}",
        )
    );
}

#[test]
fn linux_top_level_banner_comment_closer_stays_at_column_zero() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(" /*-****", " * Body", " *****/", "int x;"),
            &options,
        ),
        fixture!("/*-****", "* Body", "*****/", "int x;")
    );
}

#[test]
fn linux_relocated_else_brace_preserves_trailing_comment_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tif (a) {",
                "\t\tx;",
                "\t}",
                "\telse  {   /* note */",
                "\t\ty;",
                "\t}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        x;",
            "    } else  {   /* note */",
            "        y;",
            "    }",
            "}",
        )
    );
}

#[test]
fn linux_spaced_block_comment_opener_keeps_uniform_star_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\t\t /*",
                "\t\t * line one",
                "\t\t * line two",
                "\t\t */",
                "\tx = 1;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    /*",
            "    * line one",
            "    * line two",
            "    */",
            "    x = 1;",
            "}",
        )
    );
}

#[test]
fn block_comment_opened_on_preprocessor_line_continues_to_closer() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");
    let source = fixture!(
        "#endif /* C ||",
        "\t  D */",
        "",
        "static void f(void)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn comment_after_long_template_header_does_not_overindent_next_if() {
    let source = "template < typename IterImpl, enable_if_t < (std::is_same<IterImpl, iter_impl>::value || std::is_same<IterImpl, other_iter_impl>::value), std::nullptr_t > = nullptr >\nbool operator==(const IterImpl& other) const\n{\n    // comment\n    if (cond)\n    {\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// A comment preceding a defaulted special member (`= default;`) must keep the class
// member indent. The `default` keyword here is not a switch label, so it must not
// reindent the comment to switch-label level.
#[test]
fn comment_before_defaulted_member_keeps_member_indent() {
    let source = fixture!(
        "class C",
        "{",
        "public:",
        "    // comment",
        "    C(C&&) noexcept = default;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn block_comment_argument_line_closes_call_without_leaking_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (x) {\n\t\tcall(a,\n\t\t     /* flag= */ true);\n\t\tnext();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (x) {\n        call(a,\n             /* flag= */ true);\n        next();\n    }\n}\n",
    );
}

#[test]
fn block_comment_line_inside_continuation_keeps_continuation_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "int ok = call(arg,\n\t\t      /* note */\n\t\t      other());\n",
            &options,
        ),
        "int ok = call(arg,\n              /* note */\n              other());\n",
    );
}

#[test]
fn force_tab_x_assignment_call_comment_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 4;

    assert_eq!(
        format_c(
            fixture!(
                "void f(){",
                "value = call(",
                "/* note */",
                "\"text\");",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f() {",
            "\tvalue = call(",
            "\t\t\t\t/* note */",
            "\t\t\t\t\"text\");",
            "}",
        ),
    );
}

#[test]
fn force_tab_x_braceless_else_comment_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 4;

    assert_eq!(
        format_c(
            fixture!("void f(){", "if(a){}else", "/* note */", "if(b){}", "}",),
            &options,
        ),
        fixture!(
            "void f() {",
            "\tif(a) {}",
            "\telse",
            "\t\t/* note */",
            "\t\tif(b) {}",
            "}",
        ),
    );
}

#[test]
fn tab_indent_braceless_else_comment_uses_body_tabs() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 6;
    options.tab_width = 6;

    assert_eq!(
        format_c(
            fixture!("void f(){", "if(a){}else", "/* note */", "if(b){}", "}",),
            &options,
        ),
        fixture!(
            "void f() {",
            "\tif(a) {}",
            "\telse",
            "\t\t/* note */",
            "\t\tif(b) {}",
            "}",
        ),
    );
}

#[test]
fn run_in_enum_with_trailing_comments_keeps_element_alignment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "typedef enum { A = 0,  /* first */\n               B = 1, /* second */\n               C = 2   /* third */\n    } E;\n",
            &options,
        ),
        "typedef enum { A = 0,  /* first */\n               B = 1, /* second */\n               C = 2   /* third */\n             } E;\n",
    );
}

#[test]
fn assignment_text_inside_block_comment_does_not_indent_next_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\t/* note\n\t   value = item */\n\n\t/* table\n\t * setup */\n\tfor (i = 0; i < n; i++) {\n\t\tcall();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    /* note\n       value = item */\n\n    /* table\n     * setup */\n    for (i = 0; i < n; i++) {\n        call();\n    }\n}\n",
    );
}

// Reindented block-comment rows with the same source star column remain aligned.
#[test]
fn reindented_comment_space_led_body_line_stays_consistent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "enum e {\n\tA,\n        /*\n\t * line1\n         * line2\n\t */\n\tB,\n};\n",
            &options,
        ),
        "enum e {\n    A,\n    /*\n    * line1\n     * line2\n     */\n    B,\n};\n",
    );
}

#[test]
fn block_comment_continuation_does_not_indent_next_struct_member() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "typedef struct {\n\tword first;\t\t/* text,\n\t\t\t\t\t  more */\n\n\t/* TAG (value) fields */\n\tword second;\t\t\t/* note */\n\tword third;\t\t/* note */\n} Item;\n",
            &options,
        ),
        "typedef struct {\n    word first;\t\t/* text,\n\t\t\t\t\t  more */\n\n    /* TAG (value) fields */\n    word second;\t\t\t/* note */\n    word third;\t\t/* note */\n} Item;\n",
    );
}

#[test]
fn struct_name_in_comment_does_not_indent_next_member() {
    let source = fixture!(
        "",
        "struct Item {",
        "#define ITEM_FLAG 1",
        "    /** @base: See struct Extension. */",
        "    struct Extension base;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn trailing_comment_after_split_multi_statement_line_keeps_compact_gap() {
    assert_eq!(
        format_c(
            "\nstruct Item {\n  unsigned long bcc;     long fill_00[3]; /* Backup Cache Control */\n  unsigned long bcce;    long fill_01[3]; /* Backup Cache Error */\n};\n",
            &FormatOptions::default(),
        ),
        "\nstruct Item {\n    unsigned long bcc;\n    long fill_00[3]; /* Backup Cache Control */\n    unsigned long bcce;\n    long fill_01[3]; /* Backup Cache Error */\n};\n",
    );
}

#[test]
fn leading_block_comment_keeps_trailing_comment_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "static const int t[][2] = {\n\t/* 0 */ {0, 0},  /* two */\n\t/* 1 */ {1, 1}, /* one */\n};\n",
            &options,
        ),
        "static const int t[][2] = {\n    /* 0 */ {0, 0},  /* two */\n    /* 1 */ {1, 1}, /* one */\n};\n",
    );
}

#[test]
fn linux_style_keeps_block_comment_in_braceless_else_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");
    let source = "int helper(int value)\n{\n    if (value)\n        return 1;\n    else\n        /* note\n           more\n         */\n        return 0;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn linux_style_normalizes_mixed_block_comment_body_alignment() {
    // Mixed tab- and space-led rows normalize to one comment-body column.
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n        /*\n         * A\n\t * B\n\t * C\n         * D\n         */\n    call();\n}\n",
            &options,
        ),
        "void helper(void)\n{\n    /*\n     * A\n     * B\n     * C\n     * D\n     */\n    call();\n}\n"
    );
}

#[test]
fn allman_style_keeps_struct_header_block_comment_before_broken_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "union Item {\n    struct {\t/* note */\n        int value;\n    } item;\n};\n",
            &options,
        ),
        "union Item\n{\n    struct  \t/* note */\n    {\n        int value;\n    } item;\n};\n"
    );
}

#[test]
fn block_comment_close_line_does_not_attach_following_statement() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--pad-oper",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nvoid f(void)\n{\n    /* Held across loop iterations until the operation completes or\n       helper releases it on early shutdown. */\n    pin_item(&state);\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn block_comment_continuation_keeps_structural_tab() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=kr",
        "--indent=tab",
        "--break-one-line-headers",
        "--add-braces",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--pad-comma",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "void f()\n{\n\tfor(insert_pos = 0;\n\t        /* warning line one\n\t         *\n\t         * line three text here that is long enough to wrap nicely\n\t         * line four */\n\t        (insert_pos <= count);\n\t        insert_pos++) {\n\t\tbody();\n\t}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn trailing_block_comment_continuation_preserves_source_whitespace() {
    assert_eq!(
        format_c(
            "struct s {\n\tint conf;\t\t/* configuration for the MAX\n\t\t\t\t * (bits 0-7) */\n\tint rts;\n};\n",
            &FormatOptions::default(),
        ),
        "struct s {\n    int conf;\t\t/* configuration for the MAX\n\t\t\t\t * (bits 0-7) */\n    int rts;\n};\n",
    );
}

#[test]
fn line_comment_backslash_with_trailing_space_does_not_continue_line() {
    assert_eq!(
        format_c(
            "\nvalue = make<item>\n        ( 1, 2 ) // \\ \n        ( 3, 4 )       \n        ( 5, 6 );      \n",
            &FormatOptions::default(),
        ),
        "\nvalue = make<item>\n        ( 1, 2 ) // \\ \n        ( 3, 4 )\n        ( 5, 6 );\n",
    );
}

#[test]
fn block_comment_body_preserves_shift_after_first_star_line() {
    assert_eq!(
        format_c(
            "\nvoid foo(void)\n{\n        /*\n\t * text\n\t * more\n\t */\n        call();\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo(void)\n{\n    /*\n    * text\n     * more\n     */\n    call();\n}\n",
    );
}

#[test]
fn block_comment_closer_keeps_star_column_after_shifted_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n    \t/* first\n\t * second\n\t */\n\tcall();\n}\n",
            &options,
        ),
        "void f(void)\n{\n    /* first\n    * second\n    */\n    call();\n}\n",
    );
}

#[test]
fn block_comment_opener_after_opening_brace_stays_on_brace_line() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\t{\t/*\n\t\t * note line\n\t\t */\n\t\tint x;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    {\t/*\n         * note line\n         */\n        int x;\n    }\n}\n",
    );
}

#[test]
fn block_comment_body_does_not_add_extra_shift_when_source_star_column_survives() {
    assert_eq!(
        format_c(
            "\nvoid foo(void)\n{\n\tfor (;;) {\n\t\tif (value)\n\t\t\tresult = 1;\n\t\t\t/*\n\t\t\t * one\n\t\t\t * two\n\t\t\t */\n\t}\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo(void)\n{\n    for (;;) {\n        if (value)\n            result = 1;\n        /*\n         * one\n         * two\n         */\n    }\n}\n",
    );
}

#[test]
fn wrapped_if_condition_with_call_and_block_comment_attaches_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (aaaa &&\n\t\t\tbbb(p) > 0) { /* c */\n\t\ty;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (aaaa &&\n        bbb(p) > 0) { /* c */\n        y;\n    }\n}\n",
    );
}

#[test]
fn deref_statement_after_closed_block_comment_uses_statement_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  /* line one\n   * line two */\n  *out = get (x);\n  *out2 = get (y);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    /* line one\n     * line two */\n    *out = get (x);\n    *out2 = get (y);\n}\n",
    );
}

#[test]
fn class_name_after_class_line_comment_preserves_source_indent() {
    let source = "class //PUBLIC_MARKER\n    ValueSourceIterator\n{\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assignment_followed_only_by_comment_does_not_align_continuation_to_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "static const char text[] VALUE_ATTR = /* note */\n\t\"abc\"\n\t\"def\";\n",
            &options,
        ),
        "static const char text[] VALUE_ATTR = /* note */\n    \"abc\"\n    \"def\";\n",
    );
}

#[test]
fn line_comment_in_assignment_continuation_keeps_following_code_indent() {
    let source = "bool g() {\n    static bool s =\n        // comment one\n        ItemX().Get();\n    return s;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assignment_continuation_survives_comment_after_preprocessor() {
    let source = "inline bool f()\n{\n    static bool s_check =\n#if defined(FEATURE_ALPHA)\n        // comment one\n        // comment two\n        lookup(\"lib\").has_value(\"key\");\n#else\n        false;\n#endif\n    return s_check;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn block_comment_with_blank_line_aligns_closing_delimiter_to_opener() {
    assert_eq!(
        format_c(
            "class C {\n     /**\n        text two\n\n        more\n    */\n    void f();\n};\n",
            &FormatOptions::default(),
        ),
        "class C {\n    /**\n       text two\n\n       more\n    */\n    void f();\n};\n",
    );
}

#[test]
fn one_true_brace_attaches_header_brace_before_trailing_comment_keeping_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (c == n)              /* comment */\n\t{\n\t\tg();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (c == n) {            /* comment */\n        g();\n    }\n}\n",
    );
    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (c == n)              // comment\n\t{\n\t\tg();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (c == n) {            // comment\n        g();\n    }\n}\n",
    );
    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a) {\n\t\tg();\n\t} else              /* comment */\n\t{\n\t\th();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a) {\n        g();\n    } else {            /* comment */\n        h();\n    }\n}\n",
    );
}

#[test]
fn bare_block_brace_keeps_trailing_run_in_comment_on_brace_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\t{   /* note */\n\t\tint x = 1;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    {   /* note */\n        int x = 1;\n    }\n}\n",
    );
}

#[test]
fn attached_brace_preserves_source_whitespace_before_trailing_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (err > 0) {\t\t// skip it\n\t\tx = 1;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (err > 0) {\t\t// skip it\n        x = 1;\n    }\n}\n",
    );
}

#[test]
fn attached_else_brace_preserves_tab_gap_before_block_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a)\n\t\tg();\n\telse\t/* some case */\n\t{\n\t\th();\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a)\n        g();\n    else {\t/* some case */\n        h();\n    }\n}\n",
    );
}

#[test]
fn struct_brace_attaches_before_block_comment_preceding_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("struct foo /* exit info */ {\n\tint a;\n};\n", &options),
        "struct foo { /* exit info */\n    int a;\n};\n",
    );
}

#[test]
fn union_brace_attaches_before_block_comment_preceding_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("union foo /* tag */ {\n\tint a;\n};\n", &options),
        "union foo { /* tag */\n    int a;\n};\n",
    );
}

#[test]
fn enum_brace_keeps_block_comment_before_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum foo /* tag */ {\n\tA,\n};\n", &options),
        "enum foo /* tag */ {\n    A,\n};\n",
    );
}

#[test]
fn if_header_brace_attaches_before_block_comment_preceding_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (size) /* allow empty output */ {\n\t\tx = 1;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (size) { /* allow empty output */\n        x = 1;\n    }\n}\n",
    );
}

#[test]
fn else_header_brace_attaches_before_block_comment_preceding_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (a) {\n\t\tx = 1;\n\t} else /* value > 0 */ {\n\t\ty = 2;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (a) {\n        x = 1;\n    } else { /* value > 0 */\n        y = 2;\n    }\n}\n",
    );
}
#[test]
fn block_comment_after_inner_switch_keeps_outer_case_body_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  switch (op)\n    {\n    case A:\n      switch (edge)\n        {\n        case B:\n          break;\n        }\n\n      /* comment\n       * text\n       */\n      call();\n      break;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (op)\n    {\n    case A:\n        switch (edge)\n        {\n        case B:\n            break;\n        }\n\n        /* comment\n         * text\n         */\n        call();\n        break;\n    }\n}\n",
    );
}
#[test]
fn namespace_opening_brace_does_not_run_in_block_comment() {
    assert_eq!(
        format_c(
            "namespace Actions\n{/* NOTE: Enable.\n\tLine\n*/\n}\n",
            &FormatOptions::default(),
        ),
        "namespace Actions\n{\n/* NOTE: Enable.\nLine\n*/\n}\n",
    );
}
#[test]
fn run_in_brace_line_with_block_and_line_comments_stays_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo() {\n    if (isFoo) { /* comment1 */  // comment2\n        bar();\n    }\n}\n",
            &options,
        ),
        "void foo()\n{   if (isFoo) { /* comment1 */  // comment2\n        bar();\n    }\n}\n",
    );
}
#[test]
fn run_in_cuddled_else_trailing_comments_share_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo()       // comment0\n{\n    if (isFoo) { // comment1\n        bar1();  // comment2\n    } else {     // comment3\n        bar2();  // comment4\n    }\n}\n",
            &options,
        ),
        "void foo()       // comment0\n{   if (isFoo)   // comment1\n    {   bar1();  // comment2\n    }\n    else         // comment3\n    {   bar2();  // comment4\n    }\n}\n",
    );
}
#[test]
fn kr_function_with_header_block_comment_keeps_run_in_brace_comment_in_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo(bool isFoo) /* comment0 */\n{   // comment1\n    if (isFoo)\n    {   // comment2\n        fooBar();\n    }\n}\n",
            &options,
        ),
        "void foo(bool isFoo) /* comment0 */\n{\n    // comment1\n    if (isFoo) {\n        // comment2\n        fooBar();\n    }\n}\n",
    );
}
#[test]
fn onetrue_function_header_block_comment_keeps_brace_line_comment_in_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void) /* block */\n{ // line\n    call();\n}\n",
            &options,
        ),
        "void f(void) /* block */\n{\n    // line\n    call();\n}\n",
    );
}
#[test]
fn onetrue_header_block_comment_moves_brace_line_comment_to_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n    if (ready) /* block */\n    { // line\n        call();\n    }\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (ready) { /* block */\n        // line\n        call();\n    }\n}\n",
    );
}
#[test]
fn onetrue_brace_line_block_and_line_comments_attach_to_header() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n    if (ready)\n    { /* block */ // line\n        call();\n    }\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (ready) { /* block */ // line\n        call();\n    }\n}\n",
    );
}
#[test]
fn onetrue_same_line_header_block_comment_preserves_gap_before_brace_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n    if (ready) /* block */ { // line\n        call();\n    }\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (ready) { /* block */  // line\n        call();\n    }\n}\n",
    );
}
#[test]
fn kr_function_opening_brace_with_line_comment_breaks_before_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void foo() {  // comment\n    bar();\n}\n", &options),
        "void foo()    // comment\n{\n    bar();\n}\n",
    );
}
#[test]
fn kr_moved_brace_comment_preserves_adjacent_source_gaps() {
    for args in [
        vec!["--style=kr".to_owned()],
        vec!["--style=kr".to_owned(), "--pad-paren-out".to_owned()],
    ] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &args).expect("valid options");

        assert_eq!(
            format_c("void foo(){// comment\n}\n", &options),
            "void foo() // comment\n{\n}\n",
        );
    }
}
#[test]
fn java_attach_brace_reduces_tab_gap_before_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo()\t\t\t// comment\n{\n    if (isFoo)\t\t\t// comment\n    {\n        bar = 0;\n    }\n}\n",
            &options,
        ),
        "void foo() {\t\t// comment\n    if (isFoo) {\t\t// comment\n        bar = 0;\n    }\n}\n",
    );
}
#[test]
fn java_run_in_brace_comment_after_function_comment_moves_to_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo(bool isFoo) // comment0\n{   // comment1\n    if (isFoo)\n    {   // comment2\n        fooBar();\n    }\n}\n",
            &options,
        ),
        "void foo(bool isFoo) { // comment0\n    // comment1\n    if (isFoo) {\n        // comment2\n        fooBar();\n    }\n}\n",
    );
}
#[test]
fn java_function_brace_attaches_before_trailing_line_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo(bool isFoo) // comment0\n{\n    // comment1\n    if(isFoo)\n    {\n        // comment2\n        fooBar();\n    }\n}\n",
            &options,
        ),
        "void foo(bool isFoo) { // comment0\n    // comment1\n    if(isFoo) {\n        // comment2\n        fooBar();\n    }\n}\n",
    );
}
#[test]
fn allman_brace_line_with_block_and_line_comments_stays_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let source =
        "void foo()\n{\n    if (isFoo) { /* comment1 */  // comment2\n        bar();\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn allman_cuddled_else_trailing_comments_share_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void foo() {     // comment0\n    if (isFoo) { // comment1\n        bar1();  // comment2\n    } else {     // comment3\n        bar2();  // comment4\n    }\n}\n",
            &options,
        ),
        "void foo()       // comment0\n{\n    if (isFoo)   // comment1\n    {\n        bar1();  // comment2\n    }\n    else         // comment3\n    {\n        bar2();  // comment4\n    }\n}\n",
    );
}
#[test]
fn block_comment_after_preprocessor_split_if_indents_as_body() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  while (x)\n    {\n#if A\n      if (x)\n#else\n      if (y)\n#endif\n        /* comment */\n        break;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    while (x)\n    {\n#if A\n        if (x)\n#else\n        if (y)\n#endif\n            /* comment */\n            break;\n    }\n}\n",
    );
}
#[test]
fn switch_case_comment_after_nested_if_block_keeps_case_body_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  switch (value)\n  {\n    case A:\n      if (ready)\n        {\n          done = true;\n          break;\n        }\n      /* line one,\n       * line two\n       */\n      call();\n      break;\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (value)\n    {\n    case A:\n        if (ready)\n        {\n            done = true;\n            break;\n        }\n        /* line one,\n         * line two\n         */\n        call();\n        break;\n    }\n}\n",
    );
}
#[test]
fn pointer_assignment_after_block_comment_is_not_comment_body() {
    assert_eq!(
        format_c(
            "void f(int *minimum)\n{\n  /* reserve space\n   * for items\n   */\n  *minimum = 1;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(int *minimum)\n{\n    /* reserve space\n     * for items\n     */\n    *minimum = 1;\n}\n",
    );
}
#[test]
fn namespace_opening_brace_keeps_trailing_line_comment() {
    let source = "namespace foo {  // line c\nint x;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn operator_continuation_after_blank_comment_in_condition_keeps_condition_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tif((alpha >= beta) // comment\n\n\t        // more\n\t        || (gamma > delta)) {\n\t\tbreak;\n\t}\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    if((alpha >= beta) // comment\n\n            // more\n            || (gamma > delta)) {\n        break;\n    }\n}\n",
    );
}

#[test]
fn plus_continuation_after_blank_comment_keeps_expression_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tvalue = alpha // comment\n\t        + beta // more\n\n\t        // note\n\t        + gamma;\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    value = alpha // comment\n            + beta // more\n\n            // note\n            + gamma;\n}\n",
    );
}

#[test]
fn trailing_block_comment_preserves_absent_source_gap() {
    assert_eq!(
        format_c(
            "struct S {\n\tunsigned long value;/* note\n\t\t\t\t   * next */\n};\n",
            &FormatOptions::default(),
        ),
        "struct S {\n    unsigned long value;/* note\n\t\t\t\t   * next */\n};\n",
    );
}

#[test]
fn inline_block_trailing_line_comment_keeps_single_space() {
    let options = FormatOptions::default();

    assert_eq!(format_c("void g(){}//c\n", &options), "void g() {} //c\n");
    assert_eq!(format_c("class C{};//c\n", &options), "class C {}; //c\n");
    assert_eq!(format_c("x{}//c\n", &options), "x{}//c\n");
}

#[test]
fn gnu_block_comment_before_semicolon_has_no_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f()\n{ /* block */;\n}\n", &options),
        "void f()\n{\n    /* block */;\n}\n",
    );
}

#[test]
fn gnu_block_comment_before_semicolon_preserves_source_gap() {
    let mut options = FormatOptions::default();
    let args = ["--style=gnu", "--keep-one-line-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "{ /* block */ ; }\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn horstmann_run_in_brace_keeps_glued_trailing_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let source = "{   x=1;// c\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn horstmann_run_in_brace_aligns_trailing_comment_after_operator_pad() {
    let mut options = FormatOptions::default();
    let args = ["--style=horstmann", "--pad-oper"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(format_c("{   x=1;// c\n", &options), "{   x = 1; // c\n",);
}

#[test]
fn horstmann_malformed_run_in_line_comment_spacing_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let input = "{\nalpha\t%=break// line==catch\n#define X(x) \\\nif-  \n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn whitesmith_malformed_inline_block_comment_scope_line_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let first = format_c(
        "alphareturn/* block */(*<enum<=helper::resultthrow\n",
        &options,
    );

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn default_malformed_close_number_after_line_comment_block_is_idempotent() {
    let options = FormatOptions::default();
    let input = "]<::+throw// line~{continue-\ncase}42%::[<elsebreakvalue42alpha>=|throwalpha<#else>=default\nvalueresult;autox[\n<throwauto// linevalueItemfortryItem+!=/1casetry)#elsehelper0*resultNULL[->// lineNULL!(switchresult%default\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn block_comment_after_split_condition_body_uses_body_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            "void f(int nArg, int iMode){\n  for(i=1; i<nArg; i++){\n    if( z[0]!='-'\n     && iMode<0\n     && (mode = find(arg))>=0\n     && mode!=WWW\n    ){\n      iMode = i;\n      modeChange(mode);\n      /* If the mode is insert and the next argument\n      ** is not an option.\n      */\n      if( i+1<nArg && arg[i+1][0]!='-' ){\n        i++;\n      }\n      chng = 1;\n    }else if( other ){\n      done();\n    }\n  }\n}\n",
            &options,
        ),
        "void f(int nArg, int iMode) {\n    for(i=1; i<nArg; i++) {\n        if( z[0]!='-'\n                && iMode<0\n                && (mode = find(arg))>=0\n                && mode!=WWW\n          ) {\n            iMode = i;\n            modeChange(mode);\n            /* If the mode is insert and the next argument\n            ** is not an option.\n            */\n            if( i+1<nArg && arg[i+1][0]!='-' ) {\n                i++;\n            }\n            chng = 1;\n        } else if( other ) {\n            done();\n        }\n    }\n}\n",
    );
}

#[test]
fn horstmann_run_in_plain_block_comment_closer_is_stable() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run()\n{   /* heading\n       alpha\n          beta\n    */\n    call();\n}\n",
            &options,
        ),
        "void run()\n{   /* heading\n       alpha\n          beta\n    */\n    call();\n}\n",
    );
}

#[test]
fn pico_commented_definition_run_in_body_uses_indent_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");
    let source = "int value() // comment\n{return 1;}\n";
    let expected = "int value() // comment\n{   return 1;}\n";

    assert_eq!(format_c(source, &options), expected);
    assert_eq!(format_c(expected, &options), expected);
}

#[test]
fn remove_comment_prefix_run_in_comment_keeps_single_opener_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--remove-comment-prefix".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\n/* heading\n * body\n */\ncall();\n}\n",
            &options,
        ),
        "void run()\n{   /* heading\n        body\n    */\n    call();\n}\n",
    );
}

// Stripped multiline comment text stays one indent below its owning scope.
#[test]
fn remove_comment_prefix_strips_unterminated_body_prefix() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--remove-comment-prefix".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void run(){\ncall(); /* unterminated\n * body\n", &options,),
        "void run()\n{\n    call(); /* unterminated\n        body\n",
    );
}
