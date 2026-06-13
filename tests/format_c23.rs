#![allow(clippy::field_reassign_with_default)]

#[macro_use]
mod common;

use common::format_with;
use cstyle::config::{BraceStyle, FormatOptions, PointerAlign};

#[test]
fn preserves_c23_keywords_and_types() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "alignas(16) int buf[64];",
            "int n = alignof(double);",
            "_BitInt(32) wide = 0;",
            "unsigned _BitInt(7) flags;",
            "typeof(buf) other;",
            "typeof_unqual(buf) plain;",
            "static_assert(sizeof(int) == 4, \"msg\");",
            "void *p = nullptr;",
            "bool ready = true;",
            "constexpr int MAX = 100;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "alignas(16) int buf[64];",
            "int n = alignof(double);",
            "_BitInt(32) wide = 0;",
            "unsigned _BitInt(7) flags;",
            "typeof(buf) other;",
            "typeof_unqual(buf) plain;",
            "static_assert(sizeof(int) == 4, \"msg\");",
            "void *p = nullptr;",
            "bool ready = true;",
            "constexpr int MAX = 100;",
        )
    );
}

#[test]
fn preserves_c23_numeric_literals_while_padding_operators() {
    let mut options = FormatOptions::default();
    options.brace_style = BraceStyle::OneTrueBrace;
    options.pad_operators = true;
    options.pad_commas = true;
    options.pad_header = true;
    options.unpad_parens = true;
    options.pointer_align = PointerAlign::Name;
    let actual = format_with(
        fixture!(
            "long big = 1'000'000;",
            "int mask = 0b1010'1010;",
            "double hexf = 0x1.8p3;",
            "int wide = 100wb;",
            "unsigned uwide = 100uwb;",
            "unsigned long flags = 0xFFu;",
            "long long count = 1000LL;",
            "double sci = 1.5e10;",
            "int sum = a+1'000;",
            "char letter = u8'a';",
            "int perms = 0o755;",
            "int mode = base|0O644;",
        ),
        &options,
    );

    assert_eq!(
        actual,
        fixture!(
            "long big = 1'000'000;",
            "int mask = 0b1010'1010;",
            "double hexf = 0x1.8p3;",
            "int wide = 100wb;",
            "unsigned uwide = 100uwb;",
            "unsigned long flags = 0xFFu;",
            "long long count = 1000LL;",
            "double sci = 1.5e10;",
            "int sum = a + 1'000;",
            "char letter = u8'a';",
            "int perms = 0o755;",
            "int mode = base | 0O644;",
        )
    );
}
