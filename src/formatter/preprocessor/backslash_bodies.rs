use crate::formatter::engine::FormatEngine;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::trim::Trimmed;

pub(crate) struct BackslashBodyState {
    may_have_input: bool,
    parts: Option<(Vec<String>, usize)>,
}

impl Default for BackslashBodyState {
    fn default() -> Self {
        Self {
            may_have_input: true,
            parts: None,
        }
    }
}

impl FormatEngine<'_> {
    pub(crate) fn set_may_have_backslash_body(&mut self, may_have_input: bool) {
        self.backslash_body.may_have_input = may_have_input;
    }

    pub(crate) fn try_emit_backslash_body(&mut self, line: &str) -> bool {
        if !self.backslash_body.may_have_input && self.backslash_body.parts.is_none() {
            return false;
        }
        let current = line.trimmed();
        if let Some((mut parts, indent)) = self.backslash_body.parts.take() {
            if current.starts_with('#') || current.starts_with("template") {
                self.backslash_body.parts = Some((parts, indent));
                self.flush_backslash_body_parts();
                return false;
            }
            if current == "}" {
                self.backslash_body.parts = Some((parts, indent));
                self.flush_backslash_body_parts();
            } else {
                let part = current
                    .trim_start_matches('{')
                    .trimmed()
                    .trim_end_matches('}')
                    .trimmed();
                if !part.is_empty() {
                    parts.push(part.to_string());
                }
                self.backslash_body.parts = Some((parts, indent));
            }
            return true;
        }
        if current.starts_with('{')
            && let Some(previous) = self.output.last_line_outside_comment()
            && previous.trimmed_end().ends_with('\\')
            && !line_opens_backslash_control_body(previous.trimmed_start())
        {
            let indent = leading_visual_width(previous, self.options.tab_width);
            let body = current
                .trim_start_matches('{')
                .trimmed()
                .trim_end_matches('}')
                .trimmed();
            let mut parts = Vec::new();
            if !body.is_empty() {
                parts.push(body.to_string());
            }
            self.backslash_body.parts = Some((parts, indent));
            return true;
        }
        let Some(split) = line.find("\\{") else {
            return false;
        };
        let raw_prefix = &line[..split];
        if !raw_prefix.contains(')')
            || raw_prefix.contains('"')
            || raw_prefix.contains('@')
            || raw_prefix.contains('#')
            || line_opens_backslash_control_body(raw_prefix.trimmed_start())
        {
            return false;
        }
        let indent = self
            .output
            .last_non_empty_scoped()
            .map(|line| leading_visual_width(line, self.options.tab_width))
            .unwrap_or(0);
        let prefix = raw_prefix.trimmed_start().trimmed_end();
        let body = line[split + 2..].trimmed().trim_end_matches('}').trimmed();
        self.push_output_line_spaces(&format!("{prefix} \\"), 0, indent);
        let mut parts = Vec::new();
        if !body.is_empty() {
            parts.push(body.to_string());
        }
        self.backslash_body.parts = Some((parts, indent));
        true
    }

    pub(crate) fn flush_backslash_body_parts(&mut self) {
        if let Some((parts, indent)) = self.backslash_body.parts.take() {
            self.push_output_line_spaces(&format!("{{ {} }}", parts.join(" ")), 0, indent);
        }
    }
}

fn line_opens_backslash_control_body(line: &str) -> bool {
    let head = line.trimmed_end().trim_end_matches('\\').trimmed_end();
    ["for", "if", "while", "switch", "else", "do"]
        .iter()
        .any(|keyword| {
            head == *keyword
                || head.starts_with(&format!("{keyword} "))
                || head.starts_with(&format!("{keyword}("))
        })
}
