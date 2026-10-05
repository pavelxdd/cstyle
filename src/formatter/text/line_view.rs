//! A line being laid out, trimmed once: the layout rules read the same
//! line's trimmed forms hundreds of times.

use std::fmt;
use std::ops::Deref;

use crate::formatter::text::trim::Trimmed;

#[derive(Clone, Copy)]
pub(crate) struct LineView<'a> {
    text: &'a str,
    start: &'a str,
    trimmed: &'a str,
    end: &'a str,
}

impl<'a> LineView<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let start = text.trimmed_start();
        let trimmed = start.trimmed_end();
        let end = if trimmed.is_empty() {
            &text[..0]
        } else {
            &text[..text.len() - (start.len() - trimmed.len())]
        };
        Self {
            text,
            start,
            trimmed,
            end,
        }
    }

    pub(crate) fn trimmed_start(&self) -> &'a str {
        self.start
    }

    pub(crate) fn trimmed(&self) -> &'a str {
        self.trimmed
    }

    pub(crate) fn trimmed_end(&self) -> &'a str {
        self.end
    }
}

impl Deref for LineView<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        self.text
    }
}

impl AsRef<str> for LineView<'_> {
    fn as_ref(&self) -> &str {
        self.text
    }
}

impl fmt::Display for LineView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.text)
    }
}

impl fmt::Debug for LineView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.text, formatter)
    }
}

impl PartialEq<str> for LineView<'_> {
    fn eq(&self, other: &str) -> bool {
        self.text == other
    }
}

impl PartialEq<&str> for LineView<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.text == *other
    }
}

#[cfg(test)]
mod tests {
    use super::LineView;
    use crate::formatter::text::trim::Trimmed;

    #[test]
    fn trims_as_the_text_does() {
        for text in ["", " ", "  x  ", "x", "\tx y\t ", "  é  ", "x  ", "  x"] {
            let view = LineView::new(text);
            assert_eq!(view.trimmed_start(), text.trimmed_start(), "{text:?}");
            assert_eq!(view.trimmed(), text.trimmed(), "{text:?}");
            assert_eq!(view.trimmed_end(), text.trimmed_end(), "{text:?}");
        }
    }
}
