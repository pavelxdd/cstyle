use crate::config::LineBetweenMembers;
use crate::formatter::constructs::labels::is_standard_access_label;
use crate::formatter::engine::FormatEngine;
use crate::formatter::state::BraceType;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum MemberSpacingBoundary {
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
    pub(crate) fn insert_member_spacing_before_line(&mut self, line: &str) {
        if self.options.line_between_members == LineBetweenMembers::None {
            return;
        }
        let Some(previous) = self.layout.pending_member_spacing else {
            return;
        };
        let Some(current) = self.current_member_spacing_boundary(line) else {
            if line_clears_pending_member_spacing(line) {
                self.layout.pending_member_spacing = None;
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
            self.layout.pending_member_spacing = None;
            return;
        }
        if self
            .layout
            .previous_pre_adjust_line
            .as_deref()
            .is_some_and(|line| !line.trim_ascii().is_empty())
        {
            self.push_empty_line();
        }
        self.layout.pending_member_spacing = None;
    }

    pub(super) fn observe_member_spacing_boundary(&mut self, line: &str) {
        if self.options.line_between_members == LineBetweenMembers::None {
            return;
        }
        let trimmed = line.trim_ascii();
        if trimmed.is_empty() {
            return;
        }
        if trimmed.starts_with('}') {
            self.layout.pending_member_spacing =
                if self.layout.nesting.last_closed_brace_type == Some(BraceType::Definition) {
                    if self.in_member_container() {
                        Some(MemberSpacingBoundary::Member)
                    } else if self.layout.nesting.brace_type_stack.is_empty() {
                        Some(MemberSpacingBoundary::TopFunction)
                    } else {
                        None
                    }
                } else {
                    None
                };
            return;
        }
        if line_clears_pending_member_spacing(line) {
            self.layout.pending_member_spacing = None;
            return;
        }
        if self.in_member_container()
            && trimmed.ends_with(';')
            && let Some(boundary) = member_semicolon_boundary(trimmed)
        {
            self.layout.pending_member_spacing = Some(boundary);
        }
    }

    fn current_member_spacing_boundary(&self, line: &str) -> Option<MemberSpacingBoundary> {
        let trimmed = line.trim_ascii();
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
        if self.layout.pending_member_spacing == Some(MemberSpacingBoundary::TopFunction)
            && self.layout.nesting.brace_type_stack.is_empty()
            && looks_like_function_header(trimmed)
        {
            return Some(MemberSpacingBoundary::TopFunction);
        }
        None
    }

    fn in_member_container(&self) -> bool {
        self.layout
            .nesting
            .brace_type_stack
            .iter()
            .any(|brace_type| {
                matches!(
                    brace_type,
                    BraceType::Class | BraceType::Interface | BraceType::Struct | BraceType::Union
                )
            })
    }
}

fn line_clears_pending_member_spacing(line: &str) -> bool {
    let trimmed = line.trim_ascii();
    trimmed.is_empty()
        || trimmed.starts_with(['#', '}'])
        || is_standard_access_label(trimmed)
        || nested_type_start(trimmed)
}
