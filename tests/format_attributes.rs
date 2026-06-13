#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::format_with;
use cstyle::config::{BraceStyle, FormatOptions, PointerAlign};

#[test]
fn preserves_standard_attribute_brackets_while_formatting_declarations() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "struct [[gnu::packed]] Item { int value; };",
            "[[nodiscard]] int f(void);",
            "[[maybe_unused]] static int counter = 0;",
            "[[deprecated(\"use g\")]] void old_fn(void);",
            "int x [[maybe_unused]];",
            "[[gnu::always_inline]] inline int g(int a) { return a; }",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct [[gnu::packed]] Item {",
            "    int value;",
            "};",
            "[[nodiscard]] int f(void);",
            "[[maybe_unused]] static int counter = 0;",
            "[[deprecated(\"use g\")]] void old_fn(void);",
            "int x [[maybe_unused]];",
            "[[gnu::always_inline]] inline int g(int a)",
            "{",
            "    return a;",
            "}",
        )
    );
}

#[test]
fn preserves_gnu_attributes_while_formatting_declarations() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "struct __attribute__((packed)) S { int v; };",
            "int f(void) __attribute__((noreturn));",
            "static int x __attribute__((aligned(16)));",
            "void log_msg(const char *fmt, ...) __attribute__((format(printf, 1, 2)));",
            "__attribute__((always_inline)) inline int g(int a) { return a; }",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "struct __attribute__((packed)) S {",
            "    int v;",
            "};",
            "int f(void) __attribute__((noreturn));",
            "static int x __attribute__((aligned(16)));",
            "void log_msg(const char *fmt, ...) __attribute__((format(printf, 1, 2)));",
            "__attribute__((always_inline)) inline int g(int a)",
            "{",
            "    return a;",
            "}",
        )
    );
}
