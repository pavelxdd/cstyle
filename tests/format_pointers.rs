#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{
    BraceStyle, FormatOptions, PointerAlign, ReferenceAlign, apply_command_line_args,
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
fn split_pointer_parameter_aligns_with_first_parameter() {
    assert_eq!(
        format_c(
            fixture!("void run(char *first, char", "*second, char *third)", "{",),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run(char *first, char",
            "         *second, char *third)",
            "{",
        ),
    );
}

#[test]
fn pointer_align_name_preserves_uppercase_parameter_names() {
    let source = fixture!("int helper(State *S, State *TL);");

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn pointer_align_name_formats_core_typedef_pointer_declarators() {
    let options = options_from_args(&["--style=kr", "--mode=c", "--align-pointer=name"]);

    assert_eq!(
        format_c(
            fixture!(
                "void helper(size_t* size, ptrdiff_t* diff, off_t* offset, time_t* when, uint8_t* byte, intmax_t* max, uintmax_t* umax, intptr_t* ip, uintptr_t* up, int128_t* wide, __uint128_t* uwide, atomic_uint_fast64_t* counter);"
            ),
            &options,
        ),
        fixture!(
            "void helper(size_t *size, ptrdiff_t *diff, off_t *offset, time_t *when, uint8_t *byte, intmax_t *max, uintmax_t *umax, intptr_t *ip, uintptr_t *up, int128_t *wide, __uint128_t *uwide, atomic_uint_fast64_t *counter);"
        )
    );
}

#[test]
fn pointer_align_type_attaches_parameter_pointer_to_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nstring foo(const string *bar)   // comment\n{\n",
            &options,
        ),
        "\nstring foo(const string* bar)   // comment\n{\n",
    );
}

#[test]
fn pointer_align_type_attaches_if_initializer_pointer_to_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if(Item *item = owner->get_item())\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if(Item* item = owner->get_item())\n",
    );
}

#[test]
fn pointer_align_type_attaches_double_pointer_parameter_to_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c("\nint main(int argc, char **argv)\n{\n", &options),
        "\nint main(int argc, char** argv)\n{\n",
    );
}

#[test]
fn pointer_align_type_inserts_gap_before_parameter_block_comments() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid foo(WordList*/*keyword*/,\n         WordList**/*keyword*/) {\n}\n",
            &options,
        ),
        "\nvoid foo(WordList* /*keyword*/,\n         WordList** /*keyword*/) {\n}\n",
    );
}

#[test]
fn pointer_align_type_keeps_single_gap_before_defaulted_parameter_comments() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid call(const Type * /*item*/,\n          const Config * /*config*/ = 0)\n{}\n",
            &options,
        ),
        "\nvoid call(const Type* /*item*/,\n          const Config* /*config*/ = 0)\n{}\n",
    );
}

#[test]
fn pointer_align_type_also_attaches_references_to_type_by_default() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    const string &value;\n    const string  &  other;\n}\n",
            &options,
        ),
        "\nvoid foo()\n{\n    const string& value;\n    const string&    other;\n}\n",
    );
}

#[test]
fn pointer_align_type_attaches_unnamed_defaulted_references_to_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid call(const string & = value);\nvoid call(const string  &  = value);\n",
            &options,
        ),
        "\nvoid call(const string& = value);\nvoid call(const string&    = value);\n",
    );
}

#[test]
fn pointer_align_name_places_unnamed_reference_before_default_value() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c("\nvoid call(const string  &  = value);\n", &options,),
        "\nvoid call(const string   & = value);\n",
    );
}

#[test]
fn pointer_line_block_comment_preserves_wide_gap_without_alignment() {
    let options = options_from_args(&["--keep-one-line-blocks"]);
    let source = "\nvoid **    /* comment */\nfoo() {}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pointer_align_type_preserves_return_pointer_block_comment_column() {
    let options = options_from_args(&["--align-pointer=type", "--keep-one-line-blocks"]);

    assert_eq!(
        format_c("\nvoid **    /* comment */\nfoo() {}\n", &options),
        "\nvoid**     /* comment */\nfoo() {}\n",
    );
}

#[test]
fn pointer_align_type_also_attaches_rvalue_references_to_type_by_default() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c("\nItem &&make(Item &&item)\n{\n", &options),
        "\nItem&& make(Item&& item)\n{\n",
    );
}

#[test]
fn pointer_align_type_attaches_conditional_auto_rvalue_reference_to_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if (auto &&result = get_value()) {\n    }\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if (auto&& result = get_value()) {\n    }\n",
    );
}

#[test]
fn pointer_align_middle_centers_parameter_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c(
            "\nstring foo(const string *bar)   // comment\n{\n",
            &options,
        ),
        "\nstring foo(const string * bar)  // comment\n{\n",
    );
}

#[test]
fn pointer_align_middle_centers_if_initializer_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if(Item *item = owner->get_item())\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if(Item * item = owner->get_item())\n",
    );
}

#[test]
fn pointer_align_middle_centers_double_pointer_parameter() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c("\nint main(int argc, char **argv)\n{\n", &options),
        "\nint main(int argc, char ** argv)\n{\n",
    );
}

#[test]
fn pointer_align_middle_inserts_gap_before_parameter_block_comments() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c(
            "\nvoid foo(WordList*/*keyword*/,\n         WordList**/*keyword*/) {\n}\n",
            &options,
        ),
        "\nvoid foo(WordList * /*keyword*/,\n         WordList ** /*keyword*/) {\n}\n",
    );
}

#[test]
fn pointer_align_middle_also_centers_rvalue_references_by_default() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c("\nItem &&make(Item&& item)\n{\n", &options),
        "\nItem && make(Item && item)\n{\n",
    );
}

#[test]
fn pointer_align_name_attaches_parameter_pointer_to_name() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c(
            "\nstring foo(const string* bar)   // comment\n{\n",
            &options,
        ),
        "\nstring foo(const string *bar)   // comment\n{\n",
    );
}

#[test]
fn pointer_align_name_attaches_if_initializer_pointer_to_name() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if(Item * item = owner->get_item())\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if(Item *item = owner->get_item())\n",
    );
}

#[test]
fn pointer_align_name_attaches_double_pointer_parameter_to_name() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c("\nint main(int count, char** items)\n{\n", &options),
        "\nint main(int count, char **items)\n{\n",
    );
}

#[test]
fn pointer_align_name_also_attaches_rvalue_references_to_names_by_default() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c("\nItem&& make(Item&& item)\n{\n", &options),
        "\nItem &&make(Item &&item)\n{\n",
    );
}

#[test]
fn default_function_pointer_parens_stay_attached() {
    let source = fixture!("typedef void(*handler)(int value, int code);");

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn pointer_align_name_keeps_function_pointer_group_attached_after_custom_type() {
    let options = options_from_args(&["--style=kr", "--mode=c", "--align-pointer=name"]);
    let source = fixture!(
        "static int helper(State *state,",
        "                  Type *(*func)(int value));",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pointer_and_reference_align_name_format_short_adjacent_operator_sequences() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--pad-oper",
        "--align-pointer=name",
        "--align-reference=name",
    ]);

    assert_eq!(
        format_c(
            fixture!("wa & &;", "wa * *;", "wa & *;", "wa * &;"),
            &options
        ),
        fixture!("wa& &;", "wa * *;", "wa & *;", "wa*&;")
    );
}

#[test]
fn pad_operators_separates_adjacent_reference_and_pointer_tokens() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--pad-oper",
        "--align-pointer=name",
        "--align-reference=name",
        "--max-code-length=109",
    ]);

    assert_eq!(format_c(fixture!("&*"), &options), fixture!("& *"));
}

#[test]
fn pad_operators_pointer_align_name_separates_function_pointer_parameter_group() {
    assert_eq!(
        format_c(
            fixture!(
                "static int helper(State *state,",
                "                  struct Entry *(*func)(struct Store *, struct Key))",
                "{",
                "    return 0;",
                "}",
            ),
            &kr_c_options(),
        ),
        fixture!(
            "static int helper(State *state,",
            "                  struct Entry * (*func)(struct Store *, struct Key))",
            "{",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_attaches_macro_parameter_pointer_to_name() {
    let source = fixture!(
        "NODE(\"name\", handler, \"b\",",
        "     const Data *addr, int bits)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pointer_align_name_keeps_function_pointer_declarator_group_separated() {
    let source = fixture!(
        "static Doc *doc_new(",
        "    Value **root_out,",
        "    Value * (*mkroot)(Doc *))",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pointer_align_name_attaches_multiline_double_pointer_parameters_to_names() {
    let source = fixture!(
        "static void helper(char **name, GROUP_OF(Item) **chain, Item **item,",
        "                   Key **key, const char *text)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pointer_align_name_attaches_const_in_unnamed_array_type_id() {
    let actual = format_c(
        fixture!(
            "void helper(void)",
            "{",
            "    consume((const struct Option * const[]) { NULL });",
            "}",
        ),
        &kr_c_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void helper(void)",
            "{",
            "    consume((const struct Option *const[]) { NULL });",
            "}",
        )
    );
}

// Parentheses do not change the multiplication classification.
#[test]
fn parenthesized_identifier_product_keeps_multiplication_padding() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let source = fixture!(
        "bool z = (BOOT_A * BOOT_B) <= C;",
        "int w = BOOT_A * BOOT_B;",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn attribute_prefixed_declaration_attaches_pointer_and_reference_to_name() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    options.pad_operators = true;
    let source = fixture!(
        "class C {",
        "[[nodiscard]] const Foo &bar() const;",
        "[[nodiscard]] Foo *baz() const;",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class C",
            "{",
            "    [[nodiscard]] const Foo &bar() const;",
            "    [[nodiscard]] Foo *baz() const;",
            "};",
        )
    );
}

#[test]
fn multiline_declaration_parameter_continuation_attaches_pointer_to_name() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let source = fixture!(
        "class C {",
        "    C(OperationContext *operationContext,",
        "      Context *parent);",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class C",
            "{",
            "    C(OperationContext *operationContext,",
            "      Context *parent);",
            "};",
        )
    );
}

#[test]
fn out_of_class_constructor_parameter_continuation_attaches_pointer_to_name() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let source = fixture!(
        "Node::Node(OperationContext *source,",
        "           Context *parent)",
        "    : Context(parent)",
        "{",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

// Lambda parameter alignment is independent of declaration scope.
#[test]
fn lambda_parameter_reference_attaches_to_name_inside_function_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.reference_align = ReferenceAlign::Name;
    options.pad_operators = true;
    options.break_after_logical = true;
    let source = fixture!(
        "void f()",
        "{",
        "    auto cb = [](const Foo &result) {",
        "        use(result);",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_operators_pads_logical_and_without_rvalue_reference_spacing() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "if(alpha&&beta&&beta->ready){",
            "call();",
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
            "    if (alpha && beta && beta->ready) {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_keeps_literal_attached_after_unpadded_cast() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!("void f(void){", "call((const char *)\"text\");", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    call((const char *)\"text\");",
            "}"
        )
    );
}

#[test]
fn nopad_suppresses_operator_pointer_and_reference_padding() {
    let actual = format(fixture!(
        "void f(){x=y+z/* *NOPAD* */;char* p/* *NOPAD* */;}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    x=y+z /* *NOPAD* */;",
            "    char* p /* *NOPAD* */;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_preserves_space_before_destructor_tilde_after_keyword() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "class Foo {",
            "public:",
            "virtual ~Foo();",
            "~Bar();",
            "virtual ~Baz() {}",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Foo {",
            "public:",
            "    virtual ~Foo();",
            "    ~Bar();",
            "    virtual ~Baz() {}",
            "};",
        )
    );
}

#[test]
fn pointer_align_none_preserves_source_spacing_by_default() {
    let actual = format_c(
        fixture!("void f(){const char*s; char**argv; int (*fp)(int); x=a*b;}"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    const char*s;",
            "    char**argv;",
            "    int (*fp)(int);",
            "    x=a*b;",
            "}",
        )
    );
}

#[test]
fn pointer_align_modes_format_pointer_after_template_close_angle() {
    assert_eq!(
        format_c(fixture!("List<int>* p;"), &FormatOptions::default()),
        fixture!("List<int>* p;")
    );

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(fixture!("List<int> *p;"), &type_options),
        fixture!("List<int>* p;")
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(fixture!("List<int>* p;"), &name_options),
        fixture!("List<int> *p;")
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(fixture!("List<int>* p;"), &middle_options),
        fixture!("List<int> * p;")
    );
}

#[test]
fn pointer_and_reference_align_modes_format_declarators_after_template_close_angles() {
    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(fixture!("Map<K, V>& r = m;"), &name_options),
        fixture!("Map<K, V> &r = m;")
    );
    assert_eq!(
        format_c(fixture!("Vec<Vec<int>>* pp;"), &name_options),
        fixture!("Vec<Vec<int>> *pp;")
    );

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(fixture!("Vec<Vec<int>>* pp;"), &type_options),
        fixture!("Vec<Vec<int>>* pp;")
    );
}

#[test]
fn pointer_align_name_aligns_typedef_like_declarators_without_changing_multiplication() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(mytype *p, foo *bar, a *b);",
            "void g(void){",
            "foo(a * b);",
            "x = a * b;",
            "return a * b;",
            "Foo *p;",
            "mytype *q;",
            "u_char *s;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(mytype *p, foo *bar, a *b);",
            "void g(void)",
            "{",
            "    foo(a * b);",
            "    x = a * b;",
            "    return a * b;",
            "    Foo *p;",
            "    mytype *q;",
            "    u_char *s;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_treats_objc_block_caret_as_declarator_without_changing_xor() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!("void f(){void (^block)(int); int x=a^b;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    void (^block)(int);",
            "    int x = a ^ b;",
            "}",
        )
    );
    assert_eq!(
        format_c("void (\t^handler)(void);\n", &options),
        "void (\t^handler)(void);\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_format_type_middle_and_name() {
    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    let type_actual = format_with(fixture!("void f(){char* p; int& r;}"), &type_options);

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    let middle_actual = format_with(fixture!("void f(){char* p; int& r;}"), &middle_options);

    assert_eq!(
        type_actual,
        fixture!("void f()", "{", "    char* p;", "    int& r;", "}",)
    );
    assert_eq!(
        middle_actual,
        fixture!("void f()", "{", "    char * p;", "    int & r;", "}",)
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    let name_actual = format_with(
        fixture!("void f(){char* p; char** argv; int& r;}"),
        &name_options,
    );
    assert_eq!(
        name_actual,
        fixture!(
            "void f()",
            "{",
            "    char *p;",
            "    char **argv;",
            "    int &r;",
            "}",
        )
    );
}

#[test]
fn pointer_and_reference_align_none_preserves_source_spacing() {
    let options = FormatOptions::default();
    let source = fixture!(
        "void f()",
        "{",
        "    int *a;",
        "    int* b;",
        "    int * c;",
        "    char **d;",
        "    char ** e;",
        "    int &r = v;",
        "    int& s = v;",
        "    int & t = v;",
        "    foo(int *p, char **q);",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pointer_reference_param_spacing_preserved_without_align_option() {
    let source = "bool Create(BaseType * &o, Value *args)\n{\n    g();\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn pointer_align_middle_preserves_adjacent_pointer_groups() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    let actual = format_c(
        fixture!("void f(){char ** value; char * * spaced;}"),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    char ** value;",
            "    char * * spaced;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_formats_scope_qualified_member_pointers() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!("void f(){int Type::* member; int Type::*member;}"),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    int Type::*member;",
            "    int Type::*member;",
            "}",
        )
    );
}

#[test]
fn pointer_align_modes_format_comments_parameters_and_casts() {
    let mut type_options = FormatOptions::default();
    type_options.pad_operators = true;
    type_options.pad_commas = true;
    type_options.pointer_align = PointerAlign::Type;
    let type_actual = format_c(
        fixture!(
            "void f(){char * /* item */ value; void g(char*, char * name); char *p = (char *) value;}"
        ),
        &type_options,
    );
    assert_eq!(
        type_actual,
        fixture!(
            "void f() {",
            "    char* /* item */ value;",
            "    void g(char*, char* name);",
            "    char* p = (char*) value;",
            "}",
        )
    );

    let mut name_options = type_options.clone();
    name_options.pointer_align = PointerAlign::Name;
    let name_actual = format_c(
        fixture!(
            "void f(){char * /* item */ value; void g(char*, char * name); char *p = (char *) value;}"
        ),
        &name_options,
    );
    assert_eq!(
        name_actual,
        fixture!(
            "void f() {",
            "    char * /* item */ value;",
            "    void g(char *, char *name);",
            "    char *p = (char *) value;",
            "}",
        )
    );
}

#[test]
fn pointer_align_modes_format_reference_to_pointer_and_function_pointer_declarators() {
    let mut type_options = FormatOptions::default();
    type_options.pad_operators = true;
    type_options.pointer_align = PointerAlign::Type;
    let type_actual = format_with(
        fixture!("void f(){char *& r = p; void (*fp)(int);}"),
        &type_options,
    );
    assert_eq!(
        type_actual,
        fixture!(
            "void f()",
            "{",
            "    char*& r = p;",
            "    void (*fp)(int);",
            "}",
        )
    );

    let mut middle_options = type_options.clone();
    middle_options.pointer_align = PointerAlign::Middle;
    let middle_actual = format_with(
        fixture!("void f(){char *& r = p; void (*fp)(int);}"),
        &middle_options,
    );
    assert_eq!(
        middle_actual,
        fixture!(
            "void f()",
            "{",
            "    char *& r = p;",
            "    void (*fp)(int);",
            "}",
        )
    );

    let mut name_options = type_options.clone();
    name_options.pointer_align = PointerAlign::Name;
    let name_actual = format_with(
        fixture!("void f(){char *& r = p; void (*fp)(int);}"),
        &name_options,
    );
    assert_eq!(
        name_actual,
        fixture!(
            "void f()",
            "{",
            "    char *&r = p;",
            "    void (*fp)(int);",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_attaches_names_in_continued_function_prototypes() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.pad_commas = true;
    let actual = format_with(
        fixture!(
            "void *find_combined(hash_t *hash, int key,",
            "                    byte_alias *name, size_t len);",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void *find_combined(hash_t *hash, int key,",
            "                    byte_alias *name, size_t len);",
        )
    );
}

#[test]
fn pointer_align_name_attaches_function_argument_pointers() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void declared(int * a, char * b);",
            "void defined(int * a, char * b){}",
            "void multiline(",
            "    int * a,",
            "    char * b);",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void declared(int *a, char *b);",
            "void defined(int *a, char *b) {}",
            "void multiline(",
            "    int *a,",
            "    char *b);",
        )
    );
}

#[test]
fn pointer_align_name_preserves_non_declarator_pointer_uses_in_function_bodies() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "int product = alpha * beta;",
            "int value = *(int *)source;",
            "void (* callback)(int * arg);",
            "void unnamed(char *);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    int product = alpha * beta;",
            "    int value = *(int *)source;",
            "    void (* callback)(int *arg);",
            "    void unnamed(char *);",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_keeps_call_argument_multiplication_padded() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "call(name, end - start, NUM_ITERATIONS * NUM_WATCHERS * 2);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    call(name, end - start, NUM_ITERATIONS * NUM_WATCHERS * 2);",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_keeps_array_dimension_multiplication_padded() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(void){", "char buf[MSG_SIZE * BATCH];", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("void f(void)", "{", "    char buf[MSG_SIZE * BATCH];", "}",)
    );
}

#[test]
fn pad_operators_keeps_pointer_symbols_unary_after_comments() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){x = /* comment */ *p; y = // comment", "*p;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    x = /* comment */ *p;",
            "    y = // comment",
            "        *p;",
            "}",
        )
    );
}

#[test]
fn pad_operators_treats_else_and_delete_pointer_symbols_as_unary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){if(x) a(); else *p=1; delete *p; return *p; a ? *p : *q;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if(x) a();",
            "    else *p = 1;",
            "    delete *p;",
            "    return *p;",
            "    a ? *p : *q;",
            "}",
        )
    );
}

#[test]
fn pad_operators_distinguishes_header_declarators_from_multiplication() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){for(item*x:xs){g(x);} while((value*y=first)){break;} if(item*x+1){}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for(item*x : xs)",
            "    {",
            "        g(x);",
            "    }",
            "    while((value*y = first))",
            "    {",
            "        break;",
            "    }",
            "    if(item * x + 1) {}",
            "}",
        )
    );
}

#[test]
fn pad_operators_treats_bitwise_and_in_call_arguments_as_operator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "return read(address & mirrors[i][loc], peek);",
            "obj[loc].write(address & mirrors[i][loc], value);",
            "if(check(ctrl & (a | b))){g();}",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    return read(address & mirrors[i][loc], peek);",
            "    obj[loc].write(address & mirrors[i][loc], value);",
            "    if(check(ctrl & (a | b)))",
            "    {",
            "        g();",
            "    }",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_keeps_bitwise_and_before_macro_padded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "CHECK_VALUE(op == ADD ||",
            "            op == MOD);",
            "uint32_t value = source->user_data & UINT32_MAX;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    CHECK_VALUE(op == ADD ||",
            "                op == MOD);",
            "    uint32_t value = source->user_data & UINT32_MAX;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_treats_constructor_parameters_as_reference_declarators() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "class A {",
            "A(Config &config, QString text);",
            "void set(Config &config, const Item &r);",
            "A() { call(e & f); }",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class A {",
            "    A(Config &config, QString text);",
            "    void set(Config &config, const Item &r);",
            "    A() {",
            "        call(e & f);",
            "    }",
            "};",
        )
    );
}

#[test]
fn pad_operators_treats_pointer_symbols_between_names_as_binary_in_initializers() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    let actual = format_with(fixture!("int a[]={x*y,p&q,*r};"), &options);

    assert_eq!(actual, fixture!("int a[] = {x * y, p & q, *r};"));
}

#[test]
fn reference_align_name_treats_rvalue_references_as_declarators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_with(
        fixture!("void f(){for(auto&&x:xs){g(x);} auto&&y=h(); if(a&&b){return;}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for(auto &&x : xs)",
            "    {",
            "        g(x);",
            "    }",
            "    auto &&y = h();",
            "    if(a && b)",
            "    {",
            "        return;",
            "    }",
            "}",
        )
    );
}

#[test]
fn pad_operators_distinguishes_adjacent_pointer_stars_from_multiply_then_dereference() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){z=a**b; w=a* *b; value=table[*cursor++&0xf];}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    z = a**b;",
            "    w = a * *b;",
            "    value = table[*cursor++ & 0xf];",
            "}",
        )
    );
}

#[test]
fn pad_operators_keeps_unary_dereference_after_operator_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!("void f(unsigned value, byte_t *cursor){result = (value ^ *cursor++) & 0xff;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(unsigned value, byte_t *cursor)",
            "{",
            "    result = (value ^ *cursor++) & 0xff;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_keeps_type_like_pointer_casts_attached_to_operands() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!("void f(){p=(byte_t *)source; q=(custom_type *)value;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    p = (byte_t *)source;",
            "    q = (custom_type *)value;",
            "}",
        )
    );
}

#[test]
fn pad_operators_keeps_dereference_address_of_casts_and_function_pointers_unpadded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c(
        fixture!("void f(){x=*p+&y+a*b; void *vp=(void*)p; int (*fp)(int);}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    x = *p + &y + a * b;",
            "    void *vp = (void*)p;",
            "    int (*fp)(int);",
            "}",
        )
    );
}

#[test]
fn pointer_align_type_pad_operators_keeps_cast_and_continuation_dereference_unspaced() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    options.pad_operators = true;

    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Type* value = (Type*)",
                "                  *node;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "    Type* value = (Type*)",
            "                  *node;",
            "}",
        )
    );
}

#[test]
fn pointer_align_type_keeps_leading_nested_dereference_unspaced() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            fixture!("void f(int** pp)", "{", "    **pp = 0;", "}"),
            &options,
        ),
        fixture!("void f(int** pp)", "{", "    **pp = 0;", "}")
    );
}

#[test]
fn pointer_align_type_keeps_nested_expression_dereference_unspaced() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;

    assert_eq!(
        format_c(
            fixture!("void f()", "{", "    fill( value, **it );", "}"),
            &options,
        ),
        fixture!("void f()", "{", "    fill( value, **it );", "}")
    );
}

#[test]
fn pointer_align_name_preserves_atomic_parenthesized_pointer_declarator() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(fixture!("_Atomic(struct Item *) value;"), &options);

    assert_eq!(actual, fixture!("_Atomic(struct Item *) value;"));
}

#[test]
fn pointer_align_middle_spaces_line_terminal_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    let actual = format_c("char*\nfoo;\n", &options);
    assert_eq!(actual.lines().next(), Some("char *"));
}

#[test]
fn pointer_and_reference_align_preserve_line_terminal_type_side_gaps() {
    let source = concat!(
        "Item  *\n",
        "pointer;\n",
        "Other\t&\n",
        "reference;\n",
        "More&&\n",
        "rvalue;\n",
    );
    let expected = concat!(
        "Item  *\n",
        "pointer;\n",
        "Other\t&\n",
        "reference;\n",
        "More &&\n",
        "rvalue;\n",
    );
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    options.reference_align = ReferenceAlign::Middle;

    assert_eq!(format_c(source, &options), expected);

    options.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c("Last && \nend;\n", &options), "Last &&\nend;\n");
}

#[test]
fn pointer_align_name_spaces_line_terminal_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_c("char*\nfoo;\n", &options);
    assert_eq!(actual.lines().next(), Some("char *"));
}

#[test]
fn pointer_align_type_keeps_line_terminal_pointer_attached() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    let actual = format_c("char*\nfoo;\n", &options);
    assert_eq!(actual.lines().next(), Some("char*"));
}

#[test]
fn pointer_align_middle_spaces_line_terminal_pointer_after_unknown_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    let actual = format_c("Config*\np;\n", &options);
    assert_eq!(actual.lines().next(), Some("Config *"));
}

#[test]
fn pointer_align_middle_preserves_line_terminal_multiplication() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    let actual = format_c("a=b*\nc;\n", &options);
    assert_eq!(actual.lines().next(), Some("a=b*"));
}

#[test]
fn pointer_align_middle_keeps_split_declarator_tokens_flush() {
    let source = concat!(
        "Item\n",
        "*\n",
        "value;\n",
        "Box<\n",
        "    Other\n",
        "    *\n",
        "> item;\n",
    );
    let expected = concat!(
        "Item\n",
        "*\n",
        "value;\n",
        "Box<\n",
        "Other\n",
        "*\n",
        "> item;\n",
    );
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(format_c(source, &options), expected);
}

#[test]
fn reference_align_modes_keep_split_rvalue_reference_flush_with_type() {
    let source = "Item\n&&value;\n";

    let mut none_options = FormatOptions::default();
    none_options.reference_align = ReferenceAlign::None;
    assert_eq!(format_c(source, &none_options), source);

    let mut middle_options = FormatOptions::default();
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(format_c(source, &middle_options), "Item\n&& value;\n");
}

#[test]
fn pointer_and_reference_align_preserve_split_parameter_name_adjacency() {
    let source = concat!(
        "void call(\n",
        "    Item\n",
        "    *value,\n",
        "    Other\n",
        "    &reference,\n",
        "    More\n",
        "    &&rvalue);\n",
    );
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    options.reference_align = ReferenceAlign::Type;

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pointer_align_name_formats_lowercase_type_in_parameter_continuation() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_c("void f(int a,\n       value* b);\n", &options);
    assert_eq!(actual, "void f(int a,\n       value *b);\n");
}

#[test]
fn pointer_align_middle_formats_lowercase_type_in_parameter_continuation() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    let actual = format_c("void f(int a,\n       value* b);\n", &options);
    assert_eq!(actual, "void f(int a,\n       value * b);\n");
}

#[test]
fn pointer_align_type_keeps_lowercase_type_attached_in_parameter_continuation() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    let actual = format_c("void f(int a,\n       value* b);\n", &options);
    assert_eq!(actual, "void f(int a,\n       value* b);\n");
}

#[test]
fn pointer_align_name_preserves_binary_and_after_multiline_signature() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_c(
        fixture!(
            "static int helper(unsigned *seen,",
            "                  unsigned mask)",
            "{",
            "if((*seen & mask) != 0) {",
            "return 1;",
            "}",
            "return 0;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static int helper(unsigned *seen,",
            "                  unsigned mask)",
            "{",
            "    if((*seen & mask) != 0) {",
            "        return 1;",
            "    }",
            "    return 0;",
            "}",
        )
    );
}

#[test]
fn pointer_align_none_keeps_split_pointer_declarator_flush() {
    let options = FormatOptions::default();
    assert_eq!(
        format_c(fixture!("MyType *", "value;"), &options),
        fixture!("MyType *", "value;")
    );
}

#[test]
fn pointer_align_name_keeps_assignment_continuation_multiplication_padded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!(
            "const unsigned long limit =",
            "header_len + direction_len +",
            "3 * (header_len + tag_len);",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "const unsigned long limit =",
            "    header_len + direction_len +",
            "    3 * (header_len + tag_len);",
        )
    );
}

#[test]
fn pointer_align_name_preserves_far_side_gap_before_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(fixture!("char  *value;"), &options),
        fixture!("char  *value;")
    );
    assert_eq!(
        format_c(fixture!("char\t*value;"), &options),
        fixture!("char\t*value;")
    );
}

#[test]
fn pointer_align_name_consolidates_trailing_gap_before_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(fixture!("char*  value;"), &options),
        fixture!("char  *value;")
    );
    assert_eq!(
        format_c(fixture!("char  *  value;"), &options),
        fixture!("char    *value;")
    );
    assert_eq!(
        format_c(fixture!("char * value;"), &options),
        fixture!("char *value;")
    );
}

#[test]
fn pointer_align_type_preserves_and_consolidates_far_side_gap() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(fixture!("char  *value;"), &options),
        fixture!("char*  value;")
    );
    assert_eq!(
        format_c(fixture!("char*\tvalue;"), &options),
        fixture!("char*\tvalue;")
    );
    assert_eq!(
        format_c(fixture!("char * value;"), &options),
        fixture!("char* value;")
    );
}

#[test]
fn pointer_align_name_preserves_gap_with_double_pointer() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(fixture!("int  **value;"), &options),
        fixture!("int  **value;")
    );
    assert_eq!(
        format_c(fixture!("int**  value;"), &options),
        fixture!("int  **value;")
    );
}

#[test]
fn pointer_align_modes_collapse_gap_in_unnamed_parameter() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(fixture!("void f(char  *);"), &options),
        fixture!("void f(char *);")
    );
    options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(fixture!("void f(char  *);"), &options),
        fixture!("void f(char*);")
    );
}

#[test]
fn pointer_align_middle_with_convert_tabs_uses_source_gap_columns() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    options.convert_tabs = true;

    assert_eq!(
        format_c("Item\t*\tvalue;\nresult=(Item\t*\t)source;\n", &options),
        "Item    *   value;\nresult=(Item *   )source;\n"
    );

    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c("result=(Item\t*\t)source;\n", &options),
        "result=(Item *   )source;\n"
    );
}

#[test]
fn pointer_align_name_with_convert_tabs_expands_gap_at_source_column() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    options.convert_tabs = true;
    assert_eq!(
        format_c(
            fixture!(
                "void foo()",
                "{",
                "    WidgetBox*\tchannel;",
                "    char\t\t*\tstamp;",
                "}"
            ),
            &options
        ),
        fixture!(
            "void foo()",
            "{",
            "    WidgetBox  *channel;",
            "    char           *stamp;",
            "}"
        )
    );
}

#[test]
fn pad_operators_treats_header_body_dereference_as_unary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c(
            fixture!(
                "void f(void) {",
                "    if (value > *result) *result = value;",
                "    while (alpha) *beta = 0;",
                "    for (i = 0; i < n; i++) *gamma = i;",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    if (value > *result) *result = value;",
            "    while (alpha) *beta = 0;",
            "    for (i = 0; i < n; i++) *gamma = i;",
            "}"
        )
    );
}

#[test]
fn pad_operators_preserves_header_body_dereference_source_gap() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c(fixture!("void f(void) { if (alpha)*beta = 0; }"), &options),
        fixture!("void f(void) {", "    if (alpha)*beta = 0;", "}")
    );
    assert_eq!(
        format_c(
            fixture!("void f(void) { do *beta = 0; while (alpha); }"),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    do *beta = 0;",
            "    while (alpha);",
            "}"
        )
    );
}

#[test]
fn pad_operators_keeps_multiply_after_call_in_header_body_binary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c(
            fixture!("void f(void) { if (alpha) beta = call(gamma) * delta; }"),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    if (alpha) beta = call(gamma) * delta;",
            "}"
        )
    );
}

#[test]
fn pad_operators_treats_multiply_inside_subscript_as_binary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c(
            fixture!("void f(void) { alpha[beta*gamma - 1] = 2; }"),
            &options
        ),
        fixture!("void f(void) {", "    alpha[beta * gamma - 1] = 2;", "}")
    );
    assert_eq!(
        format_c(
            fixture!("void f(void) { alpha[beta*gamma] = delta[epsilon*zeta]; }"),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    alpha[beta * gamma] = delta[epsilon * zeta];",
            "}"
        )
    );
}

#[test]
fn pointer_align_name_does_not_change_subscript_multiplication() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(
            fixture!("void f(void) { alpha[beta*gamma] = 2; }"),
            &options
        ),
        fixture!("void f(void) {", "    alpha[beta * gamma] = 2;", "}")
    );
}

#[test]
fn pointer_align_name_aligns_reference_after_scope_qualified_type() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(
            fixture!("bool f(const std::string & name, key_t & coord);"),
            &options
        ),
        fixture!("bool f(const std::string &name, key_t &coord);")
    );
    assert_eq!(
        format_c(
            fixture!("struct S { std::function<void(const std::string & error)> error; };"),
            &options
        ),
        fixture!(
            "struct S {",
            "    std::function<void(const std::string &error)> error;",
            "};"
        )
    );
}

#[test]
fn pad_operators_keeps_dereference_argument_inside_subscript_unary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c(
            fixture!("void f(void) { result = alpha[*beta] + gamma[&delta]; }"),
            &options
        ),
        fixture!(
            "void f(void) {",
            "    result = alpha[*beta] + gamma[&delta];",
            "}"
        )
    );
}

#[test]
fn pointer_align_middle_centers_declarators_and_preserves_wide_gaps() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(fixture!("char* p;"), &options),
        fixture!("char * p;")
    );
    assert_eq!(
        format_c(fixture!("char *p;"), &options),
        fixture!("char * p;")
    );
    assert_eq!(
        format_c(fixture!("char  *  p;"), &options),
        fixture!("char  *  p;")
    );
    assert_eq!(
        format_c(fixture!("char   *p;"), &options),
        fixture!("char  * p;")
    );
    assert_eq!(
        format_c(fixture!("char*   p;"), &options),
        fixture!("char  * p;")
    );
    assert_eq!(
        format_c(fixture!("int  **  pp;"), &options),
        fixture!("int  **  pp;")
    );
}

#[test]
fn pointer_align_middle_does_not_center_multiplication() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(
            fixture!("void f(void) { result = alpha  *  beta; }"),
            &options
        ),
        fixture!("void f(void) {", "    result = alpha  *  beta;", "}")
    );
}

#[test]
fn pointer_and_reference_align_modes_format_custom_lambda_parameters() {
    let source = "auto fn=[](Item* value, Item& ref, Item&& rvalue){return value;};\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "auto fn=[](Item* value, Item& ref, Item&& rvalue) {\n    return value;\n};\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "auto fn=[](Item * value, Item & ref, Item && rvalue) {\n    return value;\n};\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "auto fn=[](Item *value, Item &ref, Item &&rvalue) {\n    return value;\n};\n"
    );
}

#[test]
fn pad_operators_keeps_custom_lambda_rvalue_reference_declarative() {
    let source = "auto fn=[](Item&& value){return value;};\n";

    let mut type_options = FormatOptions::default();
    type_options.pad_operators = true;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "auto fn = [](Item&& value) {\n    return value;\n};\n"
    );

    let mut name_options = type_options.clone();
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "auto fn = [](Item &&value) {\n    return value;\n};\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_format_template_type_ids() {
    let source = concat!(
        "Pair<Item*, const Other&> value;\n",
        "Box<Item&&> rvalue;\n",
        "template<Item* Value, Other& Ref> struct Holder;\n",
    );

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(format_c(source, &type_options), source);

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        concat!(
            "Pair<Item *, const Other &> value;\n",
            "Box<Item &&> rvalue;\n",
            "template<Item * Value, Other & Ref> struct Holder;\n",
        )
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        concat!(
            "Pair<Item *, const Other &> value;\n",
            "Box<Item &&> rvalue;\n",
            "template<Item *Value, Other &Ref> struct Holder;\n",
        )
    );
}

#[test]
fn pointer_align_modes_format_nested_alias_type_ids() {
    let source = "using Alias=Box<Item*>;\n";

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "using Alias=Box<Item *>;\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "using Alias=Box<Item *>;\n"
    );
}

#[test]
fn pointer_align_modes_preserve_unreferenced_function_pointer_group_spacing() {
    let source = "Result (**handlers)(Arg);\n";
    for align in [PointerAlign::Type, PointerAlign::Middle, PointerAlign::Name] {
        let mut options = FormatOptions::default();
        options.pointer_align = align;
        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn pointer_and_reference_align_modes_keep_referenced_function_pointer_tokens_intact() {
    let source = "Result (**&handler)(Arg);\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "Result (**& handler)(Arg);\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "Result ( **& handler)(Arg);\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::None;
    assert_eq!(
        format_c(source, &name_options),
        "Result ( **&handler)(Arg);\n"
    );
}

#[test]
fn pad_operators_keeps_multilevel_function_pointer_references_stable() {
    let source = "Result (**&handler)(Arg);\n";
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    options.reference_align = ReferenceAlign::Type;
    options.pad_operators = true;

    let formatted = format_c(source, &options);
    assert_eq!(formatted, "Result (**& handler)(Arg);\n");
    assert_eq!(format_c(&formatted, &options), formatted);
}

#[test]
fn pointer_and_reference_align_modes_format_rvalue_references_in_function_pointers() {
    let source = "Result (*&&handler)(Arg);\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "Result (*&& handler)(Arg);\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "Result ( * && handler)(Arg);\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "Result ( * &&handler)(Arg);\n"
    );
}

#[test]
fn pointer_align_modes_keep_qualified_member_pointer_operator_atomic() {
    let source = concat!(
        "Item Owner:: * value;\n",
        "Result (Owner:: * handler)(Arg);\n",
    );

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        concat!(
            "Item Owner::* value;\n",
            "Result (Owner::* handler)(Arg);\n",
        )
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        concat!(
            "Item Owner::*  value;\n",
            "Result (Owner::*  handler)(Arg);\n",
        )
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        concat!("Item Owner::*value;\n", "Result (Owner::*handler)(Arg);\n",)
    );
}

#[test]
fn pointer_and_reference_align_modes_format_referenced_member_data_pointers() {
    let source = "Item Owner::*&value;\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(format_c(source, &type_options), "Item Owner::*& value;\n");

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(format_c(source, &middle_options), "Item Owner::*& value;\n");

    let mut middle_none = FormatOptions::default();
    middle_none.pointer_align = PointerAlign::Middle;
    middle_none.reference_align = ReferenceAlign::None;
    assert_eq!(format_c(source, &middle_none), "Item Owner::*&value;\n");

    let mut middle_name = FormatOptions::default();
    middle_name.pointer_align = PointerAlign::Middle;
    middle_name.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c(source, &middle_name), "Item Owner::* &value;\n");

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c(source, &name_options), "Item Owner::*&value;\n");
}

#[test]
fn pointer_and_reference_align_modes_format_referenced_member_function_pointers() {
    let source = "Result (Owner::*&handler)(Arg);\n";

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &middle_options),
        "Result (Owner::*& handler)(Arg);\n"
    );

    let mut middle_none = FormatOptions::default();
    middle_none.pointer_align = PointerAlign::Middle;
    middle_none.reference_align = ReferenceAlign::None;
    assert_eq!(
        format_c(source, &middle_none),
        "Result (Owner::*&handler)(Arg);\n"
    );

    let mut middle_name = FormatOptions::default();
    middle_name.pointer_align = PointerAlign::Middle;
    middle_name.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &middle_name),
        "Result (Owner::* &handler)(Arg);\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::None;
    assert_eq!(
        format_c(source, &name_options),
        "Result (Owner::*&handler)(Arg);\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_combine_independently() {
    let source = "Item*& value;\nResult (*&handler)(int);\n";

    let mut type_middle = FormatOptions::default();
    type_middle.pointer_align = PointerAlign::Type;
    type_middle.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &type_middle),
        "Item* & value;\nResult (* & handler)(int);\n"
    );

    let mut middle_name = FormatOptions::default();
    middle_name.pointer_align = PointerAlign::Middle;
    middle_name.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &middle_name),
        "Item * &value;\nResult ( * &handler)(int);\n"
    );

    let mut name_none = FormatOptions::default();
    name_none.pointer_align = PointerAlign::Name;
    name_none.reference_align = ReferenceAlign::None;
    assert_eq!(
        format_c(source, &name_none),
        "Item *&value;\nResult ( *&handler)(int);\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_keep_rvalue_reference_tokens_intact_after_pointers() {
    let source = "Item*&& value;\n";

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(format_c(source, &middle_options), "Item * && value;\n");

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c(source, &name_options), "Item * &&value;\n");
    // Alignment never splits a lexical rvalue-reference token.
}

#[test]
fn pointer_align_modes_do_not_change_statement_start_dereference() {
    let source = "result=*pointer;\n*pointer=value;\n";

    for align in [PointerAlign::Type, PointerAlign::Middle, PointerAlign::Name] {
        let mut options = FormatOptions::default();
        options.pointer_align = align;

        assert_eq!(format_c(source, &options), source);
    }
}

#[test]
fn pointer_align_modes_keep_multilevel_groups_intact_in_later_declarators() {
    let source = "Item* first, **second;\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(format_c(source, &type_options), "Item* first, ** second;\n");

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "Item * first, ** second;\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(format_c(source, &name_options), "Item *first, **second;\n");
    // Declarator position does not split an adjacent pointer group.
}

#[test]
fn pointer_and_reference_align_modes_format_nested_function_declarator_parameters() {
    let source = concat!(
        "int (&handler)(Item* value, Other& ref);\n",
        "int (Owner::*member)(Item* value, Other& ref);\n",
        "void (^block)(Item* value, Other& ref);\n",
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        concat!(
            "int (&handler)(Item * value, Other & ref);\n",
            "int (Owner::* member)(Item * value, Other & ref);\n",
            "void (^block)(Item * value, Other & ref);\n",
        )
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        concat!(
            "int (&handler)(Item *value, Other &ref);\n",
            "int (Owner::*member)(Item *value, Other &ref);\n",
            "void (^block)(Item *value, Other &ref);\n",
        )
    );
}

#[test]
fn pointer_and_reference_align_modes_format_dependent_generic_lambda_parameters() {
    let source = "auto fn=[]<class T>(T* pointer, T&& reference){return pointer;};\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    type_options.pad_operators = true;
    assert_eq!(
        format_c(source, &type_options),
        "auto fn = []<class T>(T* pointer, T&& reference) {\n    return pointer;\n};\n"
    );

    let mut middle_options = type_options.clone();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "auto fn = []<class T>(T * pointer, T && reference) {\n    return pointer;\n};\n"
    );

    let mut name_options = type_options;
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "auto fn = []<class T>(T *pointer, T &&reference) {\n    return pointer;\n};\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_format_defaulted_parameters() {
    let source = "void call(Item* value=nullptr, Other& ref=get());\n";

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "void call(Item * value=nullptr, Other & ref=get());\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "void call(Item *value=nullptr, Other &ref=get());\n"
    );
}

#[test]
fn pointer_align_middle_normalizes_unnamed_declarator_type_gaps() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;

    assert_eq!(
        format_c(
            "result=(Item\t*  )source;\nvoid call(Item  *  );\nBox<Item\t*> value;\n",
            &options
        ),
        "result=(Item *  )source;\nvoid call(Item *  );\nBox<Item *> value;\n"
    );
}

#[test]
fn pointer_and_reference_align_middle_formats_casts_and_type_operands() {
    let source = concat!(
        "pointer=(Item*)source;\n",
        "left=(Item&)source;\n",
        "right=(Item&&)source;\n",
        "size=sizeof(Item*);\n",
        "kind=typeid(Item&);\n",
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        concat!(
            "pointer=(Item *)source;\n",
            "left=(Item &)source;\n",
            "right=(Item &&)source;\n",
            "size=sizeof(Item *);\n",
            "kind=typeid(Item &);\n",
        )
    );
}

#[test]
fn pointer_and_reference_align_modes_format_multiline_template_argument_types() {
    let source = "Pair<Item *,\n     Other &> value;\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    type_options.pad_operators = true;
    assert_eq!(
        format_c(source, &type_options),
        "Pair<Item*,\n     Other&> value;\n"
    );

    let mut name_options = type_options;
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c(source, &name_options), source);
}

#[test]
fn pad_operators_preserves_using_alias_declarators() {
    let source = "using Pointer=Item*;\nusing Reference=Item&;\nusing Rvalue=Item&&;\n";
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    options.reference_align = ReferenceAlign::Type;
    options.pad_operators = true;

    // Using-alias type-ids remain declarators under operator padding.
    assert_eq!(
        format_c(source, &options),
        "using Pointer = Item*;\nusing Reference = Item&;\nusing Rvalue = Item&&;\n"
    );
}

#[test]
fn pad_operators_keeps_parenthesized_type_like_reference_declarative() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("value=(size_t&count);\n", &options),
        "value = (size_t&count);\n"
    );
}

#[test]
fn pad_operators_keeps_parenthesized_type_like_pointer_declarative() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("value=(size_t*count);\n", &options),
        "value = (size_t*count);\n"
    );
}

#[test]
fn pointer_and_reference_align_middle_rebalances_tabs_across_declarators() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Middle;
    options.reference_align = ReferenceAlign::Middle;

    assert_eq!(
        format_c(
            "Item*\tvalue;\nItem\t*  other;\nItem&\tleft;\nItem\t&&  right;\n",
            &options
        ),
        "Item\t* value;\nItem\t * other;\nItem\t& left;\nItem\t && right;\n"
    );
}

#[test]
fn pointer_reference_return_type_preserves_space_before_function_name() {
    let source = fixture!(
        "struct Item {",
        "    Value*& get() {",
        "        return value;",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn trailing_return_rvalue_reference_uses_reference_align_name() {
    let mut options = FormatOptions::default();
    options.reference_align = ReferenceAlign::Name;

    assert_eq!(
        format_c("auto function()->int&&;\n", &options),
        "auto function()->int &&;\n"
    );
}

#[test]
fn pad_operators_keeps_trailing_return_pointer_declarator_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("auto function()->int*;\n", &options),
        "auto function()->int*;\n"
    );
}

#[test]
fn trailing_return_name_alignment_is_independent_of_function_body() {
    let mut pointer_options = FormatOptions::default();
    pointer_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(
            "auto declaration()->int*;\nauto definition()->int*{}\n",
            &pointer_options
        ),
        "auto declaration()->int *;\nauto definition()->int * {}\n"
    );

    let mut reference_options = FormatOptions::default();
    reference_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(
            "auto declaration()->int&;\nauto definition()->int&{}\n",
            &reference_options
        ),
        "auto declaration()->int &;\nauto definition()->int & {}\n"
    );
}

#[test]
fn pointer_align_modes_format_trailing_return_declarators_before_semicolons_and_braces() {
    let source = concat!(
        "auto pointer_declaration()->Item*;\n",
        "auto reference_declaration()->Item&;\n",
        "auto pointer_lambda=[]()->Item* {return nullptr;};\n",
        "auto reference_lambda=[]()->Item& {static Item value;return value;};\n",
    );

    for align in [PointerAlign::Middle, PointerAlign::Name] {
        let mut options = FormatOptions::default();
        options.pointer_align = align;

        assert_eq!(
            format_c(source, &options),
            concat!(
                "auto pointer_declaration()->Item *;\n",
                "auto reference_declaration()->Item &;\n",
                "auto pointer_lambda=[]()->Item * {\n",
                "    return nullptr;\n",
                "};\n",
                "auto reference_lambda=[]()->Item & {\n",
                "    static Item value;\n",
                "    return value;\n",
                "};\n",
            )
        );
    }
}

#[test]
fn reference_align_middle_formats_trailing_rvalue_references_without_semicolon_gaps() {
    let source = concat!(
        "auto declaration()->Item&&;\n",
        "auto definition()->Item&&{}\n",
        "auto lambda=[]()->Item&&{return move(value);};\n",
    );
    let mut options = FormatOptions::default();
    options.reference_align = ReferenceAlign::Middle;

    assert_eq!(
        format_c(source, &options),
        concat!(
            "auto declaration()->Item &&;\n",
            "auto definition()->Item && {}\n",
            "auto lambda=[]()->Item && {\n",
            "    return move(value);\n",
            "};\n",
        )
    );
}

#[test]
fn reference_align_modes_do_not_rewrite_function_ref_qualifiers() {
    let source = "void left()&;\nvoid right()&&;\n";
    for align in [
        ReferenceAlign::Type,
        ReferenceAlign::Middle,
        ReferenceAlign::Name,
    ] {
        let mut options = FormatOptions::default();
        options.reference_align = align;
        assert_eq!(format_c(source, &options), source);
    }
    // Function ref-qualifiers are not declarator references.
}

#[test]
fn reference_align_modes_keep_conversion_operator_parentheses_attached() {
    let source = "operator Item&&();\n";

    let mut middle_options = FormatOptions::default();
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(format_c(source, &middle_options), "operator Item &&();\n");

    let mut name_options = FormatOptions::default();
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(format_c(source, &name_options), "operator Item &&();\n");
    // Conversion-function parentheses remain attached to the type-id.
}

#[test]
fn pointer_align_modes_format_declarators_after_parenthesized_type_operators() {
    let source = "decltype(value)* result;\ntypeof(value)* other;\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(format_c(source, &type_options), source);

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "decltype(value) * result;\ntypeof(value) * other;\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "decltype(value) *result;\ntypeof(value) *other;\n"
    );
}

#[test]
fn pointer_align_modes_format_member_pointers_inside_parameters() {
    let source = "void call(int Item::*member, int (Item:: *handler)(int));\n";

    for align in [PointerAlign::Type, PointerAlign::Middle] {
        let mut options = FormatOptions::default();
        options.pointer_align = align;
        assert_eq!(
            format_c(source, &options),
            "void call(int Item::* member, int (Item::* handler)(int));\n"
        );
    }

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "void call(int Item::*member, int (Item::*handler)(int));\n"
    );
}

#[test]
fn reference_align_type_and_middle_move_parameter_gaps() {
    let source = "void call(Item &alpha, Item &&beta);\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Name;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "void call(Item& alpha, Item&& beta);\n"
    );

    let mut middle_options = type_options;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "void call(Item & alpha, Item && beta);\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_keep_declarators_as_one_unit() {
    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::None;
    assert_eq!(
        format_c("Item*& value;\nItem**& other;\n", &name_options),
        "Item *&value;\nItem **&other;\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c("Item*& value;\nItem**& other;\n", &middle_options),
        "Item *& value;\nItem **& other;\n"
    );
    // Pointer depth does not change pointer/reference alignment precedence.
}

#[test]
fn pointer_align_modes_format_adjacent_and_separated_multilevel_declarators() {
    let source = "int*** adjacent;\nint *  * value;\n";

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c(source, &type_options),
        "int*** adjacent;\nint **   value;\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        "int *** adjacent;\nint *  * value;\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        "int ***adjacent;\nint *   *value;\n"
    );
}

#[test]
fn pointer_align_modes_preserve_wide_gaps_before_declarator_comments() {
    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c("Item  *  /* note */ value;\n", &type_options),
        "Item*    /* note */ value;\n"
    );
    assert_eq!(
        format_c("Item\t*\t// note\nvalue;\n", &type_options),
        "Item*\t\t// note\nvalue;\n"
    );
    assert_eq!(
        format_c("Item * // note\nvalue;\n", &type_options),
        "Item* // note\nvalue;\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c("Item*  /* note */ value;\n", &middle_options),
        "Item *  /* note */ value;\n"
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    assert_eq!(
        format_c("Item\t*\t/* note */ value;\n", &name_options),
        "Item\t*\t/* note */ value;\n"
    );
    assert_eq!(
        format_c("Item*  // note\nvalue;\n", &name_options),
        "Item *  // note\nvalue;\n"
    );
    // Pointer alignment preserves source gaps consistently across comment kinds.
}

#[test]
fn pointer_and_reference_align_type_keeps_one_gap_before_declarator_comments() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Type;
    options.reference_align = ReferenceAlign::Type;

    assert_eq!(
        format_c(
            "Item * /* pointer */ value;\nItem & /* reference */ other;\n",
            &options
        ),
        "Item* /* pointer */ value;\nItem& /* reference */ other;\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_preserve_generated_gaps_before_attributes() {
    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    assert_eq!(
        format_c("Item  *[[maybe_unused]] value;\n", &type_options),
        "Item*  [[maybe_unused]] value;\n"
    );
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(
        format_c("auto&[left,right]=pair;\n", &type_options),
        "auto& [left,right]=pair;\n"
    );

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    assert_eq!(
        format_c("Item*  [[maybe_unused]] value;\n", &middle_options),
        "Item * [[maybe_unused]] value;\n"
    );
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c("auto&  [left,right]=pair;\n", &middle_options),
        "auto & [left,right]=pair;\n"
    );
}

#[test]
fn pointer_and_reference_align_modes_format_attributes_and_structured_bindings() {
    let source = concat!(
        "Item* [[maybe_unused]] value;\n",
        "const auto& [alpha,beta]=pair;\n",
        "auto&& [left,right]=pair;\n",
    );

    let mut type_options = FormatOptions::default();
    type_options.pointer_align = PointerAlign::Type;
    type_options.reference_align = ReferenceAlign::Type;
    assert_eq!(format_c(source, &type_options), source);

    let mut middle_options = FormatOptions::default();
    middle_options.pointer_align = PointerAlign::Middle;
    middle_options.reference_align = ReferenceAlign::Middle;
    assert_eq!(
        format_c(source, &middle_options),
        concat!(
            "Item * [[maybe_unused]] value;\n",
            "const auto & [alpha,beta]=pair;\n",
            "auto && [left,right]=pair;\n",
        )
    );

    let mut name_options = FormatOptions::default();
    name_options.pointer_align = PointerAlign::Name;
    name_options.reference_align = ReferenceAlign::Name;
    assert_eq!(
        format_c(source, &name_options),
        concat!(
            "Item *[[maybe_unused]] value;\n",
            "const auto &[alpha,beta]=pair;\n",
            "auto &&[left,right]=pair;\n",
        )
    );
}

#[test]
fn reference_align_middle_does_not_reclassify_logical_and_before_dereference() {
    let mut options = FormatOptions::default();
    options.reference_align = ReferenceAlign::Middle;

    assert_eq!(format_c("value&&*other;\n", &options), "value&&*other;\n");
}

#[test]
fn reference_align_name_does_not_reclassify_suffix_shaped_value_names() {
    let options = options_from_args(&[
        "--style=kr",
        "--mode=c",
        "--pad-oper",
        "--align-reference=name",
    ]);

    assert_eq!(
        format_c(
            fixture!(
                "long helper(long value_t, long factor)",
                "{",
                "    return value_t&factor;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "long helper(long value_t, long factor)",
            "{",
            "    return value_t & factor;",
            "}",
        )
    );
    assert_eq!(
        format_c(fixture!("value=(value_t&factor);"), &options),
        fixture!("value = (value_t & factor);")
    );
}

#[test]
fn pointer_align_name_does_not_reclassify_type_shaped_value_names() {
    let options = options_from_args(&[
        "--style=kr",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
    ]);

    assert_eq!(
        format_c(
            fixture!(
                "long helper(long value_t, long factor, long Result)",
                "{",
                "    return value_t*factor + Result*factor;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "long helper(long value_t, long factor, long Result)",
            "{",
            "    return value_t * factor + Result * factor;",
            "}",
        )
    );
}

#[test]
fn pointer_align_name_recognizes_unicode_and_extension_identifiers() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c("Тип * значение;\nItem * $value;\n", &options),
        "Тип *значение;\nItem *$value;\n"
    );
}

#[test]
fn pointer_align_modes_do_not_reclassify_template_expression_operators() {
    let source = "Box<(alpha*beta), (gamma&delta)> expression;\n";

    for align in [PointerAlign::Type, PointerAlign::Middle, PointerAlign::Name] {
        let mut options = FormatOptions::default();
        options.pointer_align = align;
        options.pad_operators = true;

        assert_eq!(
            format_c(source, &options),
            "Box<(alpha * beta), (gamma & delta)> expression;\n"
        );
    }
}

#[test]
fn pointer_align_name_keeps_parameter_pointer_attached_after_wrapped_return_type() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!(
            "template<class T>",
            "typename std::enable_if<std::is_pointer<T>::value, T>::type",
            "find_parent(char *object) {",
            "    return object;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "template<class T>",
            "typename std::enable_if<std::is_pointer<T>::value, T>::type",
            "find_parent(char *object) {",
            "    return object;",
            "}"
        )
    );
}

#[test]
fn pointer_align_name_keeps_assignment_operator_rvalue_reference_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!(
            "struct VarData {",
            "    VarData &operator=(VarData &&other) noexcept;",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct VarData {",
            "    VarData &operator=(VarData &&other) noexcept;",
            "};"
        )
    );
}

#[test]
fn pointer_align_name_preserves_source_spacing_after_parenthesized_pointer_declarator() {
    let mut options = FormatOptions::default();
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!(
            "void f(uint8_t (* restrict src)[3], uint32_t *restrict dst) {",
            "    return;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(uint8_t (* restrict src)[3], uint32_t *restrict dst) {",
            "    return;",
            "}"
        )
    );
}

#[test]
fn align_pointer_name_keeps_function_pointer_typedef_space() {
    let source = fixture!("typedef result_t (*build_fn)(void);");

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn align_pointer_name_keeps_pointer_to_pointer_cast_compact() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n\t*(Item **)((char *)base + offset) = value;\n}\n",
            &kr_c_options(),
        ),
        "void f(void)\n{\n    *(Item **)((char *)base + offset) = value;\n}\n",
    );
}

#[test]
fn align_pointer_name_keeps_suffix_type_parameter_attached_to_name() {
    let source = fixture!("int decode(const byte key[KEY_SIZE], item_tuple_t *tuple);");

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn pointer_space_inside_asm_operand_cast_is_preserved() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tasm(\"x\" : \"+r\" (crc) : ASM(*(u32 *)p));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    asm(\"x\" : \"+r\" (crc) : ASM(*(u32 *)p));\n}\n",
    );
}
#[test]
fn align_pointer_name_attaches_lowercase_struct_return_pointer() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--attach-return-type",
    ]);

    assert_eq!(
        format_c(
            "\nstatic inline struct item *item_init(struct item *item,\n                                    uint64_t value)\n{\n    return item;\n}\n",
            &options,
        ),
        "\nstatic inline struct item *item_init(struct item *item,\n                                     uint64_t value)\n{\n    return item;\n}\n"
    );
}

#[test]
fn align_pointer_name_attaches_macro_type_pointer_name() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
    ]);
    let source = "\nLIST_OF(Item) *items;\nstatic int load(LIST_OF(Item) **items_out);\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn macro_type_pointer_parameters_do_not_shift_following_parameter_indent() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--attach-return-type",
    ]);
    let source = "\nstatic void log_values(LIST_OF(Value) *first_values,\n                       LIST_OF(Value) *second_values,\n                       struct context *ctx)\n{\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pointer_align_middle_preserves_malformed_array_expression_gap() {
    // Pointer alignment does not govern expression whitespace around `^` and `*`.
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--align-pointer=middle".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            "\nfooArray[] = { red,\n               green:\n    ^          *blue\n             };\n",
            &options,
        ),
        "\nfooArray[] = { red,\n               green:\n               ^          *blue\n             };\n",
    );
}

#[test]
fn pad_operators_does_not_pad_pointer_declarator_or_cast_star() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "\nFilter::Filter(Result* result)\n    : Base(nullptr, &lock, ID_NULL)\n    , item(static_cast<Item*>(this), result)\n{}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn align_pointer_name_pad_paren_keeps_existing_gap_before_function_pointer_star() {
    let mut options = FormatOptions::default();
    let args = ["--align-pointer=name", "--pad-paren"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "\nclass Handler\n{\n    typedef void    * ( *AddHandler ) ( unsigned long );\n    typedef void* ( *AddHandler ) ( unsigned long );\n    typedef void *( *AddHandler ) ( unsigned long );\n};\n",
            &options,
        ),
        "\nclass Handler\n{\n    typedef void    * ( *AddHandler ) ( unsigned long );\n    typedef void * ( *AddHandler ) ( unsigned long );\n    typedef void * ( *AddHandler ) ( unsigned long );\n};\n",
    );
}
#[test]
fn const_pointer_array_cast_preserves_space_before_const_qualifier() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  call ((const char * const[]) { \"a\", NULL });\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    call ((const char * const[]) { \"a\", NULL });\n}\n",
    );
}

#[test]
fn pad_oper_keeps_range_for_reference_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "void f()\n{\n    for (const auto& x : data) {\n        g(x);\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_catch_clause_reference_and_pointer_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "void f()\n{\n    try {\n        a();\n    }\n    catch(const E& e) {\n        b();\n    }\n    catch(E* p) {\n        c();\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_reference_in_lambda_call_argument() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--pad-oper"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void g(){ call([&](auto& e){e*=2;}); }\n", &options,),
        "void g()\n{\n    call([&](auto& e)\n    {\n        e *= 2;\n    });\n}\n",
    );
}

#[test]
fn align_pointer_aligns_reference_param_in_assigned_lambda() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--align-pointer=name".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("auto h=[](int& e){return e;};\n", &options),
        "auto h=[](int &e) {\n    return e;\n};\n",
    );
}

#[test]
fn align_pointer_aligns_reference_in_second_decl_after_assignment() {
    let mut options = FormatOptions::default();
    let args = ["--style=pico", "--align-pointer=type"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("int*p=&a; int&r=a;\n", &options),
        "int* p=&a; int& r=a;\n",
    );
}

#[test]
fn pad_oper_keeps_structured_binding_reference_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("int main(){ auto&[a,b]=p; }\n", &options),
        "int main() {\n    auto&[a, b] = p;\n}\n",
    );
}

#[test]
fn structured_binding_reference_preserves_source_spacing() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("int main(){ auto& [a,b]=p; }\n", &options),
        "int main() {\n    auto& [a, b] = p;\n}\n",
    );
}

#[test]
fn align_reference_name_moves_structured_binding_reference_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--align-reference=name".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("int main(){ auto&[a,b]=p; }\n", &options),
        "int main() {\n    auto &[a,b]=p;\n}\n",
    );
}

#[test]
fn binary_and_after_call_result_on_continuation_keeps_operator_spacing() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--pad-oper",
        "--align-pointer=name",
        "--align-reference=name",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "static bool f(const Info_t *info)\n{\n    const bool value =\n        (helper(info->field.flags) & ALPHA_FLAG_DEBUG) != 0U;\n    return value;\n}\n";

    assert_eq!(format_c(source, &options), source);
}
