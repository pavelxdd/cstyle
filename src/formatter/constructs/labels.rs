use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::braces::classification::is_class_like_brace_type;
use crate::formatter::constructs::headers::{is_header, starts_header_word};
use crate::formatter::constructs::switch_cases::{find_case_colon, is_case_label_start};
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, raw_strings};
use crate::formatter::output::LineDelimiters;
use crate::formatter::output::buffer::OpenBraceShape;
use crate::formatter::state::BraceType;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::syntax::language;
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{ContainsAnyByte, after_leading_comments};
use crate::formatter::text::line_scan::{
    find_byte, is_comment_line, line_paren_imbalance, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start, leading_identifier};

pub(crate) fn line_kind(line: &str, access_labels: &[String]) -> LineKind {
    // Every label ends at a colon.
    if !line.contains(':') {
        LineKind::Normal
    } else if find_case_colon(line).is_some() {
        LineKind::SwitchLabel
    } else if is_plain_label(line, access_labels) || leads_with_goto_label(line) {
        LineKind::Label
    } else {
        LineKind::Normal
    }
}

/// What decides whether a line read as a label is one, needed only then.
#[derive(Clone, Copy)]
pub(crate) struct LabelContext<'a> {
    pub(crate) in_initializer: bool,
    pub(crate) in_ternary: bool,
    pub(crate) previous_line: Option<&'a str>,
}

pub(crate) struct LineLayout {
    pub(crate) indent_spaces: usize,
}

pub(crate) fn reconcile_line_kind<'a>(
    mut kind: LineKind,
    line: &str,
    access_labels: &[String],
    enclosing_brace: Option<BraceType>,
    label_context: impl FnOnce() -> LabelContext<'a>,
) -> LineKind {
    let context = label_context();
    if kind == LineKind::Normal
        && line.contains(':')
        && (is_attached_user_label(line)
            || is_split_label_colon(line, context.previous_line)
            || is_user_label_candidate(line, access_labels)
            || (starts_access_label(line, access_labels)
                && enclosing_brace.is_some_and(is_class_like_brace_type)))
    {
        kind = LineKind::Label;
    }
    if kind != LineKind::Label {
        return kind;
    }
    if context.in_initializer || context.in_ternary {
        kind = LineKind::Normal;
    }
    // The last line of an expression continued over lines, as a case label
    // `case A |\n B:`, is no label.
    if kind == LineKind::Label && context.previous_line.is_some_and(ends_with_binary_operator) {
        kind = LineKind::Normal;
    }
    if kind == LineKind::Label
        && !line.trimmed_end().ends_with(':')
        && context.previous_line.is_some_and(|previous| {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trimmed_end();
            previous_code.ends_with('(')
                || (previous_code.ends_with(',')
                    && unmatched_open_paren_column(previous_code).is_some())
        })
    {
        kind = LineKind::Normal;
    }
    kind
}

pub(crate) fn class_scope_indent(
    kind: LineKind,
    line: &str,
    enclosing_brace: Option<BraceType>,
    current_indent: usize,
    options: &FormatOptions,
) -> Option<ContinuationIndent> {
    if kind != LineKind::Label || !enclosing_brace.is_some_and(is_class_like_brace_type) {
        return None;
    }
    if options.indent_modifiers
        && !options.indent_classes
        && starts_access_label(line, &options.access_labels)
        && matches!(enclosing_brace, Some(BraceType::Class | BraceType::Struct))
    {
        let base_indent = current_indent.saturating_sub(1) * options.indent_width;
        return Some(ContinuationIndent::Spaces(
            base_indent + options.indent_width / 2,
        ));
    }
    Some(ContinuationIndent::Level(current_indent.saturating_sub(1)))
}

/// Whether `line` ends its code with an operator the next line continues.
pub(crate) fn ends_with_binary_operator(line: &str) -> bool {
    let code = line[..trailing_comment_split_limit(line)].trimmed_end();
    code.ends_with_any(b"|&+-*/^<>=") && !code.ends_with("->")
}

pub(crate) fn candidate_line_indent_spaces(
    line: &str,
    options: &FormatOptions,
    in_expression_context: impl FnOnce() -> bool,
) -> Option<usize> {
    (!options.indent_labels
        && is_user_label_candidate(line, &options.access_labels)
        && !in_expression_context())
    .then_some(0)
}

pub(crate) fn current_line_indent_spaces(
    kind: LineKind,
    line: &str,
    enclosing_brace: Option<BraceType>,
    options: &FormatOptions,
) -> Option<usize> {
    if options.indent_labels {
        return None;
    }
    let trimmed = line.trimmed_start();
    let is_unindented_label = kind == LineKind::Label
        && !trimmed.starts_with("case ")
        && !trimmed.starts_with("default:")
        && !trimmed.starts_with("else")
        && (!starts_access_label(line, &options.access_labels)
            || !enclosing_brace.is_some_and(is_class_like_brace_type));
    (is_unindented_label || is_attached_user_label(line)).then_some(0)
}

pub(crate) fn default_line_layout(
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
    Some(LineLayout { indent_spaces })
}

pub(crate) fn access_label_body_indent_spaces(
    line: &str,
    previous: &str,
    enclosing_brace: Option<BraceType>,
    options: &FormatOptions,
) -> Option<usize> {
    // An access label ends with its colon, and a block holds its body.
    find_byte(previous.as_bytes(), b':')?;
    enclosing_brace?;
    let current = line.trimmed_start();
    let previous_trimmed = previous[..trailing_comment_split_limit(previous)]
        .trimmed_end()
        .trimmed_start();
    if !is_access_label(previous_trimmed.trimmed(), &options.access_labels)
        || current.starts_with_any(b"#});")
        || current.ends_with(':')
    {
        return None;
    }
    let modifier_indent_applies =
        matches!(enclosing_brace, Some(BraceType::Class | BraceType::Struct));
    // A label half a level in leaves the members the rest of the level.
    let delta = if options.indent_modifiers && !options.indent_classes && modifier_indent_applies {
        options.indent_width - options.indent_width / 2
    } else {
        options.indent_width
    };
    Some(leading_visual_width(previous, options.tab_width) + delta)
}

impl FormatEngine<'_> {
    pub(crate) fn run_in_switch_body_label_indent_spaces(
        &self,
        kind: LineKind,
        label_line: usize,
    ) -> Option<usize> {
        if kind != LineKind::Label
            || self.options.indent_labels
            || !matches!(
                self.options.brace_style,
                BraceStyle::Horstmann | BraceStyle::Pico
            )
        {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .brace_at_depth(self.layout.nesting.brace_header_stack.len())?;
        let previous = label_line.checked_sub(1)?;
        let tokens = self.output.line_tokens(previous)?;
        if tokens.first != tokens.last
            || frame.open_token != Some(tokens.first)
            || self.output.comment_token(previous).is_some()
        {
            return None;
        }
        self.direct_switch_body_indent_spaces()
    }

    pub(crate) fn candidate_label_body_indent_spaces(&self, previous: &str) -> Option<usize> {
        let previous_code = self.output.code_trimmed_of(previous);
        (is_user_label_candidate(previous_code, &self.options.access_labels)
            && leading_visual_width(previous, self.options.tab_width) == 0
            // After an unmatched `}` no block is open to indent a body in.
            && !self.layout.nesting.brace_type_stack.is_empty()
            && self.layout.pending_braceless_block_bias.is_none()
            && !self.in_initializer_brace()
            && self.current_inline_array_column().is_none())
        .then(|| {
            self.label_statement_column()
                .unwrap_or(self.options.indent_width)
        })
    }

    pub(crate) fn following_label_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trimmed_start().starts_with_any(b"{}#") {
            return None;
        }
        let (previous, previous_code) = self.output.last_code_outside_comment()?;
        let previous_trimmed = previous_code.trimmed_start();
        if !is_user_label_candidate(previous_trimmed, &self.options.access_labels)
            && !is_attached_user_label(previous_trimmed)
        {
            return None;
        }
        let before = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip_while(|line| line.as_str() != previous.as_str())
            .skip(1)
            .find(|line| !line.trimmed().is_empty())?;
        let before_code = self.output.code_trimmed_of(before);
        let before_trimmed = before_code.trimmed_start();
        let split_else_chain = is_attached_user_label(previous_trimmed)
            || self.recent_split_else_output_chain_active();
        let current = current_indent_spaces.unwrap_or(0);
        if is_user_label_candidate(previous_trimmed, &self.options.access_labels)
            && (is_comment_line(before.trimmed_start()) || before.trimmed_start().starts_with("/*"))
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
                    + self.layout.line_adjuster.next_line_case_unindent_depth()
                        * self.options.indent_width
            } else {
                self.options.indent_width
            };
            return Some(current.max(leading_visual_width(before, self.options.tab_width) + extra));
        }
        None
    }

    pub(crate) fn label_block_indent_spaces(
        &self,
        line: &LineView<'_>,
        current_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        // The label's own line stands outside the block it opens.
        if self
            .layout
            .frame_stack
            .active_brace()
            .is_some_and(|frame| frame.label_block && frame.open_output_line == self.output.len())
        {
            return None;
        }
        let body_spaces = self.enclosing_label_block_body_indent_spaces()?;
        // A comment after a closer leaves it the closer of its row.
        let line_trimmed = self.output.code_of(line).trimmed();
        if line_trimmed == "}" && self.current_closes_label_block() {
            return Some(
                self.layout
                    .frame_stack
                    .last_closed_brace()
                    .filter(|frame| frame.label_block)
                    .map_or_else(
                        || body_spaces.saturating_sub(self.options.indent_width),
                        |frame| frame.sibling_indent_column,
                    ),
            );
        }
        if line_trimmed == "}" {
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
                    * self.layout.line_adjuster.next_line_case_unindent_depth()
                    * self.options.indent_width,
            );
        }
        (!line.trimmed_start().starts_with_any(b"#{"))
            .then(|| current_indent_spaces.unwrap_or(0).max(body_spaces))
    }

    pub(crate) fn active_label_block_indent_spaces(
        &self,
        line: &LineView<'_>,
        kind: LineKind,
        uses_normal_indent: bool,
        delimiters: impl Fn() -> LineDelimiters,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if kind != LineKind::Normal || !uses_normal_indent || line_start.starts_with_any(b")]}") {
            return None;
        }
        let frame_stack = &self.layout.frame_stack;
        // A header whose brace the line broke off stands in the block
        // around that brace.
        let frame = frame_stack
            .active_brace()
            .filter(|frame| frame.label_block)
            .or_else(|| {
                let header = frame_stack.active_header()?;
                frame_stack
                    .active_brace()
                    .filter(|brace| {
                        is_header(self.options, leading_identifier(line))
                            && brace.header.as_deref() == Some(header.header.as_str())
                            && brace.header_indent_column == header.line_indent_spaces
                    })
                    .and(frame_stack.enclosing_brace())
                    .filter(|frame| frame.label_block)
            })?;
        if delimiters().continues() {
            return None;
        }
        let target = if line_start.starts_with('{') {
            frame.sibling_indent_column
        } else {
            frame.body_indent_column
        };
        Some(target)
    }

    pub(crate) fn closed_label_block_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if line.trimmed() != "}" {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.label_block)?;
        Some(frame.sibling_indent_column)
    }

    pub(crate) fn observe_emitted_label_body_indent(
        &mut self,
        line: &LineView<'_>,
        kind: LineKind,
        line_indent_spaces: usize,
    ) {
        if !line.trimmed_end().ends_with(':')
            || self.output.code_of(line).contains('?')
            || !(kind == LineKind::Label
                || is_user_label_candidate(line, &self.options.access_labels)
                    && line_indent_spaces == 0
                || has_hash_outside_literals(line) && !line.trimmed_start().starts_with('#'))
        {
            return;
        }
        // The members after an access label stand at the class body, which
        // an indented label leaves.
        if starts_access_label(line, &self.options.access_labels) {
            return;
        }
        // Outside any block a label stands out from nothing.
        let label_level = usize::from(self.layout.indentation.indent() > 0);
        let mut next_spaces = self
            .output
            .len()
            .checked_sub(1)
            .and_then(|label_line| self.run_in_switch_body_label_indent_spaces(kind, label_line))
            .unwrap_or(line_indent_spaces + label_level * self.options.indent_width);
        if kind == LineKind::Label {
            next_spaces = next_spaces.max(
                (self
                    .layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal))
                    * self.options.indent_width,
            );
            // A label between a braceless header and its body leaves the
            // body its level.
            if self.layout.continuation_indent.next_line_indent.is_some() {
                next_spaces = next_spaces.max(self.current_line_indent_spaces());
            }
        }
        if kind == LineKind::Label
            && self.output.scoped().iter().rev().take(128).any(|line| {
                let trimmed = self.output.code_trimmed_of(line).trimmed_start();
                trimmed == "else" || trimmed.ends_with("} else")
            })
            && let Some(previous) = self
                .output
                .scoped()
                .iter()
                .rev()
                .skip(1)
                .find(|line| !line.trimmed().is_empty())
            && (is_comment_line(previous.trimmed_start())
                || previous.trimmed_start().starts_with("/*"))
        {
            next_spaces = next_spaces.max(leading_visual_width(previous, self.options.tab_width));
        }
        self.layout
            .continuation_indent
            .set_next_line_spaces(next_spaces);
    }

    fn enclosing_label_block_body_indent_spaces(&self) -> Option<usize> {
        if let Some(frame) = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.label_block)
        {
            return Some(frame.body_indent_column);
        }
        if !self.output.may_have_label_open() {
            return None;
        }
        // A label's block lies within the current top-level construct.
        let label = self.output.innermost_open_brace_line_plain_where(
            self.output.scoped_range().start,
            |index| {
                self.output.brace_meta(index).open_shape == OpenBraceShape::Label
                    && is_attached_user_label(self.output.code_trimmed(index))
            },
        )?;
        Some(self.label_block_body_indent_spaces(label))
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
            .find(|index| !self.output[*index].trimmed().is_empty());
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

/// Whether `tokens`, after a word, go on with the single colon ending a
/// label.
pub(crate) fn starts_label(tokens: &[Token]) -> bool {
    let mut code = tokens
        .iter()
        .filter(|token| !matches!(token, Token::Whitespace(_) | Token::Comment(..)));
    matches!(code.next(), Some(Token::Symbol(':')))
        && !matches!(code.next(), Some(Token::Symbol(':')))
}

pub(crate) fn is_label_start(line: &str, access_labels: &[String]) -> bool {
    is_case_label_start(line)
        || line == "default"
        || is_plain_label_start(line, access_labels)
        || line
            .strip_prefix("default")
            .and_then(|rest| rest.trimmed_start().strip_prefix(':'))
            .is_some_and(|rest| is_label_start(rest.trimmed_start(), access_labels))
}

pub(crate) fn is_access_label_start(line: &str, access_labels: &[String]) -> bool {
    language::ACCESS_MODIFIERS.contains(&line)
        || matches!(line, "signals" | "Q_SIGNALS")
        || is_qt_slot_access_label(line)
        || access_labels.iter().any(|custom| custom == line)
}

pub(crate) fn is_access_label(line: &str, access_labels: &[String]) -> bool {
    let trimmed = line.trimmed();
    trimmed.ends_with(':')
        && is_access_label_start(trimmed.trim_end_matches(':').trimmed_end(), access_labels)
}

pub(crate) fn is_standard_access_label(line: &str) -> bool {
    is_access_label(line, &[])
}

fn starts_access_label(line: &str, access_labels: &[String]) -> bool {
    let trimmed = line.trimmed_start();
    let Some((label, rest)) = trimmed.split_once(':') else {
        return false;
    };
    !rest.starts_with(':') && is_access_label_start(label.trimmed_end(), access_labels)
}

/// Whether the line starts with a goto label that a statement follows on
/// the line, as `next: x(); }` or `next: if (x) {` where the style keeps
/// one-line statements.
fn leads_with_goto_label(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    let Some((label, rest)) = trimmed.split_once(':') else {
        return false;
    };
    !label.is_empty()
        && is_single_identifier(label)
        && !matches!(
            label,
            "case" | "default" | "public" | "protected" | "private" | "signals" | "slots"
        )
        && !label.starts_with(|ch: char| ch.is_ascii_digit())
        && rest.starts_with_any(b" \t;")
        // Comments after the label leave the statement after them.
        && leads_statement_after_label(after_leading_comments(rest))
}

fn leads_statement_after_label(rest: &str) -> bool {
    !rest.starts_with_any(b"{:/")
        // A statement follows; message arguments and base lists do not.
        && !rest.contains_any_byte(b"][")
        && !matches!(
            leading_identifier(rest),
            "public" | "protected" | "private" | "virtual"
        )
        // A statement closes no paren opened before the label, as the rest
        // of a range `for` header would.
        && line_paren_imbalance(strip_trailing_comment(rest)).0 == 0
        && {
            let opens_header = matches!(
                leading_identifier(rest),
                "if" | "while" | "for" | "switch" | "do" | "try"
            );
            // A statement ends on the line unless a header or an assignment
            // goes on past it; a method parameter row does neither.
            (opens_header
                || rest.contains('=')
                || strip_trailing_comment(rest).trimmed_end().ends_with_any(b";}"))
                // A bit-field width is a constant; a statement calls, assigns,
                // jumps or opens a control header.
                && (opens_header
                    || rest.starts_with(';')
                    || rest.contains_any_byte(b"(=")
                    || matches!(
                        leading_identifier(rest),
                        "return" | "break" | "continue" | "goto"
                    ))
        }
}

pub(crate) fn is_attached_user_label(line: &str) -> bool {
    let trimmed = line.trimmed_start();
    let Some(colon) = find_byte(trimmed.as_bytes(), b':') else {
        return false;
    };
    let (label, rest) = (&trimmed[..colon], &trimmed[colon + 1..]);
    if label.is_empty()
        || matches!(
            label,
            "case" | "default" | "public" | "protected" | "private"
        )
        || label.contains(|ch: char| !is_identifier_continue(ch))
    {
        return false;
    }
    let rest = rest.trimmed_start();
    rest.starts_with('{') && !rest.starts_with("::")
}

/// Whether the line holds only the colon the label's name was split from.
fn is_split_label_colon(line: &str, previous_line: Option<&str>) -> bool {
    line[..trailing_comment_split_limit(line)].trimmed() == ":"
        && previous_line.is_some_and(|previous| {
            is_single_identifier(previous[..trailing_comment_split_limit(previous)].trimmed())
        })
}

fn is_user_label_candidate(line: &str, access_labels: &[String]) -> bool {
    if find_byte(line.as_bytes(), b':').is_none() {
        return false;
    }
    let trimmed = line[..trailing_comment_split_limit(line)].trimmed();
    let before_colon = trimmed.strip_suffix(':').unwrap_or(trimmed).trimmed_end();
    trimmed.ends_with(':')
        && !is_scope_resolution_prefix(trimmed)
        && !trimmed.starts_with_any(b":}")
        && !trimmed.contains('?')
        && !is_operator_expression(before_colon)
        && unmatched_open_paren_column(before_colon).is_none()
        && !before_colon.ends_with(')')
        && !matches!(
            leading_identifier(before_colon),
            "for" | "if" | "while" | "switch" | "catch" | "do" | "else"
        )
        && find_case_colon(trimmed).is_none()
        && !is_access_label(trimmed, access_labels)
        // A label names one thing; `unsigned flag:` is a member split at its
        // bit-field width.
        && !before_colon.contains_any_byte(b" \t")
}

/// Text with operators between names continues an expression; no label.
fn is_operator_expression(text: &str) -> bool {
    text.contains_any_byte(b"|&+^%<>=!~,") && text.chars().any(is_identifier_continue)
}

fn is_plain_label(line: &str, access_labels: &[String]) -> bool {
    let code = strip_trailing_comment(line).trimmed_end();
    if code.ends_with("::") {
        return false;
    }
    let Some(label) = code.strip_suffix(':') else {
        return false;
    };
    is_plain_label_start(label.trimmed_end(), access_labels)
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
                if line[after..].trimmed().is_empty() {
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

pub(crate) fn replayed_inline_access_body_indent_spaces(
    options: &FormatOptions,
    previous: &str,
    delimiter_replayed: bool,
) -> Option<usize> {
    if options.max_code_length.is_none()
        || !delimiter_replayed
        || !starts_access_label(previous, &options.access_labels)
        || previous.trimmed_end().ends_with('(')
    {
        return None;
    }
    let trimmed = previous.trimmed_start();
    unmatched_open_paren_column(trimmed).map(|open| {
        leading_visual_width(previous, options.tab_width) + open + 1 + options.indent_width
    })
}

pub(crate) fn max_length_inline_access_body_indent_extra(
    options: &FormatOptions,
    line: &str,
) -> Option<usize> {
    (starts_access_label(line, &options.access_labels)
        && line
            .trimmed_start()
            .split_once(':')
            .is_some_and(|(_, body)| !body.trimmed().is_empty()))
    .then_some(options.indent_width)
}

pub(crate) fn else_after_candidate_label_indent_spaces(
    options: &FormatOptions,
    kind: LineKind,
    previous: &str,
) -> Option<usize> {
    let previous_code = previous[..trailing_comment_split_limit(previous)].trimmed_end();
    (kind == LineKind::Normal && is_user_label_candidate(previous_code, &options.access_labels))
        .then(|| leading_visual_width(previous, options.tab_width) + options.indent_width)
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
