use super::FormatEngine;
use super::columns::{leading_visual_width, visual_width_from};
use super::frame::PointerRole;
use super::labels::is_standard_access_label;
use super::line_scan::unmatched_open_paren_column;

impl FormatEngine<'_> {
    pub(super) fn immediate_typedef_template_indent_spaces(
        &self,
        current: &str,
        previous: &str,
    ) -> Option<usize> {
        let previous_trimmed = previous.trim();
        if current.starts_with('<') && previous_trimmed.starts_with("typedef typename ") {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        (previous_trimmed.starts_with("typedef ")
            && previous_trimmed.contains('<')
            && previous_trimmed.ends_with(','))
        .then(|| {
            leading_visual_width(previous, self.options.tab_width) + self.options.indent_width * 2
        })
    }

    pub(super) fn typedef_template_context_indent_spaces(&self, current: &str) -> Option<usize> {
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

    pub(super) fn typedef_function_pointer_frame_indent_spaces(
        &self,
        current: &str,
    ) -> Option<usize> {
        if current.is_empty() || current.starts_with('#') {
            return None;
        }
        let frame = self
            .frame_stack
            .active_typedef_function_pointer_declaration()?;
        if current.starts_with(");") {
            frame.closing_anchor_column
        } else {
            frame.continuation_anchor_column
        }
    }

    pub(super) fn update_typedef_function_pointer_frame(&mut self, line: &str) {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let trimmed = line.trim_start();
        if leading_visual_width(line, tab_width) == 0
            && trimmed.starts_with("typedef ")
            && trimmed.contains("(*")
            && trimmed.contains(") (")
            && !trimmed.contains('\\')
            && unmatched_open_paren_column(trimmed).is_some()
        {
            let target = if trimmed.trim_end().ends_with(',') {
                unmatched_open_paren_column(trimmed.trim_end()).map_or(width, |open| {
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
                .frame_stack
                .active_typedef_function_pointer_declaration()
                .is_some();
            let frame = if has_typedef_frame {
                self.frame_stack
                    .active_typedef_function_pointer_declaration_mut()
            } else {
                self.frame_stack.active_declaration_mut()
            };
            if let Some(frame) = frame {
                frame.pointer_role = PointerRole::FunctionPointer;
                frame.is_typedef = true;
                frame.continuation_anchor_column = Some(target);
                frame.closing_anchor_column = Some(target.saturating_sub(width));
            }
        } else if trimmed.starts_with(");") || trimmed.ends_with(';') {
            self.frame_stack.clear_declarations();
        }
    }
}
