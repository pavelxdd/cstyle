use crate::config::FormatOptions;
use crate::formatter::constructs::labels::is_standard_access_label;
use crate::formatter::engine::FormatEngine;
use crate::formatter::state::frame::PointerRole;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;

impl FormatEngine<'_> {
    pub(crate) fn typedef_template_context_indent_spaces(&self, current: &str) -> Option<usize> {
        if current.is_empty() || current.starts_with('#') {
            return None;
        }
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        for index in (0..self.output.len()).rev().take(32) {
            let trimmed = self.output.trimmed(index);
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.ends_with(';')
                || trimmed == "{"
                || trimmed == "}"
                || is_standard_access_label(trimmed)
            {
                break;
            }
            if trimmed.starts_with("typedef typename ")
                && trimmed.contains('<')
                && trimmed.ends_with(',')
            {
                return Some(self.output.lead_width(index, tab_width) + width * 2);
            }
            if trimmed.starts_with("typedef typename ") {
                return Some(self.output.lead_width(index, tab_width));
            }
            if trimmed.starts_with("typedef ") && trimmed.contains('<') && trimmed.ends_with(',') {
                return Some(self.output.lead_width(index, tab_width) + width * 2);
            }
        }
        None
    }

    pub(crate) fn typedef_function_pointer_frame_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if current.is_empty() || current.starts_with('#') {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .active_typedef_function_pointer_declaration()?;
        if current.starts_with(");") {
            frame.closing_anchor_column
        } else {
            frame.continuation_anchor_column
        }
    }

    pub(crate) fn update_typedef_function_pointer_frame(&mut self, line: &LineView<'_>) {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let trimmed = line.trimmed_start();
        if leading_visual_width(line, tab_width) == 0
            && trimmed.starts_with("typedef ")
            && trimmed.contains("(*")
            && trimmed.contains(") (")
            && !trimmed.contains('\\')
            && self.open_paren_column_of(trimmed).is_some()
        {
            let target = if trimmed.trimmed_end().ends_with(',') {
                self.open_paren_column_of(trimmed.trimmed_end())
                    .map_or(width, |open| {
                        let column = visual_width_from(&trimmed[..open + 1], 0, tab_width);
                        if column > self.options.max_continuation_indent {
                            width * 2
                        } else {
                            column
                        }
                    })
            } else {
                width
            };
            let has_typedef_frame = self
                .layout
                .frame_stack
                .active_typedef_function_pointer_declaration()
                .is_some();
            let frame = if has_typedef_frame {
                self.layout
                    .frame_stack
                    .active_typedef_function_pointer_declaration_mut()
            } else {
                self.layout.frame_stack.active_declaration_mut()
            };
            if let Some(frame) = frame {
                frame.pointer_role = PointerRole::FunctionPointer;
                frame.is_typedef = true;
                frame.continuation_anchor_column = Some(target);
                frame.closing_anchor_column = Some(target.saturating_sub(width));
            }
        } else if trimmed.starts_with(");") || trimmed.ends_with(';') {
            self.layout.frame_stack.clear_declarations();
        }
    }
}

pub(crate) fn immediate_typedef_template_indent_spaces(
    options: &FormatOptions,
    current: &str,
    previous: &str,
) -> Option<usize> {
    let previous_trimmed = previous.trimmed();
    if current.starts_with('<') && previous_trimmed.starts_with("typedef typename ") {
        return Some(leading_visual_width(previous, options.tab_width));
    }
    (previous_trimmed.starts_with("typedef ")
        && previous_trimmed.contains('<')
        && previous_trimmed.ends_with(','))
    .then(|| leading_visual_width(previous, options.tab_width) + options.indent_width * 2)
}
