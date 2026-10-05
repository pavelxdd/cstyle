use crate::formatter::engine::FormatEngine;
use crate::formatter::state::PreviousToken;

impl FormatEngine<'_> {
    pub(crate) fn ensure_space(&mut self) {
        self.current.ensure_space();
    }

    pub(crate) fn trim_current_end(&mut self) {
        self.current.trim_end_spaces();
    }

    pub(crate) fn trim_current_end_horizontal_space(&mut self) {
        self.current.trim_end_horizontal_space();
    }

    pub(crate) fn emit_source_space(&mut self) {
        if self.current.is_empty() {
            return;
        }
        if let Some(ws) = self.token_input.previous_input_whitespace.clone() {
            if !ws.is_empty() && self.current.ends_with(&*ws) {
                return;
            }
            self.trim_current_end();
            if !ws.is_empty() && self.current.ends_with(&*ws) {
                return;
            }
            self.current.push_str(&ws);
        } else {
            self.trim_current_end();
        }
    }

    pub(crate) fn emit_trailing_source_space(&mut self) {
        if let Some(ws) = self.token_input.next_input_whitespace.clone() {
            self.current.push_str(&ws);
        }
    }

    pub(crate) fn emit_source_space_or_ensure(&mut self) {
        if self.current.is_empty() {
            return;
        }
        match self.token_input.previous_input_whitespace.clone() {
            Some(ws) if !ws.is_empty() => {
                if self.current.ends_with(&*ws) {
                    return;
                }
                self.trim_current_end();
                self.current.push_str(&ws);
            }
            _ => self.ensure_space(),
        }
    }

    pub(crate) fn pad_inside_paren_space(&mut self) {
        if self.options.unpad_parens {
            let use_tab = self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_some_and(|whitespace| whitespace.ends_with('\t'));
            self.trim_current_end_horizontal_space();
            self.current.push(if use_tab { '\t' } else { ' ' });
        } else {
            self.emit_source_space_or_ensure();
        }
    }

    pub(crate) fn pad_before_open_paren_space(&mut self) {
        if self.options.unpad_parens {
            let use_tab = self.layout.previous != PreviousToken::OpenParen
                && self
                    .token_input
                    .previous_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.ends_with('\t'));
            self.trim_current_end_horizontal_space();
            self.current.push(if use_tab { '\t' } else { ' ' });
        } else {
            self.emit_source_space_or_ensure();
        }
    }

    pub(crate) fn emit_trailing_source_space_or_ensure(&mut self) {
        match self.token_input.next_input_whitespace.clone() {
            Some(ws) if !ws.is_empty() => self.current.push_str(&ws),
            _ => self.ensure_space(),
        }
    }
}
