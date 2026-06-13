use super::raw_strings;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct Converter {
    enabled: bool,
    in_block_comment: bool,
    quote: Option<char>,
    raw_delimiter: Option<String>,
}

impl Converter {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            in_block_comment: false,
            quote: None,
            raw_delimiter: None,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub(super) fn convert(
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

pub(super) fn space_indent_to_force_tabs(line: &str, tab_width: usize) -> String {
    let tab_width = tab_width.max(1);
    let whitespace = leading_whitespace_len(line);
    let tab_count = whitespace / tab_width;
    let replace_len = tab_count * tab_width;
    format!("{}{}", "\t".repeat(tab_count), &line[replace_len..])
}

pub(super) fn force_tab_indent_to_spaces(line: &str, tab_width: usize) -> String {
    let tab_width = tab_width.max(1);
    let mut output = String::new();
    let mut chars = line.chars().peekable();
    while matches!(chars.peek(), Some('\t')) {
        output.push_str(&" ".repeat(tab_width));
        chars.next();
    }
    output.extend(chars);
    output
}

fn leading_whitespace_len(line: &str) -> usize {
    line.char_indices()
        .take_while(|(_, ch)| matches!(ch, ' ' | '\t'))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
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
    let mut output = String::new();
    let mut column = 0usize;
    let mut quote = start_quote;
    let mut raw_delimiter = start_raw_delimiter.map(str::to_string);
    let mut in_line_comment = false;
    let mut at_indent = true;
    let mut prev = '\0';
    let mut chars = line.char_indices().peekable();

    while let Some((byte_index, ch)) = chars.next() {
        if ch == '\n' && raw_delimiter.is_none() {
            output.push(ch);
            column = 0;
            in_line_comment = false;
            at_indent = true;
            prev = '\0';
            continue;
        }
        let raw = if let Some(delimiter) = raw_delimiter.take() {
            let end = raw_strings::closing_end(line, byte_index, &delimiter);
            Some((delimiter, end))
        } else if quote.is_none() && !in_block_comment && !in_line_comment {
            raw_strings::start(line, byte_index).map(|raw| (raw.delimiter, raw.end))
        } else {
            None
        };
        if let Some((delimiter, end)) = raw {
            let span_end = end.unwrap_or(line.len());
            push_char(&mut output, &mut column, ch, tab_width);
            prev = ch;
            while chars.peek().is_some_and(|(index, _)| *index < span_end) {
                let Some((_, next)) = chars.next() else {
                    break;
                };
                push_char(&mut output, &mut column, next, tab_width);
                prev = next;
            }
            at_indent = false;
            if end.is_none() {
                raw_delimiter = Some(delimiter);
            }
            continue;
        }

        let is_digit_separator = ch == '\''
            && prev.is_ascii_hexdigit()
            && chars
                .peek()
                .is_some_and(|(_, next)| next.is_ascii_hexdigit());
        prev = ch;

        let in_leading_indent = at_indent && matches!(ch, ' ' | '\t');
        if !matches!(ch, ' ' | '\t') {
            at_indent = false;
        }

        if ch == '\t' && quote.is_none() && !(keep_indent_tabs && in_leading_indent) {
            let spaces = tab_width - (column % tab_width);
            output.push_str(&" ".repeat(spaces));
            column += spaces;
            continue;
        }

        push_char(&mut output, &mut column, ch, tab_width);

        if in_line_comment {
            continue;
        }

        if let Some(quote_char) = quote {
            if ch == '\\' {
                if let Some((_, next)) = chars.next() {
                    push_char(&mut output, &mut column, next, tab_width);
                }
            } else if ch == quote_char {
                quote = None;
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' && matches!(chars.peek(), Some((_, '/'))) {
                push_char(&mut output, &mut column, '/', tab_width);
                chars.next();
                in_block_comment = false;
            }
            continue;
        }

        match ch {
            '"' => quote = Some(ch),
            '\'' if !is_digit_separator => quote = Some(ch),
            '/' => match chars.peek() {
                Some((_, '/')) => {
                    push_char(&mut output, &mut column, '/', tab_width);
                    chars.next();
                    in_line_comment = true;
                }
                Some((_, '*')) => {
                    push_char(&mut output, &mut column, '*', tab_width);
                    chars.next();
                    in_block_comment = true;
                }
                _ => {}
            },
            _ => {}
        }
    }
    (output, in_block_comment, quote, raw_delimiter)
}

fn push_char(output: &mut String, column: &mut usize, ch: char, tab_width: usize) {
    output.push(ch);
    if ch == '\n' {
        *column = 0;
    } else if ch == '\t' {
        *column += tab_width - (*column % tab_width);
    } else {
        *column += 1;
    }
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

    #[test]
    fn converts_force_tab_indent() {
        assert_eq!(
            force_tab_indent_to_spaces("\t\treturn x;", 2),
            "    return x;"
        );
        assert_eq!(force_tab_indent_to_spaces("\treturn x;", 0), " return x;");
        assert_eq!(
            force_tab_indent_to_spaces("printf(\"\t\");", 4),
            "printf(\"\t\");"
        );
        assert_eq!(space_indent_to_force_tabs("    x", 8), "    x");
        assert_eq!(space_indent_to_force_tabs("        x", 8), "\tx");
        assert_eq!(space_indent_to_force_tabs("            x", 8), "\t    x");
    }
}
