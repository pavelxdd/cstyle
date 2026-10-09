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
pub(crate) struct DecodedSource {
    text: String,
    encoding: TextEncoding,
    had_final_line_break: bool,
    observed_line_ending: ObservedLineEnding,
}

impl DecodedSource {
    /// Decodes `bytes`; UTF-8 input becomes the text as it is.
    pub(crate) fn from_vec(bytes: Vec<u8>) -> io::Result<Self> {
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
            match String::from_utf8(bytes) {
                Ok(text) => (text, TextEncoding::Utf8),
                Err(error) => (decode_latin1(error.as_bytes()), TextEncoding::Latin1),
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

    /// Takes the text out, leaving none.
    pub(crate) fn take_text(&mut self) -> String {
        std::mem::take(&mut self.text)
    }

    pub(crate) fn had_final_line_break(&self) -> bool {
        self.had_final_line_break
    }

    pub(crate) fn observed_line_ending(&self) -> ObservedLineEnding {
        self.observed_line_ending
    }

    /// Encodes `text` with the byte encoding detected in the input.
    pub(crate) fn encode(&self, text: String) -> Vec<u8> {
        match self.encoding {
            TextEncoding::Utf8 => text.into_bytes(),
            encoding => encode_output(&text, encoding),
        }
    }
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
        .as_chunks::<2>()
        .0
        .iter()
        .map(|chunk| convert(*chunk))
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

#[cfg(test)]
mod tests {
    use super::DecodedSource;
    use std::io;

    #[test]
    fn decodes_utf16_bodies_at_unit_and_surrogate_boundaries() {
        let cases: [(&[u8], &str); 6] = [
            (&[0xFF, 0xFE], ""),
            (&[0xFF, 0xFE, 0x41, 0x00], "A"),
            (&[0xFF, 0xFE, 0x3D, 0xD8, 0x00, 0xDE], "\u{1F600}"),
            (&[0xFE, 0xFF], ""),
            (&[0xFE, 0xFF, 0x00, 0x41], "A"),
            (&[0xFE, 0xFF, 0xD8, 0x3D, 0xDE, 0x00], "\u{1F600}"),
        ];
        for (bytes, expected) in cases {
            let mut decoded = DecodedSource::from_vec(bytes.to_vec()).expect("valid UTF-16");
            assert_eq!(decoded.take_text(), expected, "{bytes:02x?}");
        }
    }

    #[test]
    fn rejects_utf16_bodies_with_an_odd_byte_count() {
        for bytes in [&[0xFF, 0xFE, 0x41][..], &[0xFE, 0xFF, 0x00][..]] {
            let error = DecodedSource::from_vec(bytes.to_vec()).expect_err("odd UTF-16 body");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        }
    }
}
