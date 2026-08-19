#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{
    BraceStyle, FormatOptions, IndentStyle, MinConditionalIndent, PointerAlign,
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
fn indents_class_blocks_modifiers_and_initializers() {
    let source = fixture!(
        "class C{public:int x;protected:void f();private:int y;};",
        "struct S{private:int z;};",
        "class Item{public:Item(): value{1}, other(2), third{3}{call();}};",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "class C {",
            "public:",
            "    int x;",
            "protected:",
            "    void f();",
            "private:",
            "    int y;",
            "};",
            "struct S {",
            "private:",
            "    int z;",
            "};",
            "class Item {",
            "public:",
            "    Item(): value{1}, other(2), third{3} {",
            "        call();",
            "    }",
            "};",
        )
    );

    let mut indent_classes = FormatOptions::default();
    indent_classes.indent_classes = true;
    assert_eq!(
        format_c(source, &indent_classes),
        fixture!(
            "class C {",
            "    public:",
            "        int x;",
            "    protected:",
            "        void f();",
            "    private:",
            "        int y;",
            "};",
            "struct S {",
            "    private:",
            "        int z;",
            "};",
            "class Item {",
            "    public:",
            "        Item(): value{1}, other(2), third{3} {",
            "            call();",
            "        }",
            "};",
        )
    );

    let mut indent_modifiers = FormatOptions::default();
    indent_modifiers.indent_modifiers = true;
    assert_eq!(
        format_c(source, &indent_modifiers),
        fixture!(
            "class C {",
            "  public:",
            "    int x;",
            "  protected:",
            "    void f();",
            "  private:",
            "    int y;",
            "};",
            "struct S {",
            "  private:",
            "    int z;",
            "};",
            "class Item {",
            "  public:",
            "    Item(): value{1}, other(2), third{3} {",
            "        call();",
            "    }",
            "};",
        )
    );
}

#[test]
fn logical_operand_after_multiline_call_keeps_chain_indent() {
    let source = fixture!(
        "int helper(void)",
        "{",
        "    if (check_size(&size,",
        "                   alpha,",
        "                   beta) ||",
        "        append_size(&size, gamma) ||",
        "        size > limit) {",
        "        return 0;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn repeated_wrapped_calls_keep_logical_chain_indent() {
    let source = fixture!(
        "int helper(void)",
        "{",
        "    if (update(ctx, &opts->alpha,",
        "               sizeof(opts->alpha)) != 1 ||",
        "        update(ctx, &opts->beta,",
        "               sizeof(opts->beta)) != 1 ||",
        "        update(ctx, values,",
        "               sizeof(values)) != 1) {",
        "        return -1;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn source_aligned_logical_calls_preserve_explicit_columns() {
    let source = fixture!(
        "int helper(void)",
        "{",
        "    if (mount_path(root, source, target, \"none\",",
        "                   FLAG_BIND, NULL) ||",
        "                           mount_path(\"\", \"\", target, \"none\",",
        "                   FLAG_READONLY, NULL) ||",
        "                           mount_path(\"\", \"\", target, \"none\",",
        "                   FLAG_PRIVATE, NULL)) {",
        "        return -1;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_arguments_after_unary_pointer_expression_keep_sibling_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    report(\"%s%s%s\",",
        "           *path != '/' ? base : \"\",",
        "           *path != '/' ? \"/\" : \"\",",
        "           path);",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_argument_after_cast_expression_keeps_sibling_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    report(\"%p %s %s\",",
        "           (void *)ctx, left, right,",
        "           first->value, second->value);",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn one_line_ternary_return_does_not_leak_into_next_logical_chain() {
    let source = fixture!(
        "static const char *month_text(uint8_t month)",
        "{",
        "    return month >= 1u && month <= 2u ? months[month - 1u] : nullptr;",
        "}",
        "",
        "static bool show_value(const struct view *view)",
        "{",
        "    if (!write_text(&temperature, \"T=\") ||",
        "        ((view->reasons & FLAG_X) !=",
        "         0u ? !write_text(&temperature, \"??\") :",
        "         !write_number(&temperature, view->temperature_c, 1u)) ||",
        "        !write_text(&temperature, \" Wm=\") ||",
        "        !write_number(&temperature, view->w, 1u)) {",
        "        return false;",
        "    }",
        "    return true;",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn case_block_ternary_arm_keeps_assignment_continuation_column() {
    let source = fixture!(
        "static void helper(uint8_t month)",
        "{",
        "    uint8_t next = 0u;",
        "    switch (month) {",
        "        case 12u: {",
        "            next.month = next.month == 12u ?",
        "                         1u : (uint8_t)(next.month + 1u);",
        "            break;",
        "        }",
        "        default:",
        "            break;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn case_block_statement_after_multiline_ternary_keeps_case_body_indent() {
    let source = fixture!(
        "static bool helper(uint8_t value)",
        "{",
        "    uint8_t result = 0u;",
        "    switch (value) {",
        "        case 2u:",
        "            if (value > 1u) {",
        "                return false;",
        "            }",
        "            result =",
        "                result == 1u ?",
        "                2u :",
        "                (uint8_t)(result + 1u);",
        "            return true;",
        "        default:",
        "            break;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_closing_after_ternary_argument_keeps_callee_column() {
    let source = fixture!(
        "static bool helper(void)",
        "{",
        "    return write_glyph(",
        "               line, alt_mode ? UINT8_C(0x2a) : UINT8_C(0x20)",
        "           ) &&",
        "           write_text(line, \"  \");",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_argument_after_ternary_argument_keeps_call_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    m = new Text(this, id,",
                "        url.empty() ? String(\"x\")",
                "                    : url,",
                "        pos, size,",
                "        FLAGS);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    m = new Text(this, id,",
            "                 url.empty() ? String(\"x\")",
            "                 : url,",
            "                 pos, size,",
            "                 FLAGS);",
            "}",
        )
    );
}

#[test]
fn split_new_call_nested_call_argument_aligns_to_nested_open_paren() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Widget * const cell = new GenericText",
                "                                (",
                "                                    parent,",
                "                                    ID,",
                "                                    String::format(\"(%d, %d)\",",
                "                                                   i + 1, j + 1)",
                "                                );",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Widget * const cell = new GenericText",
            "    (",
            "        parent,",
            "        ID,",
            "        String::format(\"(%d, %d)\",",
            "                       i + 1, j + 1)",
            "    );",
            "}",
        )
    );
}

#[test]
fn source_indented_macro_call_does_not_shift_following_sibling() {
    let source = fixture!(
        "static const struct Entry entries[] = {",
        "    ITEM_SIZE_KIND(GROUP, \"batch\",",
        "                   OPT_BATCH, \"N\",",
        "                   \"batch size\", opts.batch),",
        "                   ITEM_EXPR(GROUP, \"server\",",
        "                             OPT_SERVER, \"PATH\",",
        "                             \"server socket\"),",
        "    ITEM_EXPR(GROUP, \"client\",",
        "              OPT_CLIENT, \"PATH\",",
        "              \"client socket\"),",
        "};",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn macro_call_after_closed_nested_condition_aligns_arguments() {
    assert_eq!(
        format_c(
            fixture!(
                "void helper(void)",
                "{",
                "    switch (kind) {",
                "        case ALPHA: {",
                "            if (__unlikely(!call(object.field, &item->field,",
                "                                  buffer, sizeof(buffer)))) {",
                "                int err = errno;",
                "                REPORT_ERROR(port, \"%s error %d: failed: %s\",",
                "                       __func__, err, TEXT(err));",
                "                cleanup();",
                "                return;",
                "            }",
                "            break;",
                "        }",
                "    }",
                "}",
            ),
            &kr_c_options(),
        ),
        fixture!(
            "void helper(void)",
            "{",
            "    switch (kind) {",
            "        case ALPHA: {",
            "            if (__unlikely(!call(object.field, &item->field,",
            "                                 buffer, sizeof(buffer)))) {",
            "                int err = errno;",
            "                REPORT_ERROR(port, \"%s error %d: failed: %s\",",
            "                             __func__, err, TEXT(err));",
            "                cleanup();",
            "                return;",
            "            }",
            "            break;",
            "        }",
            "    }",
            "}",
        )
    );
}

#[test]
fn macro_call_bitwise_chain_preserves_source_columns() {
    let source = fixture!(
        "MAIN(\"tool\", tool_main,",
        "              MAIN_ONE |",
        "              MAIN_TWO |",
        "              MAIN_THREE)",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn declaration_macro_before_noexcept_condition_keeps_source_indent() {
    let source = fixture!(
        "DECL",
        "inline void swap(T& left, T& right) noexcept(  // note",
        "    is_nothrow_move_constructible<T>::value&&  // note",
        "    is_nothrow_move_assignable<T>::value)",
        "{}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn noexcept_condition_after_trailing_comment_keeps_source_continuation_indent() {
    let source = fixture!(
        "class Item",
        "{",
        "    Item& operator=(Item other) noexcept ( // note",
        "        std::is_nothrow_move_constructible<value_type>::value&&",
        "        std::is_nothrow_move_assignable<value_type>::value&&",
        "        std::is_nothrow_move_constructible<stored_value>::value&&",
        "        std::is_nothrow_move_assignable<stored_value>::value&&",
        "        std::is_nothrow_move_assignable<base_type>::value",
        "    )",
        "    {}",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn logical_operand_after_wrapped_long_call_keeps_chain_indent() {
    let source = fixture!(
        "int helper(void)",
        "{",
        "    if (ckd_mul(&table_size,",
        "                (size_t)item_count,",
        "                (size_t)item_size) ||",
        "        ckd_add(&size, (size_t)offset, table_size) ||",
        "        size > (size_t)limit) {",
        "        return 0;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn assignment_ternary_arms_preserve_source_alignment() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    struct item *item = condition",
        "                        ? &alpha",
        "                        : &beta;",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn ternary_assignment_continuation_aligns_to_value_start() {
    // A continuation after `:` aligns to the assignment value regardless of arm parentheses.
    assert_eq!(
        format_c(
            "\nvoid foo()\n{\n\tvalue += flag ? (((alpha << 3) + 4) * unit) :\n\t\t\t\t(((alpha << 3) + 8) * unit);\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo()\n{\n    value += flag ? (((alpha << 3) + 4) * unit) :\n             (((alpha << 3) + 8) * unit);\n}\n",
    );
}

#[test]
fn ternary_assignment_continuation_with_paren_condition_aligns_to_value_start() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tlen = (tail >= head) ? (tail - head) :\n\t\t(rx_mask + 1 - head);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    len = (tail >= head) ? (tail - head) :\n          (rx_mask + 1 - head);\n}\n",
    );
}

#[test]
fn assignment_ternary_colon_arm_after_split_call_condition_aligns_to_value_start() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tresult = (call()) ?\n\t\t\tcall() :\n\t\t\t&value;\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    result = (call()) ?\n             call() :\n             &value;\n}\n",
    );
}

#[test]
fn chained_ternary_after_bare_return_keeps_one_continuation_indent() {
    let source = fixture!(
        "",
        "int foo(int size)",
        "{",
        "    return",
        "        size == 1 ? first() :",
        "        size == 2 ? second() :",
        "        fallback();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_assignment_logical_tail_keeps_assignment_continuation_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    static constexpr bool ok",
        "        = (a && b && c) ||",
        "          (d && e && f);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_assignment_call_preserves_source_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    enum result res =",
        "        parse(&ctx, \"value\");",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_argument_rows_preserve_explicit_source_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    assert_string_equal(\"message\",",
        "    describe_error(code, (char[64]) {}, 64));",
        "",
        "    size_t len = build_header(",
        "                     buf, sizeof(buf), client, server, proto,",
        "                     (uint32_t)conn->id, conn->hash, config.id,",
        "                     config.path, true);",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_argument_string_continuation_preserves_source_indent() {
    let source = fixture!(
        "void helper(void)",
        "{",
        "    return wrap(format(",
        "                    buffer, end, \"prefix \"",
        "                    \"tail\", size));",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn case_block_call_arguments_align_to_call_anchor() {
    let source = fixture!(
        "void helper(int value)",
        "{",
        "    switch (value) {",
        "        case 1: {",
        "            return encode(result, value, mode,",
        "                          source->addr, target->addr,",
        "                          source->port, target->port);",
        "        }",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn case_block_adjacent_string_arguments_align_to_call_anchor() {
    let source = fixture!(
        "void helper(int value)",
        "{",
        "    switch (value) {",
        "        case 1: {",
        "            log_event(value,",
        "                      \"prefix \"",
        "                      \"tail\");",
        "            return;",
        "        }",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &one_true_brace_c_options()), source);
}

#[test]
fn call_arguments_after_multiline_string_keep_sibling_column() {
    let source = fixture!(
        "void helper(const struct Config *config)",
        "{",
        "    push_format(config,",
        "                \"%.*s/item/%s.txt\" \"%s\"",
        "                \"%.*s/item/%s/init.txt\",",
        "                VALUE(config->path), MARK, SEP,",
        "                VALUE(config->path), MARK, SEP,",
        "                VALUE(config->path), MARK);",
        "}",
    );

    assert_eq!(format_c(source, &kr_c_options()), source);
}

#[test]
fn vtk_indent_namespaces_nests_namespace_members() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=vtk".to_owned(), "--indent-namespaces".to_owned()],
    )
    .expect("valid options");
    let actual = format_c(
        fixture!(
            "namespace Alpha{",
            "namespace Beta{",
            "int value;",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace Alpha",
            "{",
            "    namespace Beta",
            "    {",
            "        int value;",
            "    }",
            "}",
        )
    );
}

#[test]
fn wrapped_parenthesized_ternary_false_arm_aligns_to_condition() {
    let actual = format_c(
        fixture!(
            "void helper(Integer value, Integer delta)",
            "{",
            "    if (!(value >= 0 ? (Unsigned)value <= (Unsigned)INT_MAX + delta :",
            "                value >= (Integer)INT_MIN + delta)) {",
            "        value -= delta;",
            "    }",
            "}",
        ),
        &kr_c_options(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void helper(Integer value, Integer delta)",
            "{",
            "    if (!(value >= 0 ? (Unsigned)value <= (Unsigned)INT_MAX + delta :",
            "          value >= (Integer)INT_MIN + delta)) {",
            "        value -= delta;",
            "    }",
            "}",
        )
    );
}

#[test]
fn closing_paren_after_long_template_parameter_list_keeps_call_continuation_indent() {
    let actual = format_c(
        fixture!(
            "template<typename Class,",
            "typename T0, typename T1, typename T2, typename T3, typename T4, typename T5, \\",
            "typename T6, typename T7>",
            "struct X",
            "{",
            "    bool Create()",
            "    {",
            "        return obj->Create(",
            "            (args[0]).As(static_cast<T0*>(nullptr)),",
            "            (args[1]).As(static_cast<T1*>(nullptr)),",
            "            (args[2]).As(static_cast<T2*>(nullptr)),",
            "            (args[3]).As(static_cast<T3*>(nullptr)),",
            "            (args[4]).As(static_cast<T4*>(nullptr)),",
            "            (args[5]).As(static_cast<T5*>(nullptr)),",
            "            (args[6]).As(static_cast<T6*>(nullptr)),",
            "            (args[7]).As(static_cast<T7*>(nullptr))",
            "            );",
            "    }",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "template<typename Class,",
            "         typename T0, typename T1, typename T2, typename T3, typename T4, typename T5, \\",
            "         typename T6, typename T7>",
            "struct X",
            "{",
            "    bool Create()",
            "    {",
            "        return obj->Create(",
            "                   (args[0]).As(static_cast<T0*>(nullptr)),",
            "                   (args[1]).As(static_cast<T1*>(nullptr)),",
            "                   (args[2]).As(static_cast<T2*>(nullptr)),",
            "                   (args[3]).As(static_cast<T3*>(nullptr)),",
            "                   (args[4]).As(static_cast<T4*>(nullptr)),",
            "                   (args[5]).As(static_cast<T5*>(nullptr)),",
            "                   (args[6]).As(static_cast<T6*>(nullptr)),",
            "                   (args[7]).As(static_cast<T7*>(nullptr))",
            "               );",
            "    }",
            "};",
        )
    );
}

#[test]
fn return_new_source_split_open_paren_aligns_under_new_expression() {
    let source = fixture!(
        "Dialog* f()",
        "{",
        "    return new Dialog",
        "           (",
        "               parent,",
        "               message",
        "           );",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn ternary_colon_aligns_with_question_in_case_block() {
    let source = fixture!(
        "void f(int x)",
        "{",
        "    switch (x)",
        "    {",
        "    case 1:",
        "        if ( a )",
        "        {",
        "            next = vkey == VK_UP",
        "                   ? prev()",
        "                   : next();",
        "        }",
        "        break;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn source_split_nested_call_open_paren_line_aligns_under_call() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    CHECK_CALL(TextType::create",
            "              (",
            "                \"message\",",
            "                value",
            "              ));",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    CHECK_CALL(TextType::create",
            "               (",
            "                   \"message\",",
            "                   value",
            "               ));",
            "}",
        )
    );
}

#[test]
fn return_boolean_chain_preserves_extra_space_alignment() {
    let source = fixture!(
        "bool f()",
        "{",
        "    return  A() &&",
        "",
        "            B() &&",
        "            C();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assignment_logical_chain_preserves_tabbed_rhs_alignment() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "\tret =\t((arg & A) && changed(a)) ||",
            "\t\t((arg & B) && changed(b)) ||",
            "\t\t((arg & C) && changed(c));",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    ret =\t((arg & A) && changed(a)) ||",
            "            ((arg & B) && changed(b)) ||",
            "            ((arg & C) && changed(c));",
            "}",
        )
    );
}

#[test]
fn google_indent_classes_composes_with_indent_modifiers() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=google".to_string(), "--indent-classes".to_string()],
    )
    .expect("valid options");

    // Explicit class and modifier indentation levels remain additive.
    assert_eq!(
        format_c("class Item{public:int value;};\n", &options),
        fixture!("class Item {", "    public:", "        int value;", "};",),
    );
}

#[test]
fn indent_classes_applies_to_access_modified_structs_only() {
    let mut options = FormatOptions::default();
    options.indent_classes = true;
    let source = fixture!(
        "struct Plain {",
        "int x;",
        "};",
        "struct Tagged {",
        "public:",
        "int y;",
        "};",
        "union U {",
        "int a;",
        "float b;",
        "};",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "struct Plain {",
            "    int x;",
            "};",
            "struct Tagged {",
            "    public:",
            "        int y;",
            "};",
            "union U {",
            "    int a;",
            "    float b;",
            "};",
        )
    );
}
#[test]
fn indent_blocks_indents_whole_command_blocks() {
    let mut options = FormatOptions::default();
    options.indent_blocks = true;
    let actual = format_with(fixture!("int f(){if(x){return 1;}return 0;}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (x)",
            "        {",
            "            return 1;",
            "        }",
            "    return 0;",
            "}",
        )
    );
}
#[test]
fn indent_blocks_stacks_nested_command_blocks() {
    let mut options = FormatOptions::default();
    options.indent_blocks = true;
    let actual = format_with(fixture!("int f(){if(x){while(y){return 1;}}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (x)",
            "        {",
            "            while (y)",
            "                {",
            "                    return 1;",
            "                }",
            "        }",
            "}",
        )
    );
}
#[test]
fn indent_blocks_indents_switch_block_brace_and_body() {
    let mut options = FormatOptions::default();
    options.indent_blocks = true;
    let actual = format_with(
        fixture!(
            "void f(int x){",
            "switch (x) {",
            "case 1:",
            "g();",
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
            "void f(int x)",
            "{",
            "    switch (x)",
            "        {",
            "        case 1:",
            "            g();",
            "            break;",
            "        default:",
            "            break;",
            "        }",
            "}",
        )
    );
}
#[test]
fn indent_blocks_keeps_struct_union_enum_exceptions_and_indents_switch() {
    let mut options = FormatOptions::default();
    options.indent_blocks = true;
    let actual = format_with(
        fixture!(
            "struct S{int x;};",
            "union U{int x;};",
            "enum E{A};",
            "int f(int x){switch(x){case 1:return 1;}if(x){return 0;}}",
        ),
        &options,
    );

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
            "};",
            "enum E {A};",
            "int f(int x)",
            "{",
            "    switch (x)",
            "        {",
            "        case 1:",
            "            return 1;",
            "        }",
            "    if (x)",
            "        {",
            "            return 0;",
            "        }",
            "}",
        )
    );
}
#[test]
fn keeps_namespace_members_unindented_by_default() {
    assert_eq!(
        format(fixture!("namespace N { int x; }")),
        fixture!("namespace N", "{", "int x;", "}")
    );
}

#[test]
fn scope_resolution_call_keeps_function_body_indent() {
    assert_eq!(
        format(fixture!("void f(){object::method();}")),
        fixture!("void f()", "{", "    object::method();", "}")
    );
}

#[test]
fn scope_resolution_split_definition_keeps_continuation_at_base() {
    let source =
        "ImageMap::\nConvertToStandard(unsigned char brightness) const\n{\n    return x;\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn template_function_keeps_body_indent() {
    assert_eq!(
        format(fixture!("template <typename T>", "void g(T x){h(x);}",)),
        fixture!(
            "template <typename T>",
            "void g(T x)",
            "{",
            "    h(x);",
            "}",
        )
    );
}
#[test]
fn indents_namespace_blocks_when_requested() {
    let mut options = FormatOptions::default();
    options.indent_namespaces = true;
    let actual = format_with(fixture!("namespace N{int x;}"), &options);

    assert_eq!(actual, fixture!("namespace N", "{", "    int x;", "}",));
}
#[test]
fn module_used_as_function_name_indents_body() {
    let actual = format_c(
        fixture!("void module()", "{", "int bar = 1;", "}"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!("void module()", "{", "    int bar = 1;", "}")
    );
}
#[test]
fn module_used_as_objc_method_name_indents_body() {
    let actual = format_c(
        fixture!("-(void)module", "{", "int bar = 1;", "}"),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!("-(void)module", "{", "    int bar = 1;", "}")
    );
}
#[test]
fn namespace_named_interface_does_not_indent_body() {
    let actual = format_c(
        fixture!(
            "namespace abc",
            "{",
            "namespace interface",
            "{",
            "namespace xyz",
            "{",
            "",
            "}",
            "}",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "namespace abc",
            "{",
            "namespace interface",
            "{",
            "namespace xyz",
            "{",
            "",
            "}",
            "}",
            "}",
        )
    );
}
#[test]
fn corba_module_acts_as_namespace_and_interface_indents_body() {
    let actual = format_c(
        fixture!(
            "module alpha {",
            "module beta {",
            "interface Server {",
            "int value;",
            "};",
            "};",
            "};",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "module alpha {",
            "module beta {",
            "interface Server {",
            "    int value;",
            "};",
            "};",
            "};",
        )
    );
}
#[test]
fn uses_tab_indentation_option() {
    let mut options = FormatOptions::default();
    options.indent_style = cstyle::config::IndentStyle::Tabs;
    let actual = format_with(fixture!("int f(){return 0;}"), &options);

    assert_eq!(actual, fixture!("int f()", "{", "\treturn 0;", "}",));
}
#[test]
fn force_tab_x_uses_tabs_plus_spaces_for_indentation() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.indent_width = 4;
    options.tab_width = 8;
    let actual = format_with(fixture!("int f(){if(x){return 1;}}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (x)",
            "    {",
            "\treturn 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn force_tab_x_closing_braces_reuse_structural_columns() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 6;

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "if (alpha)",
                "{",
                "while (beta)",
                "{",
                "call();",
                "}",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if (alpha)",
            "    {",
            "\t  while (beta)",
            "\t  {",
            "\t\tcall();",
            "\t  }",
            "    }",
            "}",
        ),
    );
}

#[test]
fn force_tab_x_repeated_condition_rows_keep_the_same_column() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 6;

    assert_eq!(
        format_c(
            fixture!(
                "void run()",
                "{",
                "if (alpha",
                "&& beta",
                "&& gamma)",
                "call();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run()",
            "{",
            "    if (alpha",
            "\t\t&& beta",
            "\t\t&& gamma)",
            "\t  call();",
            "}",
        ),
    );
}

#[test]
fn force_tab_x_converts_internal_tabs_from_the_source_column() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    options.indent_style = IndentStyle::ForceTabs;
    options.tab_width = 6;

    assert_eq!(
        format_c(
            fixture!("void run()", "{", "int\tvalue\t=\t1;", "}"),
            &options,
        ),
        fixture!("void run()", "{", "    int   value =     1;", "}"),
    );
}

#[test]
fn later_tab_indent_replaces_force_tab_x_for_structural_tabs() {
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
                "void run(){",
                "if(alpha){",
                "while(beta){",
                "call();",
                "}",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "\tif(alpha) {",
            "\t\twhile(beta) {",
            "\t\t\tcall();",
            "\t\t}",
            "\t}",
            "}",
        ),
    );
}

#[test]
fn later_tab_indent_replaces_force_tab_x_for_repeated_condition_rows() {
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
                "void run(){",
                "if(alpha",
                "&& beta",
                "&& gamma){",
                "call();",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "\tif(alpha",
            "\t            && beta",
            "\t            && gamma) {",
            "\t\tcall();",
            "\t}",
            "}",
        ),
    );
}

#[test]
fn later_tab_indent_replaces_force_tab_x_for_assignment_call_rows() {
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
                "void run(){",
                "value = call(",
                "/* note */",
                "\"text\");",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "\tvalue = call(",
            "\t              /* note */",
            "\t              \"text\");",
            "}",
        ),
    );
}

#[test]
fn later_spaces_indent_replaces_force_tab_x_state() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &[
            "--indent=force-tab-x=6".to_owned(),
            "--indent=spaces=2".to_owned(),
        ],
    )
    .expect("valid indentation options");

    assert_eq!(
        format_c(
            fixture!(
                "void run(){",
                "if(alpha){",
                "while(beta){",
                "call();",
                "}",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "  if(alpha) {",
            "    while(beta) {",
            "      call();",
            "    }",
            "  }",
            "}",
        ),
    );
}

#[test]
fn converts_leading_tabs_when_requested() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    options.tab_width = 2;
    let actual = format_with(
        fixture!("// *INDENT-OFF*", "\t\treturn;", "// *INDENT-ON*",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("// *INDENT-OFF*", "    return;", "// *INDENT-ON*",)
    );
}

#[test]
fn convert_tabs_with_tab_indent_keeps_structural_tabs() {
    let mut options = FormatOptions::default();
    options.convert_tabs = true;
    options.indent_style = IndentStyle::Tabs;

    assert_eq!(
        format_c(fixture!("void f()", "{", "int\ta\t=\t1;", "}"), &options,),
        fixture!("void f()", "{", "\tint a   =   1;", "}"),
    );
}

#[test]
fn parenthesis_continuation_after_tabs_uses_visual_column() {
    assert_eq!(
        format_c(
            "\nvoid f()\n{\n    obj\tname\t( alpha,\n    beta );\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid f()\n{\n    obj\tname\t( alpha,\n                  beta );\n}\n",
    );
}

#[test]
fn indents_ternary_continuation_lines() {
    let actual = format(fixture!("int f(int x){return x?", "1:", "0;}",));
    assert_eq!(
        actual,
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
fn ternary_comment_between_branches_keeps_branch_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    return mode == InputMode::Text",
        "           // note",
        "           ? parse()",
        "           // note",
        "           : read();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_assertion_macro_comparison_aligns_after_open_paren() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    CHECK_EXPECTED(value.As<Type*>()",
                "                        == expected);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    CHECK_EXPECTED(value.As<Type*>()",
            "                   == expected);",
            "}",
        )
    );
}

#[test]
fn return_call_chain_after_multiline_call_keeps_return_expression_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "std::string f()",
                "{",
                "    return call(a,",
                "                b)",
                "            .next();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "std::string f()",
            "{",
            "    return call(a,",
            "                b)",
            "           .next();",
            "}",
        )
    );
}

#[test]
fn return_call_chain_after_ternary_with_qualified_name_keeps_chain_indent() {
    let source = fixture!(
        "Text f()",
        "{",
        "    return render(\"%1 at (%2, %3)\")",
        "           .with(item->kind() == Item::Box ? \"Box\" : \"Triangle\")",
        "           .with(pos.x()).with(pos.y());",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn call_argument_after_ternary_colon_line_keeps_argument_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    call",
        "    (",
        "        a ? b",
        "        : c,",
        "        d ? e",
        "        : f,",
        "        g",
        "    );",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

// Closed calls inside the ternary do not replace the unmatched outer call as owner.
#[test]
fn ternary_arm_inside_open_call_paren_aligns_to_call_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tx = malloc(flush ? worksize() :\n\t\t worksize(),\n\t\t other());\n}\n",
            &options,
        ),
        "void f(void)\n{\n    x = malloc(flush ? worksize() :\n               worksize(),\n               other());\n}\n",
    );
}

#[test]
fn ternary_call_argument_rows_keep_question_column_after_value_arm() {
    let source = fixture!(
        "void f()",
        "{",
        "    init(value,",
        "         object.HasOption(OPTION_X) ?",
        "         object.GetOption(OPTION_X) : 0,",
        "         object.HasOption(OPTION_Y) ?",
        "         object.GetOption(OPTION_Y) : 0);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assignment_chain_after_constructor_preprocessor_branches_keeps_final_column() {
    assert_eq!(
        format_c(
            fixture!(
                "class Page",
                "{",
                "};",
                "",
                "Page::Page(Book *book,",
                "           Vector<Item>& items)",
                "    : Base(book, items, data)",
                "{",
                "    m_a =",
                "    m_b =",
                "    m_c =",
                "    m_d =",
                "#if ENABLE_ALPHA",
                "    m_e =",
                "#endif // ENABLE_ALPHA",
                "#if ENABLE_BETA",
                "    m_f =",
                "#endif // ENABLE_BETA",
                "    m_g =",
                "    m_h =",
                "    m_i =",
                "    m_j =",
                "    m_k =",
                "    m_l = nullptr;",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "class Page",
            "{",
            "};",
            "",
            "Page::Page(Book *book,",
            "           Vector<Item>& items)",
            "    : Base(book, items, data)",
            "{",
            "    m_a =",
            "        m_b =",
            "            m_c =",
            "                m_d =",
            "#if ENABLE_ALPHA",
            "                    m_e =",
            "#endif // ENABLE_ALPHA",
            "#if ENABLE_BETA",
            "                        m_f =",
            "#endif // ENABLE_BETA",
            "                            m_g =",
            "                                m_h =",
            "                                    m_i =",
            "                                        m_j =",
            "                                            m_k =",
            "                                                    m_l = nullptr;",
            "}",
        )
    );
}

#[test]
fn split_cast_call_paren_keeps_cast_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "bool f(double value)",
                "{",
                "    return value ==",
                "        static_cast<double>",
                "            (expr);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "bool f(double value)",
            "{",
            "    return value ==",
            "           static_cast<double>",
            "           (expr);",
            "}",
        )
    );
}

#[test]
fn tab_indented_outer_call_arguments_align_after_the_outer_paren() {
    let options = options_from_args(&["--indent=tab=4"]);

    assert_eq!(
        format_c(
            fixture!(
                "void run() {",
                "result = outer(inner(alpha,",
                "beta),",
                "gamma);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void run() {",
            "\tresult = outer(inner(alpha,",
            "\t                     beta),",
            "\t               gamma);",
            "}",
        )
    );
}

#[test]
fn split_new_call_in_case_block_keeps_case_body_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    switch (x)",
        "    {",
        "    case 1:",
        "    {",
        "        value = new Item",
        "        (",
        "            alpha,",
        "            beta",
        "        );",
        "    }",
        "    break;",
        "    }",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_new_assignment_over_max_next_argument_keeps_double_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    ContainerModel *containerResult = new ContainerModel(new Component(self,",
        "            ID, value),",
        "            MODE_ALL);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_new_assignment_over_max_continuation_uses_double_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    BoxModel *resultContainer = new BoxModel(new Component(self,",
                "                ID, value), MODE_ALL);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    BoxModel *resultContainer = new BoxModel(new Component(self,",
            "            ID, value), MODE_ALL);",
            "}",
        )
    );
}

#[test]
fn nested_new_call_over_max_continuation_uses_outer_call_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    container->insert( new GenericTreeModelNode( container, \"Primary Record\",",
                "                                                 \"Secondary Value\", 1868 ) );",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    container->insert( new GenericTreeModelNode( container, \"Primary Record\",",
            "                       \"Secondary Value\", 1868 ) );",
            "}",
        )
    );
}

#[test]
fn pointer_lvalue_after_split_assignment_indents_one_level() {
    let source = fixture!("void f()", "{", "    *x =", "        *y = value;", "}",);

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_nested_new_call_uses_open_paren_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Holder value(new Item",
                "        (",
                "            parent,",
                "            id,",
                "            name",
                "        ));",
                "",
                "    LongTemplateName<Control> value(new LongTemplateName<Control>",
                "        (",
                "            parent,",
                "            id,",
                "            name",
                "        ));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Holder value(new Item",
            "                 (",
            "                     parent,",
            "                     id,",
            "                     name",
            "                 ));",
            "",
            "    LongTemplateName<Control> value(new LongTemplateName<Control>",
            "                                    (",
            "                                        parent,",
            "                                        id,",
            "                                        name",
            "                                    ));",
            "}",
        )
    );
}

#[test]
fn return_ternary_branch_after_colon_keeps_first_branch_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    return cond ?",
        "           first(",
        "               arg) :",
        "           second(",
        "               arg);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn return_ternary_parenthesized_minus_tail_aligns_inside_value() {
    assert_eq!(
        format_c(
            "int f() {\n  return condition ? 112\n         : (digits -\n           (flag ? 1 : 0));\n}\n",
            &FormatOptions::default(),
        ),
        "int f() {\n    return condition ? 112\n           : (digits -\n              (flag ? 1 : 0));\n}\n",
    );
}

#[test]
fn return_parenthesized_ternary_colon_branch_keeps_value_indent() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            "int f(struct Obj *obj)\n{\n\treturn (read_length(obj->length) <= ITEM_LIMIT_VALUE ?\n\t\tread_length(obj->length) :\n\t\t(read_length(obj->length) - ITEM_LIMIT_VALUE));\n}\n",
            &options,
        ),
        "int f(struct Obj *obj)\n{\n    return (read_length(obj->length) <= ITEM_LIMIT_VALUE ?\n            read_length(obj->length) :\n            (read_length(obj->length) - ITEM_LIMIT_VALUE));\n}\n",
    );
}

#[test]
fn return_ternary_after_split_template_function_parameter_stays_at_body_indent() {
    assert_eq!(
        format_c(
            "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r,\n                 context_id ctx = {}) -> Result {\n  return check(value)\n      ? apply_value(dst, value, r)\n      : apply_other(dst, value, r, ctx);\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r,\ncontext_id ctx = {}) -> Result {\n    return check(value)\n    ? apply_value(dst, value, r)\n    : apply_other(dst, value, r, ctx);\n}\n",
    );
}

#[test]
fn return_logical_after_split_template_function_parameter_stays_at_body_indent() {
    assert_eq!(
        format_c(
            "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r = {},\n                 context_id = {}) -> Result {\n  return r.type() != none &&\n         r.type() != string\n      ? apply_value(dst, value, r)\n      : apply_bytes(dst, value ? \"true\" : \"false\", r);\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T>\nFIXED auto apply(Result dst, T value, const rules& r = {},\ncontext_id = {}) -> Result {\n    return r.type() != none &&\n    r.type() != string\n    ? apply_value(dst, value, r)\n    : apply_bytes(dst, value ? \"true\" : \"false\", r);\n}\n",
    );
}

#[test]
fn return_ternary_colon_after_split_template_declaration_logical_condition_aligns_to_arm() {
    assert_eq!(
        format_c(
            "template <typename T,\n          SELECT_IF(check<T>::value)>\nFIXED auto apply(Result dst, T value, const rules& r = {},\n                 context_id = {}) -> Result {\n  return r.type() != none &&\n                 r.type() != string\n             ? apply_value(dst, value, r)\n             : apply_bytes(dst, value ? \"true\" : \"false\", r);\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T,\n          SELECT_IF(check<T>::value)>\nFIXED auto apply(Result dst, T value, const rules& r = {},\ncontext_id = {}) -> Result {\n    return r.type() != none &&\n    r.type() != string\n    ? apply_value(dst, value, r)\n        : apply_bytes(dst, value ? \"true\" : \"false\", r);\n}\n",
    );
}

#[test]
fn statement_level_ternary_colon_aligns_to_base_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    condition() ? call1()",
                "                : call2();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    condition() ? call1()",
            "    : call2();",
            "}",
        )
    );
}

#[test]
fn statement_level_ternary_continuation_aligns_to_base_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tflag ? call_a(x, y) :\n\t\tcall_b(x, y);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    flag ? call_a(x, y) :\n    call_b(x, y);\n}\n",
    );
}

#[test]
fn statement_level_ternary_with_multiple_parens_continuation_aligns_to_base_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tcond(a) ? aa(bb(x), cc(y)) :\n\t\tdd(x);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    cond(a) ? aa(bb(x), cc(y)) :\n    dd(x);\n}\n",
    );
}

#[test]
fn stroustrup_ternary_colon_continuations_preserve_source_gap() {
    let options = options_from_args(&["--style=stroustrup", "--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "int f(int a)",
                "{",
                "    return a ? alpha()",
                "         : beta();",
                "}",
            ),
            &options,
        ),
        fixture!(
            "int f(int a)",
            "{",
            "    return a ? alpha()",
            "           : beta();",
            "}",
        )
    );
    assert_eq!(
        format_c(
            fixture!(
                "template <bool Flag, bool Other>",
                "void f(char* dst,",
                "                                const char* src, size_t size) {",
                "  size < 16 ? alpha<Flag, /*Other=*/true>(dst, src, size)",
                "            : alpha<Flag, /*Other=*/false>(dst, src, size);",
                "}",
            ),
            &options,
        ),
        fixture!(
            "template <bool Flag, bool Other>",
            "void f(char* dst,",
            "       const char* src, size_t size)",
            "{",
            "    size < 16 ? alpha<Flag, /*Other=*/true>(dst, src, size)",
            "         : alpha<Flag, /*Other=*/false>(dst, src, size);",
            "}",
        )
    );
}

#[test]
fn call_argument_ternary_colon_aligns_to_question_line_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    call(",
                "        condition ? Alpha",
                "    : Beta);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    call(",
            "        condition ? Alpha",
            "        : Beta);",
            "}",
        )
    );
}

#[test]
fn ternary_value_after_colon_aligns_to_previous_arm_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    target.setBrush(mode == Editable ?",
        "                    palette.highlight() :",
        "                    palette.windowText());",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn ternary_with_braced_true_arm_breaks_after_colon() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    value = cond ? Item{1} : other;",
                "    return cond ? Item{1} : other;",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    value = cond ? Item{1} :",
            "            other;",
            "    return cond ? Item{1} :",
            "           other;",
            "}",
        )
    );
}

#[test]
fn repeated_plus_continuation_keeps_previous_operator_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    return ValueArray::encode(width) + 'x' + ValueArray::encode(height)",
                "            + (left < 0 ? '-' : '+') + ValueArray::encode(left)",
                "            + (top < 0 ? '-' : '+') + ValueArray::encode(top);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    return ValueArray::encode(width) + 'x' + ValueArray::encode(height)",
            "           + (left < 0 ? '-' : '+') + ValueArray::encode(left)",
            "           + (top < 0 ? '-' : '+') + ValueArray::encode(top);",
            "}",
        )
    );
}

#[test]
fn empty_braced_call_argument_continuation_aligns_to_call_arguments() {
    let source = fixture!(
        "bool make()",
        "{",
        "    return runProcess(binary, args, error, destination,",
        "                      {}, 60000);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn function_definition_params_cap_at_max_continuation_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "static VeryLongReturnTypeNameHere mapValueFromRow(StructuredReader &row,",
                "                                                  Operation::Options options)",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "static VeryLongReturnTypeNameHere mapValueFromRow(StructuredReader &row,",
            "        Operation::Options options)",
            "{",
            "}",
        )
    );
}

#[test]
fn nested_call_argument_over_max_aligns_to_outer_call_argument() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    VeryLongOuterFunctionNameHere(GenericNamespace::transform(\"context\",",
                "                                                              \"message\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    VeryLongOuterFunctionNameHere(GenericNamespace::transform(\"context\",",
            "                                  \"message\"));",
            "}",
        )
    );
}

#[test]
fn nested_long_declaration_call_argument_over_max_uses_two_level_fallback() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    FixedValueArray<RepresentativeWorker, memberCount> members(clip(Process::recommendedCount(),",
                "                                                                    int(memberCount)));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    FixedValueArray<RepresentativeWorker, memberCount> members(clip(Process::recommendedCount(),",
            "            int(memberCount)));",
            "}",
        )
    );
}

#[test]
fn nested_member_call_argument_over_max_uses_previous_line_fallback() {
    let source = fixture!(
        "void f()",
        "{",
        "    MessageNode::applyValues(this, id(\"Unable to read value\"),",
        "                             id(\"Cannot read %1: %2\").arg(Path::formatPathSegments(itemName()),",
        "                                     item.errorDetail()));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn over_max_inner_call_open_argument_uses_outer_call_offset() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    ASSERT(foo(a, b, c).remove(",
                "        value));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    ASSERT(foo(a, b, c).remove(",
            "               value));",
            "}",
        )
    );
}

#[test]
fn outer_call_argument_after_closed_inner_call_uses_outer_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    VERIFY(runHelper(PlainTextValue(\"helper\"),",
        "                     ValueVector() << PlainTextValue(\"--text\") << value,",
        "                     &errorContext),",
        "           errorContext.dataValue());",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn closed_inner_call_argument_continues_at_outer_call_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    target->addItem(state()->defaultValue(",
        "                        Value::Entry), id(\"Entry\"),",
        "                    Item::Entry);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn closed_member_call_argument_keeps_previous_line_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    target->setItem(Item::fromSource(",
        "                        entry.owner()->data(entry, Kind).asString(),",
        "                        \"pattern\"));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn qprintable_argument_aligns_to_nested_callee_name() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    CHECK_OK(ok, qPrintable(",
                "        QString(\"message\")",
                "        .arg(value)));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    CHECK_OK(ok, qPrintable(",
            "                 QString(\"message\")",
            "                 .arg(value)));",
            "}",
        )
    );
}

#[test]
fn qprintable_long_qualified_argument_aligns_to_nested_callee_name() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    CHECK_OK(context.operationReady(), qPrintable(",
                "        QString::fromSource(\"Could not apply %1: %2\").arg(source, context.errorDetail())));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    CHECK_OK(context.operationReady(), qPrintable(",
            "                 QString::fromSource(\"Could not apply %1: %2\").arg(source, context.errorDetail())));",
            "}",
        )
    );
}

#[test]
fn struct_assignment_ternary_continuation_is_not_overindented() {
    let actual = format(fixture!(
        "void f(){",
        "struct Config result =",
        "cond ?",
        "first : second;",
        "}",
    ));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    struct Config result =",
            "        cond ?",
            "        first : second;",
            "}",
        )
    );
}

#[test]
fn struct_split_assignment_continuation_is_not_overindented() {
    // Split assignments use one continuation level regardless of the type spelling.
    let actual = format(fixture!(
        "void f(){",
        "struct Config result =",
        "value;",
        "Config *other =",
        "value;",
        "int plain =",
        "value;",
        "}",
    ));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    struct Config result =",
            "        value;",
            "    Config *other =",
            "        value;",
            "    int plain =",
            "        value;",
            "}",
        )
    );
}

#[test]
fn split_pointer_assignment_rhs_uses_one_continuation_indent() {
    assert_eq!(
        format_c(
            "int f() {\n  auto* value =\n  condition ? \"a\" : \"b\";\n}\n",
            &FormatOptions::default(),
        ),
        "int f() {\n    auto* value =\n        condition ? \"a\" : \"b\";\n}\n",
    );
}

#[test]
fn split_pointer_to_pointer_parameter_uses_function_parameter_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void Type::mark ( Item",
                "                             ** chain )",
                "{",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void Type::mark ( Item",
            "                  ** chain )",
            "{",
            "}",
        )
    );
}

#[test]
fn split_pointer_assignment_after_wrapped_call_uses_one_continuation_indent() {
    assert_eq!(
        format_c(
            "template <typename T, typename F>\nauto apply(Result dst, const rules& r,\n           size_t span, F&& f) -> Result {\n  static_assert(a || b,\n                \"\");\n  auto* value =\n      condition ? \"a\" : \"b\";\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T, typename F>\nauto apply(Result dst, const rules& r,\n           size_t span, F&& f) -> Result {\n    static_assert(a || b,\n                  \"\");\n    auto* value =\n        condition ? \"a\" : \"b\";\n}\n",
    );
}

#[test]
fn split_declaration_assignment_after_multiline_template_function_uses_one_continuation_indent() {
    assert_eq!(
        format_c(
            "template <typename T,\n          SELECT_IF(check<T>::value)>\nFIXED auto apply(Result dst, span<T> value,\n                 const rules& r) -> Result {\n  bool flag = r.type();\n  if (r.threshold < 0 && r.count == 0) {\n    return flag ? apply_encoded(dst, value) : copy(value, dst);\n  }\n\n  size_t limit =\n      r.threshold < 0 ? SIZE_MAX : as_unsigned(r.threshold);\n}\n",
            &FormatOptions::default(),
        ),
        "template <typename T,\n          SELECT_IF(check<T>::value)>\nFIXED auto apply(Result dst, span<T> value,\n                 const rules& r) -> Result {\n    bool flag = r.type();\n    if (r.threshold < 0 && r.count == 0) {\n        return flag ? apply_encoded(dst, value) : copy(value, dst);\n    }\n\n    size_t limit =\n        r.threshold < 0 ? SIZE_MAX : as_unsigned(r.threshold);\n}\n",
    );
}

#[test]
fn logical_chain_alignment_is_not_disturbed_by_nested_paren_operators() {
    // A wrapped paren that itself contains a logical operator must not shift the
    // outer chain's continuation column; rows after the paren keep aligning under
    // the first operand.
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "int f(void)",
            "{",
            "    return a(x) == 1 &&",
            "           (h <= v ||",
            "            d(c) == 1) &&",
            "           e(z) == 1;",
            "}",
        ),
        &options,
    );
    assert_eq!(
        actual,
        fixture!(
            "int f(void)",
            "{",
            "    return a(x) == 1 &&",
            "           (h <= v ||",
            "            d(c) == 1) &&",
            "           e(z) == 1;",
            "}",
        )
    );
}
#[test]
fn aligns_case_body_call_continuations_after_open_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_switches = true;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;
    let actual = format_with(
        fixture!(
            "void f(int type){switch(type){case A:",
            "if(value.len==0){",
            "write_error(LOG_ERROR, cfg, 0,",
            "\"empty server value \\\"%V\\\"\", &server->name);",
            "return ERROR;}}}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int type)",
            "{",
            "    switch (type) {",
            "        case A:",
            "            if (value.len == 0) {",
            "                write_error(LOG_ERROR, cfg, 0,",
            "                            \"empty server value \\\"%V\\\"\", &server->name);",
            "                return ERROR;",
            "            }",
            "    }",
            "}",
        )
    );
}

#[test]
fn macro_like_continuation_arg_in_switch_keeps_paren_alignment() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tswitch (x) {\n\tdefault:\n\t\treport(\"msg %d\\n\",\n\t\t\tITEM_TYPE(item));\n\t\tbreak;\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    switch (x) {\n    default:\n        report(\"msg %d\\n\",\n               ITEM_TYPE(item));\n        break;\n    }\n}\n",
    );
}

#[test]
fn indents_assignment_and_operator_continuations() {
    let actual = format(fixture!("int f(){int x=", "1+", "2;return x;}",));
    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    int x =",
            "        1 +",
            "        2;",
            "    return x;",
            "}",
        )
    );
}
#[test]
fn indents_assignment_operator_leading_continuation_line() {
    assert_eq!(
        format(fixture!(
            "void run(void){",
            "alpha_value",
            "= beta_value + gamma();",
            "}",
        )),
        fixture!(
            "void run(void)",
            "{",
            "    alpha_value",
            "        = beta_value + gamma();",
            "}",
        )
    );

    assert_eq!(
        format(fixture!(
            "void run(void){",
            "alpha_value",
            "= beta_value",
            "+ gamma()",
            "+ delta();",
            "}",
        )),
        fixture!(
            "void run(void)",
            "{",
            "    alpha_value",
            "        = beta_value",
            "          + gamma()",
            "          + delta();",
            "}",
        )
    );
}
#[test]
fn aligns_assignment_continuation_after_rhs_start() {
    let actual = format(fixture!(
        "void f(){",
        "size_t len = upstream->u.host.len + (bracket_host ? 2 : 0) +",
        "(include_port ? 1 + port_len : 0);",
        "}",
    ));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    size_t len = upstream->u.host.len + (bracket_host ? 2 : 0) +",
            "                 (include_port ? 1 + port_len : 0);",
            "}",
        )
    );
}
#[test]
fn indents_comma_and_paren_continuations() {
    let actual = format(fixture!("int f(){return sum(a,", "b,", "c);}",));
    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    return sum(a,",
            "               b,",
            "               c);",
            "}",
        )
    );
}

#[test]
fn statement_level_comma_operator_continuation_uses_statement_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c("void f(void)\n{\n\tpos = a,\n\tnext = b;\n}\n", &options,),
        "void f(void)\n{\n    pos = a,\n    next = b;\n}\n",
    );
}

#[test]
fn keeps_existing_call_argument_continuation_alignment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_commas = true;
    options.max_continuation_indent = 80;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "if (bad) {",
            "REPORT_ERROR(\"failed %d: %s\\n\",",
            "             err, message(err));",
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
            "    if (bad) {",
            "        REPORT_ERROR(\"failed %d: %s\\n\",",
            "                     err, message(err));",
            "    }",
            "}",
        )
    );
}

#[test]
fn keeps_macro_call_after_return_operator_continuation_aligned() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.max_continuation_indent = 80;
    let actual = format_with(
        fixture!(
            "value f(void){",
            "return FROM_SEC(sec) +",
            "       TO_VALUE(ns);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "value f(void)",
            "{",
            "    return FROM_SEC(sec) +",
            "           TO_VALUE(ns);",
            "}",
        )
    );
}

#[test]
fn indents_logical_leading_operator_and_return_call() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let actual = format_with(
        fixture!(
            "int f(void){",
            "if (build(alpha) != OK ||",
            "    finish(beta) != OK ||",
            "    size != expected) {",
            "return call(",
            "       alpha,",
            "       beta);",
            "}",
            "if (append(alpha,",
            "           beta)",
            "    != OK) {",
            "return ERROR;",
            "}",
            "if (ready(alpha) && ready(beta)",
            "#if USE_GAMMA",
            "    && ready(gamma)",
            "#endif",
            "    && ready(delta)) {",
            "return OK;",
            "}",
            "return OK;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(void)",
            "{",
            "    if (build(alpha) != OK ||",
            "        finish(beta) != OK ||",
            "        size != expected) {",
            "        return call(",
            "                   alpha,",
            "                   beta);",
            "    }",
            "    if (append(alpha,",
            "               beta)",
            "        != OK) {",
            "        return ERROR;",
            "    }",
            "    if (ready(alpha) && ready(beta)",
            "#if USE_GAMMA",
            "        && ready(gamma)",
            "#endif",
            "        && ready(delta)) {",
            "        return OK;",
            "    }",
            "    return OK;",
            "}",
        )
    );
}
#[test]
fn preserves_assignment_and_return_ternary_breaks_with_reference_continuations() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let actual = format_with(
        fixture!(
            "int f(int value){",
            "int result = value > 0",
            "             ? VALUE_YES",
            "             : VALUE_NO;",
            "return check(value)",
            "       ? VALUE_YES",
            "       : VALUE_NO;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(int value)",
            "{",
            "    int result = value > 0",
            "                 ? VALUE_YES",
            "                 : VALUE_NO;",
            "    return check(value)",
            "           ? VALUE_YES",
            "           : VALUE_NO;",
            "}",
        )
    );
}
#[test]
fn designated_initializer_arg_value_does_not_overindent_following_arg() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    let actual = format_c(
        fixture!(
            "TEST(test_name,",
            "     .setup_func    = setup,",
            "     .teardown_func = teardown)",
            "{",
            "    body();",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "TEST(test_name,",
            "     .setup_func    = setup,",
            "     .teardown_func = teardown)",
            "{",
            "    body();",
            "}",
        )
    );
}
#[test]
fn aligns_assignment_call_arguments_under_open_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "int rc = helper(",
            "           alpha, beta,",
            "           \"text\");",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    int rc = helper(",
            "                 alpha, beta,",
            "                 \"text\");",
            "}",
        )
    );
}

#[test]
fn call_argument_continuation_accounts_for_source_space_after_open_paren() {
    let source = "void f() {\n    CALL( a1, b,\n          c );\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn keeps_multiline_function_parameters_at_source_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "static int helper_with_a_long_name(",
            "    alpha_t *alpha,",
            "    beta_t *beta)",
            "{",
            "return 0;",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static int helper_with_a_long_name(",
            "    alpha_t *alpha,",
            "    beta_t *beta)",
            "{",
            "    return 0;",
            "}",
        )
    );
}
#[test]
fn aligns_multiline_condition_call_arguments_inside_case_body() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.indent_switches = true;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    options.max_continuation_indent = 80;
    let actual = format_with(
        fixture!(
            "void f(int type){",
            "switch(type){",
            "case A:",
            "if(check(",
            "first, length,",
            "VALUE, status) != OK){",
            "return;",
            "}",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int type)",
            "{",
            "    switch (type) {",
            "        case A:",
            "            if (check(",
            "                    first, length,",
            "                    VALUE, status) != OK) {",
            "                return;",
            "            }",
            "    }",
            "}",
        )
    );
}
#[test]
fn nested_trailing_paren_continuation_indents_one_step_past_outer_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    error = wrap(inner_call(",
            "handle,",
            "GET));",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    error = wrap(inner_call(",
            "                     handle,",
            "                     GET));",
            "}",
        )
    );
}
#[test]
fn deep_trailing_paren_continuation_keeps_increment_above_max_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void)",
            "{",
            "    if (a == b && c == d) {",
            "        error = errno_from_libusb_error(libusb_control_transfer(",
            "handle,",
            "GET));",
            "    }",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    if (a == b && c == d) {",
            "        error = errno_from_libusb_error(libusb_control_transfer(",
            "                                            handle,",
            "                                            GET));",
            "    }",
            "}",
        )
    );
}
#[test]
fn deep_assignment_continuation_aligns_to_value_above_max_indent() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.indent_switches = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(struct ctx *r, struct c context)",
            "{",
            "{",
            "{",
            "switch (context.x) {",
            "case 0:",
            "switch (context.z) {",
            "case 1:",
            "switch (context.q) {",
            "case 1:",
            "r->F = alpha(beta) | gamma(delta)",
            "| epsilon(zeta);",
            "}",
            "}",
            "}",
            "}",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(struct ctx *r, struct c context) {",
            "    {",
            "        {",
            "            switch (context.x) {",
            "                case 0:",
            "                    switch (context.z) {",
            "                        case 1:",
            "                            switch (context.q) {",
            "                                case 1:",
            "                                    r->F = alpha(beta) | gamma(delta)",
            "                                           | epsilon(zeta);",
            "                            }",
            "                    }",
            "            }",
            "        }",
            "    }",
            "}",
        )
    );
}
#[test]
fn keeps_logical_condition_chain_indent_after_first_continuation() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    options.pad_commas = true;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let actual = format_with(
        fixture!(
            "void f(int a, int b, int c){",
            "if(a ||",
            "b ||",
            "c){",
            "call();",
            "}",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(int a, int b, int c)",
            "{",
            "    if (a ||",
            "        b ||",
            "        c) {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn logical_condition_sibling_after_or_keeps_previous_sibling_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    if (desc.align == 0 &&",
                "        (desc.flags & FLAG) == 0 &&",
                "        (desc.kind == ONE ||",
                "            desc.kind == TWO ||",
                "            desc.kind == THREE) &&",
                "        (desc.flags & MASK) == 0",
                "        && can_use(desc))",
                "        call();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    if (desc.align == 0 &&",
            "            (desc.flags & FLAG) == 0 &&",
            "            (desc.kind == ONE ||",
            "             desc.kind == TWO ||",
            "             desc.kind == THREE) &&",
            "            (desc.flags & MASK) == 0",
            "            && can_use(desc))",
            "        call();",
            "}",
        )
    );
}

#[test]
fn aligns_return_binary_operator_continuations_after_return_keyword() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "uint32_t f(const u_char *p){",
            "return ((uint32_t)p[0] << 24) |",
            "((uint32_t)p[1] << 16) |",
            "((uint32_t)p[2] << 8) |",
            "p[3];",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "uint32_t f(const u_char *p)",
            "{",
            "    return ((uint32_t)p[0] << 24) |",
            "           ((uint32_t)p[1] << 16) |",
            "           ((uint32_t)p[2] << 8) |",
            "           p[3];",
            "}",
        )
    );
}

#[test]
fn return_operator_continuation_keeps_alignment_across_multiple_lines() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c("int f()\n{\n\treturn a\n\t+ b\n\t+ c;\n}\n", &options,),
        "int f()\n{\n    return a\n           + b\n           + c;\n}\n",
    );
}

#[test]
fn return_trailing_logical_chain_keeps_sibling_alignment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!("bool run(){", "return alpha&&", "beta&&", "gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "bool run()",
            "{",
            "    return alpha&&",
            "           beta&&",
            "           gamma;",
            "}",
        )
    );
}

// Interior source whitespace does not change the semantic continuation level.
#[test]
fn return_logical_continuation_after_tab_keeps_single_continuation_level() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "int f(struct s *p)\n{\n\treturn\tp->type == MAX ||\n\t\tg(p) == p->id;\n}\n",
            &options,
        ),
        "int f(struct s *p)\n{\n    return\tp->type == MAX ||\n        g(p) == p->id;\n}\n",
    );
}

#[test]
fn return_trailing_comment_chain_keeps_sibling_alignment() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "int run(){",
        "return alpha+ // first",
        "beta+ /* second */",
        "gamma;",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "int run()",
            "{",
            "    return alpha+ // first",
            "           beta+ /* second */",
            "           gamma;",
            "}",
        )
    );
}

#[test]
fn min_conditional_indent_controls_if_continuations() {
    let source = fixture!("int f(){if(a &&", "b){return 1;}}");

    let mut options = FormatOptions::default();
    options.min_conditional_indent = MinConditionalIndent::Zero;
    assert_eq!(
        format_with(source, &options),
        fixture!(
            "int f()",
            "{",
            "    if (a &&",
            "        b)",
            "    {",
            "        return 1;",
            "    }",
            "}",
        )
    );

    options.min_conditional_indent = MinConditionalIndent::Two;
    assert_eq!(
        format_with(source, &options),
        fixture!(
            "int f()",
            "{",
            "    if (a &&",
            "            b)",
            "    {",
            "        return 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn six_space_indent_moves_conditional_columns() {
    let mut options = FormatOptions::default();
    options.indent_width = 6;

    assert_eq!(
        format_c(
            "\nvoid foo(bool ready)\n{\n    if (ready\n            && valid)\n    {\n",
            &options,
        ),
        "\nvoid foo(bool ready)\n{\n      if (ready\n                  && valid)\n      {\n",
    );
}

#[test]
fn tab_indent_uses_tab_prefix_for_conditional_rows() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::Tabs;

    assert_eq!(
        format_c(
            "\nvoid foo(bool ready)\n{\n    if (ready\n            && valid)\n    {\n",
            &options,
        ),
        "\nvoid foo(bool ready)\n{\n\tif (ready\n\t        && valid)\n\t{\n",
    );
}

#[test]
fn six_column_tab_indent_uses_visual_conditional_columns() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 6;
    options.tab_width = 6;

    assert_eq!(
        format_c(
            "\nvoid foo(bool ready)\n{\n    if (ready\n            && valid)\n    {\n",
            &options,
        ),
        "\nvoid foo(bool ready)\n{\n\tif (ready\n\t            && valid)\n\t{\n",
    );
}

#[test]
fn forced_tab_indent_uses_tabs_for_conditional_columns() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::ForceTabs;

    assert_eq!(
        format_c(
            "\nvoid foo(bool ready)\n{\n    if (ready\n            && valid)\n    {\n",
            &options,
        ),
        "\nvoid foo(bool ready)\n{\n\tif (ready\n\t\t\t&& valid)\n\t{\n",
    );
}

#[test]
fn min_conditional_indent_does_not_move_return_ternary_continuations() {
    let source = fixture!("int f(int x){return x?", "1:", "0;}");
    let cases = [
        (
            MinConditionalIndent::Zero,
            fixture!(
                "int f(int x)",
                "{",
                "    return x ?",
                "           1 :",
                "           0;",
                "}"
            ),
        ),
        (
            MinConditionalIndent::One,
            fixture!(
                "int f(int x)",
                "{",
                "    return x ?",
                "           1 :",
                "           0;",
                "}"
            ),
        ),
        (
            MinConditionalIndent::Two,
            fixture!(
                "int f(int x)",
                "{",
                "    return x ?",
                "           1 :",
                "           0;",
                "}"
            ),
        ),
        (
            MinConditionalIndent::OneHalf,
            fixture!(
                "int f(int x)",
                "{",
                "    return x ?",
                "           1 :",
                "           0;",
                "}"
            ),
        ),
    ];

    for (min_indent, expected) in cases {
        let mut options = FormatOptions::default();
        options.continuation_indent = 0;
        options.min_conditional_indent = min_indent;
        assert_eq!(format_with(source, &options), expected);
    }
}
#[test]
fn indent_after_parens_uses_continuation_indent_for_calls() {
    let mut options = FormatOptions::default();
    options.indent_after_parens = true;
    let actual = format_with(fixture!("int f(){return sum(a,", "b);}",), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    return sum(a,", "            b);", "}",)
    );
}
#[test]
fn indent_after_parens_uses_continuation_indent_for_conditions() {
    let mut options = FormatOptions::default();
    options.indent_after_parens = true;
    let actual = format_with(
        fixture!("int f(){if(check(a,", "b)){return 1;}}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (check(a,",
            "            b))",
            "    {",
            "        return 1;",
            "    }",
            "}",
        )
    );
}
#[test]
fn indent_after_parens_keeps_plain_call_arguments_on_owner_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    let source = fixture!("void run(){", "result=call(alpha,", "beta,", "gamma);", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=call(alpha,",
            "            beta,",
            "            gamma);",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_restores_outer_call_argument_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    let source = fixture!(
        "void run(){",
        "result=outer(alpha,inner(beta,",
        "gamma),",
        "delta);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=outer(alpha,inner(beta,",
            "                gamma),",
            "            delta);",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_applies_level_to_nested_long_call() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 2;
    let source = fixture!(
        "void run(){",
        "result=namespace_name::very_long_function_name(alpha,helper(beta,",
        "gamma),",
        "delta);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=namespace_name::very_long_function_name(alpha,helper(beta,",
            "                            gamma),",
            "                    delta);",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_clamps_nested_level_at_maximum() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 4;
    let source = fixture!(
        "void run(){",
        "result=outer(alpha,inner(beta,",
        "gamma),",
        "delta);",
        "}",
    );

    // The configured continuation maximum is independent of opener position.
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=outer(alpha,inner(beta,",
            "                                            gamma),",
            "                                    delta);",
            "}",
        )
    );
}

#[test]
fn gnu_member_chain_uses_assignment_anchor() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let source = fixture!(
        "void run(){",
        "result=builder.alpha()",
        ".beta()",
        ".gamma();",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=builder.alpha()",
            "           .beta()",
            "           .gamma();",
            "}",
        )
    );
}

#[test]
fn gnu_leading_logical_return_chain_uses_return_anchor() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let source = fixture!("bool run(){", "return alpha", "&&beta", "&&gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "bool run()",
            "{",
            "    return alpha",
            "           &&beta",
            "           &&gamma;",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_uses_configured_level_for_leading_return_chain() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 3;
    let source = fixture!("bool run(){", "return alpha", "&&beta", "&&gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "bool run()",
            "{",
            "    return alpha",
            "                &&beta",
            "                &&gamma;",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_uses_configured_level_for_member_chain() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 3;
    let source = fixture!(
        "void run(){",
        "result=builder.alpha()",
        ".beta()",
        ".gamma();",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=builder.alpha()",
            "                .beta()",
            "                .gamma();",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_uses_continuation_level_for_ternary_arms() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    let source = fixture!("int run(){", "return ready?", "alpha:", "beta;", "}");

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "int run()",
            "{",
            "    return ready?",
            "        alpha:",
            "        beta;",
            "}",
        )
    );
}

#[test]
fn indent_after_parens_applies_configured_level_to_function_parameters() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_after_parens = true;
    options.continuation_indent = 2;
    let source = fixture!("void operation(int alpha,", "int beta,", "int gamma);",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void operation(int alpha,",
            "        int beta,",
            "        int gamma);",
        )
    );
}

#[test]
fn zero_continuation_indent_keeps_trailing_open_function_parameters_at_base() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.continuation_indent = 0;
    let source = fixture!("void operation(", "int alpha,", "int beta);");

    assert_eq!(
        format_c(source, &options),
        fixture!("void operation(", "int alpha,", "int beta);")
    );
}

#[test]
fn macro_call_continuation_aligns_under_first_argument() {
    assert_eq!(
        format_c(
            "\nvoid foo(void)\n{\n    CHECK_VALUE(test, compare(name, \"UNKNOWN\"), 0,\n\"ID\", value);\n}\n",
            &FormatOptions::default(),
        ),
        "\nvoid foo(void)\n{\n    CHECK_VALUE(test, compare(name, \"UNKNOWN\"), 0,\n                \"ID\", value);\n}\n",
    );
}

#[test]
fn table_macro_rows_do_not_stair_step_after_comma() {
    assert_eq!(
        format_c(
            "\nTABLE_START(SIMULATION, \"item\")\n\t.compat\t= item_compat,\n\t.init     = item_init,\nTABLE_END\n",
            &FormatOptions::default(),
        ),
        "\nTABLE_START(SIMULATION, \"item\")\n.compat\t= item_compat,\n .init     = item_init,\n TABLE_END\n",
    );
}

#[test]
fn semicolon_terminated_macro_statement_formats_like_normal_statement() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int f(void)\n{\n\tDO_CHECK( inner(value) , \"\");\n}\n",
            &options,
        ),
        "int f(void)\n{\n    DO_CHECK( inner(value) , \"\");\n}\n",
    );
    assert_eq!(
        format_c("int f(void)\n{\n\t\t\tDO_CHECK(value);\n}\n", &options),
        "int f(void)\n{\n    DO_CHECK(value);\n}\n",
    );
}

#[test]
fn file_scope_standalone_macro_rows_normalize_to_structural_indent() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "#define C(n, b) X(n, b)\n  C(a, 0)\n  C(b, 1)\n#undef C\n",
            &options,
        ),
        "#define C(n, b) X(n, b)\nC(a, 0)\nC(b, 1)\n#undef C\n",
    );
}

#[test]
fn assignment_continuation_aligns_to_rhs_column_when_paren_align_exceeds_max() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tstruct config_item_type *item = container_helper(first_argument, second_argument,\n\t\tnext);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    struct config_item_type *item = container_helper(first_argument, second_argument,\n                                    next);\n}\n",
    );
}

#[test]
fn chained_assignment_overflow_aligns_to_last_rhs() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tget_count = put_count = buffer_out_items(&queue->item_fifo, NULL,\nQUEUE_ITEM_MAX);\n}\n",
            &options,
        ),
        "void f(void)\n{\n    get_count = put_count = buffer_out_items(&queue->item_fifo, NULL,\n                            QUEUE_ITEM_MAX);\n}\n",
    );
}

#[test]
fn chained_assignment_continuation_over_max_caps_to_two_levels() {
    assert_eq!(
        format_c(
            "void f() {\n    if (x) {\n        c->m_on[0] = c->m_on[1] = c->m_on[2] = c->m_on[3] =\n        c->m_on[4] = c->m_on[5] = c->m_on[6] = c->m_on[7] = 0;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void f() {\n    if (x) {\n        c->m_on[0] = c->m_on[1] = c->m_on[2] = c->m_on[3] =\n                c->m_on[4] = c->m_on[5] = c->m_on[6] = c->m_on[7] = 0;\n    }\n}\n",
    );
}

#[test]
fn comma_separated_pointer_assignments_align_after_leading_star() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  *out_width = width,\n  *out_height = height;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    *out_width = width,\n     *out_height = height;\n}\n",
    );
}

// A member named `template` does not change assignment-continuation ownership.
#[test]
fn assignment_continuation_indents_with_template_member() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  obj->priv->template->children =\n    g_slist_prepend (obj->priv->template->children, item);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    obj->priv->template->children =\n        g_slist_prepend (obj->priv->template->children, item);\n}\n",
    );
}

#[test]
fn nested_paren_overflow_continuation_aligns_to_enclosing_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "int helper(int flags)\n{\n\treturn outer_call(flags, inner_function_longname(name, first, second, flags,\ncontext_value, make_value));\n}\n",
            &options,
        ),
        "int helper(int flags)\n{\n    return outer_call(flags, inner_function_longname(name, first, second, flags,\n                      context_value, make_value));\n}\n",
    );
}

#[test]
fn nested_paren_overflow_in_if_aligns_to_middle_fitting_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void helper(void)\n{\n\tif (!entry && (!is_map(root) || node_tag_get(root, node, MAP_FREE,\nget_slot_offset(node, slot))))\n\t\treturn;\n}\n",
            &options,
        ),
        "void helper(void)\n{\n    if (!entry && (!is_map(root) || node_tag_get(root, node, MAP_FREE,\n                   get_slot_offset(node, slot))))\n        return;\n}\n",
    );
}

#[test]
fn return_logical_chain_with_call_keeps_operand_alignment() {
    let mut options = FormatOptions::default();
    let args = ["--style=linux", "--mode=c"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c(
            "int f(void)\n{\n\treturn has_feature(A) &&\n\t\thas_feature(B) &&\n\t\thas_feature(C);\n}\n",
            &options,
        ),
        "int f(void)\n{\n    return has_feature(A) &&\n           has_feature(B) &&\n           has_feature(C);\n}\n",
    );
}

#[test]
fn return_logical_chain_keeps_alignment_after_multiline_call() {
    assert_eq!(
        format_c(
            "bool f()\n{\n    return ReadSource(input)\n        && SaveOutput(input, stream,\n            mode, map)\n        && WriteDone(stream);\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n    return ReadSource(input)\n           && SaveOutput(input, stream,\n                         mode, map)\n           && WriteDone(stream);\n}\n",
    );
}

#[test]
fn conditional_first_continuation_uses_open_paren_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let source = fixture!(
        "void run(){",
        "if(alpha&&",
        "beta&&",
        "gamma){",
        "call();",
        "}",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    if(alpha&&",
            "       beta&&",
            "       gamma)",
            "    {",
            "        call();",
            "    }",
            "}",
        )
    );
}

#[test]
fn linux_condition_continuation_overflow_uses_two_levels() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tif ((chan->chan_id == param->chan_id) && (param->dma_dev ==",
                "\t\tchan->device->dev)) {",
                "\t\tx = 1;",
                "\t}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    if ((chan->chan_id == param->chan_id) && (param->dma_dev ==",
            "            chan->device->dev)) {",
            "        x = 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn linux_logical_condition_tail_after_closed_inner_paren_uses_minimum_indent() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tif ((mode == A ||",
                "\t     mode == B ||",
                "\t     mode == C) &&",
                "\t     value != D)",
                "\t\treturn;",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    if ((mode == A ||",
            "         mode == B ||",
            "         mode == C) &&",
            "        value != D)",
            "        return;",
            "}",
        )
    );
}

#[test]
fn nested_logical_operand_after_or_keeps_inner_operand_column() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tdo {",
                "\t\tcall();",
                "\t} while (!done &&",
                "\t\t !ready() &&",
                "\t\t ((flags & MASK) ||",
                "\t\t  (!empty(queue) &&",
                "\t\t   !stopped(port))));",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    do {",
            "        call();",
            "    } while (!done &&",
            "             !ready() &&",
            "             ((flags & MASK) ||",
            "              (!empty(queue) &&",
            "               !stopped(port))));",
            "}",
        )
    );
}

#[test]
fn return_nested_logical_operand_after_or_keeps_inner_operand_column() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "int f(void)",
                "{",
                "\treturn ((alpha) ||",
                "\t\t(!beta &&",
                "\t\t gamma));",
                "}",
            ),
            &options,
        ),
        fixture!(
            "int f(void)",
            "{",
            "    return ((alpha) ||",
            "            (!beta &&",
            "             gamma));",
            "}",
        )
    );
}

#[test]
fn logical_chain_indent_does_not_leak_after_multiline_header_block() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            fixture!(
                "void f(void)",
                "{",
                "\tdo {",
                "\t\tif (alpha) {",
                "\t\t} else if (!stopped(port) &&",
                "\t\t\t   get(port, &ch)) {",
                "\t\t\tcall();",
                "\t\t}",
                "\t} while (!done &&",
                "\t\t !ready() &&",
                "\t\t ((flags & MASK) ||",
                "\t\t  (!empty(queue) &&",
                "\t\t   !stopped(port))));",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f(void)",
            "{",
            "    do {",
            "        if (alpha) {",
            "        } else if (!stopped(port) &&",
            "                   get(port, &ch)) {",
            "            call();",
            "        }",
            "    } while (!done &&",
            "             !ready() &&",
            "             ((flags & MASK) ||",
            "              (!empty(queue) &&",
            "               !stopped(port))));",
            "}",
        )
    );
}

#[test]
fn logical_chain_indent_does_not_leak_from_outer_header_into_inner_if() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f()\n{\n\tif (x) {\n\t} else if (alpha(MASK_A | MASK_B,\n\t\t\t   NULL) &&\n\t\t   beta(FEATURE_C)) {\n\t\tif (one(FEATURE_D) &&\n\t\t    two(FEATURE_E) &&\n\t\t    three(FEATURE_F))\n\t\t\tcall(x);\n\t}\n}\n",
            &options,
        ),
        "void f()\n{\n    if (x) {\n    } else if (alpha(MASK_A | MASK_B,\n                     NULL) &&\n               beta(FEATURE_C)) {\n        if (one(FEATURE_D) &&\n            two(FEATURE_E) &&\n            three(FEATURE_F))\n            call(x);\n    }\n}\n",
    );
}

#[test]
fn over_max_call_argument_uses_two_level_fallback() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "void run(){",
        "result=namespace_name::very_long_function_name(alpha,",
        "beta,",
        "gamma);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=namespace_name::very_long_function_name(alpha,",
            "            beta,",
            "            gamma);",
            "}",
        )
    );
}

#[test]
fn nested_call_close_restores_outer_delimiter_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    let source = fixture!(
        "void run(){",
        "result=outer(",
        "inner(",
        "alpha,",
        "beta),",
        "gamma);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    result=outer(",
            "               inner(",
            "                   alpha,",
            "                   beta),",
            "               gamma);",
            "}",
        )
    );
}

#[test]
fn member_access_after_closed_inner_calls_aligns_to_enclosing_macro_argument() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let source = "void t()\n{\n    CHECK(outer(ctx,\n                build(\n                    0x10U,\n                    conv(reinterpret_cast<const uint8_t *>(data())\n                         + off(Header, field))))\n                         .valid);\n}\n";

    assert_eq!(
        format_c(source, &options),
        "void t()\n{\n    CHECK(outer(ctx,\n                build(\n                    0x10U,\n                    conv(reinterpret_cast<const uint8_t *>(data())\n                         + off(Header, field))))\n          .valid);\n}\n"
    );
}

#[test]
fn closing_paren_after_brace_block_argument_aligns_to_statement() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    let source = fixture!(
        "void f()",
        "{",
        "    auto *x = new Thing(",
        "        a,",
        "    [] {",
        "        return 1;",
        "    },",
        "    nullptr",
        "                             );",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void f()",
            "{",
            "    auto *x = new Thing(",
            "        a,",
            "    [] {",
            "        return 1;",
            "    },",
            "    nullptr",
            "    );",
            "}",
        )
    );
}

#[test]
fn trailing_call_argument_after_brace_block_aligns_to_sibling_column() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.min_conditional_indent = MinConditionalIndent::Zero;
    options.break_after_logical = true;
    let source = fixture!(
        "void f()",
        "{",
        "    enqueueRequest(CMD,",
        "    [](int s) {",
        "        return build(s);",
        "    },",
        "    std::move(cb),",
        "        options);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void f()",
            "{",
            "    enqueueRequest(CMD,",
            "    [](int s) {",
            "        return build(s);",
            "    },",
            "    std::move(cb),",
            "    options);",
            "}",
        )
    );
}

#[test]
fn call_argument_comment_uses_configured_continuation_owner() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Allman;
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 8;
    options.tab_width = 8;
    options.indent_after_parens = true;
    options.continuation_indent = 2;
    let source = fixture!(
        "void run(){",
        "result=call(alpha,",
        "/* detail */",
        "beta);",
        "}",
    );

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "\tresult=call(alpha,",
            "\t                                /* detail */",
            "\t                                beta);",
            "}",
        )
    );
}

#[test]
fn adjacent_string_assignment_over_max_aligns_to_rhs_value() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    auto value = VeryLongOuterFunctionNameHereAndMoreAndMore(\"base\"",
                "                                                             \"tail\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    auto value = VeryLongOuterFunctionNameHereAndMoreAndMore(\"base\"",
            "                 \"tail\");",
            "}",
        )
    );
}

#[test]
fn adjacent_string_assignment_nested_call_over_max_uses_inner_callee_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    longOperationContextValue = new Target(id(\"select option \"",
                "                                        \"for value\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    longOperationContextValue = new Target(id(\"select option \"",
            "                                           \"for value\"));",
            "}",
        )
    );
}

#[test]
fn adjacent_string_assignment_nested_call_over_max_uses_outer_call_when_inner_is_over_max() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    ItemType *item = new ItemType(ScopeName::transform(\"message \"",
                "                                                        \"tail\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    ItemType *item = new ItemType(ScopeName::transform(\"message \"",
            "                                  \"tail\"));",
            "}",
        )
    );
}

#[test]
fn adjacent_string_after_call_argument_uses_call_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    RECORD_CHECK(\"\", \"first line\"",
        "                 \"second line\", Continue);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn adjacent_string_after_multiple_call_strings_uses_outer_call_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    GenericContainer::apply(this, id(\"About\"), id(\"This example demonstrates the \"",
                "                       \"different features.\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    GenericContainer::apply(this, id(\"About\"), id(\"This example demonstrates the \"",
            "                            \"different features.\"));",
            "}",
        )
    );
}

#[test]
fn adjacent_string_after_operator_led_call_uses_operator_continuation_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    const Buffer description = ScopeObject::name()",
                "                               + PlainString(\"first line\\n\"",
                "                                        \"second line\\n\"",
                "                                        \"third line\\n\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    const Buffer description = ScopeObject::name()",
            "                               + PlainString(\"first line\\n\"",
            "                                       \"second line\\n\"",
            "                                       \"third line\\n\");",
            "}",
        )
    );
}

#[test]
fn adjacent_string_nested_call_over_max_aligns_to_outer_call() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    VeryLongOuterFunctionNameHere(PlainTextValue(\"base\"",
                "                                                \"tail\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    VeryLongOuterFunctionNameHere(PlainTextValue(\"base\"",
            "                                  \"tail\"));",
            "}",
        )
    );
}

#[test]
fn adjacent_string_simple_call_over_max_uses_two_level_fallback() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    VeryLongOuterFunctionNameHereAndMoreAndMore(\"base\"",
                "                                                \"tail\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    VeryLongOuterFunctionNameHereAndMoreAndMore(\"base\"",
            "            \"tail\");",
            "}",
        )
    );
}

#[test]
fn block_comment_before_adjacent_string_call_argument_uses_call_indent() {
    let source = fixture!(
        "void f(void){",
        "  value = call(",
        "    /* note */",
        "    \"alpha\"",
        "    \"beta\"",
        "    , other);",
        "  next();",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(void) {",
            "    value = call(",
            "                /* note */",
            "                \"alpha\"",
            "                \"beta\"",
            "                , other);",
            "    next();",
            "}",
        )
    );
}

#[test]
fn string_argument_continuation_in_logical_call_aligns_to_call_argument() {
    assert_eq!(
        format_c(
            "void f(void){\n  if( info(cx, a,b,c,d,e,f,g,h)\n      && 0==call(cx, \"generic content item value\"\n                 \" next value=%Q\",\n                 source, value)\n    ){\n    done();\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    if( info(cx, a,b,c,d,e,f,g,h)\n            && 0==call(cx, \"generic content item value\"\n                       \" next value=%Q\",\n                       source, value)\n      ) {\n        done();\n    }\n}\n",
    );
}

#[test]
fn call_argument_after_string_argument_aligns_to_call_argument() {
    assert_eq!(
        format_c(
            "void f(void){\n  if( source ){\n    apply(span, ctx, \"generic content item value\",\n      source, value);\n  }else{\n    apply(span, ctx, \"generic content item value?\", value);\n  }\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void) {\n    if( source ) {\n        apply(span, ctx, \"generic content item value\",\n              source, value);\n    } else {\n        apply(span, ctx, \"generic content item value?\", value);\n    }\n}\n",
    );
}

#[test]
fn statement_after_top_level_adjacent_string_call_keeps_block_indent() {
    let source = fixture!(
        "void f(void){",
        "  int *err;",
        "  exec(db,",
        "    \"alpha\"",
        "    \"beta\"",
        "    ,0,&err);",
        "  if( err ){",
        "    free(err);",
        "  }",
        "}",
    );

    assert_eq!(
        format_c(source, &FormatOptions::default()),
        fixture!(
            "void f(void) {",
            "    int *err;",
            "    exec(db,",
            "         \"alpha\"",
            "         \"beta\"",
            "         ,0,&err);",
            "    if( err ) {",
            "        free(err);",
            "    }",
            "}",
        )
    );
}

#[test]
fn statement_after_long_adjacent_string_call_keeps_block_indent() {
    let mut source = String::from("void f(void){\n  int *err;\n  exec(db,\n");
    let mut expected = String::from("void f(void) {\n    int *err;\n    exec(db,\n");

    for index in 0..20 {
        source.push_str(&format!("    \"item{index}\"\n"));
        expected.push_str(&format!("         \"item{index}\"\n"));
    }

    source.push_str("    ,0,&err);\n  if( err ){\n    free(err);\n  }\n}\n");
    expected.push_str("         ,0,&err);\n    if( err ) {\n        free(err);\n    }\n}\n");

    assert_eq!(format_c(&source, &FormatOptions::default()), expected);
}

#[test]
fn whitesmith_return_binary_continuation_aligns_after_keyword() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let source = fixture!("int run(){", "return alpha+", "beta+", "gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "int run()",
            "    {",
            "    return alpha+",
            "           beta+",
            "           gamma;",
            "    }",
        )
    );
}

#[test]
fn whitesmith_stream_continuation_aligns_after_first_operand() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Whitesmith;
    options.indent_braces = true;
    options.indent_classes = true;
    options.indent_switches = true;
    let source = fixture!("void run(){", "output<<alpha", "<<beta", "<<gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "    {",
            "    output<<alpha",
            "          <<beta",
            "          <<gamma;",
            "    }",
        )
    );
}

#[test]
fn case_body_stream_chain_keeps_previous_operator_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    switch (x)",
                "    {",
                "        case A:",
                "            str << GetInfo(a)",
                "                << ' '",
                "                << GetInfo(b);",
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
            "        str << GetInfo(a)",
            "            << ' '",
            "            << GetInfo(b);",
            "        break;",
            "    }",
            "}",
        )
    );
}

#[test]
fn stream_chain_after_parenthesized_ternary_keeps_operator_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    msg << \"The item is \" << (item->IsEnabled() ? \"enabled\"",
        "                              : \"disabled\")",
        "        << '\\n';",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_inside_parenthesized_argument_aligns_to_first_stream_operator() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"basic\") << \"pattern\" << true << (List()",
                "            << \"debug\"",
                "            << \"static\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"basic\") << \"pattern\" << true << (List()",
            "                           << \"debug\"",
            "                           << \"static\");",
            "}",
        )
    );
}

#[test]
fn stream_chain_inside_parenthesized_argument_uses_open_paren_when_within_limit() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"normal-crash\") << (StringList()",
                "            << \"first\"",
                "            << \"second\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"normal-crash\") << (StringList()",
            "                                      << \"first\"",
            "                                      << \"second\");",
            "}",
        )
    );
}

#[test]
fn stream_chain_inside_parenthesized_argument_keeps_comment_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"basic\") << \"pattern\" << true << (List()",
                "            << \"debug\"",
                "            // comment",
                "            << \"static\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"basic\") << \"pattern\" << true << (List()",
            "                           << \"debug\"",
            "                           // comment",
            "                           << \"static\");",
            "}",
        )
    );
}

#[test]
fn stream_chain_inside_continued_parenthesized_argument_aligns_after_open_paren() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"ifs\") << \"pattern\"",
                "                         << true << (List()",
                "            << \"debug\"",
                "            << \"static\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"ifs\") << \"pattern\"",
            "                         << true << (List()",
            "                                     << \"debug\"",
            "                                     << \"static\");",
            "}",
        )
    );
}

#[test]
fn stream_chain_inside_template_parenthesized_argument_uses_two_level_fallback() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"ifs-invalid1\") << \"pattern\"",
                "                                  << false << (Items<ByteBuffer>()",
                "            << \"debug\"",
                "            << \"static\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"ifs-invalid1\") << \"pattern\"",
            "                                  << false << (Items<ByteBuffer>()",
            "                                          << \"debug\"",
            "                                          << \"static\");",
            "}",
        )
    );
}

#[test]
fn stream_chain_inside_call_argument_aligns_to_parenthesized_list() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    REQUIRE(compareValues(spy, (ValueList() << Value::Running",
                "                                  << Value::Paused",
                "                                  << Value::Stopped)));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    REQUIRE(compareValues(spy, (ValueList() << Value::Running",
            "                                << Value::Paused",
            "                                << Value::Stopped)));",
            "}",
        )
    );
}

#[test]
fn over_max_stream_chain_uses_two_indent_levels() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"Path::Dirs | Path::NoDotAndDotDot\") << path << values",
                "        << int(Path::Dirs | Path::NoDotAndDotDot) << int(Path::Name)",
                "        << filter(TextBox(\"directory\"));",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"Path::Dirs | Path::NoDotAndDotDot\") << path << values",
            "            << int(Path::Dirs | Path::NoDotAndDotDot) << int(Path::Name)",
            "            << filter(TextBox(\"directory\"));",
            "}",
        )
    );
}

#[test]
fn over_max_assignment_call_argument_aligns_to_value_start() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    TextItemSet actual = Path(directoryPath).listItems(TextItemSet() << \"*.txt\", Path::NoFilter,",
                "                                                       Path::Time);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    TextItemSet actual = Path(directoryPath).listItems(TextItemSet() << \"*.txt\", Path::NoFilter,",
            "                         Path::Time);",
            "}",
        )
    );
}

#[test]
fn adjacent_string_in_stream_call_aligns_to_stream_operator() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"longfile\") << TextBox::fromSource(\"longFileName\"",
                "                                                    \"longFileName\"",
                "                                                    \"longFileName.txt\") << true;",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"longfile\") << TextBox::fromSource(\"longFileName\"",
            "                              \"longFileName\"",
            "                              \"longFileName.txt\") << true;",
            "}",
        )
    );
}

#[test]
fn stream_after_adjacent_string_keeps_string_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    Table::addRow(\"row\") << \"first\"",
        "                         \"second\"",
        "                         << value;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn string_after_trailing_stream_operator_aligns_after_operator() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"case\") <<",
                "                           \"first\"",
                "                           \"second\" << value;",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"case\") <<",
            "                          \"first\"",
            "                          \"second\" << value;",
            "}",
        )
    );
}

#[test]
fn string_after_indented_trailing_stream_operator_keeps_line_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    call(List() <<",
        "         value <<",
        "         \"first\"",
        "         \"second\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn string_after_indented_string_stream_operator_keeps_line_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    call(List() <<",
        "         \"path\" <<",
        "         \"first\"",
        "         \"second\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn adjacent_string_row_after_stream_string_keeps_chain_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    str << \"a\"",
        "        << call() << \"\\n\"",
        "        \"cd \" << call() << \"\\n\"",
        "        \"wget\";",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn adjacent_string_call_argument_before_stream_keeps_string_indent() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"utf16\")",
                "            << ByteArray(\"\\xfe\\xff\"",
                "                          \"\\x00\\xe5\", 4) << value;",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"utf16\")",
            "            << ByteArray(\"\\xfe\\xff\"",
            "                         \"\\x00\\xe5\", 4) << value;",
            "}",
        )
    );
}

#[test]
fn adjacent_string_after_over_max_stream_head_uses_two_level_fallback() {
    let source = fixture!(
        "void f()",
        "{",
        "    Table::addRow(\"one-interface-annotated\") << \"<node><interface name=\\\"iface.iface1\\\">\"",
        "            \"<annotation name=\\\"foo.testing\\\" value=\\\"nothing to see here\\\" />\"",
        "            \"</interface></node>\" << value;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn adjacent_string_after_multiple_streams_aligns_to_first_stream_operator() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"row\") << true << Bytes(\"base \"",
                "                                          \"tail\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"row\") << true << Bytes(\"base \"",
            "                         \"tail\");",
            "}",
        )
    );
}

#[test]
fn adjacent_string_in_line_start_stream_call_keeps_string_column() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"mix\")",
                "        << TextBox::fromText(\"base\"",
                "                             \"tail\")",
                "        << \"x\";",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"mix\")",
            "            << TextBox::fromText(\"base\"",
            "                                 \"tail\")",
            "            << \"x\";",
            "}",
        )
    );
}

#[test]
fn adjacent_string_in_line_start_stream_call_skips_preprocessor_lines() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow(\"x\")",
                "        << TextBox(\"entrylist/file,\"",
                "#ifndef X",
                "                   \"entrylist/linktofile.lnk,\"",
                "#endif",
                "                   \"entrylist/directory/dummy,\"",
                "                   \"entrylist/writable\").split(',');",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow(\"x\")",
            "            << TextBox(\"entrylist/file,\"",
            "#ifndef X",
            "                       \"entrylist/linktofile.lnk,\"",
            "#endif",
            "                       \"entrylist/directory/dummy,\"",
            "                       \"entrylist/writable\").split(',');",
            "}",
        )
    );
}

#[test]
fn adjacent_string_in_nested_stream_call_caps_to_statement_continuation() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    Table::addRow( \"244 chars to resolvedpath\" ) << FileValue(TextBox::fromSource(\"longFileNamelongFileNamelongFileNamelongFileName\"",
                "                                                     \"longFileNamelongFileNamelongFileNamelongFileName\"",
                "                                                     \"longFileNamelongFileNamelongFileNamelongFileName.txt\")).resolvedFilePath();",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    Table::addRow( \"244 chars to resolvedpath\" ) << FileValue(TextBox::fromSource(\"longFileNamelongFileNamelongFileNamelongFileName\"",
            "            \"longFileNamelongFileNamelongFileNamelongFileName\"",
            "            \"longFileNamelongFileNamelongFileNamelongFileName.txt\")).resolvedFilePath();",
            "}",
        )
    );
}

#[test]
fn call_argument_sibling_after_closed_call_keeps_previous_argument_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto report = scope_guard([=]() {",
        "        auto typeName = [](int type) {",
        "            return (type == 0 ? \"std\"",
        "                    : type == 1 ? \"gen\" : \"dst\");",
        "        };",
        "        qDebug(\"Long name round-tripped %s (%s) to %s (%s) via %s\",",
        "               zoneName.valueData(), typeName(timeType),",
        "               match.ianaId.valueData(), typeName(match.timeType),",
        "               localeName.encode().valueData());",
        "    });",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_call_after_stream_operator_aligns_to_stream_rhs() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    traceLog().details() << GenericNamespace::transform(",
                "                                 \"context\",",
                "                                 \"message\");",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    traceLog().details() << GenericNamespace::transform(",
            "                             \"context\",",
            "                             \"message\");",
            "}",
        )
    );
}

#[test]
fn stream_operator_adjacent_string_aligns_to_rhs() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    traceLog() << \"base \"",
                "                  \"tail\";",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    traceLog() << \"base \"",
            "               \"tail\";",
            "}",
        )
    );
}

#[test]
fn stream_operator_call_argument_strings_keep_argument_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    traceLog().details() << GenericNamespace::transform(",
        "                             \"ctx\",",
        "                             \"line1\\n\"",
        "                             \"line2\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_started_in_call_argument_keeps_argument_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    call(\"name\",",
        "         List() << \"first\" << value",
        "         << \"second\"",
        "         << \"third\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_line_with_nested_braced_value_keeps_source_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    append(\"row\")",
        "    << Value { { 1 } }",
        "            << expected;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_after_nested_braced_value_uses_two_indent_levels() {
    let source = fixture!(
        "void f()",
        "{",
        "    append(\"row\") << Value { { 1 } }",
        "            << expected;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braced_value_stream_continuation_after_stream_head_keeps_base_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    append(\"row\") << name",
        "    << List{",
        "        A,",
        "        B,",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braced_value_stream_continuation_after_indented_stream_head_keeps_base_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    append(\"row\")",
        "            << name",
        "    << List{",
        "        A,",
        "        B,",
        "    };",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn parenthesized_value_after_over_max_stream_head_uses_two_level_fallback() {
    let source = fixture!(
        "void f()",
        "{",
        "    Check::append(\"generic+stream+case+fallback\") << first << second <<",
        "            (A | B | C | D) << value;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn braced_stream_value_over_max_keeps_source_item_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    Check::append(\"generic stream\") << ValueVector{A, B,",
        "                                    C, D}",
        "                                    << value;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn member_chain_after_prior_return_statement_does_not_use_return_indent() {
    let source = fixture!(
        "class C {",
        "    List values()",
        "    {",
        "        return List() << a() << b()",
        "               << c();",
        "    }",
        "",
        "    void send()",
        "    {",
        "        object()",
        "        .call();",
        "    }",
        "};",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_braced_value_inside_stream_call_keeps_base_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    append(\"row\")",
        "            << wrap(",
        "    Value { 1, \"foo\", { { \"bar\", 2 } } })",
        "            << expected;",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn stream_chain_after_multiline_parenthesized_stream_returns_to_parenthesized_head() {
    assert_eq!(
        format_c(
            "void f()\n{\n    Check::append(\"generic ordered\") << Order::DescendingBy\n                                  << Order::ExactMatch\n                                  << (ValueVector()\n                                      << \"a0\"\n                                      << \"a1\"\n                                      << \"a2\"\n                                      << \"a3\"\n                                      << \"a4\"\n                                      << \"a5\"\n                                      << \"a6\"\n                                      << \"a7\"\n                                      << \"a8\"\n                                      << \"a9\"\n                                      << \"a10\"\n                                      << \"a11\"\n                                      << \"a12\")\n                                  << (ValueVector()\n                                      << \"x\");\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Check::append(\"generic ordered\") << Order::DescendingBy\n                                     << Order::ExactMatch\n                                     << (ValueVector()\n                                         << \"a0\"\n                                         << \"a1\"\n                                         << \"a2\"\n                                         << \"a3\"\n                                         << \"a4\"\n                                         << \"a5\"\n                                         << \"a6\"\n                                         << \"a7\"\n                                         << \"a8\"\n                                         << \"a9\"\n                                         << \"a10\"\n                                         << \"a11\"\n                                         << \"a12\")\n                                     << (ValueVector()\n                                         << \"x\");\n}\n",
    );
}

#[test]
fn stream_chain_after_closed_parenthesized_stream_uses_outer_chain_indent() {
    let source = fixture!(
        "void f()",
        "{",
        "    values << (List() << 1 << 2)",
        "           << (List() << first",
        "               << second",
        "               << third)",
        "           << empty();",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn nested_logical_call_argument_keeps_operator_ladder() {
    let source = fixture!(
        "void f()",
        "{",
        "    CHECK_TRUE(a < b",
        "               || (a == b",
        "                   && (c < d",
        "                       || (c == d",
        "                           && e < f))),",
        "               \"where\",",
        "               \"what\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn logical_condition_second_call_after_nested_call_tail_uses_operand_indent() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (result == 0 &&\n\t\t(copy_to_sink(a,\n\t\t\t      b, c) ||\n\t\t copy_to_sink(d,\n\t\t\t      e, f)))\n\t\tresult = -1;\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (result == 0 &&\n            (copy_to_sink(a,\n                          b, c) ||\n             copy_to_sink(d,\n                          e, f)))\n        result = -1;\n}\n",
    );
}

#[test]
fn parenthesized_logical_operands_do_not_stair_step_after_call_tail() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            "int f(void)\n{\n\tif ((ret = call(TYPE_A,\n\t\t\t&set_a)) ||\n\t    (ret = call(TYPE_B,\n\t\t\t&set_b)) ||\n\t    (ret = call(TYPE_C,\n\t\t\t&set_c)))\n\t\treturn ret;\n}\n",
            &options,
        ),
        "int f(void)\n{\n    if ((ret = call(TYPE_A,\n                    &set_a)) ||\n            (ret = call(TYPE_B,\n                        &set_b)) ||\n            (ret = call(TYPE_C,\n                        &set_c)))\n        return ret;\n}\n",
    );
}

#[test]
fn logical_and_operand_in_nested_or_group_keeps_group_content_column() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if ((aa == B &&\n       cc == d) ||\n      (ee == F &&\n       gg == h &&\n       ii == j))\n    go ();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if ((aa == B &&\n            cc == d) ||\n            (ee == F &&\n             gg == h &&\n             ii == j))\n        go ();\n}\n",
    );
}

#[test]
fn nested_logical_operand_inner_continuation_keeps_paren_column() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  show = (about->artists != NULL ||\n          about->credits != NULL ||\n          (about->translator != NULL &&\n           strcmp (about->translator, \"one\") &&\n           strcmp (about->translator, \"two\")));\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    show = (about->artists != NULL ||\n            about->credits != NULL ||\n            (about->translator != NULL &&\n             strcmp (about->translator, \"one\") &&\n             strcmp (about->translator, \"two\")));\n}\n",
    );
}

#[test]
fn ternary_call_argument_over_max_continuation_uses_ternary_arm_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    const auto selectedValue = cond()\n                              ? source\n                              : source.adjust(source.destinationDimensions().asSize(), Mode::PreserveRatio,\n                                             Mode::SmoothedAdjustment);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    const auto selectedValue = cond()\n                               ? source\n                               : source.adjust(source.destinationDimensions().asSize(), Mode::PreserveRatio,\n                                       Mode::SmoothedAdjustment);\n}\n",
    );
}

#[test]
fn call_argument_after_nested_member_call_uses_outer_call_column() {
    let source = fixture!(
        "void f()",
        "{",
        "    CHECK_EQ(run(pool.data(), &Obj::member, &obj,",
        "                 String(value)).results(),",
        "             List({String(value)}));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn assignment_call_argument_after_over_max_braced_list_aligns_to_rhs() {
    assert_eq!(
        format_c(
            "void f()\n{\n    auto result = RepresentativeFunctionForArgumentFallbackPath(&pool, std::vector { 1, 2, 3 },\n                                                                 arg);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    auto result = RepresentativeFunctionForArgumentFallbackPath(&pool, std::vector { 1, 2, 3 },\n                  arg);\n}\n",
    );
}

#[test]
fn call_argument_after_braced_list_arg_aligns_to_call_opener() {
    assert_eq!(
        format_c(
            fixture!(
                "void f()",
                "{",
                "    GenericOptionValue option({ \"i\"_id, \"indicator\"_id },",
                "                                      \"Show indicators\"_id);",
                "}",
            ),
            &FormatOptions::default(),
        ),
        fixture!(
            "void f()",
            "{",
            "    GenericOptionValue option({ \"i\"_id, \"indicator\"_id },",
            "                              \"Show indicators\"_id);",
            "}",
        )
    );
}

#[test]
fn stream_chain_continues_after_multiline_braced_argument() {
    assert_eq!(
        format_c(
            "void f()\n{\n    Check::append(\"row\")\n        << \"input\"\n        << ValueVector{u\"a\"_v, u\"bc\"_v, u\"d\"_v, u\"e\"_v, u\"\"_v, u\"g\"_v,\n                       u\"hi\"_v, u\"\"_v, u\"\"_v, u\"\"_v, u\"\"_v}\n        << \"output\";\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Check::append(\"row\")\n            << \"input\"\n            << ValueVector{u\"a\"_v, u\"bc\"_v, u\"d\"_v, u\"e\"_v, u\"\"_v, u\"g\"_v,\n                           u\"hi\"_v, u\"\"_v, u\"\"_v, u\"\"_v, u\"\"_v}\n            << \"output\";\n}\n",
    );
}

#[test]
fn nested_condition_comments_keep_continuation_indent() {
    assert_eq!(
        format_c(
            "size_t f(uint32_t cp) {\n  return to_unsigned(\n      1 + (cp >= 0x1100 &&\n           (cp <= 0x115f ||  // first\n            cp == 0x2329 ||  // second\n            cp == 0x232a ||  // third\n            // group:\n            (cp >= 1 && cp <= 2) ||\n            (cp >= 3 && cp <= 4) ||\n            // another\n            (cp >= 5 && cp <= 6)));\n}\n",
            &FormatOptions::default(),
        ),
        "size_t f(uint32_t cp) {\n    return to_unsigned(\n               1 + (cp >= 0x1100 &&\n                    (cp <= 0x115f ||  // first\n                     cp == 0x2329 ||  // second\n                     cp == 0x232a ||  // third\n                     // group:\n                     (cp >= 1 && cp <= 2) ||\n                     (cp >= 3 && cp <= 4) ||\n                     // another\n                     (cp >= 5 && cp <= 6)));\n}\n",
    );
}

#[test]
fn gnu_stream_continuation_aligns_after_first_operand() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Gnu;
    options.indent_blocks = true;
    let source = fixture!("void run(){", "output<<alpha", "<<beta", "<<gamma;", "}",);

    assert_eq!(
        format_c(source, &options),
        fixture!(
            "void run()",
            "{",
            "    output<<alpha",
            "          <<beta",
            "          <<gamma;",
            "}",
        )
    );
}

#[test]
fn honors_continuation_indent_option_with_maximum() {
    let mut options = FormatOptions::default();
    options.indent_after_parens = true;
    options.continuation_indent = 4;
    options.max_continuation_indent = 4;
    let actual = format_with(fixture!("int f(){return sum(a,", "b);}",), &options);

    assert_eq!(
        actual,
        fixture!("int f()", "{", "    return sum(a,", "            b);", "}",)
    );
}
#[test]
fn tab_indent_continuation_uses_tabs_then_spaces() {
    let mut options = FormatOptions::default();
    options.indent_style = IndentStyle::Tabs;
    options.indent_width = 4;
    let actual = format_c(
        fixture!("void f(void)", "{", "\tx = aaaa +", "\tbbbb;", "}",),
        &options,
    );

    assert_eq!(
        actual,
        fixture!("void f(void)", "{", "\tx = aaaa +", "\t    bbbb;", "}",)
    );
}

#[test]
fn tab_indent_uses_body_level_tabs_for_braceless_call_continuation() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");
    let source = fixture!(
        "void f()",
        "{",
        "\tif(a && b)",
        "\t\tcall(x,",
        "\t\t     y);",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn tab_indent_caps_assignment_call_arguments_at_max_continuation_indent() {
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
    let source = fixture!(
        "void f()",
        "{",
        "\twidget_table[CONST_INDEX].text_field = render_text_element(",
        "\t        alpha,",
        "\t        beta,",
        "\t        NULL);",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn tab_indent_keeps_assignment_call_alignment_within_continuation_cap() {
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
                "void f()",
                "{",
                "\tif (cond) {",
                "\t\talpha_map[index].field_value = create_item(",
                "\t\t        a,",
                "\t\t        b);",
                "\t}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tif(cond) {",
            "\t\talpha_map[index].field_value = create_item(",
            "\t\t                                   a,",
            "\t\t                                   b);",
            "\t}",
            "}",
        )
    );
}

#[test]
fn tab_indent_assignment_led_continuation_uses_spaces_after_structural_tab() {
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
        "int f(void)",
        "{",
        "\tvalue.member",
        "\t    = helper(a, b);",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn wrapped_argument_opening_block_brace_uses_structural_indent() {
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
        "void f()",
        "{",
        "\tif(x) {",
        "\t\tmap_int(",
        "\t\taaa, [](int n) {",
        "\t\t\treturn n;",
        "\t\t});",
        "\t}",
        "}",
    );

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn comma_led_call_argument_continuation_keeps_tab_split() {
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
    let source = "void f()\n{\n\tif(cond) {\n\t\tfprintf(stderr, \"long string here and more text to wrap\"\n\t\t        , aaa\n\t\t        , bbb);\n\t}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn leading_stream_operator_chain_off_call_uses_two_levels() {
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
    let source = "void f()\n{\n\tfor(const ITEMS item: items) {\n\t\tCHECK_EQ(item, state.values[i])\n\t\t        << \"Failed for dataset #\" << i\n\t\t        << \" origin: \" << linenumber;\n\t}\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn leading_logical_operator_chain_keeps_stable_alignment() {
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
    let source = "int f(void)\n{\n\treturn (result & FLAG_A) // comment\n\t       || (result == B)\n\t       || (result == C)\n\t       || (result == D);\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn macro_call_closing_paren_aligns_to_open_paren_column() {
    assert_eq!(
        format(
            "REGISTER_FEATURES_PAGE(ExampleWidgetPage, \"Sample\",\n                       MODULE_FLAGS | WITH_EXTRA_FLAGS\n                       );\n",
        ),
        "REGISTER_FEATURES_PAGE(ExampleWidgetPage, \"Sample\",\n                       MODULE_FLAGS | WITH_EXTRA_FLAGS\n                      );\n",
    );
}

#[test]
fn macro_call_operator_continuation_aligns_to_first_argument() {
    assert_eq!(
        format(
            "void f()\n{\n    CHECK_STATE( (state == A) || (state == B)\n        || (state == C),\n        TXT(\"msg\") );\n}\n",
        ),
        "void f()\n{\n    CHECK_STATE( (state == A) || (state == B)\n                 || (state == C),\n                 TXT(\"msg\") );\n}\n",
    );
}

#[test]
fn macro_call_comparison_continuation_aligns_to_first_argument() {
    let options = options_from_args(&["--mode=c"]);

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tASSERT(k && decode_word(it->raw_data)\n\t       <= decode_word(it[-1].raw_data));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    ASSERT(k && decode_word(it->raw_data)\n           <= decode_word(it[-1].raw_data));\n}\n",
    );
}

#[test]
fn macro_shaped_continuation_argument_keeps_paren_alignment() {
    let mut options = FormatOptions::default();
    let args = ["--style=kr", "--indent=tab", "--pad-comma", "--unpad-paren"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "void f()\n{\n\tcall(MODE, m,\n\t     name,\n\t     _(colors[i])\n\t    );\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn macro_call_inner_paren_continuation_aligns_to_inner_not_macro() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tCALL_SETUP(ctx, flags,\ncreate_context_file_mode(\"context\", &file_ops, state,\nflags, MODE_NONBLOCK));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    CALL_SETUP(ctx, flags,\n               create_context_file_mode(\"context\", &file_ops, state,\n                                        flags, MODE_NONBLOCK));\n}\n",
    );
}

#[test]
fn macro_arg_inner_paren_continuation_aligns_to_inner_paren() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tif (cond) {\n\t\tASSERT_STATE(counter_value_subtract(alpha->average_value_count,\nbeta->set->total_value_counter) < 0);\n\t}\n}\n",
            &options,
        ),
        "void f(void)\n{\n    if (cond) {\n        ASSERT_STATE(counter_value_subtract(alpha->average_value_count,\n                                            beta->set->total_value_counter) < 0);\n    }\n}\n",
    );
}

#[test]
fn nested_continuation_paren_alignment_is_capped_from_statement_base() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f() {\nouter_function_call(alpha, beta,\ninner_function_with_long_name(gamma,\ndelta));\n}\n",
            &options,
        ),
        "void f()\n{\n\touter_function_call(alpha, beta,\n\t                    inner_function_with_long_name(gamma,\n\t                            delta));\n}\n",
    );
}

#[test]
fn plain_shift_chain_continuations_align_under_first_operator() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=kr".to_owned(), "--indent=tab".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            fixture!("void f()", "{", "\tobj >> a", "\t>> b", "\t>> c;", "}",),
            &options,
        ),
        fixture!(
            "void f()",
            "{",
            "\tobj >> a",
            "\t    >> b",
            "\t    >> c;",
            "}",
        )
    );
}

#[test]
fn operator_led_return_chain_keeps_all_continuations_aligned() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "uint32_t f(uint32_t addr)",
            "{",
            "return peek(addr)",
            "| peek(addr + 1) << 8",
            "| peek(addr + 2) << 16;",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "uint32_t f(uint32_t addr)",
            "{",
            "    return peek(addr)",
            "           | peek(addr + 1) << 8",
            "           | peek(addr + 2) << 16;",
            "}"
        )
    );
}

#[test]
fn unbalanced_parens_do_not_leak_continuation_indent_into_next_block() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    let actual = format_with(
        fixture!(
            "static inline uint16_t f(uint16_t w) {",
            "    return ((w & UINT16_C(0xFF00) >> 8) | ((w & UINT16_C(0x00FF) << 8);",
            "}",
            "static inline uint32_t g(uint32_t w) {",
            "    return ((w & UINT32_C(0xFF000000) >> 24) | ((w & UINT32_C(0x00FF0000) >> 8) |",
            "            ((w & UINT32_C(0x0000FF00) << 8) | ((w & UINT32_C(0x000000FF) << 24);",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "static inline uint16_t f(uint16_t w) {",
            "    return ((w & UINT16_C(0xFF00) >> 8) | ((w & UINT16_C(0x00FF) << 8);",
            "}",
            "static inline uint32_t g(uint32_t w) {",
            "    return ((w & UINT32_C(0xFF000000) >> 24) | ((w & UINT32_C(0x00FF0000) >> 8) |",
            "            ((w & UINT32_C(0x0000FF00) << 8) | ((w & UINT32_C(0x000000FF) << 24);",
            "}"
        )
    );
}

#[test]
fn nested_if_paren_continuation_aligns_under_inner_paren() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::Attach;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void) {",
            "    if ((mode == A ||",
            "            (state == B &&",
            "             mode == C))) {",
            "        x;",
            "    }",
            "}"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void) {",
            "    if ((mode == A ||",
            "            (state == B &&",
            "             mode == C))) {",
            "        x;",
            "    }",
            "}"
        )
    );
}

#[test]
fn operator_continuation_realigns_to_shallower_paren_after_inner_parens_close() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=linux".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(
            "void f()\n{\n\tif (((a) && ((p > q)\n\t\t|| (r > s)))\n\t\t|| ((b) && (t > u))) {\n\t\tx = 1;\n\t}\n}\n",
            &options,
        ),
        "void f()\n{\n    if (((a) && ((p > q)\n                 || (r > s)))\n        || ((b) && (t > u))) {\n        x = 1;\n    }\n}\n",
    );
}

#[test]
fn aligns_logical_chains_and_nested_calls_with_configured_wrapping() {
    let mut options = FormatOptions::default();
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    options.break_after_logical = true;
    options.max_continuation_indent = 80;
    let actual = format_with(
        fixture!(
            "int f(const header_t *key){",
            "    return match_header(key, \"Alpha\", sizeof(\"Alpha\") - 1) ||",
            "           match_header(key, \"Beta\", sizeof(\"Beta\") - 1) ||",
            "           match_header(key, \"Long-Header\",",
            "                        sizeof(\"Long-Header\") - 1) ||",
            "           match_header(key, \"Other-Header\",",
            "                        sizeof(\"Other-Header\") - 1);",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "int f(const header_t *key)",
            "{",
            "    return match_header(key, \"Alpha\", sizeof(\"Alpha\") - 1) ||",
            "           match_header(key, \"Beta\", sizeof(\"Beta\") - 1) ||",
            "           match_header(key, \"Long-Header\",",
            "                        sizeof(\"Long-Header\") - 1) ||",
            "           match_header(key, \"Other-Header\",",
            "                        sizeof(\"Other-Header\") - 1);",
            "}",
        )
    );
}

#[test]
fn breaks_consecutive_statement_words_onto_their_source_lines() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f() {", "alpha", "beta;", "}"), &options),
        fixture!("void f() {", "    alpha", "    beta;", "}")
    );
}

#[test]
fn recomputes_bare_return_operand_onto_indented_line() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f() {", "return", "value;", "}"), &options),
        fixture!("void f() {", "    return", "        value;", "}")
    );
}

#[test]
fn breaks_operator_led_continuation_line() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("void f() {", "alpha", "*beta;", "}"), &options),
        fixture!("void f() {", "    alpha", "    *beta;", "}")
    );
}

#[test]
fn leading_operator_without_prior_statement_keeps_column() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(fixture!("", "< value alpha"), &options),
        fixture!("", "< value alpha")
    );
}

#[test]
fn aligns_after_parens_by_default_for_conditions() {
    let actual = format(fixture!("int f(){if(check(a,", "b)){return 1;}}",));

    assert_eq!(
        actual,
        fixture!(
            "int f()",
            "{",
            "    if (check(a,",
            "              b))",
            "    {",
            "        return 1;",
            "    }",
            "}",
        )
    );
}

#[test]
fn return_call_open_paren_continuation_aligns_after_return() {
    let source = "int f() {\n    return Create\n           (a, b);\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn return_call_split_open_paren_block_aligns_after_return() {
    let source = "class C {\n    int f() {\n        return Base::Create\n               (\n                   doc, view\n               );\n    }\n};\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_return_call_closing_paren_keeps_call_continuation_indent() {
    assert_eq!(
        format_c(
            "bool f() {\n    return obj->Create(\n        arg\n        );\n}\n",
            &FormatOptions::default(),
        ),
        "bool f() {\n    return obj->Create(\n               arg\n           );\n}\n",
    );
}

#[test]
fn comment_after_return_call_open_paren_uses_return_continuation_indent() {
    assert_eq!(
        format_c(
            "return inner(// note\n value);\n",
            &FormatOptions::default(),
        ),
        "return inner(// note\n           value);\n"
    );
    assert_eq!(
        format_c(
            "co_return inner(// note\n value);\n",
            &FormatOptions::default(),
        ),
        "co_return inner(// note\n    value);\n"
    );
}

#[test]
fn malformed_catch_call_comment_uses_header_continuation_indent() {
    assert_eq!(
        format_c("catch inner(// note\n value);\n", &FormatOptions::default(),),
        "catch inner(// note\n        value);\n"
    );
}

#[test]
fn aligns_function_pointer_parameter_continuations_inside_structs() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!(
            "typedef char *(*handler_pt)(int *first,",
            "int *second);",
            "typedef byte_alias *(*byte_handler_pt)(byte_alias *buf, size_t len);",
            "typedef void (*writer_pt)(int level,",
            "byte_alias *buf, size_t len);",
            "struct S{",
            "int (*handler)(int *first,",
            "int *second);",
            "char *(*factory)(void);",
            "};"
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "typedef char *(*handler_pt)(int *first,",
            "                            int *second);",
            "typedef byte_alias *(*byte_handler_pt)(byte_alias *buf, size_t len);",
            "typedef void (*writer_pt)(int level,",
            "                          byte_alias *buf, size_t len);",
            "struct S {",
            "    int (*handler)(int *first,",
            "                   int *second);",
            "    char *(*factory)(void);",
            "};",
        )
    );
}

#[test]
fn long_expression_operator_led_continuation_uses_two_level_fallback() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    CHECK(very_long_expression_name_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa == call(a, b, Style_Currency",
            "        | Style_WithThousandsSep));",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    CHECK(very_long_expression_name_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa == call(a, b, Style_Currency",
            "            | Style_WithThousandsSep));",
            "}",
        )
    );
}

#[test]
fn new_expression_first_arg_over_max_uses_two_level_fallback() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    Dialog* dialog = new VeryLongDialogTypeName(this,",
            "                           \"message\",",
            "                           \"caption\");",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    Dialog* dialog = new VeryLongDialogTypeName(this,",
            "            \"message\",",
            "            \"caption\");",
            "}",
        )
    );
}

#[test]
fn new_expression_empty_open_paren_args_use_statement_continuation() {
    let source = fixture!(
        "void f()",
        "{",
        "    auto task = new Type(",
        "        a,",
        "        b);",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn split_new_expression_call_arguments_use_assignment_continuation_indent() {
    assert_eq!(
        format_c(
            "bool f() {\n    o = new Class(\n        arg\n        );\n}\n",
            &FormatOptions::default(),
        ),
        "bool f() {\n    o = new Class(\n        arg\n    );\n}\n",
    );
}

#[test]
fn new_expression_split_call_paren_uses_statement_indent() {
    let actual = format_c(
        fixture!(
            "void f()",
            "{",
            "    value = new Type",
            "        (",
            "            a,",
            "            b",
            "        );",
            "}",
        ),
        &FormatOptions::default(),
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    value = new Type",
            "    (",
            "        a,",
            "        b",
            "    );",
            "}",
        )
    );
}

#[test]
fn return_ternary_tail_after_closed_condition_paren_aligns_to_return_value() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            "int f(void)\n{\n\treturn (check_a(value) ||\n\t\tcheck_b(value))\n\t       ? -1 : 0;\n}\n",
            &options,
        ),
        "int f(void)\n{\n    return (check_a(value) ||\n            check_b(value))\n           ? -1 : 0;\n}\n",
    );
}

#[test]
fn file_scope_call_continuation_falls_back_when_rhs_also_exceeds_max() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            "static struct config_state really_long_global_value = INIT_STATE_FLAGS(\"name\", count / 10, 3,\n\tEXTRA_FLAG_ON_RELEASE);\n",
            &options,
        ),
        "static struct config_state really_long_global_value = INIT_STATE_FLAGS(\"name\", count / 10, 3,\n        EXTRA_FLAG_ON_RELEASE);\n",
    );
}

// A closed inner subscript does not shift its outer operator continuation.
#[test]
fn nested_subscript_operator_continuation_aligns_to_operator_column() {
    let options = options_from_args(&["--style=linux", "--mode=c"]);

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tqmul = table[inv[exp[faila] ^\n\texp[failb]]];\n}\n",
            &options,
        ),
        "void f(void)\n{\n    qmul = table[inv[exp[faila] ^\n                                exp[failb]]];\n}\n",
    );
}

#[test]
fn logical_return_chain_keeps_call_column_after_wrapped_first_call() {
    let options = options_from_args(&[
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--pad-comma",
        "--pad-header",
        "--unpad-paren",
        "--break-after-logical",
        "--align-pointer=name",
        "--align-reference=name",
        "--min-conditional-indent=0",
        "--max-continuation-indent=80",
        "--max-code-length=109",
    ]);
    let source = "int helper(const Item *item)\n{\n    return item_option_name_equals(item, \"FirstValue\",\n                                   sizeof(\"FirstValue\") - 1) ||\n           item_option_name_equals(item, \"Next-Value\",\n                                   sizeof(\"Next-Value\") - 1) ||\n           item_option_name_equals(item, \"Another\", sizeof(\"Another\") - 1);\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn allman_style_caps_deep_nested_call_continuation() {
    let options = options_from_args(&["--style=allman", "--mode=c"]);
    let source = "template <typename A, typename B>\nusing ContainerIterPairType = decltype(std::make_pair(\n        value<A>(), value<B>()));\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn linux_style_macro_logical_continuation_aligns_under_condition() {
    let options = options_from_args(&["--style=linux"]);

    assert_eq!(
        format_c(
            "static inline int helper(int value)\n{\n    CHECK_NOW((value < TYPE_MIN)\n           || (value > TYPE_MAX));\n\n    return value;\n}\n",
            &options,
        ),
        "static inline int helper(int value)\n{\n    CHECK_NOW((value < TYPE_MIN)\n              || (value > TYPE_MAX));\n\n    return value;\n}\n",
    );
}

#[test]
fn linux_style_nested_macro_call_continuation_aligns_under_argument() {
    let options = options_from_args(&["--style=linux"]);
    let source =
        "void helper(void)\n{\n    LOG_CFG((\"value=%d\\n\",\n             value, other));\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn nested_ternary_colon_arm_aligns_continuation_inside_open_paren() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  CALL (MISC, \"fmt\",\n        format == 0 ? \"ARGB8888\"\n        : (format == 1 ? \"XRGB8888\"\n           : (char *) &format), format);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    CALL (MISC, \"fmt\",\n          format == 0 ? \"ARGB8888\"\n          : (format == 1 ? \"XRGB8888\"\n             : (char *) &format), format);\n}\n",
    );
}

#[test]
fn logical_or_operand_after_deep_call_close_uses_standard_continuation() {
    assert_eq!(
        format_c(
            "static void r(void)\n{\n  if (!init (&layout,\n             get_format (image),\n             width,\n             1) ||\n      !(data = alloc (size)))\n    {\n      return;\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "static void r(void)\n{\n    if (!init (&layout,\n               get_format (image),\n               width,\n               1) ||\n            !(data = alloc (size)))\n    {\n        return;\n    }\n}\n",
    );
}

#[test]
fn nested_assignment_in_condition_does_not_trigger_parameter_default_continuation() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (!(a = find (id)) ||\n      !(b = copy (id)))\n    return;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (!(a = find (id)) ||\n            !(b = copy (id)))\n        return;\n}\n",
    );
}

#[test]
fn logical_or_operand_aligns_to_outer_paren_after_nested_close() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if ((aaa (m) &\n       (A | B |\n        C | D)) ||\n      bbb (m) != NULL)\n    go ();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if ((aaa (m) &\n            (A | B |\n             C | D)) ||\n            bbb (m) != NULL)\n        go ();\n}\n",
    );
}

#[test]
fn logical_or_group_second_operand_aligns_with_first() {
    assert_eq!(
        format_c(
            "void g(void)\n{\n  if (alpha != beta ||\n      (helper_one (gamma) == 0 &&\n       helper_two (delta) != 0) ||\n      (helper_one (gamma) != 0 &&\n       helper_three (delta, gamma) != 0))\n    {\n      call ();\n    }\n}\n",
            &FormatOptions::default(),
        ),
        "void g(void)\n{\n    if (alpha != beta ||\n            (helper_one (gamma) == 0 &&\n             helper_two (delta) != 0) ||\n            (helper_one (gamma) != 0 &&\n             helper_three (delta, gamma) != 0))\n    {\n        call ();\n    }\n}\n",
    );
}

#[test]
fn function_pointer_param_continuation_expands_leading_tab() {
    assert_eq!(
        format_c(
            "struct Config\n{\n  void\t(* on_change) (Config *config,\n                       double          value);\n};\n",
            &FormatOptions::default(),
        ),
        "struct Config\n{\n    void\t(* on_change) (Config *config,\n                           double          value);\n};\n",
    );
}

#[test]
fn function_pointer_typedef_params_align_under_paren_within_limit() {
    let source = "typedef void (*Cb) (Application *instance,\n                    const char  *handle,\n                    UserData     user_data);\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn function_pointer_typedef_params_cap_at_max_continuation_indent() {
    assert_eq!(
        format_c(
            "typedef void (*VeryLongCallbackNameHere12345) (Application *contextx,\n                                               const char  *buffer,\n                                               UserData     user_data);\n",
            &FormatOptions::default(),
        ),
        "typedef void (*VeryLongCallbackNameHere12345) (Application *contextx,\n        const char  *buffer,\n        UserData     user_data);\n",
    );
}

#[test]
fn string_literal_after_plus_in_nested_call_aligns_to_first_string_literal() {
    assert_eq!(
        format_c(
            "void f()\n{\n    CHECK_QUERY(obj, exec(\"first value \" + db.entryName() + \" next operation \" + db.entryName() +\n                \"final text value!!\"));\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    CHECK_QUERY(obj, exec(\"first value \" + db.entryName() + \" next operation \" + db.entryName() +\n                          \"final text value!!\"));\n}\n",
    );
}

#[test]
fn assignment_stream_chain_continuation_aligns_to_value_start() {
    assert_eq!(
        format_c(
            "void f()\n{\n    const StringItems expectedItemDirs = StringItems() << primaryDirSet + LiteralString(\"/collection\")\n            << secondaryDirs + LiteralString(\"/collection\");\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    const StringItems expectedItemDirs = StringItems() << primaryDirSet + LiteralString(\"/collection\")\n                                         << secondaryDirs + LiteralString(\"/collection\");\n}\n",
    );
}

#[test]
fn stream_chain_after_inline_multiline_braced_argument_returns_to_stream_column() {
    assert_eq!(
        format_c(
            "void f()\n{\n    Suite::addRow(\"Spaces\") << QStringList{ \"   Documents (*.doc)\", \"Everything (*.*)\",\n                                            \"   Stuff (  *.stf   *.tng)\", \"    *.exe\" }\n                                            << \".doc,.stf,.tng,.exe\";\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Suite::addRow(\"Spaces\") << QStringList{ \"   Documents (*.doc)\", \"Everything (*.*)\",\n                                            \"   Stuff (  *.stf   *.tng)\", \"    *.exe\" }\n                            << \".doc,.stf,.tng,.exe\";\n}\n",
    );
}

#[test]
fn adjacent_string_literal_argument_after_expression_string_aligns_to_first_string() {
    assert_eq!(
        format_c(
            "void f()\n{\n    socket->write(\"GET \" + obj.toEncoded(Path::RemovePrefix | Path::RemoveQualifier | Path::RemoveTrailing) + \" HTTP/1.0\\r\\n\"\n            \"Connection: close\\r\\n\"\n            \"Client-Name: sample_component_runner/1.0\\r\\n\"\n            \"Host: \" + encodedHost + \"\\r\\n\"\n            \"\\r\\n\");\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    socket->write(\"GET \" + obj.toEncoded(Path::RemovePrefix | Path::RemoveQualifier | Path::RemoveTrailing) + \" HTTP/1.0\\r\\n\"\n                  \"Connection: close\\r\\n\"\n                  \"Client-Name: sample_component_runner/1.0\\r\\n\"\n                  \"Host: \" + encodedHost + \"\\r\\n\"\n                  \"\\r\\n\");\n}\n",
    );
}

#[test]
fn assignment_ternary_colon_after_question_row_keeps_question_row_indent() {
    assert_eq!(
        format_c(
            "void f()\n{\n    const TimeState::RecordData resultData =\n        firstTime.hasActiveState() ? contextx.previousCheckpoint(firstTime)\n    : contextx.nextCheckpoint(firstTime);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    const TimeState::RecordData resultData =\n        firstTime.hasActiveState() ? contextx.previousCheckpoint(firstTime)\n        : contextx.nextCheckpoint(firstTime);\n}\n",
    );
}

#[test]
fn nested_ternary_colon_row_after_colon_question_row_keeps_colon_column() {
    assert_eq!(
        format_c(
            "void f()\n{\n    Data::EntryType item =  (!store.entryItems.hasNone()) ? store.entryItems.at(0)\n                            :  (!store.lookupMap.hasNone())  ? *store.lookupMap.begin()\n                            :  *store.keyList.begin();\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    Data::EntryType item =  (!store.entryItems.hasNone()) ? store.entryItems.at(0)\n                            :  (!store.lookupMap.hasNone())  ? *store.lookupMap.begin()\n                            :  *store.keyList.begin();\n}\n",
    );
}

#[test]
fn call_argument_after_previous_argument_row_keeps_argument_column() {
    assert_eq!(
        format_c(
            "void f()\n{\n    ::BuildInterface(modeFlags, reinterpret_cast<const wchar_t*>(tagName.bytes()),\n                     NAME(\"sample_process_id\"), options,\n                     (canvasWidth - sizeX) / 2, (canvasHeight - size_y) / 2, sizeX, size_y,\n                          0, NONE, runtimeData, NONE);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    ::BuildInterface(modeFlags, reinterpret_cast<const wchar_t*>(tagName.bytes()),\n                     NAME(\"sample_process_id\"), options,\n                     (canvasWidth - sizeX) / 2, (canvasHeight - size_y) / 2, sizeX, size_y,\n                     0, NONE, runtimeData, NONE);\n}\n",
    );
}

#[test]
fn macro_nested_member_call_argument_after_open_paren_indents_four_levels() {
    assert_eq!(
        format_c(
            "void f()\n{\n    CONFIRM(timeData.hasReferenceId(\n        Timestamp(DateX(2020, 1, 1), TimeX(1, 30), zone1).toReferenceValues()));\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    CONFIRM(timeData.hasReferenceId(\n                Timestamp(DateX(2020, 1, 1), TimeX(1, 30), zone1).toReferenceValues()));\n}\n",
    );
}

#[test]
fn if_negated_qualified_call_argument_continuation_indents_two_levels() {
    assert_eq!(
        format_c(
            "bool f()\n{\n    if (!AbstractValueModel::beginUpdateItems(originParent, pos, pos + len - 1,\n        destinationRecord, end))\n        return false;\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n    if (!AbstractValueModel::beginUpdateItems(originParent, pos, pos + len - 1,\n            destinationRecord, end))\n        return false;\n}\n",
    );
}

#[test]
fn call_argument_after_ternary_arg_keeps_arg_column() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  texture = call (aa (x),\n                  bb (x)\n                  ? A\n                  : B,\n                  yy,\n                  cc (x));\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    texture = call (aa (x),\n                    bb (x)\n                    ? A\n                    : B,\n                    yy,\n                    cc (x));\n}\n",
    );
}

#[test]
fn return_cast_member_arrow_after_closed_paren_aligns_to_return_value() {
    assert_eq!(
        format_c(
            "void f()\n{\n    return static_cast<TransportClient::ConnectionManagerId *>(\n               ApplicationContextData::runtimeConnectionId())\n    ->channel();\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    return static_cast<TransportClient::ConnectionManagerId *>(\n               ApplicationContextData::runtimeConnectionId())\n           ->channel();\n}\n",
    );
}

#[test]
fn nested_logical_group_continuations_keep_operand_columns() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (!a &&\n      (((b &&\n\t!c) ||\n       (!b &&\n\tc))))\n    call();\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (!a &&\n            (((b &&\n               !c) ||\n              (!b &&\n               c))))\n        call();\n}\n",
    );
}
#[test]
fn long_call_after_braceless_ternary_keeps_argument_rows_aligned_to_open_paren() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (message == EVENT_SCROLL)\n    direction = (((short) HIGH_WORD (value)) > 0)\n                  ? DIRECTION_UP\n                  : DIRECTION_DOWN;\n\n  event = very_long_event_factory_name_with_suffix (surface,\n                                                    pointer,\n                                                    NULL,\n                                                    tick,\n                                                    state,\n                                                    direction,\n                                                    unknown);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (message == EVENT_SCROLL)\n        direction = (((short) HIGH_WORD (value)) > 0)\n                    ? DIRECTION_UP\n                    : DIRECTION_DOWN;\n\n    event = very_long_event_factory_name_with_suffix (surface,\n                                                      pointer,\n                                                      NULL,\n                                                      tick,\n                                                      state,\n                                                      direction,\n                                                      unknown);\n}\n",
    );
}
#[test]
fn call_after_braceless_ternary_keeps_all_argument_rows_aligned() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  if (message == EVENT_SCROLL)\n    direction = cond\n                  ? DIRECTION_UP\n                  : DIRECTION_DOWN;\n\n  event = make_event (surface,\n                      pointer,\n                      NULL);\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    if (message == EVENT_SCROLL)\n        direction = cond\n                    ? DIRECTION_UP\n                    : DIRECTION_DOWN;\n\n    event = make_event (surface,\n                        pointer,\n                        NULL);\n}\n",
    );
}
#[test]
fn typedef_function_pointer_with_space_after_star_keeps_parameter_column() {
    let source = "typedef void (* FnType) (Item *self,\n                         Context *context,\n                         void *data);\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn long_macro_statement_argument_keeps_open_paren_alignment() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  LONG_CONTENT_CHECK_EXCEPTION (eof,\n                                ERROR_DOMAIN,\n                                ERROR_CODE)\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    LONG_CONTENT_CHECK_EXCEPTION (eof,\n                                  ERROR_DOMAIN,\n                                  ERROR_CODE)\n}\n",
    );
}
#[test]
fn nested_macro_argument_over_max_uses_continuation_fallback() {
    assert_eq!(
        format_c(
            "DEFINE_ITEM_WITH_TRAITS (Item, item, BASE_TYPE,\n                         IMPLEMENT_VALUE_TRAIT (TRAIT_TYPE,\n                                                item_trait_init));\n",
            &FormatOptions::default(),
        ),
        "DEFINE_ITEM_WITH_TRAITS (Item, item, BASE_TYPE,\n                         IMPLEMENT_VALUE_TRAIT (TRAIT_TYPE,\n                                 item_trait_init));\n",
    );
}
#[test]
fn comma_separated_member_assignments_align_after_arrow() {
    assert_eq!(
        format_c(
            "void f(void)\n{\n  object->first = alpha,\n  object->second = beta;\n}\n",
            &FormatOptions::default(),
        ),
        "void f(void)\n{\n    object->first = alpha,\n            object->second = beta;\n}\n",
    );
}
#[test]
fn split_member_access_arrow_keeps_member_indent() {
    let source = "void f()\n{\n    object->\n    Call();\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn outer_call_close_after_nested_call_aligns_to_outer_open() {
    let source = "void f()\n{\n    QueueEvent(\n        new X(\n            a)\n    );\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn single_line_ternary_call_does_not_leak_into_following_loop() {
    let source = "void f(void)\n{\n    for (run = 0; run < N; run++)\n    {\n        left = a ();\n\n        attempts = measure () ? MAX_ATTEMPTS : random_int_range (0, MAX_ATTEMPTS);\n        for (try = 0; try < attempts; try++)\n        {\n            unsigned id = b (0, MAX);\n        }\n    }\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn nested_ternary_colon_at_line_end_branch_aligns_inside_paren() {
    let source = "void f(void)\n{\n    display (\"%s\",\n             (mode == A ? \"FIRST\" :\n              (mode == B ? \"SECOND\" :\n               \"???\")));\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
#[test]
fn comma_operator_operand_in_open_paren_aligns_inside_paren() {
    let source = "void f(void)\n{\n    display (\"%s\",\n             (a == C ? \"X\" :\n              (sprintf (buf, \"%p\", a),\n               buf)));\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn call_arg_continuation_starting_with_paren_aligns_to_open_paren() {
    let source = "void f()\n{\n    set_event_action(AS_VALUE(target), \"changed\", AS_HANDLER\n                     (receiver), VALUE_TO_HANDLE(value));\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn macro_call_nested_split_call_after_string_concat_aligns_to_nested_call() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tCHECK_EQ(\"alpha \"\n\t          \"beta\",\n\t          replace(\n\t              \"gamma\",\n\t              value));\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    CHECK_EQ(\"alpha \"\n             \"beta\",\n             replace(\n                 \"gamma\",\n                 value));\n}\n",
    );
}

#[test]
fn parenthesized_assignment_ternary_arms_align_to_condition() {
    assert_eq!(
        format_c(
            "void f()\n{\n\tvalue =\n\t\t(alpha > beta\n\t\t ? alpha\n\t\t : beta);\n}\n",
            &FormatOptions::default(),
        ),
        "void f()\n{\n    value =\n        (alpha > beta\n         ? alpha\n         : beta);\n}\n",
    );
}

#[test]
fn return_parenthesized_condition_ternary_aligns_to_condition() {
    assert_eq!(
        format_c(
            "int f()\n{\n\treturn (alpha || beta\n\t\t\t? one + 1 : two);\n}\n",
            &FormatOptions::default(),
        ),
        "int f()\n{\n    return (alpha || beta\n            ? one + 1 : two);\n}\n",
    );
}

#[test]
fn return_nested_logical_group_keeps_inner_and_outer_columns() {
    assert_eq!(
        format_c(
            "bool f()\n{\n    return (first().call()\n           || second().call())\n            && third();\n}\n",
            &FormatOptions::default(),
        ),
        "bool f()\n{\n    return (first().call()\n            || second().call())\n           && third();\n}\n",
    );
}

#[test]
fn return_logical_tail_after_closed_inner_paren_uses_return_value_column() {
    assert_eq!(
        format_c(
            "int f(struct item *port)\n{\n\treturn ((port->a.flags & A) &&\n\t\t!(port->a.flags & B)) ||\n\t\t(port->b.flags & C);\n}\n",
            &FormatOptions::default(),
        ),
        "int f(struct item *port)\n{\n    return ((port->a.flags & A) &&\n            !(port->a.flags & B)) ||\n           (port->b.flags & C);\n}\n",
    );
}

#[test]
fn logical_tail_after_closed_inner_paren_uses_previous_operator_column_minus_one() {
    let source = "void f()\n{\n    REQUIRE((state() == A\n             && descriptor() == B)\n            || state() == C);\n}\n";

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn multiline_subscript_closing_bracket_aligns_with_opening_bracket() {
    let mut options = FormatOptions::default();
    let args = ["--style=allman", "--pad-oper"].map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");

    assert_eq!(
        format_c("void f(){\narray[index\n];\n}\n", &options),
        "void f()\n{\n    array[index\n         ];\n}\n",
    );
}
