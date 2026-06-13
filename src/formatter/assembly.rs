pub(super) fn is_asm_block_header(word: &str) -> bool {
    matches!(word, "_asm" | "__asm")
}

#[derive(Default)]
pub(super) struct AssemblyMacroLines {
    active: bool,
    preserve_leading: bool,
    after_preprocessor: bool,
    current_indent_spaces: usize,
}

impl AssemblyMacroLines {
    pub(super) fn take_raw_line(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim_start();
        if self.active && trimmed.starts_with('#') {
            self.after_preprocessor = true;
            return None;
        }
        if line.is_empty() || !(self.active || is_macro_start(trimmed)) {
            return None;
        }

        let output = format_macro_line(
            line,
            self.preserve_leading,
            self.after_preprocessor
                .then_some(self.current_indent_spaces),
        );
        self.current_indent_spaces = output.chars().take_while(|&ch| ch == ' ').count();
        if is_macro_start(trimmed) {
            self.active = true;
        }
        if trimmed.starts_with('@') {
            self.preserve_leading = true;
        }
        if is_macro_end(trimmed) {
            self.active = false;
        }
        self.after_preprocessor = false;
        Some(output)
    }

    pub(super) fn observe_preprocessor(&mut self) {
        if self.active {
            self.after_preprocessor = true;
        }
    }
}

fn is_macro_start(trimmed: &str) -> bool {
    trimmed
        .strip_prefix(".macro")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn is_macro_end(trimmed: &str) -> bool {
    trimmed
        .strip_prefix(".endm")
        .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
}

fn format_macro_line(
    line: &str,
    preserve_leading: bool,
    after_preprocessor_indent: Option<usize>,
) -> String {
    let trimmed = line.trim_start();
    if !preserve_leading {
        return trimmed.to_string();
    }
    let leading = &line[..line.len() - trimmed.len()];
    let leading_width = leading.chars().fold(0usize, |column, ch| {
        if ch == '\t' {
            column + (8 - column % 8)
        } else {
            column + 1
        }
    });
    if let Some(indent) = after_preprocessor_indent
        && leading_width == 0
        && indent > 0
    {
        format!("{}{}", " ".repeat(indent), trimmed)
    } else if leading.contains('\t') {
        format!("{}{}", " ".repeat(leading_width), trimmed)
    } else {
        trimmed.to_string()
    }
}

impl FormatEngine<'_> {
    pub(super) fn is_in_asm_operator_context(&self) -> bool {
        let current = self.current.trim_start();
        ((current.starts_with("asm(") || current.starts_with("__asm__("))
            && self.stack_state.paren_depth > 0)
            || current.starts_with("_asm ")
            || current.starts_with("__asm ")
    }
}
use super::FormatEngine;
