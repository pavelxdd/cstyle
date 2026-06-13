#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{
    BraceStyle, FormatOptions, IndentStyle, MinConditionalIndent, PointerAlign,
    apply_command_line_args,
};

#[test]
fn keeps_class_initializer_braced_values_attached() {
    let actual = format(fixture!(
        "class Item{Item(): value{1}, other(2) { call(); }};"
    ));

    assert_eq!(
        actual,
        fixture!(
            "class Item",
            "{",
            "    Item(): value{1}, other(2)",
            "    {",
            "        call();",
            "    }",
            "};",
        )
    );
}

#[test]
fn leading_comma_constructor_initializer_keeps_member_indent() {
    let source = fixture!(
        "class C",
        "{",
        "    C() : a(x)",
        "        , b(y)",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_member_after_multiline_signature_ignores_parameter_column() {
    let source = fixture!(
        "class RequestHandler : public Service",
        "{",
        "public:",
        "    RequestHandler(const Path &sourcePath,",
        "                   const TextBox &destinationPath,",
        "                   FileChangeObserver *tracker,",
        "                   Service *parent = nullptr)",
        "        : Service(parent),",
        "          added(false),",
        "          sourcePath(sourcePath),",
        "          tracker(tracker)",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_member_after_multiline_base_call_returns_to_member_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "class C",
                "{",
                "    C() : Base(a,",
                "               b),",
                "          m(x)",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class C",
            "{",
            "    C() : Base(a,",
            "                   b),",
            "        m(x)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn constructor_direct_list_rows_align_after_the_opening_brace() {
    assert_eq!(
        format_c(
            fixture!(
                "Item::Item()",
                ": first(call(alpha,",
                "beta))",
                ", second{one,",
                "two}",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "Item::Item()",
            "    : first(call(alpha,",
            "                 beta))",
            "    , second{one,",
            "             two}",
            "{",
            "}",
        )
    );
}

#[test]
fn tab_indent_keeps_wrapped_constructor_initializer_rows_two_levels_deep() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "class C {",
                "\tC(int v)",
                "\t\t: a(v)",
                "\t\t, b(v)",
                "\t{",
                "\t\tcall();",
                "\t}",
                "};",
            ),
            &options,
        ),
        fixture!(
            "class C",
            "{",
            "\tC(int v)",
            "\t\t: a(v)",
            "\t\t, b(v)",
            "\t{",
            "\t\tcall();",
            "\t}",
            "};",
        )
    );
}

#[test]
fn kr_constructor_body_brace_breaks_after_braced_initializer_members() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "class C {",
                "\tC(int v)",
                "\t\t: a{v}",
                "\t\t, b{v} {",
                "\t\tcall();",
                "\t}",
                "};",
            ),
            &options,
        ),
        fixture!(
            "class C",
            "{",
            "\tC(int v)",
            "\t\t: a{v}",
            "\t\t, b{v}",
            "\t{",
            "\t\tcall();",
            "\t}",
            "};",
        )
    );
}

#[test]
fn out_of_class_constructor_initializer_rows_keep_indent_and_body_brace() {
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
    let source = fixture!(
        "Config::Config(int id)",
        "\t: alpha{id}",
        "\t, beta{id}",
        "\t, gamma{id}",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn tab_indented_constructor_arguments_align_after_the_call_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent=tab=4".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("Item::Item()", ": value(call(alpha,", "beta))", "{", "}",),
            &options,
        ),
        fixture!(
            "Item::Item()",
            "\t: value(call(alpha,",
            "\t             beta))",
            "{",
            "}",
        )
    );
}

#[test]
fn member_init_paren_continuation_aligns_past_open_paren() {
    assert_eq!(
        format_c(
            "class Foo : public Bar {\npublic:\n    Foo(Config &config, int x)\n        : Bar(config, x,\nalpha, beta) {\n    }\n};\n",
            &FormatOptions::default(),
        ),
        "class Foo : public Bar {\npublic:\n    Foo(Config &config, int x)\n        : Bar(config, x,\n              alpha, beta) {\n    }\n};\n",
    );
}

#[test]
fn later_tab_indent_replaces_force_tab_x_for_constructor_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--indent=force-tab-x=2".to_owned(),
            "--indent=tab=6".to_owned(),
        ],
    )
    .expect("valid indentation options");

    assert_eq!(
        format_c(
            fixture!(
                "Item::Item()",
                ": value{alpha,",
                "beta},",
                "other(call(gamma,",
                "delta))",
                "{",
                "}",
            ),
            &options,
        ),
        fixture!(
            "Item::Item()",
            "\t: value{alpha,",
            "\t        beta},",
            "\t        other(call(gamma,",
            "\t                   delta))",
            "{",
            "}",
        ),
    );
}

#[test]
fn split_constructor_initializer_call_indents_under_initializer() {
    let source = fixture!(
        "class C",
        "{",
        "public:",
        "    C() : Base",
        "        (",
        "            alpha,",
        "            beta",
        "        )",
        "    {",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_arguments_after_trailing_comment_indent_one_more_level() {
    assert_eq!(
        format_c(
            fixture!(
                "class C",
                "{",
                "    C() : Base(a, // first",
                "               b, // second",
                "               c) // third",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class C",
            "{",
            "    C() : Base(a, // first",
            "                   b, // second",
            "                   c) // third",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn constructor_initializer_empty_call_arguments_indent_two_levels() {
    assert_eq!(
        format_c(
            fixture!(
                "class C",
                "{",
                "public:",
                "    C() : Base(",
                "        a,",
                "        b)",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class C",
            "{",
            "public:",
            "    C() : Base(",
            "            a,",
            "            b)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn preprocessor_member_initializer_comma_rows_keep_constructor_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "    Item(const Item& other)",
        "        : base(other)",
        "#if WITH_POSITIONS",
        "        , start_position(other.start_position)",
        "        , end_position(other.end_position)",
        "#endif",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preprocessor_member_initializer_list_keeps_constructor_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "    Item(const Value& value)",
        "#if WITH_POSITIONS",
        "        : start_position(value.start()),",
        "          end_position(value.end())",
        "#endif",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preprocessor_constructor_keeps_following_class_members_at_class_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "class Item",
                "    : public Base",
                "{",
                "    META_TAG",
                "",
                "    static constexpr int blockSize = 1 << 12;",
                "#if defined(OS_A)",
                "    static constexpr int maxBits = 24;",
                "#elif defined(OS_B)",
                "    static constexpr int maxBits = 28;",
                "#elif defined(OS_C)",
                "    static constexpr int maxBits = 28;",
                "#elif defined (OS_D)",
                "    static constexpr int maxBits = 28;",
                "#elif defined(SUPPORT)",
                "    static constexpr int maxBits = 36;",
                "#  define MUST_SET_MAX_SIZE_BITS",
                "#else",
                "    static constexpr int maxBits = 24;",
                "#endif",
                "",
                "public:",
                "    Item()",
                "#ifdef MUST_SET_MAX_SIZE_BITS",
                "        // comment",
                "        : maxBits(value() ? 28 : maxBits)",
                "#endif",
                "    {",
                "    }",
                "",
                "private:",
                "    void method();",
                "    Type const &data(int index);",
                "",
                "private slots:",
                "    void init();",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class Item",
            "    : public Base",
            "{",
            "    META_TAG",
            "",
            "    static constexpr int blockSize = 1 << 12;",
            "#if defined(OS_A)",
            "    static constexpr int maxBits = 24;",
            "#elif defined(OS_B)",
            "    static constexpr int maxBits = 28;",
            "#elif defined(OS_C)",
            "    static constexpr int maxBits = 28;",
            "#elif defined (OS_D)",
            "    static constexpr int maxBits = 28;",
            "#elif defined(SUPPORT)",
            "    static constexpr int maxBits = 36;",
            "#  define MUST_SET_MAX_SIZE_BITS",
            "#else",
            "    static constexpr int maxBits = 24;",
            "#endif",
            "",
            "public:",
            "    Item()",
            "#ifdef MUST_SET_MAX_SIZE_BITS",
            "    // comment",
            "        : maxBits(value() ? 28 : maxBits)",
            "#endif",
            "    {",
            "    }",
            "",
            "private:",
            "    void method();",
            "    Type const &data(int index);",
            "",
            "private slots:",
            "    void init();",
            "};",
        )
    );
}

#[test]
fn constructor_initializer_preprocessor_branch_keeps_member_indent_after_many_members() {
    let actual = format_c(
        fixture!(
            "class Event",
            "{",
            "public:",
            "    Event(const Event& event)",
            "        : Base(event),",
            "        item(event.item),",
            "        column(event.column),",
            "        model(event.model),",
            "        value(event.value),",
            "        position(event.position),",
            "        first(event.first),",
            "        second(event.second),",
            "        third(event.third),",
            "        done(event.done)",
            "#if USE_EXTRA",
            "        , data(event.data),",
            "        format(event.format),",
            "        buffer(event.buffer),",
            "        size(event.size),",
            "        flags(event.flags),",
            "        effect(event.effect),",
            "        index(event.index)",
            "#endif",
            "        { }",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class Event",
            "{",
            "public:",
            "    Event(const Event& event)",
            "        : Base(event),",
            "          item(event.item),",
            "          column(event.column),",
            "          model(event.model),",
            "          value(event.value),",
            "          position(event.position),",
            "          first(event.first),",
            "          second(event.second),",
            "          third(event.third),",
            "          done(event.done)",
            "#if USE_EXTRA",
            "        , data(event.data),",
            "          format(event.format),",
            "          buffer(event.buffer),",
            "          size(event.size),",
            "          flags(event.flags),",
            "          effect(event.effect),",
            "          index(event.index)",
            "#endif",
            "    { }",
            "};",
        )
    );
}

#[test]
fn constructor_base_initializer_split_open_paren_keeps_initializer_column() {
    let source = fixture!(
        "class C : public Base<Impl>",
        "{",
        "public:",
        "    C()",
        "        : Base<Impl>",
        "          (",
        "              new Button(parent, id)",
        "          )",
        "    {",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_after_header_line_comment_indents() {
    let actual = format_c(
        fixture!(
            "C::C(int value) // comment",
            ": Base()",
            ", member(value)",
            "{",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "C::C(int value) // comment",
            "    : Base()",
            "    , member(value)",
            "{",
            "}",
        )
    );
}

#[test]
fn member_init_after_trailing_comment_indents_one_level() {
    let source =
        "class Foo\n{\npublic:\n    Foo()            // comment\n        : m_veto(false) { }\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_after_standalone_line_comment_keeps_initializer_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "C::C()",
                "    // comment",
                "    : a(1),",
                "      b(2)",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "C::C()",
            "// comment",
            "    : a(1),",
            "      b(2)",
            "{",
            "}",
        )
    );
}

#[test]
fn member_initializer_after_trailing_colon_signature_keeps_initializer_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "    Item(int count, const Item& value):",
        "        data{count, value}",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn member_init_colon_after_multiline_signature_indents_one_level() {
    let source = "class Foo\n{\npublic:\n    Foo( const TextData& fileName,\n         ImageMapType type = DEFAULT,\n         int a=-1, int b=-1 ) :\n        Bar(fileName, type)\n    {\n    }\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preserves_constructor_initializer_colon_spacing_and_continuation_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "class C {",
            "public:",
            "C(int a, int b, int c) :",
            "mA{a}, mB{b},",
            "mC{c} {}",
            "Item(int a):mA{a} {}",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class C {",
            "public:",
            "    C(int a, int b, int c) :",
            "        mA{a}, mB{b},",
            "        mC{c} {}",
            "    Item(int a): mA{a} {}",
            "};",
        )
    );
}

#[test]
fn split_constructor_initializer_colon_keeps_single_space_after_colon() {
    let source = "class C {\n    C(\n        int id_\n    ) : id(id_), loc(l_)\n    {\n    }\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_continuation_aligns_to_first_member_without_colon_space() {
    assert_eq!(
        format_c(
            "class C\n{\n    C(int a, int b, int c)\n        :alpha(a), beta(b),\n        gamma(c)\n    { }\n};\n",
            &FormatOptions::default(),
        ),
        "class C\n{\n    C(int a, int b, int c)\n        :alpha(a), beta(b),\n         gamma(c)\n    { }\n};\n",
    );
}

#[test]
fn aligns_constructor_initializer_colons_and_braced_argument_continuations() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "class Foo : public Bar {",
            "public:",
            "Foo(Config &config, int x)",
            ": Bar{config, x,",
            "alpha, beta} {",
            "}",
            "Foo(Config &config, int a,",
            "int b, int c)",
            ": Bar{config, a, b,",
            "gamma, delta} {",
            "}",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Foo : public Bar {",
            "public:",
            "    Foo(Config &config, int x)",
            "        : Bar{config, x,",
            "              alpha, beta} {",
            "    }",
            "    Foo(Config &config, int a,",
            "        int b, int c)",
            "        : Bar{config, a, b,",
            "              gamma, delta} {",
            "    }",
            "};",
        )
    );
}

#[test]
fn function_try_initializer_keeps_colon_and_initializer_on_separate_lines() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("Item::Item() try", ": value()", "{", "run();", "}"),
            &options,
        ),
        fixture!(
            "Item::Item() try",
            ":",
            "    value()",
            "{",
            "    run();",
            "}",
        )
    );
}

#[test]
fn standalone_function_try_marker_preserves_initializer_line_breaks() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("Item::Item()", "try", ":", "value()", "{", "run();", "}",),
            &options,
        ),
        fixture!(
            "Item::Item()",
            "try",
            ":",
            "    value()",
            "{",
            "    run();",
            "}",
        )
    );
}

#[test]
fn function_try_initializer_keeps_attached_comment_with_colon() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "Item::Item() try",
                ": /* note */ value()",
                "{",
                "run();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "Item::Item() try",
            ": /* note */",
            "    value()",
            "{",
            "    run();",
            "}",
        )
    );
}

#[test]
fn function_try_initializer_members_use_one_structural_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "Item::Item() try",
                ": alpha(1),",
                "beta(2)",
                "{",
                "run();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "Item::Item() try",
            ":",
            "    alpha(1),",
            "    beta(2)",
            "{",
            "    run();",
            "}",
        )
    );
}

// The initializer colon stays with its constructor header at every nesting depth.
#[test]
fn nested_function_try_initializer_colon_keeps_constructor_column() {
    let actual = format_c(
        fixture!(
            "class Item{",
            "public:",
            "Item() try",
            ": value(1){",
            "call();",
            "}catch(...){",
            "recover();",
            "}",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item {",
            "public:",
            "    Item() try",
            "    :",
            "        value(1) {",
            "        call();",
            "    } catch(...) {",
            "        recover();",
            "    }",
            "};",
        )
    );
}

#[test]
fn indent_after_parens_keeps_constructor_call_level_after_prior_member() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 4;
    let source = fixture!(
        "Item::Item(int value):",
        "first(value),",
        "second(call(alpha,",
        "beta)){",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "Item::Item(int value):",
            "    first(value),",
            "    second(call(alpha,",
            "                                    beta))",
            "{",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_keeps_constructor_call_arguments_on_owner_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    let source = fixture!(
        "Item::Item(int value):",
        "first(value),",
        "second(call(alpha,",
        "beta)){",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "Item::Item(int value):",
            "    first(value),",
            "    second(call(alpha,",
            "            beta))",
            "{",
            "}",
        )
    );
}

#[test]
fn constructor_initializer_comment_preserves_member_and_call_owners() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "Item::Item(int value):",
        "first(value),",
        "// member",
        "second(call(alpha,",
        "beta)){",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "Item::Item(int value):",
            "    first(value),",
            "// member",
            "    second(call(alpha,",
            "                beta))",
            "{",
            "}",
        )
    );
}

#[test]
fn constructor_initializer_comment_preserves_structural_tabs() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    options.tab_width = 4;
    let source = fixture!(
        "Item::Item(int value):",
        "first(value),",
        "// member",
        "second(call(alpha,",
        "beta)){",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "Item::Item(int value):",
            "\tfirst(value),",
            "// member",
            "\tsecond(call(alpha,",
            "\t            beta))",
            "{",
            "}",
        )
    );
}

#[test]
fn whitesmith_constructor_brace_uses_initializer_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let source = fixture!(
        "Item::Item(int value):",
        "first(value),",
        "second(call(alpha,",
        "beta)){",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "Item::Item(int value):",
            "    first(value),",
            "    second(call(alpha,",
            "                beta))",
            "    {",
            "    }",
        )
    );
}

#[test]
fn whitesmith_keeps_short_constructor_initializer_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid Whitesmith style");

    assert_eq!(
        format_c(
            fixture!("struct Item {", "int value;", "Item() : value(0) {}", "};",),
            &options,
        ),
        fixture!(
            "struct Item",
            "    {",
            "    int value;",
            "    Item() : value(0) {}",
            "    };",
        ),
    );
}

#[test]
fn same_line_constructor_initializer_continuation_after_comma_uses_initializer_indent() {
    assert_eq!(
        format_c(
            "struct SampleData {\n    SampleData() : firstActionCount(0), delayTicks(0), result(0),\n    currentOperationIteration(0), updateRequested(false)\n    {\n    }\n};\n",
            &FormatOptions::default(),
        ),
        "struct SampleData {\n    SampleData() : firstActionCount(0), delayTicks(0), result(0),\n        currentOperationIteration(0), updateRequested(false)\n    {\n    }\n};\n",
    );
}

#[test]
fn whitesmith_constructor_body_uses_initializer_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;

    assert_eq!(
        format_c(
            fixture!("Item::Item()", ": value(1)", "{", "run();", "}"),
            &options,
        ),
        fixture!(
            "Item::Item()",
            "    : value(1)",
            "    {",
            "    run();",
            "    }",
        )
    );
}

#[test]
fn multiline_constructor_initializer_body_keeps_constructor_body_indent() {
    let source = fixture!(
        "class Item {",
        " public:",
        "  Item(",
        "      int value)",
        "      : base(value), member(value) {",
        "    call();",
        "    if (ready()) done();",
        "  }",
        "};",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "class Item {",
            "public:",
            "    Item(",
            "        int value)",
            "        : base(value), member(value) {",
            "        call();",
            "        if (ready()) done();",
            "    }",
            "};",
        )
    );
}

#[test]
fn in_class_constructor_initializer_list_indents_one_level_past_signature() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let source = fixture!(
        "class C {",
        "public:",
        "    explicit C(Context *parent)",
        "        : Context(parent)",
        "    {",
        "    }",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class C",
            "{",
            "public:",
            "    explicit C(Context *parent)",
            "        : Context(parent)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn member_init_header_keeps_signature_indent_with_default_arg_and_break_after_logical() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    options.break_after_logical = true;
    let source = fixture!(
        "class C {",
        "public:",
        "    explicit C(Context *parent = nullptr)",
        "        : Base(parent)",
        "    {",
        "    }",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class C",
            "{",
            "public:",
            "    explicit C(Context *parent = nullptr)",
            "        : Base(parent)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn constructor_member_initializer_call_arguments_align_under_first_argument() {
    assert_eq!(
        format_c(
            fixture!(
                "class C",
                "{",
                "    C(int parent, int id, int pos)",
                "    : button(parent,",
                "             id,",
                "             pos)",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class C",
            "{",
            "    C(int parent, int id, int pos)",
            "        : button(parent,",
            "                 id,",
            "                 pos)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn constructor_member_initializer_argument_rows_align_past_initializer_column() {
    let source = fixture!(
        "class C {",
        "    C(int value)",
        "        : first(value),",
        "          second(",
        "              value),",
        "          third(",
        "              value) {",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braced_constructor_member_initializer_rows_keep_initializer_column() {
    let source = fixture!(
        "Class::Class()",
        "    : alpha{ value },",
        "      beta{ value },",
        "      gamma{ value }",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_member_initializer_after_comment_keeps_initializer_column() {
    let source = fixture!(
        "Class::Class()",
        "    : alpha(0),",
        "      // comment",
        "      beta(0)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_constructor_member_initializer_nested_call_arguments_use_inner_call_column() {
    assert_eq!(
        format_c(
            fixture!(
                "class C",
                "{",
                "    C()",
                "        : Base(),",
                "          m_control",
                "          (",
                "              make",
                "              (",
                "               a,",
                "               b",
                "              )",
                "          ),",
                "          m_other(0)",
                "    {}",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class C",
            "{",
            "    C()",
            "        : Base(),",
            "          m_control",
            "          (",
            "              make",
            "              (",
            "                  a,",
            "                  b",
            "              )",
            "          ),",
            "          m_other(0)",
            "    {}",
            "};",
        )
    );
}

#[test]
fn split_constructor_initializer_nested_call_arguments_keep_call_column_after_comma() {
    assert_eq!(
        format_c(
            fixture!(
                "class WidgetImpl : public ServiceImplBase<DocumentWidgetRecord>",
                "{",
                "public:",
                "    WidgetImpl(Parent* parent, size_t n, const Buffer* buffers)",
                "        : ServiceImplBase<DocumentWidgetRecord>",
                "          (",
                "            new Widget(parent, ID_ALL,",
                "                       InitialPosition, InitialSize,",
                "                       n, buffers)",
                "          )",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class WidgetImpl : public ServiceImplBase<DocumentWidgetRecord>",
            "{",
            "public:",
            "    WidgetImpl(Parent* parent, size_t n, const Buffer* buffers)",
            "        : ServiceImplBase<DocumentWidgetRecord>",
            "          (",
            "              new Widget(parent, ID_ALL,",
            "                         InitialPosition, InitialSize,",
            "                         n, buffers)",
            "          )",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn constructor_initializer_direct_list_elements_align_to_first_element() {
    let source = fixture!(
        "C::C()",
        "    : base(),",
        "      data{Person{one},",
        "           Person{two},",
        "           Person{three}},",
        "      next(0)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_member_after_line_comment_keeps_member_column() {
    let source = fixture!(
        "class C",
        "{",
        "    C() :",
        "        // comment",
        "        m(value)",
        "    {",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_call_argument_uses_call_column_over_max() {
    assert_eq!(
        format_c(
            fixture!(
                "class T",
                "{",
                "    T() :",
                "      firstSpy(&source, &Source::first),",
                "      backgroundOperationCompletionValueSpy(&source,",
                "                                            &Source::backgroundOperationCompletionValue),",
                "      lastSpy(&source, &Source::last)",
                "    {",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class T",
            "{",
            "    T() :",
            "        firstSpy(&source, &Source::first),",
            "        backgroundOperationCompletionValueSpy(&source,",
            "                                              &Source::backgroundOperationCompletionValue),",
            "        lastSpy(&source, &Source::last)",
            "    {",
            "    }",
            "};",
        )
    );
}

#[test]
fn member_init_continuation_does_not_leak_into_next_signature() {
    let source = "class Foo\n{\npublic:\n    Foo(const TextData& name, ImageMapType type, const IntPair& mapSpot)\n        : Foo(name, type, mapSpot.x, mapSpot.y) { }\n    Foo(const TextData& name,\n        ImageMapType type = DEFAULT,\n        int mapSpotX = 0, int mapSpotY = 0);\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_argument_continuation_aligns_after_open_paren() {
    let source = fixture!(
        "C::C(Config &config)",
        "    : server(),",
        "      spy(&server,",
        "          &Server::signal),",
        "      next(&server)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn constructor_initializer_call_commas_preserve_source_spacing() {
    assert_eq!(
        format_c(
            fixture!(
                "C::C( const String& label, const String& name,",
                "   const Data& value ) : Base(label,name,value.get())",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "C::C( const String& label, const String& name,",
            "      const Data& value ) : Base(label,name,value.get())",
            "{",
            "}",
        )
    );
}

#[test]
fn constructor_initializer_ternary_arms_align_to_condition_column() {
    assert_eq!(
        format_c(
            fixture!(
                "C::C(int value)",
                "    : first(a),",
                "      member(cond()",
                "    ? alpha()",
                "    : beta())",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "C::C(int value)",
            "    : first(a),",
            "      member(cond()",
            "             ? alpha()",
            "             : beta())",
            "{",
            "}",
        )
    );
}

#[test]
fn bitwise_or_continuation_in_constructor_initializer_aligns_to_first_operand() {
    assert_eq!(
        format_c(
            fixture!(
                "class Engine",
                "{",
                "protected:",
                "    struct File",
                "    {",
                "        File()",
                "            : userId(0)",
                "            , groupId(0)",
                "            , flags(",
                "                    ReadOwnerPerm | WriteOwnerPerm | ExeOwnerPerm",
                "                    | ReadUserPerm | WriteUserPerm | ExeUserPerm",
                "                    | FileType | ExistsFlag)",
                "        {",
                "        }",
                "    };",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class Engine",
            "{",
            "protected:",
            "    struct File",
            "    {",
            "        File()",
            "            : userId(0)",
            "            , groupId(0)",
            "            , flags(",
            "                  ReadOwnerPerm | WriteOwnerPerm | ExeOwnerPerm",
            "                  | ReadUserPerm | WriteUserPerm | ExeUserPerm",
            "                  | FileType | ExistsFlag)",
            "        {",
            "        }",
            "    };",
            "};",
        )
    );
}

#[test]
fn constructor_initializer_with_body_brace_uses_body_indent() {
    assert_eq!(
        format_c(
            "template <typename T, size_t SIZE = 500, typename Allocator = allocator<T>>\nclass Holder final : public detail::holder<T> {\nprivate:\n  int value;\n\npublic:\n  Holder(Holder&& other) noexcept\n      : detail::holder<T>(grow) {\n    move(other);\n  }\n};\n",
            &FormatOptions::default(),
        ),
        "template <typename T, size_t SIZE = 500, typename Allocator = allocator<T>>\nclass Holder final : public detail::holder<T> {\nprivate:\n    int value;\n\npublic:\n    Holder(Holder&& other) noexcept\n        : detail::holder<T>(grow) {\n        move(other);\n    }\n};\n",
    );
}

#[test]
fn constructor_initializer_continuation_after_template_method_keeps_colon_offset() {
    assert_eq!(
        format_c(
            "class Value {\n private:\n  int data_;\n\n public:\n  template <typename T, SELECT_IF(check<T>::value)>\n  Value(T value) : data_(value) {}\n\n  template <typename Visitor> auto visit(Visitor&& visitor) -> decltype(visitor(0)) {\n    return visitor(data_);\n  }\n};\n\ntemplate <typename Base> class Item : public Base {\n public:\n  explicit Item(string first = \"\", string second = \"\",\n                string third = \"\")\n      : first_(first),\n        second_(second),\n        third_(third) {}\n};\n",
            &FormatOptions::default(),
        ),
        "class Value {\nprivate:\n    int data_;\n\npublic:\n    template <typename T, SELECT_IF(check<T>::value)>\n    Value(T value) : data_(value) {}\n\n    template <typename Visitor> auto visit(Visitor&& visitor) -> decltype(visitor(0)) {\n        return visitor(data_);\n    }\n};\n\ntemplate <typename Base> class Item : public Base {\npublic:\n    explicit Item(string first = \"\", string second = \"\",\n                  string third = \"\")\n        : first_(first),\n          second_(second),\n          third_(third) {}\n};\n",
    );
}

#[test]
fn colon_alone_member_initializers_align_to_colon_indent() {
    let actual = format_c(
        fixture!(
            "class C",
            "{",
            "    C()",
            "  :",
            "    m_a(1),",
            "    m_b(2)",
            "    {",
            "    }",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class C",
            "{",
            "    C()",
            "        :",
            "        m_a(1),",
            "        m_b(2)",
            "    {",
            "    }",
            "};",
        )
    );
}

// Member-initializer rows after a trailing comma keep the first initializer column.
#[test]
fn constructor_member_initializer_rows_keep_first_initializer_column() {
    let source = fixture!(
        "Class::Class()",
        "    : alpha(0),",
        "      beta(0),",
        "      gamma(0) {}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// A same-line template class header must not replace the initializer column.
#[test]
fn member_init_comma_after_same_line_template_class_keeps_initializer_indent() {
    let source = fixture!(
        "template<typename IteratorType> class C",
        "{",
        "public:",
        "    C(IteratorType it, int index)",
        "    noexcept(cond",
        "             && other)",
        "        : anchor(std::move(it))",
        "        , array_index(index)",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// A constructor whose signature ends with a trailing qualifier (`noexcept`, `const`,
// ref-qualifier, ...) still opens a member-initializer list, so the leading `:` line
// must indent one level past the constructor, like the plain case.
#[test]
fn member_init_list_after_trailing_qualifier_is_indented() {
    let source = fixture!(
        "class C {",
        "public:",
        "    C(int a) noexcept",
        "        : x(a)",
        "        , y(0)",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn run_in_styles_keep_empty_function_body_compact() {
    let input = "struct Item {\n    Item()\n        : value{0}\n    {\n    }\n    int value;\n};\n";
    let cases = [
        (
            "horstmann",
            "struct Item\n{   Item()\n        : value{0}\n    {\n    }\n    int value;\n};\n",
        ),
        (
            "pico",
            "struct Item\n{   Item()\n        : value{0 }\n    {}\n    int value; };\n",
        ),
        (
            "lisp",
            "struct Item {\n    Item()\n        : value{0 } {}\n    int value; };\n",
        ),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(format_c(input, &options), expected);
    }
}

#[test]
fn template_constructor_member_initializer_stays_inline() {
    assert_eq!(
        format_c(
            "struct CString {\n\ttemplate <std::size_t sz> CString(char (&dest)[sz]) : dest{dest}, count{sz} {}\n\tchar *const dest;\n};\n",
            &FormatOptions::default(),
        ),
        "struct CString {\n    template <std::size_t sz> CString(char (&dest)[sz]) : dest{dest}, count{sz} {}\n    char *const dest;\n};\n"
    );
}

#[test]
fn constructor_initializer_ternary_call_does_not_leak_argument_indent_to_siblings() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");
    let source = "Box::Box()\n    : one(flag\n          ? a\n          : new Box(\n              x,\n              y))\n    , two(z)\n{\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn constructor_initializer_ternary_new_call_keeps_one_line_lambda_argument() {
    let mut options = FormatOptions::default();
    let args = ["--style=1tbs", "--pad-oper", "--pad-comma"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "Box::Box()\n    : one(flag\n          ? a\n          : new Box(\n              nullptr,\n              &Box::ready,\n              [this]() -> bool { return call(); },\n              Config{},\n              this))\n    , two(z)\n{\n}\n";

    assert_eq!(format_c(source, &options), source);
}
