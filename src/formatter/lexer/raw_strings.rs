use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::source::lex::is_identifier_continue;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RawStringStart {
    pub(crate) delimiter: String,
    pub(crate) end: Option<usize>,
}

pub(crate) fn start(line: &str, start: usize) -> Option<RawStringStart> {
    let rest = line.get(start..)?;
    // Every prefix ends in `R"`, after `u8`, one of `u L U`, or nothing.
    let bytes = rest.as_bytes();
    let quote_r = match bytes.first() {
        Some(b'R') => 0,
        Some(b'u') if bytes.get(1) == Some(&b'8') => 2,
        Some(b'u' | b'L' | b'U') => 1,
        _ => return None,
    };
    if bytes.get(quote_r) != Some(&b'R') || bytes.get(quote_r + 1) != Some(&b'"') {
        return None;
    }
    if line
        .get(..start)?
        .chars()
        .next_back()
        .is_some_and(is_identifier_continue)
    {
        return None;
    }
    let prefix = ["u8R\"", "LR\"", "uR\"", "UR\"", "R\""]
        .into_iter()
        .find(|prefix| rest.starts_with(prefix))?;
    let after_prefix = &rest[prefix.len()..];
    let open = after_prefix.find('(')?;
    if after_prefix[..open].contains_any_byte(b"\r\n") {
        return None;
    }
    let delimiter = &after_prefix[..open];
    let body_start = start + prefix.len() + open + 1;
    let end = closing_end(line, body_start, delimiter);
    Some(RawStringStart {
        delimiter: delimiter.to_string(),
        end,
    })
}

pub(crate) fn closing_end(line: &str, start: usize, delimiter: &str) -> Option<usize> {
    let closing = format!("){delimiter}\"");
    line.get(start..)?
        .find(&closing)
        .map(|offset| start + offset + closing.len())
}

pub(crate) fn end(line: &str, start: usize) -> Option<usize> {
    self::start(line, start).map(|raw| raw.end.unwrap_or(line.len()))
}
