//! A line being laid out, trimmed once: the layout rules read the same
//! line's trimmed forms hundreds of times.

use std::cell::OnceCell;
use std::fmt;
use std::ops::Deref;

use crate::config::FormatOptions;
use crate::formatter::braces::classification::line_opens_lambda_block;
use crate::formatter::continuation::call_arguments::line_opens_attachable_lambda_block;
use crate::formatter::text::line_scan::line_paren_imbalance;
use crate::formatter::text::trim::Trimmed;

#[derive(Clone)]
pub(crate) struct LineView<'a> {
    text: &'a str,
    start: &'a str,
    trimmed: &'a str,
    end: &'a str,
    /// Unmatched `)`/`]` and the columns of unmatched `(`/`[`, read once a
    /// rule asks.
    parens: OnceCell<(usize, Vec<usize>)>,
    /// Whether the line opens a lambda's body, and one the style attaches.
    opens_lambda: OnceCell<bool>,
    opens_attachable_lambda: OnceCell<bool>,
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
            parens: OnceCell::new(),
            opens_lambda: OnceCell::new(),
            opens_attachable_lambda: OnceCell::new(),
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

    /// `line_opens_lambda_block` of the line.
    pub(crate) fn opens_lambda_block(&self) -> bool {
        *self
            .opens_lambda
            .get_or_init(|| line_opens_lambda_block(self.text))
    }

    /// `line_opens_attachable_lambda_block` of the line under `options`,
    /// which stay the same for every line.
    pub(crate) fn opens_attachable_lambda_block(&self, options: &FormatOptions) -> bool {
        *self
            .opens_attachable_lambda
            .get_or_init(|| line_opens_attachable_lambda_block(options, self.text))
    }

    /// `line_paren_imbalance` of the line.
    pub(crate) fn paren_imbalance(&self) -> (usize, &[usize]) {
        let (closes, opens) = self.parens.get_or_init(|| line_paren_imbalance(self.text));
        (*closes, opens)
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
