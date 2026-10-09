use crate::common::{format, format_exact, format_with};
use cstyle::api::format_bytes;
use cstyle::config::{
    BraceStyle, FormatOptions, MinConditionalIndent, PointerAlign, ReferenceAlign,
    apply_command_line_args,
};

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
fn nested_direct_list_braces_stay_attached_to_types() {
    let source = fixture!("int x[] = {Item{1}, Item{2}};");

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn braced_declaration_preserves_source_gap_before_brace() {
    let options = FormatOptions::default();
    let tabbed = fixture!("int value\t{1, 2};");
    let spaced = fixture!("int value  {1, 2};");

    assert_eq!(format_exact(tabbed, &options), tabbed);
    assert_eq!(format_exact(spaced, &options), spaced);
}

#[test]
fn multiline_braced_declarations_keep_braces_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f() {",
                "\tConfig items[3] {",
                "\t\t1, 2, 3",
                "\t};",
                "\tConfig names{",
                "\t\t4",
                "\t};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tConfig items[3] {",
            "\t\t1, 2, 3",
            "\t};",
            "\tConfig names{",
            "\t\t4",
            "\t};",
            "}",
        )
    );
}

#[test]
fn nested_designated_field_brace_preserves_source_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f() {",
                "\tConfig sc{.name={\"a\"}, .color=6};",
                "\tint a[] = {1, 2};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tConfig sc{.name={\"a\"}, .color=6};",
            "\tint a[] = {1, 2};",
            "}",
        )
    );
}

#[test]
fn multiline_designated_initializer_keeps_run_in_alignment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f() {",
                "\tConfig player{.count = n, .turn_value = 23,",
                "\t              .source_id=15, .target_id=51};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tConfig player{.count = n, .turn_value = 23,",
            "\t              .source_id=15, .target_id=51};",
            "}",
        )
    );
}

#[test]
fn multiline_designated_fields_preserve_brace_spacing_and_align_siblings() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f() {",
                "\tConfig p{.name={1}, .color=23,",
                "\t        .count=17};",
                "\tConfig q{.name = {2}, .color=5,",
                "\t        .count=9};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tConfig p{.name={1}, .color=23,",
            "\t         .count=17};",
            "\tConfig q{.name = {2}, .color=5,",
            "\t         .count=9};",
            "}",
        )
    );
}

#[test]
fn tab_indent_uses_tabs_for_split_array_elements() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent=tab".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "const char *items[] =",
                "{",
                "    \"first\",",
                "    \"second\",",
                "    \"third\",",
            ),
            &options,
        ),
        fixture!(
            "const char *items[] =",
            "{",
            "\t\"first\",",
            "\t\"second\",",
            "\t\"third\",",
        )
    );
}

#[test]
fn tab_indent_uses_tabs_for_commented_initializer_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--indent=tab".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "static Action table[row_count][column_count] = {",
                "    // columns",
                "    /* alpha */ { NULL,      DO(first) },",
                "    /* beta  */ { DO(second), NULL },",
            ),
            &options,
        ),
        fixture!(
            "static Action table[row_count][column_count] = {",
            "\t// columns",
            "\t/* alpha */ { NULL,      DO(first) },",
            "\t/* beta  */ { DO(second), NULL },",
        )
    );
}

#[test]
fn java_keeps_comment_separated_array_initializer_brace_split() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=java".to_owned()]).expect("valid options");
    let source = fixture!(
        "const char *items[] = /* note */",
        "{",
        "    \"first\",",
        "    \"second\",",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn formats_array_and_compound_literal_braces() {
    let actual = format(fixture!(
        "int a[]={1,2};",
        "int b[][2]={{1,2},{3,4}};",
        "struct S s=(struct S){.x=1,.y=2};",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int a[] = {1, 2};",
            "int b[][2] = {{1, 2}, {3, 4}};",
            "struct S s = (struct S) {.x = 1, .y = 2};",
        )
    );
}

#[test]
fn nested_initializer_keeps_trailing_comment_on_closing_brace() {
    let source = fixture!(
        "static const Item value = {",
        "  {{NULL}, EMPTY,  /* value */",
        "   DEAD, 0, {NULL}}  /* key */",
        "};",
    );

    assert_eq!(
        format_exact(source, &FormatOptions::default()),
        fixture!(
            "static const Item value = {",
            "    {   {NULL}, EMPTY,  /* value */",
            "        DEAD, 0, {NULL}",
            "    }  /* key */",
            "};",
        )
    );
}

#[test]
fn initializer_element_open_brace_keeps_three_space_line_comment_gap() {
    let source = fixture!(
        "void f()",
        "{",
        "    Item items[] = {",
        "        { // position",
        "            0",
        "        },",
        "        { // normal",
        "            1",
        "        }",
        "    };",
        "}",
    );

    assert_eq!(
        format_exact(source, &FormatOptions::default()),
        fixture!(
            "void f()",
            "{",
            "    Item items[] = {",
            "        {   // position",
            "            0",
            "        },",
            "        {   // normal",
            "            1",
            "        }",
            "    };",
            "}",
        )
    );
}

#[test]
fn index_braced_initializer_gets_space_after_open_bracket() {
    assert_eq!(
        format_exact(
            fixture!("void f()", "{", "    adapter[{0, 0}, 0] = \"1.0\";", "}"),
            &FormatOptions::default(),
        ),
        fixture!("void f()", "{", "    adapter[ {0, 0}, 0] = \"1.0\";", "}")
    );
}

#[test]
fn preprocessor_branch_negative_braced_init_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "#ifdef X",
                "    T value{-Item};",
                "#endif",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "#ifdef X",
            "    T value {-Item};",
            "#endif",
            "}",
        )
    );
}

#[test]
fn first_braced_init_after_endif_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "#if X",
                "    skip();",
                "#endif",
                "    Timer timer1{OPERATION_TIMEOUT};",
                "    Timer timer2{OPERATION_TIMEOUT};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "#if X",
            "    skip();",
            "#endif",
            "    Timer timer1 {OPERATION_TIMEOUT};",
            "    Timer timer2{OPERATION_TIMEOUT};",
            "}",
        )
    );
}

#[test]
fn first_braced_init_after_if_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "#if X",
                "    T value{item};",
                "    T next{item};",
                "#endif",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "#if X",
            "    T value {item};",
            "    T next{item};",
            "#endif",
            "}",
        )
    );
}

#[test]
fn nested_preprocessor_string_view_braced_init_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    if (ok) {",
                "#if X",
                "        CHECK_EQ(std::u16string_view{result}.size(), 2);",
                "#endif",
                "#ifndef Y",
                "        const auto value = EncodedStringView{bytes};",
                "#endif",
                "#if Z",
                "        if (BinaryDataView{tag}.contains(\"marker\"))",
                "            done();",
                "#endif",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    if (ok) {",
            "#if X",
            "        CHECK_EQ(std::u16string_view {result}.size(), 2);",
            "#endif",
            "#ifndef Y",
            "        const auto value = EncodedStringView {bytes};",
            "#endif",
            "#if Z",
            "        if (BinaryDataView {tag}.contains(\"marker\"))",
            "            done();",
            "#endif",
            "    }",
            "}",
        )
    );
}

#[test]
fn repeated_preprocessor_leading_comma_rows_keep_sibling_indent() {
    assert_eq!(
        format_exact(
            fixture!(
                "const char* values[] = {",
                "    \"a\"",
                "#if X",
                "      , \"b\"",
                "#endif",
                "#if Y",
                "      , \"c\"",
                "#endif",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "const char* values[] = {",
            "    \"a\"",
            "#if X",
            "    , \"b\"",
            "#endif",
            "#if Y",
            "    , \"c\"",
            "#endif",
            "};",
        )
    );
}

// Inline braced arguments stay tied to their initializer brace in every call context.
#[test]
fn nested_inline_braced_call_argument_aligns_body_to_its_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    auto guard = defer([&] {",
                "        verify(Namespace::execute(\"tool\", { \"first\",",
                "            \"second\", value }) == 0);",
                "    });",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    auto guard = defer([&] {",
            "        verify(Namespace::execute(\"tool\", { \"first\",",
            "                                            \"second\", value",
            "                                          }) == 0);",
            "    });",
            "}",
        )
    );
}

#[test]
fn operator_continuation_in_inline_braced_initializer_aligns_to_element() {
    let source = fixture!(
        "void f()",
        "{",
        "    value = Type{first + call()",
        "                 + more()};",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn return_braced_initializer_keeps_trailing_close_on_last_element() {
    let source = fixture!(
        "Result f()",
        "{",
        "    return { Status::Error,",
        "             message };",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn return_braced_initializer_pair_after_comma_stays_one_line() {
    let source = "Table<int, ByteBuffer> f()\n{\n    return {{PrimaryRole, TEXT_LITERAL(\"alpha\")},\n        {SecondaryRole, TEXT_LITERAL(\"beta\")}};\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn braced_initializer_after_open_paren_preserves_source_space() {
    let source = fixture!(
        "void f()",
        "{",
        "    call( {1});",
        "    adapter[ {0, 0}, 0] = \"x\";",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn allman_range_for_braced_init_keeps_initializer_brace_attached() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    for (char c : {'a',",
                "                   'b'})",
                "    {",
                "        g(c);",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    for (char c :",
            "            {'a',",
            "             'b'",
            "            })",
            "    {",
            "        g(c);",
            "    }",
            "}",
        )
    );
}

#[test]
fn range_for_one_line_braced_range_stays_in_header() {
    let source = "void f()\n{\n    auto clear = [&] {\n        for (auto *s : {&a, &b, &c})\n            s->clear();\n    };\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn gnu_range_for_braced_init_keeps_command_body_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    for (char c : {'a',",
                "                   'b'})",
                "    {",
                "        g(c);",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    for (char c :",
            "            {'a',",
            "             'b'",
            "            })",
            "        {",
            "            g(c);",
            "        }",
            "}",
        )
    );
}

#[test]
fn run_in_braced_initializer_pair_with_multiline_call_stays_run_in() {
    let source = fixture!(
        "void f()",
        "{",
        "    result[\"compiler\"] = {{\"family\", \"gcc\"}, {\"version\", detail::concat(",
        "                a, '.',",
        "                b, '.',",
        "                c)",
        "        }",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn range_for_braced_initializer_breaks_run_in_list() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    for (int value : {10, 8, 16}) {",
                "        call(value);",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    for (int value : {",
            "                10, 8, 16",
            "            }) {",
            "        call(value);",
            "    }",
            "}",
        )
    );
}

#[test]
fn range_for_double_brace_initializer_stays_run_in() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    for (auto item : std::vector<std::vector<const char *>>{{\"red\", \"r\"}, {\"green\", \"g\"}}) {",
                "        call();",
                "    }",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    for (auto item : std::vector<std::vector<const char *>> {{\"red\", \"r\"}, {\"green\", \"g\"}}) {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn braced_call_argument_sibling_calls_keep_initializer_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    object->call({",
        "        first(0, one, two),",
        "        second(1, three, four)",
        "    });",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn inline_array_after_commented_first_element_aligns_next_element() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    const String value[] = { \"\", // none",
                "                       \"alpha\",",
                "                             \"beta\" };",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    const String value[] = { \"\", // none",
            "                             \"alpha\",",
            "                             \"beta\"",
            "                           };",
            "}",
        )
    );
}

#[test]
fn braced_call_argument_run_in_initializer_stays_run_in() {
    let source = fixture!(
        "void f()",
        "{",
        "    model.setLabels({ translate(\"Name\"),",
        "                      translate(\"Office\") });",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn stroustrup_keeps_single_string_braced_argument_inline() {
    let options = options_from_args(&["--style=stroustrup", "--mode=c"]);
    let source = fixture!(
        "void f()",
        "{",
        "    EXPECT_EQ(value, call(",
        "    {\"text\"}));",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn stroustrup_braced_argument_uses_base_indent_instead_of_paren_alignment() {
    let options = options_from_args(&["--style=stroustrup", "--mode=c"]);

    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    EXPECT_EQ(value, call(",
                "                          {\"text\"}));",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    EXPECT_EQ(value, call(",
            "    {\"text\"}));",
            "}",
        )
    );
}

#[test]
fn multiline_array_initializer_breaks_attached_closing_brace_to_own_line() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    const char* inputs[] = {\"a\",",
                "                            \"b\",",
                "                            \"c\"};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    const char* inputs[] = {\"a\",",
            "                            \"b\",",
            "                            \"c\"",
            "                           };",
            "}",
        )
    );
}

#[test]
fn enclosed_array_run_in_element_brace_preserves_source_gap() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_exact(
            fixture!(
                "struct s v = {",
                "\t.arr = { { .a = 1,",
                "\t\t\t.b = 2 } },",
                "\t.n = 2,",
                "};",
            ),
            &options,
        ),
        fixture!(
            "struct s v = {",
            "    .arr = { {",
            "            .a = 1,",
            "            .b = 2",
            "        }",
            "    },",
            "    .n = 2,",
            "};",
        )
    );
}

#[test]
fn braced_initializer_sibling_after_braced_element_keeps_element_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    Map value{",
        "        Map::key_container_type{ 6, 2, 1 },",
        "        Map::mapped_container_type{ \"foo\", \"bar\", \"baz\" }",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn multiline_braced_call_argument_after_comma_breaks_closing_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    CONFIRM(Executor::execute(\"tool\", { \"create\", \"-quiet\",",
                "        \"-type\", \"SPARSE\", \"-size\", size,",
                "        \"-name\", name, image }) == 0);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    CONFIRM(Executor::execute(\"tool\", { \"create\", \"-quiet\",",
            "                                        \"-type\", \"SPARSE\", \"-size\", size,",
            "                                        \"-name\", name, image",
            "                                      }) == 0);",
            "}",
        )
    );
}

#[test]
fn return_braced_new_initializer_rows_use_brace_body_indent() {
    assert_eq!(
        format_exact(
            fixture!(
                "List f()",
                "{",
                "    return {new Item(first),",
                "            new Item(second),",
                "            new Item(third)};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "List f()",
            "{",
            "    return {new Item(first),",
            "               new Item(second),",
            "               new Item(third)};",
            "}",
        )
    );
}

#[test]
fn initializer_call_argument_over_max_continuation_uses_initializer_indent() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    object.setBindings({",
                "        GenericResourceDescriptor::createResource(0, GenericResourceDescriptor::OperationMode,",
                "                                     content.get(), context.get())",
                "    });",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    object.setBindings({",
            "        GenericResourceDescriptor::createResource(0, GenericResourceDescriptor::OperationMode,",
            "                content.get(), context.get())",
            "    });",
            "}",
        )
    );
}

#[test]
fn initializer_brace_continuation_anchors_to_statement_base() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;

    assert_eq!(
        format_exact(
            fixture!("int table[]", "    = {", "    1,", "    2,", "};"),
            &options,
        ),
        fixture!("int table[]", "= {", "    1,", "    2,", "};")
    );
}

#[test]
fn nested_initializer_brace_continuation_anchors_to_statement_base() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let source = fixture!(
        "void f()",
        "{",
        "    int table[]",
        "        = {",
        "        1,",
        "        2,",
        "    };",
        "}",
    );

    assert_eq!(
        format_exact(source, &options),
        fixture!(
            "void f()",
            "{",
            "    int table[]",
            "    = {",
            "        1,",
            "        2,",
            "    };",
            "}",
        )
    );
}

#[test]
fn run_in_direct_list_array_element_keeps_attached_closing_brace() {
    let source = fixture!(
        "const std::array<Item, 2> items = {",
        "    Item{ a,",
        "          call() },",
        "    Item{ b,",
        "          call() }",
        "};",
    );

    assert_eq!(
        format_exact(source, &FormatOptions::default()),
        fixture!(
            "const std::array<Item, 2> items = {",
            "    Item{",
            "        a,",
            "        call() },",
            "    Item{",
            "        b,",
            "        call() }",
            "};",
        )
    );
}

#[test]
fn direct_list_initializer_keeps_split_opening_brace_by_default() {
    assert_eq!(
        format_exact(
            fixture!(
                "void run(){",
                "static std::array<int, 2> values",
                "{",
                "1, 2",
                "};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run() {",
            "    static std::array<int, 2> values",
            "    {",
            "        1, 2",
            "    };",
            "}",
        )
    );
}

#[test]
fn vtk_direct_list_initializer_indents_brace_block() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let expected = fixture!(
        "void run()",
        "{",
        "    static int values",
        "        {",
        "        1",
        "        };",
        "}",
    );

    assert_eq!(
        format_exact(
            fixture!("void run()", "{", "static int values {", "1", "};", "}",),
            &options,
        ),
        expected
    );
    assert_eq!(
        format_exact(
            fixture!("void run()", "{", "static int values", "{", "1", "};", "}",),
            &options,
        ),
        expected
    );
}

#[test]
fn brace_initializer_call_arguments_align_under_inline_call() {
    let actual = format_exact(
        fixture!(
            "void f()",
            "{",
            "    Ptr p{call(",
            "        first,",
            "        second(",
            "            value",
            "        ),",
            "        third",
            "    )};",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    Ptr p{call(",
            "              first,",
            "              second(",
            "                  value",
            "              ),",
            "              third",
            "          )};",
            "}",
        )
    );
}

#[test]
fn array_initializer_same_line_brace_gets_space_after_assignment() {
    let actual = format_exact(
        fixture!(
            "static TextType names[2]={get(\"A\"),",
            "                          get(\"B\")",
            "                         };",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "static TextType names[2]= {get(\"A\"),",
            "                           get(\"B\")",
            "                          };",
        )
    );
}

// A leading negative literal does not turn later elements into operator continuations.
#[test]
fn negative_first_array_element_keeps_sibling_indent_consistent() {
    let source = fixture!(
        "static const int values[] =",
        "{",
        "    -1,",
        "    A,",
        "",
        "    B",
        "};",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

// A leading negative literal does not change attached-brace body or closer indentation.
#[test]
fn negative_first_array_element_attached_brace_keeps_sibling_indent_consistent() {
    let source = fixture!(
        "static const double T[] = {",
        "    -0.1,",
        "    0.2,",
        "    -0.3,",
        "};",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn operator_broken_first_array_element_aligns_to_element_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "static const int v[] = { A, B +\n\t\tC,\n\t\tD + E,\n\t\tF };\n",
            &options,
        ),
        "static const int v[] = { A, B +\n                         C,\n                         D + E,\n                         F\n                       };\n",
    );
}

#[test]
fn double_brace_aggregate_indents_elements_two_levels() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;

    assert_eq!(
        format_exact(
            fixture!(
                "constexpr std::array<T, 2> V = {{",
                "    {.a = 1, .b = 2},",
                "    {.a = 3, .b = 4},",
                "    }",
                "};",
            ),
            &options,
        ),
        fixture!(
            "constexpr std::array<T, 2> V = {{",
            "        {.a = 1, .b = 2},",
            "        {.a = 3, .b = 4},",
            "    }",
            "};",
        )
    );
}

#[test]
fn dedented_one_line_nested_initializer_keeps_source_gap() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    int values[][2] =",
                "        { { 0, 0 } };",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    int values[][2] =",
            "    { { 0, 0 } };",
            "}",
        )
    );
}

#[test]
fn dedented_nested_initializer_opening_keeps_run_in_gap() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    int values[][2] =",
                "        { { 0, 0 },{ 1, 0 },",
                "          { 0, 1 } };",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    int values[][2] =",
            "    {   { 0, 0 },{ 1, 0 },",
            "        { 0, 1 }",
            "    };",
            "}",
        )
    );
}

#[test]
fn std_array_double_brace_initializer_body_keeps_nested_indent() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    static constexpr std::array<std::uint8_t, 9> values = {{",
                "0,",
                "1",
                "}",
                "};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    static constexpr std::array<std::uint8_t, 9> values = {{",
            "            0,",
            "            1",
            "        }",
            "    };",
            "}",
        )
    );
}

#[test]
fn keeps_braced_initializer_arguments_inline_after_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "void f() {",
            "call({1, 2}, x);",
            "set({a, b});",
            "int y = ({ int t = a; t + 1; });",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    call({1, 2}, x);",
            "    set({a, b});",
            "    int y = ({ int t = a; t + 1; });",
            "}",
        )
    );
}

#[test]
fn preserves_source_spacing_at_aggregate_brace_edges() {
    let options = FormatOptions::default();

    assert_eq!(
        format_exact(fixture!("int m[2][2]={{1,2},{3,4}};"), &options),
        fixture!("int m[2][2]= {{1,2},{3,4}};")
    );
    assert_eq!(
        format_exact(fixture!("x={a, {1,2}};"), &options),
        fixture!("x= {a, {1,2}};")
    );
    assert_eq!(
        format_exact(fixture!("int a[]={  1,2  };"), &options),
        fixture!("int a[]= {  1,2  };")
    );
    assert_eq!(
        format_exact(fixture!("int a[]={\t1,2};"), &options),
        fixture!("int a[]= {\t1,2};")
    );
    assert_eq!(
        format_exact(fixture!("x={{{1}}};"), &options),
        fixture!("x= {{{1}}};")
    );
}

#[test]
fn keeps_generic_compound_literal_braces_attached_and_designators_indented() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(item_t *out, buffer_t *buf, size_t cap){",
            "*out = (item_t) {",
            ".buf = buf,",
            ".cap = cap,",
            ".status = ITEM_OK,",
            "};",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(item_t *out, buffer_t *buf, size_t cap)",
            "{",
            "    *out = (item_t) {",
            "        .buf = buf,",
            "        .cap = cap,",
            "        .status = ITEM_OK,",
            "    };",
            "}",
        )
    );
}

#[test]
fn attaches_split_array_initializer_braces_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "static Item *items[] =",
            "{",
            "alpha, beta",
            "};",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    static Item *items[] = {",
            "        alpha, beta",
            "    };",
            "}",
        )
    );
}

#[test]
fn run_in_array_initializer_keeps_closing_brace_on_own_line() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "int arr[] = { 1, 2, 3,",
            "              4, 5, 6",
            "            };"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int arr[] = { 1, 2, 3,",
            "              4, 5, 6",
            "            };"
        )
    );
}

#[test]
fn attaches_new_array_initializer_brace_in_one_true_brace_style() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;

    assert_eq!(
        format_with(fixture!("void f(){int *p = new int[3]{1,2,3};}"), &options),
        fixture!("void f()", "{", "    int *p = new int[3] {1, 2, 3};", "}",)
    );
}

#[test]
fn formats_multiline_arrays_without_comma_continuation_overindent() {
    let actual = format(fixture!(
        "int a[] = {",
        "1,",
        "2",
        "};",
        "int b[][2] = {",
        "{1,2},",
        "{3,4}",
        "};",
    ));

    assert_eq!(
        actual,
        fixture!(
            "int a[] = {",
            "    1,",
            "    2",
            "};",
            "int b[][2] =",
            "{",
            "    {1, 2},",
            "    {3, 4}",
            "};",
        )
    );
}

#[test]
fn formats_multiline_compound_literals_without_comma_continuation_overindent() {
    let actual = format(fixture!(
        "struct S s = (struct S) {",
        ".x=1,",
        ".y=2 // keep",
        "};",
        "int z=0;",
    ));

    assert_eq!(
        actual,
        fixture!(
            "struct S s = (struct S) {",
            "    .x = 1,",
            "    .y = 2 // keep",
            "};",
            "int z = 0;",
        )
    );
}

fn compound_literal_options() -> FormatOptions {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    options
}

#[test]
fn compound_literal_operator_continuation_aligns_under_value_when_last() {
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".alpha = OP,",
            ".value = ((uint64_t)first) |",
            "((uint64_t)second << 32) |",
            "((uint64_t)third << 34),",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .alpha = OP,",
            "        .value = ((uint64_t)first) |",
            "                 ((uint64_t)second << 32) |",
            "                 ((uint64_t)third << 34),",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_operator_continuation_does_not_leak_to_first_member() {
    // Value alignment must not leak onto following members; they stay at member level.
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".value = ((uint64_t)first) |",
            "((uint64_t)second << 32),",
            ".alpha = OP,",
            ".beta = end,",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .value = ((uint64_t)first) |",
            "                 ((uint64_t)second << 32),",
            "        .alpha = OP,",
            "        .beta = end,",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_operator_continuation_does_not_leak_from_middle_member() {
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".alpha = OP,",
            ".value = ((uint64_t)first) |",
            "((uint64_t)second << 32),",
            ".beta = end,",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .alpha = OP,",
            "        .value = ((uint64_t)first) |",
            "                 ((uint64_t)second << 32),",
            "        .beta = end,",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_pointer_cast_address_of_member_keeps_source_spacing() {
    let parens = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".data = ((uint8_t *)&value),",
            ".len = sizeof(value),",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        parens,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .data = ((uint8_t *)&value),",
            "        .len = sizeof(value),",
            "    };",
            "}",
        )
    );
    let no_parens = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".data = (uint8_t *)&value,",
            ".len = sizeof(value),",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        no_parens,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .data = (uint8_t *)&value,",
            "        .len = sizeof(value),",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_address_of_after_cast_is_not_reference_padded() {
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Config) {",
            ".addr = (uintptr_t)&value,",
            ".len = sizeof(value),",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Config) {",
            "        .addr = (uintptr_t)&value,",
            "        .len = sizeof(value),",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_nested_levels_indent_and_align_correctly() {
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Outer) {",
            ".id = 1,",
            ".inner = (struct Inner) {",
            ".value = ((uint64_t)first) |",
            "((uint64_t)second << 32),",
            ".deep = (struct Deep) {",
            ".a = ((uint8_t *)&value),",
            ".b = sizeof(value),",
            "},",
            "},",
            ".tail = end,",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Outer) {",
            "        .id = 1,",
            "        .inner = (struct Inner) {",
            "            .value = ((uint64_t)first) |",
            "                     ((uint64_t)second << 32),",
            "            .deep = (struct Deep) {",
            "                .a = ((uint8_t *)&value),",
            "                .b = sizeof(value),",
            "            },",
            "        },",
            "        .tail = end,",
            "    };",
            "}",
        )
    );
}

#[test]
fn compound_literal_nested_operator_continuation_does_not_leak() {
    // Value alignment must not leak onto the following nested member.
    let actual = format_exact(
        fixture!(
            "void f(void)",
            "{",
            "result = (struct Outer) {",
            ".a = (struct Inner) {",
            ".value = ((uint64_t)first) |",
            "((uint64_t)second << 32),",
            ".next = 7,",
            "},",
            ".b = end,",
            "};",
            "}",
        ),
        &compound_literal_options(),
    );
    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    result = (struct Outer) {",
            "        .a = (struct Inner) {",
            "            .value = ((uint64_t)first) |",
            "                     ((uint64_t)second << 32),",
            "            .next = 7,",
            "        },",
            "        .b = end,",
            "    };",
            "}",
        )
    );
}

#[test]
fn initializer_operator_continuation_sits_at_member_level() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void g(void)",
            "{",
            "struct Config result = {",
            ".value = ((unsigned)alpha) |",
            "((unsigned)beta << 8) |",
            "((unsigned)gamma << 16),",
            "};",
            "}",
        ),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "void g(void)",
            "{",
            "    struct Config result = {",
            "        .value = ((unsigned)alpha) |",
            "        ((unsigned)beta << 8) |",
            "        ((unsigned)gamma << 16),",
            "    };",
            "}",
        )
    );
}

#[test]
fn initializer_paren_continuation_still_aligns_under_open_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void g(void)",
            "{",
            "struct Config result = {",
            ".value = call(alpha,",
            "beta),",
            "};",
            "}",
        ),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "void g(void)",
            "{",
            "    struct Config result = {",
            "        .value = call(alpha,",
            "                      beta),",
            "    };",
            "}",
        )
    );
}

#[test]
fn split_compound_literal_brace_does_not_inherit_cast_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "void run()",
        "{",
        "    Item value=(Item)",
        "    {",
        "        .alpha=first+",
        "               second,",
        "        .beta=third",
        "    };",
        "}",
    );

    // A member's binary continuation does not shift later compound-literal siblings.
    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn inline_array_initializer_keeps_content_inline_and_aligns_continuation() {
    let options = FormatOptions::default();
    let actual = format_exact(
        fixture!("int values[] = {alpha, beta,", "gamma, delta};"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int values[] = {alpha, beta,",
            "                gamma, delta",
            "               };"
        )
    );
}

#[test]
fn inline_array_rows_ignore_overindented_source_continuation() {
    assert_eq!(
        format_exact(
            fixture!(
                "const float values[] = { 1.0f, 2.0f, 3.0f,",
                "                             4.0f, 5.0f, 6.0f,",
                "                          7.0f, 8.0f, 9.0f,",
                "                             10.0f, 11.0f, 12.0f };",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "const float values[] = { 1.0f, 2.0f, 3.0f,",
            "                         4.0f, 5.0f, 6.0f,",
            "                         7.0f, 8.0f, 9.0f,",
            "                         10.0f, 11.0f, 12.0f",
            "                       };",
        )
    );
}

#[test]
fn inline_array_after_trailing_block_comment_aligns_next_element() {
    assert_eq!(
        format_exact(
            fixture!(
                "const float values[] = { 1.0f, 2.0f, 3.0f, /* a */",
                "                             4.0f, 5.0f, 6.0f, /* b */",
                "                             7.0f, 8.0f, 9.0f  /* c */ };",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "const float values[] = { 1.0f, 2.0f, 3.0f, /* a */",
            "                         4.0f, 5.0f, 6.0f, /* b */",
            "                         7.0f, 8.0f, 9.0f  /* c */",
            "                       };",
        )
    );
}

#[test]
fn nested_brace_initializer_uses_normal_indent() {
    // A brace-init list that contains a nested brace is not aligned to the
    // open-brace content column; it uses one level of normal indentation and
    // breaks the closing brace to the base column.
    let options = FormatOptions::default();
    let actual = format_exact(
        fixture!("int grid[][2] = {{1, 2}, {3, 4},", "{5, 6}, {7, 8}};"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int grid[][2] = {{1, 2}, {3, 4},",
            "    {5, 6}, {7, 8}",
            "};"
        )
    );
}

#[test]
fn preserves_extra_space_between_one_line_brace_elements() {
    let options = FormatOptions::default();
    let source = fixture!("int a[] = {", "    {1},  {2},", "    {3},  {4},", "};",);

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn keeps_one_line_compound_literals_inline() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "value = (item_t) {0};",
            "int values[] = {0};",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    value = (item_t) {0};",
            "    int values[] = {0};",
            "}",
        )
    );
}

#[test]
fn keeps_simple_braced_initializers_on_one_line() {
    let actual = format(fixture!("void f(){int value{1}; int list[]{1,2};}"));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    int value{1};",
            "    int list[] {1, 2};",
            "}",
        )
    );
}

#[test]
fn array_brace_breaks_when_attached_and_not_on_first_line() {
    for style in [BraceStyle::Allman, BraceStyle::Gnu] {
        let mut options = FormatOptions::default();
        options.brace_style = style;
        assert_eq!(
            format_exact(
                fixture!(
                    "void f(void)",
                    "{",
                    "    int values[] = {",
                    "        1, 2",
                    "    };",
                    "}",
                ),
                &options
            ),
            fixture!(
                "void f(void)",
                "{",
                "    int values[] =",
                "    {",
                "        1, 2",
                "    };",
                "}",
            ),
            "style {style:?} must break an attached array brace below the first line"
        );
    }
}

#[test]
fn array_brace_attaches_for_attach_family() {
    let source = fixture!("int values[] =", "{", "    1, 2", "};");

    let mut attach = FormatOptions::default();
    attach.brace_style = BraceStyle::Attach;
    assert_eq!(
        format_exact(source, &attach),
        fixture!("int values[] = {", "    1, 2", "};")
    );

    let mut otbs = FormatOptions::default();
    otbs.brace_style = BraceStyle::OneTrueBrace;
    assert_eq!(
        format_exact(source, &otbs),
        fixture!("int values[] = {", "    1, 2", "};")
    );

    let mut lisp = FormatOptions::default();
    lisp.brace_style = BraceStyle::Lisp;
    assert_eq!(
        format_exact(source, &lisp),
        fixture!("int values[] = {", "    1, 2 };")
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    assert_eq!(
        format_exact(source, &ratliff),
        fixture!("int values[] = {", "    1, 2", "    };")
    );
}

#[test]
fn compound_literal_brace_attaches_in_attaching_styles() {
    let source = fixture!("Item v = (Item)", "{", "    .x = 1", "};");

    let mut attach = FormatOptions::default();
    attach.brace_style = BraceStyle::Attach;
    assert_eq!(
        format_exact(source, &attach),
        fixture!("Item v = (Item) {", "    .x = 1", "};")
    );

    let mut lisp = FormatOptions::default();
    lisp.brace_style = BraceStyle::Lisp;
    assert_eq!(
        format_exact(source, &lisp),
        fixture!("Item v = (Item) {", "    .x = 1 };")
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    assert_eq!(
        format_exact(source, &ratliff),
        fixture!("Item v = (Item) {", "    .x = 1", "    };")
    );

    let mut otbs = FormatOptions::default();
    otbs.brace_style = BraceStyle::OneTrueBrace;
    assert_eq!(
        format_exact(source, &otbs),
        fixture!("Item v = (Item) {", "    .x = 1", "};")
    );
}

#[test]
fn multiline_compound_literal_keeps_run_in_designated_fields() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  value = (struct Item) { .first = alpha,\n                          .second = beta\n                        };\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    value = (struct Item) { .first = alpha,\n                            .second = beta\n                          };\n}\n",
    );
}

#[test]
fn compound_literal_argument_after_ternary_uses_body_indent() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  display_status_dialog (APP_WINDOW (application_current_window (app)),\n                         \"display-name\", compare_a (CHANNEL, \"alpha\") == 0\n                                         ? \"Sample Tool (Preview)\"\n                                         : \"Sample Tool\",\n                         \"version\", version,\n                         \"comments\", \"Program\",\n                         \"contributors\", (const char *[]) { \"A\", NULL },\n                         \"logo\", logo,\n                         NULL);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    display_status_dialog (APP_WINDOW (application_current_window (app)),\n                           \"display-name\", compare_a (CHANNEL, \"alpha\") == 0\n                           ? \"Sample Tool (Preview)\"\n                           : \"Sample Tool\",\n                           \"version\", version,\n                           \"comments\", \"Program\",\n                           \"contributors\", (const char *[]) { \"A\", NULL },\n                           \"logo\", logo,\n                           NULL);\n}\n",
    );
}

#[test]
fn enclosed_array_elements_with_nested_brace_stay_one_line() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  call (a,\n        (SampleRecord[2]) {\n          { MAX (0.0, p), { 1, 1, 1, 1 } },\n          { MIN (1.0, p), { 0, 0, 0, 1 } }\n        }, 2);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    call (a,\n    (SampleRecord[2]) {\n        { MAX (0.0, p), { 1, 1, 1, 1 } },\n        { MIN (1.0, p), { 0, 0, 0, 1 } }\n    }, 2);\n}\n",
    );
}

#[test]
fn split_compound_literal_brace_keeps_designated_initializers_aligned() {
    let options = FormatOptions::default();

    assert_eq!(
        format_exact(
            fixture!(
                "long value = (Config)",
                "{",
                ".alpha = 1,",
                ".beta = 2,",
                ".gamma = 3,",
                "};",
            ),
            &options
        ),
        fixture!(
            "long value = (Config)",
            "{",
            "    .alpha = 1,",
            "    .beta = 2,",
            "    .gamma = 3,",
            "};",
        )
    );

    assert_eq!(
        format_exact(
            fixture!(
                "long value = run(&(Config)",
                "{",
                ".alpha = 1,",
                ".beta = 2,",
                "});",
            ),
            &options
        ),
        fixture!(
            "long value = run(&(Config)",
            "{",
            "    .alpha = 1,",
            "    .beta = 2,",
            "});",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    assert_eq!(
        format_exact(
            fixture!("Config value = (Config)", "{", ".alpha = 1", "};"),
            &ratliff
        ),
        fixture!("Config value = (Config) {", "    .alpha = 1", "    };")
    );
}

#[test]
fn designated_initializer_indent_survives_prior_aggregate_declaration() {
    let source = fixture!(
        "struct item {",
        "    int value;",
        "};",
        "",
        "static struct item items[] = {",
        "    [ITEM_ALPHA] = {",
        "        .value = 1,",
        "    },",
        "    [ITEM_BETA] = {",
        "        .value = 2,",
        "    },",
        "};",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn escaped_string_designated_initializers_keep_sibling_indent() {
    let source = fixture!(
        "static const char *const labels[] = {",
        "    [ITEM_ALPHA] = \"\\033[0;35m[A] \", // alpha",
        "    [ITEM_BETA] = \"\\033[0;31m[B] \", // beta",
        "};",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn final_designated_initializer_keeps_member_indent_before_closer() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    Data data = {",
        "        .count = 0",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn misindented_final_designated_initializer_moves_to_member_column() {
    let actual = format_exact(
        fixture!(
            "void helper(void)",
            "{",
            "    Data data = {",
            "    .count = 0",
            "    };",
            "}",
        ),
        &one_true_brace_c_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void helper(void)",
            "{",
            "    Data data = {",
            "        .count = 0",
            "    };",
            "}",
        )
    );
}

#[test]
fn designated_initializer_after_nested_call_keeps_member_indent() {
    let source = fixture!(
        "typedef struct {",
        "    int *items;",
        "    int count;",
        "} Data;",
        "",
        "void helper(void)",
        "{",
        "    Data data = {",
        "        .items = allocate(LIMIT, sizeof(Item)),",
        "        .count = 0",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

// A completed shift expression does not change the next member column.
#[test]
fn designated_initializer_member_after_shift_value_keeps_member_indent() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_exact(
            fixture!(
                "void f(void)",
                "{",
                "\tstruct Item item = {",
                "\t\t.first = &data->value,",
                "\t\t.shifted = 1UL << data->alignment,",
                "\t\t.last = data->kind",
                "\t};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    struct Item item = {",
            "        .first = &data->value,",
            "        .shifted = 1UL << data->alignment,",
            "        .last = data->kind",
            "    };",
            "}",
        )
    );
}

#[test]
fn designated_initializer_rows_preserve_explicit_source_indent() {
    let source = fixture!(
        "static const struct Item items[] = {",
        "    [ITEM_ALPHA] = {",
        "        .value = 1,",
        "    },",
        "[ITEM_BETA] = {",
        "    .value = 2,",
        "},",
        "};",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn range_designator_after_run_in_members_keeps_member_indent() {
    let source = fixture!(
        "static const int values[256] = {",
        "    ['0'] = 0, ['1'] = 1,",
        "    [0 ... 47] = -1, [58 ... 64] = -1",
        "};",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn macro_initializer_rows_keep_member_indent_after_multiline_element() {
    let source = fixture!(
        "static const struct Entry entries[] = {",
        "    ITEM_SIZE(GROUP, \"batch\",",
        "              OPT_BATCH, \"N\",",
        "              \"batch size\", value),",
        "    ITEM(GROUP, \"server\",",
        "         OPT_SERVER, \"PATH\",",
        "         \"server socket\"),",
        "    ITEM(GROUP, \"client\",",
        "         OPT_CLIENT, \"PATH\",",
        "         \"client socket\"),",
        "};",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn multiline_define_run_in_designated_initializer_rows_align_as_siblings() {
    assert_eq!(
        format_exact(
            fixture!(
                "#define DEFINE_ITEM(func, name, ...) \\",
                "    static void func(struct Item *item); \\",
                "    const struct Entry CONCAT(func, __LINE__) = { .name = NAME(name), \\",
                "                                                 .func = func, __VA_ARGS__ }; \\",
                "    static void func(struct Item *item)",
            ),
            &kr_c_options(),
        ),
        fixture!(
            "#define DEFINE_ITEM(func, name, ...) \\",
            "    static void func(struct Item *item); \\",
            "    const struct Entry CONCAT(func, __LINE__) = { .name = NAME(name), \\",
            "                                                  .func = func, __VA_ARGS__ }; \\",
            "    static void func(struct Item *item)",
        )
    );
}

#[test]
fn local_struct_initializer_rows_keep_indent_after_multiline_element() {
    let source = fixture!(
        "TEST(values_layout)",
        "{",
        "    struct value_entry {",
        "        enum state state;",
        "        const char *label;",
        "        value_t *value;",
        "    } items[] = {",
        "        { READY, \"ready\", &ready },",
        "        { SKIPPED, \"skipped\", &skipped },",
        "        {",
        "            DEFERRED,",
        "            \"deferred\",",
        "            &deferred",
        "        },",
        "        { BUSY, \"busy\", &busy },",
        "        { INVALID, \"invalid\", &invalid },",
        "        {",
        "            FAILED,",
        "            \"failed\",",
        "            &failed",
        "        },",
        "        { EXPIRED, \"expired\", &expired },",
        "        { STOPPED, \"stopped\", &stopped },",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn compound_literal_members_preserve_source_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    call(&(uint32_t) {",
        "        0",
        "    });",
        "    memcpy(dst, (uint8_t[]) {",
        "        0x02, 0x00, 0x00",
        "    }, 3);",
        "    target->hop = (struct Hop) {",
        "        HOP_SERVICE, 1",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn nested_compound_literal_array_rows_stand_at_their_brace() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    struct Packet *packet = (struct Packet *)buffer;",
        "    *packet = (struct Packet) {",
        "        .source = {",
        "            .addr = {",
        "                0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0,",
        "                0, 0, 0, 0, 0, 0, 0, 0x0a",
        "            }",
        "        },",
        "        .size = sizeof(buffer),",
        "    };",
        "}",
    );

    // astyle stands the rows of a brace nested in a compound literal at the
    // brace.
    assert_eq!(
        format_exact(source, &one_true_brace_c_options()),
        fixture!(
            "void helper(void)",
            "{",
            "    struct Packet *packet = (struct Packet *)buffer;",
            "    *packet = (struct Packet) {",
            "        .source = {",
            "            .addr = {",
            "            0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0,",
            "            0, 0, 0, 0, 0, 0, 0, 0x0a",
            "            }",
            "        },",
            "        .size = sizeof(buffer),",
            "    };",
            "}",
        )
    );
}

#[test]
fn nested_compound_literal_range_elements_match_source_shape() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--mode=c".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            fixture!(
                "struct item items[] = {",
                "\t{",
                "\t\t.ranges = (const struct range[]) { { .start = 1, .end = 2 } },",
                "\t},",
                "\t{",
                "\t\t.ranges = (const struct range[]){",
                "\t\t\t{ .start = 1, .end = 2 },",
                "\t\t\t{ .start = 3, .end = 4 }",
                "\t\t},",
                "\t},",
                "};",
            ),
            &options,
        ),
        fixture!(
            "struct item items[] = {",
            "    {",
            "        .ranges = (const struct range[]) { { .start = 1, .end = 2 } },",
            "    },",
            "    {",
            "        .ranges = (const struct range[]){",
            "            { .start = 1, .end = 2 },",
            "            { .start = 3, .end = 4 }",
            "        },",
            "    },",
            "};",
        )
    );
}

// Subscripted compound literals keep the condition's structural column.
#[test]
fn compound_literal_subscript_in_condition_stays_consistent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "void f(void)\n{\n\twhile (j < (const int[]){ 64, 63 }[i]) {\n\t\tx;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    while (j < (const int[]) { 64, 63 }[i]) {\n        x;\n    }\n}\n",
    );
}

#[test]
fn empty_initializer_sentinel_preserves_member_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    const struct Entry entries[] = {",
        "        { \"alpha\", 0, 0, 'a' },",
        "        {}",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn array_value_rows_preserve_source_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    const uint8_t data[] = {",
        "        0xff, 0xff, 0xff, 0xff,",
        "        0x00",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &one_true_brace_c_options()), source);
}

#[test]
fn one_line_compound_literals_add_brace_gap_without_expanding() {
    let actual = format_exact(
        fixture!(
            "void helper(void)",
            "{",
            "    struct Item item = (struct Item){ .alpha = 1, .beta = 2 };",
            "    consume((struct Item){ .alpha = 1, .beta = 2 });",
            "}",
        ),
        &one_true_brace_c_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void helper(void)",
            "{",
            "    struct Item item = (struct Item) { .alpha = 1, .beta = 2 };",
            "    consume((struct Item) { .alpha = 1, .beta = 2 });",
            "}",
        )
    );
}

// Compound literals and aggregate initializers share one-line policy.
#[test]
fn one_line_compound_literal_return_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "int helper(void)\n{\n\treturn (struct Config) { (alpha | ((u64)beta << 32)) & MASK };\n}\n",
            &options,
        ),
        "int helper(void)\n{\n    return (struct Config) { (alpha | ((u64)beta << 32)) & MASK };\n}\n",
    );
}

#[test]
fn one_line_compound_literal_assignment_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "void store(struct Config *dst)\n{\n\t*dst = (struct Config) { .fn = alpha, .arg = beta, .id = GAMMA, };\n}\n",
            &options,
        ),
        "void store(struct Config *dst)\n{\n    *dst = (struct Config) { .fn = alpha, .arg = beta, .id = GAMMA, };\n}\n",
    );
}

#[test]
fn compound_literal_assignment_in_braceless_if_body_stays_inline() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  if (c)\n    result = (sample_rectangle_id_t) { 0, 0, 0, 0 };\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (c)\n        result = (sample_rectangle_id_t) { 0, 0, 0, 0 };\n}\n",
    );
}

#[test]
fn one_line_compound_literal_call_argument_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "void call(void)\n{\n\tconsume((struct Config){ 1, 2 });\n}\n",
            &options,
        ),
        "void call(void)\n{\n    consume((struct Config) { 1, 2 });\n}\n",
    );
}

#[test]
fn macro_call_compound_literal_argument_uses_body_indent() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  CALL_CHECKER (createSampleValue, get_record (data->record),\n                &(SampleItemCreateState) {\n                  .state = MODE,\n                  .value = out,\n                });\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    CALL_CHECKER (createSampleValue, get_record (data->record),\n    &(SampleItemCreateState) {\n        .state = MODE,\n        .value = out,\n    });\n}\n",
    );
}

#[test]
fn nested_array_compound_literal_body_elements_use_body_indent() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  call (a,\n        (float [4]) { 1, 1, 1, 1 },\n        (ColorId[4]) {\n    { 0, 0, 0, 0.75 },\n    { 0, 0, 0, 0.75 },\n  });\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    call (a,\n          (float [4]) { 1, 1, 1, 1 },\n    (ColorId[4]) {\n        { 0, 0, 0, 0.75 },\n        { 0, 0, 0, 0.75 },\n    });\n}\n",
    );
}

#[test]
fn multiline_compound_literal_closing_brace_splits_from_last_value() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  records = make_value_table ((const char *[]) {\n    \"a\",\n    NONE});\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    records = make_value_table ((const char *[]) {\n        \"a\",\n        NONE\n    });\n}\n",
    );
}

#[test]
fn nested_compound_literal_inside_aggregate_initializer_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "struct Item values[] = {\n\t[0] = {\n\t\t.labels = (const char*[]) { \"a\", \"b\" },\n\t\t.nested = (struct Item) { 1, 2 },\n\t},\n};\n",
            &options,
        ),
        "struct Item values[] = {\n    [0] = {\n        .labels = (const char*[]) { \"a\", \"b\" },\n        .nested = (struct Item) { 1, 2 },\n    },\n};\n",
    );
}

#[test]
fn empty_compound_literal_stays_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "void reset(struct Config *dst)\n{\n\t*dst = (struct Config) {};\n}\n",
            &options,
        ),
        "void reset(struct Config *dst)\n{\n    *dst = (struct Config) {};\n}\n",
    );
}

#[test]
fn one_line_compound_literal_kept_inline_with_keep_one_line_blocks() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--keep-one-line-blocks"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_exact(
            "int helper(void)\n{\n\treturn (struct Config) { alpha, beta };\n}\n",
            &options,
        ),
        "int helper(void)\n{\n    return (struct Config) { alpha, beta };\n}\n",
    );
}

#[test]
fn designated_initializer_compound_literal_keeps_attached_brace() {
    let source = "static const Item item =\n{\n    .fallbacks = (Format[]) {\n        FORMAT_X,\n    },\n};\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

// Designated-initializer context does not change compound-literal brace attachment.
#[test]
fn compound_literal_cast_braces_stay_attached_in_designated_initializers() {
    let source = fixture!(
        "const struct Group group = {",
        "    .name = NAME(\"group\"),",
        "    .options = (const struct Option *const[]) {",
        "        &(const struct Option) {",
        "            .name = NAME(\"value\"),",
        "        },",
        "        NULL",
        "    },",
        "    .defaults = &(const struct Config) {",
        "        .enabled = true,",
        "    },",
        "};",
    );

    assert_eq!(format_exact(source, &kr_c_options()), source);
}

#[test]
fn indented_styles_place_compound_literal_braces_at_body_indent() {
    let source = fixture!(
        "void make() {",
        "Item value=(Item) {",
        ".x=1,",
        ".y=2",
        "};",
        "}",
    );

    let mut whitesmith = FormatOptions::default();
    whitesmith.brace_style = BraceStyle::Whitesmith;
    whitesmith.indent_braces = true;
    whitesmith.indent_classes = true;
    whitesmith.indent_switches = true;
    assert_eq!(
        format_exact(source, &whitesmith),
        fixture!(
            "void make()",
            "    {",
            "    Item value=(Item)",
            "        {",
            "        .x=1,",
            "        .y=2",
            "        };",
            "    }",
        )
    );

    let mut vtk = FormatOptions::default();
    vtk.brace_style = BraceStyle::Vtk;
    assert_eq!(
        format_exact(source, &vtk),
        fixture!(
            "void make()",
            "{",
            "    Item value=(Item)",
            "        {",
            "        .x=1,",
            "        .y=2",
            "        };",
            "}",
        )
    );

    let mut ratliff = FormatOptions::default();
    ratliff.brace_style = BraceStyle::Ratliff;
    ratliff.indent_braces = true;
    ratliff.indent_classes = true;
    assert_eq!(
        format_exact(source, &ratliff),
        fixture!(
            "void make() {",
            "    Item value=(Item) {",
            "        .x=1,",
            "        .y=2",
            "        };",
            "    }",
        )
    );
}

#[test]
fn run_in_styles_keep_compound_designators_aligned() {
    let source = fixture!(
        "void make() {",
        "Item value=(Item) {",
        ".x=1,",
        ".y=2",
        "};",
        "}",
    );

    // Sibling compound-literal designators share one run-in body column.
    let mut horstmann = FormatOptions::default();
    horstmann.brace_style = BraceStyle::Horstmann;
    horstmann.indent_switches = true;
    assert_eq!(
        format_exact(source, &horstmann),
        fixture!(
            "void make()",
            "{   Item value=(Item)",
            "    {   .x=1,",
            "        .y=2",
            "    };",
            "}",
        )
    );

    let mut pico = FormatOptions::default();
    pico.brace_style = BraceStyle::Pico;
    pico.break_one_line_blocks = false;
    pico.break_one_line_statements = false;
    pico.indent_switches = true;
    assert_eq!(
        format_exact(source, &pico),
        fixture!(
            "void make()",
            "{   Item value=(Item)",
            "    {   .x=1,",
            "        .y=2 }; }",
        )
    );
}

#[test]
fn allman_breaks_nested_typed_initializer_braces() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;

    // Nested and outer typed initializer braces follow the same break style.
    assert_eq!(
        format_exact(
            fixture!(
                "Item make()",
                "{",
                "return Item {",
                "Inner {",
                "1",
                "}",
                "};",
                "}",
            ),
            &options,
        ),
        fixture!(
            "Item make()",
            "{",
            "    return Item",
            "    {",
            "        Inner",
            "        {",
            "            1",
            "        }",
            "    };",
            "}",
        )
    );
}

#[test]
fn ratliff_nested_return_initializer_closes_at_initializer_body_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Ratliff;
    options.indent_braces = true;

    assert_eq!(
        format_exact(
            fixture!("Item make() {", "return Item {", "1,", "2", "};", "}",),
            &options,
        ),
        fixture!(
            "Item make() {",
            "    return Item {",
            "        1,",
            "        2",
            "        };",
            "    }",
        )
    );
}

#[test]
fn split_aggregate_braces_preserve_source_indentation_by_default() {
    let options = FormatOptions::default();

    assert_eq!(
        format_exact(
            fixture!(
                "void f(void)",
                "{",
                "    int values[] =",
                "    {",
                "        1, 2",
                "    };",
                "}",
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    int values[] =",
            "    {",
            "        1, 2",
            "    };",
            "}",
        )
    );

    assert_eq!(
        format_exact(
            fixture!(
                "void f(void)",
                "{",
                "    Item v = (Item)",
                "    {",
                "        .x = 1",
                "    };",
                "}",
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    Item v = (Item)",
            "    {",
            "        .x = 1",
            "    };",
            "}",
        )
    );
}

#[test]
fn empty_decltype_braced_call_argument_stays_inline() {
    let source = "void f()\n{\n    CHECK_EQ(values, decltype(values) {});\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn templated_empty_braced_temporary_callable_stays_inline() {
    let source = fixture!(
        "void f() {",
        "    const auto hash = std::hash<string_t> {}(value);",
        "}",
    );

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn template_braced_initializer_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            "void f()\n{\n    index(List<int>{0, 0}, 0);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    index(List<int> {0, 0}, 0);\n}\n",
    );
}

#[test]
fn return_template_direct_list_initializer_has_space_before_brace() {
    assert_eq!(
        format_exact(
            fixture!(
                "void f()",
                "{",
                "    return std::vector<Item>{first, last};",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    return std::vector<Item> {first, last};",
            "}",
        )
    );
}

#[test]
fn macro_argument_template_braced_initializer_gets_space_before_brace() {
    assert_eq!(
        format_exact(
            "#define API_TEST(name, expr) expr\nAPI_TEST(path, index(List<int>{0, 0}, 0))\n",
            &FormatOptions::default(),
        ),
        "#define API_TEST(name, expr) expr\nAPI_TEST(path, index(List<int> {0, 0}, 0))\n",
    );
}

#[test]
fn templated_empty_braced_temporary_last_call_arg_stays_inline() {
    let source = fixture!("void f() {", "    g(seq<int> {});", "}",);

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn braced_temporary_call_argument_after_condition_splits_closing_call() {
    let source = "void f() {\n  if (a ||\n      b) {\n    out = write(out,\n                Result<T>{items, items + 1,\n                          static_cast<uint32_t>(v)});\n  }\n}\n";
    let expected = "void f() {\n    if (a ||\n            b) {\n        out = write(out,\n                    Result<T> {items, items + 1,\n                               static_cast<uint32_t>(v)\n                              });\n    }\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), expected,);
}

#[test]
fn long_braced_temporary_argument_after_condition_keeps_element_indent() {
    let source = "void f() {\n  if (a ||\n      b) {\n    out = write_escaped_cp(out,\n                           find_escape_result<Char>{v_array, v_array + 1,\n                                                     static_cast<uint32_t>(v)});\n  }\n}\n";
    let expected = "void f() {\n    if (a ||\n            b) {\n        out = write_escaped_cp(out,\n                               find_escape_result<Char> {v_array, v_array + 1,\n                                       static_cast<uint32_t>(v)\n                                                        });\n    }\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), expected,);
}

#[test]
fn multiline_array_first_element_brace_preserves_source_space() {
    let source = fixture!("int x[] = { { 1, 2 },", "    { 3, 4 }", "};",);

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn default_array_initializer_keeps_run_in_first_element_brace() {
    let source = "int a[][2] =\n{   {1, 2},\n    {3, 4}\n};\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn default_array_initializer_expands_adjacent_run_in_first_element_brace_gap() {
    assert_eq!(
        format_exact(
            "int a[][2] =\n{{1, 2},\n {3, 4}\n};\n",
            &FormatOptions::default(),
        ),
        "int a[][2] =\n{   {1, 2},\n    {3, 4}\n};\n",
    );
}

#[test]
fn multiline_array_first_element_brace_preserves_absent_source_space() {
    let source = fixture!("int x[] = {{ 1, 2 },", "    { 3, 4 }", "};",);

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn one_line_initializer_on_own_line_stays_one_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "static int values[] =\n{[0 ... 3] = -1};\nvoid f(void)\n{\n\tstatic unsigned short patterns[] =\n\t{0x0000, 0xffff};\n}\n",
            &options,
        ),
        "static int values[] =\n{[0 ... 3] = -1};\nvoid f(void)\n{\n    static unsigned short patterns[] =\n    {0x0000, 0xffff};\n}\n",
    );
}

#[test]
fn compound_literal_member_after_call_keeps_body_indent() {
    // Sibling call elements share the compound-literal body column.
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "struct r f(void)\n{\n\treturn (struct r) {\n\t\tget_symbol(mod, A, section),\n\t\tget_symbol(mod, B, section),\n\t};\n}\n",
            &options,
        ),
        "struct r f(void)\n{\n    return (struct r) {\n        get_symbol(mod, A, section),\n        get_symbol(mod, B, section),\n    };\n}\n",
    );
}

// A completed shift expression does not own the following initializer element.
#[test]
fn initializer_element_after_shift_keeps_consistent_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "void f(void)\n{\n\tstruct s c = {\n\t\t.a = x,\n\t\t.b = 1 << n,\n\t\t.c = y\n\t};\n}\n",
            &options,
        ),
        "void f(void)\n{\n    struct s c = {\n        .a = x,\n        .b = 1 << n,\n        .c = y\n    };\n}\n",
    );
}
#[test]
fn range_for_braced_init_list_indents_from_control_paren_continuation() {
    // A braced-init-list used as the range expression of a for-statement is
    // indented from the control-paren continuation level (two indents past the
    // `for`), not the statement base. Content lands two levels past the closing
    // brace, which itself aligns with the for-paren continuation.
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=stroustrup".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "void f() {\n    for (auto s : {\n        \"alpha\", \"beta\", \"gamma\"\n    }) {\n        call(s);\n    }\n}\n",
            &options,
        ),
        "void f()\n{\n    for (auto s : {\n                \"alpha\", \"beta\", \"gamma\"\n            }) {\n        call(s);\n    }\n}\n"
    );
}

#[test]
fn range_for_braced_init_list_on_own_line_attaches_to_colon_header() {
    // When the range-for braced-init opener starts its own source line after the
    // `:`, it attaches to the header line and the list indents from the control-
    // paren continuation, matching the same-line form.
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=stroustrup".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "void f() {\n    for (auto s :\n    {\n        \"alpha\", \"beta\"\n    }) {\n        call(s);\n    }\n}\n",
            &options,
        ),
        "void f()\n{\n    for (auto s : {\n                \"alpha\", \"beta\"\n            }) {\n        call(s);\n    }\n}\n"
    );
}

#[test]
fn indented_brace_styles_indent_nested_initializer_braces() {
    let input = "static char values[A][B] = {\n\t/* row */\n\t{1, 2},\n};\n";
    let expected = "static char values[A][B] = {\n    /* row */\n        {1, 2},\n    };\n";

    for style in ["whitesmith", "ratliff"] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(format_exact(input, &options), expected);
    }
}

#[test]
fn indented_brace_styles_align_nested_initializer_rows_to_inner_brace() {
    let input = fixture!("static Item items[]={", "{", "1,", "2,", "},", "};",);
    let cases = [
        (
            "whitesmith",
            fixture!(
                "static Item items[]= {",
                "        {",
                "        1,",
                "        2,",
                "        },",
                "    };",
            ),
        ),
        (
            "vtk",
            fixture!(
                "static Item items[]= {",
                "        {",
                "        1,",
                "        2,",
                "        },",
                "};",
            ),
        ),
        (
            "ratliff",
            fixture!(
                "static Item items[]= {",
                "        {",
                "        1,",
                "        2,",
                "        },",
                "    };",
            ),
        ),
    ];

    for (style, expected) in cases {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")]).expect("valid style");

        assert_eq!(format_exact(input, &options), expected, "{style}");
    }
}

#[test]
fn nested_designated_initializer_keeps_member_indent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--unpad-paren",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nvoid f(void)\n{\n    state = (struct State) {\n        .result = {\n            .path = PATH_ALPHA,\n            .chain = CHAIN_NONE,\n        },\n    };\n}\n";

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn macro_initializer_keeps_designated_initializer_indent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--indent-preprocessor",
        "--indent-preproc-define",
        "--pad-oper",
        "--align-pointer=name",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\n#define ITEM_DECL(__func, __name) \\\n    static const struct item CONCAT(__func, __LINE__) \\\n        = { \\\n            .func = (__func), \\\n            .name = (__name), \\\n          }; \\\n    int __func(int argc, char **argv)\n";

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn default_breaks_nonempty_named_direct_list_in_auto_declaration() {
    assert_eq!(
        format_exact("auto value = Type{1};\n", &FormatOptions::default()),
        fixture!("auto value = Type {", "    1", "};")
    );
}

#[test]
fn default_spaces_empty_named_direct_list_in_auto_declaration() {
    assert_eq!(
        format_exact("auto value = Type{};\n", &FormatOptions::default()),
        "auto value = Type {};\n"
    );
}

#[test]
fn default_keeps_named_direct_list_in_local_auto_declaration() {
    let source = fixture!("void function()", "{", "    auto value = Type{1};", "}",);

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn braced_init_block_body_indents_one_level() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source = "void f()\n{\n\tConfig names{\n\t\tcall(a),\n\t\tcall(b),\n\t\tcall(c),\n\t};\n\treturn x;\n}\n";

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn multiline_nested_designated_init_brace_keeps_source_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_exact(
            "Config table[] = {\n[A] = {\n.value=0,\n.list={\n[X]=1,\n[Y]=2\n},\n.count=0\n}\n};\n",
            &options,
        ),
        "Config table[] = {\n\t[A] = {\n\t\t.value=0,\n\t\t.list={\n\t\t\t[X]=1,\n\t\t\t[Y]=2\n\t\t},\n\t\t.count=0\n\t}\n};\n",
    );
}

#[test]
fn multiline_run_in_initializer_closing_brace_breaks_to_own_line() {
    assert_eq!(
        format_exact(
            "\nvoid foo(void)\n{\n    static char *names[] = { \"A\", \"B\",\n                             \"C\", \"D\" };\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo(void)\n{\n    static char *names[] = { \"A\", \"B\",\n                             \"C\", \"D\"\n                           };\n}\n",
    );
}

#[test]
fn nested_designated_initializer_run_in_braces_expand() {
    assert_eq!(
        format_exact(
            "\nstruct Config value = {\n    .outer = { .inner = {\n        .bits = VALUE\n    }}\n};\n",
            &FormatOptions::default(),
        ),
        "\nstruct Config value = {\n    .outer = {\n        .inner = {\n            .bits = VALUE\n        }\n    }\n};\n",
    );
}

#[test]
fn compound_literal_argument_body_uses_body_indent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--indent=spaces=4",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
        "--align-reference=name",
        "--max-continuation-indent=80",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "void f(void)\n{\n    call(&worker,\n    &(struct Data) {\n        .id = (uint32_t)item->id,\n        .hash = item->hash,\n        .cookie = 0x42,\n        .status = 0,\n    });\n}\n";

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn nested_call_compound_literal_argument_body_uses_body_indent() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--indent=spaces=4",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
        "--align-reference=name",
        "--max-continuation-indent=80",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_exact(
            "void f(void)\n{\n    assert_true(process_pending_item(\n    context.queue,\n    &(struct Data) {\n        .id = (uint32_t)item->id,\n        .hash = item->hash,\n        .cookie = 0x2ab,\n        .status = STATUS_FULL,\n    }));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    assert_true(process_pending_item(\n                    context.queue,\n    &(struct Data) {\n        .id = (uint32_t)item->id,\n        .hash = item->hash,\n        .cookie = 0x2ab,\n        .status = STATUS_FULL,\n    }));\n}\n",
    );
}

#[test]
fn nested_array_brace_with_first_element_breaks_to_structural_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "struct v t[] = {\n\t{\n\t\t.p = { 1, 2,\n\t\t\t3, 4 },\n\t\t.valid = 1\n\t},\n};\n",
            &options,
        ),
        "struct v t[] = {\n    {\n        .p = {\n            1, 2,\n            3, 4\n        },\n        .valid = 1\n    },\n};\n",
    );
}

#[test]
fn run_in_brace_opening_multiline_struct_keeps_no_space_and_indents_per_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact("struct s a[] = {{\n\t.x = 1\n}};\n", &options),
        "struct s a[] = {{\n        .x = 1\n    }\n};\n",
    );
    assert_eq!(
        format_exact(
            "struct s a[] = {{\n\t.in = \"x\",\n\t.s1 = {{\n\t\t\t.out = \"y\",\n\t\t},{\n\t\t\t.out = \"z\",\n\t\t}\n\t}\n},{\n\t.in = \"w\",\n}};\n",
            &options,
        ),
        "struct s a[] = {{\n        .in = \"x\",\n        .s1 = {{\n                .out = \"y\",\n            },{\n                .out = \"z\",\n            }\n        }\n    },{\n        .in = \"w\",\n    }\n};\n",
    );
}

#[test]
fn one_line_brace_elements_after_comma_keep_source_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact("int a[] = {\n\t{1,2},{3,4},{5,6}\n};\n", &options),
        "int a[] = {\n    {1,2},{3,4},{5,6}\n};\n",
    );
}

#[test]
fn array_open_brace_with_trailing_comment_indents_body_one_level() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "static const int a[] = {    /* note */\n\t1, 2, 3,\n\t4, 5, 6\n};\n",
            &options,
        ),
        "static const int a[] = {    /* note */\n    1, 2, 3,\n    4, 5, 6\n};\n",
    );
}

#[test]
fn array_element_brace_keeps_trailing_run_in_comment_on_brace_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "static struct x arr[] = {\n\t{\t/* one */\n\t\t1, 2,\n\t\t3 },\n};\n",
            &options,
        ),
        "static struct x arr[] = {\n    {\t/* one */\n        1, 2,\n        3\n    },\n};\n",
    );
}

// Completed shift expressions do not change later array-element indentation.
#[test]
fn array_elements_after_shift_operator_keep_structural_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "static const unsigned long arr[] = {\n\t0x1,\n\t0x2 << 12,\n\t0x3,\n\t0x4,\n};\n",
            &options,
        ),
        "static const unsigned long arr[] = {\n    0x1,\n    0x2 << 12,\n    0x3,\n    0x4,\n};\n",
    );
}

#[test]
fn designated_init_member_resets_indent_after_multiline_macro_call() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "struct t arr[] = {\n\t{\n\t\t.desc = \"x\",\n\t\tPBUF(0x00, 0x01,\n\t\t     0x02, 0x03),\n\t\t.uval = 5,\n\t\t.start_bit = 95,\n\t},\n};\n",
            &options,
        ),
        "struct t arr[] = {\n    {\n        .desc = \"x\",\n        PBUF(0x00, 0x01,\n             0x02, 0x03),\n        .uval = 5,\n        .start_bit = 95,\n    },\n};\n",
    );
}

#[test]
fn broken_array_closing_brace_keeps_source_gap_before_trailing_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_exact(
            "static const int a[] = {\n    1, 2,\n    3, 99}; /* note */\n",
            &options,
        ),
        "static const int a[] = {\n    1, 2,\n    3, 99\n}; /* note */\n",
    );
}

#[test]
fn inline_compound_literal_does_not_leak_into_later_parameter_list() {
    let source = "static const Item it =\n{\n    .f = (T[]) { -1, },\n};\n\nint\nfn (int  format,\n    int *out_format,\n    int *sw)\n{\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn nested_compound_literal_single_element_close_brace_aligns_with_statement() {
    let source = "static const Item items[] =\n{\n    {\n        .fallbacks = (Format[]) {\n            FORMAT_X,\n        },\n    },\n};\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn braced_string_list_second_element_in_call_aligns_to_call_argument_column() {
    assert_eq!(
        format_exact(
            "void f()\n{\n    CONFIRM(sample_runner.runCommand(\"cmd\", QStringList{\"/c\",\n                                                        \"echo.>>\" + Path::toNativeSeparators(baseDir + \"/version.inc\")}));\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    CONFIRM(sample_runner.runCommand(\"cmd\", QStringList{\"/c\",\n                                     \"echo.>>\" + Path::toNativeSeparators(baseDir + \"/version.inc\")}));\n}\n",
    );
}

#[test]
fn call_arguments_after_split_compound_literal_use_body_indent() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  expression = build_expression_value_node (VALUE_TYPE,\n                                            NULL,\n                                            1, (Expression *[1]) { expression },\n                                            CALLBACK (convert_value_to_text),\n                                            NULL, NULL);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    expression = build_expression_value_node (VALUE_TYPE,\n                 NULL,\n                 1, (Expression *[1]) { expression },\n                 CALLBACK (convert_value_to_text),\n                 NULL, NULL);\n}\n",
    );
}

#[test]
fn split_compound_literal_call_argument_uses_body_indent() {
    assert_eq!(
        format_exact(
            "void f(void)\n{\n  call (arg,\n        &(const T) {\n          x, y\n        },\n        -1);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    call (arg,\n    &(const T) {\n        x, y\n    },\n    -1);\n}\n",
    );
}
#[test]
fn compound_literal_array_arg_operator_first_element_keeps_sibling_column() {
    let source = "void f(void)\n{\n    init (matrix, (float[4]) {\n        1.0 - (1.0 - R) * value, R * value,\n        G * value, 0.0\n    });\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}
#[test]
fn designated_init_compound_literal_array_body_indents_from_field() {
    let source = "void f(void)\n{\n    CALL (device,\n    &(CreateInfo) {\n        .sType = TYPE,\n        .pBindings = (Binding[1]) {\n            {\n                .binding = 0,\n                .count = n,\n            }\n        },\n    },\n    NULL);\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn designated_initializer_rows_ignore_overindented_source_continuation() {
    assert_eq!(
        format_exact(
            "void f()\n{\n\tConfig value{.alpha={1}, .beta={2}, .gamma=3,\n\t              .zeta=6, .eta=7,\n\t              .theta=8};\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Config value{.alpha={1}, .beta={2}, .gamma=3,\n                 .zeta=6, .eta=7,\n                 .theta=8};\n}\n",
    );
}

#[test]
fn compound_literal_array_first_braced_call_element_stays_consistent() {
    let source = "void f()\n{\n    call((T[2]) {\n        { first(0.0, progress - 5.0 / width), { 1, 1, 1, 1 } },\n        { second(1.0, progress + 5.0 / width), { 0, 0, 0, 1 } }\n    }, 2);\n}\n";

    assert_eq!(format_exact(source, &FormatOptions::default()), source);
}

#[test]
fn operator_empty_block_continues_expression() {
    let options = FormatOptions::default();

    assert_eq!(format_exact("%{}\n", &options), "% {}\n");
    assert_eq!(format_exact("x %{} y\n", &options), "x % {} y\n");
    assert_eq!(format_exact("x %{}+y\n", &options), "x % {}+y\n");
    assert_eq!(format_exact("x +{} y\n", &options), "x + {} y\n");
}

#[test]
fn expression_brace_hash_body_is_not_hoisted() {
    let options = FormatOptions::default();

    assert_eq!(
        format_exact("x % {#endif} y\n", &options),
        "x % {#endif} y\n",
    );
    assert_eq!(format_exact("x={#endif}\n", &options), "x= {#endif}\n");
}

#[test]
fn allman_malformed_unary_brace_body_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let first = format_exact("!=x{!=y\n\n)z.}\n", &options);

    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn allman_malformed_trailing_operator_before_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = "19switchcatch/voidcallvalueelse||defaultcallbreakif#endif|{classItem-tryvalue==/:throwbeta116constexpr(!beta8.while~resulttryint::3131\t?11class40auto:throwItem,namespacebreak16\tdo->{%\t<=>throw>whileauto?valuewhile13enumalpha41<=&&||callvaluecatch-int=value\n}20%<=>39}37::Item)#if Acase// line>=>catchvoid)|case\n";
    let first = format_exact(input, &options);

    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn allman_malformed_operator_initializer_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let input = "throwhelperthrow[continueswitch#if Aswitch->betagamma?constexprdo<=>alpha?elsegamma||if<=>{alpha<=namespace?\t&&constexpr~void[betagamma+resultbreak/* block */switch?</* block */tryreturn||~catch]helper!.valuethrowvalue\n\nauto==={value<-\n";
    let first = format_exact(input, &options);

    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn default_malformed_operator_brace_body_indent_is_idempotent() {
    let options = FormatOptions::default();
    let input = "+::\nwhile   ydefault  !=Config\n=Config\n,if\t{  42\tnamespacezItem\t#else\n,z\nConfigdo<=z\n->;\n(constexpr\n<=\t{switch  42\t%struct? NULL\nfor\n";
    let first = format_exact(input, &options);

    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn gnu_standalone_colon_keeps_broken_brace_indent_idempotent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected = b":\n        {";

    assert_eq!(
        format_bytes(b":{", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn broken_malformed_brace_uses_inline_initializer_scope_on_first_pass() {
    for (style, expected) in [
        ("allman", b"g{(\n  {\n      2".as_slice()),
        ("gnu", b"g{(\n  {\n      2".as_slice()),
        ("vtk", b"g{(\n  {\n  2".as_slice()),
        ("lisp", b"g{(\n  {\n      2".as_slice()),
    ] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[format!("--style={style}")])
            .expect("valid options");

        assert_eq!(
            format_bytes(b"g{(\n{2", &options).expect("format bytes"),
            expected,
        );
        assert_eq!(
            format_bytes(expected, &options).expect("format bytes"),
            expected,
        );
    }
}

#[test]
fn gnu_nested_malformed_colon_brace_uses_active_scope_on_first_pass() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let expected = b"g{:\n  {";

    assert_eq!(
        format_bytes(b"g{:{", &options).expect("format bytes"),
        expected,
    );
    assert_eq!(
        format_bytes(expected, &options).expect("format bytes"),
        expected,
    );
}

#[test]
fn broken_brace_styles_keep_designated_initializer_layout() {
    let allman = "int x;\nstatic struct p b =\n{\n    .d = 1,\n    .m = {\n        N_(\"a\"),\n        N_(\"b\")\n    },\n    .e = 2,\n};\n";
    let whitesmith =
        "int x;\nstatic struct p b =\n    {\n    .d = 1,\n    .e = { 1, 2 },\n    };\n";
    for (style, source) in [
        ("--style=allman", allman),
        ("--style=whitesmith", whitesmith),
    ] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[style.to_owned()]).expect("valid options");

        assert_eq!(format_exact(source, &options), source, "{style}");
    }
}

#[test]
fn initializer_rows_after_block_comments_keep_their_indent() {
    let mut allman = FormatOptions::default();
    apply_command_line_args(&mut allman, &["--style=allman".to_owned()]).expect("valid options");
    let allman_source =
        "static T t[] =\n{\n    /*\n     * a\n     */\n\n    {\n        \"opera\",\n    },\n};\n";

    let mut whitesmith = FormatOptions::default();
    apply_command_line_args(&mut whitesmith, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let whitesmith_source = "static T t[] =\n    {\n        { A, \"a\" },\n    /* (many different) strings:\n\n       B\n     */\n        { B, \"b\" },\n    };\n";

    assert_eq!(format_exact(allman_source, &allman), allman_source);
    assert_eq!(
        format_exact(whitesmith_source, &whitesmith),
        whitesmith_source
    );
}

#[test]
fn designated_element_after_a_comment_keeps_the_element_column() {
    let options = options_from_args(&["--style=kr"]);
    let source = fixture!(
        "static const struct s t[] = {",
        "    [A] = {",
        "        .type = 1,",
        "    },",
        "    /*",
        "     * c",
        "     */",
        "    [B] = {",
        "        .type = 2,",
        "    },",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn whitesmith_indents_the_closing_brace_of_a_nested_designated_element() {
    let options = options_from_args(&["--style=whitesmith"]);
    let source = fixture!(
        "static const struct s t[] = {",
        "    [A] = {",
        "        .type = 1,",
        "        },",
        "    [B] = {",
        "        .type = 2,",
        "        },",
        "    [C] = 3,",
        "    };",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_file_scope_nested_brace_rows_indent_their_elements() {
    let options = options_from_args(&["--style=vtk"]);
    let source = fixture!(
        "static const struct t l[] =",
        "{",
        "    /* c */",
        "    {",
        "        \"a\", \"\",",
        "        0",
        "    },",
        "    {",
        "        \"b\", \"\",",
        "        1",
        "    },",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_function_scope_nested_brace_rows_keep_elements_at_the_brace() {
    let options = options_from_args(&["--style=vtk"]);
    let source = fixture!(
        "void f(void)",
        "{",
        "    static const struct",
        "        {",
        "        const char *c;",
        "        } tests[] =",
        "        {",
        "            { \"a\", 1 },",
        "            {",
        "            \"b\",",
        "            2",
        "            },",
        "            { \"c\", 3 },",
        "        };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn comment_and_element_after_a_split_element_keep_the_element_column() {
    let options = options_from_args(&["--style=kr"]);
    let source = fixture!(
        "void f(void)",
        "{",
        "    struct option o[] = {",
        "        OPT_FILENAME(0, \"b\", &s,",
        "                     N_(\"build a temporary index based on embedded index information\")),",
        "        /* Think twice before adding \"--nul\" synonym to this */",
        "        OPT_SET_INT(1, NULL, &l,",
        "                    N_(\"x\"), 2),",
        "        OPT_END()",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_function_scope_initializer_rows_keep_values_at_the_brace_and_indent_brace_rows() {
    let options = options_from_args(&["--style=vtk"]);
    let source = fixture!(
        "int main(void)",
        "{",
        "    struct FtpFile ftpfile =",
        "        {",
        "        \"curl.tar.gz\", /* name */",
        "        NULL",
        "        };",
        "    static const struct testcase tests[] =",
        "        {",
        "            { \"a\", \"b\", 0 },",
        "            { \"c\", \"d\", 1 }",
        "        };",
        "    return 0;",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_file_scope_brace_rows_start_at_the_element_column() {
    let options = options_from_args(&["--style=vtk"]);
    let source = fixture!(
        "const struct helptxt helptext[] =",
        "{",
        "    {",
        "        \"a\",",
        "        \"b\",",
        "        1",
        "    },",
        "    {",
        "        \"c\",",
        "        \"d\",",
        "        2",
        "    },",
        "    { NULL, NULL, 0 }",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn whitesmith_brace_row_after_a_directive_stands_past_the_plain_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let source = fixture!(
        "static const struct f table[] = {",
        "    FEATURE(\"a\", NULL, 1),",
        "#ifdef X",
        "    FEATURE(\"b\", NULL, 2),",
        "#endif",
        "        {NULL, NULL, 0}",
        "    };",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_last_split_brace_row_after_a_one_line_row_stays_at_the_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "static const struct testparams testparams[] = {",
        "    { F_A,  CURLE_OK },",
        "    {",
        "        F_RESUME | F_HTTP416,",
        "        CURLE_RANGE_ERROR",
        "    },",
        "    { F_C,  CURLE_OK },",
        "    {",
        "        F_RESUME | F_HTTP416,",
        "        CURLE_OK",
        "    }",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_file_scope_array_brace_rows_stand_a_level_past_the_elements() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "static int codes[256][16] =",
        "{",
        "    /* 0 */",
        "        {",
        "            {0x04, 0}, {0x05, 0}",
        "        },",
        "        {",
        "            {0x04, 0}, {0x05, 0}",
        "        }",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_array_brace_rows_in_a_function_stand_a_level_past_their_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "void f(void)",
        "{",
        "    static int codes[256][16] =",
        "        {",
        "            {",
        "                {0x04, 0}, {0x05, 0}",
        "            },",
        "            {",
        "            0x04, 0x05",
        "            }",
        "        };",
        "    int a[] =",
        "        {",
        "        1, 2",
        "        };",
        "}",
        "int b[] =",
        "{",
        "    1, 2",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_compound_literal_rows_in_a_designated_initializer_stand_past_their_cast() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "void f(void)",
        "{",
        "    RedisModuleCommandInfo info =",
        "        {",
        "        .arity = -5,",
        "        .key_specs = (RedisModuleCommandKeySpec[])",
        "            {",
        "                {",
        "                .flags = 1,",
        "                .fk.range = {0,1,0}",
        "                },",
        "                {0}",
        "            },",
        "        };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn whitesmith_compound_literal_rows_in_a_designated_initializer_stand_past_their_cast() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let source = fixture!(
        "void f(void)",
        "    {",
        "    RedisModuleCommandInfo info =",
        "        {",
        "        .arity = -5,",
        "        .key_specs = (RedisModuleCommandKeySpec[])",
        "            {",
        "                {",
        "                .flags = 1,",
        "                .fk.range = {0,1,0}",
        "                },",
        "                {0}",
        "            },",
        "        };",
        "    }",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_rows_after_a_comment_on_their_line_keep_the_row_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "static const struct",
        "{",
        "    u8 nName;",
        "    char zName[7];",
        "} aXformType[] =",
        "{",
        "    /* 0 */ { 6, \"second\" },",
        "    /* 1 */ { 6, \"minute\" },",
        "    /* 2 */ { 4, \"hour\" },",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn whitesmith_rows_led_by_a_comment_stay_at_the_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let source = fixture!(
        "void f(void)",
        "    {",
        "    static const u8 trans[2][2] =",
        "        {",
        "        /* State: */",
        "        /* 0 */ { 1, 0, },",
        "        /* 1 */ { 1, 1, },",
        "        };",
        "    }",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn first_row_led_by_a_comment_stands_a_level_past_the_brace_line() {
    let options = FormatOptions::default();
    let source = fixture!(
        "void f(void)",
        "{",
        "    if (x) {",
        "    } else",
        "#ifdef A",
        "        if (y) {",
        "        } else",
        "#endif",
        "            if (z) {",
        "                const u8 aMoveOp[] = {",
        "                    /* TK_GT */  OP_SeekGT,",
        "                    /* TK_LE */  OP_SeekLE",
        "                };",
        "            }",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn element_continued_in_a_one_line_brace_stands_at_the_first_element() {
    let options = FormatOptions::default();
    let source = fixture!(
        "void f(void)",
        "{",
        "    const char *keys[] = { \"key1\", ignore_case ? \"FOObarFrotz\" :",
        "                           \"fooBarFrotz\"",
        "                         };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn leading_comma_after_a_conditional_element_stands_at_the_element() {
    let options = FormatOptions::default();
    let source = fixture!(
        "static const char * const disabled[] = {",
        "    \"bindlocal: \"",
        "#ifdef A",
        "    \"OFF\"",
        "#endif",
        "    ,",
        "    \"cookies: \"",
        "#ifdef B",
        "    \"OFF\"",
        "#endif",
        "    ,",
        "    NULL",
        "};",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn whitesmith_one_line_initializer_after_an_assignment_line_is_indented() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let source = fixture!(
        "static const char *a[] =",
        "    {\"index-pack\", \"--stdin\", NULL};",
        "void f(void)",
        "    {",
        "    static const char *c[] =",
        "        {\"call\", \"return\"};",
        "    }",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn call_arguments_in_an_array_row_stack_from_the_row() {
    let options = FormatOptions::default();
    let source = fixture!(
        "void f(void)",
        "{",
        "    x = {",
        "        OPT_CALLBACK_F(1, \"reflog\", &reflog_base, N_(\"<n>\"),",
        "                       N_(\"show <n> most recent ref-log entries starting at \"",
        "                          \"base\"),",
        "                       PARSE_OPT_OPTARG | PARSE_OPT_NONEG,",
        "                       parse_reflog_param),",
        "    };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn horstmann_rows_after_a_run_in_initializer_brace_stand_at_the_first_row() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    let source = fixture!(
        "void f(int c)",
        "{   switch (c)",
        "    {   case 1:",
        "        {   const struct decoration_options opts =",
        "            {   .prefix = (char *) \"\",",
        "                .suffix = (char *) \"\",",
        "            };",
        "            g();",
        "            break;",
        "        }",
        "    }",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn vtk_nested_initializer_in_code_closes_at_its_elements() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");
    let source = fixture!(
        "void f(void)",
        "{",
        "    struct path_walk_context ctx =",
        "        {",
        "        .repo = 1,",
        "        .path_stack = {",
        "            .compare = 2,",
        "            .cb_data = &ctx",
        "            },",
        "        .x = 3",
        "        };",
        "    struct option options[] =",
        "        {",
        "        OPT_X(1),",
        "            {",
        "            .type = 1,",
        "            .short_name = 2,",
        "            },",
        "        OPT_END()",
        "        };",
        "}",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn ratliff_brace_row_after_a_directive_is_indented_past_plain_rows() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");
    let source = fixture!(
        "static const struct feat features_table[] = {",
        "    FEATURE(\"a\", NULL, 1),",
        "#ifdef HAVE_ZSTD",
        "    FEATURE(\"zstd\", NULL, 2),",
        "#endif",
        "        {NULL, NULL, 0}",
        "    };",
    );

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn leading_comma_stands_at_the_row_of_elements_it_follows() {
    let options = FormatOptions::default();
    let source = "void f()\n{\n    size_t a[] = {\n        0\n        , 20\n    };\n    size_t b[] = {\n        0, 4\n        , 20\n    };\n}\n";

    assert_eq!(format_exact(source, &options), source);
}

#[test]
fn elements_after_a_leading_nested_brace_stand_a_level_in() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid style options");

    assert_eq!(
        format_exact(
            "void f(void)\n{\n    int b[] = {{1, 2},\n        3,\n4, {5},\n6\n    };\n}\n",
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    int b[] = {{1, 2},",
            "        3,",
            "        4, {5},",
            "        6",
            "    };",
            "}",
        )
    );
}

// Styles indenting braces close each compound literal a level past the line
// that opened it, the cast before the brace notwithstanding.
#[test]
fn indented_brace_styles_close_nested_compound_literals_at_their_fields() {
    for (style, function_brace) in [("whitesmith", "    "), ("vtk", "")] {
        let mut options = FormatOptions::default();
        apply_command_line_args(
            &mut options,
            &[format!("--style={style}"), "--mode=c".to_owned()],
        )
        .expect("valid options");
        assert_eq!(
            format_exact(
                "void f()\n{\n    y = (U){ .a = (V){\n        1 } };\n}\n",
                &options
            ),
            format!(
                "void f()\n{function_brace}{{\n    y = (U) {{\n        .a = (V){{\n            1\n            }}\n        }};\n{function_brace}}}\n"
            ),
            "{style}"
        );
    }
}

#[test]
fn enum_row_broken_off_by_its_closing_brace_keeps_the_element_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let input = "void f(void)\n{\n\tenum { LEFT,\n\t       CENTRE,\n\t       TOTAL } current = LEFT, last = LEFT;\n\tint x;\n}\n";
    let expected = "void f(void)\n{\n    enum { LEFT,\n           CENTRE,\n           TOTAL\n         } current = LEFT, last = LEFT;\n    int x;\n}\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn rows_after_a_closed_named_list_align_in_their_own_list() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let input = "enum e {\n};\nint s[] = {\n\t[A] = { 1 },\n\t[B] = { c,\n\t\td }\n};\n";
    let expected = "enum e {\n};\nint s[] = {\n    [A] = { 1 },\n    [B] = {\n        c,\n        d\n    }\n};\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn designated_row_closer_broken_off_its_row_closes_at_the_row_level() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--min-conditional-indent=0".to_owned()])
        .expect("valid options");
    let input = "static struct tr2_sysenv_entry tr2_sysenv_settings[] = {\n\t[TR2_SYSENV_CFG_PARAM]     = { \"GIT_TRACE2_CONFIG_PARAMS\",\n\t\t\t\t       \"trace2.configparams\" },\n\t[TR2_SYSENV_ENV_VARS]      = { \"GIT_TRACE2_ENV_VARS\",\n\t\t\t\t       \"trace2.envvars\" },\n};\n";
    let expected = "static struct tr2_sysenv_entry tr2_sysenv_settings[] = {\n    [TR2_SYSENV_CFG_PARAM]     = {\n        \"GIT_TRACE2_CONFIG_PARAMS\",\n        \"trace2.configparams\"\n    },\n    [TR2_SYSENV_ENV_VARS]      = {\n        \"GIT_TRACE2_ENV_VARS\",\n        \"trace2.envvars\"\n    },\n};\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn broken_row_brace_among_aligned_elements_stands_at_their_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let input = "void f()\n{\n\tconst char *key_val[][2] = { { \"key1\", \"value1\" },\n\t\t\t\t     { \"key2\", \"value2\" },\n\t\t\t\t     { c,\n\t\t\t\t       \"value4\" } };\n}\n";
    let expected = "void f()\n{\n    const char *key_val[][2] = { { \"key1\", \"value1\" },\n        { \"key2\", \"value2\" },\n        {\n            c,\n            \"value4\"\n        }\n    };\n}\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn initializer_elements_align_to_the_tab_expanded_brace_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");
    let input = "void f()\n{\n\tint\t\t mib[6] = { A, B, C, 0,\n\t\t\t\t    sizeof(struct k), 0 };\n\tx = 1;\n}\n";
    let expected = "void f()\n{\n    int\t\t mib[6] = { A, B, C, 0,\n                        sizeof(struct k), 0\n                   };\n    x = 1;\n}\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn broken_row_brace_among_aligned_rows_keeps_their_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");
    let input = "void f()\n{\n\tconst char *k[][2] = { { \"key1\", \"value1\" },\n\t\t\t     { \"key2\", \"value2\" },\n\t\t\t     { c ? \"key4\" : \"foo\",\n\t\t\t       \"value4\" } };\n\tx = 1;\n}\n";
    let expected = "void f()\n{\n    const char *k[][2] = { { \"key1\", \"value1\" },\n        { \"key2\", \"value2\" },\n        {\n            c ? \"key4\" : \"foo\",\n            \"value4\"\n        }\n    };\n    x = 1;\n}\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn nested_run_in_brace_runs_into_its_block_comment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");
    let input = "struct spec {\n  const char * const pem;\n  bool ok;\n};\n\nstatic const struct spec certs[] = {\n  { /* a valid certificate */\n    \"MIIB\",\n    TRUE },\n};\n";
    let expected = "struct spec\n{   const char * const pem;\n    bool ok; };\n\nstatic const struct spec certs[] =\n{   {   /* a valid certificate */\n        \"MIIB\",\n        TRUE }, };\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn inner_closer_split_from_the_outer_stands_among_the_rows() {
    let options = FormatOptions::default();
    let input = "void f(void)\n{\n\tstruct r r1[] = { {\n\t\t.refname = \"b\",\n\t\t.value = { 1, 2 },\n\t} };\n}\n";
    let expected = "void f(void)\n{\n    struct r r1[] = { {\n            .refname = \"b\",\n            .value = { 1, 2 },\n        }\n    };\n}\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
    let nested = "void f(void)\n{\n\tstruct reftable_record\n\t\trec = { .type = T,\n\t\t\t.u.obj = {\n\t\t\t\t.offsets = e,\n\t\t\t} };\n}\n";
    let first = format_exact(nested, &options);
    assert_eq!(format_exact(&first, &options), first);
}

#[test]
fn argument_brace_after_a_line_ending_in_a_comma_is_an_initializer() {
    let input = "void f(void)\n{\n    g(a,\n        {\n        free(n);\n    });\n    g(a,\n      { 1, 2 });\n}\n";
    for (style, expected) in [
        (
            "--style=vtk",
            "void f(void)\n{\n    g(a,\n        {\n        free(n);\n        });\n    g(a,\n    { 1, 2 });\n}\n",
        ),
        (
            "--style=kr",
            "void f(void)\n{\n    g(a, {\n        free(n);\n    });\n    g(a,\n    { 1, 2 });\n}\n",
        ),
        (
            "--style=whitesmith",
            "void f(void)\n    {\n    g(a,\n        {\n        free(n);\n        });\n    g(a,\n        { 1, 2 });\n    }\n",
        ),
    ] {
        let mut options = FormatOptions::default();
        apply_command_line_args(&mut options, &[style.to_owned()]).expect("valid options");
        let first = format_exact(input, &options);

        assert_eq!(first, expected, "{style}");
        assert_eq!(format_exact(&first, &options), first, "{style}");
    }
}

#[test]
fn whitesmith_array_brace_after_a_struct_closer_stands_at_the_closer() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");
    let input = "struct s {\n    int x;\n} b[] = { /* c */\n    {1}, {2}\n};\n";
    let expected =
        "struct s\n    {\n    int x;\n    } b[] =   /* c */\n    {\n        {1}, {2}\n    };\n";
    let first = format_exact(input, &options);

    assert_eq!(first, expected);
    assert_eq!(format_exact(&first, &options), first);
}
