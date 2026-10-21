use crate::formatter::lexer::raw_strings;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Converter {
    enabled: bool,
    in_block_comment: bool,
    quote: Option<char>,
    raw_delimiter: Option<String>,
}

impl Converter {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            in_block_comment: false,
            quote: None,
            raw_delimiter: None,
        }
    }

    pub(crate) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(crate) fn convert(
        &mut self,
        line: String,
        tab_width: usize,
        keep_indent_tabs: bool,
    ) -> String {
        if !self.enabled {
            return line;
        }
        let kept = if keep_indent_tabs {
            KeptTabs::Indent
        } else {
            KeptTabs::None
        };
        let (line, in_block_comment, quote, raw_delimiter) = to_spaces_stateful(
            &line,
            tab_width,
            self.in_block_comment,
            self.quote,
            kept,
            self.raw_delimiter.as_deref(),
        );
        self.in_block_comment = in_block_comment;
        self.quote = quote;
        self.raw_delimiter = raw_delimiter;
        line
    }
}

/// `source` with its tabs as spaces, but for the tabs block comment rows are
/// laid from, which the output converts once the rows stand, and those of
/// regions with formatting disabled.
pub(crate) fn source_to_spaces(source: &str, tab_width: usize) -> String {
    to_spaces_stateful(
        source,
        tab_width,
        false,
        None,
        KeptTabs::CommentLayout,
        None,
    )
    .0
}

/// `blanks` starting at `start_column` with each tab as the spaces it spans.
pub(crate) fn blanks_to_spaces(blanks: &str, start_column: usize, tab_width: usize) -> String {
    let tab_width = tab_width.max(1);
    let mut spaces = String::with_capacity(blanks.len());
    let mut column = start_column;
    for ch in blanks.chars() {
        let width = if ch == '\t' {
            tab_width - column % tab_width
        } else {
            1
        };
        spaces.extend(std::iter::repeat_n(' ', width));
        column += width;
    }
    spaces
}

/// The tabs a conversion leaves in place.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum KeptTabs {
    None,
    /// The tabs of a line's indent.
    Indent,
    /// The tabs of a line's indent and those between a line's leading braces
    /// and a block comment after them, which a comment's rows are measured
    /// from, and every tab from a disabling comment that no code precedes on
    /// its line through the line of the comment enabling formatting again.
    CommentLayout,
}

/// The text of the comment opening at `start`, up to its end or its line's.
fn comment_text(line: &str, start: usize, block: bool) -> &str {
    let rest = &line[start..];
    let end = if block {
        rest.find("*/").map_or(rest.len(), |end| end + 2)
    } else {
        rest.find('\n').unwrap_or(rest.len())
    };
    &rest[..end]
}

/// Whether the bytes at `at` hold blanks up to a `/*`.
fn blanks_reach_block_comment(bytes: &[u8], at: usize) -> bool {
    let rest = &bytes[at..];
    let blanks = rest
        .iter()
        .take_while(|&&byte| matches!(byte, b' ' | b'\t'))
        .count();
    rest[blanks..].starts_with(b"/*")
}

/// Whether `byte` is an ASCII character one column wide that opens or
/// closes no literal, comment, or raw string.
fn is_plain_byte(byte: u8) -> bool {
    matches!(byte, b' '..=b'~')
        && !matches!(
            byte,
            b'"' | b'\'' | b'/' | b'*' | b'\\' | b'u' | b'L' | b'U' | b'R'
        )
}

fn to_spaces_stateful(
    line: &str,
    tab_width: usize,
    mut in_block_comment: bool,
    start_quote: Option<char>,
    kept: KeptTabs,
    start_raw_delimiter: Option<&str>,
) -> (String, bool, Option<char>, Option<String>) {
    let tab_width = tab_width.max(1);
    let bytes = line.as_bytes();
    let mut output = String::with_capacity(line.len() + line.len() / 8);
    // Bytes up to `copied` are in the output; only tabs are rewritten.
    let mut copied = 0usize;
    let mut column = 0usize;
    let mut quote = start_quote.map(|ch| ch as u8);
    let mut raw_delimiter = start_raw_delimiter.map(str::to_string);
    let mut in_line_comment = false;
    let mut at_indent = true;
    // Whether the line holds only blanks and braces so far.
    let mut braces_lead = !in_block_comment;
    let track_disabled = kept == KeptTabs::CommentLayout;
    // Whether the line holds code outside comments so far.
    let mut code_on_line = start_quote.is_some();
    let mut disabled = false;
    let mut enables_after_line = false;
    let mut previous = 0u8;
    let mut index = 0usize;

    // Advances the column over the byte at `at`, which stays in the output.
    let advance = |column: &mut usize, byte: u8| match byte {
        b'\n' => *column = 0,
        b'\t' => *column += tab_width - (*column % tab_width),
        // A UTF-8 continuation byte adds no column.
        _ if byte & 0xC0 == 0x80 => {}
        _ => *column += 1,
    };

    while index < bytes.len() {
        let byte = bytes[index];
        // A run of bytes that start nothing and end nothing only moves the
        // column.
        if raw_delimiter.is_none() && is_plain_byte(byte) {
            let run = bytes[index..]
                .iter()
                .take_while(|&&byte| is_plain_byte(byte))
                .count();
            if bytes[index..index + run].iter().any(|&byte| byte != b' ') {
                at_indent = false;
            }
            if bytes[index..index + run]
                .iter()
                .any(|&byte| !matches!(byte, b' ' | b'{' | b'}'))
            {
                braces_lead = false;
            }
            if !in_block_comment && !in_line_comment && !at_indent {
                code_on_line = true;
            }
            index += run;
            column += run;
            previous = bytes[index - 1];
            continue;
        }
        if byte == b'\n' && raw_delimiter.is_none() {
            column = 0;
            in_line_comment = false;
            at_indent = true;
            braces_lead = !in_block_comment && quote.is_none();
            code_on_line = quote.is_some();
            if enables_after_line {
                disabled = false;
                enables_after_line = false;
            }
            previous = 0;
            index += 1;
            continue;
        }
        let raw = if let Some(delimiter) = raw_delimiter.take() {
            let end = raw_strings::closing_end(line, index, &delimiter);
            Some((delimiter, end))
        } else if quote.is_none()
            && !in_block_comment
            && !in_line_comment
            && matches!(byte, b'u' | b'L' | b'U' | b'R')
        {
            raw_strings::start(line, index).map(|raw| (raw.delimiter, raw.end))
        } else {
            None
        };
        if let Some((delimiter, end)) = raw {
            let span_end = end.unwrap_or(line.len()).max(index + 1);
            for &span_byte in &bytes[index..span_end] {
                advance(&mut column, span_byte);
            }
            previous = bytes[span_end - 1];
            index = span_end;
            at_indent = false;
            braces_lead = false;
            code_on_line = true;
            if end.is_none() {
                raw_delimiter = Some(delimiter);
            }
            continue;
        }

        let is_digit_separator = byte == b'\''
            && previous.is_ascii_hexdigit()
            && bytes.get(index + 1).is_some_and(u8::is_ascii_hexdigit);
        previous = byte;

        let blank = matches!(byte, b' ' | b'\t');
        let in_leading_indent = at_indent && blank;
        if !blank {
            at_indent = false;
            braces_lead = false;
        }
        let kept_tab = match kept {
            KeptTabs::None => false,
            KeptTabs::Indent => in_leading_indent,
            KeptTabs::CommentLayout => {
                disabled
                    || in_leading_indent
                    || braces_lead
                        && !in_block_comment
                        && !in_line_comment
                        && blanks_reach_block_comment(bytes, index)
            }
        };

        if byte == b'\t' && quote.is_none() && !kept_tab {
            output.push_str(&line[copied..index]);
            let spaces = tab_width - (column % tab_width);
            output.extend(std::iter::repeat_n(' ', spaces));
            column += spaces;
            index += 1;
            copied = index;
            continue;
        }

        advance(&mut column, byte);
        index += 1;

        if in_line_comment {
            continue;
        }

        if let Some(quote_byte) = quote {
            if byte == b'\\' {
                // The escaped character, whole.
                let next_len = line[index..].chars().next().map_or(0, char::len_utf8);
                for &escaped in &bytes[index..index + next_len] {
                    advance(&mut column, escaped);
                }
                index += next_len;
            } else if byte == quote_byte {
                quote = None;
            }
            continue;
        }

        if in_block_comment {
            if byte == b'*' && bytes.get(index) == Some(&b'/') {
                advance(&mut column, b'/');
                index += 1;
                in_block_comment = false;
            }
            continue;
        }

        let opens_comment = byte == b'/' && matches!(bytes.get(index), Some(b'/' | b'*'));
        if opens_comment && track_disabled {
            let text = comment_text(line, index - 1, bytes[index] == b'*');
            if !disabled && !code_on_line && text.contains("*INDENT-OFF*") {
                disabled = true;
            } else if disabled && text.contains("*INDENT-ON*") {
                enables_after_line = true;
            }
        }
        if !blank && !opens_comment {
            code_on_line = true;
        }
        match byte {
            b'"' => quote = Some(byte),
            b'\'' if !is_digit_separator => quote = Some(byte),
            b'/' => match bytes.get(index) {
                Some(b'/') => {
                    advance(&mut column, b'/');
                    index += 1;
                    in_line_comment = true;
                }
                Some(b'*') => {
                    advance(&mut column, b'*');
                    index += 1;
                    in_block_comment = true;
                }
                _ => {}
            },
            _ => {}
        }
    }
    output.push_str(&line[copied..]);
    (
        output,
        in_block_comment,
        quote.map(char::from),
        raw_delimiter,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(
        line: &str,
        tab_width: usize,
        in_block_comment: bool,
        start_quote: Option<char>,
        keep_indent_tabs: bool,
    ) -> String {
        let kept = if keep_indent_tabs {
            KeptTabs::Indent
        } else {
            KeptTabs::None
        };
        to_spaces_stateful(line, tab_width, in_block_comment, start_quote, kept, None).0
    }

    #[test]
    fn expands_tabs_to_tab_stops() {
        assert_eq!(
            convert("int\ta\t=\t1;", 4, false, None, false),
            "int a   =   1;"
        );
        assert_eq!(
            convert("x;\t// a\tb", 4, false, None, false),
            "x;  // a    b"
        );
        assert_eq!(
            convert("x;\t/* a\tb */", 4, false, None, false),
            "x;  /* a    b */"
        );
    }

    #[test]
    fn preserves_literal_tabs() {
        assert_eq!(
            convert("s = \"a\tb\";", 4, false, None, false),
            "s = \"a\tb\";"
        );
        assert_eq!(convert("c = '\t';", 4, false, None, false), "c = '\t';");
    }

    #[test]
    fn carries_lexical_state() {
        assert_eq!(
            convert("a\tb */ x\ty", 4, true, None, false),
            "a   b */ x  y"
        );
        assert_eq!(
            convert("a\tb\";\tx\ty", 4, false, Some('"'), false),
            "a\tb\"; x   y"
        );
        assert_eq!(
            convert("v = 1'000;\tx", 4, false, None, false),
            "v = 1'000;  x"
        );
    }

    #[test]
    fn converter_owns_cross_line_lexical_state() {
        let mut converter = Converter::new(true);

        assert_eq!(
            converter.convert("/* text".to_string(), 4, false),
            "/* text"
        );
        assert_eq!(
            converter.convert("\"a\tb\" */\tx".to_string(), 4, false),
            "\"a  b\" */   x"
        );
        assert_eq!(converter.convert("\"a".to_string(), 4, false), "\"a");
        assert_eq!(
            converter.convert("\tb\";\tx".to_string(), 4, false),
            "\tb\"; x"
        );
    }

    #[test]
    fn source_conversion_keeps_the_tabs_comment_rows_are_laid_from() {
        assert_eq!(
            source_to_spaces("\t{\t/* a\n\t\t b */\tx;\n\ty;\t{\t/* c */\n", 4),
            "\t{\t/* a\n\t\t b */   x;\n\ty;  {   /* c */\n"
        );
    }

    #[test]
    fn source_conversion_keeps_a_disabled_region() {
        assert_eq!(
            source_to_spaces(
                "a;\tb;\n// *INDENT-OFF*\nc;\td;\n// *INDENT-ON*\te;\nf;\tg;\n",
                4
            ),
            "a;  b;\n// *INDENT-OFF*\nc;\td;\n// *INDENT-ON*\te;\nf;  g;\n"
        );
        assert_eq!(
            source_to_spaces("x; // *INDENT-OFF*\ny;\tz;\n", 4),
            "x; // *INDENT-OFF*\ny;  z;\n"
        );
    }

    #[test]
    fn keeps_leading_indent_tabs_when_requested() {
        assert_eq!(convert("\t\tx\ty;", 4, false, None, true), "\t\tx   y;");
    }
}
