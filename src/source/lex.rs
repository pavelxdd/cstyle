pub fn is_identifier_start(ch: char) -> bool {
    matches!(ch, '_' | '$') || ch.is_ascii_alphabetic() || (!ch.is_ascii() && !ch.is_whitespace())
}

pub fn is_identifier_continue(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

pub fn is_word_char(ch: char) -> bool {
    is_identifier_continue(ch) || ch == '.'
}

pub fn is_potential_operator_char(ch: char) -> bool {
    ch.is_ascii_punctuation()
        && !matches!(
            ch,
            '{' | '}' | '(' | ')' | '[' | ']' | ';' | ',' | '#' | '\\' | '\'' | '"'
        )
}

pub fn is_digit_separator(chars: &[char], index: usize) -> bool {
    chars.get(index) == Some(&'\'')
        && index > 0
        && chars.get(index - 1).is_some_and(char::is_ascii_hexdigit)
        && chars.get(index + 1).is_some_and(char::is_ascii_hexdigit)
}

pub fn trailing_word(line: &str) -> &str {
    let line = line.trim_ascii_end();
    let Some((last_index, last_char)) = line.char_indices().next_back() else {
        return "";
    };
    if !is_word_char(last_char) {
        return "";
    }

    let mut start = last_index;
    for (index, ch) in line.char_indices().rev() {
        if !is_word_char(ch) {
            break;
        }
        start = index;
    }
    &line[start..]
}

pub fn leading_identifier(line: &str) -> &str {
    let line = line.trim_ascii_start();
    let bytes = line.as_bytes();
    let mut end = 0;
    // ASCII bytes are read as they are; a wider character is decoded.
    while let Some(&byte) = bytes.get(end) {
        if byte.is_ascii() {
            if !(byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$')) {
                break;
            }
            end += 1;
        } else {
            match line[end..].chars().next() {
                Some(ch) if is_identifier_continue(ch) => end += ch.len_utf8(),
                _ => break,
            }
        }
    }
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_leading_identifiers_as_a_character_search_does() {
        for line in [
            "",
            "  ",
            "x",
            "  abc def",
            "a$b(c",
            "é1x y",
            "1abc",
            "_a_b",
            " \tx",
            "aé b",
            "(x",
            "a\u{a0}b",
        ] {
            let trimmed = line.trim_ascii_start();
            let end = trimmed
                .find(|ch: char| !is_identifier_continue(ch))
                .unwrap_or(trimmed.len());
            assert_eq!(leading_identifier(line), &trimmed[..end], "{line:?}");
        }
    }

    #[test]
    fn finds_trailing_word() {
        assert_eq!(trailing_word("if"), "if");
        assert_eq!(trailing_word("return value"), "value");
        assert_eq!(trailing_word("array[index]"), "");
        assert_eq!(trailing_word("object.if"), "object.if");
    }

    #[test]
    fn finds_leading_identifier() {
        assert_eq!(leading_identifier("if (value)"), "if");
        assert_eq!(leading_identifier("  else"), "else");
        assert_eq!(leading_identifier("object.member"), "object");
        assert_eq!(leading_identifier("(value)"), "");
    }

    #[test]
    fn accepts_ascii_and_extended_identifier_characters() {
        assert!(is_identifier_start('_'));
        assert!(is_identifier_start('$'));
        assert!(is_identifier_start('a'));
        assert!(is_identifier_start('é'));
        assert!(is_identifier_start('α'));
        assert!(is_identifier_continue('9'));
        assert!(is_identifier_continue('\u{301}'));
        assert!(!is_identifier_start('9'));
        assert!(!is_identifier_start(' '));
    }

    #[test]
    fn matches_word_boundaries() {
        assert!(is_word_char('a'));
        assert!(is_word_char('9'));
        assert!(is_word_char('_'));
        assert!(is_word_char('.'));
        assert!(is_word_char('é'));
        assert!(!is_word_char(' '));
    }

    #[test]
    fn detects_potential_operators() {
        assert!(is_potential_operator_char('+'));
        for ch in ['{', '}', '(', ')', '[', ']', ';', ',', '#', '\\', '\'', '"'] {
            assert!(!is_potential_operator_char(ch), "unexpected operator {ch}");
        }
    }

    #[test]
    fn detects_c_digit_separators() {
        let decimal = "1'000".chars().collect::<Vec<_>>();
        assert!(is_digit_separator(&decimal, 1));

        let hex = "0xDEAD'BEEF".chars().collect::<Vec<_>>();
        assert!(is_digit_separator(&hex, 6));

        let char_literal = "'a'".chars().collect::<Vec<_>>();
        assert!(!is_digit_separator(&char_literal, 0));
        assert!(!is_digit_separator(&char_literal, 2));
    }
}
