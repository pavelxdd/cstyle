use crate::formatter::engine::FormatEngine;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;

pub(crate) struct SwigState {
    may_have_input: bool,
    pending_typemap_line: Option<(String, usize)>,
    pythoncode_indent_spaces: Option<usize>,
}

impl Default for SwigState {
    fn default() -> Self {
        Self {
            may_have_input: true,
            pending_typemap_line: None,
            pythoncode_indent_spaces: None,
        }
    }
}

impl FormatEngine<'_> {
    pub(crate) fn set_may_have_swig(&mut self, may_have_input: bool) {
        self.swig.may_have_input = may_have_input;
    }

    pub(crate) fn try_emit_swig_line(&mut self, line: &LineView<'_>) -> bool {
        if !self.swig.may_have_input
            && self.swig.pending_typemap_line.is_none()
            && self.swig.pythoncode_indent_spaces.is_none()
        {
            return false;
        }
        let current = line.trimmed_start();
        if let Some((mut pending, spaces)) = self.swig.pending_typemap_line.take() {
            let part = current.trimmed();
            if !part.is_empty() {
                pending.push(' ');
                pending.push_str(part);
            }
            if part == "}" || part.ends_with('}') {
                self.push_output_line_spaces(pending.trimmed_end(), 0, spaces);
            } else {
                self.swig.pending_typemap_line = Some((pending, spaces));
            }
            return true;
        }
        if let Some(spaces) = self.swig.pythoncode_indent_spaces {
            if current == "}" {
                self.swig.pythoncode_indent_spaces = None;
                self.push_output_line_spaces(current, 0, spaces);
            } else {
                self.push_output_line_spaces(current, 1, spaces + self.options.indent_width);
            }
            return true;
        }
        if current.starts_with("%typemap") && current.contains('{') && !current.contains('}') {
            let spaces = self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                * self.options.indent_width;
            self.swig.pending_typemap_line = Some((current.trimmed_end().to_string(), spaces));
            return true;
        }
        if current.starts_with("%pythoncode") && current.ends_with('{') {
            let spaces = self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                * self.options.indent_width;
            self.swig.pythoncode_indent_spaces = Some(spaces);
            self.push_output_line_spaces(current, 0, spaces);
            return true;
        }
        false
    }
}
