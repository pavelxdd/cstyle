use crate::config::{BraceStyle, LineEnding};
use crate::formatter::braces::classification::{
    ExternCGuard, block_indent_extra, brace_indent_applies, is_lambda_body_header,
    is_lambda_capture_header, is_namespace_or_module_block_header,
    lambda_header_has_trailing_return, line_ends_lambda_parameter_list,
    line_opens_lambda_or_capture_only_block, line_opens_parameterized_lambda_block,
};
use crate::formatter::braces::compound_literals::line_ends_compound_literal_cast;
use crate::formatter::braces::initializers::{
    InlineArrayFrame, initializer_brace_line_comment_gap,
};
use crate::formatter::constructs::headers::{
    is_header, line_is_control_body_header, starts_header_word,
};
use crate::formatter::constructs::labels;
use crate::formatter::constructs::return_types::is_parameter_return_type_prefix;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{CommentKind, Token, token_char_len};
use crate::formatter::output::block_spacing::is_break_blocks_closing_header;
use crate::formatter::state::frame::{BraceSemanticKind, ConstructorInitializerLayout};
use crate::formatter::state::indentation::LineKind;
use crate::formatter::state::{BraceType, PreviousToken};
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::blocks::{BlockKind, is_code_token};
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::{function_name_start, scoped_name_is_constructor};
use crate::formatter::text::columns::leading_visual_width;
use crate::formatter::text::line_scan;
use crate::formatter::text::line_scan::has_hash_outside_literals;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, is_comment_only_line, line_comment_split_limit,
    reverse_scan_skips_block_comment, trailing_comment_split_limit,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::formatter::tokens::operators::head_ends_binary_operator;
use crate::source::lex::{is_identifier_start, is_word_char, leading_identifier};

fn is_semicolonless_call_line(line: &str) -> bool {
    let trimmed = line.trimmed();
    let Some(open) = trimmed.find('(') else {
        return false;
    };
    if !trimmed.ends_with(')')
        || trimmed.contains(';')
        || trimmed.contains('{')
        || trimmed.contains('}')
    {
        return false;
    }
    let name = trimmed[..open].trimmed();
    let macro_part = name.strip_prefix("wx").unwrap_or(name);
    if macro_part.is_empty()
        || !macro_part
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
        || !macro_part
            .chars()
            .any(|ch| ch.is_ascii_uppercase() || ch == '_')
    {
        return false;
    }
    let mut depth = 0i32;
    for ch in trimmed.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    depth == 0
}

fn attach_case_label_brace_to_line(line: &str, access_labels: &[String]) -> Option<String> {
    let comment_start = trailing_comment_split_limit(line);
    let code = line[..comment_start].trimmed_end();
    let trimmed = code.trimmed();
    if !trimmed.ends_with(':')
        || !labels::is_label_start(trimmed.trim_end_matches(':'), access_labels)
    {
        return None;
    }
    Some(attach_brace_in_comment_gap(code, &line[code.len()..]))
}

fn attach_brace_before_trailing_comment(line: &str) -> Option<String> {
    let line_comment_start = line_comment_split_limit(line);
    let comment_start = if line_comment_start < line.len() {
        line_comment_start
    } else {
        trailing_comment_split_limit(line)
    };
    if comment_start == line.len() {
        return None;
    }
    let code = line[..comment_start].trimmed_end();
    if code.is_empty() {
        return None;
    }
    Some(attach_brace_in_comment_gap(code, &line[comment_start..]))
}

fn attach_brace_in_comment_gap(code: &str, rest: &str) -> String {
    let comment = rest.trimmed_start();
    let gap = rest.len() - comment.len();
    if gap == 0 {
        return attach_brace_to_code_and_comment(code, rest);
    }
    format!(
        "{code}{}{}",
        brace_in_comment_gap(&rest[..gap]),
        comment.trimmed_end()
    )
}

/// The gap before a trailing comment with a brace attached into it: astyle
/// pads the gap to three columns, never starts it with a tab, and puts the
/// brace in its second column.
fn brace_in_comment_gap(gap: &str) -> String {
    let mut columns: Vec<char> = gap.chars().collect();
    let padding = 3usize.saturating_sub(columns.len());
    columns.splice(0..0, std::iter::repeat_n(' ', padding));
    if columns[0] == '\t' {
        columns.insert(0, ' ');
    }
    columns[1] = '{';
    columns.into_iter().collect()
}

fn is_single_trailing_block_comment(comment: &str) -> bool {
    let comment = comment.trimmed();
    comment
        .strip_prefix("/*")
        .and_then(|rest| rest.find("*/").map(|end| &rest[end + 2..]))
        .is_some_and(str::is_empty)
}

fn attach_brace_to_code_and_comment(code: &str, comment: &str) -> String {
    let mut output = format!("{code} {{");
    let comment = comment.trimmed_start();
    if !comment.is_empty() {
        output.push(' ');
        output.push_str(comment);
    }
    output
}

/// The brace being opened, as the placement steps of `push_open_brace` see it.
struct OpenBrace {
    header: Option<String>,
    brace_type: BraceType,
    block_indent_extra: usize,
    class_block_indent_extra: usize,
    breaks_before_call: bool,
}

#[derive(Clone, Copy)]
struct OpenBraceLambda {
    line_opens_body: bool,
    previous_line_opens_body: bool,
    opens_body: bool,
    header_indent: Option<usize>,
    breaks_before_call: bool,
}

/// A single-line comment token that directly follows the brace on its line.
#[derive(Clone, Copy)]
struct OpenBraceComments<'t> {
    line: Option<&'t str>,
    block: Option<&'t str>,
    next_is_trailing: bool,
}

impl<'t> OpenBraceComments<'t> {
    fn from_next(next: Option<&'t Token>, current_is_blank: bool) -> Self {
        let line = match next {
            Some(Token::Comment(CommentKind::Line, comment)) if !current_is_blank => {
                Some(comment.as_str())
            }
            _ => None,
        };
        let block = match next {
            Some(Token::Comment(CommentKind::Block, comment))
                if !current_is_blank && !comment.contains('\n') =>
            {
                Some(comment.as_str())
            }
            _ => None,
        };
        let next_is_trailing = !current_is_blank && matches!(next, Some(Token::Comment(_, _)));
        Self {
            line,
            block,
            next_is_trailing,
        }
    }
}

#[derive(Clone, Copy)]
struct OpenBracePlacement<'t> {
    comments: OpenBraceComments<'t>,
    lambda: OpenBraceLambda,
    attach_case_label_brace: bool,
    inline_nested_header_level: Option<usize>,
    headerless_inline_command_column: Option<usize>,
}

impl FormatEngine<'_> {
    pub(crate) fn continuation_adjacent_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Horstmann
                | BraceStyle::Pico
        ) || line.trimmed() != "{"
        {
            return None;
        }
        let previous_index = self.output.last_non_empty_index()?;
        // The ` */` of a block comment is no operator-led continuation.
        if self.output.comment_start_index(previous_index) != previous_index {
            return None;
        }
        let previous = &self.output[previous_index];
        let previous_code = self
            .output
            .code_before_comment(previous_index)
            .trimmed_end();
        if previous_code.trimmed_start().starts_with_any(b":,") {
            return Some(self.layout.indentation.indent() * self.options.indent_width);
        }
        let is_header_condition_continuation =
            previous_code.ends_with(')') && self.layout.frame_stack.active_header().is_some();
        if previous_code.contains_from_first_byte("#define")
            || is_header_condition_continuation
            // A variadic `...` parameter leads with no operator.
            || previous_code.trimmed_start().starts_with("...")
            // Nor a parameter's declarator split from its type.
            || self.output.line_tokens(previous_index).is_some_and(|span| {
                let tokens = &self.tree.tokens;
                (matches!(&tokens[span.first], Token::Operator(operator)
                    if operator == "*" || operator == "&")
                    || matches!(tokens[span.first], Token::Symbol('*' | '&')))
                    && self.tree.groups.enclosing(span.first).is_some_and(|group| {
                        self.tree.groups.get(group).delimiter == Delimiter::Paren
                    })
                    && self
                        .tree
                        .previous_code_token(span.first)
                        .is_some_and(|before| matches!(tokens[before], Token::Word(_)))
            })
            // Nor does an argument or a statement led by a unary operator.
            || self.output.line_tokens(previous_index).is_some_and(|span| {
                self.tree.previous_code_token(span.first).is_some_and(|before| {
                    matches!(
                        self.tree.tokens[before],
                        Token::Symbol(',' | '(' | ';' | '{' | '}')
                    )
                })
            })
            || !(head_ends_binary_operator(previous_code)
                || previous_code.ends_with("->")
                || previous_code.trimmed_start().starts_with_any(b"<>|&+-*/%=!?.~"))
        {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn isolated_opening_brace_body_indent_spaces(
        &self,
        line: &LineView<'_>,
        line_kind: LineKind,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_kind != LineKind::Normal
            || line_start.starts_with_any(b"{}#")
            || ["do", "if", "for", "while", "switch"]
                .iter()
                .any(|word| starts_header_word(line_start, word))
            || self
                .layout
                .frame_stack
                .active_brace()
                .is_some_and(|frame| frame.header.is_some())
        {
            return None;
        }
        let opening_brace = self.output.last_line_outside_comment()?;
        if self.output.code_of(opening_brace).trimmed() != "{" {
            return None;
        }
        let opening_indent = leading_visual_width(opening_brace, self.options.tab_width);
        let class_body_extra = usize::from(
            self.options.indent_classes
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    Some(BraceType::Class)
                ),
        );
        let vtk_indented_brace = self.options.brace_style == BraceStyle::Vtk
            && opening_indent > 0
            && self.layout.frame_stack.active_brace().is_some_and(|frame| {
                self.layout.nesting.brace_type_stack.last() == Some(&frame.brace_type)
                    && !matches!(
                        frame.semantic_kind,
                        BraceSemanticKind::Definition | BraceSemanticKind::Aggregate
                    )
                    && !(frame.semantic_kind == BraceSemanticKind::Namespace
                        && self.options.indent_namespaces)
                    || matches!(
                        frame.semantic_kind,
                        BraceSemanticKind::Definition | BraceSemanticKind::Aggregate
                    ) && self.should_indent_brace_line(frame.brace_type)
            });
        Some(if self.options.brace_style == BraceStyle::Whitesmith {
            opening_indent + class_body_extra * self.options.indent_width
        } else if self.options.indent_braces
            || vtk_indented_brace
            || self.layout.indentation.current_block_indent_increment() == Some(0)
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    Some(BraceType::Namespace | BraceType::Extern)
                )
        {
            opening_indent
        } else {
            opening_indent
                + (1 + class_body_extra) * self.options.indent_width
                + self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        })
    }

    pub(crate) fn lambda_opening_brace_indent_spaces(&self, line: &LineView<'_>) -> Option<usize> {
        if line.trimmed() != "{" {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)?;
        Some(
            if self.options.brace_style == BraceStyle::Whitesmith
                || self.options.brace_style == BraceStyle::Vtk && frame.header_indent_column > 0
            {
                frame.body_indent_column
            } else {
                frame.sibling_indent_column
            },
        )
    }

    pub(crate) fn lambda_body_indent_spaces_after_opening_brace(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let line_start = line.trimmed_start();
        if line_start.starts_with_any(b"{}")
            || line.trimmed_end().ends_with('{')
            || is_lambda_body_header(line_start)
            || !self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| self.output.code_trimmed_of(previous).ends_with('{'))
        {
            return None;
        }
        self.layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.semantic_kind == BraceSemanticKind::Lambda)
            .map(|frame| frame.body_indent_column)
    }

    pub(crate) fn embedded_capture_lambda_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::None {
            return None;
        }
        let current = line.trimmed_start();
        if current.is_empty() || current.starts_with('#') {
            return None;
        }
        for previous in self.output.scoped().iter().rev().take(8) {
            let trimmed = previous.trimmed_start();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('}') {
                return None;
            }
            let embedded = previous
                .rfind('[')
                .is_some_and(|index| previous[..index].contains('{'));
            if embedded && line_opens_lambda_or_capture_only_block(previous.trimmed_end()) {
                let base = leading_visual_width(previous, self.options.tab_width);
                let parameterized = line_opens_parameterized_lambda_block(previous.trimmed_end());
                return if current.starts_with('}') {
                    Some(if parameterized {
                        base
                    } else {
                        base + self.options.indent_width
                    })
                } else {
                    Some(
                        base + if parameterized {
                            self.options.indent_width
                        } else {
                            self.options.indent_width * 2
                        },
                    )
                };
            }
        }
        None
    }

    pub(crate) fn whitesmith_identifier_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
        normal_indent: usize,
    ) -> Option<usize> {
        if line.trimmed() != "{"
            || self.options.brace_style != BraceStyle::Whitesmith
            || !self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    !is_namespace_or_module_block_header(previous)
                        && previous
                            .trimmed_start()
                            .chars()
                            .next()
                            .is_some_and(is_identifier_start)
                })
        {
            return None;
        }
        Some(normal_indent * self.options.indent_width)
    }

    pub(crate) fn whitesmith_operator_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "{"
            || self.options.brace_style != BraceStyle::Whitesmith
            || self.innermost_brace_is_compound_literal()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        self.output
            .code_of(previous)
            .trimmed_start()
            .starts_with_any(b"<>|&+-*/%=!?:,.")
            .then_some(0)
    }

    /// Whether the `{` the pending line holds opens a value: a compound
    /// literal or an initializer continuing its statement.
    fn pending_brace_opens_value(&self) -> bool {
        self.output
            .pending_tokens()
            .and_then(|span| self.tree.groups.opened_at(span.first))
            .is_some_and(|group| {
                matches!(
                    self.tree.blocks.kind(group),
                    Some(BlockKind::CompoundLiteral | BlockKind::Initializer)
                )
            })
    }

    pub(crate) fn whitesmith_definition_or_command_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "{"
            || self.options.brace_style != BraceStyle::Whitesmith
            // A compound literal's element brace stands at its elements.
            || self.innermost_brace_is_compound_literal()
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if self.pending_brace_opens_value() {
            // A type's closer stands a level past the statement it ends.
            let closer_level = usize::from(previous.trimmed_start().starts_with('}'));
            return Some(
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width
                    - closer_level * self.options.indent_width,
            );
        }
        let previous_code = self.output.code_trimmed_of(previous);
        if is_namespace_or_module_block_header(previous_code) && !self.options.indent_namespaces {
            return None;
        }
        let previous_is_header_continuation = previous_code.ends_with(')')
            && !line_is_control_body_header(previous_code.trimmed_start());
        let previous_definition_brace_spaces = (!self.output_ends_objc_method_header())
            .then(|| {
                let owner = self.output.scoped().iter().rev().find(|line| {
                    let trimmed = line.trimmed_start();
                    !trimmed.is_empty() && !is_comment_only_line(trimmed)
                })?;
                let code = self.output.code_trimmed_of(owner);
                if code.trimmed_start().starts_with('#') {
                    return None;
                }
                let (closes, opens) = line_scan::line_paren_imbalance(code);
                (closes == 0 && opens.is_empty()).then(|| {
                    leading_visual_width(owner, self.options.tab_width) + self.options.indent_width
                })
            })
            .flatten();
        Some(match self.layout.frame_stack.active_brace() {
            // A class body's brace stands a level past the enclosing body.
            Some(frame) if frame.class_base => frame.sibling_indent_column.max(
                self.layout
                    .frame_stack
                    .enclosing_brace()
                    .map_or(0, |enclosing| enclosing.body_indent_column)
                    + self.options.indent_width,
            ),
            Some(frame) if frame.semantic_kind == BraceSemanticKind::Lambda => {
                frame.body_indent_column
            }
            Some(frame) if frame.semantic_kind == BraceSemanticKind::Definition => self
                .layout
                .frame_stack
                .active_constructor_initializer()
                .and_then(|frame| {
                    let base = self.constructor_initializer_base_indent_spaces()?;
                    Some(match frame.layout {
                        ConstructorInitializerLayout::SameLine => base,
                        ConstructorInitializerLayout::Split => {
                            frame.colon_line_indent_spaces
                                + usize::from(frame.function_try) * self.options.indent_width
                        }
                    })
                })
                .or_else(|| self.split_definition_brace_indent_spaces())
                .or(previous_definition_brace_spaces)
                .unwrap_or(frame.header_indent_column + self.options.indent_width),
            // A block after a statement in a switch body belongs to that
            // statement, not to the switch.
            _ => match self
                .layout
                .frame_stack
                .active_header()
                .filter(|_| !self.directly_in_switch_body())
            {
                Some(header)
                    if previous_is_header_continuation
                        || self
                            .layout
                            .frame_stack
                            .active_brace()
                            .and_then(|frame| frame.header.as_deref())
                            == Some(header.header.as_str()) =>
                {
                    header.body_indent_spaces
                }
                _ => {
                    // A directive line says nothing of the code's column; the
                    // line of the code before it does.
                    let code_line = previous
                        .trimmed_start()
                        .starts_with('#')
                        .then(|| {
                            let first = self.output.pending_tokens()?.first;
                            let token = self.tree.previous_code_token(first)?;
                            self.output.line_with_token(token)
                        })
                        .flatten();
                    code_line.map_or_else(
                        || leading_visual_width(previous, self.options.tab_width),
                        |index| self.output.lead_width(index, self.options.tab_width),
                    ) + self.options.indent_width
                }
            },
        })
    }

    fn split_definition_brace_indent_spaces(&self) -> Option<usize> {
        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(32)
        {
            let code = self.output.code_trimmed_of(previous);
            if reverse_scan_skips_block_comment(code, &mut in_block_comment) {
                continue;
            }
            if code.ends_with_any(b";{}") {
                return None;
            }
            let (closes, mut opens) = self.paren_imbalance_of(code);
            if close_pending > 0 {
                for &column in opens.iter().rev().take(close_pending) {
                    let before = code[..column].trimmed_end();
                    let Some(name_start) = function_name_start(before) else {
                        continue;
                    };
                    let return_type = before[..name_start].trimmed_end();
                    let name = before[name_start..].trimmed_start();
                    if !name.is_empty()
                        && !is_header(self.options, name)
                        && (is_parameter_return_type_prefix(return_type)
                            || name_start == 0 && scoped_name_is_constructor(name))
                    {
                        return Some(
                            leading_visual_width(previous, self.options.tab_width)
                                + self.options.indent_width,
                        );
                    }
                }
            }
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
        }
        None
    }

    pub(crate) fn gnu_continuation_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if line.trimmed() != "{" || self.options.brace_style != BraceStyle::Gnu {
            return None;
        }
        let (_, previous_code) = self.output.last_code_outside_comment()?;
        if !head_ends_binary_operator(previous_code)
            && !["<=", ">=", "==", "!="]
                .iter()
                .any(|operator| previous_code.ends_with(operator))
        {
            return None;
        }
        Some(
            if previous_code.ends_with('/')
                && self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .take(4)
                    .any(|line| has_hash_outside_literals(line))
            {
                self.options.indent_width
            } else {
                self.layout.indentation.indent() * self.options.indent_width
            },
        )
    }

    pub(crate) fn gnu_command_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.brace_style != BraceStyle::Gnu || line.trimmed() != "{" {
            return None;
        }
        let header = self.layout.frame_stack.active_header()?;
        if !self.layout.frame_stack.active_brace().is_some_and(|brace| {
            brace.semantic_kind == BraceSemanticKind::Command
                && brace.header.as_deref() == Some(header.header.as_str())
        }) {
            return None;
        }
        Some(if matches!(header.header.as_str(), "case" | "default") {
            header.line_indent_spaces
        } else {
            header.body_indent_spaces
        })
    }

    pub(crate) fn indented_command_body_indent_spaces(&self) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
        ) {
            return None;
        }
        let brace = self.layout.frame_stack.active_brace()?;
        let header = self.layout.frame_stack.active_header()?;
        // A brace after a statement in a case body is no case block.
        let after_statement = matches!(header.header.as_str(), "case" | "default")
            && !brace.case_block
            && self
                .output
                .pending_tokens()
                .and_then(|span| self.tree.previous_code_token(span.first))
                .is_some_and(|previous| !matches!(self.tree.tokens[previous], Token::Symbol(':')));
        (brace.semantic_kind == BraceSemanticKind::Command
            && brace.header.as_deref() == Some(header.header.as_str())
            && !after_statement)
            .then_some(header.body_indent_spaces)
    }

    pub(crate) fn vtk_or_ratliff_headerless_command_opening_brace_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if !matches!(
            self.options.brace_style,
            BraceStyle::Vtk | BraceStyle::Ratliff
        ) || line.trimmed() != "{"
        {
            return None;
        }
        self.layout
            .frame_stack
            .active_brace()
            .filter(|frame| {
                frame.semantic_kind == BraceSemanticKind::Command && frame.header.is_none()
            })
            .map(|frame| {
                // A head split over lines, as a macro loop's arguments, starts
                // where its parens open.
                let lines = self.output.scoped();
                let paren_balance = |line: &str| {
                    let code = &self.output.code_of(line);
                    code.matches('(').count() as isize - code.matches(')').count() as isize
                };
                let head = lines.iter().rposition(|line| !line.trimmed().is_empty());
                // The head's row may have moved after the frame took its column.
                let header_column = head
                    .map(|index| &lines[index])
                    .filter(|line| {
                        let code = self.output.code_trimmed_of(line);
                        code.trimmed_start()
                            .starts_with(|ch: char| ch.is_alphabetic() || ch == '_')
                            && !code.ends_with_any(b";{}")
                            && paren_balance(line) == 0
                    })
                    .map_or(frame.header_indent_column, |line| {
                        frame
                            .header_indent_column
                            .max(leading_visual_width(line, self.options.tab_width))
                    });
                head.filter(|&index| paren_balance(&lines[index]) < 0)
                    .map_or(header_column, |mut index| {
                        let mut balance = paren_balance(&lines[index]);
                        while balance < 0 && index > 0 {
                            index -= 1;
                            balance += paren_balance(&lines[index]);
                        }
                        leading_visual_width(&lines[index], self.options.tab_width)
                    })
                    + self.options.indent_width
            })
    }

    /// A `{` alone after an `else` line stands at least where the style
    /// puts it, from the adjusted `else` line.
    pub(crate) fn align_else_opening_brace_after_adjustment(&self, line: String) -> String {
        if line.trimmed() != "{" {
            return line;
        }
        let Some(else_indent) = self
            .output
            .may_have_else()
            .then(|| {
                (0..self.output.len())
                    .rev()
                    .find(|&index| !self.output.trimmed(index).is_empty())
                    .filter(|&index| self.output.trimmed(index) == "else")
                    .map(|index| self.output.lead_width(index, self.options.tab_width))
            })
            .flatten()
        else {
            return line;
        };
        let style_indent = if matches!(
            self.options.brace_style,
            BraceStyle::Gnu | BraceStyle::Whitesmith | BraceStyle::Vtk
        ) {
            else_indent + self.options.indent_width
        } else {
            else_indent
        };
        if leading_visual_width(&line, self.options.tab_width) < style_indent {
            format!("{}{{", " ".repeat(style_indent))
        } else {
            line
        }
    }

    fn current_split_lambda_body_header(&self) -> Option<(String, usize)> {
        let current = &self.current[..trailing_comment_split_limit(&self.current)];
        let mut head = current.trimmed().to_string();
        let body_indent = self.continuation_base_indent() * self.options.indent_width;
        if is_lambda_body_header(&head) {
            return Some((head, body_indent));
        }
        for raw in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(12)
        {
            let code = self.output.code_of(raw).trimmed();
            if code.starts_with('#') {
                break;
            }
            if head.is_empty() {
                head.push_str(code);
            } else {
                head.insert(0, ' ');
                head.insert_str(0, code);
            }
            if is_lambda_body_header(&head) {
                return Some((head, body_indent));
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                break;
            }
        }
        None
    }

    /// Whether a `}` earlier on the source line of the brace being placed
    /// was broken onto a line of its own.
    fn closing_brace_broken_off_source_line(&self) -> bool {
        let tokens = &self.tree.tokens;
        let Some(open) = self.current.active_token() else {
            return false;
        };
        let line_start = tokens[..open]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |index| index + 1);
        tokens[line_start..open]
            .iter()
            .find(|token| !matches!(token, Token::Whitespace(_)))
            .is_some_and(|token| matches!(token, Token::Symbol('}')))
            && !self.current.trimmed_start().starts_with('}')
    }

    fn push_attached_comment_with_source_gap(&mut self, comment: &str) {
        if comment.trimmed_start().starts_with("//")
            && self.current.trimmed() == "{"
            && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
        {
            self.trim_current_end();
            let gap = initializer_brace_line_comment_gap(self.options, &self.current);
            self.current.push_str(&gap);
            self.current.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
            return;
        }
        match self.token_input.next_input_whitespace.clone() {
            Some(ws) if !ws.is_empty() => {
                self.trim_current_end();
                // An added brace that took more than the gap leaves the
                // comment its source column.
                let overrun = self
                    .current
                    .active_token()
                    .and_then(|brace| self.added_opener_overruns.get(&brace))
                    .copied()
                    .unwrap_or(0);
                let prefix = self.broken_off_else_prefix_width();
                let target = self
                    .layout
                    .line_state
                    .trailing_comment_columns
                    .front()
                    .map(|target| target.saturating_sub(overrun + prefix))
                    .filter(|_| ws.chars().all(|ch| ch == ' '));
                // Code joined or padded before the brace moves the comment no
                // further right than its source column, as astyle keeps its
                // column.
                // Code joined or padded before the brace moves the comment no
                // further right than its source column; spaces unpadding took
                // out go into the gap.
                let unpad_parens = self.options.unpad_parens;
                let column_gap = |current: &str, target: usize| {
                    let code_len = current.trimmed().chars().count();
                    // Unpadding takes at most a space off each side of a paren.
                    let deficit = target.saturating_sub(code_len + ws.len());
                    if unpad_parens && deficit <= 2 * current.matches(['(', ')']).count() {
                        return target.saturating_sub(code_len).max(1);
                    }
                    let shift = (code_len + ws.len()).saturating_sub(target);
                    ws.len().saturating_sub(shift).max(1)
                };
                // The space joining a closing brace to its header counts as no
                // padding.
                if let Some(target) = target
                    && !self.current.trimmed_start().starts_with('}')
                {
                    // A line that lost a `}` broken off before it keeps its
                    // comment at the source column.
                    let gap = if self.closing_brace_broken_off_source_line()
                        && is_break_blocks_closing_header(leading_identifier(
                            self.current.trimmed_start(),
                        )) {
                        target
                            .saturating_sub(self.current.trimmed().chars().count())
                            .max(1)
                    } else {
                        column_gap(&self.current, target)
                    };
                    self.current.push_str(&" ".repeat(gap));
                } else if self
                    .token_input
                    .previous_input_whitespace
                    .as_deref()
                    .is_none_or(str::is_empty)
                    && ws.chars().all(|ch| ch == ' ')
                {
                    // The space before the brace comes out of the gap, unless
                    // the line lost a `}` broken off before it.
                    let absorbed = usize::from(!self.closing_brace_broken_off_source_line());
                    let keep = ws.len().saturating_sub(absorbed).max(1);
                    self.current.push_str(&ws[..keep]);
                } else if let Some(target) = target {
                    let run_in_closer = self.run_in_closer_row() == Some(true);
                    let gap = column_gap(&self.current, target);
                    let gap = gap.saturating_sub(usize::from(run_in_closer)).max(1);
                    self.current.push_str(&" ".repeat(gap));
                } else {
                    self.current.push_str(&ws);
                }
            }
            // A comment written against its brace stays there.
            _ => {}
        }
        self.current.push_str(comment.trimmed_end());
        self.comments.skip_next_attached_comment = true;
    }

    fn reorder_brace_before_current_block_comment(&self) -> Option<String> {
        let comment_start = self.current_trailing_comment_split_limit();
        if comment_start == self.current.len() {
            return None;
        }
        if !is_single_trailing_block_comment(&self.current[comment_start..]) {
            return None;
        }
        attach_brace_before_trailing_comment(&self.current)
    }

    fn try_push_bare_open_brace_run(&mut self) -> bool {
        if self.options.brace_style != BraceStyle::None
            || self.token_input.token_begins_source_line
            || self.layout.command_state.previous_command_char != Some('{')
            || self.layout.nesting.paren_depth > 0
            || !matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::NonStatement | BraceType::Array)
            )
        {
            return false;
        }
        let line_is_open_brace_run = |line: &str| {
            let trimmed = line.trimmed_start();
            !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|ch| ch == '{' || ch == ' ' || ch == '\t')
        };
        let current_is_run = self.current.is_open_brace_run();
        if !current_is_run {
            let attach_to_previous = self.current_is_blank()
                && self
                    .output
                    .last()
                    .is_some_and(|line| line_is_open_brace_run(line));
            if !attach_to_previous {
                return false;
            }
            let (line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
            self.current.replace(line);
            self.current.restore_tokens(tokens);
        }
        if !self.current.ends_with_any(b" \t") {
            self.current.push_str("   ");
        }
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        self.current.mark_open_brace_run();
        self.previous_was_newline = false;
        self.layout.continuation_indent.next_line_indent = None;
        self.layout.continuation_indent.next_line_indent_spaces =
            Some(leading_visual_width(&self.current, self.options.tab_width));
        self.push_brace_frame(None, BraceType::Array, false, None, false);
        self.layout.nesting.enter_brace(None, BraceType::Array, 0);
        self.layout.indentation.enter_block_with_extra(false, 0);
        self.layout.previous = PreviousToken::Other;
        true
    }

    pub(crate) fn push_open_brace(
        &mut self,
        next: Option<&Token>,
        token_index: usize,
        inferred_definition_brace: bool,
    ) {
        self.unmatched_closing_brace_recovery = false;
        let class_base = self.layout.in_class_base_clause;
        self.clear_continuation_before_open_brace();
        if self.try_push_open_brace_without_block() {
            return;
        }
        self.break_source_line_before_open_brace();
        let brace_header = self.push_pre_brace_header();
        self.observe_block_spacing_open_brace();
        let brace_type = self.classify_pushed_open_brace(
            brace_header.as_deref(),
            next,
            inferred_definition_brace,
        );
        let lambda = self.open_brace_lambda_context();
        self.prepare_continuation_for_open_brace(brace_header.as_deref(), brace_type);

        let inline_nested_header_level = self.layout.inline_nested_header_braceless_bias.take();
        let block_indent_extra =
            block_indent_extra(brace_header.as_deref(), brace_type, self.options);
        let class_block_indent_extra = self.class_block_indent_extra(brace_type, token_index);
        self.push_brace_frame(
            brace_header.as_ref(),
            brace_type,
            lambda.opens_body,
            lambda.header_indent,
            class_base,
        );
        let headerless_inline_command_column = (brace_type == BraceType::Command
            && brace_header.is_none()
            && !lambda.opens_body
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman | BraceStyle::Gnu | BraceStyle::Vtk | BraceStyle::Lisp
            ))
        .then(|| self.current_inline_array_column())
        .flatten();
        self.realign_compound_literal_cast_brace(brace_type);
        let mut brace = OpenBrace {
            header: brace_header,
            brace_type,
            block_indent_extra,
            class_block_indent_extra,
            breaks_before_call: lambda.breaks_before_call,
        };
        if self.should_attach_control_paren_init_brace_from_previous_line(brace_type) {
            let (line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
            self.current.replace(line);
            self.current.restore_tokens(tokens);
            self.token_input.token_begins_source_line = false;
            self.open_multiline_attached_initializer_brace(
                brace.header.take(),
                brace_type,
                block_indent_extra,
                false,
            );
            return;
        }
        let comments = OpenBraceComments::from_next(next, self.current_is_blank());
        if self.try_open_brace_before_trailing_comment(&mut brace, &comments) {
            return;
        }
        if self.try_open_initializer_brace(&mut brace, next, token_index, lambda.line_opens_body) {
            return;
        }
        let attach_case_label_brace = self.should_attach_output_case_label_brace(brace_type);
        if self.should_open_multiline_attached_initializer_brace(brace_type, next) {
            self.open_multiline_attached_initializer_brace(
                brace.header.take(),
                brace_type,
                block_indent_extra,
                false,
            );
            return;
        }
        let placement = OpenBracePlacement {
            comments,
            lambda,
            attach_case_label_brace,
            inline_nested_header_level,
            headerless_inline_command_column,
        };
        let attached_case_label_output_brace = self.place_open_brace(&brace, &placement, next);
        self.enter_pushed_brace_scope(
            brace,
            inline_nested_header_level,
            headerless_inline_command_column,
            attach_case_label_brace || attached_case_label_output_brace,
        );
        self.layout.previous = PreviousToken::Other;
    }

    fn clear_continuation_before_open_brace(&mut self) {
        if self.layout.in_class_base_clause && self.current_is_blank() {
            self.layout.continuation_indent.clear_next_line();
            self.layout.nesting.clear_continuation_indents();
        }
        self.layout.in_class_base_clause = false;
        if self.layout.objc.method_continuation && self.current_is_blank() {
            self.layout.continuation_indent.clear_next_line();
        }
    }

    /// Pushes a brace that opens no block: inside `#define`, after a backslash
    /// continuation, in one-line block mode, or in a bare brace run.
    fn try_push_open_brace_without_block(&mut self) -> bool {
        if self.current.trimmed_start().starts_with("#define") {
            self.emit_source_space_or_ensure();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return true;
        }
        if !self.token_input.token_begins_source_line && self.current.trimmed_end().ends_with('\\')
        {
            self.ensure_space();
            self.current.push('{');
            self.emit_trailing_source_space();
            self.layout.command_state.observe_char('{');
            self.layout.previous = PreviousToken::Other;
            self.previous_was_newline = false;
            return true;
        }
        if self.one_line_block_mode {
            self.push_inline_open_brace();
            return true;
        }
        if self.try_push_bare_open_brace_run() {
            return true;
        }
        false
    }

    fn break_source_line_before_open_brace(&mut self) {
        if self.token_input.token_begins_source_line
            && line_ends_compound_literal_cast(self.current.trimmed_end())
        {
            self.finish_line();
            self.previous_was_newline = true;
        }
        if self.token_input.token_begins_source_line
            && is_semicolonless_call_line(&self.current)
            && !is_header(
                self.options,
                leading_identifier(self.current.trimmed_start()),
            )
        {
            self.finish_line();
            self.layout.pending_braceless_block_bias = None;
            if self.layout.indentation.last_braceless_block().is_some() {
                self.layout.indentation.exit_braceless_block();
            }
            self.layout
                .continuation_indent
                .set_next_line_level(self.statement_level());
            self.previous_was_newline = true;
        }
    }

    fn classify_pushed_open_brace(
        &mut self,
        brace_header: Option<&str>,
        next: Option<&Token>,
        inferred_definition_brace: bool,
    ) -> BraceType {
        let mut brace_type = self.classify_opening_brace(brace_header, self.pending_extern);
        self.layout.objc.method_continuation = false;
        if inferred_definition_brace {
            brace_type = BraceType::Definition;
        }
        // A control condition split over lines can end like a function head.
        // At file scope astyle reads any such head as a definition.
        if brace_type == BraceType::Definition
            && self.pushed_brace_kind() == Some(BlockKind::Control)
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Command | BraceType::Definition)
            )
        {
            brace_type = BraceType::Command;
        }
        if brace_type == BraceType::Command
            && !matches!(next, None | Some(Token::Newline))
            && ((self.current.trimmed_end().ends_with('(') && self.current.trimmed() != "(")
                || (self.current.trimmed_start().starts_with("for (")
                    && !self.current.trimmed_end().ends_with(')')
                    && !matches!(
                        self.options.brace_style,
                        BraceStyle::Allman
                            | BraceStyle::Whitesmith
                            | BraceStyle::Vtk
                            | BraceStyle::Gnu
                            | BraceStyle::Horstmann
                            | BraceStyle::Pico
                    )))
        {
            brace_type = BraceType::Initializer;
        }
        if self.current_is_blank()
            && brace_type == BraceType::Command
            && self
                .layout
                .compound_literal
                .forced_break_depths
                .last()
                .is_some_and(|depth| *depth == self.layout.nesting.brace_header_stack.len())
        {
            brace_type = BraceType::Array;
        }
        brace_type
    }

    fn open_brace_lambda_context(&mut self) -> OpenBraceLambda {
        let capture_only_lambda = is_lambda_capture_header(self.current.trimmed_end());
        let line_opens_lambda_body = self.current_is_lambda_body_header() || capture_only_lambda;
        let previous_lambda_header = (!line_opens_lambda_body)
            .then(|| self.current_split_lambda_body_header())
            .flatten();
        let previous_line_opens_lambda_body = previous_lambda_header.is_some();
        let opens_lambda_body = line_opens_lambda_body || previous_line_opens_lambda_body;
        let lambda_header_indent = if line_opens_lambda_body {
            let structural_indent = self.continuation_base_indent() * self.options.indent_width;
            Some(
                capture_only_lambda
                    .then(|| {
                        self.layout
                            .frame_stack
                            .active_brace()
                            .filter(|frame| {
                                matches!(
                                    frame.semantic_kind,
                                    BraceSemanticKind::Array | BraceSemanticKind::Initializer
                                )
                            })
                            .map(|frame| frame.body_indent_column)
                    })
                    .flatten()
                    .unwrap_or(structural_indent),
            )
        } else {
            previous_lambda_header.as_ref().map(|(_, indent)| *indent)
        };
        let lambda_body_has_trailing_return = if line_opens_lambda_body {
            self.current.has_arrow_after_paren()
        } else {
            previous_lambda_header
                .as_ref()
                .is_some_and(|(header, _)| lambda_header_has_trailing_return(header))
        };
        if opens_lambda_body {
            self.layout.nesting.clear_continuation_indents();
            self.layout
                .continuation_indent
                .after_one_shot_continuation_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = None;
        }
        let lambda_body_breaks_before_call = opens_lambda_body && !lambda_body_has_trailing_return;
        if self.current_is_lambda_body_header() && !line_opens_lambda_body {
            self.layout.nesting.clear_continuation_indents();
            self.layout
                .continuation_indent
                .after_one_shot_continuation_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = None;
            self.layout.pending_braceless_block_bias = None;
        }
        OpenBraceLambda {
            line_opens_body: line_opens_lambda_body,
            previous_line_opens_body: previous_line_opens_lambda_body,
            opens_body: opens_lambda_body,
            header_indent: lambda_header_indent,
            breaks_before_call: lambda_body_breaks_before_call,
        }
    }

    fn prepare_continuation_for_open_brace(
        &mut self,
        brace_header: Option<&str>,
        brace_type: BraceType,
    ) {
        // A row's brace among aligned elements stands at their column.
        let element_row = matches!(
            brace_type,
            BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
        ) && self.current_inline_array_column().is_some();
        if self.current_is_blank() {
            if !element_row {
                self.layout.continuation_indent.next_line_indent_spaces = None;
            }
            if self.layout.command_state.header_broken_before_comment {
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.command_state.header_broken_before_comment = false;
            }
            let previous_closing_header_indent = self
                .output
                .scoped()
                .iter()
                .rev()
                .find(|line| !line.trimmed().is_empty() && !line.trimmed_start().starts_with('#'))
                .filter(|line| {
                    brace_header.is_some_and(is_break_blocks_closing_header)
                        || is_break_blocks_closing_header(line.trimmed())
                })
                .map(|line| leading_visual_width(line, self.options.tab_width));
            if brace_type == BraceType::Command
                && let Some(indent) = previous_closing_header_indent
            {
                let brace_indent = if matches!(
                    self.options.brace_style,
                    BraceStyle::Gnu | BraceStyle::Whitesmith | BraceStyle::Vtk
                ) {
                    indent + self.options.indent_width
                } else {
                    indent
                };
                self.clear_current();
                self.layout.continuation_indent.next_line_indent = None;
                self.layout.continuation_indent.next_line_indent_spaces = Some(brace_indent);
            }
        }
        if matches!(
            brace_type,
            BraceType::Array
                | BraceType::CompoundLiteral
                | BraceType::Initializer
                | BraceType::DeferArray
        ) {
            self.layout.nesting.clear_continuation_indents();
            if self.current_is_blank() {
                self.layout.continuation_indent.next_line_indent = None;
                if self
                    .layout
                    .compound_literal
                    .forced_break_depths
                    .last()
                    .is_some_and(|depth| *depth == self.layout.nesting.brace_header_stack.len())
                {
                    self.layout.continuation_indent.next_line_indent_spaces =
                        self.current_inline_array_column();
                }
            }
        }
        if brace_type == BraceType::Command {
            self.layout.continuation_indent.logical_chain_indent_spaces = None;
        }
        self.pending_extern = false;
        let block_after_semicolonless_call = self.current_is_blank()
            && brace_type == BraceType::Command
            && (self
                .layout
                .previous_pre_adjust_line
                .as_deref()
                .is_some_and(is_semicolonless_call_line)
                || self
                    .output
                    .last_non_empty_scoped()
                    .is_some_and(|line| is_semicolonless_call_line(line)));
        if block_after_semicolonless_call {
            self.layout.pending_braceless_block_bias = None;
            if self.layout.indentation.last_braceless_block().is_some() {
                self.layout.indentation.exit_braceless_block();
            }
            self.layout.continuation_indent.next_line_indent = Some(self.statement_level());
            self.layout.continuation_indent.next_line_indent_spaces = None;
        }
        // A brace after an `else` and a comment is the else's own body.
        let else_body_brace = brace_header == Some("else")
            || self
                .current
                .active_token()
                .and_then(|brace| self.tree.previous_code_token(brace))
                .is_some_and(|previous| {
                    matches!(&self.tree.tokens[previous], Token::Word(word) if word == "else")
                });
        if let Some(level) = self.layout.pending_braceless_block_bias.take()
            && brace_type == BraceType::Command
            && !else_body_brace
        {
            // The bias counts the case body level the indentation lacks.
            let delta = level.saturating_sub(
                self.layout.indentation.indent() + self.case_body_indent_extra(LineKind::Normal),
            );
            if delta > 0 {
                let base = self.layout.indentation.indent();
                self.layout.indentation.enter_braceless_block(delta);
                // The bias of a braceless chain in an `else` body split off
                // by a directive counts the levels the split kept apart.
                let split_else = &mut self.preprocessor.split_else;
                if split_else.extra_indent
                    && base <= split_else.brace_indent
                    && level >= split_else.brace_indent + split_else.extra_levels
                {
                    split_else.reset();
                }
            }
        }
    }

    fn realign_compound_literal_cast_brace(&mut self, brace_type: BraceType) {
        if brace_type == BraceType::CompoundLiteral
            && self.layout.nesting.paren_depth > 0
            && line_ends_compound_literal_cast(self.current.trimmed_end())
            && self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| self.output.code_trimmed_of(line).ends_with(','))
        {
            let level = self
                .layout
                .indentation
                .line_indent(LineKind::Normal, self.options)
                + self.case_body_indent_extra(LineKind::Normal);
            let spaces = ContinuationIndent::Level(level).columns(self.options.indent_width);
            if self.current_line_indent_spaces() > spaces {
                self.layout.continuation_indent.set_next_line_spaces(spaces);
                self.layout.continuation_indent.logical_chain_indent_spaces = None;
                self.layout.nesting.clear_continuation_indents();
            }
        }
    }

    fn try_open_brace_before_trailing_comment(
        &mut self,
        brace: &mut OpenBrace,
        comments: &OpenBraceComments<'_>,
    ) -> bool {
        let OpenBraceComments {
            line: attached_line_comment,
            block: attached_block_comment,
            ..
        } = *comments;
        let brace_type = brace.brace_type;
        let block_indent_extra = brace.block_indent_extra;
        let class_block_indent_extra = brace.class_block_indent_extra;
        let lambda_body_breaks_before_call = brace.breaks_before_call;
        let current_comment_start = self.current_trailing_comment_split_limit();
        if matches!(brace_type, BraceType::Definition | BraceType::NonStatement)
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && attached_line_comment.is_some()
            && !self.token_input.token_followed_by_final_line_comment
            && is_single_trailing_block_comment(&self.current[current_comment_start..])
        {
            self.finish_line();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            self.finish_line();
        } else if brace_type == BraceType::Command
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman | BraceStyle::Horstmann
            )
            && self.token_input.token_followed_by_line_comment_on_line
            && let Some(comment) = attached_block_comment
        {
            let header_indent_spaces = ContinuationIndent::Level(
                self.layout
                    .indentation
                    .line_indent(LineKind::Normal, self.options)
                    + self.case_body_indent_extra(LineKind::Normal),
            )
            .columns(self.options.indent_width);
            self.emit_opening_brace_space(brace_type);
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            self.push_attached_comment_with_source_gap(comment);
            self.layout
                .nesting
                .enter_brace(brace.header.take(), brace_type, block_indent_extra);
            if lambda_body_breaks_before_call {
                self.layout.nesting.mark_current_brace_break_before_call();
            }
            self.layout
                .indentation
                .enter_block_with_extra(false, block_indent_extra + class_block_indent_extra);
            self.layout
                .continuation_indent
                .set_next_line_spaces(header_indent_spaces);
            self.layout.previous = PreviousToken::Other;
            return true;
        }
        if attached_line_comment.is_some()
            && !self.token_input.token_begins_source_line
            && (matches!(brace_type, BraceType::Definition | BraceType::NonStatement)
                || (brace_type == BraceType::Command
                    && self.layout.command_state.current_header.is_none())
                // An enum brace the style breaks takes its comment along.
                || brace_type == BraceType::Enum && !self.options.attach_enum)
            && (!matches!(brace_type, BraceType::Definition | BraceType::NonStatement)
                || self.token_input.token_followed_by_final_line_comment)
            && !self.options.add_braces
            && self.options.brace_style == BraceStyle::OneTrueBrace
        {
            if brace_type == BraceType::Definition {
                self.trim_current_end();
            } else if brace_type == BraceType::Enum {
                self.trim_current_end();
                self.finish_line();
            } else {
                self.emit_opening_brace_space(brace_type);
            }
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if let Some(comment) = attached_line_comment {
                self.push_attached_comment_with_source_gap(comment);
            }
            self.finish_line();
            self.layout
                .nesting
                .enter_brace(brace.header.take(), brace_type, block_indent_extra);
            if lambda_body_breaks_before_call {
                self.layout.nesting.mark_current_brace_break_before_call();
            }
            self.layout
                .indentation
                .enter_block_with_extra(false, block_indent_extra + class_block_indent_extra);
            self.layout.previous = PreviousToken::Other;
            return true;
        }
        if self.token_input.token_followed_by_final_line_comment
            && attached_line_comment.is_some()
            && matches!(brace_type, BraceType::Command | BraceType::Definition)
            && self.options.line_ending != LineEnding::Preserve
            && matches!(
                self.options.brace_style,
                BraceStyle::None | BraceStyle::OneTrueBrace
            )
        {
            self.trim_current_end();
            self.finish_line();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if let Some(comment) = attached_line_comment {
                self.push_attached_comment_with_source_gap(comment);
            }
            self.finish_line();
            self.layout
                .nesting
                .enter_brace(brace.header.take(), brace_type, block_indent_extra);
            if lambda_body_breaks_before_call {
                self.layout.nesting.mark_current_brace_break_before_call();
            }
            self.layout
                .indentation
                .enter_block_with_extra(false, block_indent_extra + class_block_indent_extra);
            self.layout.previous = PreviousToken::Other;
            return true;
        }
        if self.token_input.token_followed_by_final_line_comment
            && attached_line_comment.is_some()
            && matches!(brace_type, BraceType::Command | BraceType::Definition)
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
        {
            let is_closing_header = is_break_blocks_closing_header(self.current.trimmed());
            self.trim_current_end();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if is_closing_header {
                self.current.push_str("      ");
            } else {
                self.current.push_str("    ");
            }
            if let Some(comment) = attached_line_comment {
                self.current.push_str(comment.trimmed_end());
                self.comments.skip_next_attached_comment = true;
            }
            self.finish_line();
            self.layout
                .nesting
                .enter_brace(brace.header.take(), brace_type, block_indent_extra);
            if lambda_body_breaks_before_call {
                self.layout.nesting.mark_current_brace_break_before_call();
            }
            self.layout
                .indentation
                .enter_block_with_extra(false, block_indent_extra + class_block_indent_extra);
            self.layout.previous = PreviousToken::Other;
            return true;
        }
        false
    }

    fn try_open_initializer_brace(
        &mut self,
        brace: &mut OpenBrace,
        next: Option<&Token>,
        token_index: usize,
        line_opens_lambda_body: bool,
    ) -> bool {
        let brace_type = brace.brace_type;
        let block_indent_extra = brace.block_indent_extra;
        let init_run_in = brace_type == BraceType::Initializer && !self.is_objc_method_line();
        let range_for_init_run_in = matches!(
            brace_type,
            BraceType::Command | BraceType::Array | BraceType::Initializer
        ) && self.current.trimmed_start().starts_with("for (")
            && self.current.trimmed_end().ends_with(':')
            && matches!(
                self.options.brace_style,
                BraceStyle::None
                    | BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            );
        if range_for_init_run_in && !matches!(next, None | Some(Token::Newline)) {
            if self.options.brace_style == BraceStyle::None {
                self.open_attached_range_for_init_brace(brace.header.take(), block_indent_extra);
            } else {
                self.open_range_for_init_brace(brace.header.take(), brace_type, block_indent_extra);
            }
            return true;
        }
        if !self.is_objc_method_line()
            && self.layout.line_state.has_nested_designated_init_brace
            && matches!(
                brace_type,
                BraceType::Initializer | BraceType::Array | BraceType::CompoundLiteral
            )
            && matches!(next, Some(Token::Symbol('.')))
            && self
                .token_input
                .next_input_whitespace
                .as_ref()
                .is_some_and(|whitespace| !whitespace.is_empty())
        {
            self.open_expanded_init_brace(brace.header.take(), brace_type, block_indent_extra);
            return true;
        }
        if brace_type == BraceType::Enum
            && self.token_input.token_begins_source_line
            && !self.current_is_blank()
            && self.options.brace_style == BraceStyle::None
        {
            self.finish_line();
        }
        if brace_type == BraceType::Enum
            && self.token_input.token_begins_source_line
            && self.current_is_blank()
            && self.options.brace_style == BraceStyle::None
            && !matches!(next, None | Some(Token::Newline))
            && !self.comments.next_comment_ends_line
        {
            let first_is_brace = matches!(next, Some(Token::Symbol('{')));
            self.open_inline_array_brace(
                brace.header.take(),
                brace_type,
                block_indent_extra,
                token_index,
                first_is_brace,
            );
            return true;
        }
        let non_attaching_lambda_body = line_opens_lambda_body
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
            );
        let operator_led_broken_brace = matches!(
            self.options.brace_style,
            BraceStyle::Allman
                | BraceStyle::Whitesmith
                | BraceStyle::Vtk
                | BraceStyle::Gnu
                | BraceStyle::Horstmann
                | BraceStyle::Pico
        ) && self
            .current
            .trimmed_start()
            .starts_with_any(b"<>|&+-*/%=!?:,.");
        // An enum brace that starts its source line stays broken.
        let source_broken_enum =
            brace_type == BraceType::Enum && self.token_input.token_begins_source_line;
        if (matches!(
            brace_type,
            BraceType::Array | BraceType::CompoundLiteral | BraceType::Enum
        ) || init_run_in)
            && !non_attaching_lambda_body
            && !self.current_is_blank()
            && !operator_led_broken_brace
            && !source_broken_enum
            && !matches!(next, None | Some(Token::Newline))
            && !self.comments.next_comment_ends_line
        {
            let first_is_brace = matches!(next, Some(Token::Symbol('{')));
            self.open_inline_array_brace(
                brace.header.take(),
                brace_type,
                block_indent_extra,
                token_index,
                first_is_brace,
            );
            return true;
        }
        if self.current_is_blank()
            && !self.token_input.token_begins_source_line
            && self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with('['))
        {
            let (line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
            self.current.replace(line);
            self.current.restore_tokens(tokens);
            self.current.push_str(" {");
            self.layout.command_state.observe_char('{');
            self.previous_was_newline = false;
        } else if self.current_is_blank()
            && matches!(
                brace_type,
                BraceType::Array
                    | BraceType::Initializer
                    | BraceType::CompoundLiteral
                    | BraceType::Command
            )
            && !(brace_type == BraceType::Command
                && matches!(
                    self.options.brace_style,
                    BraceStyle::Gnu | BraceStyle::Whitesmith | BraceStyle::Vtk
                )
                && (brace
                    .header
                    .as_deref()
                    .is_some_and(is_break_blocks_closing_header)
                    || self
                        .output
                        .last_non_empty_scoped()
                        .is_some_and(|line| is_break_blocks_closing_header(line.trimmed()))))
            && matches!(next, None | Some(Token::Newline))
            && self.current_inline_array_column().is_some()
            && !self.current_open_brace_is_lambda_body()
        {
            self.open_multiline_attached_initializer_brace(
                brace.header.take(),
                brace_type,
                block_indent_extra,
                false,
            );
            return true;
        }
        false
    }

    /// Places the brace on the current or previous line, or on its own line,
    /// and returns whether it was attached to an output case label.
    fn place_open_brace(
        &mut self,
        brace: &OpenBrace,
        placement: &OpenBracePlacement<'_>,
        next: Option<&Token>,
    ) -> bool {
        let OpenBracePlacement {
            comments:
                OpenBraceComments {
                    line: attached_line_comment,
                    ..
                },
            attach_case_label_brace,
            ..
        } = *placement;
        let mut attached_case_label_output_brace = false;
        let brace_type = brace.brace_type;
        if self.current_is_blank()
            && self.token_input.token_begins_source_line
            && matches!(brace_type, BraceType::Array | BraceType::Initializer)
            && matches!(next, Some(Token::Symbol('{')))
            && self.options.brace_style == BraceStyle::None
        {
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            match self.token_input.next_input_whitespace.as_deref() {
                Some(whitespace) if !whitespace.is_empty() => self.current.push_str(whitespace),
                _ => self.current.push_str("   "),
            }
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces = Some(
                ContinuationIndent::Level(
                    self.layout
                        .indentation
                        .line_indent(LineKind::Normal, self.options),
                )
                .columns(self.options.indent_width),
            );
        } else if self.current_is_blank()
            && matches!(next, Some(Token::Comment(CommentKind::Line, _)))
            && !self.options.add_braces
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && self.output.last().is_some_and(|line| {
                let code = &self.output.code_of(line);
                let trimmed = code.trimmed();
                let first = trimmed
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .next()
                    .unwrap_or_default();
                trimmed.ends_with(')')
                    && !line[trailing_comment_split_limit(line)..]
                        .trimmed_start()
                        .starts_with("/*")
                    && !trimmed.starts_with('#')
                    && !is_header(self.options, first)
            })
        {
            let (mut line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
            self.republish_tokens(tokens);
            line.push('{');
            if let Some(Token::Comment(CommentKind::Line, comment)) = next {
                line.push(' ');
                line.push_str(comment.trimmed_end());
                self.comments.skip_next_attached_comment = true;
            }
            self.layout.command_state.observe_char('{');
            self.adjust_and_publish_line(line);
            self.update_current_brace_indent_from_last_output_line();
            self.previous_was_newline = false;
        } else if self.current_is_blank()
            && !self.token_input.token_begins_source_line
            && self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with('['))
        {
            let (line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
            self.current.replace(line);
            self.current.restore_tokens(tokens);
            self.current.push_str(" {");
            self.layout.command_state.observe_char('{');
            self.previous_was_newline = false;
        } else if attach_case_label_brace
            || self.should_attach_output_initializer_brace(brace_type)
            || self.should_attach_output_header_brace(brace_type)
        {
            attached_case_label_output_brace =
                self.attach_open_brace_to_output_line(brace_type, next);
        } else if matches!(brace_type, BraceType::Definition | BraceType::NonStatement)
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && !self.token_input.token_followed_by_final_line_comment
            // A brace leading its line keeps the comment after it.
            && !self.token_input.token_begins_source_line
            && let Some(comment) = attached_line_comment
        {
            let before_brace = if self.token_input.previous_input_was_adjacent {
                String::new()
            } else {
                self.token_input
                    .previous_input_whitespace
                    .clone()
                    .unwrap_or_default()
                    .into_owned()
            };
            let after_brace = self
                .token_input
                .next_input_whitespace
                .clone()
                .unwrap_or_default();
            self.trim_current_end();
            self.current.push_str(&before_brace);
            self.current.push(' ');
            self.current.push_str(&after_brace);
            self.current.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
            self.finish_line();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            self.finish_line();
        } else if self.current.trimmed_start().starts_with('}')
            && self.current.trimmed_end().ends_with('[')
        {
            self.emit_source_space_or_ensure();
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if !matches!(next, Some(Token::Preprocessor(_) | Token::Symbol('#'))) {
                self.finish_line();
                self.update_current_brace_indent_from_last_output_line();
            }
        } else if self.current_is_blank()
            && self.token_input.token_begins_source_line
            && self.options.brace_style == BraceStyle::Gnu
            && (matches!(next, Some(Token::Symbol('~')))
                || matches!(next, Some(Token::Operator(operator)) if operator == "~"))
            && self.output.scoped().iter().rev().take(3).any(|line| {
                line.contains_from_first_byte("#else") || line.contains_from_first_byte("#define")
            })
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(self.options.indent_width);
            self.current.push_str("{ ");
            self.layout.command_state.observe_char('{');
            self.previous_was_newline = false;
        } else if self.should_attach_opening_brace(brace_type, next) {
            self.attach_open_brace_to_current_line(brace, placement, next);
        } else {
            self.break_open_brace_onto_own_line(brace, placement, next);
        }
        attached_case_label_output_brace
    }

    /// Attaches the brace to the last output line and returns whether that
    /// line is a case label.
    /// Gives the line published next the tokens of the output line taken
    /// back, with the `{` being attached to it.
    fn republish_tokens(&mut self, tokens: Option<TokenSpan>) {
        let brace = self.current.active_token();
        let span = match (tokens, brace) {
            (Some(span), Some(brace)) => Some(TokenSpan {
                first: span.first.min(brace),
                last: span.last.max(brace),
            }),
            (span, None) => span,
            (None, Some(brace)) => Some(TokenSpan {
                first: brace,
                last: brace,
            }),
        };
        self.output.set_pending_tokens(span);
    }

    fn attach_open_brace_to_output_line(
        &mut self,
        brace_type: BraceType,
        next: Option<&Token>,
    ) -> bool {
        let (line, tokens) = self.output.pop_with_tokens().unwrap_or_default();
        self.republish_tokens(tokens);
        // Unstyled, a brace on its label's source line keeps the gap before it.
        let source_gap = (self.options.brace_style == BraceStyle::None
            && !self.token_input.token_begins_source_line)
            .then(|| self.token_input.previous_input_whitespace.clone())
            .flatten()
            .filter(|gap| !gap.is_empty() && !gap.contains('\n'));
        let case_label_line = attach_case_label_brace_to_line(&line, &self.options.access_labels)
            .map(|attached| match &source_gap {
                Some(gap) if trailing_comment_split_limit(&line) == line.len() => {
                    format!("{}{gap}{{", line.trimmed_end())
                }
                _ => attached,
            });
        let attached_case_label_output_brace = case_label_line.is_some();
        let line_with_brace_before_comment = case_label_line
            .is_none()
            .then(|| attach_brace_before_trailing_comment(&line))
            .flatten();
        let brace_attached_before_trailing_comment = line_with_brace_before_comment.is_some();
        let mut line = case_label_line
            .or(line_with_brace_before_comment)
            .unwrap_or_else(|| {
                let mut line = line;
                line.push_str(" {");
                line
            });
        self.layout.command_state.observe_char('{');
        let line_already_has_line_comment =
            line[trailing_comment_split_limit(&line)..].contains("//");
        let line_comment_starts_body =
            brace_attached_before_trailing_comment && self.token_input.token_begins_source_line;
        if let Some(Token::Comment(CommentKind::Line, comment)) = next
            && !line_already_has_line_comment
            && !line_comment_starts_body
        {
            if line.trimmed() == "{"
                && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
            {
                line.push_str(&initializer_brace_line_comment_gap(self.options, &line));
            } else {
                line.push(' ');
            }
            line.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
        } else if matches!(next, Some(Token::Comment(CommentKind::Line, _)))
            && line_comment_starts_body
        {
            self.comments.line_comment_starts_reordered_brace_body = true;
        } else if self.options.brace_style == BraceStyle::OneTrueBrace
            && brace_type == BraceType::Command
            && self.token_input.token_followed_by_line_comment_on_line
            && let Some(Token::Comment(CommentKind::Block, comment)) = next
            && !comment.contains('\n')
        {
            if let Some(whitespace) = self.token_input.next_input_whitespace.as_deref()
                && !whitespace.is_empty()
            {
                line.push_str(whitespace);
            } else {
                line.push(' ');
            }
            line.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
        } else if attached_case_label_output_brace
            && let Some(Token::Comment(CommentKind::Block, comment)) = next
            && !comment.contains('\n')
        {
            if let Some(whitespace) = self.token_input.next_input_whitespace.as_deref()
                && !whitespace.is_empty()
            {
                line.push_str(whitespace);
            } else {
                line.push(' ');
            }
            line.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
        }
        // The line goes through the case unindent again.
        let extra = if attached_case_label_output_brace {
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width
        } else {
            self.layout.line_adjuster.last_line_case_unindent()
        };
        if extra > 0 {
            line = format!("{}{}", " ".repeat(extra), line);
        }
        if brace_attached_before_trailing_comment
            && extra == 0
            && self
                .options
                .max_code_length
                .is_some_and(|max| line.len() > max)
        {
            // The brace before the comment lengthens the line astyle
            // measures for a split.
            let spaces = leading_visual_width(&line, self.options.tab_width);
            let level = self.layout.indentation.indent();
            self.push_formatted_line_exact(line.trimmed_start(), level, spaces);
        } else {
            self.adjust_and_publish_line(line);
        }
        self.update_current_brace_indent_from_last_output_line();
        self.previous_was_newline = false;
        attached_case_label_output_brace
    }

    fn attach_open_brace_to_current_line(
        &mut self,
        brace: &OpenBrace,
        placement: &OpenBracePlacement<'_>,
        next: Option<&Token>,
    ) {
        let OpenBracePlacement {
            comments:
                OpenBraceComments {
                    line: attached_line_comment,
                    block: attached_block_comment,
                    ..
                },
            lambda,
            ..
        } = *placement;
        let line_opens_lambda_body = lambda.line_opens_body;
        let brace_type = brace.brace_type;
        let attached_block_comment_with_line_comment = attached_line_comment.is_none()
            && attached_block_comment.is_some()
            && self.token_input.token_followed_by_final_line_comment
            && self.options.brace_style == BraceStyle::None;
        let reorder_allowed = matches!(
            brace_type,
            BraceType::Command | BraceType::Definition | BraceType::Struct | BraceType::Union
        );
        if reorder_allowed
            && let Some(reordered) = self.reorder_brace_before_current_block_comment()
        {
            if matches!(next, Some(Token::Comment(CommentKind::Line, _))) {
                if self.token_input.token_begins_source_line {
                    self.comments.line_comment_starts_reordered_brace_body = true;
                } else if let Some(gap) = self.token_input.previous_input_whitespace.clone()
                    && !gap.is_empty()
                    && !gap.contains('\n')
                {
                    self.comments.reordered_brace_line_comment_gap = Some(gap.into_owned());
                }
            }
            self.current.replace(reordered);
            self.layout.command_state.observe_char('{');
            self.finish_line();
        } else {
            let block_comment_starts_broken_body = attached_line_comment.is_none()
                && attached_block_comment.is_some()
                && self.layout.line_state.is_one_line_block
                && self.options.break_one_line_blocks
                && !self
                    .token_input
                    .next_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| whitespace.contains('\n'));
            self.emit_opening_brace_space(brace_type);
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if line_opens_lambda_body
                && self
                    .layout
                    .continuation_indent
                    .next_line_indent_spaces
                    .is_some()
                && self.layout.indentation.statement_depth() > 0
            {
                self.layout
                    .continuation_indent
                    .set_next_line_level(self.statement_level());
                self.layout.nesting.clear_continuation_indents();
            }
            let attach_brace_line_block_comment = brace_type == BraceType::Command
                && self.options.brace_style == BraceStyle::OneTrueBrace
                && self.token_input.token_begins_source_line
                && self.token_input.token_followed_by_line_comment_on_line
                && attached_line_comment.is_none()
                && attached_block_comment.is_some();
            if let Some(comment) = attached_line_comment.or(attached_block_comment)
                && !block_comment_starts_broken_body
                && (!self.token_input.token_begins_source_line || attach_brace_line_block_comment)
                && (brace_type != BraceType::Namespace || attached_line_comment.is_some())
            {
                self.push_attached_comment_with_source_gap(comment);
            }
            if !attached_block_comment_with_line_comment {
                self.finish_line();
                self.update_current_brace_indent_from_last_output_line();
            }
        }
    }

    fn break_open_brace_onto_own_line(
        &mut self,
        brace: &OpenBrace,
        placement: &OpenBracePlacement<'_>,
        next: Option<&Token>,
    ) {
        let brace_type = brace.brace_type;
        let source_attached_initializer_line = matches!(
            brace_type,
            BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
        ) && matches!(next, Some(Token::Newline))
            && !matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            );
        let gnu_macro_open_brace =
            self.options.brace_style == BraceStyle::Gnu && has_hash_outside_literals(&self.current);
        let allman_operator_led_brace = matches!(self.options.brace_style, BraceStyle::Allman)
            && self
                .current
                .trimmed_start()
                .starts_with_any(b"<>|&+-*/%=!?:,.");
        let source_attached_malformed_open_brace = !self.options.remove_braces
            && brace.header.is_none()
            && !self.current_is_blank()
            && !self.token_input.token_begins_source_line
            && self
                .token_input
                .previous_input_whitespace
                .as_deref()
                .is_none_or(|whitespace| !whitespace.contains('\n'))
            && !gnu_macro_open_brace
            && !allman_operator_led_brace
            && (source_attached_initializer_line
                || matches!(next, Some(Token::Symbol('{' | '#')))
                || self.current.trimmed_end().ends_with('{')
                || (self.current.trimmed_start().starts_with('}')
                    && self.current.trimmed_end().ends_with('[')));
        if source_attached_malformed_open_brace {
            if self.current.trimmed_end().ends_with('[') {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
            self.current.push('{');
            self.layout.command_state.observe_char('{');
            if !matches!(next, Some(Token::Symbol('{' | '#'))) {
                let body_indent = if source_attached_initializer_line {
                    self.current_line_indent_spaces() + self.options.indent_width
                } else {
                    // The line publishes as it stands, so it takes its
                    // indent here.
                    let lead = self.current_line_indent_spaces();
                    if !self.current.starts_with_any(b" \t") {
                        for _ in 0..lead {
                            self.current.insert(0, ' ');
                        }
                    }
                    leading_visual_width(&self.current, self.options.tab_width)
                        + self.options.indent_width * 2
                };
                self.current_is_preindented = true;
                self.layout
                    .continuation_indent
                    .set_next_line_spaces(body_indent);
                self.finish_line();
                self.update_current_brace_indent_from_last_output_line();
            }
        } else {
            self.open_brace_on_own_line(brace, placement, next);
        }
    }

    fn open_brace_on_own_line(
        &mut self,
        brace: &OpenBrace,
        placement: &OpenBracePlacement<'_>,
        next: Option<&Token>,
    ) {
        let OpenBracePlacement {
            comments:
                OpenBraceComments {
                    line: attached_line_comment,
                    block: attached_block_comment,
                    next_is_trailing: next_is_trailing_comment,
                    ..
                },
            lambda,
            inline_nested_header_level,
            headerless_inline_command_column,
            ..
        } = *placement;
        let previous_line_opens_lambda_body = lambda.previous_line_opens_body;
        let brace_type = brace.brace_type;
        // A comment after the brace of a case label split off before it
        // stays on the label's line.
        if self.current_is_blank()
            && !self.token_input.token_begins_source_line
            && let Some(Token::Comment(kind, comment)) = next
            && (*kind == CommentKind::Line || !comment.contains('\n'))
            && let Some(index) = self.output.len().checked_sub(1)
            && self.output_line_is_case_label(index)
        {
            let before = self
                .token_input
                .previous_input_whitespace
                .clone()
                .unwrap_or_default();
            let after = self
                .token_input
                .next_input_whitespace
                .clone()
                .unwrap_or_default();
            let gap = if before.contains('\t') || after.contains('\t') {
                " ".to_owned()
            } else {
                format!("{before} {after}")
            };
            let line = format!(
                "{}{gap}{}",
                self.output[index].trimmed_end(),
                comment.trimmed_end()
            );
            self.output.set(index, line);
            self.comments.skip_next_attached_comment = true;
        }
        let block_indent_extra = brace.block_indent_extra;
        let comment_starts_block = (self.token_input.token_begins_source_line
            || (self.layout.line_state.is_one_line_block && self.options.break_one_line_blocks))
            && next_is_trailing_comment;
        if let Some(comment) = attached_line_comment.or(attached_block_comment)
            && !comment_starts_block
        {
            self.push_trailing_comment_before_broken_brace(comment);
        }
        let objc_method_brace = self.is_objc_method_line()
            || (brace_type == BraceType::Definition && self.output_ends_objc_method_header());
        let header_text = self.current.trimmed_start();
        let header_is_standalone_colon = header_text.trimmed() == ":"
            || (header_text.trimmed().is_empty()
                && self
                    .output
                    .last_non_empty_scoped()
                    .is_some_and(|line| line.trimmed() == ":"));
        let inline_initializer_command_brace_spaces = (!objc_method_brace)
            .then_some(headerless_inline_command_column)
            .flatten();
        let else_while_header =
            brace_type == BraceType::Command && header_text.starts_with("else while");
        let else_nested_loop_header = brace_type == BraceType::Command
            && (header_text.starts_with("else for") || header_text.starts_with("else switch"));
        let whitesmith_namespace_brace_spaces = (brace_type == BraceType::Namespace
            && self.options.brace_style == BraceStyle::Whitesmith)
            .then(|| {
                let header_spaces = if self.current_is_blank() {
                    self.output
                        .last_non_empty_scoped()
                        .map(|line| leading_visual_width(line, self.options.tab_width))
                        .unwrap_or(0)
                } else {
                    self.current_line_indent_spaces()
                };
                header_spaces
                    + usize::from(self.options.indent_namespaces) * self.options.indent_width
            });
        self.finish_line();
        if let Some(level) = inline_nested_header_level
            .or_else(|| self.layout.inline_nested_header_braceless_bias.take())
            && brace.header.as_deref() != Some("else")
            && matches!(brace_type, BraceType::Command | BraceType::Definition)
        {
            let delta = level.saturating_sub(self.layout.indentation.indent());
            if delta > 0 {
                self.layout.indentation.enter_braceless_block(delta);
            }
        }
        // A row's brace among aligned elements keeps their column.
        let element_row = matches!(
            brace_type,
            BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
        ) && self.current_inline_array_column().is_some();
        if !element_row
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
        {
            self.layout.continuation_indent.clear_next_line();
            self.layout.nesting.clear_continuation_indents();
        }
        let standalone_colon_brace_spaces = (header_is_standalone_colon
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman | BraceStyle::Gnu
            ))
        .then(|| {
            self.output
                .last_non_empty_scoped()
                .map(|line| leading_visual_width(line, self.options.tab_width))
                .unwrap_or(0)
                + if self.options.brace_style == BraceStyle::Gnu {
                    self.options.indent_width * 2
                } else {
                    self.options.indent_width
                }
        });
        let inline_preprocessor_header_brace_spaces = (self.current_is_blank()
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
                    | BraceStyle::Whitesmith
            ))
        .then(|| {
            self.output
                .last_non_empty_scoped()
                .filter(|line| {
                    let trimmed = line.trimmed_start();
                    line.contains_from_first_byte("#if") && !trimmed.starts_with('#')
                })
                .map(|line| leading_visual_width(line, self.options.tab_width))
        })
        .flatten();
        let previous_else_indent_spaces = self
            .current_is_blank()
            .then(|| {
                self.output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| {
                        !self.output.code_of(line).trimmed().is_empty()
                            && !line.trimmed_start().starts_with('#')
                    })
                    .filter(|line| {
                        brace.header.as_deref() == Some("else") || line.trimmed() == "else"
                    })
                    .map(|line| leading_visual_width(line, self.options.tab_width))
            })
            .flatten();
        let closing_header_broken_brace_spaces = previous_else_indent_spaces
            .filter(|_| matches!(self.options.brace_style, BraceStyle::Gnu | BraceStyle::Vtk))
            .map(|spaces| spaces + self.options.indent_width);
        let whitesmith_broken_brace_spaces = (self.current_is_blank()
            && match self.options.brace_style {
                BraceStyle::Whitesmith => !objc_method_brace,
                BraceStyle::Vtk => !objc_method_brace && self.should_indent_brace_line(brace_type),
                _ => false,
            }
            && (brace_type != BraceType::Namespace || self.options.indent_namespaces))
            .then(|| {
                previous_else_indent_spaces
                    .map(|spaces| spaces + self.options.indent_width)
                    .unwrap_or_else(|| {
                        (self.layout.indentation.indent()
                            + 1
                            + self.case_body_indent_extra(LineKind::Normal))
                            * self.options.indent_width
                    })
            });
        if self.current_is_blank()
            && let Some(previous_else_indent_spaces) = previous_else_indent_spaces
        {
            let extra = usize::from(matches!(
                self.options.brace_style,
                BraceStyle::Gnu | BraceStyle::Whitesmith | BraceStyle::Vtk
            )) + self.case_body_indent_extra(LineKind::Normal);
            self.layout.continuation_indent.set_next_line_spaces(
                previous_else_indent_spaces + extra * self.options.indent_width,
            );
        }
        if else_while_header {
            self.layout.continuation_indent.set_next_line_level(
                self.statement_level() + usize::from(self.options.indent_blocks) + 2,
            );
        } else if else_nested_loop_header {
            self.layout.continuation_indent.set_next_line_level(
                self.statement_level()
                    + usize::from(self.options.indent_blocks)
                    + usize::from(inline_nested_header_level.is_none()),
            );
        }
        if brace_type == BraceType::Command
            && self.options.brace_style == BraceStyle::Gnu
            && self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with("})"))
        {
            let level = self.layout.indentation.indent() + 1;
            self.layout.continuation_indent.set_next_line_level(level);
            self.layout.inline_nested_header_braceless_bias = Some(level);
        }
        if self.current_is_blank()
            && self
                .output
                .last_non_empty_scoped()
                .is_some_and(|line| line.trimmed() == "else")
        {
            self.clear_current();
        }
        self.current.push('{');
        self.layout.command_state.observe_char('{');
        let attach_runin_comment = self.token_input.token_begins_source_line
            && self.layout.command_state.current_header.is_none()
            && match brace_type {
                BraceType::Command => self.options.brace_style == BraceStyle::OneTrueBrace,
                BraceType::Array | BraceType::Initializer => true,
                _ => false,
            };
        let runin_comment = if attach_runin_comment {
            match next {
                // A bare block moves a line comment into its body.
                Some(Token::Comment(CommentKind::Line, comment))
                    if brace_type != BraceType::Command =>
                {
                    Some(comment.as_str())
                }
                // A block comment before the first element stays with it.
                Some(Token::Comment(CommentKind::Block, comment))
                    if !comment.contains('\n') && self.comments.next_comment_ends_line =>
                {
                    Some(comment.as_str())
                }
                _ => None,
            }
        } else {
            None
        };
        if let Some(comment) = runin_comment {
            if comment.trimmed_start().starts_with("//")
                && self.current.trimmed() == "{"
                && (self.in_initializer_brace() || self.current_inline_array_column().is_some())
            {
                let gap = initializer_brace_line_comment_gap(self.options, &self.current);
                self.current.push_str(&gap);
            } else {
                match self.token_input.next_input_whitespace.as_deref() {
                    Some(ws) if !ws.is_empty() => self.current.push_str(ws),
                    // A bare block keeps a comment written against its brace.
                    _ if brace_type == BraceType::Command => {}
                    _ => self.current.push(' '),
                }
            }
            self.current.push_str(comment.trimmed_end());
            self.comments.skip_next_attached_comment = true;
        }
        if (!objc_method_brace && self.should_indent_brace_line(brace_type))
            || block_indent_extra > 0
        {
            self.layout
                .continuation_indent
                .set_next_line_level(self.statement_level() + 1);
        }
        if let Some(spaces) = inline_initializer_command_brace_spaces
            .or(standalone_colon_brace_spaces)
            .or(inline_preprocessor_header_brace_spaces)
            .or(whitesmith_namespace_brace_spaces)
            .or(whitesmith_broken_brace_spaces)
            .or(closing_header_broken_brace_spaces)
        {
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        }
        if objc_method_brace {
            self.layout.continuation_indent.next_line_indent = None;
            self.layout.continuation_indent.next_line_indent_spaces =
                Some(self.layout.indentation.indent() * self.options.indent_width);
        }
        self.finish_line();
        self.update_current_brace_indent_from_last_output_line();
        if let Some(spaces) = whitesmith_namespace_brace_spaces
            .or(whitesmith_broken_brace_spaces)
            .or_else(|| {
                (self.options.brace_style == BraceStyle::Vtk)
                    .then_some(closing_header_broken_brace_spaces)
                    .flatten()
            })
        {
            self.layout.continuation_indent.set_next_line_spaces(spaces);
        } else if let Some(spaces) = closing_header_broken_brace_spaces {
            self.layout
                .continuation_indent
                .set_next_line_spaces(spaces + self.options.indent_width);
        } else if let Some(spaces) = inline_initializer_command_brace_spaces
            .or(standalone_colon_brace_spaces)
            .or(inline_preprocessor_header_brace_spaces)
        {
            self.layout
                .continuation_indent
                .set_next_line_spaces(spaces + self.options.indent_width);
        }
        // Every style runs a brace into a nested brace that follows it;
        // run-in styles do that for all braces later.
        let runs_into_nested_brace = matches!(next, Some(Token::Symbol('{')))
            && matches!(brace_type, BraceType::Array | BraceType::Initializer)
            && !matches!(
                self.options.brace_style,
                BraceStyle::Horstmann | BraceStyle::Pico
            );
        if (self.options.brace_style == BraceStyle::None || runs_into_nested_brace)
            && brace_type != BraceType::Namespace
            && self.token_input.token_begins_source_line
            && !previous_line_opens_lambda_body
            && !matches!(next, None | Some(Token::Newline))
            && (!(self.options.break_one_line_blocks && self.layout.line_state.is_one_line_block)
                || matches!(
                    brace_type,
                    BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
                ))
            && self.output.last().is_some_and(|line| line.trimmed() == "{")
        {
            self.source_run_in_brace_lines.push(self.output.len() - 1);
        }
        if comment_starts_block
            && self.options.brace_style == BraceStyle::None
            && brace_type != BraceType::Namespace
        {
            self.schedule_run_in_comment_brace_merge(self.output.len() - 1);
        }
    }

    fn push_trailing_comment_before_broken_brace(&mut self, comment: &str) {
        let before_gap = if self.token_input.previous_input_was_adjacent {
            String::new()
        } else {
            self.token_input
                .previous_input_whitespace
                .clone()
                .unwrap_or_default()
                .into_owned()
        };
        let after_gap = self
            .token_input
            .next_input_whitespace
            .clone()
            .unwrap_or_default();
        self.trim_current_end();
        if let Some(&target) = self.layout.line_state.trailing_comment_columns.front()
            && !before_gap.contains('\t')
            && !after_gap.contains('\t')
        {
            // The comment keeps its source column past code that a moved
            // closing brace and the broken brace left.
            let code_len = self.current.trimmed().chars().count();
            let run_in_closer = self.run_in_closer_row() == Some(true);
            let target = target
                .saturating_sub(self.broken_else_if_prefix_width())
                .saturating_sub(usize::from(run_in_closer));
            self.current
                .push_str(&" ".repeat(target.saturating_sub(code_len).max(1)));
        } else {
            self.current.push_str(&before_gap);
            self.current.push(' ');
            self.current.push_str(&after_gap);
        }
        self.current.push_str(comment.trimmed_end());
        self.comments.skip_next_attached_comment = true;
    }

    /// The source width of what a broken else-if moved off the line its
    /// brace is on, before the `if`.
    /// Width of the `else` an `else switch`-like line left on the line
    /// before, which the comment's source column counts.
    fn broken_off_else_prefix_width(&self) -> usize {
        let tokens = &self.tree.tokens;
        let Some(first) = self.current.tokens().map(|span| span.first) else {
            return 0;
        };
        if first >= tokens.len() {
            return 0;
        }
        let line_start = tokens[..first]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |newline| newline + 1);
        let Some(code_start) = (line_start..first).find(|&index| is_code_token(&tokens[index]))
        else {
            return 0;
        };
        if !tokens[code_start..first].iter().all(|token| {
            matches!(token, Token::Whitespace(_))
                || matches!(token, Token::Word(word) if word == "else")
        }) {
            return 0;
        }
        tokens[code_start..first].iter().map(token_char_len).sum()
    }

    fn broken_else_if_prefix_width(&self) -> usize {
        if !self.current.trimmed_start().starts_with("if")
            || !self
                .output
                .last()
                .is_some_and(|line| line.trimmed_end().ends_with("else"))
        {
            return 0;
        }
        let tokens = &self.tree.tokens;
        let Some(brace) = self
            .current
            .active_token()
            .filter(|&brace| brace < tokens.len())
        else {
            return 0;
        };
        let line_start = tokens[..brace]
            .iter()
            .rposition(|token| matches!(token, Token::Newline))
            .map_or(0, |newline| newline + 1);
        let Some(first) = (line_start..brace).find(|&index| is_code_token(&tokens[index])) else {
            return 0;
        };
        (first..brace)
            .find(|&index| matches!(&tokens[index], Token::Word(word) if word == "if"))
            .map_or(0, |header| {
                tokens[first..header].iter().map(token_char_len).sum()
            })
    }

    fn enter_pushed_brace_scope(
        &mut self,
        mut brace: OpenBrace,
        inline_nested_header_level: Option<usize>,
        headerless_inline_command_column: Option<usize>,
        attached_case_label: bool,
    ) {
        let brace_type = brace.brace_type;
        let block_indent_extra = brace.block_indent_extra;
        let class_block_indent_extra = brace.class_block_indent_extra;
        let lambda_body_breaks_before_call = brace.breaks_before_call;
        if let Some(level) = inline_nested_header_level
            .or_else(|| self.layout.inline_nested_header_braceless_bias.take())
            && brace.header.as_deref() != Some("else")
            && matches!(brace_type, BraceType::Command | BraceType::Definition)
        {
            let delta = level.saturating_sub(self.layout.indentation.indent());
            if delta > 0 {
                self.layout.indentation.enter_braceless_block(delta);
            }
        }
        if brace.header.as_deref() == Some("else") {
            let brace_level = self
                .output
                .last()
                .map(|line| {
                    leading_visual_width(line, self.options.tab_width) / self.options.indent_width
                })
                .unwrap_or_else(|| self.layout.indentation.indent());
            while let Some((base, delta)) = self.layout.indentation.last_braceless_block()
                && self.layout.indentation.indent() == base + delta
                && self.layout.indentation.indent() > brace_level
            {
                self.layout.indentation.exit_braceless_block();
            }
        }
        self.layout
            .nesting
            .enter_brace(brace.header.take(), brace_type, block_indent_extra);
        if lambda_body_breaks_before_call {
            self.layout.nesting.mark_current_brace_break_before_call();
        }
        let cpp_extern_c_block =
            brace_type == BraceType::Extern && self.extern_c_guard == ExternCGuard::ExternC;
        if cpp_extern_c_block {
            self.extern_c_guard = ExternCGuard::InsideBlock;
        }
        if let Some(opening_indent) = headerless_inline_command_column {
            self.layout.indentation.enter_block_without_indent(false);
            self.inline_array.frames.push(InlineArrayFrame {
                depth: self.layout.nesting.brace_header_stack.len(),
                body_column: opening_indent + self.options.indent_width,
                brace_column: opening_indent,
                output_line: self.output.len(),
                aggregate_assignment: false,
            });
        } else if (brace_type == BraceType::Namespace && !self.options.indent_namespaces)
            || cpp_extern_c_block
        {
            self.layout.indentation.enter_block_without_indent(false);
        } else {
            // A brace opening directly after another brace on the same line (`{{`) nests one
            // level deeper than the shared line indent, so its body indents twice.
            let opens_double_brace = self.current_is_blank()
                && self
                    .output
                    .last()
                    .is_some_and(|line| line.trimmed_end().ends_with("{{"));
            let double_brace_extra = usize::from(
                matches!(
                    brace_type,
                    BraceType::Array
                        | BraceType::Initializer
                        | BraceType::CompoundLiteral
                        | BraceType::DeferArray
                ) && opens_double_brace,
            );
            self.layout.indentation.enter_block_with_extra(
                false,
                block_indent_extra + class_block_indent_extra + double_brace_extra,
            );
        }
        if attached_case_label {
            self.register_attached_case_label_brace();
        }
    }

    pub(super) fn emit_opening_brace_space(&mut self, brace_type: BraceType) {
        if self.current_is_lambda_body_header() {
            self.emit_source_space_or_ensure();
            return;
        }
        if is_lambda_capture_header(self.current.trimmed_end()) {
            let current = self.current.trimmed_end();
            let embedded = current
                .rfind('[')
                .is_some_and(|index| current[..index].contains('{'));
            if self.options.brace_style == BraceStyle::None && embedded {
                self.emit_source_space();
            } else {
                self.emit_source_space_or_ensure();
            }
            return;
        }
        let init_after_declarator = brace_type == BraceType::Initializer
            && (self
                .layout
                .command_state
                .previous_command_char
                .is_some_and(|ch| is_word_char(ch) || ch == ']' || ch == '>')
                || self.current.trimmed_end().ends_with('>'));
        // A compound literal nested in an aggregate keeps the gap written
        // before its brace, as any brace there does.
        let nested_compound_literal = brace_type == BraceType::CompoundLiteral
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral)
            );
        if self.is_nested_designated_init_field() || nested_compound_literal {
            if self.options.pad_operators && self.current.trimmed_end().ends_with('=') {
                self.emit_source_space_or_ensure();
            } else {
                self.emit_source_space();
            }
        } else if self.layout.previous == PreviousToken::OpenParen && self.options.pad_parens_inside
        {
            self.pad_inside_paren_space();
        } else if self.layout.command_state.previous_command_char == Some('(')
            || self.layout.command_state.previous_command_char == Some('{')
            || self.layout.command_state.previous_command_char == Some(',')
            || init_after_declarator
        {
            self.emit_source_space();
        } else {
            self.emit_source_space_or_ensure();
        }
    }

    fn should_open_multiline_attached_initializer_brace(
        &self,
        brace_type: BraceType,
        next: Option<&Token>,
    ) -> bool {
        if self.should_open_control_paren_init_brace(brace_type, next) {
            return true;
        }
        !self.current_is_blank()
            && brace_type == BraceType::CompoundLiteral
            && matches!(next, None | Some(Token::Newline))
            && line_ends_compound_literal_cast(self.current.trimmed_end())
            && !line_ends_lambda_parameter_list(self.current.trimmed_end())
            && self.should_attach_opening_brace(brace_type, next)
    }

    fn should_open_control_paren_init_brace(
        &self,
        brace_type: BraceType,
        next: Option<&Token>,
    ) -> bool {
        matches!(
            brace_type,
            BraceType::Array
                | BraceType::Initializer
                | BraceType::CompoundLiteral
                | BraceType::Command
        ) && matches!(next, None | Some(Token::Newline))
            && self.layout.nesting.paren_depth > 0
            && self.control_paren_init_brace_indent_spaces().is_some()
    }

    pub(super) fn control_paren_init_brace_indent_spaces(&self) -> Option<usize> {
        if self.layout.nesting.paren_depth == 0
            || !matches!(
                self.options.brace_style,
                BraceStyle::OneTrueBrace | BraceStyle::Attach
            )
        {
            return None;
        }
        self.for_header_continuation_indent_spaces()
    }

    fn should_attach_control_paren_init_brace_from_previous_line(
        &self,
        brace_type: BraceType,
    ) -> bool {
        matches!(
            self.options.brace_style,
            BraceStyle::OneTrueBrace | BraceStyle::Attach
        ) && matches!(
            brace_type,
            BraceType::Array
                | BraceType::Initializer
                | BraceType::CompoundLiteral
                | BraceType::Command
        ) && self.token_input.token_begins_source_line
            && self.current_is_blank()
            && self.layout.nesting.paren_depth > 0
            && self.output.last().is_some_and(|line| {
                let trimmed = line.trimmed();
                trimmed.starts_with("for (") && trimmed.ends_with(':')
            })
    }

    fn class_block_indent_extra(&self, brace_type: BraceType, token_index: usize) -> usize {
        if !self.options.indent_classes {
            return 0;
        }
        match brace_type {
            BraceType::Class => 1,
            BraceType::Struct => usize::from(self.access_modified_braces.contains(&token_index)),
            _ => 0,
        }
    }

    pub(crate) fn should_indent_brace_line(&self, brace_type: BraceType) -> bool {
        if self.options.brace_style == BraceStyle::Whitesmith {
            return brace_type != BraceType::Namespace || self.options.indent_namespaces;
        }
        if self.options.brace_style == BraceStyle::Vtk
            && matches!(
                brace_type,
                BraceType::Initializer | BraceType::CompoundLiteral
            )
        {
            // An initializer at file scope opens flush, however its
            // declaration continued.
            let at_file_scope = self
                .current
                .active_token()
                .and_then(|brace| self.tree.groups.opened_at(brace))
                .is_some_and(|group| self.tree.groups.get(group).parent.is_none());
            return !at_file_scope && self.layout.indentation.indent() > 0;
        }
        if self.options.brace_style == BraceStyle::Ratliff && brace_type == BraceType::Initializer {
            return true;
        }
        if brace_type == BraceType::Namespace {
            return self.options.indent_braces && self.options.indent_namespaces;
        }
        if !brace_indent_applies(brace_type) {
            return false;
        }
        match self.options.brace_style {
            // astyle indents the braces of a type inside a function or an
            // `extern "C"` block, and of a function inside the latter, like
            // a statement block's.
            BraceStyle::Vtk => {
                let enclosed_by = |types: &[BraceType]| {
                    self.layout
                        .nesting
                        .brace_type_stack
                        .iter()
                        .any(|enclosing| types.contains(enclosing))
                };
                brace_type == BraceType::Command
                    || (brace_type == BraceType::Array && self.layout.indentation.indent() > 0)
                    || matches!(
                        brace_type,
                        BraceType::Class | BraceType::Struct | BraceType::Union | BraceType::Enum
                    ) && enclosed_by(&[
                        BraceType::Definition,
                        BraceType::Command,
                        BraceType::Extern,
                    ])
                    || brace_type == BraceType::Definition && enclosed_by(&[BraceType::Extern])
            }
            BraceStyle::Whitesmith => {
                brace_type != BraceType::Namespace || self.options.indent_namespaces
            }
            _ => self.options.indent_braces,
        }
    }

    fn output_line_is_case_label(&self, index: usize) -> bool {
        let line = &self.output[index];
        let trimmed = self.output.code_before_comment(index).trimmed();
        trimmed.ends_with(':')
            && trimmed.len() == line.trimmed().len()
            && labels::is_label_start(trimmed.trim_end_matches(':'), &self.options.access_labels)
    }

    fn should_attach_output_case_label_brace(&self, brace_type: BraceType) -> bool {
        if !self.current_is_blank()
            || !matches!(brace_type, BraceType::NonStatement | BraceType::Command)
        {
            return false;
        }
        let after_case_label = self.output.last().is_some_and(|line| {
            let code = &self.output.code_of(line);
            let trimmed = code.trimmed();
            trimmed.ends_with(':')
                && labels::is_label_start(
                    trimmed.trim_end_matches(':'),
                    &self.options.access_labels,
                )
        });
        if !after_case_label {
            return false;
        }
        match self.options.brace_style {
            BraceStyle::None => !self.token_input.token_begins_source_line,
            BraceStyle::Attach
            | BraceStyle::OneTrueBrace
            | BraceStyle::WebKit
            | BraceStyle::Ratliff
            | BraceStyle::Lisp => true,
            BraceStyle::Allman
            | BraceStyle::Whitesmith
            | BraceStyle::Vtk
            | BraceStyle::Gnu
            | BraceStyle::Horstmann
            | BraceStyle::Pico => false,
        }
    }

    fn should_attach_output_initializer_brace(&self, brace_type: BraceType) -> bool {
        if !self.current_is_blank() {
            return false;
        }
        let Some(last) = self.output.last() else {
            return false;
        };
        let last = last.trimmed_end();
        // A directive line takes no brace.
        if last.trimmed_start().starts_with('#') {
            return false;
        }
        // A brace that would take the line past the maximum length stays on
        // its own.
        if self.options.max_code_length.is_some_and(|max| {
            leading_visual_width(last, self.options.tab_width)
                + last.trimmed_start().len()
                + " {".len()
                > max
        }) {
            return false;
        }
        if matches!(brace_type, BraceType::Array | BraceType::Initializer)
            && (last.ends_with('=') || last.ends_with(',') && self.layout.nesting.paren_depth > 0)
            && matches!(
                self.options.brace_style,
                BraceStyle::OneTrueBrace
                    | BraceStyle::WebKit
                    | BraceStyle::Attach
                    | BraceStyle::Lisp
                    | BraceStyle::Ratliff
            )
        {
            return true;
        }
        // A compound literal nested in an aggregate keeps its place, as any
        // brace there does.
        line_ends_compound_literal_cast(last)
            && (matches!(
                self.options.brace_style,
                BraceStyle::Attach | BraceStyle::Lisp | BraceStyle::Ratliff
            ) || matches!(
                self.options.brace_style,
                BraceStyle::OneTrueBrace | BraceStyle::WebKit
            ) && !matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral)
            ))
    }

    fn should_attach_output_header_brace(&self, brace_type: BraceType) -> bool {
        // A type header that a comment ends takes the brace its style
        // attaches before the comment.
        if matches!(
            brace_type,
            BraceType::Struct
                | BraceType::Union
                | BraceType::Class
                | BraceType::Interface
                | BraceType::Enum
        ) && !self.preprocessor.last_output_was_preprocessor
            && self.current_is_blank()
            && self.output.last().is_some_and(|last| {
                let comment_start =
                    line_comment_split_limit(last).min(trailing_comment_split_limit(last));
                let code = last[..comment_start].trimmed();
                comment_start < last.len()
                    && !code.is_empty()
                    && !code.starts_with('#')
                    && !code.ends_with_any(b";{},")
                    // astyle leaves an enum's brace after a block comment.
                    && (brace_type != BraceType::Enum
                        || last[comment_start..].trimmed_start().starts_with("//"))
            })
        {
            return self.style_attaches_opening_brace(brace_type, None);
        }
        if !matches!(
            brace_type,
            BraceType::Command | BraceType::Definition | BraceType::NonStatement
        ) || self.preprocessor.last_output_was_preprocessor
            || !self.current_is_blank()
        {
            return false;
        }
        if !matches!(
            self.options.brace_style,
            BraceStyle::Attach
                | BraceStyle::OneTrueBrace
                | BraceStyle::WebKit
                | BraceStyle::Ratliff
                | BraceStyle::Lisp
        ) {
            return false;
        }
        if brace_type == BraceType::Definition
            && matches!(
                self.options.brace_style,
                BraceStyle::OneTrueBrace | BraceStyle::WebKit
            )
            && !matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Command | BraceType::Definition)
            )
        {
            return false;
        }
        if brace_type != BraceType::Command
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && self.token_input.token_followed_by_line_comment_on_line
            && self.output.last().is_some_and(|line| {
                let trimmed = line.trimmed_end();
                trimmed.ends_with("*/") && trimmed.contains("/*")
            })
        {
            return false;
        }
        let objc_method_header =
            brace_type == BraceType::Definition && self.output_ends_objc_method_header();
        self.output.last().is_some_and(|last| {
            let line_comment_start = line_comment_split_limit(last);
            let comment_start = if line_comment_start < last.len() {
                line_comment_start
            } else {
                trailing_comment_split_limit(last)
            };
            let code = &last[..comment_start];
            let trimmed = code.trimmed();
            if trimmed.starts_with('#') {
                return false;
            }
            if comment_start == last.len() {
                return trimmed.ends_with(')')
                    || is_lambda_body_header(trimmed)
                    || objc_method_header;
            }
            let comment = last[comment_start..].trimmed();
            if brace_type != BraceType::Command {
                return matches!(
                    self.options.brace_style,
                    BraceStyle::Attach | BraceStyle::Ratliff | BraceStyle::Lisp
                ) && (trimmed.ends_with(')') || objc_method_header)
                    && (is_single_trailing_block_comment(comment) || comment.starts_with("//"));
            }
            let last_word = trimmed
                .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .next()
                .unwrap_or_default();
            if !trimmed.ends_with(')') && !matches!(last_word, "else" | "do" | "try") {
                return false;
            }
            is_single_trailing_block_comment(comment) || comment.starts_with("//")
        })
    }

    fn should_attach_opening_brace(&self, brace_type: BraceType, next: Option<&Token>) -> bool {
        !self.current_is_blank() && self.style_attaches_opening_brace(brace_type, next)
    }

    pub(crate) fn style_attaches_opening_brace(
        &self,
        brace_type: BraceType,
        next: Option<&Token>,
    ) -> bool {
        if self.options.break_one_line_blocks
            && self.options.brace_style == BraceStyle::OneTrueBrace
            && brace_type != BraceType::Command
            && !matches!(next, None | Some(Token::Newline))
            && self.current.trimmed_end().ends_with(')')
            && self.current.last_open_paren().is_some()
            && !matches!(
                self.current
                    .trimmed_start()
                    .trim_start_matches('}')
                    .trimmed_start()
                    .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .next(),
                Some("if" | "else" | "for" | "while" | "switch" | "catch" | "__except")
            )
        {
            return false;
        }
        // Only `extern "C"` keeps a brace of its own placing; other
        // linkages, as `extern "C++"`, take the style's for any block.
        let brace_type = if brace_type == BraceType::Extern
            && self.current.contains_from_first_byte("\"C++\"")
        {
            BraceType::Command
        } else {
            brace_type
        };
        if brace_type == BraceType::Extern {
            if self.options.attach_extern_c {
                return true;
            }
            if !matches!(
                self.options.brace_style,
                BraceStyle::Horstmann | BraceStyle::Pico
            ) {
                return !self.token_input.token_begins_source_line;
            }
        }
        if self.options.attach_namespace && brace_type == BraceType::Namespace {
            return true;
        }
        if self.options.attach_class && brace_type == BraceType::Class {
            return true;
        }
        if brace_type == BraceType::Enum
            && self.options.attach_enum
            && matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            )
        {
            return !self.token_input.token_begins_source_line;
        }
        // Run-in braces leave inline functions broken.
        if self.options.attach_inline
            && !matches!(
                self.options.brace_style,
                BraceStyle::Horstmann | BraceStyle::Pico
            )
            && matches!(
                brace_type,
                BraceType::Command
                    | BraceType::NonStatement
                    | BraceType::Definition
                    | BraceType::Initializer
            )
            && self
                .layout
                .nesting
                .brace_type_stack
                .contains(&BraceType::Class)
        {
            return true;
        }
        if matches!(
            brace_type,
            BraceType::Array | BraceType::CompoundLiteral | BraceType::Initializer
        ) && matches!(next, None | Some(Token::Newline))
        {
            let break_mode = matches!(
                self.options.brace_style,
                BraceStyle::Allman
                    | BraceStyle::Whitesmith
                    | BraceStyle::Vtk
                    | BraceStyle::Gnu
                    | BraceStyle::Horstmann
                    | BraceStyle::Pico
            );
            if (break_mode
                || (self.options.brace_style == BraceStyle::None
                    && self.token_input.token_begins_source_line))
                && !self.output.is_empty()
                && !self.is_nested_designated_init_field()
                // Like any brace nested in an aggregate, a compound literal's
                // keeps its place.
                && !(brace_type == BraceType::CompoundLiteral
                    && matches!(
                        self.layout.nesting.brace_type_stack.last(),
                        Some(BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral)
                    ))
            {
                return false;
            }
            return true;
        }
        if matches!(
            brace_type,
            BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
        ) && matches!(
            self.options.brace_style,
            BraceStyle::Attach
                | BraceStyle::OneTrueBrace
                | BraceStyle::WebKit
                | BraceStyle::Ratliff
                | BraceStyle::Lisp
        ) && !self.token_input.token_begins_source_line
            && !matches!(next, None | Some(Token::Newline))
        {
            return true;
        }
        if self.current.trimmed_end().ends_with(')')
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Command | BraceType::Definition)
            )
            && matches!(
                self.options.brace_style,
                BraceStyle::Attach
                    | BraceStyle::OneTrueBrace
                    | BraceStyle::WebKit
                    | BraceStyle::Ratliff
                    | BraceStyle::Lisp
            )
        {
            return true;
        }
        if self.current.trimmed_start().starts_with('[')
            && self.current_is_lambda_body_header()
            && self.current.has_arrow_after_paren()
            && matches!(
                self.options.brace_style,
                BraceStyle::Attach
                    | BraceStyle::OneTrueBrace
                    | BraceStyle::WebKit
                    | BraceStyle::Ratliff
                    | BraceStyle::Lisp
            )
        {
            return true;
        }
        match self.options.brace_style {
            BraceStyle::None => !self.token_input.token_begins_source_line,
            BraceStyle::Attach | BraceStyle::Ratliff | BraceStyle::Lisp => !matches!(
                brace_type,
                BraceType::Array | BraceType::Initializer | BraceType::CompoundLiteral
            ),
            BraceStyle::OneTrueBrace => {
                !matches!(
                    brace_type,
                    BraceType::NonStatement
                        | BraceType::Extern
                        | BraceType::Namespace
                        | BraceType::Class
                        | BraceType::Interface
                        | BraceType::Definition
                        | BraceType::Initializer
                ) && (brace_type != BraceType::Struct || self.options.attach_struct)
                    && (brace_type != BraceType::Enum || self.options.attach_enum)
            }
            BraceStyle::WebKit => !matches!(
                brace_type,
                BraceType::Array
                    | BraceType::CompoundLiteral
                    | BraceType::Definition
                    | BraceType::Initializer
            ),
            BraceStyle::Allman
            | BraceStyle::Whitesmith
            | BraceStyle::Vtk
            | BraceStyle::Gnu
            | BraceStyle::Horstmann
            | BraceStyle::Pico => false,
        }
    }
}
