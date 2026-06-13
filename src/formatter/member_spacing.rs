use super::FormatEngine;
use super::labels::is_standard_access_label;
use super::state::FormatterBraceType;
use crate::config::LineBetweenMembers;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum MemberSpacingBoundary {
    Field,
    Member,
    TopFunction,
}

fn member_semicolon_boundary(trimmed: &str) -> Option<MemberSpacingBoundary> {
    if trimmed.starts_with('}') || is_standard_access_label(trimmed) {
        return None;
    }
    if looks_like_function_header(trimmed) {
        Some(MemberSpacingBoundary::Member)
    } else {
        Some(MemberSpacingBoundary::Field)
    }
}

fn looks_like_function_header(trimmed: &str) -> bool {
    if trimmed.starts_with("typedef ") || trimmed.contains('=') {
        return false;
    }
    trimmed.contains('(') && trimmed.contains(')')
}

fn nested_type_start(trimmed: &str) -> bool {
    trimmed.starts_with("class ")
        || trimmed.starts_with("struct ")
        || trimmed.starts_with("union ")
        || trimmed.starts_with("enum ")
}

impl FormatEngine<'_> {
    pub(super) fn insert_member_spacing_before_line(&mut self, line: &str) {
        if self.options.line_between_members == LineBetweenMembers::None {
            return;
        }
        let Some(previous) = self.pending_member_spacing else {
            return;
        };
        let Some(current) = self.current_member_spacing_boundary(line) else {
            if self.line_clears_pending_member_spacing(line) {
                self.pending_member_spacing = None;
            }
            return;
        };
        if matches!(
            (previous, current, self.options.line_between_members),
            (
                MemberSpacingBoundary::Field,
                MemberSpacingBoundary::Field,
                LineBetweenMembers::Members
            )
        ) {
            self.pending_member_spacing = None;
            return;
        }
        if self
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|line| !line.trim().is_empty())
        {
            self.push_empty_line();
        }
        self.pending_member_spacing = None;
    }

    pub(super) fn observe_member_spacing_boundary(&mut self, line: &str) {
        if self.options.line_between_members == LineBetweenMembers::None {
            return;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return;
        }
        if trimmed.starts_with('}') {
            self.pending_member_spacing = if self.stack_state.last_closed_brace_type
                == Some(FormatterBraceType::Definition)
            {
                if self.in_member_container() {
                    Some(MemberSpacingBoundary::Member)
                } else if self.stack_state.brace_type_stack.is_empty() {
                    Some(MemberSpacingBoundary::TopFunction)
                } else {
                    None
                }
            } else {
                None
            };
            return;
        }
        if self.line_clears_pending_member_spacing(line) {
            self.pending_member_spacing = None;
            return;
        }
        if self.in_member_container()
            && trimmed.ends_with(';')
            && let Some(boundary) = member_semicolon_boundary(trimmed)
        {
            self.pending_member_spacing = Some(boundary);
        }
    }

    fn current_member_spacing_boundary(&self, line: &str) -> Option<MemberSpacingBoundary> {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with(['#', '{', '}'])
            || is_standard_access_label(trimmed)
            || nested_type_start(trimmed)
        {
            return None;
        }
        if self.in_member_container() {
            if trimmed.ends_with(';') {
                return member_semicolon_boundary(trimmed);
            }
            if looks_like_function_header(trimmed) {
                return Some(MemberSpacingBoundary::Member);
            }
            return None;
        }
        if self.pending_member_spacing == Some(MemberSpacingBoundary::TopFunction)
            && self.stack_state.brace_type_stack.is_empty()
            && looks_like_function_header(trimmed)
        {
            return Some(MemberSpacingBoundary::TopFunction);
        }
        None
    }

    fn line_clears_pending_member_spacing(&self, line: &str) -> bool {
        let trimmed = line.trim();
        trimmed.is_empty()
            || trimmed.starts_with(['#', '}'])
            || is_standard_access_label(trimmed)
            || nested_type_start(trimmed)
    }

    fn in_member_container(&self) -> bool {
        self.stack_state.brace_type_stack.iter().any(|brace_type| {
            matches!(
                brace_type,
                FormatterBraceType::Class
                    | FormatterBraceType::Interface
                    | FormatterBraceType::Struct
                    | FormatterBraceType::Union
            )
        })
    }
}
