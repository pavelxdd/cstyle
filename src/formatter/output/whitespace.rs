use super::super::FormatEngine;
use super::super::state::PreviousToken;
impl FormatEngine<'_> {
    pub(in super::super) fn ensure_space(&mut self) {
        self.current.ensure_space();
    }

    pub(in super::super) fn trim_current_end(&mut self) {
        self.current.trim_end_spaces();
    }

    pub(in super::super) fn trim_current_end_horizontal_space(&mut self) {
        self.current.trim_end_horizontal_space();
    }

    pub(in super::super) fn emit_source_space(&mut self) {
        if self.current.is_empty() {
            return;
        }
        if let Some(ws) = self.token_input.previous_input_whitespace.clone() {
            if !ws.is_empty() && self.current.ends_with(&ws) {
                return;
            }
            self.trim_current_end();
            if !ws.is_empty() && self.current.ends_with(&ws) {
                return;
            }
            self.current.push_str(&ws);
        } else {
            self.trim_current_end();
        }
    }

    pub(in super::super) fn emit_trailing_source_space(&mut self) {
        if let Some(ws) = self.token_input.next_input_whitespace.clone() {
            self.current.push_str(&ws);
        }
    }

    pub(in super::super) fn emit_source_space_or_ensure(&mut self) {
        if self.current.is_empty() {
            return;
        }
        match self.token_input.previous_input_whitespace.clone() {
            Some(ws) if !ws.is_empty() => {
                if self.current.ends_with(&ws) {
                    return;
                }
                self.trim_current_end();
                self.current.push_str(&ws);
            }
            _ => self.ensure_space(),
        }
    }

    pub(in super::super) fn pad_inside_paren_space(&mut self) {
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

    pub(in super::super) fn pad_before_open_paren_space(&mut self) {
        if self.options.unpad_parens {
            let use_tab = self.previous != PreviousToken::OpenParen
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

    pub(in super::super) fn emit_trailing_source_space_or_ensure(&mut self) {
        match self.token_input.next_input_whitespace.clone() {
            Some(ws) if !ws.is_empty() => self.current.push_str(&ws),
            _ => self.ensure_space(),
        }
    }
}
