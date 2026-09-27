use crate::source::lex;

pub const HEADERS: &[&str] = &[
    "_Defer",
    "__except",
    "__finally",
    "__try",
    "case",
    "catch",
    "default",
    "defer",
    "do",
    "else",
    "for",
    "foreach",
    "if",
    "switch",
    "try",
    "while",
    "Q_FOREACH",
];
pub const NON_PAREN_HEADERS: &[&str] = &[
    "_Defer",
    "__finally",
    "__try",
    "case",
    "catch",
    "default",
    "defer",
    "do",
    "else",
    "try",
];
pub const PRE_BLOCK_WORDS: &[&str] = &[
    "class",
    "interface",
    "module",
    "namespace",
    "struct",
    "union",
];
pub const BLOCK_WORDS: &[&str] = &["struct", "union", "enum", "extern"];
pub const PRE_COMMAND_QUALIFIERS: &[&str] = &[
    "autoreleasepool",
    "const",
    "final",
    "interrupt",
    "noexcept",
    "override",
    "sealed",
    "try",
    "volatile",
];

pub const AUTO: &str = "auto";
pub const NEW: &str = "new";
pub const DELETE: &str = "delete";
pub const OPERATOR: &str = "operator";
pub const RETURN: &str = "return";
pub const THROW: &str = "throw";
pub const ACCESS_MODIFIERS: &[&str] = &["private", "protected", "public"];
pub const STREAM_NAMES: &[&str] = &["cerr", "cin", "cout"];

pub const ASSIGNMENT_OPERATORS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>=",
];
pub const OPERATORS: &[&str] = &[
    ">>=", "<<=", "++", "--", "->", "==", "!=", "<=>", ">=", ">>", "<=", "<<", "&&", "||", "::",
    "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<?", ">?", "+", "-", "*", "/", "%", "?", ":",
    "=", "<", ">", "!", "|", "&", "~", "^",
];
const TOKEN_OPERATORS: &[&str] = &[
    ">>=", "<<=", "++", "--", "->", "==", "!=", "<=>", ">=", ">>", "<=", "<<", "&&", "||", "::",
    "+=", "-=", "*=", "/=", "%=", "|=", "&=", "^=", "<?", ">?", "+", "-", "*", "/", "%", "=", "<",
    ">", "!", "|", "&", "~", "^",
];

pub fn is_header(word: &str) -> bool {
    HEADERS.contains(&word)
}

pub fn is_non_paren_header(word: &str) -> bool {
    NON_PAREN_HEADERS.contains(&word)
}

pub fn is_non_type_keyword(word: &str) -> bool {
    matches!(
        word,
        "return"
            | "goto"
            | "case"
            | "sizeof"
            | "new"
            | "delete"
            | "throw"
            | "if"
            | "else"
            | "while"
            | "for"
            | "switch"
            | "do"
            | "catch"
    )
}

pub fn is_type_like_pointer_word(word: &str) -> bool {
    is_pointer_type_word(word) || word.ends_with("_t")
}

pub fn is_macro_like_word(word: &str) -> bool {
    word.len() > 1
        && word.chars().any(|ch| ch == '_' || ch.is_ascii_uppercase())
        && !word.chars().any(|ch| ch.is_ascii_lowercase())
}

pub fn is_pointer_type_word(word: &str) -> bool {
    matches!(
        word,
        "auto"
            | "bool"
            | "char"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "class"
            | "decltype"
            | "double"
            | "enum"
            | "float"
            | "int"
            | "long"
            | "short"
            | "signed"
            | "struct"
            | "typename"
            | "union"
            | "unsigned"
            | "const"
            | "void"
            | "volatile"
            | "wchar_t"
            | "_Atomic"
            | "_BitInt"
            | "_Bool"
            | "_Complex"
    ) || is_core_typedef_word(word)
}

pub fn is_numeric_variable_word(word: &str) -> bool {
    matches!(
        word,
        "bool"
            | "char"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "double"
            | "float"
            | "int"
            | "long"
            | "short"
            | "signed"
            | "unsigned"
            | "wchar_t"
            | "_BitInt"
            | "_Bool"
            | "_Complex"
    ) || is_core_typedef_word(word)
}

pub fn is_leading_continuation_operator(operator: &str) -> bool {
    matches!(
        operator,
        "!=" | "==" | "<" | "<=" | "<=>" | ">" | ">=" | "&&" | "||"
    )
}

fn is_core_typedef_word(word: &str) -> bool {
    matches!(
        word,
        "FILE"
            | "__int128_t"
            | "__uint128_t"
            | "atomic_bool"
            | "atomic_char"
            | "atomic_char16_t"
            | "atomic_char32_t"
            | "atomic_int"
            | "atomic_int_fast16_t"
            | "atomic_int_fast32_t"
            | "atomic_int_fast64_t"
            | "atomic_int_fast8_t"
            | "atomic_int_least16_t"
            | "atomic_int_least32_t"
            | "atomic_int_least64_t"
            | "atomic_int_least8_t"
            | "atomic_intmax_t"
            | "atomic_intptr_t"
            | "atomic_llong"
            | "atomic_long"
            | "atomic_ptrdiff_t"
            | "atomic_schar"
            | "atomic_short"
            | "atomic_size_t"
            | "atomic_uchar"
            | "atomic_uint"
            | "atomic_uint_fast16_t"
            | "atomic_uint_fast32_t"
            | "atomic_uint_fast64_t"
            | "atomic_uint_fast8_t"
            | "atomic_uint_least16_t"
            | "atomic_uint_least32_t"
            | "atomic_uint_least64_t"
            | "atomic_uint_least8_t"
            | "atomic_uintmax_t"
            | "atomic_uintptr_t"
            | "atomic_ullong"
            | "atomic_ulong"
            | "atomic_ushort"
            | "atomic_wchar_t"
            | "blkcnt_t"
            | "blksize_t"
            | "clock_t"
            | "dev_t"
            | "div_t"
            | "errno_t"
            | "fpos_t"
            | "fsblkcnt_t"
            | "fsfilcnt_t"
            | "gid_t"
            | "id_t"
            | "int128_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "int8_t"
            | "int_fast16_t"
            | "int_fast32_t"
            | "int_fast64_t"
            | "int_fast8_t"
            | "int_least16_t"
            | "int_least32_t"
            | "int_least64_t"
            | "int_least8_t"
            | "intmax_t"
            | "intptr_t"
            | "ino_t"
            | "key_t"
            | "ldiv_t"
            | "lldiv_t"
            | "max_align_t"
            | "mbstate_t"
            | "mode_t"
            | "nlink_t"
            | "nullptr_t"
            | "off_t"
            | "pid_t"
            | "ptrdiff_t"
            | "rsize_t"
            | "sig_atomic_t"
            | "size_t"
            | "ssize_t"
            | "suseconds_t"
            | "time_t"
            | "uint128_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
            | "uint8_t"
            | "uint_fast16_t"
            | "uint_fast32_t"
            | "uint_fast64_t"
            | "uint_fast8_t"
            | "uint_least16_t"
            | "uint_least32_t"
            | "uint_least64_t"
            | "uint_least8_t"
            | "uintmax_t"
            | "uintptr_t"
            | "useconds_t"
            | "va_list"
    )
}

pub fn match_operator(source: &[char], index: usize) -> Option<&'static str> {
    let ch = *source.get(index)?;
    if !lex::is_potential_operator_char(ch) {
        return None;
    }
    TOKEN_OPERATORS
        .iter()
        .copied()
        .find(|operator| matches_at(source, index, operator))
}

fn matches_at(source: &[char], index: usize, needle: &str) -> bool {
    let mut chars = needle.chars();
    let mut offset = 0;
    loop {
        match chars.next() {
            Some(expected) if source.get(index + offset) == Some(&expected) => offset += 1,
            Some(_) => return false,
            None => return true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_control_headers() {
        for header in [
            "_Defer",
            "__except",
            "__finally",
            "__try",
            "case",
            "catch",
            "default",
            "defer",
            "do",
            "else",
            "for",
            "foreach",
            "if",
            "switch",
            "try",
            "while",
            "Q_FOREACH",
        ] {
            assert!(is_header(header));
        }
        for header in [
            "_Defer",
            "__finally",
            "__try",
            "case",
            "catch",
            "default",
            "defer",
            "do",
            "else",
            "try",
        ] {
            assert!(is_non_paren_header(header));
        }
        assert!(!is_header("if_value"));
        assert!(!is_non_paren_header("for"));
    }

    #[test]
    fn matches_longest_operator_token() {
        let line = "<<= <? >? :: ? : ...".chars().collect::<Vec<_>>();
        assert_eq!(match_operator(&line, 0), Some("<<="));
        assert_eq!(match_operator(&line, 4), Some("<?"));
        assert_eq!(match_operator(&line, 7), Some(">?"));
        assert_eq!(match_operator(&line, 10), Some("::"));
        assert_eq!(match_operator(&line, 13), None);
        assert_eq!(match_operator(&line, 15), None);
        assert_eq!(match_operator(&line, 17), None);
    }

    #[test]
    fn tokenizer_rejects_non_operator_starts_and_out_of_bounds_indices() {
        let line = "word + value".chars().collect::<Vec<_>>();
        assert_eq!(match_operator(&line, 0), None);
        assert_eq!(match_operator(&line, line.len()), None);
        assert_eq!(match_operator(&line, 5), Some("+"));
    }

    #[test]
    fn operator_table_keeps_longer_prefix_matches_first() {
        for (index, operator) in OPERATORS.iter().enumerate() {
            for previous in &OPERATORS[..index] {
                assert!(
                    !operator.starts_with(previous),
                    "shorter operator {previous} appears before {operator}"
                );
            }
        }
    }
}
