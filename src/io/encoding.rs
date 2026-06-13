use crate::config::{FormatOptions, LineEnding};
use crate::formatter::format_c;
use crate::source::line_endings::{ObservedLineEnding, preferred_line_ending};
use std::io;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum TextEncoding {
    Utf8,
    Latin1,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct DecodedSource {
    text: String,
    encoding: TextEncoding,
    had_final_line_break: bool,
    observed_line_ending: ObservedLineEnding,
}

impl DecodedSource {
    pub(super) fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF])
            || bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00])
        {
            return Err(invalid_data("UTF-32 input is not supported"));
        }

        let (text, encoding) = if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            (decode_utf8(rest)?, TextEncoding::Utf8Bom)
        } else if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
            (
                decode_utf16(rest, u16::from_le_bytes)?,
                TextEncoding::Utf16Le,
            )
        } else if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
            (
                decode_utf16(rest, u16::from_be_bytes)?,
                TextEncoding::Utf16Be,
            )
        } else {
            match std::str::from_utf8(bytes) {
                Ok(text) => (text.to_string(), TextEncoding::Utf8),
                Err(_) => (decode_latin1(bytes), TextEncoding::Latin1),
            }
        };
        let had_final_line_break = text.ends_with('\n') || text.ends_with('\r');
        let observed_line_ending = preferred_line_ending(&text);
        Ok(Self {
            text,
            encoding,
            had_final_line_break,
            observed_line_ending,
        })
    }

    pub(super) fn format(&self, options: &FormatOptions) -> Vec<u8> {
        let options = effective_line_ending_options(options, self.observed_line_ending);
        let mut output = format_c(&self.text, &options);
        if !self.had_final_line_break {
            let line_break = options.line_break();
            if output.ends_with(line_break) {
                output.truncate(output.len() - line_break.len());
            }
        }
        encode_output(&output, self.encoding)
    }
}

fn effective_line_ending_options(
    options: &FormatOptions,
    observed_line_ending: ObservedLineEnding,
) -> FormatOptions {
    let mut options = options.clone();
    if options.line_ending == LineEnding::Preserve {
        options.line_ending = match observed_line_ending {
            ObservedLineEnding::CrLf => LineEnding::Crlf,
            ObservedLineEnding::Cr => LineEnding::Cr,
            ObservedLineEnding::None | ObservedLineEnding::Lf => LineEnding::Lf,
        };
    }
    options
}

fn decode_utf8(bytes: &[u8]) -> io::Result<String> {
    std::str::from_utf8(bytes)
        .map(str::to_string)
        .map_err(|error| invalid_data(format!("invalid UTF-8 input: {error}")))
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&byte| byte as char).collect()
}

fn decode_utf16(bytes: &[u8], convert: fn([u8; 2]) -> u16) -> io::Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(invalid_data("invalid UTF-16 input length"));
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| convert([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units)
        .map_err(|error| invalid_data(format!("invalid UTF-16 input: {error}")))
}

fn encode_output(text: &str, encoding: TextEncoding) -> Vec<u8> {
    match encoding {
        TextEncoding::Utf8 => text.as_bytes().to_vec(),
        TextEncoding::Latin1 => text.chars().map(|ch| ch as u8).collect(),
        TextEncoding::Utf8Bom => {
            let mut output = vec![0xEF, 0xBB, 0xBF];
            output.extend_from_slice(text.as_bytes());
            output
        }
        TextEncoding::Utf16Le => {
            let mut output = vec![0xFF, 0xFE];
            for unit in text.encode_utf16() {
                output.extend_from_slice(&unit.to_le_bytes());
            }
            output
        }
        TextEncoding::Utf16Be => {
            let mut output = vec![0xFE, 0xFF];
            for unit in text.encode_utf16() {
                output.extend_from_slice(&unit.to_be_bytes());
            }
            output
        }
    }
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
