//! Token text: a range of the shared source text, so lexing allocates no
//! string per token.

use std::fmt;
use std::ops::Deref;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct TokenText {
    source: Rc<String>,
    start: u32,
    end: u32,
}

impl TokenText {
    /// The text `source[start..end]`.
    pub(crate) fn slice(source: &Rc<String>, start: usize, end: usize) -> Self {
        Self {
            source: Rc::clone(source),
            start: u32::try_from(start).expect("source fits in u32"),
            end: u32::try_from(end).expect("source fits in u32"),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.source[self.start as usize..self.end as usize]
    }
}

impl Deref for TokenText {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for TokenText {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for TokenText {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for TokenText {
    fn from(text: &str) -> Self {
        let source = Rc::new(text.to_owned());
        Self::slice(&source, 0, text.len())
    }
}

impl From<String> for TokenText {
    fn from(text: String) -> Self {
        let end = text.len();
        Self::slice(&Rc::new(text), 0, end)
    }
}

impl From<&String> for TokenText {
    fn from(text: &String) -> Self {
        Self::from(text.as_str())
    }
}

impl From<TokenText> for String {
    fn from(text: TokenText) -> Self {
        text.as_str().to_owned()
    }
}

impl PartialEq for TokenText {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for TokenText {}

impl PartialEq<str> for TokenText {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for TokenText {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for TokenText {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<TokenText> for str {
    fn eq(&self, other: &TokenText) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<TokenText> for &str {
    fn eq(&self, other: &TokenText) -> bool {
        *self == other.as_str()
    }
}

impl std::hash::Hash for TokenText {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl fmt::Debug for TokenText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), formatter)
    }
}

impl fmt::Display for TokenText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), formatter)
    }
}

impl PartialEq<TokenText> for String {
    fn eq(&self, other: &TokenText) -> bool {
        self.as_str() == other.as_str()
    }
}
