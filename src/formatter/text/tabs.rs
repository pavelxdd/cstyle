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
        let (line, in_block_comment, quote, raw_delimiter) = to_spaces_stateful(
            &line,
            tab_width,
            self.in_block_comment,
            self.quote,
            keep_indent_tabs,
            self.raw_delimiter.as_deref(),
        );
        self.in_block_comment = in_block_comment;
        self.quote = quote;
        self.raw_delimiter = raw_delimiter;
        line
    }
}

pub(crate) fn source_to_spaces(source: &str, tab_width: usize) -> String {
    to_spaces_stateful(source, tab_width, false, None, false, None).0
}

fn to_spaces_stateful(
    line: &str,
    tab_width: usize,
    mut in_block_comment: bool,
    start_quote: Option<char>,
    keep_indent_tabs: bool,
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
        if byte == b'\n' && raw_delimiter.is_none() {
            column = 0;
            in_line_comment = false;
            at_indent = true;
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
        }

        if byte == b'\t' && quote.is_none() && !(keep_indent_tabs && in_leading_indent) {
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
        to_spaces_stateful(
            line,
            tab_width,
            in_block_comment,
            start_quote,
            keep_indent_tabs,
            None,
        )
        .0
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
    fn keeps_leading_indent_tabs_when_requested() {
        assert_eq!(convert("\t\tx\ty;", 4, false, None, true), "\t\tx   y;");
    }
}
