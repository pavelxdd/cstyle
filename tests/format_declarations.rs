#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, PointerAlign, apply_command_line_args};

#[test]
fn enum_declaration_inserts_gap_before_opening_brace() {
    assert_eq!(
        format_c(
            fixture!(
                "typedef enum ValueKind{",
                "    VALUE_INTEGER = (INTEGER),",
                "    VALUE_BOOL = (BOOL),",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "typedef enum ValueKind {",
            "    VALUE_INTEGER = (INTEGER),",
            "    VALUE_BOOL = (BOOL),",
        )
    );
}

#[test]
fn union_declaration_inserts_gap_before_opening_brace() {
    assert_eq!(
        format_c(fixture!("typedef union Value{"), &FormatOptions::default(),),
        fixture!("typedef union Value {")
    );
}

#[test]
fn struct_declaration_inserts_gap_before_opening_brace() {
    assert_eq!(
        format_c(
            fixture!("typedef struct StackInfo{"),
            &FormatOptions::default(),
        ),
        fixture!("typedef struct StackInfo {")
    );
}

#[test]
fn formats_bit_field_colons_as_declarators() {
    let actual = format(fixture!("struct S{unsigned x:3;unsigned y:5;};"));

    assert_eq!(
        actual,
        fixture!(
            "struct S",
            "{",
            "    unsigned x: 3;",
            "    unsigned y: 5;",
            "};",
        )
    );
}

#[test]
fn keeps_anonymous_bit_field_on_one_line() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "struct Config {",
            "int : 1;",
            "uint8_t : 0;",
            "uint16_t : 16;",
            "uint8_t alpha : 7, : 1, beta : 4;",
            "};"
        ),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "struct Config",
            "{",
            "    int : 1;",
            "    uint8_t : 0;",
            "    uint16_t : 16;",
            "    uint8_t alpha : 7, : 1, beta : 4;",
            "};"
        )
    );
}

#[test]
fn pad_operators_keeps_existing_multi_space_bit_field_gap() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "struct Config {",
            "uint32_t status   : 22;",
            "uint32_t          :  2;",
            "};"
        ),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "struct Config",
            "{",
            "    uint32_t status   : 22;",
            "    uint32_t          :  2;",
            "};"
        )
    );
}

#[test]
fn preserves_bit_field_colon_spacing_without_padding() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!(
                "struct Config {",
                "    field alpha : 1;",
                "    field beta : 2;",
                "    int gamma:3;",
                "};"
            ),
            &options
        ),
        fixture!(
            "struct Config {",
            "    field alpha : 1;",
            "    field beta : 2;",
            "    int gamma:3;",
            "};"
        )
    );
}

#[test]
fn bit_field_declarator_rows_keep_previous_declarator_indent() {
    let source = fixture!(
        "class C",
        "{",
        "    bool m_hasExplicitFgCol:1,",
        "         m_hasExplicitBgCol:1,",
        "         m_hasExplicitFont:1;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn formats_configured_access_labels_as_class_labels() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    options.access_labels = vec!["custom section".to_string(), "custom group".to_string()];
    let actual = format_with(
        fixture!(
            "class C {",
            "custom section:",
            "void add() const;",
            "custom group:",
            "void b();",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class C {",
            "custom section:",
            "    void add() const;",
            "custom group:",
            "    void b();",
            "};",
        )
    );
}

#[test]
fn qt_private_slots_macro_stays_at_access_specifier_column() {
    let source = fixture!(
        "class C",
        "{",
        "    Q_OBJECT",
        "public:",
        "    C();",
        "",
        "private Q_SLOTS:",
        "    void f();",
        "",
        "private:",
        "    int value;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn slot_access_label_uses_class_label_indent() {
    let source = "class C\n{\npublic:\n    C();\n\nprivate slots:\n    void f();\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn access_label_after_preprocessor_constructor_keeps_member_indent() {
    let source = fixture!(
        "class Item",
        "    : public Base",
        "{",
        "    Q_OBJECT",
        "",
        "public:",
        "    Item()",
        "#ifdef VALUE",
        "        : value(1)",
        "#endif",
        "    {",
        "    }",
        "",
        "private:",
        "    void call();",
        "    int value;",
        "",
        "private slots:",
        "    void init();",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn method_signature_after_preprocessor_constexpr_keeps_member_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "    template < typename CandidateCV, typename Candidate = detail::uncvref_t<CandidateCV>>",
        "#if ENABLE_CONSTEXPR",
        "    constexpr",
        "#endif",
        "    auto get() const noexcept(",
        "    noexcept(std::declval<const source_type&>().template convert<Candidate>(detail::priority_tag<4> {})))",
        "    -> decltype(std::declval<const source_type&>().template convert<Candidate>(detail::priority_tag<4> {}))",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn anonymous_struct_base_clause_stays_on_header_line() {
    let source = fixture!(
        "void f()",
        "{",
        "    struct : BaseJob {",
        "        void run() override",
        "        {",
        "        }",
        "    } worker;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn base_clause_preserves_source_gap_before_global_scope_qualifier() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source = fixture!("struct Item: public ::space::Base {", "};");

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn formats_type_declaration_blocks_with_trailing_semicolons() {
    let actual = format(fixture!(
        "struct S{int x;};",
        "union U{int x;float y;};",
        "enum E{A,B};",
    ));
    assert_eq!(
        actual,
        fixture!(
            "struct S",
            "{",
            "    int x;",
            "};",
            "union U",
            "{",
            "    int x;",
            "    float y;",
            "};",
            "enum E {A, B};",
        )
    );
}

#[test]
fn keeps_run_in_enum_values_on_brace_line_and_aligns_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "enum clock_id { CLOCK_CPU, CLOCK_PANEL, CLOCK_RUN,",
            "                CLOCK_3M, CLOCK_NUM_ITEMS",
            "              };"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "enum clock_id { CLOCK_CPU, CLOCK_PANEL, CLOCK_RUN,",
            "                CLOCK_3M, CLOCK_NUM_ITEMS",
            "              };"
        )
    );
}

#[test]
fn enum_opening_brace_on_next_line_stays_separate_with_run_in_values() {
    let source = fixture!(
        "enum class ValueKind : IntegerSize::Unsigned",
        "{ Zero, One, Two=2 };",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn enum_value_after_expression_with_trailing_block_comment_keeps_enum_indent() {
    let source = fixture!(
        "enum {",
        "    A = 5,",
        "    B = (1 << A) - 1, /* comment */",
        "    C = 24,",
        "    D = 25",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn operator_template_declaration_does_not_indent_following_macro() {
    let source = fixture!(
        "template<> OutputType& operator<< <InputValue>(OutputType& out, InputValue& value);",
        "",
        "TEST_ENTRY(genericRun);",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn user_defined_literal_operator_suffix_stays_attached() {
    let source = fixture!(
        "inline Value operator\"\"_value(const char* text, std::size_t size)",
        "{",
        "    return parse(text, size);",
        "}",
        "inline Value operator\"\" _value(const char* text, std::size_t size);",
        "using literals::operator\"\"_value;",
        "using literals::operator\"\" _value;",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_member_assignment_operator_noexcept_keeps_member_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "public:",
        "    Item& operator=(Item&&)",
        "    noexcept(condition",
        "             && other) = default;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn template_template_parameter_default_keeps_source_continuation_indent() {
    let source = fixture!(
        "",
        "template<typename T, template<typename E,",
        "                              typename Allocator = allocator<E> >",
        "         class Container = vector >",
        "class Foo",
        "{",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn class_member_template_logical_constraint_continuation_aligns_to_expression() {
    assert_eq!(
        format_c(
            "struct Item {\n  template <typename UInt, ENABLE_IF(std::is_same<UInt, uint64_t>::value ||\n                                     std::is_same<UInt, uint128_t>::value)>\n  void multiply(UInt value) {}\n};\n",
            &FormatOptions::default(),
        ),
        "struct Item {\n    template <typename UInt, ENABLE_IF(std::is_same<UInt, uint64_t>::value ||\n                                       std::is_same<UInt, uint128_t>::value)>\n    void multiply(UInt value) {}\n};\n",
    );
}

#[test]
fn multiline_template_parameter_default_logical_tail_stays_at_parameter_indent() {
    assert_eq!(
        format_c(
            "template <\n  typename T, typename U,\n  bool check = trait<T>::value &&\n      mapped_type<T, U>::value != custom,\n  ENABLE_IF(check)>\nauto write(Output out, T value) -> Output {\n  return out;\n}\n",
            &FormatOptions::default(),
        ),
        "template <\n    typename T, typename U,\n    bool check = trait<T>::value &&\n    mapped_type<T, U>::value != custom,\n    ENABLE_IF(check)>\nauto write(Output out, T value) -> Output {\n    return out;\n}\n",
    );
}

#[test]
fn multiline_template_declaration_aligns_constraint_and_function() {
    assert_eq!(
        format_c(
            "template <typename OutputIt,\n          ENABLE_IF(is_back_insert_iterator<OutputIt>::value&&\n                    is_contiguous<typename OutputIt::container>::value)>\ninline auto base_iterator(OutputIt it,\n                          typename OutputIt::container_type::value_type*)\n-> OutputIt {\n  return it;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename OutputIt,\n          ENABLE_IF(is_back_insert_iterator<OutputIt>::value&&\n                    is_contiguous<typename OutputIt::container>::value)>\ninline auto base_iterator(OutputIt it,\n                          typename OutputIt::container_type::value_type*)\n-> OutputIt {\n    return it;\n}\n",
    );
}

#[test]
fn constrained_single_line_template_keeps_following_function_at_base() {
    assert_eq!(
        format_c(
            fixture!(
                "template <typename To, typename From, ENABLE_IF(sizeof(To) > sizeof(From))>",
                "inline auto bit_cast(const From& from) -> To {",
                "  return To();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template <typename To, typename From, ENABLE_IF(sizeof(To) > sizeof(From))>",
            "inline auto bit_cast(const From& from) -> To {",
            "    return To();",
            "}",
        )
    );
}

#[test]
fn multiline_template_declaration_caps_overindented_constraint() {
    assert_eq!(
        format_c(
            "template <typename T>\nCONST auto to_pointer(Appender<T> it, size_t n) -> T* {\n  if (value) return nullptr;\n  return data;\n}\n\ntemplate <typename OutputIt,\n          REQUIRES_COND(is_back_insert_iterator<OutputIt>::value&&\n                             is_contiguous<typename OutputIt::container>::value)>\ninline auto base_iterator(OutputIt it,\n                          typename OutputIt::container_type::value_type*)\n-> OutputIt {\n  return it;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nCONST auto to_pointer(Appender<T> it, size_t n) -> T* {\n    if (value) return nullptr;\n    return data;\n}\n\ntemplate <typename OutputIt,\n          REQUIRES_COND(is_back_insert_iterator<OutputIt>::value&&\n                        is_contiguous<typename OutputIt::container>::value)>\ninline auto base_iterator(OutputIt it,\n                          typename OutputIt::container_type::value_type*)\n-> OutputIt {\n    return it;\n}\n",
    );
}

#[test]
fn struct_base_logical_continuation_stays_at_struct_indent() {
    let source = fixture!(
        "template <typename T, bool = is_value<T>::value>",
        "struct is_fast : bool_constant<traits<T>::first &&",
        "sizeof(T) <= sizeof(double)> {};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn struct_template_argument_logical_continuation_uses_one_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "template <typename T>",
                "struct Info<T, enable_if_t<trait<T>::first ||",
                "trait<T>::second ||",
                "trait<T>::third>> {",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template <typename T>",
            "struct Info<T, enable_if_t<trait<T>::first ||",
            "    trait<T>::second ||",
            "    trait<T>::third>> {",
            "};",
        )
    );
}

#[test]
fn split_struct_base_after_template_argument_list_stays_at_base() {
    assert_eq!(
        format_c(
            fixture!(
                "template <typename T>",
                "struct Info<T, enable_if_t<sizeof(check<T>()) != 0>>",
                "    : std::true_type {};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template <typename T>",
            "struct Info<T, enable_if_t<sizeof(check<T>()) != 0>>",
            ": std::true_type {};",
        )
    );
}

#[test]
fn template_base_after_wrapped_struct_header_aligns_to_header() {
    assert_eq!(
        format_c(
            "template <typename T>\nstruct Value<Item<T>>\n    : Base {};\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nstruct Value<Item<T>>\n                       : Base {};\n",
    );
}

#[test]
fn typedef_template_argument_rows_keep_two_level_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "typedef PixelEncoding<unsigned char, 32,",
                "                      NativePixelEncoding::RED,",
                "                      NativePixelEncoding::GREEN,",
                "                      PIXEL_ENCODING_ALPHA> AlphaPixelEncoding;",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "typedef PixelEncoding<unsigned char, 32,",
            "        NativePixelEncoding::RED,",
            "        NativePixelEncoding::GREEN,",
            "        PIXEL_ENCODING_ALPHA> AlphaPixelEncoding;",
        )
    );
}

#[test]
fn typedef_function_pointer_parameter_rows_indent_one_level() {
    let source = fixture!(
        "typedef int (*Handler) (/* comment */",
        "    Display* /* display */,",
        "    Event* /* event */",
        ");",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn typedef_inline_template_arguments_and_value_keep_two_level_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "struct X",
                "{",
                "    typedef typename T<A,",
                "                      B,",
                "                      C>::value",
                "            V;",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "struct X",
            "{",
            "    typedef typename T<A,",
            "            B,",
            "            C>::value",
            "            V;",
            "};",
        )
    );
}

#[test]
fn typedef_template_argument_rows_return_to_typedef_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "struct X",
                "{",
                "    typedef typename T",
                "            <",
                "                A,",
                "                B",
                "            >::value",
                "            value;",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "struct X",
            "{",
            "    typedef typename T",
            "    <",
            "    A,",
            "    B",
            "    >::value",
            "    value;",
            "};",
        )
    );
}

#[test]
fn split_class_export_name_keeps_base_clause_on_name_line() {
    assert_eq!(
        format_c(
            fixture!(
                "class EXPORT",
                "Name : public Base",
                "{",
                "public:",
                "    Name();",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class EXPORT",
            "    Name : public Base",
            "{",
            "public:",
            "    Name();",
            "};",
        )
    );
}

#[test]
fn base_clause_continuation_indents_one_level() {
    assert_eq!(
        format_c(
            "class Foo : public Bar,\npublic Baz\n{\npublic:\n    int x;\n};\n",
            &FormatOptions::default(),
        ),
        "class Foo : public Bar,\n    public Baz\n{\npublic:\n    int x;\n};\n",
    );
}

#[test]
fn exported_class_base_clause_continuation_uses_class_indent() {
    assert_eq!(
        format_c(
            "class EXPORT Widget : public Base<Control>,\n                                            public Handler\n{\npublic:\n    int x;\n};\n",
            &FormatOptions::default(),
        ),
        "class EXPORT Widget : public Base<Control>,\n    public Handler\n{\npublic:\n    int x;\n};\n",
    );
}

#[test]
fn colon_led_base_clause_indents_one_level() {
    let source = "class LIBRARY_API_FLAG Foo\n    : public Bar\n{\npublic:\n    int x;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn colon_led_base_clause_after_preprocessor_indents_one_level() {
    let source = "class ObjectTableBase\n#if !FEATURE_DATA_BACKEND\n    : public BaseType\n#endif\n{\npublic:\n    int x;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn colon_led_base_clause_in_preprocessor_else_branch_indents_one_level() {
    assert_eq!(
        format_c(
            "class TransportX\n#if FEATURE_NET_X\n : public ChannelClientX\n#else\n : public BaseType\n#endif\n{\npublic:\n    int x;\n};\n",
            &FormatOptions::default(),
        ),
        "class TransportX\n#if FEATURE_NET_X\n    : public ChannelClientX\n#else\n    : public BaseType\n#endif\n{\npublic:\n    int x;\n};\n",
    );
}

#[test]
fn comma_led_base_clause_continuation_indents_one_level() {
    let source = "class OptionItem : public OptionItemBase\n    , public CustomLayout\n{\npublic:\n    int x;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn comma_led_base_clause_after_preprocessor_indents_one_level() {
    let source = "class OptionItem : public OptionItemBase\n#if FEATURE_ALT_STYLE\n    , public CustomLayout\n#endif\n{\npublic:\n    int x;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_class_keyword_indents_macro_and_base_name() {
    assert_eq!(
        format_c(
            fixture!("class", "EXPORT", "Name : public Base", "{", "};"),
            &FormatOptions::default(),
        ),
        fixture!("class", "    EXPORT", "    Name : public Base", "{", "};",)
    );
    assert_eq!(
        format_c(
            fixture!("class", "EXPORT", "Name: public Base", "{", "};"),
            &FormatOptions::default(),
        ),
        fixture!("class", "    EXPORT", "    Name: public Base", "{", "};",)
    );
}

#[test]
fn split_template_class_base_after_template_argument_list_uses_one_indent() {
    assert_eq!(
        format_c(
            "template <typename T, typename U>\nclass Wrapper<std::basic_string<T, U>, T>\n    : public Base<T> {};\n\ntemplate <typename T, typename U>\nstruct Wrapper<T, U, void_t<result<T>>>\n    : Wrapper<result<T>, U> {\n  void f() {}\n};\n",
            &FormatOptions::default(),
        ),
        "template <typename T, typename U>\nclass Wrapper<std::basic_string<T, U>, T>\n    : public Base<T> {};\n\ntemplate <typename T, typename U>\nstruct Wrapper<T, U, void_t<result<T>>>\n    : Wrapper<result<T>, U> {\n    void f() {}\n};\n",
    );
}

#[test]
fn templated_struct_name_base_clause_continuation_indents_one_level() {
    let source = "template<>\nstruct Finder<TextData>\n    : public Finder<const TextData&> {};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn base_clause_continuation_after_nested_template_close_keeps_one_level() {
    assert_eq!(
        format_c(
            "class Foo : public Alpha<Beta<int>>,\n            public Gamma\n{\n};\n",
            &FormatOptions::default(),
        ),
        "class Foo : public Alpha<Beta<int>>,\n    public Gamma\n{\n};\n",
    );
}

#[test]
fn split_template_trait_base_after_nested_template_argument_aligns_to_header() {
    assert_eq!(
        format_c(
            "template <typename T, typename U, typename A>\nstruct is_contiguous<std::basic_string<T, U, A>>\n    : std::true_type {};\n",
            &FormatOptions::default(),
        ),
        "template <typename T, typename U, typename A>\nstruct is_contiguous<std::basic_string<T, U, A>>\n            : std::true_type {};\n",
    );
}

#[test]
fn template_specialization_base_clause_keeps_continuation_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename T>",
                "struct iterator_traits < T, enable_if_t < !std::is_pointer<T>::value >>",
                "    : iterator_types<T>",
                "{",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename T>",
            "struct iterator_traits < T, enable_if_t < !std::is_pointer<T>::value >>",
            "            : iterator_types<T>",
            "{",
            "};",
        )
    );
}

// Split partial-specialization contents do not change class-body indentation.
#[test]
fn split_partial_specialization_body_uses_normal_class_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename ContainerType>",
                "struct container_value_adapter_factory< ContainerType,",
                "       void_t<decltype(begin(std::declval<ContainerType>()), end(std::declval<ContainerType>()))>>",
                "       {",
                "           using adapter_type = int;",
                "",
                "           static adapter_type create(const ContainerType& container)",
                "{",
                "    return value_adapter(begin(container), end(container));",
                "}",
                "       };",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename ContainerType>",
            "struct container_value_adapter_factory< ContainerType,",
            "       void_t<decltype(begin(std::declval<ContainerType>()), end(std::declval<ContainerType>()))>>",
            "{",
            "    using adapter_type = int;",
            "",
            "    static adapter_type create(const ContainerType& container)",
            "    {",
            "        return value_adapter(begin(container), end(container));",
            "    }",
            "};",
        )
    );
}

#[test]
fn same_line_template_class_does_not_overindent_switch_case_body() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename IteratorType> class Item",
                "{",
                "public:",
                "    string_type get() const",
                "    {",
                "        switch (type)",
                "        {",
                "            case object:",
                "                return key();",
                "            default:",
                "                return empty;",
                "        }",
                "    }",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename IteratorType> class Item",
            "{",
            "public:",
            "    string_type get() const",
            "    {",
            "        switch (type)",
            "        {",
            "        case object:",
            "            return key();",
            "        default:",
            "            return empty;",
            "        }",
            "    }",
            "};",
        )
    );
}

#[test]
fn void_t_template_argument_continuation_keeps_enclosing_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename It>",
                "struct iterator_types <",
                "    It,",
                "    void_t<typename It::difference_type, typename It::value_type, typename It::pointer,",
                "typename It::reference, typename It::iterator_category >>",
                "{};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename It>",
            "struct iterator_types <",
            "    It,",
            "    void_t<typename It::difference_type, typename It::value_type, typename It::pointer,",
            "    typename It::reference, typename It::iterator_category >>",
            "{};",
        )
    );
}

#[test]
fn formats_trailing_return_declaration_inside_function_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;

    assert_eq!(
        format_with(fixture!("void f(){auto g(int x)->int;}"), &options),
        fixture!("void f()", "{", "    auto g(int x)->int;", "}",)
    );
}

#[test]
fn attaches_declarator_names_after_type_declaration_closing_braces() {
    let actual = format(fixture!(
        "typedef struct{int x;} T;",
        "union U{int x;} u;",
        "enum E{A,B} e;",
    ));
    assert_eq!(
        actual,
        fixture!(
            "typedef struct",
            "{",
            "    int x;",
            "} T;",
            "union U",
            "{",
            "    int x;",
            "} u;",
            "enum E {A, B} e;",
        )
    );
}

#[test]
fn aggregate_closing_brace_keeps_space_before_declarator() {
    let source = fixture!(
        "",
        "typedef struct {",
        "    int value;",
        "} Item;",
        "",
        "struct {",
        "    int value;",
        "} items[2];",
        "",
        "struct {",
        "    int value;",
        "} __packed item;",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn closing_brace_before_pointer_declarator_keeps_source_space() {
    let source = "class Outer {\n    class Rep {\n        int m_ref;\n    } *m_rep;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_aggregate_pointer_declarator_keeps_member_indent() {
    assert_eq!(
        format_c(
            "struct Outer {\n  struct Inner {\n    int value;\n  } items[5],\n    *current;\n  int next;\n};\n",
            &FormatOptions::default(),
        ),
        "struct Outer {\n    struct Inner {\n        int value;\n    } items[5],\n    *current;\n    int next;\n};\n",
    );
}

#[test]
fn array_bound_operator_continuation_aligns_under_line_end_operator() {
    assert_eq!(
        format_c(
            "\nstruct Item {\n    int values[PARAM_ALPHA -\n               PARAM_BETA + 1];\n    long values2[PARAM_ALPHA -\n                 PARAM_BETA + 1];\n};\n",
            &FormatOptions::default(),
        ),
        "\nstruct Item {\n    int values[PARAM_ALPHA -\n                           PARAM_BETA + 1];\n    long values2[PARAM_ALPHA -\n                             PARAM_BETA + 1];\n};\n",
    );
}

#[test]
fn struct_array_bound_continuation_is_not_overindented() {
    assert_eq!(
        format_c(
            "\nstruct Item {\n    struct Type values[PARAM_ALPHA -\n                       PARAM_BETA + 1];\n};\n",
            &FormatOptions::default(),
        ),
        "\nstruct Item {\n    struct Type values[PARAM_ALPHA -\n                                   PARAM_BETA + 1];\n};\n",
    );
}

#[test]
fn struct_split_declaration_name_gets_continuation_indent() {
    assert_eq!(
        format_c(
            "\nstruct Entry\nname;\nconst struct Entry\nitems[2] = {\n    {1},\n};\nunion Item *\nptr;\nenum Kind\nkind;\n",
            &FormatOptions::default(),
        ),
        "\nstruct Entry\n    name;\nconst struct Entry\n    items[2] = {\n    {1},\n};\nunion Item *\n    ptr;\nenum Kind\nkind;\n",
    );
}

#[test]
fn attaches_pointer_declarator_after_anonymous_struct_brace() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "    struct {",
                "        int base;",
                "    } *ctx = arg;",
                "    union {",
                "        int u;",
                "    } **pp = 0;",
                "}"
            ),
            &options
        ),
        fixture!(
            "void f(void)",
            "{",
            "    struct {",
            "        int base;",
            "    } *ctx = arg;",
            "    union {",
            "        int u;",
            "    } **pp = 0;",
            "}"
        )
    );
}

#[test]
fn aligns_declaration_comma_continuation_to_second_word() {
    let actual = format_c(
        fixture!(
            "void helper(void){",
            "verylongtypename gamma,",
            "delta,",
            "epsilon;",
            "int alpha,",
            "beta;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void helper(void) {",
            "    verylongtypename gamma,",
            "                     delta,",
            "                     epsilon;",
            "    int alpha,",
            "        beta;",
            "}",
        )
    );
}

// Every leading type form uses one assignment-continuation level.
#[test]
fn struct_and_enum_assignment_continuations_use_same_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tstruct t q =",
                "\t\tv1;",
                "\tenum e b =",
                "\t\tv2;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    struct t q =",
            "        v1;",
            "    enum e b =",
            "        v2;",
            "}",
        )
    );
}

#[test]
fn keeps_multiline_template_angles_at_statement_indent() {
    assert_eq!(
        format_c(fixture!("List<", "int>", "v;"), &FormatOptions::default()),
        fixture!("List<", "int>", "v;")
    );

    assert_eq!(
        format_c(
            fixture!("void f(void) {", "List<", "int>", "v;", "}"),
            &FormatOptions::default()
        ),
        fixture!("void f(void) {", "    List<", "    int>", "    v;", "}")
    );
}

#[test]
fn formats_multiline_enum_members_without_comma_continuation_indent() {
    let actual = format(fixture!(
        "enum {",
        "A = 1,",
        "B = 2,",
        "};",
        "typedef enum {",
        "X = 1,",
        "Y = 2",
        "} name_t;",
    ));
    // Source-attached enum braces remain attached regardless of preceding declarations.
    assert_eq!(
        actual,
        fixture!(
            "enum {",
            "    A = 1,",
            "    B = 2,",
            "};",
            "typedef enum {",
            "    X = 1,",
            "    Y = 2",
            "} name_t;",
        )
    );
}

#[test]
fn gnu_enum_brace_layout_does_not_depend_on_prior_type_definition() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");
    let actual = format_c(
        fixture!(
            "struct Data{",
            "int value;",
            "};",
            "enum Kind{",
            "Alpha,",
            "Beta",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct Data",
            "{",
            "    int value;",
            "};",
            "enum Kind {",
            "    Alpha,",
            "    Beta",
            "};",
        )
    );
}

#[test]
fn indent_classes_indents_signal_and_slot_labels_with_class_members() {
    let mut options = FormatOptions::default();
    options.indent_classes = true;
    let actual = format_c(
        fixture!(
            "class Item{",
            "signals:",
            "void changed();",
            "public slots:",
            "void run();",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item {",
            "    signals:",
            "        void changed();",
            "    public slots:",
            "        void run();",
            "};",
        )
    );
}

#[test]
fn indent_modifiers_indents_signal_and_slot_labels_half_a_level() {
    let mut options = FormatOptions::default();
    options.indent_modifiers = true;
    let actual = format_c(
        fixture!(
            "class Item{",
            "signals:",
            "void changed();",
            "public slots:",
            "void run();",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "class Item {",
            "  signals:",
            "    void changed();",
            "  public slots:",
            "    void run();",
            "};",
        )
    );
}

#[test]
fn indent_modifiers_indents_struct_access_labels_half_a_level() {
    let mut options = FormatOptions::default();
    options.indent_modifiers = true;

    assert_eq!(
        format_c(fixture!("struct Item", "{", "private:"), &options),
        fixture!("struct Item", "{", "  private:"),
    );
}

#[test]
fn indent_modifiers_does_not_indent_union_access_labels() {
    let mut options = FormatOptions::default();
    options.indent_modifiers = true;
    let actual = format_c(
        fixture!(
            "union Item{",
            "public:",
            "int alpha;",
            "private:",
            "long beta;",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "union Item {",
            "public:",
            "    int alpha;",
            "private:",
            "    long beta;",
            "};",
        )
    );
}

#[test]
fn indent_modifiers_does_not_indent_interface_access_labels() {
    let mut options = FormatOptions::default();
    options.indent_modifiers = true;
    let actual = format_c(
        fixture!(
            "interface Item{",
            "public:",
            "void run();",
            "private:",
            "int value;",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "interface Item {",
            "public:",
            "    void run();",
            "private:",
            "    int value;",
            "};",
        )
    );
}

// Every union or interface access label uses the label column.
#[test]
fn horstmann_union_and_interface_access_labels_use_the_label_column() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("union Item{", "public:", "int value;", "};"),
            &options,
        ),
        fixture!("union Item", "{", "public:", "    int value;", "};")
    );
    assert_eq!(
        format_c(
            fixture!("interface Item{", "public:", "int value;", "};"),
            &options,
        ),
        fixture!("interface Item", "{", "public:", "    int value;", "};")
    );
}

// Signal labels use the access-label column in every option state.
#[test]
fn horstmann_signal_label_run_in_uses_the_access_label_column() {
    let source = fixture!(
        "class Item{",
        "signals:",
        "void changed();",
        "public slots:",
        "void run();",
        "};",
    );

    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class Item",
            "{",
            "signals:",
            "    void changed();",
            "public slots:",
            "    void run();",
            "};",
        )
    );

    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent-classes".to_owned(),
        ],
    )
    .expect("valid options");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class Item",
            "{   signals:",
            "        void changed();",
            "    public slots:",
            "        void run();",
            "};",
        )
    );

    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=horstmann".to_owned(),
            "--indent-modifiers".to_owned(),
        ],
    )
    .expect("valid options");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class Item",
            "{ signals:",
            "    void changed();",
            "  public slots:",
            "    void run();",
            "};",
        )
    );
}

#[test]
fn whitesmith_breaks_access_label_from_one_line_class_member() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("class Item{public:int value;};\n", &options),
        fixture!(
            "class Item",
            "    {",
            "    public:",
            "        int value;",
            "    };",
        )
    );
}

#[test]
fn enum_member_value_does_not_overindent_following_members() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "enum E {",
            "    A_NONE       = 0x000, /**< No event. */",
            "    A_READ       = 0x001, /**< Read. */",
            "    A_WRITE      = 0x002, /**< Write. */",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "enum E {",
            "    A_NONE       = 0x000, /**< No event. */",
            "    A_READ       = 0x001, /**< Read. */",
            "    A_WRITE      = 0x002, /**< Write. */",
            "};",
        )
    );
}

#[test]
fn scoped_enum_member_value_does_not_overindent_following_members() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "enum class Scoped {",
            "    A = 0,",
            "    B = 1,",
            "    C = 2",
            "};",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "enum class Scoped {",
            "    A = 0,",
            "    B = 1,",
            "    C = 2",
            "};",
        )
    );
}

#[test]
fn stroustrup_style_attaches_class_base_clause_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=stroustrup".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "class Derived : public Base {\nprotected:\n    Derived() {}\n};\n",
            &options,
        ),
        "class Derived : public Base {\nprotected:\n    Derived() {}\n};\n"
    );
}

#[test]
fn mozilla_style_breaks_struct_and_enum_braces() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=mozilla".to_owned()]).expect("valid options");
    let struct_source = "typedef struct _item\n{\n    int value;\n} Item;\n";
    let enum_source = "typedef enum _item\n{\n    Alpha,\n} Item;\n";

    assert_eq!(format_c(struct_source, &options), struct_source);
    assert_eq!(format_c(enum_source, &options), enum_source);
}

#[test]
fn enum_underlying_type_colon_stays_on_header_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=1tbs".to_owned(),
            "--mode=c".to_owned(),
            "--pad-oper".to_owned(),
            "--align-pointer=name".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "\nenum : uint8_t {\n    VALUE_A = 1,\n\n    VALUE_B = 64 * 1024,\n};\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn enum_member_after_blank_comment_line_uses_enum_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=1tbs".to_owned(),
            "--mode=c".to_owned(),
            "--pad-oper".to_owned(),
            "--align-pointer=name".to_owned(),
        ],
    )
    .expect("valid options");
    let source = "\nenum : size_t {\n    A = 1 << 16, // note\n\n    B = 64 * 1024 * 1024,\n};\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn whitesmith_class_base_brace_uses_class_body_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let source = fixture!(
        "class Item:",
        "public Base<Alpha,",
        "Beta>,",
        "private Other{",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "class Item:",
            "    public Base<Alpha,",
            "    Beta>,",
            "    private Other",
            "    {",
            "    };",
        )
    );
}

#[test]
fn multi_variable_declaration_aligns_continuations_to_first_declarator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "const uint32_t alpha  = 0xD1A881,",
            "beta   = call(0xD02590),",
            "gamma  = call(0xD0259A),",
            "delta  = 0xD3FFFF;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    const uint32_t alpha  = 0xD1A881,",
            "                   beta   = call(0xD02590),",
            "                   gamma  = call(0xD0259A),",
            "                   delta  = 0xD3FFFF;",
            "}"
        )
    );
}

#[test]
fn struct_multi_declarator_keeps_continuation_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=1tbs".to_owned()]).expect("valid options");
    let source = fixture!(
        "struct Config {",
        "    int alpha,",
        "        beta,",
        "        gamma;",
        "};",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn initialized_member_declarator_rows_align_to_first_name() {
    let source = fixture!(
        "struct S",
        "{",
        "    size_t m_size = 0,",
        "           m_count = 0;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multi_declarator_member_continuation_aligns_under_first_declarator() {
    let source = "class C {\nprivate:\n    RgbValue m_rowText,\n             m_rowBack;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multi_declarator_after_line_comment_keeps_declarator_indent() {
    let source = fixture!(
        "class C",
        "{",
        "    Type *a,",
        "         *b,",
        "         // comment",
        "         *c;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multi_declarator_continuation_row_keeps_source_column() {
    let source = fixture!(
        "class C",
        "{",
        "    Type32    mx, my,        // comment",
        "              mx0, my0,",
        "              mx1, my1;",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multi_declarator_with_paren_initializer_aligns_next_name() {
    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "    Buffer first(size),",
                "                 second(size);",
                "    Date start(1, 2, 3),",
                "         end = start.get();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void run()",
            "{",
            "    Buffer first(size),",
            "           second(size);",
            "    Date start(1, 2, 3),",
            "         end = start.get();",
            "}",
        )
    );
}

#[test]
fn aligns_continued_type_alias_declarations() {
    let actual = format(fixture!(
        "void f(){",
        "ValueType alpha, beta, gamma,",
        "delta, epsilon;",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    ValueType alpha, beta, gamma,",
            "              delta, epsilon;",
            "}",
        )
    );
}

#[test]
fn top_level_using_alias_rhs_uses_one_continuation_indent() {
    assert_eq!(
        format_c(
            "template <typename T, bool value = check<T>()>\nusing Result =\n        conditional_t<trait<T>::value || value, int, T>;\n",
            &FormatOptions::default(),
        ),
        "template <typename T, bool value = check<T>()>\nusing Result =\n    conditional_t<trait<T>::value || value, int, T>;\n",
    );
}

#[test]
fn using_alias_nested_template_argument_continuation_aligns_under_argument() {
    assert_eq!(
        format_c(
            fixture!(
                "struct C{",
                "    template<typename It> using r=typename std::enable_if<std::is_convertible<typename std::iterator_traits<It>::iterator_category,",
                "std::input_iterator_tag>::value>::type;",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "struct C {",
            "    template<typename It> using r=typename std::enable_if<std::is_convertible<typename std::iterator_traits<It>::iterator_category,",
            "                                std::input_iterator_tag>::value>::type;",
            "};",
        )
    );
}

#[test]
fn using_alias_std_function_parameter_row_aligns_under_first_parameter() {
    assert_eq!(
        format_c(
            "class C\n{\n    using Handler = std::function<void(const HttpData &request, HttpData &response,\n          ResponseControl &control)>;\n};\n",
            &FormatOptions::default(),
        ),
        "class C\n{\n    using Handler = std::function<void(const HttpData &request, HttpData &response,\n                                       ResponseControl &control)>;\n};\n",
    );
}

#[test]
fn std_conditional_type_alias_rows_keep_first_argument_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "struct Item {",
                "    using reference =",
                "        typename std::conditional<std::is_const<ValueType>::value,",
                "typename ValueType::const_reference,",
                "typename ValueType::reference>::type;",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "struct Item {",
            "    using reference =",
            "        typename std::conditional<std::is_const<ValueType>::value,",
            "        typename ValueType::const_reference,",
            "        typename ValueType::reference>::type;",
            "};",
        )
    );
}

#[test]
fn std_conditional_template_argument_rows_keep_first_argument_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "template<typename T>",
                "using value_base_type = typename std::conditional <",
                "                        std::is_same<T, void>::value,",
                "item_default_base,",
                "T",
                "                        >::type;",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "template<typename T>",
            "using value_base_type = typename std::conditional <",
            "                        std::is_same<T, void>::value,",
            "                        item_default_base,",
            "                        T",
            "                        >::type;",
        )
    );
}

#[test]
fn using_alias_rhs_after_split_template_function_parameter_stays_at_body_indent() {
    assert_eq!(
        format_c(
            "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r,\n                 context_id ctx = {}) -> Result {\n  using resolved_type =\n      selector_type<check<T>::value, unsigned char, unsigned>;\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r,\ncontext_id ctx = {}) -> Result {\n    using resolved_type =\n    selector_type<check<T>::value, unsigned char, unsigned>;\n}\n",
    );
}

#[test]
fn preserves_top_level_qualifier_split_at_base_indent() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("static", "int value;"), &options),
        fixture!("static", "int value;")
    );
}

#[test]
fn whitesmith_indents_class_members_and_member_function_braces() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;

    assert_eq!(
        format_c(
            fixture!("class Item { int value; int read() { return value; } };"),
            &options,
        ),
        fixture!(
            "class Item",
            "    {",
            "        int value;",
            "        int read()",
            "            {",
            "            return value;",
            "            }",
            "    };",
        )
    );
    assert_eq!(
        format_c(
            fixture!(
                "class Item {",
                "int read()",
                "{",
                "return value;",
                "}",
                "};",
            ),
            &options,
        ),
        fixture!(
            "class Item",
            "    {",
            "        int read()",
            "            {",
            "            return value;",
            "            }",
            "    };",
        )
    );
}

#[test]
fn enum_value_assignment_inside_preprocessor_branch_indents_one_level() {
    assert_eq!(
        format_c(
            "class C\n{\n    enum Category {\n        Alpha\n#ifdef MODE_TEST\n        = 1'234'567\n#endif\n    };\n};\n",
            &FormatOptions::default(),
        ),
        "class C\n{\n    enum Category {\n        Alpha\n#ifdef MODE_TEST\n            = 1'234'567\n#endif\n    };\n};\n",
    );
}

#[test]
fn nested_template_declaration_assignment_aligns_value_to_declarator() {
    assert_eq!(
        format_c(
            "void f()\n{\n    std::optional<Items<ValueT>> sampleValuesList =\n        db.readValue(Header::PayloadLength);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    std::optional<Items<ValueT>> sampleValuesList =\n                                  db.readValue(Header::PayloadLength);\n}\n",
    );
}

#[test]
fn vtk_indents_member_function_body_below_definition() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Vtk;

    assert_eq!(
        format_c(
            fixture!(
                "class Item {",
                "int read()",
                "{",
                "return value;",
                "}",
                "};",
            ),
            &options,
        ),
        fixture!(
            "class Item",
            "{",
            "    int read()",
            "    {",
            "        return value;",
            "    }",
            "};",
        )
    );
}

#[test]
fn keeps_named_bit_field_macro_width_on_one_line() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "typedef struct {",
            "unsigned result: RESULT_BITS;",
            "uint8_t flag: 1;",
            "} item_t;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "typedef struct {",
            "    unsigned result: RESULT_BITS;",
            "    uint8_t flag: 1;",
            "} item_t;",
        )
    );
}

#[test]
fn macro_width_bitfields_stay_on_one_line() {
    let source = "\nstatic const struct {\n    unsigned offset:WORD_BITS / 2;\n    unsigned width:WORD_BITS / 2;\n} bit_info[] = {\n    VALUE\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn enum_member_after_missing_comma_continues_previous_member() {
    let actual = format_c(
        fixture!(
            "enum E",
            "{",
            "A = 1   //!< comment",
            "B = 2,  //!< comment",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "enum E",
            "{",
            "    A = 1   //!< comment",
            "        B = 2,  //!< comment",
            "};",
        )
    );
}

#[test]
fn enum_member_after_multiline_call_member_starts_at_body_indent() {
    let actual = format_c(
        fixture!(
            "enum E",
            "{",
            "A = B |",
            "    C(",
            "            B),",
            "",
            "/**",
            "    Comment.",
            "*/",
            "D = X |",
            "    Y,",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "enum E",
            "{",
            "    A = B |",
            "        C(",
            "            B),",
            "",
            "    /**",
            "        Comment.",
            "    */",
            "    D = X |",
            "        Y,",
            "};",
        )
    );
}

#[test]
fn enum_assignment_operator_continuation_survives_line_backslash() {
    let source = fixture!(
        "enum E",
        "{",
        "    ALL =",
        "        A| \\",
        "        B",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn template_continuation_reuses_previous_parameter_indent() {
    let actual = format_c(
        fixture!(
            "template<typename Class,",
            "typename T0,",
            "typename T1>",
            "struct X {};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "template<typename Class,",
            "         typename T0,",
            "         typename T1>",
            "struct X {};",
        )
    );
}

#[test]
fn template_continuation_survives_line_continuation_backslash() {
    let actual = format_c(
        fixture!(
            "template<typename Class,",
            "typename T0, \\",
            "typename T1>",
            "struct X {};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "template<typename Class,",
            "         typename T0, \\",
            "         typename T1>",
            "struct X {};",
        )
    );
}

#[test]
fn run_in_enum_assignment_continuation_aligns_after_value() {
    let actual = format_c(
        fixture!(
            "struct C",
            "{",
            "    enum { VALUE =",
            "        A | B };",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "struct C",
            "{",
            "    enum { VALUE =",
            "               A | B",
            "         };",
            "};",
        )
    );
}

#[test]
fn enum_assignment_operator_continuation_aligns_under_value() {
    assert_eq!(
        format_c(
            fixture!(
                "enum {",
                "    FIRST = 0x01,",
                "    VALUE =  ALPHA |",
                "    BETA,",
                "    LAST,",
                "};",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "enum {",
            "    FIRST = 0x01,",
            "    VALUE =  ALPHA |",
            "             BETA,",
            "    LAST,",
            "};",
        ),
    );
}

#[test]
fn enum_member_value_continuation_keeps_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=1tbs".to_owned(), "--pad-oper".to_owned()],
    )
    .expect("valid options");
    let source = fixture!(
        "enum {",
        "    MASK =",
        "        ALPHA |",
        "        BETA |",
        "        GAMMA,",
        "    NEXT = DELTA,",
        "};",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn split_semicolonless_macro_call_does_not_indent_following_member() {
    let actual = format_c(
        fixture!(
            "class C {",
            "    INLINE_MEMBER_MACRO(void f(int x),",
            "        m_a = (T)x; )",
            "    void g();",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "class C {",
            "    INLINE_MEMBER_MACRO(void f(int x),",
            "                        m_a = (T)x; )",
            "    void g();",
            "};",
        )
    );
}

#[test]
fn multiline_macro_arg_with_struct_does_not_indent_following_function() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "MACRO(value,\n\t struct Item, field)\n\nstatic void helper(struct Item *a,\n\t struct Item *b)\n{\n\tcall();\n}\n",
            &options,
        ),
        "MACRO(value,\n      struct Item, field)\n\nstatic void helper(struct Item *a,\n                   struct Item *b)\n{\n    call();\n}\n",
    );
}

#[test]
fn pointer_declarators_after_assigned_declarator_use_base_indent() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    TextType *families = new TextType[3],",
            "             *styles = new TextType[3],",
            "             *weights = new TextType[3];",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    TextType *families = new TextType[3],",
            "    *styles = new TextType[3],",
            "    *weights = new TextType[3];",
            "}",
        )
    );
}

// The base-clause continuation stays under the partial-specialization header.
#[test]
fn partial_specialization_base_clause_uses_stable_indent() {
    let actual = format_c(
        fixture!(
            "template<typename T>",
            "struct iterator_traits < T, enable_if_t < !std::is_pointer<T>::value >>",
            ": iterator_types<T>",
            "{",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "template<typename T>",
            "struct iterator_traits < T, enable_if_t < !std::is_pointer<T>::value >>",
            "            : iterator_types<T>",
            "{",
            "};",
        )
    );
}

// A broken template header does not turn its definition into a continuation.
#[test]
fn templated_struct_with_broken_brace_stays_at_column_zero() {
    let source = "template <class Key, class T,\n          class A = std::allocator<std::pair<const Key, T>>>\nstruct S : std::vector<std::pair<const Key, T>, A>\n{\n    int x;\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn struct_tag_named_class_keeps_uniform_member_indent() {
    // In C mode, `class` is an ordinary struct-tag identifier.
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "static const struct class vc = {\n\t.name = \"vc\",\n};\n",
            &options,
        ),
        "static const struct class vc = {\n    .name = \"vc\",\n};\n",
    );
}

#[test]
fn nested_inline_enum_definition_brace_is_padded_consistently() {
    assert_eq!(
        format_c(
            "void f() {\n    enum E{A, B};\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    enum E {A, B};\n}\n"
    );
}

#[test]
fn template_parameter_continuations_keep_source_alignment() {
    let source = "\ntemplate < class X,\n           class Y >\ntemplate <\n    class X,\n    class Y >\nvoid foo()\n{}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn template_parameter_continuation_aligns_after_open_angle() {
    assert_eq!(
        format_c(
            "template<typename Class,\ntypename T0, typename T1>\nstruct C {};\n",
            &FormatOptions::default(),
        ),
        "template<typename Class,\n         typename T0, typename T1>\nstruct C {};\n",
    );
}

#[test]
fn template_parameter_list_closing_angle_matches_continuation_indent() {
    assert_eq!(
        format_c(
            "template <\n    typename itemT, typename policyT,\n    typename RT, typename ...moreT\n>\nRefValue<itemT> f();\n",
            &FormatOptions::default(),
        ),
        "template <\n    typename itemT, typename policyT,\n    typename RT, typename ...moreT\n    >\nRefValue<itemT> f();\n",
    );
}

#[test]
fn empty_statement_after_semicolon_stays_inline() {
    let source = "int f(int a);;\nint g(int b);\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn anonymous_class_declarator_keeps_space_after_brace() {
    let source = "void g() {\n    class {\n        int m;\n    } f(h1);\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn run_in_indent_classes_keeps_member_indent_on_brace_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=run-in".to_owned(), "--indent-classes".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c("class FooClass\n{\n    bool value;\n};\n", &options),
        "class FooClass\n{       bool value;\n};\n",
    );
}
#[test]
fn run_in_indent_classes_keeps_access_label_on_brace_line() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=run-in".to_owned(), "--indent-classes".to_owned()],
    )
    .expect("valid options");
    let source = "class FooClass\n{   private:\n        bool var1;\n};\n";

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn run_in_class_brace_does_not_run_in_access_label() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=run-in".to_owned()]).expect("valid options");
    let source = "class FooClass\n{\nprivate:\n    bool var1;\n};\n";

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn exported_class_nested_template_base_continuation_keeps_one_level() {
    assert_eq!(
        format_c(
            "class EXPORT Foo : public Base<Nested<Control>>,\n                                            public Interface\n{\n};\n",
            &FormatOptions::default(),
        ),
        "class EXPORT Foo : public Base<Nested<Control>>,\n    public Interface\n{\n};\n",
    );
}

#[test]
fn ratliff_indents_class_body_one_level_extra() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("class C { int y; };\n", &options),
        "class C {\n        int y;\n    };\n",
    );
}

#[test]
fn ratliff_indents_access_labels_and_members() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=ratliff".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "class C {\npublic:\nint a;\nprivate:\nint b;\n};\n",
            &options,
        ),
        "class C {\n    public:\n        int a;\n    private:\n        int b;\n    };\n",
    );
}

#[test]
fn mozilla_breaks_one_line_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=mozilla".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color { RED, GREEN, BLUE };\n", &options),
        "enum Color\n{ RED, GREEN, BLUE };\n",
    );
}

#[test]
fn allman_keeps_one_line_enum_inline() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color { RED, GREEN, BLUE };\n", &options),
        "enum Color { RED, GREEN, BLUE };\n",
    );
}

// Break-base styles keep the source position of multi-line enum opening braces.
#[test]
fn allman_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN\n};\n",
    );
}

#[test]
fn allman_keeps_source_broken_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color\n{\nRED,\nGREEN\n};\n", &options),
        "enum Color\n{\n    RED,\n    GREEN\n};\n",
    );
}

#[test]
fn gnu_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=gnu".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN\n};\n",
    );
}

#[test]
fn whitesmith_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=whitesmith".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN\n    };\n",
    );
}

#[test]
fn vtk_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=vtk".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN\n};\n",
    );
}

#[test]
fn horstmann_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=horstmann".to_owned()])
        .expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN\n};\n",
    );
}

#[test]
fn pico_keeps_source_attached_enum_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=pico".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum Color {\nRED,\nGREEN\n};\n", &options),
        "enum Color {\n    RED,\n    GREEN };\n",
    );
}

#[test]
fn mozilla_breaks_enum_underlying_type_opening_brace() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=mozilla".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("enum class Color:int { Red, Green };\n", &options),
        "enum class Color:int\n{ Red, Green };\n",
    );
}

#[test]
fn horstmann_run_in_member_indent_classes_handles_template_header() {
    let mut options = FormatOptions::default();
    let args = ["--style=horstmann", "--indent-classes"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "template<typename T> class Vec { T* data; public: T a; };\n",
            &options,
        ),
        "template<typename T> class Vec\n{       T* data;\n    public:\n        T a;\n};\n",
    );
}

#[test]
fn horstmann_run_in_access_label_uses_label_indent_fill() {
    let mut options = FormatOptions::default();
    let args = ["--style=horstmann", "--indent-modifiers"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "class Widget : public Base {\npublic:\nint a;\nprivate:\nint b;\n};\n",
            &options,
        ),
        "class Widget : public Base\n{ public:\n    int a;\n  private:\n    int b;\n};\n",
    );
}

#[test]
fn mozilla_breaks_enum_brace_under_keep_one_line_blocks() {
    let source = "enum E { A, B, C };\nint main() { return 0; }\n";
    let expected = "enum E\n{ A, B, C };\nint main() { return 0; }\n";

    for option in ["--keep-one-line-blocks", "--add-one-line-braces"] {
        let mut options = FormatOptions::default();
        let args = ["--style=mozilla", option].map(str::to_owned);
        apply_command_line_args(&mut options, &args).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn google_style_indents_access_labels_and_members() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=google".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("class C{public:int x;};\n", &options),
        "class C {\n  public:\n    int x;\n};\n",
    );
}

#[test]
fn indent_classes_overrides_style_implied_indent_modifiers() {
    let source = "class Config\n{\npublic:\nint a;\nprivate:\nint b;\n};\n";
    let expected =
        "class Config {\n    public:\n        int a;\n    private:\n        int b;\n};\n";
    let cases: &[&[&str]] = &[
        &["--style=google", "--indent-classes"],
        &["--style=attach", "--indent-modifiers", "--indent-classes"],
    ];

    for arguments in cases {
        let mut options = FormatOptions::default();
        let arguments: Vec<_> = arguments.iter().map(|value| (*value).to_owned()).collect();
        apply_command_line_args(&mut options, &arguments).expect("valid options");
        assert_eq!(format_c(source, &options), expected);
    }
}

#[test]
fn leading_unicode_identifier_preserves_source_separator() {
    let source = "int α;\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn dollar_sign_extension_identifier_preserves_declaration_spacing() {
    assert_eq!(
        format_c(
            "int $value=1;\nvoid $call(){return;}\n",
            &FormatOptions::default(),
        ),
        "int $value=1;\nvoid $call() {\n    return;\n}\n",
    );
}
