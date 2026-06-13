#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, PointerAlign, apply_command_line_args};

#[test]
fn splits_return_types_when_requested() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;
    options.break_return_type_decl = true;
    let actual = format_with(
        fixture!("static int f(int x){return x;}", "unsigned long g(void);",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static int",
            "f(int x)",
            "{",
            "    return x;",
            "}",
            "unsigned long",
            "g(void);",
        )
    );
}

#[test]
fn attaches_already_broken_return_types_when_requested() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    options.attach_return_type_decl = true;
    let actual = format_with(
        fixture!(
            "static int",
            "f(int x)",
            "{return x;}",
            "static int",
            "    helper(int alpha, int beta,",
            "           int gamma)",
            "{return gamma;}",
            "unsigned long",
            "g(void);",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static int f(int x)",
            "{",
            "    return x;",
            "}",
            "static int helper(int alpha, int beta,",
            "                  int gamma)",
            "{",
            "    return gamma;",
            "}",
            "unsigned long g(void);",
        )
    );
}

#[test]
fn linux_function_parameter_closing_paren_aligns_to_open_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "int wrapper_func_name(unsigned char *buf, long in_len,",
                "\tlong (*fill)(void *x)",
                "\t)",
                "{",
                "}",
            ),
            &options,
        ),
        fixture!(
            "int wrapper_func_name(unsigned char *buf, long in_len,",
            "                      long (*fill)(void *x)",
            "                     )",
            "{",
            "}",
        )
    );
}

#[test]
fn linux_split_double_pointer_parameter_aligns_to_first_parameter() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "static int alloc_adapter(struct config_adapter",
                "\t**adapter_ref)",
                "{",
                "}",
            ),
            &options,
        ),
        fixture!(
            "static int alloc_adapter(struct config_adapter",
            "                         **adapter_ref)",
            "{",
            "}",
        )
    );
}

#[test]
fn attach_return_type_options_keep_split_struct_pointer_return_type() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--lineend=linux",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
        "--align-reference=name",
        "--attach-return-type",
        "--attach-return-type-decl",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = fixture!(
        "struct Type *",
        "open_item(const char *name, int version)",
        "{",
        "    struct Item *item;",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn attach_return_type_keeps_comments_and_unconfigured_macros_separate() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    let actual = format_with(
        fixture!(
            "// keep",
            "int",
            "f(void)",
            "{return 0;}",
            "#define API int",
            "API",
            "g(void)",
            "{return 1;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "// keep",
            "int f(void)",
            "{",
            "    return 0;",
            "}",
            "#define API int",
            "API",
            "g(void)",
            "{",
            "    return 1;",
            "}",
        )
    );
}

#[test]
fn attach_return_type_does_not_glue_body_after_macro_like_header() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--add-braces",
        "--pad-oper",
        "--pad-comma",
        "--break-after-logical",
        "--attach-return-type",
        "--break-one-line-headers",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "int parse(int magic, int flen)",
                "{",
                "    if (magic != ALPHA ||",
                "        flen < GAMMA) {",
                "        call(out,",
                "             ERR, 1);",
                "        return E;",
                "    }",
                "}",
            ),
            &options,
        ),
        fixture!(
            "int parse(int magic, int flen)",
            "{",
            "    if (magic != ALPHA ||",
            "            flen < GAMMA) {",
            "        call(out,",
            "             ERR, 1);",
            "        return E;",
            "    }",
            "}",
        )
    );
}

#[test]
fn attach_return_type_decl_does_not_join_else_comment_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.attach_return_type_decl = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    if (a) {",
            "        if (b) {",
            "            inner();",
            "        }",
            "    } else { // TAG",
            "        helper(loop, op, fd, ev);",
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
            "    if (a) {",
            "        if (b) {",
            "            inner();",
            "        }",
            "    } else { // TAG",
            "        helper(loop, op, fd, ev);",
            "    }",
            "}",
        )
    );
}

#[test]
fn break_return_type_keeps_scope_qualifier_with_name() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;
    let actual = format_c(
        fixture!("int Ns::foo(void)", "{", "    return 0;", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("int", "Ns::foo(void)", "{", "    return 0;", "}")
    );
}

#[test]
fn break_return_type_decl_keeps_scope_operator_with_name() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;
    let actual = format_c(fixture!("bool Cls::operator==(int a);"), &options);

    assert_eq!(actual, fixture!("bool", "Cls::operator==(int a);"));
}

#[test]
fn break_return_type_leaves_conversion_operator_without_return_type() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;

    assert_eq!(
        format_c("Type::operator bool() const{}\n", &options),
        fixture!("Type::operator bool() const {}")
    );
    // Conversion operators have no return type, including `explicit` ones.
    assert_eq!(
        format_c(
            "explicit Type::operator bool() const{return true;}\n",
            &options,
        ),
        fixture!(
            "explicit Type::operator bool() const {",
            "    return true;",
            "}",
        )
    );
}

#[test]
fn return_type_options_do_not_split_control_headers() {
    let source = "void run(){if(ready){one();}else if(other()){two();}else call();}\n";

    let mut definitions = FormatOptions::default();
    definitions.break_return_type = true;
    assert_eq!(
        format_c(source, &definitions),
        fixture!(
            "void",
            "run() {",
            "    if(ready) {",
            "        one();",
            "    }",
            "    else if(other()) {",
            "        two();",
            "    }",
            "    else call();",
            "}",
        )
    );

    let mut declarations = FormatOptions::default();
    declarations.break_return_type_decl = true;
    assert_eq!(
        format_c(source, &declarations),
        fixture!(
            "void run() {",
            "    if(ready) {",
            "        one();",
            "    }",
            "    else if(other()) {",
            "        two();",
            "    }",
            "    else call();",
            "}",
        )
    );
}

#[test]
fn break_return_type_handles_named_allocation_operators() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;

    assert_eq!(
        format_c(
            "void *Type::operator new(unsigned long size){return allocate(size);}\n",
            &options,
        ),
        fixture!(
            "void *",
            "Type::operator new(unsigned long size) {",
            "    return allocate(size);",
            "}",
        )
    );
    assert_eq!(
        format_c(
            "void Type::operator delete(void *value){release(value);}\n",
            &options,
        ),
        fixture!(
            "void",
            "Type::operator delete(void *value) {",
            "    release(value);",
            "}",
        )
    );
}

#[test]
fn break_return_type_handles_template_and_comment_prefixes() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;

    assert_eq!(
        format_c(
            "template<class T>\nT value(T item){return item;}\n",
            &options,
        ),
        fixture!(
            "template<class T>",
            "T",
            "value(T item) {",
            "    return item;",
            "}",
        )
    );
    assert_eq!(
        format_c("long long /* note */ value(){return 1;}\n", &options),
        fixture!("long long /* note */", "value() {", "    return 1;", "}",)
    );
}

#[test]
fn whitesmith_return_type_options_preserve_scope_operators() {
    let source = "const Item *ns::Type::value(){return nullptr;}\n";

    let mut split = FormatOptions::default();
    split.brace_style = BraceStyle::Whitesmith;
    split.indent_braces = true;
    split.indent_classes = true;
    split.indent_switches = true;
    split.break_return_type = true;
    assert_eq!(
        format_c(source, &split),
        fixture!(
            "const Item *",
            "ns::Type::value()",
            "    {",
            "    return nullptr;",
            "    }",
        )
    );

    let mut attach = split.clone();
    attach.break_return_type = false;
    attach.attach_return_type = true;
    assert_eq!(
        format_c(
            "const Item *\nns::Type::value(){return nullptr;}\n",
            &attach,
        ),
        fixture!(
            "const Item *ns::Type::value()",
            "    {",
            "    return nullptr;",
            "    }",
        )
    );
}

#[test]
fn break_return_type_supports_leading_cpp_attribute() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;

    assert_eq!(
        format_c("[[nodiscard]] long long value(){return 1;}\n", &options),
        fixture!("[[nodiscard]] long long", "value() {", "    return 1;", "}",)
    );
}

#[test]
fn break_return_type_leaves_destructor_without_return_type() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;
    let actual = format_c(
        fixture!("Cls::~Cls(void)", "{", "    clean();", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("Cls::~Cls(void)", "{", "    clean();", "}")
    );
}

#[test]
fn attach_return_type_rejoins_qualified_name() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    let actual = format_c(
        fixture!("int", "Ns::foo(void)", "{", "    return 0;", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("int Ns::foo(void)", "{", "    return 0;", "}")
    );
}

#[test]
fn attach_return_type_does_not_join_initializer_continuations() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    options.attach_return_type_decl = true;
    let actual = format_with(
        fixture!(
            "static const option_t options[] = {",
            "{",
            "1, MAX_CONNECTIONS, 0, 0,",
            "offsetof(server_t, max_connections)",
            "},",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static const option_t options[] = {",
            "    {",
            "        1, MAX_CONNECTIONS, 0, 0,",
            "        offsetof(server_t, max_connections)",
            "    },",
            "};",
        )
    );
}

#[test]
fn splits_return_type_before_overloaded_operator_functions() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;
    options.pad_operators = true;
    let actual = format_with(fixture!("T operator+(T a,T b){return a+b;}"), &options);

    assert_eq!(
        actual,
        fixture!("T", "operator+(T a, T b)", "{", "    return a + b;", "}",)
    );
}

#[test]
fn allman_breaks_attached_trailing_return_and_control_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=break".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("auto run() -> void {", "    if (ready) {"),
            &options,
        ),
        fixture!("auto run() -> void", "{", "    if (ready)", "    {",)
    );
}

#[test]
fn allman_keeps_broken_trailing_return_brace_and_breaks_control_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=break".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("auto run() -> void", "{", "    if (ready) {"),
            &options,
        ),
        fixture!("auto run() -> void", "{", "    if (ready)", "    {",)
    );
}

#[test]
fn kr_breaks_trailing_return_definition_brace_and_attaches_control_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=kr".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            fixture!("auto run() -> void {", "    if (ready) {"),
            &options,
        ),
        fixture!("auto run() -> void", "{", "    if (ready) {")
    );
}

#[test]
fn whitesmith_split_trailing_return_brace_uses_definition_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;

    assert_eq!(
        format_c(fixture!("auto fn(", " int x)->int", "{", "}"), &options),
        fixture!("auto fn(", "    int x)->int", "    {", "    }")
    );
}

#[test]
fn allman_breaks_trailing_return_one_line_function_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;

    assert_eq!(
        format_c("auto f()->int{return 1;}\n", &options),
        "auto f()->int\n{\n    return 1;\n}\n",
    );
}

#[test]
fn trailing_return_member_one_line_body_breaks_body() {
    assert_eq!(
        format_c(
            "struct Value {\n  int value_;\npublic:\n  constexpr auto value() const noexcept -> int { return value_; }\n};\n",
            &FormatOptions::default(),
        ),
        "struct Value {\n    int value_;\npublic:\n    constexpr auto value() const noexcept -> int {\n        return value_;\n    }\n};\n",
    );
}

#[test]
fn split_struct_return_type_keeps_function_name_at_top_level() {
    let source = fixture!(
        "",
        "static struct Item * __init",
        "select_item(struct Item *a, struct Item *b)",
        "{",
        "    return a;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn splits_return_types_with_pointers_qualifiers_and_macros() {
    let mut options = FormatOptions::default();
    options.break_return_type = true;
    options.break_return_type_decl = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "static const char *name(void){return 0;}",
            "API unsigned long value(int x);",
            "int (*factory(void))(int);",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static const char *",
            "name(void)",
            "{",
            "    return 0;",
            "}",
            "API unsigned long",
            "value(int x);",
            "int (*factory(void))(int);",
        )
    );
}

#[test]
fn attaches_core_typedef_pointer_return_types() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "static off_t *",
            "    helper(time_t *when, uintmax_t *size, atomic_uint_fast64_t *counter, int128_t *wide)",
            "{return counter ? size : wide;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static off_t *helper(time_t *when, uintmax_t *size, atomic_uint_fast64_t *counter, int128_t *wide)",
            "{",
            "    return counter ? size : wide;",
            "}",
        )
    );
}

#[test]
fn attaches_standard_pointer_return_types() {
    let mut options = FormatOptions::default();
    options.attach_return_type = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "static char *",
            "    helper(char *value)",
            "{return value;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static char *helper(char *value)",
            "{",
            "    return value;",
            "}",
        )
    );
}

#[test]
fn keeps_split_pointer_return_type_flush() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("static char *", "helper(void)", "{", "return 0;", "}",),
            &options,
        ),
        fixture!("static char *", "helper(void)", "{", "    return 0;", "}",)
    );
}

#[test]
fn split_function_declaration_open_paren_line_aligns_parameters_to_open_paren() {
    assert_eq!(
        format_c(
            "class C\n{\nprivate:\n    static bool checkForInputValues\n        (const AbstractDataSource& context,\n         AbstractDataSource::ItemOption options);\n};\n",
            &FormatOptions::default(),
        ),
        "class C\n{\nprivate:\n    static bool checkForInputValues\n    (const AbstractDataSource& context,\n     AbstractDataSource::ItemOption options);\n};\n",
    );
}

#[test]
fn templated_split_function_parameter_continuation_stays_at_base() {
    assert_eq!(
        format_c(
            "template <typename T>\nFIXED auto apply(Result dst, source_view input,\n                 const rules& r = {}) -> Result {\n  return dst;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nFIXED auto apply(Result dst, source_view input,\nconst rules& r = {}) -> Result {\n    return dst;\n}\n",
    );
}

#[test]
fn constrained_template_split_function_default_parameter_tail_stays_at_base() {
    assert_eq!(
        format_c(
            "template <typename T, typename Result,\n          SELECT_IF(check<T>::value)>\nauto apply(Result dst, const T* value, const rules& r = {},\n           context_id = {}) -> Result {\n  return dst;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T, typename Result,\n          SELECT_IF(check<T>::value)>\nauto apply(Result dst, const T* value, const rules& r = {},\ncontext_id = {}) -> Result {\n    return dst;\n}\n",
    );
}

#[test]
fn templated_split_function_default_parameter_tail_after_multiple_rows_stays_at_base() {
    assert_eq!(
        format_c(
            "template <typename T>\nFIXED auto apply_value(Result dst, const T& value,\n                       int span, Kind separator_key,\n                       const rules& r, mode value_mode,\n                       context_id ctx = {}) -> Result {\n  return dst;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nFIXED auto apply_value(Result dst, const T& value,\n                       int span, Kind separator_key,\n                       const rules& r, mode value_mode,\ncontext_id ctx = {}) -> Result {\n    return dst;\n}\n",
    );
}

#[test]
fn constrained_template_split_function_trailing_return_stays_at_base() {
    assert_eq!(
        format_c(
            "template <typename Char, typename ResultIt, typename UInt,\n          SELECT_IF(is_standard_output_type<ResultIt>::value)>\nFIXED inline auto compute_value(int input_key, ResultIt dst, UInt value,\n                                int item_count, bool exact = false)\n    -> ResultIt {\n  return dst;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename Char, typename ResultIt, typename UInt,\n          SELECT_IF(is_standard_output_type<ResultIt>::value)>\nFIXED inline auto compute_value(int input_key, ResultIt dst, UInt value,\n                                int item_count, bool exact = false)\n-> ResultIt {\n    return dst;\n}\n",
    );
}

#[test]
fn parameter_default_empty_value_continuation_indents_one_level() {
    let actual = format_c(
        fixture!(
            "class C",
            "{",
            "    void f(int a,",
            "           Flags flags =",
            "           Flags::Null );",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class C",
            "{",
            "    void f(int a,",
            "           Flags flags =",
            "               Flags::Null );",
            "};",
        )
    );
}

#[test]
fn parameter_default_operator_continuation_aligns_after_value_start() {
    let actual = format_c(
        fixture!(
            "class C",
            "{",
            "    static bool f(int value = FIRST|",
            "                                     SECOND|",
            "                                     THIRD);",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class C",
            "{",
            "    static bool f(int value = FIRST|",
            "                              SECOND|",
            "                              THIRD);",
            "};",
        )
    );
}

#[test]
fn class_member_function_parameter_continuation_is_not_overindented() {
    let actual = format_c(
        fixture!(
            "struct S {",
            "    virtual bool parse_error(std::size_t position,",
            "    const std::string& last_token,",
            "    const exception& ex) = 0;",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "struct S {",
            "    virtual bool parse_error(std::size_t position,",
            "                             const std::string& last_token,",
            "                             const exception& ex) = 0;",
            "};",
        )
    );
}

#[test]
fn function_parameter_continuation_aligns_past_space_after_open_paren() {
    assert_eq!(
        format_c(
            "class Foo {\n    void Repaint( bool clearBackground,\n                  const Region *area = nullptr ) override { call(); }\n};\n",
            &FormatOptions::default(),
        ),
        "class Foo {\n    void Repaint( bool clearBackground,\n                  const Region *area = nullptr ) override {\n        call();\n    }\n};\n",
    );
}

#[test]
fn leading_comma_parameter_keeps_parameter_column_after_assigned_param() {
    let source = "class C\n{\n    void f(int a,\n           int b = 5\n           , int c\n          );\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn long_return_type_param_continuation_aligns_under_first_param() {
    let source = "struct S\n{\n    DirectImageSurfacePtr CreateCompatible(const Extent& size = DefaultExtent,\n                                           int flags = 0);\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// Braced temporaries do not change trailing-return continuation ownership.
#[test]
fn trailing_return_decltype_with_braced_temporary_keeps_continuation_indent() {
    let source = "template<typename B, typename A,\n         enable_if_t<cond<A>::value, int> = 0>\nauto f(const B& j, A& arr)\n-> decltype(g(j, arr, tag<3> {}),\nj.get<A>(),\nvoid())\n{\n}\n";
    let expected = "template<typename B, typename A,\n         enable_if_t<cond<A>::value, int> = 0>\nauto f(const B& j, A& arr)\n-> decltype(g(j, arr, tag<3> {}),\n            j.get<A>(),\n            void())\n{\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

// Braced temporaries do not change `noexcept` continuation ownership.
#[test]
fn noexcept_continuation_with_braced_temporary_keeps_normal_indent() {
    let source = "struct S {\n    static auto f(J&& j) noexcept(\n    noexcept(h(j, tag<T> {})))\n    {\n        return h(j, tag<T> {});\n    }\n};\n";
    let expected = "struct S {\n    static auto f(J&& j) noexcept(\n        noexcept(h(j, tag<T> {})))\n    {\n        return h(j, tag<T> {});\n    }\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), expected,);
}

#[test]
fn linux_style_caps_deep_function_parameter_continuation() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");
    let source = "\nstatic inline const struct Config *helper(unsigned long value,\n        unsigned long size)\n{\n    return 0;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn comma_first_function_arguments_keep_source_alignment() {
    assert_eq!(
        format_c(
            "\nstatic void set_info(const std::string& name\n                      , const std::string& user = \"\"\n                      , const std::string host = \"localhost\"\n                      , const unsigned int port = 3306);\n",
            &FormatOptions::default(),
        ),
        "\nstatic void set_info(const std::string& name\n                     , const std::string& user = \"\"\n                     , const std::string host = \"localhost\"\n                     , const unsigned int port = 3306);\n",
    );
}

#[test]
fn split_union_return_function_parameters_get_extra_continuation_indent() {
    assert_eq!(
        format_c(
            "\nstatic inline union Value check_value(\n    int first,\n    int second)\n{\n    return get_value();\n}\n",
            &FormatOptions::default(),
        ),
        "\nstatic inline union Value check_value(\n        int first,\n        int second)\n{\n    return get_value();\n}\n",
    );
}

#[test]
fn split_return_type_with_attribute_paren_keeps_name_at_base() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "static format_attr(3, 0) struct Item *\ncreate_item(unsigned int flags)\n{\n\treturn 0;\n}\n",
            &options,
        ),
        "static format_attr(3, 0) struct Item *\ncreate_item(unsigned int flags)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn split_return_type_pointer_function_name_stays_at_base() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "static inline const struct Info\n\t*helper(struct Port *port)\n{\n\treturn port->info;\n}\n",
            &options,
        ),
        "static inline const struct Info\n*helper(struct Port *port)\n{\n    return port->info;\n}\n",
    );
}

#[test]
fn split_function_definition_template_pointer_return_type_keeps_name_at_base() {
    let source =
        "template <typename T>\ninline Foo<T> *\nmakeFoo(const T& a, int b)\n{\n    return 0;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_function_definition_multi_arg_template_pointer_return_type() {
    let source = "template <typename A, typename B>\ninline Bar<A, B> *\nmakeBar(const A& a, const B &b)\n{\n    return 0;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// Return-prefix wrapping does not change definition-brace placement.
#[test]
fn wrapped_return_type_function_brace_breaks_consistently() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "static enum Result value_attribute\ncompute_value(char *id)\n{\n\treturn 0;\n}\n",
            &options,
        ),
        "static enum Result value_attribute\ncompute_value(char *id)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn function_declaration_params_align_under_tab_expanded_paren() {
    assert_eq!(
        format_c(
            "static int\t    read_status\t\t\t (Context     *context,\n\t\t\t\t\t  StatusEvent *state);\n",
            &FormatOptions::default(),
        ),
        "static int\t    read_status\t\t\t (Context     *context,\n                                      StatusEvent *state);\n",
    );
}

#[test]
fn function_declaration_params_cap_at_tab_expanded_max_continuation_indent() {
    assert_eq!(
        format_c(
            "StatusId parse_accelerator_key\t\t      (const char      *source_name,\n\t\t\t\t\t       Count\t       *result_value_id);\n",
            &FormatOptions::default(),
        ),
        "StatusId parse_accelerator_key\t\t      (const char      *source_name,\n        Count\t       *result_value_id);\n",
    );
}

// Return-type splitting does not change definition-brace placement.
#[test]
fn split_return_type_function_keeps_brace_on_own_line() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "static enum Result\ncompute_value(char *id)\n{\n\treturn 0;\n}\n",
            &options,
        ),
        "static enum Result\ncompute_value(char *id)\n{\n    return 0;\n}\n",
    );
}

#[test]
fn break_return_type_decl_keeps_member_call_statement() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(
        format_c("void f(){data.push_back(v);}\n", &options),
        "void f() {\n    data.push_back(v);\n}\n",
    );
}

#[test]
fn break_return_type_decl_keeps_pointer_member_call_statement() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(
        format_c("void f(){ptr->method(v);}\n", &options),
        "void f() {\n    ptr->method(v);\n}\n",
    );
}

#[test]
fn break_return_type_decl_keeps_do_while_tail() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(
        format_c("void f(){do{x++;}while(n<5);}\n", &options),
        "void f() {\n    do {\n        x++;\n    }\n    while(n<5);\n}\n",
    );
}

#[test]
fn break_return_type_decl_keeps_comment_prefixed_call() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(
        format_c("void f()\n{\n/*inline*/ g();\n}\n", &options),
        "void f()\n{\n    /*inline*/ g();\n}\n",
    );
}

#[test]
fn break_return_type_decl_keeps_multiline_do_while_tail() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(
        format_c("void f(int n)\n{\ndo{\nn++;\n}while(n<5);\n}\n", &options,),
        "void f(int n)\n{\n    do {\n        n++;\n    } while(n<5);\n}\n",
    );
}

#[test]
fn break_return_type_decl_still_splits_function_declaration() {
    let mut options = FormatOptions::default();
    options.break_return_type_decl = true;

    assert_eq!(format_c("int foo();\n", &options), "int\nfoo();\n");
}
