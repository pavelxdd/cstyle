#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, IndentStyle, apply_command_line_args};

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

fn options_from_args(args: &[&str]) -> FormatOptions {
    let mut options = FormatOptions::default();
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    apply_command_line_args(&mut options, &args).expect("valid C options");
    options
}

fn one_true_brace_c_options() -> FormatOptions {
    options_from_args(ONE_TRUE_BRACE_C_ARGS)
}

fn kr_c_options() -> FormatOptions {
    options_from_args(KR_C_ARGS)
}

#[test]
fn nested_switch_keeps_block_indent_inside_case_brace() {
    let source = fixture!(
        "void helper(int value)",
        "{",
        "    switch (value) {",
        "        case 1: {",
        "            switch (other) {",
        "                case 2:",
        "                    call();",
        "                    break;",
        "            }",
        "            break;",
        "        }",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn braceless_case_bodies_keep_case_indent() {
    let source = fixture!(
        "void helper(int value)",
        "{",
        "    switch (value) {",
        "        case 0:",
        "            if (value > 1)",
        "                return;",
        "            if (value < 0)",
        "                value = 0;",
        "            return;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn case_macro_rows_preserve_explicit_source_indent() {
    let source = fixture!(
        "static int helper(int value)",
        "{",
        "#define ITEM(x) case x: return x",
        "    switch (value) {",
        "            ITEM(1);  ITEM(2);  ITEM(3);",
        "            ITEM(4);  ITEM(5);  ITEM(6);",
        "        default:",
        "            return 0;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn formats_nested_switch_case_blocks() {
    let actual = format(fixture!(
        "int f(int x,int y){switch(x){case 1:{switch(y){case 2:return 2;}}default:return 0;}}"
    ));
    assert_eq!(
        actual,
        fixture!(
            "int f(int x, int y)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "    {",
            "        switch (y)",
            "        {",
            "        case 2:",
            "            return 2;",
            "        }",
            "    }",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );
}

#[test]
fn split_case_return_preserves_adjacent_line_comment() {
    assert_eq!(
        format_c(
            fixture!(
                "Date f(int value)",
                "{",
                "    switch (value) {",
                "    case 6: return Date(2050, 1, 1);// comment",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "Date f(int value)",
            "{",
            "    switch (value) {",
            "    case 6:",
            "        return Date(2050, 1, 1);// comment",
            "    }",
            "}",
        )
    );
}

#[test]
fn switch_case_with_parenthesized_value_breaks_return_body() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto native = [](Type value) {",
        "        switch (value) {",
        "        case(A):",
        "            return B;",
        "        case(C):",
        "            return D;",
        "        default:",
        "            return E;",
        "        }",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn switch_case_body_uses_unindented_case_body_level() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (result) {",
                "        case Pass:",
                "            CHECK(true);",
                "            CHECK_EQ(2 + 1, 3);",
                "            break;",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    switch (result) {",
            "    case Pass:",
            "        CHECK(true);",
            "        CHECK_EQ(2 + 1, 3);",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn expands_combined_case_labels_without_shifting_following_labels() {
    let actual = format_c(
        fixture!(
            "static int compare(Item *left, Node *right, int flag) {",
            "  if (category(left) != category(right)) {",
            "    if (is_short(right) && is_long(left)) {",
            "      return equal(value(left), value(right));",
            "    }",
            "    else if (flag && is_empty(right)) {",
            "      return 0;",
            "   }",
            "   else",
            "     return 0;",
            "  }",
            "  else {",
            "    switch (category(right)) {",
            "      case ALPHA: case BETA: case GAMMA:",
            "        return 1;",
            "      case DELTA:",
            "        return equal(value(left), value(right));",
            "      case EPSILON:",
            "        return compare_value(left, right);",
            "      default:",
            "        return 0;",
            "    }",
            "  }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static int compare(Item *left, Node *right, int flag) {",
            "    if (category(left) != category(right)) {",
            "        if (is_short(right) && is_long(left)) {",
            "            return equal(value(left), value(right));",
            "        }",
            "        else if (flag && is_empty(right)) {",
            "            return 0;",
            "        }",
            "        else",
            "            return 0;",
            "    }",
            "    else {",
            "        switch (category(right)) {",
            "        case ALPHA:",
            "        case BETA:",
            "        case GAMMA:",
            "            return 1;",
            "        case DELTA:",
            "            return equal(value(left), value(right));",
            "        case EPSILON:",
            "            return compare_value(left, right);",
            "        default:",
            "            return 0;",
            "        }",
            "    }",
            "}",
        )
    );
}

#[test]
fn split_switch_header_keeps_case_structure() {
    let actual = format_c(
        fixture!(
            "void f(int x){",
            "switch",
            "(x){",
            "case 1:",
            "call();",
            "}",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch",
            "    (x) {",
            "    case 1:",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn block_comment_in_switch_header_keeps_case_structure() {
    let actual = format_c(
        fixture!(
            "void f(int x){",
            "switch /* comment */ (x){",
            "case 1:",
            "call();",
            "}",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch /* comment */ (x) {",
            "    case 1:",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn line_comment_split_switch_header_keeps_case_structure() {
    let actual = format_c(
        fixture!(
            "void f(int x){",
            "switch // comment",
            "(x){",
            "case 1:",
            "call();",
            "}",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch // comment",
            "    (x) {",
            "    case 1:",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn multiline_block_comment_in_switch_header_keeps_case_structure() {
    let actual = format_c(
        fixture!(
            "void f(int x){",
            "switch /* first",
            " { second */ (x){",
            "case 1:",
            "call();",
            "}",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch /* first",
            " { second */ (x) {",
            "    case 1:",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn gnu_indent_blocks_keeps_case_brace_at_case_level() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let actual = format_c(
        fixture!(
            "void run(int value){",
            "switch(value){",
            "case 1:",
            "{",
            "process();",
            "}",
            "break;",
            "default:",
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
            "    switch(value)",
            "        {",
            "        case 1:",
            "        {",
            "            process();",
            "        }",
            "        break;",
            "        default:",
            "            break;",
            "        }",
            "}",
        )
    );
}

#[test]
fn force_tabs_uses_tab_at_case_body_column() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.indent_width = 4;
    options.tab_width = 8;
    let actual = format_with(
        fixture!("int f(int x){switch(x){case 1:{return 1;}}}"),
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
            "\treturn 1;",
            "    }",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_switches_indents_case_labels_and_bodies() {
    let mut options = FormatOptions::default();
    options.indent_switches = true;
    let actual = format_with(
        fixture!("int f(int x){switch(x){case 1:return 1;}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "        case 1:",
            "            return 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn indent_cases_does_not_add_a_second_level_to_unbraced_case_bodies() {
    let source = fixture!("int f(int x){switch(x){case 1:return 1;default:return 0;}}");

    let default_options = FormatOptions::default();
    assert_eq!(
        format_with(source, &default_options),
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "        return 1;",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );

    let mut switch_options = FormatOptions::default();
    switch_options.indent_switches = true;
    assert_eq!(
        format_with(source, &switch_options),
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "        case 1:",
            "            return 1;",
            "        default:",
            "            return 0;",
            "    }",
            "}",
        )
    );

    let mut case_options = FormatOptions::default();
    case_options.indent_cases = true;
    assert_eq!(
        format_with(source, &case_options),
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case 1:",
            "        return 1;",
            "    default:",
            "        return 0;",
            "    }",
            "}",
        )
    );

    let mut both_options = FormatOptions::default();
    both_options.indent_switches = true;
    both_options.indent_cases = true;
    assert_eq!(
        format_with(source, &both_options),
        fixture!(
            "int f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "        case 1:",
            "            return 1;",
            "        default:",
            "            return 0;",
            "    }",
            "}",
        )
    );
}

#[test]
fn consecutive_case_labels_split_onto_separate_lines() {
    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    switch (value)",
                "    {",
                "    case 1:case 2:",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run()",
            "{",
            "    switch (value)",
            "    {",
            "    case 1:",
            "    case 2:",
        ),
    );
}

#[test]
fn split_case_label_line_keeps_default_comment_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int value)\n{\n\tswitch (value)\n\t{\n\tcase 0: case 1: default:   /* note */\n\t\tresult = 1;\n\t\tbreak;\n\t}\n}\n",
            &options,
        ),
        "void f(int value)\n{\n    switch (value)\n    {\n    case 0:\n    case 1:\n    default:   /* note */\n        result = 1;\n        break;\n    }\n}\n",
    );
}

#[test]
fn indent_switches_preserves_nested_case_label_columns() {
    let mut options = FormatOptions::default();
    options.indent_switches = true;
    let source = fixture!(
        "void f(int x)",
        "{",
        "    switch (x)",
        "    {",
        "        case 3:",
        "        {",
        "            switch (x)",
        "            {",
        "                case 1:",
        "                    a = 0;",
        "            }",
        "        }",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn indent_switches_keeps_standalone_macro_invocation_at_case_body_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_switches = true;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    switch (x) {",
            "        case 0x21:",
            "            doit();",
            "            break;",
            "            ITEM_CASE(0x26, GAMMA)",
            "            break;",
            "        case 0x28:",
            "            other();",
            "            break;",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    switch (x) {",
            "        case 0x21:",
            "            doit();",
            "            break;",
            "            ITEM_CASE(0x26, GAMMA)",
            "            break;",
            "        case 0x28:",
            "            other();",
            "            break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn macro_call_first_in_case_body_breaks_from_following_statement() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int mode)\n{\n\tswitch (mode) {\n\tcase A:\n\t\tguard_scope(mutex, &item)\n\t\t\tvalue = alpha | (beta << 4);\n\t\treturn store_value(value, output);\n\tcase B:\n\t\tbreak;\n\t}\n}\n",
            &options,
        ),
        "void f(int mode)\n{\n    switch (mode) {\n    case A:\n        guard_scope(mutex, &item)\n        value = alpha | (beta << 4);\n        return store_value(value, output);\n    case B:\n        break;\n    }\n}\n",
    );
}

#[test]
fn over_indented_macro_under_case_label_normalizes_to_case_body_indent() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "int f(int x)\n{\n\tswitch (x) {\n\t\tcase A:\n\t\t\tCHECK();\n\t}\n}\n",
            &options,
        ),
        "int f(int x)\n{\n    switch (x) {\n    case A:\n        CHECK();\n    }\n}\n",
    );
}

#[test]
fn statements_after_switch_keep_function_body_indent() {
    let actual = format_c(
        fixture!(
            "template <typename OutputIt, typename Char>",
            "auto f(OutputIt out, int cp) -> OutputIt {",
            "  auto c = static_cast<Char>(cp);",
            "  switch (cp) {",
            "    case 1:",
            "      *out++ = static_cast<Char>('x');",
            "      break;",
            "    default:",
            "      if (cp < 0x100) return call<2, Char>(out, 'x', cp);",
            "      if (cp < 0x10000)",
            "        return call<4, Char>(out, 'u', cp);",
            "      for (Char ch : view(",
            "          begin, end)) {",
            "        out = call<2, Char>(out, 'x',",
            "                            static_cast<int>(ch));",
            "      }",
            "      return out;",
            "  }",
            "  *out++ = c;",
            "  return out;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "template <typename OutputIt, typename Char>",
            "auto f(OutputIt out, int cp) -> OutputIt {",
            "    auto c = static_cast<Char>(cp);",
            "    switch (cp) {",
            "    case 1:",
            "        *out++ = static_cast<Char>('x');",
            "        break;",
            "    default:",
            "        if (cp < 0x100) return call<2, Char>(out, 'x', cp);",
            "        if (cp < 0x10000)",
            "            return call<4, Char>(out, 'u', cp);",
            "        for (Char ch : view(",
            "                    begin, end)) {",
            "            out = call<2, Char>(out, 'x',",
            "                                static_cast<int>(ch));",
            "        }",
            "        return out;",
            "    }",
            "    *out++ = c;",
            "    return out;",
            "}",
        )
    );
}

#[test]
fn complete_ternary_in_case_does_not_misalign_later_case_body() {
    assert_eq!(
        format_c(
            "int f(int x, int d)\n{\n  switch (x)\n    {\n    case A:\n      return d == R ? E : S;\n    case B:\n    default:\n      return x;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "int f(int x, int d)\n{\n    switch (x)\n    {\n    case A:\n        return d == R ? E : S;\n    case B:\n    default:\n        return x;\n    }\n}\n",
    );
}

#[test]
fn pointer_assignment_first_statement_after_split_case_uses_case_body_indent() {
    assert_eq!(
        format_c(
            "int f(int key, int *span) {\n  struct Item { char c; union { WORD_PAD; } u; };\n  *span = 0;\n  switch (key) {\n    case 'b': *span = sizeof(char); return IVal;\n    case 'B': *span = sizeof(char); return UVal;\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "int f(int key, int *span) {\n    struct Item {\n        char c;\n        union {\n            WORD_PAD;\n        } u;\n    };\n    *span = 0;\n    switch (key) {\n    case 'b':\n        *span = sizeof(char);\n        return IVal;\n    case 'B':\n        *span = sizeof(char);\n        return UVal;\n    }\n}\n",
    );
}

#[test]
fn switch_case_assignment_ternary_after_split_call_keeps_call_indent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  switch (type)\n    {\n    case A:\n      *result_type =\n        bitset_intersection(\n          a, b, &result)\n        ? BITSET_CONTAINER_TYPE\n        : ARRAY_CONTAINER_TYPE;\n      return result;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (type)\n    {\n    case A:\n        *result_type =\n            bitset_intersection(\n                a, b, &result)\n            ? BITSET_CONTAINER_TYPE\n            : ARRAY_CONTAINER_TYPE;\n        return result;\n    }\n}\n",
    );
}

#[test]
fn case_label_with_trailing_comment_after_blank_keeps_body_indent() {
    let actual = format_c(
        fixture!(
            "void f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "        case A :",
            "            value = 0;",
            "            break ;",
            "",
            "        case B : // comment",
            "            value = 1;",
            "            break ;",
            "    }",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch (x)",
            "    {",
            "    case A :",
            "        value = 0;",
            "        break ;",
            "",
            "    case B : // comment",
            "        value = 1;",
            "        break ;",
            "    }",
            "}",
        )
    );
}

#[test]
fn nested_label_colon_inside_case_expression_is_not_a_case_separator() {
    let actual = format_c(
        fixture!("void f(int x){switch(x){case ({ retry: 1; }): call();}}"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x) {",
            "    switch(x) {",
            "    case ({ retry: 1; }):",
            "        call();",
            "    }",
            "}",
        )
    );
}
#[test]
fn switch_first_case_return_uses_consistent_case_body_indent() {
    // First and later case bodies use the same indentation.
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--indent-switches",
        "--pad-oper",
        "--pad-header",
        "--align-pointer=name",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nstatic inline enum item_kind item_next(enum item_kind kind)\n{\n    switch (kind) {\n        case ITEM_ALPHA:\n            return ITEM_ALPHA_NEXT;\n        case ITEM_BETA:\n            return ITEM_BETA_NEXT;\n        default:\n            return ITEM_NONE;\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn case_range_ellipsis_preserves_source_spacing() {
    let source = "\nvoid foo(int value)\n{\n    switch (value) {\n    case 0x30 ... 0x3f:\n        break;\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn ternary_continuation_in_case_body_keeps_assignment_indent() {
    let source = "void f(void)\n{\n    switch (t)\n    {\n    case A:\n        *result_type =\n            array_union(CAST(c1),\n                        CAST(c2), &result)\n            ? BITSET_TYPE\n            : ARRAY_TYPE;\n        return result;\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn ternary_continuation_ignores_earlier_call_opener() {
    let source = "void f(void)\n{\n    switch (t)\n    {\n    case A:\n        helper(arg1,\n               arg2);\n        break;\n    case B:\n        *result_type =\n            array_union(CAST(c1),\n                        CAST(c2), &result)\n            ? BITSET_TYPE\n            : ARRAY_TYPE;\n        return result;\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn nested_ternary_in_case_body_grows_per_level_despite_case_unindent() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n    switch (x)\n    {\n    case A:\n      {\n        char buf[256];\n        helper (mode, (state = compute (msg),\n                       call (\" %s %s\",\n                             render (state->flags),\n                             (state->next == ALPHA ? \"ALPHA\" :\n                              (state->next == BETA ? \"BETA\" :\n                               (state->next == GAMMA ? \"GAMMA\" :\n                                (state->next == DELTA ? \"DELTA\" :\n                                 (sprintf (buf, \"%p\", state->next),\n                                  buf))))),\n                             state->cx)));\n      }\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    switch (x)\n    {\n    case A:\n    {\n        char buf[256];\n        helper (mode, (state = compute (msg),\n                       call (\" %s %s\",\n                             render (state->flags),\n                             (state->next == ALPHA ? \"ALPHA\" :\n                              (state->next == BETA ? \"BETA\" :\n                               (state->next == GAMMA ? \"GAMMA\" :\n                                (state->next == DELTA ? \"DELTA\" :\n                                 (sprintf (buf, \"%p\", state->next),\n                                  buf))))),\n                             state->cx)));\n    }\n    }\n}\n",
    );
}

#[test]
fn vtk_switch_aligns_case_labels_with_indented_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int n)\n{\nswitch(n) {\ncase 0:\na();\nbreak;\ndefault:\nb();\n}\n}\n",
            &options,
        ),
        "void f(int n)\n{\n    switch(n)\n        {\n        case 0:\n            a();\n            break;\n        default:\n            b();\n        }\n}\n",
    );
}

#[test]
fn ratliff_switch_aligns_case_labels_with_indented_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int n)\n{\nswitch(n) {\ncase 0:\na();\nbreak;\ndefault:\nb();\n}\n}\n",
            &options,
        ),
        "void f(int n) {\n    switch(n) {\n        case 0:\n            a();\n            break;\n        default:\n            b();\n        }\n    }\n",
    );
}

#[test]
fn pico_runs_in_switch_brace_and_indents_case_labels() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(int n)\n{\nswitch(n)\n{\ncase 0:\na();\nbreak;\ndefault:\nb();\n}\n}\n",
            &options,
        ),
        "void f(int n)\n{   switch(n)\n    {   case 0:\n            a();\n            break;\n        default:\n            b(); } }\n",
    );
}

#[test]
fn one_line_switch_options_keep_structural_parent_indent() {
    let source = "void f(){\nswitch(value){case 1: one(); break;}\n}\n";
    let expected =
        "void f() {\n    switch(value) {\n    case 1:\n        one();\n        break;\n    }\n}\n";

    for argument in ["--keep-one-line-blocks", "--add-one-line-braces"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[argument.to_owned()]).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn statement_keep_does_not_attach_a_following_switch_label() {
    let mut options = FormatOptions::default();
    options.break_one_line_statements = false;

    assert_eq!(
        format_c(
            "void run()\n{\n    switch (value) { case 1: one(); break; default: two(); }\n}\n",
            &options,
        ),
        "void run()\n{\n    switch (value) {\n    case 1: one();\n        break;\n    default: two();\n    }\n}\n",
    );
}

#[test]
fn whitesmith_kept_one_line_switch_preserves_case_sequence() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=whitesmith",
        "--keep-one-line-blocks",
        "--keep-one-line-statements",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void run()\n{\n    switch (value) { case 1: one(); break; default: two(); }\n}\n",
            &options,
        ),
        "void run()\n    {\n    switch (value) { case 1: one(); break; default: two(); }\n    }\n",
    );
}

#[test]
fn whitesmith_switch_header_break_preserves_kept_case_actions() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=whitesmith",
        "--keep-one-line-blocks",
        "--keep-one-line-statements",
        "--break-one-line-headers",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "void run()\n{\n    switch (value) { case 1: one(); break; default: two(); }\n}\n",
            &options,
        ),
        "void run()\n    {\n    switch (value)\n        {\n        case 1: one(); break; default: two();\n        }\n    }\n",
    );
}

#[test]
fn ratliff_indent_switches_does_not_double_indent_cases() {
    let source = "int f(int x)\n{\nswitch(x) {\ncase 0:\nreturn 1;\ndefault:\nreturn 2;\n}\n}\n";
    let expected = "int f(int x) {\n    switch(x) {\n        case 0:\n            return 1;\n        default:\n            return 2;\n        }\n    }\n";
    let cases: &[&[&str]] = &[
        &["--style=ratliff", "--indent-switches"],
        &["--style=ratliff", "--indent-switches", "--indent-cases"],
    ];

    for arguments in cases {
        let mut options = FormatOptions::default();
        let arguments: Vec<_> = arguments.iter().map(|value| (*value).to_owned()).collect();
        apply_command_line_args(&mut options, &arguments).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn vtk_indent_switches_does_not_double_indent_cases() {
    let mut options = FormatOptions::default();
    let args = ["--style=vtk", "--indent-switches"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "int f(int x)\n{\nswitch(x) {\ncase 0:\nreturn 1;\ndefault:\nreturn 2;\n}\n}\n",
            &options,
        ),
        "int f(int x)\n{\n    switch(x)\n        {\n        case 0:\n            return 1;\n        default:\n            return 2;\n        }\n}\n",
    );
}

#[test]
fn nested_switch_first_case_label_keeps_switch_level_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f()\n{\nswitch(x)\n{\ncase 1:\ng();\nbreak;\ndefault:\nh();\n}\n}\n",
            &options,
        ),
        "void f()\n{\n    switch(x)\n    {\n    case 1:\n        g();\n        break;\n    default:\n        h();\n    }\n}\n",
    );
}

#[test]
fn block_comment_after_case_label_keeps_the_case_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 1:\n/* body */\ncall();\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n    {\n    case 1:\n        /* body */\n        call();\n        break;\n    }\n}\n",
    );
}

#[test]
fn whitesmith_break_after_nested_switch_returns_to_the_outer_case_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int alpha,int beta){\nswitch(alpha){\ncase 1:\nswitch(beta){\ncase 2:\ncall();\nbreak;\ndefault:\nbreak;\n}\nbreak;\ndefault:\nbreak;\n}\n}\n",
            &options,
        ),
        "void run(int alpha,int beta)\n    {\n    switch(alpha)\n        {\n        case 1:\n            switch(beta)\n                {\n                case 2:\n                    call();\n                    break;\n                default:\n                    break;\n                }\n            break;\n        default:\n            break;\n        }\n    }\n",
    );
}

#[test]
fn ratliff_break_after_nested_control_returns_to_the_case_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nif(ready){\nwhile(next()){\nswitch(value){\ncase 1:\nif(active){\ncall();\n}\nbreak;\ndefault:\nbreak;\n}\n}\n}\n}\n",
            &options,
        ),
        "void run(int value) {\n    if(ready) {\n        while(next()) {\n            switch(value) {\n                case 1:\n                    if(active) {\n                        call();\n                        }\n                    break;\n                default:\n                    break;\n                }\n            }\n        }\n    }\n",
    );
}

#[test]
fn horstmann_nested_case_label_keeps_label_and_body_at_distinct_columns() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){\nswitch(value){\ncase 3:{\ncase 4:\ncall();\nbreak;\n}\n}\n}\n",
            &options,
        ),
        "void run(int value)\n{   switch(value)\n    {   case 3:\n        {   case 4:\n                call();\n                break;\n            }\n    }\n}\n",
    );
}

#[test]
fn vtk_switch_non_case_body_uses_switch_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){switch(value){\ncall();\n}}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n        {\n            call();\n        }\n}\n",
    );
}

#[test]
fn vtk_switch_preprocessor_opener_uses_conditional_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=vtk".to_owned(), "--indent-preproc-cond".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void run(int value){switch(value){\n#if A\ncase 1:break;\n#endif\n}}\n",
            &options,
        ),
        "void run(int value)\n{\n    switch(value)\n        {\n            #if A\n        case 1:\n            break;\n            #endif\n        }\n}\n",
    );
}
