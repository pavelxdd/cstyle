#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::format_c;
use cstyle::config::{BraceStyle, FormatOptions, IndentStyle, apply_command_line_args};

#[test]
fn allman_lambda_body_breaks_before_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void run(){auto transform=[](int value){return value;};}"),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    auto transform=[](int value)",
            "    {",
            "        return value;",
            "    };",
            "}",
        )
    );
}

#[test]
fn allman_lambda_parameter_continuation_aligns_after_open_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(){",
                "auto value=[](int alpha,",
                "int beta){",
                "return alpha+beta;",
                "};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    auto value=[](int alpha,",
            "                  int beta)",
            "    {",
            "        return alpha+beta;",
            "    };",
            "}",
        )
    );
}

#[test]
fn pico_lambda_body_keeps_one_line_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void run(){auto transform=[](int value){return value;};}"),
            &options,
        ),
        fixture!("void run() {auto transform=[](int value) {return value;};}")
    );
}

#[test]
fn vtk_statement_after_lambda_returns_to_the_enclosing_body_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!(
            "void run(){",
            "auto value=[](){",
            "call();",
            "};",
            "use(value);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void run()",
            "{",
            "    auto value=[]()",
            "        {",
            "        call();",
            "        };",
            "    use(value);",
            "}",
        )
    );
}

#[test]
fn vtk_attached_lambda_brace_uses_lambda_body_column_idempotently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "void run()",
        "{",
        "    auto value=[](int alpha,",
        "                  int beta){",
        "        return alpha+beta;",
        "    };",
        "}",
    );
    let expected = fixture!(
        "void run()",
        "{",
        "    auto value=[](int alpha,",
        "                  int beta)",
        "        {",
        "        return alpha+beta;",
        "        };",
        "}",
    );

    let formatted = format_c(source, &options);
    assert_eq!(formatted, expected);
    assert_eq!(format_c(&formatted, &options), formatted);
}

#[test]
fn lambda_trailing_return_type_brace_attaches_in_one_true_brace() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto x = [this]() -> bool",
        "    {",
        "        return g();",
        "    };",
        "}",
    );

    assert_eq!(
        format_c(source, &one_true_brace_options()),
        fixture!(
            "void f()",
            "{",
            "    auto x = [this]() -> bool {",
            "        return g();",
            "    };",
            "}",
        )
    );
}

// Attaching brace style applies inside call-argument lambda bodies.
#[test]
fn control_brace_stays_attached_inside_lambda_call_argument_body() {
    let source = fixture!(
        "void f()",
        "{",
        "    obj.connect(&timer, [&] {",
        "        while (consumed < n) {",
        "            step();",
        "        }",
        "    });",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_options()), source);
}

#[test]
fn kr_lambda_nested_control_braces_attach_and_cuddle_else() {
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

    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "\tauto gp = [this](const char* name) -> std::string {",
                "\t\tif(value != nullptr)",
                "\t\t{",
                "\t\t\treturn a;",
                "\t\t} else",
                "\t\t{",
                "\t\t\treturn b;",
                "\t\t}",
                "\t};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tauto gp = [this](const char* name) -> std::string {",
            "\t\tif(value != nullptr) {",
            "\t\t\treturn a;",
            "\t\t} else {",
            "\t\t\treturn b;",
            "\t\t}",
            "\t};",
            "}",
        )
    );
}

#[test]
fn top_level_lambda_assignment_breaks_brace_like_definition_kr() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.attach_struct = true;
    options.attach_enum = true;

    assert_eq!(
        format_c(
            fixture!("auto lambda = [](int x){ return x*2; };", "int after = 1;",),
            &options,
        ),
        fixture!(
            "auto lambda = [](int x)",
            "{",
            "    return x*2;",
            "};",
            "int after = 1;",
        )
    );
}

#[test]
fn lambda_call_argument_after_split_arg_uses_call_line_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "int f()",
                "{",
                "    invoke(",
                "        first,",
                "        [value = value()] {",
                "            call(value);",
                "        },",
                "        queued);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "int f()",
            "{",
            "    invoke(",
            "        first,",
            "    [value = value()] {",
            "        call(value);",
            "    },",
            "    queued);",
            "}",
        )
    );
}

#[test]
fn get_if_argument_aligns_to_value_expression_in_lambda_body() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    callback( //",
                "            [&value](const Request &request) {",
                "                const auto *result = std::get_if<Result>(",
                "                        &request.result);",
                "            });",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    callback( //",
            "    [&value](const Request &request) {",
            "        const auto *result = std::get_if<Result>(",
            "                                 &request.result);",
            "    });",
            "}",
        )
    );
}

#[test]
fn lambda_in_braced_initializer_preserves_source_brace_gap() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    auto value = MaybeThread{[&]{",
                "        work();",
                "    }};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    auto value = MaybeThread{[&]{",
            "            work();",
            "        }};",
            "}",
        )
    );
}

#[test]
fn parameterized_lambda_in_braced_initializer_uses_lambda_block_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    Handler observer {[&]() {",
        "        ++count;",
        "    }};",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn one_line_parameterized_lambda_in_braced_initializer_breaks_body() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Handler observer {[&](){ ++count; }};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Handler observer {[&]() {",
            "        ++count;",
            "    }};",
            "}",
        )
    );
}

#[test]
fn top_level_lambda_assignment_does_not_indent_brace_vtk() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;

    assert_eq!(
        format_c(
            fixture!("auto lambda = [](int x){ return x*2; };", "int after = 1;",),
            &options,
        ),
        fixture!(
            "auto lambda = [](int x)",
            "{",
            "    return x*2;",
            "};",
            "int after = 1;",
        )
    );
}

#[test]
fn top_level_lambda_assignment_returns_to_outer_indent_whitesmith() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;

    assert_eq!(
        format_c(
            fixture!("auto lambda = [](int x){ return x*2; };", "int after = 1;",),
            &options,
        ),
        fixture!(
            "auto lambda = [](int x)",
            "    {",
            "    return x*2;",
            "    };",
            "int after = 1;",
        )
    );
}

fn one_true_brace_options() -> FormatOptions {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options
}

#[test]
fn formats_basic_lambda_body_under_existing_style_options() {
    let actual = format_c(
        fixture!(
            "void f(){",
            "    auto value=[](){",
            "        return 1;",
            "    };",
            "}"
        ),
        &one_true_brace_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    auto value=[]() {",
            "        return 1;",
            "    };",
            "}"
        )
    );
}

#[test]
fn pads_lambda_capture_assignment_under_pad_oper() {
    let mut options = one_true_brace_options();
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "void f(){",
            "    int base=1;",
            "    auto value=[&total, scale=2](int item) mutable {",
            "        total += item*scale;",
            "        return item>base;",
            "    };",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    int base = 1;",
            "    auto value = [&total, scale = 2](int item) mutable {",
            "        total += item * scale;",
            "        return item > base;",
            "    };",
            "}"
        )
    );
}

#[test]
fn keeps_lambda_trailing_return_arrow_spacing_under_existing_options() {
    let actual = format_c(
        fixture!(
            "void f(){",
            "    auto ready=[](int value)->bool {",
            "        return value > 0;",
            "    };",
            "}"
        ),
        &one_true_brace_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    auto ready=[](int value)->bool {",
            "        return value > 0;",
            "    };",
            "}"
        )
    );
}

#[test]
fn formats_lambda_call_argument_body_without_extra_indent_option() {
    let actual = format_c(
        fixture!(
            "void f(){",
            "    call(alpha, [](int value) {",
            "        return value + 1;",
            "    });",
            "}"
        ),
        &one_true_brace_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    call(alpha, [](int value) {",
            "        return value + 1;",
            "    });",
            "}"
        )
    );
}

#[test]
fn lambda_split_call_after_closed_ternary_argument_aligns_to_call() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto const show = [&](const char* what, bool dark)",
        "    {",
        "        io.emitText(Buffer::format(\"%s\", dark ? \"dark\" : \"light\"),",
        "                    x, y);",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn lambda_parameter_continuation_keeps_lambda_line_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    const auto call = [](const char* name,",
                "        const Value& value) {",
                "        return value;",
                "    };",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    const auto call = [](const char* name,",
            "    const Value& value) {",
            "        return value;",
            "    };",
            "}",
        )
    );
}

#[test]
fn lambda_parameter_continuation_preserves_open_paren_alignment() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto row = [](const String &format, Value val, const String &expected,",
        "                  const Locale &loc = Locale::classic())",
        "    {",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_lambda_brace_does_not_inherit_parameter_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "void run()",
        "{",
        "    auto value=[](int alpha,",
        "                  int beta)",
        "    {",
        "        return alpha+beta;",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn attached_lambda_parameter_indent_after_parens_keeps_lambda_line_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){",
        "auto value=[](int alpha,",
        "int beta){",
        "return alpha+beta;",
        "};",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run() {",
            "    auto value=[](int alpha,",
            "    int beta) {",
            "        return alpha+beta;",
            "    };",
            "}",
        )
    );
}

#[test]
fn attached_lambda_parameter_comment_keeps_lambda_owners() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let source = fixture!(
        "void run(){",
        "auto value=[](int alpha,",
        "/* parameter */ int beta){",
        "return alpha+",
        "beta;",
        "};",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run() {",
            "    auto value=[](int alpha,",
            "    /* parameter */ int beta) {",
            "        return alpha+",
            "               beta;",
            "    };",
            "}",
        )
    );
}

#[test]
fn split_lambda_parameter_alignment_uses_visual_tab_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    let source = fixture!(
        "void run(){",
        "auto value=[](int alpha,",
        "int beta){",
        "return alpha+beta;",
        "};",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "\tauto value=[](int alpha,",
            "\t              int beta)",
            "\t{",
            "\t\treturn alpha+beta;",
            "\t};",
            "}",
        )
    );
}

#[test]
fn split_lambda_parameter_comment_uses_semantic_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "void run(){",
        "auto value=[](int alpha,",
        "/* parameter */ int beta){",
        "return alpha+",
        "beta;",
        "};",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    auto value=[](int alpha,",
            "                  /* parameter */ int beta)",
            "    {",
            "        return alpha+",
            "               beta;",
            "    };",
            "}",
        )
    );
}

#[test]
fn whitesmith_split_lambda_uses_semantic_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let source = fixture!(
        "void run(){",
        "auto value=[](int alpha,",
        "int beta){",
        "return alpha+beta;",
        "};",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "    {",
            "    auto value=[](int alpha,",
            "                  int beta)",
            "        {",
            "        return alpha+beta;",
            "        };",
            "    }",
        )
    );
}

#[test]
fn immediately_invoked_multiline_lambda_call_breaks_after_body() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    call([](int x) {",
                "        return x;",
                "    }(value));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    call([](int x) {",
            "        return x;",
            "    }",
            "    (value));",
            "}",
        )
    );
}

#[test]
fn multiline_return_lambda_invocation_splits_after_body() {
    assert_eq!(
        format_c(
            "int f(unsigned n) {\n  return [](unsigned m) {\n    int value = 0;\n    do {\n      ++value;\n    } while ((m >>= 4) != 0);\n    return value;\n  }(n);\n}\n",
            &FormatOptions::default(),
        ),
        "int f(unsigned n) {\n    return [](unsigned m) {\n        int value = 0;\n        do {\n            ++value;\n        } while ((m >>= 4) != 0);\n        return value;\n    }\n    (n);\n}\n",
    );
}

#[test]
fn call_argument_after_multiline_lambda_body_uses_statement_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    CHECK_EQ(run([](int a, double b) { return a + b; }, 12, 15).result(),",
                "             double(12+15));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    CHECK_EQ(run([](int a, double b) {",
            "        return a + b;",
            "    }, 12, 15).result(),",
            "    double(12+15));",
            "}",
        )
    );
}

#[test]
fn lambda_body_brace_gets_space_after_capture_list() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Check::append(\"row\") << Factory([]{",
                "        call();",
                "    });",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Check::append(\"row\") << Factory([] {",
            "        call();",
            "    });",
            "}",
        )
    );
}

#[test]
fn no_param_lambda_call_argument_body_stays_one_line() {
    let source = fixture!(
        "void f()",
        "{",
        "    Scheduler::runAfter(200, &context, [] { ::exit(EXIT_SUCCESS); });",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn empty_param_lambda_call_argument_body_stays_one_line() {
    let source = fixture!(
        "void f()",
        "{",
        "    observe(&object, &Object::signal, [=]() {});",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn no_param_multiline_lambda_empty_invocation_breaks_after_body() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    call([]() {",
                "        return 1;",
                "    }());",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    call([]() {",
            "        return 1;",
            "    }",
            "    ());",
            "}",
        )
    );
}

#[test]
fn trailing_return_multiline_lambda_empty_invocation_stays_attached() {
    let source = "void f()\n{\n    auto value = []() -> int {\n        return 1;\n    }();\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn empty_param_immediately_invoked_lambda_stays_one_line() {
    let source = "void f()\n{\n    auto value = ([]() {}());\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn lambda_argument_after_comment_keeps_statement_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    observe(source, notice, source,\n            // comment one\n            // comment two\n            [value](int x) {\n                call(x);\n            });\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    observe(source, notice, source,\n            // comment one\n            // comment two\n    [value](int x) {\n        call(x);\n    });\n}\n",
    );
}

#[test]
fn prefixed_lambda_argument_after_comment_keeps_statement_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    observe(a, b,\n            // comment:\n            c, []() { call(); });\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    observe(a, b,\n            // comment:\n    c, []() {\n        call();\n    });\n}\n",
    );
}

#[test]
fn wrapped_lambda_argument_one_line_body_breaks_by_default() {
    assert_eq!(
        format_c(
            "void f()\n{\n    const Iterator start =\n        stable_partition(all.begin(), end, [parentName](const Item &item)\n                         { return !item.parent().contains(parentName); });\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    const Iterator start =\n        stable_partition(all.begin(), end, [parentName](const Item &item)\n    {\n        return !item.parent().contains(parentName);\n    });\n}\n",
    );
}

#[test]
fn lambda_call_argument_after_comma_uses_inner_call_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto report = onScopeExit([inputScale, progressValue, elapsedTime]() {",
        "        qDebug(\"format\",",
        "               inputScale, progressValue, elapsedTime);",
        "    });",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// Sibling lambdas share the call-argument column.
#[test]
fn lambda_call_arguments_keep_consistent_paren_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    Call(\n        [](int a,\n            int b)\n    {\n        return a;\n    },\n    [](int c,\n        int d)\n    {\n        return c;\n    });\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Call(\n        [](int a,\n           int b)\n    {\n        return a;\n    },\n        [](int c,\n           int d)\n    {\n        return c;\n    });\n}\n",
    );
}

// Call context does not change lambda-body ownership inside a condition.
#[test]
fn lambda_body_in_if_condition_indents_consistently() {
    assert_eq!(
        format_c(
            "void f()\n{\n    if ( !Wait(point, \"msg\", [&]() {\n            return c != 0;\n        }) )\n        return;\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    if ( !Wait(point, \"msg\", [&]() {\n        return c != 0;\n    }) )\n    return;\n}\n",
    );
}

#[test]
fn spaced_lambda_parameter_list_opens_lambda_body_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    {",
                "        STATIC_INIT static auto lambda = [] (int value) { ++value; return value; };",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    {",
            "        STATIC_INIT static auto lambda = [] (int value) {",
            "            ++value;",
            "            return value;",
            "        };",
            "    }",
            "}",
        )
    );
}

#[test]
fn fluent_chain_after_multiline_lambda_keeps_source_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    auto result = task([](int value){ return value; })",
                "                      .withArguments(1)",
                "                      .spawn()",
                "                      .result();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    auto result = task([](int value) {",
            "        return value;",
            "    })",
            "    .withArguments(1)",
            "    .spawn()",
            "    .result();",
            "}",
        )
    );
}

#[test]
fn lambda_stream_chain_keeps_source_aligned_following_operator() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto add = [](const ValueArray &tag, const char *data) {",
        "        Check::append(tag) << ValueArray(data, (tag.size() + 7) / 8) << tag.size()",
        "                           << convertValueToBits(tag);",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_after_split_lambda_header_uses_two_indent_levels() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto row = [](const String &format, Value val, const String &expected,",
        "                  const Locale &loc = Locale::classic())",
        "    {",
        "        Check::append(\"%s:%s\", loc.name().c_str(), qPrintable(format))",
        "                << format << val << loc << expected;",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn lambda_return_ternary_with_braced_true_arm_aligns_to_return_value() {
    assert_eq!(
        format_c(
            "void f() {\n  return apply_result<char>(\n      dst, rules, count, content_count, [=](generic_iterator<ResultIt> at) {\n        return is_ready\n                   ? create_generic_value(bounded_result_iterator{at, count}, v)\n                         .stored_iterator_ref\n                   : copy<char>(v.data(), v.data() + count, at);\n      });\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    return apply_result<char>(\n    dst, rules, count, content_count, [=](generic_iterator<ResultIt> at) {\n        return is_ready\n               ? create_generic_value(bounded_result_iterator{at, count}, v)\n               .stored_iterator_ref\n               :\n               copy<char>(v.data(), v.data() + count, at);\n    });\n}\n",
    );
}

#[test]
fn kr_split_trailing_return_lambda_keeps_one_line_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source = "void f()\n{\n\tauto g = [](int n)->bool\n\t{ return n; };\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_trailing_return_lambda_keeps_one_line_body_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(){auto g = [](int n)->bool { return n; };}\n",
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    auto g = [](int n)->bool { return n; };",
            "}",
        )
    );
}

#[test]
fn default_breaks_one_line_lambda_assignment_without_trailing_return() {
    assert_eq!(
        format_c(
            "auto value = [] { return 1; };\n",
            &FormatOptions::default(),
        ),
        fixture!("auto value = [] {", "    return 1;", "};")
    );
}

#[test]
fn default_breaks_capture_only_lambda_inside_auto_initializer_call() {
    assert_eq!(
        format_c(
            "auto value = call([] { return 1; });\n",
            &FormatOptions::default(),
        ),
        fixture!("auto value = call([] {", "    return 1;", "});")
    );
}

#[test]
fn default_breaks_capture_only_lambda_after_split_auto_declaration() {
    assert_eq!(
        format_c(
            "auto\nvalue = [] { return 1; };\n",
            &FormatOptions::default(),
        ),
        fixture!("auto", "value = [] {", "    return 1;", "};")
    );
}

#[test]
fn default_keeps_capture_only_lambda_with_explicit_variable_type() {
    let source = "Callback value = [] { return 1; };\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn default_keeps_ternary_colon_with_closed_auto_lambda_arm() {
    assert_eq!(
        format_c(
            "auto value = condition ? [] { return 1; } : [] { return 2; };\n",
            &FormatOptions::default(),
        ),
        fixture!(
            "auto value = condition ? [] {",
            "    return 1;",
            "} :",
            "[] { return 2; };",
        )
    );
}

#[test]
fn allman_keep_one_line_lambda_keeps_body_attached() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--keep-one-line-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "auto g = [](int n) { return n; };\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn wrapped_lambda_argument_opening_block_keeps_statement_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source =
        "void helper()\n{\n\teach(count,\n\t[&buf, &n](Item &it) {\n\t\tdo_work();\n\t});\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn lambda_braceless_if_body_keeps_source_line_break() {
    let source = "void f() {\n    Bind([this]() {\n        if ( cond )\n            Refresh();\n    });\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn parameterized_lambda_with_only_empty_statement_stays_one_line() {
    let source = "void f()\n{\n    request.setResultHandler([](ValueSpace::Arg) { ; });\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_lambda_closing_with_trailing_argument_returns_to_lambda_header_indent() {
    let source = "void f()\n{\n    TimerX::scheduleAt(1000ms, [&]() {\n        TimerX::scheduleAt(1000ms, [&]() {\n            call();\n        }, RT::DeferredDispatch);\n        runState.processInputs();\n    }, RT::DeferredDispatch);\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_if_header_block_close_brace_in_lambda_aligns_to_if_start() {
    let source = "void f()\n{\n    g([](int x) {\n        if((a == b)\n                && (c < d)) {\n            e = f;\n            g++;\n        }\n    });\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn lambda_body_inside_braced_initializer_call_uses_block_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tauto item{find_if(items.begin(), items.end(),\n\t                  [value](Item& item)\n\t{\n\t\treturn value == &item;\n\t})};\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    auto item{find_if(items.begin(), items.end(),\n                      [value](Item& item)\n    {\n        return value == &item;\n    })};\n}\n",
    );
}

#[test]
fn vtk_in_function_lambda_does_not_leak_indent_to_function_close() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void g(){ auto f=[](int y){return y;}; }\n", &options,),
        "void g()\n{\n    auto f=[](int y)\n        {\n        return y;\n        };\n}\n",
    );
}

#[test]
fn pico_preserves_source_space_before_one_line_lambda_close_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("auto lambda = [](int x){ return x*2; };\n", &options),
        "auto lambda = [](int x) { return x*2; };\n",
    );
}

#[test]
fn kr_breaks_assigned_trailing_return_lambda_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("auto f = [](int a) -> int { return a; };\n", &options,),
        "auto f = [](int a) -> int\n{\n    return a;\n};\n",
    );
}

#[test]
fn kr_keeps_argument_trailing_return_lambda_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let source = "call([](int a) -> int { return a; });\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_breaks_assigned_trailing_return_lambda_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("auto f = [](int a) -> int { return a; };\n", &options,),
        "auto f = [](int a) -> int\n{\n    return a;\n};\n",
    );
}

#[test]
fn horstmann_breaks_assigned_trailing_return_lambda_body() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("auto f = [](int a) -> int { return a; };\n", &options,),
        "auto f = [](int a) -> int\n{   return a;\n};\n",
    );
}

#[test]
fn inline_trailing_return_lambda_preserves_brace_spacing() {
    let source = "call([](int a)->int{ return a; });\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn inline_trailing_return_lambda_keeps_source_brace_space() {
    let source = "call([](int a)->int { return a; });\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assigned_trailing_return_lambda_preserves_brace_spacing() {
    assert_eq!(
        format_c(
            "int main(){ auto f=[](int y)->int{ return y; }; }\n",
            &FormatOptions::default(),
        ),
        "int main() {\n    auto f=[](int y)->int{ return y; };\n}\n",
    );
}

#[test]
fn pico_inline_trailing_return_lambda_preserves_brace_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int main(){ auto f=[x=1](int y)->int{ return x+y; }; }\n",
            &options,
        ),
        "int main() { auto f=[x=1](int y)->int{ return x+y; }; }\n",
    );
}
