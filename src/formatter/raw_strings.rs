use crate::source::lex::is_identifier_continue;

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct RawStringStart {
    pub(super) delimiter: String,
    pub(super) end: Option<usize>,
}

pub(super) fn start(line: &str, start: usize) -> Option<RawStringStart> {
    let rest = line.get(start..)?;
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
    if after_prefix[..open].contains(['\r', '\n']) {
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

pub(super) fn closing_end(line: &str, start: usize, delimiter: &str) -> Option<usize> {
    let closing = format!("){delimiter}\"");
    line.get(start..)?
        .find(&closing)
        .map(|offset| start + offset + closing.len())
}

pub(super) fn end(line: &str, start: usize) -> Option<usize> {
    self::start(line, start).map(|raw| raw.end.unwrap_or(line.len()))
}
