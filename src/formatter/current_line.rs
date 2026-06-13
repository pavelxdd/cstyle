use super::columns::visual_width_from;
use std::cell::Cell;
use std::ops::Deref;

#[derive(Default)]
pub(super) struct CurrentLine {
    text: String,
    char_len: Cell<Option<(usize, usize)>>,
    open_brace_run_len: Cell<Option<usize>>,
    blank: Cell<Option<(usize, bool)>>,
    visual_width: Cell<Option<(usize, usize)>>,
    visual_width_from: Cell<Option<(usize, usize, usize)>>,
    last_open_brace: Cell<Option<(usize, Option<usize>)>>,
    trailing_comment: Cell<Option<TrailingCommentScan>>,
}

impl CurrentLine {
    pub(super) fn as_str(&self) -> &str {
        &self.text
    }

    pub(super) fn push(&mut self, ch: char) {
        self.text.push(ch);
    }

    pub(super) fn push_str(&mut self, text: &str) {
        self.text.push_str(text);
    }

    pub(super) fn pop(&mut self) -> Option<char> {
        let popped = self.text.pop();
        if popped.is_some() {
            self.invalidate();
        }
        popped
    }

    pub(super) fn truncate(&mut self, new_len: usize) {
        if new_len >= self.text.len() {
            return;
        }
        self.text.truncate(new_len);
        self.invalidate();
    }

    pub(super) fn insert(&mut self, index: usize, ch: char) {
        self.text.insert(index, ch);
        self.invalidate();
    }

    pub(super) fn clear(&mut self) {
        if self.text.is_empty() {
            return;
        }
        self.text.clear();
        self.invalidate();
    }

    pub(super) fn replace(&mut self, text: String) {
        self.text = text;
        self.invalidate();
    }

    pub(super) fn take(&mut self) -> String {
        self.invalidate();
        std::mem::take(&mut self.text)
    }

    pub(super) fn ensure_space(&mut self) {
        if !self.text.is_empty() && !self.text.ends_with(' ') {
            self.text.push(' ');
        }
    }

    pub(super) fn trim_end_spaces(&mut self) {
        self.trim_end_matching(|ch| ch == ' ');
    }

    pub(super) fn trim_end_horizontal_space(&mut self) {
        self.trim_end_matching(|ch| matches!(ch, ' ' | '\t'));
    }

    fn trim_end_matching(&mut self, matches: impl Fn(char) -> bool) {
        let Some(last) = self.text.chars().next_back() else {
            return;
        };
        if !matches(last) {
            return;
        }
        let before_chars = self.char_len();
        let mut popped = 0;
        while self.text.chars().next_back().is_some_and(&matches) {
            self.text.pop();
            popped += 1;
        }
        self.invalidate();
        self.char_len
            .set(Some((self.text.len(), before_chars - popped)));
    }

    pub(super) fn char_len(&self) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_chars)) = self.char_len.get() {
            if cached_bytes == len {
                return cached_chars;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let total = cached_chars + self.text[cached_bytes..].chars().count();
                debug_assert_eq!(total, self.text.chars().count());
                self.char_len.set(Some((len, total)));
                return total;
            }
        }
        let total = self.text.chars().count();
        self.char_len.set(Some((len, total)));
        total
    }

    pub(super) fn is_blank(&self) -> bool {
        let len = self.text.len();
        if let Some((cached_bytes, cached_blank)) = self.blank.get()
            && cached_bytes == len
        {
            return cached_blank;
        }
        let blank = self.text.trim().is_empty();
        self.blank.set(Some((len, blank)));
        blank
    }

    pub(super) fn visual_width(&self, tab_width: usize) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_width)) = self.visual_width.get() {
            if cached_bytes == len {
                return cached_width;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let width = cached_width
                    + visual_width_from(&self.text[cached_bytes..], cached_width, tab_width);
                self.visual_width.set(Some((len, width)));
                return width;
            }
        }
        let width = visual_width_from(&self.text, 0, tab_width);
        self.visual_width.set(Some((len, width)));
        width
    }

    pub(super) fn visual_width_from(&self, start_column: usize, tab_width: usize) -> usize {
        let len = self.text.len();
        if let Some((cached_bytes, cached_start, cached_width)) = self.visual_width_from.get()
            && cached_start == start_column
        {
            if cached_bytes == len {
                return cached_width;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let width = cached_width
                    + visual_width_from(
                        &self.text[cached_bytes..],
                        start_column + cached_width,
                        tab_width,
                    );
                self.visual_width_from.set(Some((len, start_column, width)));
                return width;
            }
        }
        let width = visual_width_from(&self.text, start_column, tab_width);
        self.visual_width_from.set(Some((len, start_column, width)));
        width
    }

    pub(super) fn last_open_brace(&self) -> Option<usize> {
        let len = self.text.len();
        if let Some((cached_bytes, cached_index)) = self.last_open_brace.get() {
            if cached_bytes == len {
                return cached_index;
            }
            if cached_bytes < len && self.text.is_char_boundary(cached_bytes) {
                let index = self.text[cached_bytes..]
                    .rfind('{')
                    .map(|index| cached_bytes + index)
                    .or(cached_index);
                self.last_open_brace.set(Some((len, index)));
                return index;
            }
        }
        let index = self.text.rfind('{');
        self.last_open_brace.set(Some((len, index)));
        index
    }

    pub(super) fn trailing_comment_split_limit(&self) -> usize {
        let len = self.text.len();
        let mut scan = match self.trailing_comment.get() {
            Some(scan) if scan.scanned <= len && self.text.is_char_boundary(scan.scanned) => scan,
            _ => TrailingCommentScan::default(),
        };
        if scan.comment_start.is_none() {
            scan.advance(self.text.as_bytes());
            self.trailing_comment.set(Some(scan));
        }
        match scan.comment_start {
            Some(index) => self.text[..index].trim_end().len(),
            None => len,
        }
    }

    pub(super) fn is_open_brace_run(&self) -> bool {
        if self.open_brace_run_len.get() == Some(self.text.len()) {
            return true;
        }
        let trimmed = self.text.trim_start();
        !trimmed.is_empty()
            && trimmed
                .chars()
                .all(|ch| ch == '{' || ch == ' ' || ch == '\t')
    }

    pub(super) fn mark_open_brace_run(&self) {
        self.open_brace_run_len.set(Some(self.text.len()));
    }

    fn invalidate(&self) {
        self.char_len.set(None);
        self.open_brace_run_len.set(None);
        self.blank.set(None);
        self.visual_width.set(None);
        self.visual_width_from.set(None);
        self.last_open_brace.set(None);
        self.trailing_comment.set(None);
    }
}

impl Deref for CurrentLine {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.text
    }
}

#[derive(Clone, Copy, Default)]
struct TrailingCommentScan {
    scanned: usize,
    quote: Option<u8>,
    escaped: bool,
    comment_start: Option<usize>,
}

impl TrailingCommentScan {
    fn advance(&mut self, bytes: &[u8]) {
        let mut index = self.scanned;
        while index < bytes.len() {
            let ch = bytes[index];
            if let Some(quote_char) = self.quote {
                if self.escaped {
                    self.escaped = false;
                } else if ch == b'\\' {
                    self.escaped = true;
                } else if ch == quote_char {
                    self.quote = None;
                }
                index += 1;
                continue;
            }
            match ch {
                b'"' | b'\'' => self.quote = Some(ch),
                b'/' => match bytes.get(index + 1) {
                    Some(b'/') | Some(b'*') => {
                        self.comment_start = Some(index);
                        self.scanned = index;
                        return;
                    }
                    None => {
                        self.scanned = index;
                        return;
                    }
                    _ => {}
                },
                _ => {}
            }
            index += 1;
        }
        self.scanned = bytes.len();
    }
}

#[cfg(test)]
mod tests {
    use super::CurrentLine;

    #[test]
    fn blank_cache_rechecks_after_trim_and_same_length_push() {
        let mut current = CurrentLine::default();
        current.push(' ');
        assert!(current.is_blank());

        current.trim_end_spaces();
        current.push('x');

        assert!(!current.is_blank());
    }

    #[test]
    fn blank_cache_rechecks_after_same_length_replacement() {
        let mut current = CurrentLine::default();
        current.push(' ');
        assert!(current.is_blank());

        current.pop();
        current.push('x');

        assert!(!current.is_blank());
    }
}
