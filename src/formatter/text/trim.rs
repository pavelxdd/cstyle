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
        self.trim_ascii()
    }

    #[inline(never)]
    fn trimmed_start(&self) -> &str {
        self.trim_ascii_start()
    }

    #[inline(never)]
    fn trimmed_end(&self) -> &str {
        self.trim_ascii_end()
    }
}
