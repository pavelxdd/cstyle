#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::{format, format_c, format_with};
use cstyle::config::{BraceStyle, FormatOptions, apply_command_line_args};

#[test]
fn keeps_inline_assembly_braces_attached() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    let actual = format_with(
        fixture!("void f(){_asm{mov eax, ebx}__asm{mov eax, ebx}}"),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    _asm{mov eax, ebx}",
            "    __asm{mov eax, ebx}",
            "}",
        )
    );
}

#[test]
fn suppresses_indent_for_cplusplus_guarded_extern_c_block() {
    let options = FormatOptions::default();
    let source = fixture!(
        "#ifdef __cplusplus",
        "extern \"C\" {",
        "#endif",
        "",
        "int value;",
        "void helper(void);",
        "",
        "#ifdef __cplusplus",
        "}",
        "#endif",
    );
    assert_eq!(format_c(source, &options), source);
}

#[test]
fn attach_extern_c_attaches_guarded_linkage_brace() {
    let mut options = FormatOptions::default();
    options.attach_extern_c = true;

    assert_eq!(
        format_c(
            fixture!(
                "#ifdef __cplusplus",
                "extern \"C\"",
                "{",
                "#endif",
                "",
                "#ifdef __cplusplus",
                "}",
                "#endif",
            ),
            &options,
        ),
        fixture!(
            "#ifdef __cplusplus",
            "extern \"C\" {",
            "#endif",
            "",
            "#ifdef __cplusplus",
            "}",
            "#endif",
        ),
    );
}

#[test]
fn indents_plain_extern_c_block_without_cplusplus_guard() {
    let options = FormatOptions::default();
    let source = fixture!("extern \"C\" {", "int value;", "void helper(void);", "}");
    assert_eq!(
        format_c(source, &options),
        fixture!(
            "extern \"C\" {",
            "    int value;",
            "    void helper(void);",
            "}",
        )
    );
}

#[test]
fn source_attached_extern_brace_is_independent_of_namespace_position() {
    let mut options = FormatOptions::default();
    apply_command_line_args(&mut options, &["--style=allman".to_owned()]).expect("valid options");

    assert_eq!(
        format_c(fixture!("extern \"C\"{", "void run(void);", "}"), &options,),
        fixture!("extern \"C\" {", "    void run(void);", "}")
    );
    assert_eq!(
        format_c(
            fixture!(
                "namespace alpha{",
                "extern \"C\"{",
                "void run(void);",
                "}",
                "}",
            ),
            &options,
        ),
        fixture!(
            "namespace alpha",
            "{",
            "extern \"C\" {",
            "    void run(void);",
            "}",
            "}",
        )
    );
}

#[test]
fn expands_compact_extern_c_block() {
    let actual = format(fixture!("extern \"C\"{int f();}"));

    assert_eq!(actual, fixture!("extern \"C\" {", "    int f();", "}",));
}

#[test]
fn formats_defer_blocks_as_separate_statements() {
    let actual = format(fixture!(
        "void g(){defer{cleanup();}_Defer{cleanup2();}return;}"
    ));

    assert_eq!(
        actual,
        fixture!(
            "void g()",
            "{",
            "    defer {cleanup();}",
            "    _Defer {cleanup2();}",
            "    return;",
            "}",
        )
    );
}

#[test]
fn one_true_brace_style_attaches_control_braces_inside_multiline_defer() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_header = true;
    options.pad_operators = true;
    let actual = format_with(
        fixture!(
            "void f(void){",
            "int done = 0;",
            "defer {",
            "if (!done) {",
            "cleanup();",
            "}",
            "};",
            "}",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "void f(void)",
            "{",
            "    int done = 0;",
            "    defer {",
            "        if (!done) {",
            "            cleanup();",
            "        }",
            "    };",
            "}",
        )
    );
}

#[test]
fn treats_inline_assembly_as_plain_c_statement() {
    let actual = format(fixture!(
        "void f(){asm(\"nop\");__asm__ volatile(\"nop\");}"
    ));
    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    asm(\"nop\");",
            "    __asm__ volatile(\"nop\");",
            "}",
        )
    );
}

#[test]
fn asm_constraint_string_keeps_source_space_before_operand_paren() {
    let source = fixture!(
        "",
        "void foo()",
        "{",
        "    asm volatile(\"x\" : : \"r\" (value), \"m\" (*ptr));",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn wrapped_asm_operand_colon_keeps_single_source_space() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tasm(\"eor\"\n\t    : \"=w\"(res) : \"w\"(p), \"w\"(q));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    asm(\"eor\"\n        : \"=w\"(res) : \"w\"(p), \"w\"(q));\n}\n",
    );
}

#[test]
fn wrapped_asm_empty_output_colon_keeps_input_operand_space() {
    let mut options = FormatOptions::default();
    apply_command_line_args(
        &mut options,
        &["--style=linux".to_owned(), "--mode=c".to_owned()],
    )
    .expect("valid options");

    assert_eq!(
        format_c(
            "void f(void)\n{\n\tasm(\"x\"\n\t    : : \"m\" (value));\n}\n",
            &options,
        ),
        "void f(void)\n{\n    asm(\"x\"\n        : : \"m\" (value));\n}\n",
    );
}

#[test]
fn asm_colon_continuations_keep_operand_column() {
    let source = fixture!(
        "",
        "void foo(void)",
        "{",
        "    asm volatile(\"call %2\"",
        "                 : \"+r\"(a0), \"+r\"(a1)",
        "                 : \"i\"(VALUE)",
        "                 : \"$0\", \"$1\");",
        "}",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}

#[test]
fn formats_microsoft_inline_assembly_blocks() {
    let actual = format(fixture!("void f(){__asm { mov eax, ebx }return;}"));

    assert_eq!(
        actual,
        fixture!(
            "void f()",
            "{",
            "    __asm { mov eax, ebx }",
            "    return;",
            "}",
        )
    );
}

#[test]
fn keeps_asm_one_line_block_intact_with_default_options() {
    let options = FormatOptions::default();

    assert_eq!(
        format_c(
            fixture!("void f() {", "    _asm {cld};", "    _asm  {nop};", "}"),
            &options
        ),
        fixture!("void f() {", "    _asm {cld};", "    _asm  {nop};", "}")
    );
}

#[test]
fn preserves_spaced_and_nested_defer_one_line_blocks() {
    let actual = format(fixture!("void g(){if(x){defer { cleanup(); }}return;}"));

    assert_eq!(
        actual,
        fixture!(
            "void g()",
            "{",
            "    if (x)",
            "    {",
            "        defer { cleanup(); }",
            "    }",
            "    return;",
            "}",
        )
    );
}

#[test]
fn defer_one_line_blocks_ignore_rewrite_options() {
    let mut options = FormatOptions::default();
    options.add_braces = true;
    options.remove_braces = true;
    options.break_one_line_headers = true;
    let actual = format_with(fixture!("void g(){_Defer { cleanup(); }return;}"), &options);

    assert_eq!(
        actual,
        fixture!(
            "void g()",
            "{",
            "    _Defer { cleanup(); }",
            "    return;",
            "}",
        )
    );
}

#[test]
fn preserves_stddefer_macro_definition() {
    let actual = format(fixture!(
        "#define defer _Defer",
        "void g(){defer{cleanup();}}",
    ));
    assert_eq!(
        actual,
        fixture!(
            "#define defer _Defer",
            "void g()",
            "{",
            "    defer {cleanup();}",
            "}",
        )
    );
}
#[test]
fn defer_block_inner_if_keeps_one_true_brace_style() {
    let mut options = FormatOptions::default();
    let args = [
        "--style=1tbs",
        "--mode=c",
        "--pad-oper",
        "--align-pointer=name",
        "--attach-return-type",
    ]
    .map(str::to_owned);
    apply_command_line_args(&mut options, &args).expect("valid options");
    let source = "\nvoid f(void *ptr)\n{\n    defer {\n        if (ptr) {\n            cleanup(ptr);\n        }\n    }\n}\n";

    assert_eq!(format_c(source, &options), source);
}

#[test]
fn assembler_macro_lines_preserve_operand_and_comment_spacing() {
    assert_eq!(
        format_c(
            "\n#ifdef __ASSEMBLY__\n    .macro LOAD out, base\nld  \\out, [\\base]  @ load value\n@ keep comment\nmcr \\out, [\\base]  @ second\n    .endm\n#endif\n",
            &FormatOptions::default(),
        ),
        "\n#ifdef __ASSEMBLY__\n.macro LOAD out, base\nld  \\out, [\\base]  @ load value\n@ keep comment\nmcr \\out, [\\base]  @ second\n.endm\n#endif\n",
    );
}

#[test]
fn swig_percent_blocks_keep_source_shape() {
    let source = fixture!(
        "#ifdef BINDINGS",
        "%typemap(out) Type& { $result = $self; retain($result); }",
        "%pythoncode {",
        "    EVENT_A = binding.Event(X)",
        "    EVENT_B = binding.Event(Y)",
        "}",
        "#endif",
    );

    assert_eq!(format_c(source, &FormatOptions::default()), source);
}
