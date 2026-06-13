use super::brace_classification::is_class_like_brace_type;
use super::buffer::OpenBraceShape;
use super::columns::leading_visual_width;
use super::headers::starts_header_word;
use super::indentation::LineKind;
use super::language;
use super::line_scan::{is_comment_line, trailing_comment_split_limit};
use super::raw_strings;
use super::state::{ContinuationIndent, FormatterBraceType};
use super::switch_cases::{find_case_colon, is_case_label_start};
use super::{FormatEngine, unmatched_open_paren_column};
use crate::config::{FormatOptions, IndentStyle};
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(super) fn line_kind(line: &str, access_labels: &[String]) -> LineKind {
    if find_case_colon(line).is_some() {
        LineKind::SwitchLabel
    } else if is_plain_label(line, access_labels) {
        LineKind::Label
    } else {
        LineKind::Normal
    }
}

#[derive(Clone, Copy)]
pub(super) struct ClassificationContext<'a> {
    pub(super) enclosing_brace: Option<FormatterBraceType>,
    pub(super) in_initializer: bool,
    pub(super) in_ternary: bool,
    pub(super) previous_line: Option<&'a str>,
}

pub(super) struct LineLayout {
    pub(super) indent_level: Option<usize>,
    pub(super) indent_spaces: usize,
}

pub(super) fn reconcile_line_kind(
    mut kind: LineKind,
    line: &str,
    access_labels: &[String],
    context: ClassificationContext<'_>,
) -> LineKind {
    if kind == LineKind::Normal
        && line.contains(':')
        && (is_attached_user_label(line)
            || is_user_label_candidate(line, access_labels)
            || (starts_access_label(line, access_labels)
                && context
                    .enclosing_brace
                    .is_some_and(is_class_like_brace_type)))
    {
        kind = LineKind::Label;
    }
    if kind == LineKind::Label && (context.in_initializer || context.in_ternary) {
        kind = LineKind::Normal;
    }
    if kind == LineKind::Label
        && !line.trim_end().ends_with(':')
        && context.previous_line.is_some_and(|previous| {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            previous_code.ends_with('(')
                || (previous_code.ends_with(',')
                    && unmatched_open_paren_column(previous_code).is_some())
        })
    {
        kind = LineKind::Normal;
    }
    kind
}

pub(super) fn class_scope_indent(
    kind: LineKind,
    line: &str,
    enclosing_brace: Option<FormatterBraceType>,
    current_indent: usize,
    options: &FormatOptions,
) -> Option<ContinuationIndent> {
    if kind != LineKind::Label || !enclosing_brace.is_some_and(is_class_like_brace_type) {
        return None;
    }
    if options.indent_modifiers
        && !options.indent_classes
        && starts_access_label(line, &options.access_labels)
        && matches!(
            enclosing_brace,
            Some(FormatterBraceType::Class | FormatterBraceType::Struct)
        )
    {
        let base_indent = current_indent.saturating_sub(1) * options.indent_width;
        return Some(ContinuationIndent::Spaces(
            base_indent + options.indent_width / 2,
        ));
    }
    Some(ContinuationIndent::Level(current_indent.saturating_sub(1)))
}

pub(super) fn candidate_line_indent_spaces(
    line: &str,
    options: &FormatOptions,
    in_expression_context: bool,
) -> Option<usize> {
    (is_user_label_candidate(line, &options.access_labels)
        && !options.indent_labels
        && !in_expression_context)
        .then_some(0)
}

pub(super) fn current_line_indent_spaces(
    kind: LineKind,
    line: &str,
    enclosing_brace: Option<FormatterBraceType>,
    options: &FormatOptions,
) -> Option<usize> {
    if options.indent_labels {
        return None;
    }
    let trimmed = line.trim_start();
    let is_unindented_label = kind == LineKind::Label
        && !trimmed.starts_with("case ")
        && !trimmed.starts_with("default:")
        && !trimmed.starts_with("else")
        && !(starts_access_label(line, &options.access_labels)
            && enclosing_brace.is_some_and(is_class_like_brace_type));
    (is_unindented_label || is_attached_user_label(line)).then_some(0)
}

pub(super) fn default_line_layout(
    kind: LineKind,
    has_class_scope_layout: bool,
    indent: usize,
    case_body_extra: usize,
    options: &FormatOptions,
) -> Option<LineLayout> {
    if kind != LineKind::Label || has_class_scope_layout {
        return None;
    }
    let indent_spaces =
        (indent + usize::from(options.indent_labels) * case_body_extra) * options.indent_width;
    Some(LineLayout {
        indent_level: (options.indent_style == IndentStyle::Tabs)
            .then_some(indent_spaces / options.indent_width.max(1)),
        indent_spaces,
    })
}

pub(super) fn access_label_body_indent_spaces(
    line: &str,
    previous: &str,
    enclosing_brace: Option<FormatterBraceType>,
    options: &FormatOptions,
) -> Option<usize> {
    let current = line.trim_start();
    let previous_trimmed = previous[..trailing_comment_split_limit(previous)]
        .trim_end()
        .trim_start();
    if !is_access_label(previous_trimmed.trim(), &options.access_labels)
        || current.starts_with(['#', '}', ')', ';'])
        || current.ends_with(':')
    {
        return None;
    }
    let modifier_indent_applies = matches!(
        enclosing_brace,
        Some(FormatterBraceType::Class | FormatterBraceType::Struct)
    );
    let delta = if options.indent_modifiers && !options.indent_classes && modifier_indent_applies {
        options.indent_width / 2
    } else {
        options.indent_width
    };
    Some(leading_visual_width(previous, options.tab_width) + delta)
}

impl FormatEngine<'_> {
    pub(super) fn replayed_inline_access_body_indent_spaces(
        &self,
        previous: &str,
        delimiter_replayed: bool,
    ) -> Option<usize> {
        if self.options.max_code_length.is_none()
            || !delimiter_replayed
            || !starts_access_label(previous, &self.options.access_labels)
            || previous.trim_end().ends_with('(')
        {
            return None;
        }
        let trimmed = previous.trim_start();
        unmatched_open_paren_column(trimmed).map(|open| {
            leading_visual_width(previous, self.options.tab_width)
                + open
                + 1
                + self.options.indent_width
        })
    }

    pub(super) fn max_length_inline_access_body_indent_extra(&self, line: &str) -> Option<usize> {
        (starts_access_label(line, &self.options.access_labels)
            && line
                .trim_start()
                .split_once(':')
                .is_some_and(|(_, body)| !body.trim().is_empty()))
        .then_some(self.options.indent_width)
    }

    pub(super) fn candidate_label_body_indent_spaces(&self, previous: &str) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (is_user_label_candidate(previous_code, &self.options.access_labels)
            && leading_visual_width(previous, self.options.tab_width) == 0
            && self.pending_braceless_block_bias.is_none()
            && !self.in_initializer_brace()
            && self.current_inline_array_column().is_none())
        .then_some(self.options.indent_width)
    }

    pub(super) fn else_after_candidate_label_indent_spaces(
        &self,
        kind: LineKind,
        previous: &str,
    ) -> Option<usize> {
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        (kind == LineKind::Normal
            && is_user_label_candidate(previous_code, &self.options.access_labels))
        .then(|| leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn following_label_body_indent_spaces(
        &self,
        line: &str,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trim_start().starts_with(['{', '}', '#']) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        if !is_user_label_candidate(previous_trimmed, &self.options.access_labels)
            && !is_attached_user_label(previous_trimmed)
        {
            return None;
        }
        let before = self
            .output
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trim().is_empty())?;
        let before_code = before[..trailing_comment_split_limit(before)].trim_end();
        let before_trimmed = before_code.trim_start();
        let split_else_chain = is_attached_user_label(previous_trimmed)
            || self.recent_split_else_output_chain_active();
        let current = current_indent_spaces.unwrap_or(0);
        if is_user_label_candidate(previous_trimmed, &self.options.access_labels)
            && (is_comment_line(before.trim_start()) || before.trim_start().starts_with("/*"))
            && split_else_chain
        {
            return Some(current.max(leading_visual_width(before, self.options.tab_width)));
        }
        if is_user_label_candidate(previous_trimmed, &self.options.access_labels)
            && before_trimmed.ends_with('{')
            && split_else_chain
        {
            return Some(current.max(
                leading_visual_width(before, self.options.tab_width) + self.options.indent_width,
            ));
        }
        if is_attached_user_label(previous_trimmed) {
            let follows_switch_label =
                before_trimmed.starts_with("case ") || before_trimmed.starts_with("default:");
            let extra = if follows_switch_label {
                self.options.indent_width * 2
                    + self.line_adjuster.next_line_case_unindent_depth() * self.options.indent_width
            } else {
                self.options.indent_width
            };
            return Some(current.max(leading_visual_width(before, self.options.tab_width) + extra));
        }
        None
    }

    pub(super) fn label_block_indent_spaces(
        &self,
        line: &str,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        let body_spaces = self.enclosing_label_block_body_indent_spaces()?;
        if line.trim() == "}" && self.current_closes_label_block() {
            return Some(
                self.frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.label_block)
                    .map_or_else(
                        || body_spaces.saturating_sub(self.options.indent_width),
                        |frame| frame.sibling_indent_column,
                    ),
            );
        }
        if line.trim() == "}" {
            let (open_spaces, open_trimmed) = self
                .output
                .current_closing_brace_open(self.options.tab_width)
                .map(|(spaces, _, trimmed)| (spaces, trimmed))
                .unwrap_or((body_spaces, ""));
            let needs_case_unindent = starts_header_word(open_trimmed, "if")
                || starts_header_word(open_trimmed, "for")
                || starts_header_word(open_trimmed, "while")
                || starts_header_word(open_trimmed, "do")
                || open_trimmed.starts_with("else")
                || open_trimmed.starts_with("case ")
                || open_trimmed.starts_with("default:");
            let base = if open_trimmed.starts_with("switch") {
                open_spaces
            } else {
                open_spaces.max(body_spaces)
            };
            return Some(
                base + usize::from(needs_case_unindent)
                    * self.line_adjuster.next_line_case_unindent_depth()
                    * self.options.indent_width,
            );
        }
        (!line.trim_start().starts_with(['#', '{']))
            .then(|| current_indent_spaces.unwrap_or(0).max(body_spaces))
    }

    pub(super) fn active_label_block_indent_spaces(
        &self,
        line: &str,
        kind: LineKind,
        uses_normal_indent: bool,
        closes_outer_delimiter: bool,
        has_owned_continuation: bool,
    ) -> Option<usize> {
        if kind != LineKind::Normal
            || !uses_normal_indent
            || closes_outer_delimiter
            || has_owned_continuation
            || line.trim_start().starts_with([')', ']', '}'])
        {
            return None;
        }
        let frame = self
            .frame_stack
            .active_brace()
            .filter(|frame| frame.label_block)?;
        let target = if line.trim_start().starts_with('{') {
            frame.sibling_indent_column
        } else {
            frame.body_indent_column
        };
        Some(
            target
                + self.line_adjuster.case_unindent_depth_for_line(line) * self.options.indent_width,
        )
    }

    pub(super) fn closed_label_block_indent_spaces(&self, line: &str) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let frame = self
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.label_block)?;
        Some(
            frame.sibling_indent_column
                + self.line_adjuster.case_unindent_depth_for_line(line) * self.options.indent_width,
        )
    }

    pub(super) fn observe_emitted_label_body_indent(
        &mut self,
        line: &str,
        kind: LineKind,
        line_indent_spaces: usize,
    ) {
        if !line.trim_end().ends_with(':')
            || line[..trailing_comment_split_limit(line)].contains('?')
            || !(kind == LineKind::Label
                || is_user_label_candidate(line, &self.options.access_labels)
                    && line_indent_spaces == 0
                || line.contains('#') && !line.trim_start().starts_with('#'))
        {
            return;
        }
        let mut next_spaces = line_indent_spaces + self.options.indent_width;
        if kind == LineKind::Label && !starts_access_label(line, &self.options.access_labels) {
            next_spaces = next_spaces.max(
                (self.state.line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width,
            );
        }
        if kind == LineKind::Label
            && self.output.iter().rev().take(128).any(|line| {
                let trimmed = line[..trailing_comment_split_limit(line)]
                    .trim_end()
                    .trim_start();
                trimmed == "else" || trimmed.ends_with("} else")
            })
            && let Some(previous) = self
                .output
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trim().is_empty())
            && (is_comment_line(previous.trim_start()) || previous.trim_start().starts_with("/*"))
        {
            next_spaces = next_spaces.max(leading_visual_width(previous, self.options.tab_width));
        }
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = Some(next_spaces);
    }

    fn enclosing_label_block_body_indent_spaces(&self) -> Option<usize> {
        if let Some(frame) = self
            .frame_stack
            .active_brace()
            .filter(|frame| frame.label_block)
        {
            return Some(frame.body_indent_column);
        }
        if !self.output.may_have_label_open() {
            return None;
        }
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes;
            if meta.opens > depth && meta.open_shape == OpenBraceShape::Label {
                let trimmed = self.output.code_trimmed(index);
                if is_attached_user_label(trimmed) {
                    return Some(self.label_block_body_indent_spaces(index));
                }
            }
            depth = depth.saturating_sub(meta.opens);
        }
        None
    }

    fn current_closes_label_block(&self) -> bool {
        self.output.may_have_label_open()
            && self
                .output
                .current_closing_brace_open(self.options.tab_width)
                .is_some_and(|(_, shape, trimmed)| {
                    shape == OpenBraceShape::Label && is_attached_user_label(trimmed)
                })
    }

    fn label_block_body_indent_spaces(&self, label_index: usize) -> usize {
        let indent_width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let before = (0..label_index)
            .rev()
            .find(|index| !self.output[*index].trim().is_empty());
        let Some(before) = before else {
            return self.output.lead_width(label_index, tab_width) + indent_width;
        };
        let before_trimmed = self.output.code_trimmed(before);
        let follows_switch_label =
            before_trimmed.starts_with("case ") || before_trimmed.starts_with("default:");
        let extra = if follows_switch_label {
            indent_width * 2
        } else {
            indent_width
        };
        self.output.lead_width(before, tab_width) + extra
    }
}

pub(super) fn is_label_start(line: &str, access_labels: &[String]) -> bool {
    is_case_label_start(line) || line == "default" || is_plain_label_start(line, access_labels)
}

pub(super) fn is_access_label_start(line: &str, access_labels: &[String]) -> bool {
    language::ACCESS_MODIFIERS.contains(&line)
        || matches!(line, "signals" | "Q_SIGNALS")
        || is_qt_slot_access_label(line)
        || access_labels.iter().any(|custom| custom == line)
}

pub(super) fn is_access_label(line: &str, access_labels: &[String]) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with(':')
        && is_access_label_start(trimmed.trim_end_matches(':').trim_end(), access_labels)
}

pub(super) fn is_standard_access_label(line: &str) -> bool {
    is_access_label(line, &[])
}

pub(super) fn starts_access_label(line: &str, access_labels: &[String]) -> bool {
    let trimmed = line.trim_start();
    let Some((label, rest)) = trimmed.split_once(':') else {
        return false;
    };
    !rest.starts_with(':') && is_access_label_start(label.trim_end(), access_labels)
}

pub(super) fn is_attached_user_label(line: &str) -> bool {
    let trimmed = line.trim_start();
    let Some((label, rest)) = trimmed.split_once(':') else {
        return false;
    };
    if label.is_empty()
        || matches!(
            label,
            "case" | "default" | "public" | "protected" | "private"
        )
        || label.contains(|ch: char| !is_identifier_continue(ch))
    {
        return false;
    }
    let rest = rest.trim_start();
    rest.starts_with('{') && !rest.starts_with("::")
}

pub(super) fn is_user_label_candidate(line: &str, access_labels: &[String]) -> bool {
    let trimmed = line[..trailing_comment_split_limit(line)].trim();
    let before_colon = trimmed.strip_suffix(':').unwrap_or(trimmed).trim_end();
    trimmed.ends_with(':')
        && !is_scope_resolution_prefix(trimmed)
        && !trimmed.starts_with(':')
        && !trimmed.starts_with("::")
        && !trimmed.contains('?')
        && unmatched_open_paren_column(before_colon).is_none()
        && !before_colon.ends_with(')')
        && !matches!(
            first_word(before_colon),
            "for" | "if" | "while" | "switch" | "catch" | "do" | "else"
        )
        && find_case_colon(trimmed).is_none()
        && !is_access_label(trimmed, access_labels)
}

fn is_plain_label(line: &str, access_labels: &[String]) -> bool {
    let code = strip_trailing_comment(line).trim_end();
    if code.ends_with("::") {
        return false;
    }
    let Some(label) = code.strip_suffix(':') else {
        return false;
    };
    is_plain_label_start(label.trim_end(), access_labels)
}

fn strip_trailing_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    while index < bytes.len() {
        let ch = bytes[index];
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == b'\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if let Some(end) = raw_strings::end(line, index) {
            index = end;
            continue;
        }
        match ch {
            b'"' | b'\'' => quote = Some(ch),
            b'/' if bytes.get(index + 1) == Some(&b'/') => return &line[..index],
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let Some(end) = line[index + 2..].find("*/") else {
                    return line;
                };
                let after = index + 2 + end + 2;
                if line[after..].trim().is_empty() {
                    return &line[..index];
                }
                index = after;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    line
}

fn is_plain_label_start(label: &str, access_labels: &[String]) -> bool {
    is_single_identifier(label)
        || is_access_label_start(label, access_labels)
        || is_expression_label_start(label)
}

fn is_qt_slot_access_label(label: &str) -> bool {
    matches!(
        label,
        "public slots"
            | "protected slots"
            | "private slots"
            | "public Q_SLOTS"
            | "protected Q_SLOTS"
            | "private Q_SLOTS"
    )
}

fn is_expression_label_start(label: &str) -> bool {
    let mut chars = label.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !is_identifier_start(first) {
        return false;
    }
    let mut has_operator = false;
    for ch in chars {
        if is_identifier_continue(ch) {
            continue;
        }
        if matches!(ch, '+' | '-' | '*' | '/' | '.' | ':') {
            has_operator = true;
            continue;
        }
        return false;
    }
    has_operator
}

fn is_single_identifier(label: &str) -> bool {
    let mut chars = label.chars();
    chars.next().is_some_and(is_identifier_start) && chars.all(is_identifier_continue)
}

fn is_scope_resolution_prefix(trimmed: &str) -> bool {
    let Some(prefix) = trimmed.strip_suffix("::") else {
        return false;
    };
    !prefix.is_empty()
        && prefix
            .chars()
            .all(|ch| is_identifier_continue(ch) || matches!(ch, ':' | '<' | '>' | ',' | '~' | ' '))
}

fn first_word(line: &str) -> &str {
    let end = line
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(line.len());
    &line[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_case_default_and_plain_labels() {
        assert_eq!(line_kind("case 1:", &[]), LineKind::SwitchLabel);
        assert_eq!(line_kind("default:", &[]), LineKind::SwitchLabel);
        assert_eq!(line_kind("again:", &[]), LineKind::Label);
        assert_eq!(line_kind("return x ? 1 : 0;", &[]), LineKind::Normal);
    }

    #[test]
    fn recognizes_label_starts_before_colon() {
        assert!(is_label_start("case 1", &[]));
        assert!(is_label_start("default", &[]));
        assert!(is_label_start("again", &[]));
        assert!(!is_label_start("1", &[]));
        assert!(!is_label_start("return x ? 1", &[]));
    }

    #[test]
    fn classifies_ignored_case_text_and_identifier_labels() {
        assert_eq!(line_kind("case value // :", &[]), LineKind::Normal);
        assert_eq!(line_kind("default_value:", &[]), LineKind::Label);
    }
}
