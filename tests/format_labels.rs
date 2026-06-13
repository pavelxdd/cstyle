#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{FormatOptions, apply_command_line_args};

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
fn attached_block_user_label_stays_at_column_zero() {
    let source = fixture!(
        "int helper(void)",
        "{",
        "    goto error;",
        "",
        "error: {",
        "        int err = errno;",
        "    }",
        "    return -1;",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn preserves_source_space_before_plain_label_colon() {
    let actual = format_c(
        fixture!("void f(void){", "again : doStuff();", "next();", "}"),
        &FormatOptions::default(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "again :",
            "    doStuff();",
            "    next();",
            "}",
        )
    );
}

#[test]
fn formats_plain_labels_and_nested_blocks() {
    let actual = format(fixture!("void f(){start:while(x){x--;}}"));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "start:",
            "    while (x)",
            "    {",
            "        x--;",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_labels_indents_labels_to_parent_scope_column() {
    let mut options = FormatOptions::default();
    options.indent_labels = true;
    let actual = format_with(fixture!("void f(){while(x){again:return;}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    while (x)",
            "    {",
            "    again:",
            "        return;",
            "    }",
            "}",
        )
    );
}

#[test]
fn label_in_split_else_block_keeps_following_statement_indent() {
    let actual = format_c(
        fixture!(
            "void f(void){",
            "#ifndef OMIT",
            "  if(a){x();}else",
            "#endif",
            "",
            "  if( c ){",
            "    if( ready ){",
            "label:",
            "      fail();",
            "    }else{",
            "      ok();",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "#ifndef OMIT",
            "    if(a) {",
            "        x();",
            "    }",
            "    else",
            "#endif",
            "",
            "        if( c ) {",
            "            if( ready ) {",
            "label:",
            "                fail();",
            "            } else {",
            "                ok();",
            "            }",
            "        }",
            "}",
        )
    );
}

#[test]
fn label_in_braceless_split_else_body_keeps_following_statement_indent() {
    let actual = format_c(
        fixture!(
            "void f(void){",
            "#ifndef OMIT",
            "  if(a){x();}else",
            "#endif",
            "",
            "  if( c ){",
            "    if( a ){",
            "      call();",
            "    }else",
            "      /* comment */",
            "label:",
            "      help();",
            "  }else",
            "",
            "  if( d ){",
            "    next();",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "#ifndef OMIT",
            "    if(a) {",
            "        x();",
            "    }",
            "    else",
            "#endif",
            "",
            "        if( c ) {",
            "            if( a ) {",
            "                call();",
            "            } else",
            "                /* comment */",
            "label:",
            "                help();",
            "        } else",
            "",
            "            if( d ) {",
            "                next();",
            "            }",
            "}",
        )
    );
}

#[test]
fn expression_shaped_label_splits_following_statement() {
    assert_eq!(
        format_c(
            "void f()\n{\n    alpha-beta: gamma/delta\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\nalpha-beta:\n    gamma/delta\n}\n",
    );
}

#[test]
fn first_statement_after_function_label_uses_body_indent() {
    assert_eq!(
        format_c(
            "void f(void) {\n  if(value){\n    goto done;\n  }\ndone:\n  call();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    if(value) {\n        goto done;\n    }\ndone:\n    call();\n}\n",
    );
}

#[test]
fn split_function_parameters_after_prior_label_keep_parameter_indent() {
    assert_eq!(
        format_c(
            "int f(int n) {\n  if( n ) goto usage;\nusage:\n  return n;\n}\n\nstatic void prepare(\n  Type *cx,\n  int *ok\n){\n}\n",
            &FormatOptions::default(),
        ),
        "int f(int n) {\n    if( n ) goto usage;\nusage:\n    return n;\n}\n\nstatic void prepare(\n    Type *cx,\n    int *ok\n) {\n}\n",
    );
}

#[test]
fn braceless_if_body_keeps_indent_after_intervening_label() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (ret && ret != -FAILED)\nerr:\n\t\treturn do_thing(a, b);\n\tnext();\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (ret && ret != -FAILED)\nerr:\n        return do_thing(a, b);\n    next();\n}\n",
    );
}

#[test]
fn label_before_file_scope_initializer_does_not_indent_preprocessor_member() {
    let source = "int f(void)\n{\nexit_label:\n\treturn result;\n}\n\nstatic const struct operation_table_type value = {\n\t.action\t\t= standard_action,\n#if HAS_OPTION(OPTION_SET)\n\t.value\t\t= default_value,\n\t.map_items\t= default_map_items,\n\t.item_handler\t= default_item_handler,\n#endif\n};\n";
    let expected = "int f(void)\n{\nexit_label:\n    return result;\n}\n\nstatic const struct operation_table_type value = {\n    .action\t\t= standard_action,\n#if HAS_OPTION(OPTION_SET)\n    .value\t\t= default_value,\n    .map_items\t= default_map_items,\n    .item_handler\t= default_item_handler,\n#endif\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}
#[test]
fn labels_keep_following_statement_on_next_line() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
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
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nint f(void)\n{\n    int err = 1;\nerror_one:\n    cleanup_one();\nerror_two:\n    cleanup_two();\n    return err;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn goto_label_with_trailing_comment_stays_at_column_zero() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tgoto underrun;\nunderrun:\t\t\t/* comment */\n\tx = 1;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    goto underrun;\nunderrun:\t\t\t/* comment */\n    x = 1;\n}\n",
    );
}

#[test]
fn default_malformed_colon_brace_body_label_keeps_label_indent() {
    assert_eq!(
        format_c(
            "for8&trywhile+:{continue7/  :returnx{<=\n",
            &FormatOptions::default(),
        ),
        "for8&trywhile+: {\ncontinue7/  :\n    returnx{<=\n",
    );
}

#[test]
fn whitesmith_malformed_colon_brace_body_label_keeps_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("for8&trywhile+:{continue7/  :returnx{<=\n", &options),
        "for8&trywhile+:\n    {\ncontinue7/  :\n    returnx{<=\n",
    );
}

#[test]
fn default_malformed_label_adjacent_braces_are_expanded_idempotently() {
    let options = FormatOptions::default();
    let first = format_c("x:{{result}y\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_bracket_brace_after_label_body_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let first = format_c("x:![{a;// linez\n\n!b\n", &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn allman_malformed_label_continuation_brace_indent_is_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "default#endif][#endif==*constexprswitch||gammaItem;42beta)gamma1:result!continuex?Itemdoif]if!=value?)namespace1throwvalue/!=  ||helperstructdefault{call>=;enum&&namespace\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn horstmann_indent_labels_does_not_move_the_enclosing_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=horstmann".to_owned(), "--indent-labels".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void run(){\nif(ready){\nagain:\ncall();\n}\n}\n", &options,),
        "void run()\n{   if(ready)\n    {   again:\n        call();\n    }\n}\n",
    );
}

#[test]
fn horstmann_force_tabs_preserves_run_in_label_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent=force-tab=4".to_owned(),
        ],
    )
    .expect("valid options");
    let input = "void run(){\nagain:\ncall();\n}\n";
    let expected = "void run()\n{\tagain:\n\tcall();\n}\n";
    let first = format_c(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn java_indent_labels_uses_the_parent_scope_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=java".to_owned(), "--indent-labels".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\nagain:\ncall();\nif(next()) goto again;\n}\n}\n",
            &options,
        ),
        "void run() {\n    if(ready) {\n    again:\n        call();\n        if(next()) goto again;\n    }\n}\n",
    );
}

#[test]
fn user_label_inside_indented_switch_stays_at_column_zero_by_default() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--indent-switches".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:\nretry:\ncall();\nbreak;\ndefault:\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n        case 1:\nretry:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn indent_labels_inside_indented_switch_uses_the_case_label_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent-switches".to_owned(),
            "--indent-labels".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:\nretry:\ncall();\nbreak;\ndefault:\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n        case 1:\n        retry:\n            call();\n            break;\n        default:\n            break;\n    }\n}\n",
    );
}

#[test]
fn tab_indented_label_inside_indented_case_uses_structural_tabs() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=allman".to_owned(),
            "--indent=tab=4".to_owned(),
            "--indent-switches".to_owned(),
            "--indent-cases".to_owned(),
            "--indent-labels".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:\nretry:\ncall();\nbreak;\ndefault:\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n\tswitch(value)\n\t{\n\t\tcase 1:\n\t\tretry:\n\t\t\tcall();\n\t\t\tbreak;\n\t\tdefault:\n\t\t\tbreak;\n\t}\n}\n",
    );
}

#[test]
fn whitesmith_label_block_brace_and_body_share_the_nested_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\nretry:{\ncall();\n}\n}\n}\n",
            &options,
        ),
        "void run()\n    {\n    if(ready)\n        {\nretry:\n            {\n            call();\n            }\n        }\n    }\n",
    );
}

#[test]
fn ratliff_label_block_closer_uses_the_nested_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\nretry:{\ncall();\n}\n}\n}\n",
            &options,
        ),
        "void run() {\n    if(ready) {\nretry: {\n            call();\n            }\n        }\n    }\n",
    );
}

#[test]
fn whitesmith_statement_after_consecutive_labels_uses_the_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\nfirst:\nsecond:\ncall();\ngoto first;\n}\n}\n",
            &options,
        ),
        "void run()\n    {\n    if(ready)\n        {\nfirst:\nsecond:\n        call();\n        goto first;\n        }\n    }\n",
    );
}

#[test]
fn java_label_block_uses_the_nested_scope_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\nretry:{\ncall();\nif(next()) goto retry;\n}\n}\n}\n",
            &options,
        ),
        "void run() {\n    if(ready) {\nretry: {\n            call();\n            if(next()) goto retry;\n        }\n    }\n}\n",
    );
}

#[test]
fn vtk_comment_after_user_label_uses_the_label_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(){\nif(ready){\n// before\nretry:\n/* after */\ncall();\n}\n}\n",
            &options,
        ),
        "void run()\n{\n    if(ready)\n        {\n// before\nretry:\n        /* after */\n        call();\n        }\n}\n",
    );
}

#[test]
fn indent_labels_inside_case_uses_the_case_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=allman".to_owned(), "--indent-labels".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:\nretry:\ncall();\nif(next()) goto retry;\nbreak;\ndefault:\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n    case 1:\n    retry:\n        call();\n        if(next()) goto retry;\n        break;\n    default:\n        break;\n    }\n}\n",
    );
}
