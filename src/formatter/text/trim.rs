//! Trimming kept out of line: the formatter trims its lines in thousands of
//! places, and the trim loop inlined at each of them would crowd the
//! instruction cache.

pub(crate) trait Trimmed {
    fn trimmed(&self) -> &str;
    fn trimmed_start(&self) -> &str;
    fn trimmed_end(&self) -> &str;
}

impl Trimmed for str {
    #[inline(never)]
    fn trimmed(&self) -> &str {
        self[leading_whitespace(self.as_bytes())..].trim_ascii_end()
    }

    #[inline(never)]
    fn trimmed_start(&self) -> &str {
        &self[leading_whitespace(self.as_bytes())..]
    }

    #[inline(never)]
    fn trimmed_end(&self) -> &str {
        self.trim_ascii_end()
    }
}

/// The length of the ASCII whitespace `bytes` start with; an indent of
/// spaces is measured eight bytes at a time.
fn leading_whitespace(bytes: &[u8]) -> usize {
    if !bytes.first().is_some_and(u8::is_ascii_whitespace) {
        return 0;
    }
    let mut index = leading_spaces(bytes);
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

/// The number of spaces `bytes` start with, read eight bytes at a time.
pub(crate) fn leading_spaces(bytes: &[u8]) -> usize {
    const SPACES: u64 = u64::from_le_bytes([b' '; 8]);
    let (chunks, remainder) = bytes.as_chunks::<8>();
    let mut index = 0;
    for chunk in chunks {
        // The first byte that is no space sets the lowest differing bits.
        let differing = u64::from_le_bytes(*chunk) ^ SPACES;
        if differing != 0 {
            return index + differing.trailing_zeros() as usize / 8;
        }
        index += 8;
    }
    index + remainder.iter().take_while(|&&byte| byte == b' ').count()
}

#[cfg(test)]
mod tests {
    use super::Trimmed;

    #[test]
    fn leading_spaces_counts_spaces_at_every_length() {
        for len in 0..=40 {
            for spaces in 0..=len {
                let mut text = vec![b' '; spaces];
                text.resize(len, b'x');
                assert_eq!(super::leading_spaces(&text), spaces, "{len} {spaces}");
            }
        }
    }

    #[test]
    fn trims_as_the_standard_library_does() {
        for text in [
            "",
            " ",
            "        ",
            "         x ",
            "\t\t  x\n",
            "x",
            "                 \r\nab  \t",
            "        \u{a0}x",
            "  é ",
            "    if (x)",
            "            \tx",
            "                x",
            "       \t        x",
        ] {
            assert_eq!(text.trimmed_start(), text.trim_ascii_start(), "{text:?}");
            assert_eq!(text.trimmed(), text.trim_ascii(), "{text:?}");
            assert_eq!(text.trimmed_end(), text.trim_ascii_end(), "{text:?}");
        }
    }
}
