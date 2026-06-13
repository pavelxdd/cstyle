#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::api::format_bytes;
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
fn preserves_existing_initializer_edge_spaces() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    let actual = format_with(
        fixture!("pair_t token = { 0, source.data };", "int a[]={1,2};"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("pair_t token = { 0, source.data };", "int a[] = {1, 2};")
    );
}

#[test]
fn pad_operators_keeps_prefix_increment_and_dereference_compact() {
    let source = fixture!(
        "void helper(int *ptr)",
        "{",
        "    ++*ptr;",
        "    --*ptr;",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pad_operators_keeps_multiplication_padded_after_macro_call() {
    let source = fixture!(
        "bool helper(double diff, double alpha, double beta, double eps)",
        "{",
        "    return diff <= MAX(alpha, beta) * eps;",
        "}",
    );

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn pad_operators_distinguishes_unary_minus_from_alignof_subtraction() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    size_t offset = -(uintptr_t)buffer & (alignof(item_t) - 1);",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pad_operators_keeps_line_start_unary_minus_attached_in_call_arguments() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    assert_equal(make_time(value,",
        "                           now(),",
        "                           -1, -1, 0), 1);",
        "    log_error(\"%d\",",
        "              -result);",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn pad_commas_keeps_inserted_gap_before_lambda_capture() {
    let mut options = FormatOptions::default();
    options.pad_commas = true;

    assert_eq!(
        format_with("call(value,[](int item) { return item; });\n", &options),
        fixture!("call(value, [](int item)", "{", "    return item;", "});"),
    );
}

#[test]
fn pad_commas_pads_after_default_macro_argument() {
    let options =
        options_from_args(&["--style=1tbs", "--mode=c", "--lineend=linux", "--pad-comma"]);

    assert_eq!(
        format_c(
            fixture!(
                "void Foo()",
                "{",
                "    ADD_KEYWORD(return, TK_RETURN);",
                "    ADD_KEYWORD(switch, TK_SWITCH);",
                "    ADD_KEYWORD(case,TK_CASE);",
                "    ADD_KEYWORD(default,TK_DEFAULT);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void Foo()",
            "{",
            "    ADD_KEYWORD(return, TK_RETURN);",
            "    ADD_KEYWORD(switch, TK_SWITCH);",
            "    ADD_KEYWORD(case, TK_CASE);",
            "    ADD_KEYWORD(default, TK_DEFAULT);",
            "}",
        )
    );
}

#[test]
fn empty_macro_argument_between_commas_has_no_padding() {
    assert_eq!(
        format_c(
            "REGISTER_TYPE(PartialTag, PARTIAL, CONST_FUNCTION, Order::partial, , )\n",
            &FormatOptions::default(),
        ),
        "REGISTER_TYPE(PartialTag, PARTIAL, CONST_FUNCTION, Order::partial,, )\n",
    );
}

#[test]
fn pad_operators_preserves_full_line_conflict_markers() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "<<<<<<< ours",
            "int a=1+2;",
            "=======",
            "int a=3+4;",
            ">>>>>>> theirs",
            "int b=5+6;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "<<<<<<< ours",
            "int a = 1 + 2;",
            "=======",
            "int a = 3 + 4;",
            ">>>>>>> theirs",
            "int b = 5 + 6;",
        )
    );
}

#[test]
fn pad_operators_keeps_space_before_line_splice() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c("int value=left +\\\n right;\n", &options);

    assert_eq!(actual, "int value = left + \\\n            right;\n");
}

#[test]
fn pad_operators_pads_class_base_colon_after_colon_only() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "namespace n{class A:public B,private C{void f();};}\n",
            &options,
        ),
        "namespace n\n{\nclass A: public B, private C\n{\n    void f();\n};\n}\n",
    );
}

#[test]
fn pad_operators_commas_and_headers_keep_space_before_nested_designated_initializer_brace() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;

    assert_eq!(
        format_c("void f(){Data data={.a=1,.b={2,3}};}\n", &options),
        "void f()\n{\n    Data data = {.a = 1, .b = {2, 3}};\n}\n",
    );
}

#[test]
fn formats_unary_and_ternary_operators() {
    let actual = format(fixture!(
        "int f(int x){return x? -1:+1; x=a*-b + !c + ~d; --x; x++;}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "int f(int x)",
            "{",
            "    return x ? -1 : +1;",
            "    x = a * -b + !c + ~d;",
            "    --x;",
            "    x++;",
            "}",
        )
    );
}
#[test]
fn pad_operators_distinguishes_cast_dereference_and_multiplication() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_c(
        fixture!(
            "void f(){",
            "value=(const char *)\"x\";",
            "other=*(T *)ptr;",
            "product=(a*b);",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    value = (const char *)\"x\";",
            "    other = *(T *)ptr;",
            "    product = (a * b);",
            "}"
        )
    );
}

#[test]
fn pad_operators_distinguishes_reference_address_of_and_bitwise_and() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_c(
        fixture!(
            "void f(){",
            "T &ref=item;",
            "ptr=&item;",
            "mask=left&right;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    T &ref = item;",
            "    ptr = &item;",
            "    mask = left & right;",
            "}"
        )
    );
}

#[test]
fn nopad_marker_only_suppresses_operator_padding() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;
    options.pad_parens_outside = true;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;

    assert_eq!(
        format_c("if(-x) call(a,-b); // *NOPAD*\n", &options),
        "if ( -x ) call ( a, -b ); // *NOPAD*\n"
    );
}

#[test]
fn nopad_marker_does_not_suppress_comma_padding() {
    let source = "call(a,b); // *NOPAD*\n";
    let expected = "call(a, b); // *NOPAD*\n";

    let mut options = FormatOptions::default();
    options.pad_commas = true;
    assert_eq!(format_c(source, &options), expected);

    options.pad_commas = false;
    options.pad_operators = true;
    assert_eq!(format_c(source, &options), expected);
}

#[test]
fn nopad_preserves_existing_spaceship_spacing() {
    let source = fixture!("auto value = left <=> right; // *NOPAD*");

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nopad_keeps_noexcept_separate_from_operator_function() {
    assert_eq!(
        format_c(
            fixture!(
                "#if FEATURE",
                "    inline std::partial_ordering operator<=>(const Value lhs, const Value rhs) noexcept // *NOPAD*",
                "#else",
                "    inline bool operator<(const Value lhs, const Value rhs) noexcept",
                "#endif",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "#if FEATURE",
            "inline std::partial_ordering operator<=>(const Value lhs, const Value rhs) noexcept // *NOPAD*",
            "#else",
            "inline bool operator<(const Value lhs, const Value rhs) noexcept",
            "#endif",
            "{",
            "}",
        )
    );
}

#[test]
fn comma_padding_keeps_following_line_comment_adjacent() {
    let source = "void f(){call(a,// note\n b);}\n";
    let expected = fixture!("void f() {", "    call(a,// note", "         b);", "}");
    assert_eq!(format_c(source, &FormatOptions::default()), expected);

    let mut options = FormatOptions::default();
    options.pad_commas = true;
    assert_eq!(format_c(source, &options), expected);

    options.pad_commas = false;
    options.pad_operators = true;
    assert_eq!(format_c(source, &options), expected);
}

#[test]
fn unpadded_postfix_and_unary_signs_keep_source_adjacency() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c("int f(){return a+++b+a---b;}\n", &options),
        fixture!("int f() {", "    return a+++b+a---b;", "}")
    );
}

#[test]
fn pad_operators_keeps_numeric_cast_unary_sign_attached_to_number() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(){x=(int)-1; y=(foo)-bar; z=(size_t)+2;",
            "a=sizeof(size_t)+64; b=sizeof(foo)+64;}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    x = (int) -1;",
            "    y = (foo) - bar;",
            "    z = (size_t) +2;",
            "    a = sizeof(size_t) +64;",
            "    b = sizeof(foo) + 64;",
            "}",
        )
    );
}

#[test]
fn array_size_subtraction_keeps_binary_minus_space() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "struct Item {\n\tunsigned char data[sizeof(size_t) - 2];\n};\n",
            &options,
        ),
        "struct Item {\n    unsigned char data[sizeof(size_t) - 2];\n};\n",
    );
}

#[test]
fn unpadded_trailing_line_comment_stays_adjacent_to_operator() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c("int f(){return a+// note\n b;}\n", &options),
        fixture!("int f() {", "    return a+// note", "           b;", "}")
    );
}

#[test]
fn pad_operators_keeps_binary_gap_before_fold_ellipsis() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "template<class... T> auto sum(T... value){return (value+...);}\n",
            &options
        ),
        fixture!(
            "template<class... T> auto sum(T... value) {",
            "    return (value + ...);",
            "}",
        )
    );
}

#[test]
fn pad_operators_spaces_overloaded_comma_parameter_list() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("Item operator,(Item);\n", &options),
        "Item operator, (Item);\n"
    );
}

#[test]
fn pad_operators_keeps_operator_parameter_reference_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("bool operator==(const Item&);\n", &options),
        "bool operator==(const Item&);\n"
    );
}

#[test]
fn pad_operators_formats_add_braces_rewritten_body() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;
    let actual = format_c(
        fixture!("void f()", "{", "if (flag)", "product=left*right;", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (flag)",
            "    {",
            "        product = left * right;",
            "    }",
            "}"
        )
    );
}

#[test]
fn pad_operators_formats_remove_braces_rewritten_body() {
    let mut options = FormatOptions::default();
    options.remove_braces = true;
    options.brace_style = BraceStyle::Allman;
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "if (flag)",
            "{",
            "product=left*right;",
            "}",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (flag)",
            "        product = left * right;",
            "}"
        )
    );
}

#[test]
fn pad_operators_preserves_macro_star_argument() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_c(fixture!("void f(){", "MACRO(*);", "}"), &options);

    assert_eq!(actual, fixture!("void f()", "{", "    MACRO(*);", "}"));
}

#[test]
fn preserves_source_operator_spacing_without_padding_options() {
    let options = FormatOptions::default();
    let source = fixture!(
        "int  a  =  1;",
        "int b=2;",
        "x = y==z ? p : q;",
        "r = a?b:c;",
        "n = a*b + c/d - e%f;",
    );

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn pad_operators_preserves_word_alignment_spacing() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = fixture!("int    alpha = 1;", "double bb = 2;", "x   = 1;",);

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn preserves_open_paren_leading_spacing_without_options() {
    let options = FormatOptions::default();
    let source = fixture!(
        "int main()",
        "{",
        "    foo (a, b);",
        "    bar(c);",
        "    int  v  =  call  (1);",
        "    return  f ();",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn pad_parens_outside_preserves_existing_open_paren_spacing() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    let source = fixture!("int main()", "{", "    int  v  =  call  (1);", "}",);

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn pad_operators_keeps_operator_keyword_attached_to_overload_token() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(fixture!("T operator+(T a,T b){return a+b;}"), &options);

    assert_eq!(
        actual,
        fixture!("T operator+(T a, T b)", "{", "    return a + b;", "}",)
    );
}
#[test]
fn pad_operators_does_not_treat_relational_char_literals_as_template_arguments() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_with(fixture!("int f(){if(code<'z'&&code>'a')call();}"), &options),
        fixture!(
            "int f()",
            "{",
            "    if(code < 'z' && code > 'a')call();",
            "}"
        )
    );
}

#[test]
fn pad_operators_preserves_compound_operator_sequences() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(int x){",
            "switch(x){case A+B:call();}",
            "x=a<?>b;y=a?>b;z=a+++b;w=a-- -b;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch(x)",
            "    {",
            "    case A+B:",
            "        call();",
            "    }",
            "    x = a<?>b;",
            "    y = a ? >b;",
            "    z = a++ +b;",
            "    w = a-- -b;",
            "}",
        )
    );
}

#[test]
fn pad_operators_preserves_inline_assembly_expressions() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){", "asm(a+b);", "__asm mov eax+4;", "}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    asm(a+b);",
            "    __asm mov eax+4;",
            "}",
        )
    );
}

#[test]
fn pad_operators_preserves_objective_c_selectors_and_messages() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "SEL s=@selector(doThing:withValue:);",
            "[object doThing:value withValue:other];",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    SEL s = @selector(doThing:withValue:);",
            "    [object doThing:value withValue:other];",
            "}",
        )
    );
}
#[test]
fn pad_operators_preserves_source_spacing_in_case_labels() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(int x){switch(x){",
            "case 0x00 >> 2:break;",
            "case A+B:call();",
            "case A + B:other();",
            "}}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int x)",
            "{",
            "    switch(x)",
            "    {",
            "    case 0x00 >> 2:",
            "        break;",
            "    case A+B:",
            "        call();",
            "    case A + B:",
            "        other();",
            "    }",
            "}",
        )
    );
}
#[test]
fn pad_operators_pads_alternative_word_operators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(fixture!("int f(){return a and(b) or c;}"), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    return a and (b) or c;", "}",)
    );
}
#[test]
fn pad_operators_keeps_unary_operators_after_casts_and_in_subscripts_unpadded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "check_status(fd,SOL_SOCKET,SO_ERROR,(void *)&err,&len);",
            "alpha=(void *)&(value);",
            "beta=(void *)&((Item *)source)->field;",
            "count=(size_t)*cursor;",
            "data=(void *)*cursor;",
            "if(line[-1]=='\\r') line--;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    check_status(fd, SOL_SOCKET, SO_ERROR, (void *)&err, &len);",
            "    alpha = (void *) & (value);",
            "    beta = (void *) & ((Item *)source)->field;",
            "    count = (size_t) * cursor;",
            "    data = (void *)*cursor;",
            "    if (line[-1] == '\\r') line--;",
            "}",
        )
    );
}
#[test]
fn pad_operators_treats_multiply_after_cast_operand_as_arithmetic() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "a=(uint64_t)conns*MSGS;",
            "b=(uint64_t)workers*(uint64_t)conns*MSGS;",
            "c=ops_len*sizeof(struct op);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    a = (uint64_t)conns * MSGS;",
            "    b = (uint64_t)workers * (uint64_t)conns * MSGS;",
            "    c = ops_len * sizeof(struct op);",
            "}",
        )
    );
}
#[test]
fn pad_operators_treats_multiply_before_sizeof_on_continuation_as_arithmetic() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    size_t bytes = base +",
            "                   len * sizeof(struct op);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    size_t bytes = base +",
            "                   len * sizeof(struct op);",
            "}",
        )
    );
}
#[test]
fn pad_operators_keeps_address_of_after_plain_cast_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "a=(uintptr_t)&probe;",
            "b=(size_t)&arr[0];",
            "c=(uintptr_t)&(value);",
            "d=(size_t)*cursor;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    a = (uintptr_t)&probe;",
            "    b = (size_t)&arr[0];",
            "    c = (uintptr_t) & (value);",
            "    d = (size_t) * cursor;",
            "}",
        )
    );
}
#[test]
fn preserves_aligned_spaces_before_body_after_call_with_uppercase_arg() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.break_one_line_statements = false;
    let actual = format_with(
        fixture!(
            "void f(){",
            "if (a(A))          goto w_err;",
            "if (seek(fd, DATA_START, SET))          goto w_err;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    if (a(A))          goto w_err;",
            "    if (seek(fd, DATA_START, SET))          goto w_err;",
            "}",
        )
    );
}
#[test]
fn pad_operators_spaces_address_of_after_cast_in_initializer_braces() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.reference_align = ReferenceAlign::Name;
    let actual = format_with(
        fixture!(
            "struct Config s = {",
            ".alpha=(uintptr_t)&value,",
            ".beta=(uintptr_t)&other,",
            "};",
            "void f(){",
            "a=(uintptr_t)&value;",
            "g((uintptr_t)&value);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct Config s = {",
            "    .alpha = (uintptr_t) &value,",
            "    .beta = (uintptr_t) &other,",
            "};",
            "void f()",
            "{",
            "    a = (uintptr_t)&value;",
            "    g((uintptr_t)&value);",
            "}",
        )
    );
}
#[test]
fn pad_operators_keeps_unary_sign_after_open_brace_unpadded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "int a[3]={ -1, 2, -3 };",
            "struct s c={ .v=-5, .w=+6 };",
            "int b={ x-y };",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    int a[3] = { -1, 2, -3 };",
            "    struct s c = { .v = -5, .w = +6 };",
            "    int b = { x - y };",
            "}",
        )
    );
}
#[test]
fn pad_operators_pads_binary_operators_after_underscore_names() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    let actual = format_with(
        fixture!(
            "void f(){",
            "result = item_count*sizeof(item_t);",
            "total = sizeof(item_t)*item_count;",
            "if ((item_flags&(READ|WRITE)) != 0) call();",
            "buffer = allocate(pool, item_count*sizeof(item_t));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    result = item_count * sizeof(item_t);",
            "    total = sizeof(item_t) * item_count;",
            "    if ((item_flags & (READ | WRITE)) != 0) call();",
            "    buffer = allocate(pool, item_count * sizeof(item_t));",
            "}",
        )
    );
}
#[test]
fn pad_operators_preserves_template_angle_spacing() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!("void f(){vector<vector<int>> a; if(a>>b){c();} if(a<b>c){d();}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    vector<vector<int>> a;",
            "    if(a >> b)",
            "    {",
            "        c();",
            "    }",
            "    if(a<b>c)",
            "    {",
            "        d();",
            "    }",
            "}",
        )
    );
}
#[test]
fn preserves_template_angle_spacing_by_default() {
    let options = FormatOptions::default();
    for source in [
        "vector <int> a;\n",
        "vector< int > a;\n",
        "template<class T> void f();\n",
        "Map<Key, List<int> > m;\n",
        "A<B<C<int> > > x;\n",
    ] {
        assert_eq!(format_c(source, &options), source);
    }
}
#[test]
fn close_templates_collapses_adjacent_closing_angles() {
    let mut options = FormatOptions::default();
    options.close_templates = true;
    assert_eq!(
        format_c("Map<Key, List<int> > m;\n", &options),
        "Map<Key, List<int>> m;\n"
    );
    assert_eq!(
        format_c("A<B<C<int> > > x;\n", &options),
        "A<B<C<int>>> x;\n"
    );
}
#[test]
fn close_templates_leaves_separated_closing_angles_untouched() {
    let mut options = FormatOptions::default();
    options.close_templates = true;
    assert_eq!(format_c("int x = a >> b;\n", &options), "int x = a >> b;\n");
    assert_eq!(
        format_c("vector< int > a;\n", &options),
        "vector< int > a;\n"
    );
}
#[test]
fn unpad_parens_preserves_cast_exceptions_with_outside_padding() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    options.unpad_parens = true;
    options.pad_header = true;
    let actual = format_with(
        fixture!("void f(){call(); call(x); if(x)g(); p=(int*)q; n=(int)-1;}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    call();",
            "    call (x);",
            "    if (x) g();",
            "    p= (int*) q;",
            "    n= (int)-1;",
            "}",
        )
    );
}
#[test]
fn unpad_parens_keeps_header_like_space_exceptions() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_header = true;
    options.unpad_parens = true;
    let actual = format_c(
        fixture!(
            "int f(){return ( value ); throw ( error ); x = new ( place ) Item; y = delete ( value ); z = and ( value ); n = var1 ( value );}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f() {",
            "    return (value);",
            "    throw (error);",
            "    x = new (place) Item;",
            "    y = delete (value);",
            "    z = and (value);",
            "    n = var1(value);",
            "}",
        )
    );
}
#[test]
fn unpad_parens_removes_space_after_open_paren_before_semicolon() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    options.pad_header = true;
    let actual = format_c(
        fixture!(
            "void f(void)",
            "{",
            "    for ( ;; ) {",
            "        a();",
            "    }",
            "    for ( ; i < n; ) {",
            "        b();",
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
            "    for (;;) {",
            "        a();",
            "    }",
            "    for (; i < n;) {",
            "        b();",
            "    }",
            "}",
        )
    );
}
#[test]
fn unpad_parens_does_not_add_space_before_core_type_word_paren() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.unpad_parens = true;
    let actual = format_c(
        fixture!(
            "typedef bool handler_t(int v);",
            "int g(){x = foo_t (3); y = foo_t(3); z = int (a); w = bool(b);}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "typedef bool handler_t(int v);",
            "int g() {",
            "    x = foo_t(3);",
            "    y = foo_t(3);",
            "    z = int (a);",
            "    w = bool(b);",
            "}",
        )
    );
}

#[test]
fn unpad_parens_preserves_operator_overload_source_spacing() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let source = fixture!("PARSER& operator >> (PARSER& source, int& value);");

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_parens_spaces_function_pointer_declarator() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-paren".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(fixture!("typedef void(*handler)(int value);"), &options,),
        fixture!("typedef void ( *handler ) ( int value );"),
    );
}

#[test]
fn unpad_parens_preserves_source_gap_between_cast_and_unary_operand() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--style=kr".to_owned(),
            "--indent=tab".to_owned(),
            "--pad-comma".to_owned(),
            "--unpad-paren".to_owned(),
        ],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!(
                "void f() {",
                "\ta = call((char*) &value, n);",
                "\tb = (double) -1;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\ta = call((char*) &value, n);",
            "\tb = (double) -1;",
            "}",
        )
    );
}

#[test]
fn postfix_updates_preserve_source_gap() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void f() {", "\tcount ++;", "\trow --;", "}"),
            &options,
        ),
        fixture!("void f()", "{", "\tcount ++;", "\trow --;", "}")
    );
}

#[test]
fn pad_parens_inside_and_outside_space_nested_calls_and_headers() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    options.pad_parens_inside = true;
    let actual = format_with(fixture!("int f(){call(a,(b+c)); if(a)b();}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    call ( a, ( b+c ) );",
            "    if ( a ) b();",
            "}",
        )
    );
}

#[test]
fn pad_parens_inside_and_outside_keep_single_gap_before_trailing_comments() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    options.pad_parens_inside = true;

    assert_eq!(
        format_c(
            "\nvoid foo(bool isFoo)    // comment\n{\n    if (isFoo(a, b))    // comment\n",
            &options,
        ),
        "\nvoid foo ( bool isFoo ) // comment\n{\n    if ( isFoo ( a, b ) ) // comment\n",
    );
}

#[test]
fn pad_parens_inside_and_outside_leave_square_brackets_unpadded() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    options.pad_parens_inside = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if (buffer[6+font->GetSize1()] == 128)\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if ( buffer[6+font->GetSize1()] == 128 )\n",
    );
}

#[test]
fn pad_first_paren_outside_composes_with_unpad_parens() {
    let mut options = FormatOptions::default();
    options.pad_first_paren_outside = true;
    options.unpad_parens = true;
    let actual = format_with(fixture!("int f(){call ( a,( b ) ); other(x);}"), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    call (a, (b));", "    other (x);", "}",)
    );
}
#[test]
fn padding_options_change_only_requested_surfaces() {
    let source = fixture!("void f(){if(a+b)call(x,y?1:2);}");
    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!("void f() {", "    if(a+b)call(x,y?1:2);", "}",)
    );

    let mut operators = FormatOptions::default();
    operators.pad_operators = true;
    assert_eq!(
        format_c(source, &operators),
        fixture!("void f() {", "    if(a + b)call(x, y ? 1 : 2);", "}",)
    );

    let mut header = FormatOptions::default();
    header.pad_header = true;
    assert_eq!(
        format_c(source, &header),
        fixture!("void f() {", "    if (a+b)call(x,y?1:2);", "}",)
    );
}
#[test]
fn pad_header_does_not_match_longer_or_dotted_words() {
    let actual = format(fixture!("void f(){object.if(x);ifx(y);if(x){return 1;}}"));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    object.if(x);",
            "    ifx(y);",
            "    if (x)",
            "    {",
            "        return 1;",
            "    }",
            "}",
        )
    );
}
#[test]
fn pad_operators_treats_spaceship_as_one_operator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(fixture!("auto cmp=alpha<=>beta;"), &options);

    assert_eq!(actual, fixture!("auto cmp = alpha <=> beta;"));
}

// Template-angle context does not reclassify unparenthesized shifts.
#[test]
fn pad_operators_treats_both_shift_tokens_as_binary_operators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_with(fixture!("int f(){return a<<b>>c;}"), &options),
        fixture!("int f()", "{", "    return a << b >> c;", "}")
    );
}

// Binary operator gaps remain present beside unary pointer operators.
#[test]
fn pad_operators_keeps_binary_gaps_next_to_unary_pointer_operators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_with(fixture!("int f(){return a*&b+c+*&d;}"), &options),
        fixture!("int f()", "{", "    return a * &b + c + *&d;", "}")
    );
}

#[test]
fn pad_operators_keeps_sizeof_pointer_operand_unary() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_with(fixture!("int f(){return sizeof *p+sizeof &q;}"), &options),
        fixture!("int f()", "{", "    return sizeof *p + sizeof &q;", "}")
    );
}

#[test]
fn pad_operators_keeps_sizeof_attached_in_standalone_call_argument_continuations() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "copy(target,",
            "     (count - before)*sizeof(item_t *));",
            "buffer = allocate(pool,",
            "                  (count - before)*sizeof(item_t *));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    copy(target,",
            "         (count - before)*sizeof(item_t *));",
            "    buffer = allocate(pool,",
            "                      (count - before) * sizeof(item_t *));",
            "}",
        )
    );
}

#[test]
fn pad_operators_preserves_source_spacing_for_sizeof_multiply_in_standalone_call() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(){",
            "fill(target, 0,",
            "     (count - before) * sizeof(*ptr));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    fill(target, 0,",
            "         (count - before) * sizeof(*ptr));",
            "}",
        )
    );
}

#[test]
fn pad_operators_pads_expressions_without_detaching_unary_pointers() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "size_t size=(MIN_PAGES*page_size);",
            "uint8_t hi=(value>>8)&0xff;",
            "size_t len=colon-*pos;",
            "size_t total=(size/page_size)*page_size;",
            "if(!*ready||chunk>*remaining){call();}",
            "int read(u_char *pos, const u_char *end);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    size_t size = (MIN_PAGES * page_size);",
            "    uint8_t hi = (value >> 8) & 0xff;",
            "    size_t len = colon - *pos;",
            "    size_t total = (size / page_size) * page_size;",
            "    if (!*ready || chunk > *remaining) {",
            "        call();",
            "    }",
            "    int read(u_char *pos, const u_char *end);",
            "}",
        )
    );
}

#[test]
fn pad_operators_also_pads_commas() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c("int f(int a,int b,int c);\n", &options);
    assert_eq!(actual, "int f(int a, int b, int c);\n");
}

#[test]
fn pad_operators_spaces_relational_logical_and_arithmetic_operators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if (len>cap||cap>3*(len+8))\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if (len > cap || cap > 3 * (len + 8))\n",
    );
}

#[test]
fn pad_operators_spaces_float_comparisons_and_logical_operator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("\nvoid foo()\n{\n    if (age<3.0f&&age>0.0f)\n", &options,),
        "\nvoid foo()\n{\n    if (age < 3.0f && age > 0.0f)\n",
    );
}

#[test]
fn pad_operators_spaces_multiplication_before_uppercase_operand() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    if (pages*ROWS_PER_PAGE != values)\n",
            &options,
        ),
        "\nvoid foo()\n{\n    if (pages * ROWS_PER_PAGE != values)\n",
    );
}

#[test]
fn pad_operators_spaces_arithmetic_inside_array_initializer() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    Item next[] = {\n        Item (ceil (x+w*z))\n",
            &options,
        ),
        "\nvoid foo()\n{\n    Item next[] = {\n        Item (ceil (x + w * z))\n",
    );
}

#[test]
fn pad_operators_keeps_postfix_decrement_attached_in_while_header() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "void foo()\n{\n    while (length--)\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_commas_spaces_rectangular_array_initializer_elements() {
    let mut options = FormatOptions::default();
    options.pad_commas = true;

    assert_eq!(
        format_c(
            "\nstatic bool[,] set = {\n    {T,T,x,T, x,T,x,T, T,T,x,T, T,T,T,x},\n    {x,x,x,x, x,x,x,x, x,x,x,T, T,T,T,x},\n    {T,T,x,T, x,T,x,T, T,T,x,T, T,T,T,x},\n    {x,T,T,T, T,T,T,T, T,T,T,T, T,T,T,T}\n",
            &options,
        ),
        "\nstatic bool[,] set = {\n    {T, T, x, T, x, T, x, T, T, T, x, T, T, T, T, x},\n    {x, x, x, x, x, x, x, x, x, x, x, T, T, T, T, x},\n    {T, T, x, T, x, T, x, T, T, T, x, T, T, T, T, x},\n    {x, T, T, T, T, T, T, T, T, T, T, T, T, T, T, T}\n",
    );
}

#[test]
fn pad_operators_pads_commas_in_call_and_array() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c("int x[]={1,2,3};\nf(a,b);\n", &options);
    assert_eq!(actual, "int x[] = {1, 2, 3};\nf(a, b);\n");
}

#[test]
fn pad_operators_preserves_existing_multi_space_after_comma() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c("f(a,  b);\n", &options);
    assert_eq!(actual, "f(a,  b);\n");
}

#[test]
fn no_option_preserves_return_unary_adjacency() {
    let options = FormatOptions::default();
    for src in [
        "return*p;\n",
        "return&a;\n",
        "return!a;\n",
        "return~a;\n",
        "return-a;\n",
        "return+a;\n",
    ] {
        assert_eq!(format_c(src, &options), src.to_string(), "{src}");
    }
}

#[test]
fn no_option_preserves_return_unary_multi_space() {
    let options = FormatOptions::default();
    assert_eq!(format_c("return  *p;\n", &options), "return  *p;\n");
}

#[test]
fn pad_operators_pads_only_sign_after_return() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(format_c("return-a;\n", &options), "return -a;\n");
    assert_eq!(format_c("return+a;\n", &options), "return +a;\n");
    // Pointer, address-of, logical-not and bit-not stay attached to the operand.
    assert_eq!(format_c("return*p;\n", &options), "return*p;\n");
    assert_eq!(format_c("return&a;\n", &options), "return&a;\n");
    assert_eq!(format_c("return!a;\n", &options), "return!a;\n");
    assert_eq!(format_c("return~a;\n", &options), "return~a;\n");
}

#[test]
fn pad_operators_keeps_case_label_unary_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c("switch(x){case-1:f();}\n", &options);
    assert!(actual.contains("case-1"), "{actual}");
    assert!(!actual.contains("case -1"), "{actual}");
}

#[test]
fn pad_parens_outside_pads_every_nested_open_paren() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(
        format_c("x=foo(bar(a),baz(b));\n", &options),
        "x=foo (bar (a),baz (b) );\n"
    );
}

#[test]
fn pad_parens_outside_keeps_single_gap_before_trailing_comments() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;

    assert_eq!(
        format_c(
            "\nvoid foo(bool isFoo)  // comment\n{\n    if (isFoo(a, b))  // comment\n",
            &options,
        ),
        "\nvoid foo (bool isFoo) // comment\n{\n    if (isFoo (a, b) ) // comment\n",
    );
}

#[test]
fn pad_parens_outside_spaces_after_close_before_operators() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(format_c("y=(a)*c;\n", &options), "y= (a) *c;\n");
    assert_eq!(format_c("y=(a)/c;\n", &options), "y= (a) /c;\n");
    assert_eq!(format_c("y=(a)=c;\n", &options), "y= (a) =c;\n");
    assert_eq!(format_c("y=(a)|c;\n", &options), "y= (a) |c;\n");
    assert_eq!(format_c("y=(a)&&c;\n", &options), "y= (a) &&c;\n");
    assert_eq!(format_c("y=(a)(c);\n", &options), "y= (a) (c);\n");
}

#[test]
fn pad_parens_outside_updates_assignment_comment_continuation() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;

    assert_eq!(
        format_c("x=(a)// note\n+b;\n", &options),
        "x= (a) // note\n   +b;\n"
    );
}

#[test]
fn pad_parens_outside_keeps_close_attached_for_excluded_chars() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(format_c("y=(a);\n", &options), "y= (a);\n");
    assert_eq!(format_c("y=(a).c;\n", &options), "y= (a).c;\n");
    assert_eq!(format_c("y=(a)+c;\n", &options), "y= (a)+c;\n");
    assert_eq!(format_c("y=(a)-c;\n", &options), "y= (a)-c;\n");
    assert_eq!(format_c("y=(a)&c;\n", &options), "y= (a)&c;\n");
    assert_eq!(format_c("y=(a)^c;\n", &options), "y= (a)^c;\n");
}

#[test]
fn pad_parens_outside_pads_cast_operand_inside_parens_only() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(
        format_c("f((void *)&err);\n", &options),
        "f ( (void *) &err);\n"
    );
    assert_eq!(
        format_c("x=(void *)&err;\n", &options),
        "x= (void *)&err;\n"
    );
}

#[test]
fn pad_parens_outside_skips_empty_parens() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(format_c("if(){}\n", &options), "if() {}\n");
    assert_eq!(format_c("foo();\n", &options), "foo();\n");
}

#[test]
fn pad_first_paren_outside_pads_every_nested_open_paren() {
    let mut options = FormatOptions::default();
    options.pad_first_paren_outside = true;
    assert_eq!(
        format_c("x=foo(bar(a),baz(b));\n", &options),
        "x=foo (bar (a),baz (b));\n"
    );
    assert_eq!(format_c("f(a)(b);\n", &options), "f (a) (b);\n");
    assert_eq!(format_c("x=(a)+b;\n", &options), "x= (a)+b;\n");
}

#[test]
fn pad_first_paren_outside_spaces_calls_inside_nested_groups() {
    let mut options = FormatOptions::default();
    options.pad_first_paren_outside = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    mode = ((inkind[j] == NONEmode\n             || (ITEM_GET_SIZE(outkind[j]) > ITEM_GET_SIZE(inkind[j])))\n",
            &options,
        ),
        "\nvoid foo()\n{\n    mode = ((inkind[j] == NONEmode\n             || (ITEM_GET_SIZE (outkind[j]) > ITEM_GET_SIZE (inkind[j])))\n",
    );
}

#[test]
fn pad_first_paren_outside_keeps_single_gap_before_trailing_comments() {
    let mut options = FormatOptions::default();
    options.pad_first_paren_outside = true;

    assert_eq!(
        format_c(
            "\nvoid foo(bool isFoo)  // comment\n{\n    if(isFoo(a, b))     // comment\n",
            &options,
        ),
        "\nvoid foo (bool isFoo) // comment\n{\n    if (isFoo (a, b))   // comment\n",
    );
}

#[test]
fn pad_first_paren_outside_does_not_space_after_close() {
    let mut options = FormatOptions::default();
    options.pad_first_paren_outside = true;
    assert_eq!(format_c("y=(a)*c;\n", &options), "y= (a)*c;\n");
}

#[test]
fn pad_header_pads_empty_header_parens() {
    let mut options = FormatOptions::default();
    options.pad_header = true;
    assert_eq!(format_c("if(){}\n", &options), "if () {}\n");
}

#[test]
fn pad_parens_inside_preserves_wide_source_gaps() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;

    assert_eq!(format_c("call (  a  );\n", &options), "call (  a  );\n");

    options.unpad_parens = true;
    assert_eq!(format_c("call (  a  );\n", &options), "call( a );\n");
}

#[test]
fn default_preserves_tab_gap_before_trailing_comment() {
    assert_eq!(
        format_c("\nint value;\t\t// note\n", &FormatOptions::default()),
        "\nint value;\t\t// note\n",
    );
}

#[test]
fn pad_operators_preserves_tab_gap_before_trailing_comment() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("\nint value=1;\t// note\n", &options),
        "\nint value = 1;\t// note\n",
    );
}

#[test]
fn pad_parens_inside_keeps_single_gap_before_trailing_comments() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;

    assert_eq!(
        format_c(
            "\nvoid foo(bool isFoo)   // comment\n{\n    if (isFoo(a, b))  // comment\n",
            &options,
        ),
        "\nvoid foo( bool isFoo ) // comment\n{\n    if ( isFoo( a, b ) ) // comment\n",
    );
}

#[test]
fn pad_parens_inside_spaces_nested_delimiters() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;

    assert_eq!(
        format_c("x=((int)y); call({a,b}); outer((),(a));\n", &options),
        fixture!("x=( ( int )y );", "call( {a,b} );", "outer( (),( a ) );")
    );
}

#[test]
fn pad_parens_inside_does_not_insert_gap_before_function_pointer_group() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;
    options.pointer_align = PointerAlign::Name;

    assert_eq!(
        format_c(fixture!("", "struct Entry* (*)( unsigned int );"), &options,),
        fixture!("", "struct Entry *( * )( unsigned int );")
    );
}

#[test]
fn pad_parens_inside_spaces_global_scope_operand() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;

    assert_eq!(format_c("call(::value);\n", &options), "call( ::value );\n");

    options.pad_parens_inside = false;
    options.pad_commas = true;
    assert_eq!(
        format_c("call(arg,::value);\n", &options),
        "call(arg, ::value);\n"
    );
}

#[test]
fn pad_parens_inside_wins_over_unpad_for_for_semicolons() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;
    options.unpad_parens = true;

    assert_eq!(format_c("for(;;)f();\n", &options), "for( ;; )f();\n");
}

#[test]
fn unpad_parens_removes_opening_gap_before_block_comment_and_global_scope() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;

    assert_eq!(
        format_c("call(  /* note */ value);\n", &options),
        "call(/* note */ value);\n"
    );
    assert_eq!(format_c("call(  ::value);\n", &options), "call(::value);\n");
}

#[test]
fn pad_parens_outside_preserves_source_gap_before_line_ending_comment() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;

    assert_eq!(
        format_c("call(  // note\n value);\n", &options),
        "call (  // note\n    value);\n"
    );
    assert_eq!(
        format_c("call(  /* note */\n value);\n", &options),
        "call (  /* note */\n    value);\n"
    );
}

#[test]
fn unpad_parens_removes_gap_before_line_ending_comment() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;

    assert_eq!(
        format_c("call(  // note\n value);\n", &options),
        "call(// note\n    value);\n"
    );
    assert_eq!(
        format_c("call(  /* note */\n value);\n", &options),
        "call(/* note */\n    value);\n"
    );

    options.pad_parens_outside = true;
    assert_eq!(
        format_c("call(  // note\n value);\n", &options),
        "call ( // note\n    value);\n"
    );
}

#[test]
fn unpad_parens_repositions_trailing_comments_after_removed_spaces() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;

    assert_eq!(
        format_c(
            "\nvoid foo ( bool isFoo ) // comment\n{\n    if ( isFoo ( a, b ) ) // comment\n",
            &options,
        ),
        "\nvoid foo(bool isFoo)    // comment\n{\n    if(isFoo(a, b))       // comment\n",
    );
}

#[test]
fn unpad_parens_suppresses_outside_gap_between_closing_parens() {
    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    options.unpad_parens = true;

    assert_eq!(
        format_c("using X=decltype(f(a));\n", &options),
        "using X=decltype (f (a));\n"
    );
}

#[test]
fn pad_header_spaces_expression_keywords() {
    let mut options = FormatOptions::default();
    options.pad_header = true;

    assert_eq!(
        format_c("return(a); throw(a); new(a)Item; delete(a);\n", &options),
        fixture!("return (a);", "throw (a);", "new (a)Item;", "delete (a);")
    );
}

#[test]
fn pad_header_spaces_switch_and_case_keywords() {
    let mut options = FormatOptions::default();
    options.pad_header = true;

    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n    switch(x) {\n    case(a + b) * c:\n        //...\n    }\n}\n",
            &options,
        ),
        "\nvoid foo()\n{\n    switch (x) {\n    case (a + b) * c:\n        //...\n    }\n}\n",
    );
}

#[test]
fn pad_header_leaves_exception_specification_attached() {
    let mut options = FormatOptions::default();
    options.pad_header = true;
    let source = "\nvoid with_type() throw(int) {\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn paren_padding_preserves_block_continuation_indent_after_line_comment() {
    let source = "long_name(// note\n a);\n";
    assert_eq!(
        format_c(source, &FormatOptions::default()),
        "long_name(// note\n    a);\n"
    );

    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;
    assert_eq!(format_c(source, &options), "long_name( // note\n    a );\n");

    options.pad_parens_inside = false;
    options.pad_parens_outside = true;
    assert_eq!(format_c(source, &options), "long_name ( // note\n    a);\n");
}

#[test]
fn paren_padding_preserves_block_continuation_indent_after_block_comment() {
    let source = "long_name(/* note */\n a);\n";
    assert_eq!(
        format_c(source, &FormatOptions::default()),
        "long_name(/* note */\n    a);\n"
    );
    assert_eq!(
        format_c(
            "long_name(/* first\n * second */\n a);\n",
            &FormatOptions::default(),
        ),
        "long_name(/* first\n * second */\n    a);\n"
    );

    let mut options = FormatOptions::default();
    options.pad_parens_outside = true;
    assert_eq!(
        format_c(source, &options),
        "long_name ( /* note */\n    a);\n"
    );
    assert_eq!(
        format_c("long_name(/* first\n * second */\n a);\n", &options),
        "long_name (/* first\n * second */\n    a);\n"
    );

    options.pad_parens_inside = true;
    assert_eq!(
        format_c(source, &options),
        "long_name ( /* note */\n    a );\n"
    );
}

#[test]
fn pad_parens_inside_aligns_comment_to_first_argument() {
    let mut options = FormatOptions::default();
    options.pad_parens_inside = true;

    assert_eq!(
        format_c("long_name(a\n /* note */);\n", &options),
        "long_name( a\n           /* note */ );\n"
    );
    assert_eq!(
        format_c("outer(inner(/* note */\n a));\n", &options),
        "outer( inner( /* note */\n           a ) );\n"
    );
}

#[test]
fn pad_operators_preserves_wider_source_spacing_around_operators() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c("x=alpha  =  beta;\n", &options),
        "x = alpha  =  beta;\n"
    );
    assert_eq!(
        format_c("x=alpha  +  beta;\n", &options),
        "x = alpha  +  beta;\n"
    );
    assert_eq!(
        format_c("x=alpha\t=\tbeta;\n", &options),
        "x = alpha\t=\tbeta;\n"
    );
    assert_eq!(format_c("value =  3;\n", &options), "value =  3;\n");
    assert_eq!(
        format_c("alpha+beta  = 1;\n", &options),
        "alpha + beta  = 1;\n"
    );
}

#[test]
fn preserves_source_spacing_inside_parens() {
    let options = FormatOptions::default();
    assert_eq!(
        format_c("call(  alpha  );\n", &options),
        "call(  alpha  );\n"
    );
    assert_eq!(
        format_c("call(\talpha\t);\n", &options),
        "call(\talpha\t);\n"
    );
}

#[test]
fn preserves_source_spacing_around_brackets() {
    let options = FormatOptions::default();
    assert_eq!(format_c("x=arr [i];\n", &options), "x=arr [i];\n");
    assert_eq!(format_c("x=arr[ i ];\n", &options), "x=arr[ i ];\n");
    assert_eq!(format_c("x=arr[  i  ];\n", &options), "x=arr[  i  ];\n");
    assert_eq!(format_c("x=arr [ i ];\n", &options), "x=arr [ i ];\n");
    assert_eq!(format_c("x=arr[\ti\t];\n", &options), "x=arr[\ti\t];\n");
    assert_eq!(format_c("x=arr[i] [j];\n", &options), "x=arr[i] [j];\n");
    assert_eq!(format_c("x=arr[i];\n", &options), "x=arr[i];\n");
}

#[test]
fn preserves_source_space_before_semicolon() {
    let options = FormatOptions::default();
    assert_eq!(format_c("int x ;\n", &options), "int x ;\n");
    assert_eq!(format_c("int x  ;\n", &options), "int x  ;\n");
    assert_eq!(format_c("int x\t;\n", &options), "int x\t;\n");
}

#[test]
fn for_loop_preserves_semicolon_spacing_and_pads_before_statement() {
    let options = FormatOptions::default();
    assert_eq!(
        format_c("for(i=0;i<n;i++) {}\n", &options),
        "for(i=0; i<n; i++) {}\n"
    );
    assert_eq!(
        format_c("for(i=0 ; i<n ; i++) {}\n", &options),
        "for(i=0 ; i<n ; i++) {}\n"
    );
    assert_eq!(
        format_c("for(i=0;  i<n;i++) {}\n", &options),
        "for(i=0;  i<n; i++) {}\n"
    );
    assert_eq!(format_c("for( ; ; ) {}\n", &options), "for( ; ; ) {}\n");
}

#[test]
fn preserves_close_paren_word_adjacency() {
    let options = FormatOptions::default();
    assert_eq!(
        format_c("value=(alpha)beta;\n", &options),
        "value=(alpha)beta;\n"
    );
    assert_eq!(
        format_c("result=call(a)(b);\n", &options),
        "result=call(a)(b);\n"
    );
}

#[test]
fn pad_operators_pads_word_operator_after_close_paren() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c("if((alpha)and(beta)){}\n", &options),
        "if((alpha) and (beta)) {}\n"
    );
}

#[test]
fn preserves_word_operator_adjacency_without_pad_operators() {
    let options = FormatOptions::default();
    assert_eq!(
        format_c("if((alpha)and(beta)){}\n", &options),
        "if((alpha)and(beta)) {}\n"
    );
}

#[test]
fn pad_operators_preserves_multi_space_before_word_operator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    assert_eq!(
        format_c("if(alpha  and beta){}\n", &options),
        "if(alpha  and beta) {}\n"
    );
    assert_eq!(
        format_c("call(  alpha+beta  );\n", &options),
        "call(  alpha + beta  );\n"
    );
}

#[test]
fn pad_operators_preserves_ternary_alignment_spaces() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let input = concat!(
        "int e = ((m & READ)   ? IN  : 0) |\n",
        "        ((m & WRITE)  ? OUT : 0) |\n",
        "        ((m & POLLET) ? ET  : 0);\n",
    );
    assert_eq!(format_c(input, &options), input);
    assert_eq!(
        format_c("int e = a?b:c;\n", &options),
        "int e = a ? b : c;\n"
    );
}

#[test]
fn pad_operators_pads_ternary_colon_on_continuation_line() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman".to_owned(), "--pad-oper".to_owned()];
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(fixture!("int f(int x){return x?", "1:", "0;}"), &options,),
        fixture!(
            "int f(int x)",
            "{",
            "    return x ?",
            "           1 :",
            "           0;",
            "}",
        )
    );
}

#[test]
fn pad_operators_preserves_existing_assignment_alignment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "enum {",
            "    A              = 1,",
            "    LONG_NAME      = 2,",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "enum {",
            "    A              = 1,",
            "    LONG_NAME      = 2,",
            "};"
        )
    );
}

#[test]
fn preserves_source_space_after_unary_prefix_operators() {
    let actual = format_c(
        fixture!(
            "int g(int *alpha,int *beta){",
            "int value = alpha > * beta;",
            "int gamma = ! value;",
            "int delta = & value;",
            "int epsilon = * beta;",
            "return value;",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "int g(int *alpha,int *beta) {",
            "    int value = alpha > * beta;",
            "    int gamma = ! value;",
            "    int delta = & value;",
            "    int epsilon = * beta;",
            "    return value;",
            "}",
        )
    );
}

#[test]
fn default_spacing_distinguishes_exponent_ternary_and_member_operators() {
    let actual = format(fixture!(
        "double f(int a){return 1e-5+0.2E+3; x=a?-b:+c; y=a->b*c;}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "double f(int a)",
            "{",
            "    return 1e-5 + 0.2E+3;",
            "    x = a ? -b : +c;",
            "    y = a->b * c;",
            "}",
        )
    );
}

#[test]
fn pads_non_empty_for_header_semicolons_by_default() {
    let options = FormatOptions::default();
    let actual = format_c(fixture!("void f(){for(i=0;i<n;i++){} for(;;){}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f() {",
            "    for(i=0; i<n; i++) {}",
            "    for(;;) {}",
            "}",
        )
    );
}

#[test]
fn source_space_before_member_access_is_preserved() {
    let source = fixture!(
        "void f()",
        "{",
        "    object .call();",
        "    text) .arg(value);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn member_access_preserves_source_space_after_dot() {
    let source = fixture!(
        "void f()",
        "{",
        "    call(OptionValues(0).first(A). second(B));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn binary_xor_after_postincrement_preserves_source_space() {
    let actual = format_c(
        fixture!("void f()", "{", "\t*output++ = *input++ ^ mask;", "}"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!("void f()", "{", "    *output++ = *input++ ^ mask;", "}")
    );
}

#[test]
fn pad_operators_spaces_range_for_colon() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            fixture!("void run()", "{", "    for (auto value:values)"),
            &options,
        ),
        fixture!("void run()", "{", "    for (auto value : values)")
    );
}

#[test]
fn pad_operators_pads_for_header_operators_and_semicolons() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(fixture!("void f(){for(i=0;i<n;i++){call(i);}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for(i = 0; i < n; i++)",
            "    {",
            "        call(i);",
            "    }",
            "}",
        )
    );
}

#[test]
fn pad_commas_pads_call_arguments_and_keeps_for_header_semicolon_gaps() {
    let mut options = FormatOptions::default();
    options.pad_commas = true;
    let actual = format_with(fixture!("void f(){for(i=0;i<n;i++){call(i,j);}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    for(i=0; i<n; i++)",
            "    {",
            "        call(i, j);",
            "    }",
            "}",
        )
    );
}

// A prefix `^` after an assignment is unary (Apple block / C++26 reflection `^^`
// operand), not a binary xor, so --pad-oper must not pad it. Binary `^` is still
// padded.
#[test]
fn prefix_caret_after_assignment_is_not_padded() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(format_c("auto r=^^int;\n", &options), "auto r = ^^int;\n");
    assert_eq!(format_c("auto r=^x;\n", &options), "auto r = ^x;\n");
    assert_eq!(format_c("int b=a^c;\n", &options), "int b = a ^ c;\n");
}

#[test]
fn spaceship_operator_is_padded_as_one_operator() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(format_c("auto x=a<=>b;\n", &options), "auto x = a <=> b;\n",);
    assert_eq!(
        format_c("bool f(int a,int b){return a<=>b<0;}\n", &options),
        "bool f(int a, int b) {\n    return a <=> b < 0;\n}\n",
    );
}

#[test]
fn pad_oper_keeps_unary_minus_after_cast_unpadded() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (value == (type_t) -1) {\n\t\tcall();\n\t}\n}\n",
            &kr_c_options(),
        ),
        "void f(void)\n{\n    if (value == (type_t) -1) {\n        call();\n    }\n}\n",
    );
}

#[test]
fn space_after_cast_before_unary_bitnot_is_preserved() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            "comp_t f(int exp)\n{\n\tif (exp > (((comp_t) ~0U) >> MANTSIZE))\n\t\treturn (comp_t) ~0U;\n}\n",
            &options,
        ),
        "comp_t f(int exp)\n{\n    if (exp > (((comp_t) ~0U) >> MANTSIZE))\n        return (comp_t) ~0U;\n}\n",
    );
}

#[test]
fn pad_oper_keeps_address_of_member_argument_attached() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
    ]);
    let source = "\nstatic inline bool pending(Item *item)\n{\n    return load_explicit(&item->state.value, order_acquire);\n}\n";

    assert_eq!(format_c(source, &options), source);
}
#[test]
fn pad_oper_keeps_cast_pointer_deref_attached() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--unpad-paren",
    ]);

    assert_eq!(
        format_c(
            "\nvoid foo(const char *str)\n{\n    if (!isdigit((unsigned char)*str)) {\n        return;\n    }\n}\n",
            &options,
        ),
        "\nvoid foo(const char *str)\n{\n    if(!isdigit((unsigned char)*str)) {\n        return;\n    }\n}\n"
    );
}

#[test]
fn pad_oper_keeps_inline_asm_triple_colon_attached() {
    let options = options_from_args(&["--style=1tbs", "--mode=c", "--pad-oper", "--unpad-paren"]);
    let source = "\nvoid spin(void)\n{\n    __asm__ __volatile__(\"nop\" ::: \"memory\");\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_multiply_after_sizeof_pointer_padded() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-comma",
        "--align-pointer=name",
        "--unpad-paren",
    ]);
    let source =
        "\nvoid f(char **argv, int count)\n{\n    copy(argv, sizeof(char *) * count);\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_binary_and_between_mask_operands_padded() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-reference=name",
        "--unpad-paren",
    ]);
    let source = "\nint f(int flag, int mask)\n{\n    return (flag & mask) == 0;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_binary_and_after_underscore_name_in_ternary_padded() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--align-pointer=name",
        "--align-reference=name",
    ]);
    let source = "long helper(void)\n{\n    long event = (event_flags & VALUE)\n                 ? CLEAR_EVENT : LEVEL_EVENT;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_binary_multiply_after_type_like_names_padded() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--unpad-paren",
    ]);
    let source = "\nvoid *f(void *arena, void *ptr, size_t old_nmemb, size_t size, size_t new_nmemb)\n{\n    return arena\n           ? arena_chunk_realloc(arena, ptr, old_nmemb * size, new_nmemb * size)\n           : alloc(ptr, new_nmemb, size);\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_binary_ops_after_return_and_nested_call_padded() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-header",
        "--align-pointer=name",
        "--align-reference=name",
        "--unpad-paren",
    ]);
    let source = "\nint f(int flag, int item_size)\n{\n    if (flag) {\n        return flag & VALUE_MASK;\n    }\n    call(sizeof(meta) +\n         (ITEM_COUNT * item_size));\n    return 0;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_oper_keeps_cast_deref_after_paren_attached() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-header",
        "--align-pointer=name",
        "--unpad-paren",
    ]);
    let source = "\nint f(const char *cur)\n{\n    if ((unsigned char)*cur >= 32) {\n        return *cur;\n    }\n    return 0;\n}\n";

    assert_eq!(format_c(source, &options), source);
}

// The arrow keeps its source whitespace because no option governs that gap.
#[test]
fn preserves_arrow_operator_source_spacing_without_pad_operators() {
    let options = options_from_args(&[
        "--style=kr",
        "--indent=tab",
        "--break-one-line-headers",
        "--add-braces",
        "--attach-return-type",
        "--attach-return-type-decl",
        "--pad-comma",
        "--unpad-paren",
    ]);
    let source = "void f()\n{\n\ta -> b;\n\tp->q;\n\tauto g = [](int n)->bool { return n; };\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn default_preserves_whitespace_before_comma() {
    // No comma option is active, so the complete source gap is preserved.
    let source = "call(alpha \t,beta);\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn pad_operators_preserves_template_parameter_inner_spacing() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "\ntemplate < typename T && !is_integral >\n{}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn pad_operators_keeps_adjacent_template_angles_attached() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let source = "\ntemplate <typename T,\n          typename I = Type<\n              Other<T>::value>>\nauto call(T&& value);\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn cast_before_numeric_literal_keeps_source_spacing() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int f(void)\n{\n\tint z = (u64)1U;\n\tint w = (u32)63U;\n}\n",
            &options,
        ),
        "int f(void)\n{\n    int z = (u64)1U;\n    int w = (u32)63U;\n}\n",
    );
}

#[test]
fn cast_preserves_source_double_space_before_identifier() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tx = (unsigned long *)  bits;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    x = (unsigned long *)  bits;\n}\n",
    );
}

#[test]
fn pointer_cast_preserves_source_space_before_address_of() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\ta = (Config *) &item->field;\n\tb = (Config *)&item->field;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    a = (Config *) &item->field;\n    b = (Config *)&item->field;\n}\n",
    );
}

#[test]
fn pad_parens_inside_pads_empty_for_header() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-paren".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f()\n{\n    for(;;) {\n        s();\n    }\n}\n",
            &options,
        ),
        "void f()\n{\n    for ( ;; ) {\n        s();\n    }\n}\n",
    );
}

#[test]
fn pad_oper_pads_variadic_ellipsis_after_comma() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("void log(int level,...);\n", &options),
        "void log(int level, ...);\n",
    );
}

#[test]
fn pad_parens_inside_pads_ellipsis_after_open_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-paren".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("void f()\n{\n    g(...);\n}\n", &options),
        "void f()\n{\n    g ( ... );\n}\n",
    );
}

#[test]
fn pad_oper_pads_constructor_initializer_colon() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c(
            "class Foo\n{\npublic:\n    Foo(int a):x(a) {}\n};\n",
            &options,
        ),
        "class Foo\n{\npublic:\n    Foo(int a): x(a) {}\n};\n",
    );
}

#[test]
fn pad_paren_inside_pads_open_paren_before_bracket() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-paren".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("int v = ([1,2]);\n", &options),
        "int v = ( [1,2] );\n",
    );
}

#[test]
fn pad_paren_inside_only_pads_open_paren_before_bracket() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--pad-paren-in".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("int v = ([1,2]);\n", &options),
        "int v = ( [1,2] );\n",
    );
}

#[test]
fn pad_oper_pads_ternary_colon_before_open_paren() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("int z=a?(c):(d);\n", &options),
        "int z = a ? (c) : (d);\n",
    );
}

// Enum underlying-type colons pad on both sides, unlike class-base colons.
#[test]
fn pad_oper_pads_enum_underlying_type_colon_both_sides() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("enum class Color:int { Red };\n", &options),
        "enum class Color : int { Red };\n",
    );
}

#[test]
fn pad_oper_pads_unscoped_enum_underlying_type_colon_both_sides() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(
        format_c("enum Color:int { Red };\n", &options),
        "enum Color : int { Red };\n",
    );
}

#[test]
fn pad_oper_malformed_switch_body_after_run_in_catch_is_idempotent() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let input = "~// line||;  !=\n]Item// line// line{forreturngamma =x<=try&&{ constexprcontinue case(->catchgammaautoreturn)&&class forItem>= 42\nenum\nItem,#if A  beta\n0%\ncallcase #endifnamespacecontinue ==#else;#elsey||\n0helpernamespace:for,\n{catch\nforelse\nswitch\ny\n{\n#else#endifelse||namespacegammado structforelse==case#define X(x) \\\t\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pad_oper_malformed_bracket_after_switch_word_is_idempotent() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let input = "if\nbreakcontinue  catch#endif>=catchreturn1class,+else\n-\n&&ifyenum\n]\n>=+,}<=switchclassConfig*\n]callx[\n% ! 142:defaultnamespacecase  while}\tswitch\nItem<=  !\n]->\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn unpad_paren_malformed_orphan_else_branch_body_is_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let input = "helpercaseItem  {::1  ;#else\nfor%\nfor||default#elseyenum// line\ncatch ->beta  }\nhelpertry{default=\n%call\tforenum!= constexprItem::elseconstexpr==\n// line\t1\nifz switch>=  ?\n]\tConfig)callConfig\nnamespacecontinue(>=switch=#endifNULLcase>=catchgamma\ncall\n!=else\t}*/* block */Configdefault=caseelsedodoalphatry <=\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pad_oper_malformed_colon_before_operator_line_is_idempotent() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let input = "case/* block */alphanamespace42  /* block */result<=>%beta||#else->/else->NULL&\t1if);<=->gamma,throw#elsecall:~&&|/#endifNULL&&==\n\n#endifNULL\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pad_oper_malformed_embedded_define_line_comment_body_indent_is_idempotent() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let input = "\n->{||/* block */while\t+#define X(x) \\// line!  ::\n1*try>=\n,default\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn unpad_paren_malformed_preprocessor_word_after_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let input = "switch+!=while\ncallreturn  case[1\n#if A\t;callalpha%struct!=\n)while  casecally||auto\n,\t- ;gamma>=>=#else::NULLhelper\nz\n{;#elseswitch||==structbreak}break&&\tcatch>===!=struct\t:\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn malformed_questions_before_unmatched_close_are_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let first = format_bytes(b"??}\ndefault=:f", &options).expect("format bytes");

    assert_eq!(format_bytes(&first, &options).expect("format bytes"), first,);
}

#[test]
fn malformed_assignment_colon_continuation_is_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let first = format_bytes(b"{\ndefault =:o", &options).expect("format bytes");

    assert_eq!(format_bytes(&first, &options).expect("format bytes"), first,);
}

#[test]
fn malformed_bare_question_before_brace_is_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let first = format_bytes(b"?\n{:f", &options).expect("format bytes");

    assert_eq!(format_bytes(&first, &options).expect("format bytes"), first,);
}

#[test]
fn unpad_paren_malformed_label_after_define_is_idempotent() {
    let mut options = FormatOptions::default();
    options.unpad_parens = true;
    let input = "&&betacontinue1,||#else#else\nreturn#if A&&\n)? z-}-Config#define X(x) \\ 42\nconstexprbeta\n0Item\nswitch( xz::||beta\t-?y namespace)}\ntry~\n}ycontinue\ncall\ngamma-\n==1structNULL::Item\ny\n{\nfor\nbreak* ;\ndefault#define X(x) \\/* block */,z<=>=call:for\n";
    let first = format_c(input, &options);

    assert_eq!(format_c(&first, &options), first);
}

#[test]
fn pad_oper_keeps_dollar_sign_extension_identifier_intact() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;

    assert_eq!(format_c("int $value=1;\n", &options), "int $value = 1;\n",);
}

#[test]
fn enum_underlying_type_colon_preserved_without_pad_oper() {
    let source = "enum class Color:int { Red };\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn preserves_tab_space_between_adjacent_operators_idempotent() {
    let source = "x+({+\t |0\n";

    let first = format_c(source, &FormatOptions::default());
    assert_eq!(first, source);
    assert_eq!(format_c(&first, &FormatOptions::default()), source);
}

#[test]
fn pad_paren_out_does_not_pad_empty_function_calls() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=1tbs".to_owned(), "--pad-paren-out".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void run(){if(alpha) first();}\n", &options,),
        "void run()\n{\n    if (alpha) {\n        first();\n    }\n}\n",
    );
}
