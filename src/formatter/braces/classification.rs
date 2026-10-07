use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::braces::compound_literals::line_ends_compound_literal_cast;
use crate::formatter::braces::rewrite::is_defer_header;
use crate::formatter::constructs::headers::is_conditional_header_line;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::preprocessor::is_cplusplus_conditional;
use crate::formatter::state::BraceType;
use crate::formatter::structure::blocks::BlockKind;
use crate::formatter::syntax::language;
use crate::formatter::syntax::language::is_macro_like_word;
use crate::formatter::text::line_scan::{
    ContainsAnyByte, is_comment_only_line, trailing_comment_split_limit, trailing_matching_parens,
    unmatched_open_paren_column,
};
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_word_char, trailing_word};

pub(crate) fn line_opens_lambda_block(line: &str) -> bool {
    let trimmed = line.trimmed();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let head = head.trimmed_end();
    is_lambda_body_header(head)
        || head
            .rfind('[')
            .is_some_and(|index| is_lambda_body_header(head[index..].trimmed_start()))
}

pub(crate) fn is_lambda_body_header(head: &str) -> bool {
    // A lambda's capture list closes with `]`.
    if !head.as_bytes().contains(&b']') {
        return false;
    }
    is_lambda_body_header_with_arrow(head, head.rfind("->"))
}

/// `is_lambda_body_header` of `head`, whose last `->` is at `arrow`.
fn is_lambda_body_header_with_arrow(head: &str, arrow: Option<usize>) -> bool {
    let mut head = head.trimmed_end();
    if let Some(arrow) = arrow {
        let before = head[..arrow].trimmed_end();
        if before.ends_with(')') {
            head = before;
        }
    }
    if !head.ends_with(')') {
        return false;
    }
    let Some(open) = matching_open_index(head, '(', ')') else {
        return false;
    };
    let capture = head[..open].trimmed_end();
    if !capture.ends_with(']') {
        return false;
    }
    let Some(lb) = matching_open_index(capture, '[', ']') else {
        return false;
    };
    match capture[..lb].trimmed_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

pub(crate) fn is_lambda_capture_header(head: &str) -> bool {
    let head = head.trimmed_end();
    if !head.ends_with(']') {
        return false;
    }
    let Some(lb) = matching_open_index(head, '[', ']') else {
        return false;
    };
    match head[..lb].trimmed_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

pub(super) fn line_opens_parameterized_lambda_block(line: &str) -> bool {
    if !line_opens_lambda_block(line) {
        return false;
    }
    let trimmed = line.trimmed();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let Some(capture_end) = head.rfind(']') else {
        return false;
    };
    head[capture_end + 1..].trimmed_start().starts_with('(')
}

pub(crate) fn line_opens_lambda_or_capture_only_block(line: &str) -> bool {
    if line_opens_lambda_block(line) {
        return true;
    }
    let trimmed = line.trimmed();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let head = head.trimmed_end();
    let Some(capture_start) = head.rfind('[') else {
        return false;
    };
    let capture = head[capture_start..].trimmed();
    if !capture.ends_with(']') {
        return false;
    }
    match head[..capture_start].trimmed_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

fn is_namespace_block_header(line: &str) -> bool {
    let trimmed = line.trimmed();
    trimmed == "namespace"
        || trimmed.starts_with("namespace ")
        || trimmed.starts_with("inline namespace ")
}

fn current_ends_block_literal_header(head: &str) -> bool {
    if !head.ends_with(')') {
        return false;
    }
    let Some(open) = matching_open_index(head, '(', ')') else {
        return false;
    };
    let before = head[..open].trimmed_end().trim_end_matches(|ch: char| {
        is_word_char(ch) || matches!(ch, '*' | '&' | ' ' | '\t' | ':')
    });
    let Some(rest) = before.strip_suffix('^') else {
        return false;
    };
    match rest.trimmed_end().chars().next_back() {
        None => true,
        Some(ch) => matches!(ch, '=' | '(' | ',' | '[' | ':' | '{'),
    }
}

fn matching_open_index(text: &str, open: char, close: char) -> Option<usize> {
    if !text.ends_with(close) {
        return None;
    }
    let mut depth = 0i32;
    for (index, ch) in text.char_indices().rev() {
        if ch == close {
            depth += 1;
        } else if ch == open {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

pub(super) fn is_namespace_or_module_block_header(line: &str) -> bool {
    is_namespace_block_header(line) || line.trimmed_start().starts_with("module ")
}

impl FormatEngine<'_> {
    pub(super) fn classify_opening_brace(
        &mut self,
        header: Option<&str>,
        pending_extern: bool,
    ) -> BraceType {
        let block_word = self.layout.command_state.pending_block_word.take();
        let block_word = match block_word.as_deref() {
            Some("struct" | "union" | "enum" | "class" | "interface")
                if self.aggregate_header_ends_with_paren_group()
                    || self.layout.command_state.previous_command_char == Some(')') =>
            {
                None
            }
            _ => block_word,
        };
        let lambda_header = self.current_is_lambda_body_header()
            || is_lambda_capture_header(self.current.trimmed_end());
        let lambda_in_block_scope = lambda_header
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Command | BraceType::Definition)
            );
        let active_delimiter = self
            .layout
            .frame_stack
            .active_delimiter_with_id()
            .map(|(id, _)| id);
        let header_condition_closed = header.is_some_and(|header| {
            (language::is_non_paren_header(header)
                || matches!(header, "autoreleasepool" | "@try" | "@finally")
                || self.layout.command_state.previous_command_char == Some(')'))
                && self
                    .layout
                    .frame_stack
                    .active_header()
                    .is_some_and(|frame| {
                        frame.header == header && frame.parent_delimiter == active_delimiter
                    })
        });
        let current_namespace_header = {
            let split = if self.current.holds_comment_opener() {
                trailing_comment_split_limit(&self.current)
            } else {
                self.current.len()
            };
            is_namespace_block_header(&self.current[..split])
        };
        let previous_namespace_header = self.current_is_blank()
            && self.output.last_non_empty_scoped().is_some_and(|line| {
                let code = &self.output.code_of(line);
                is_namespace_block_header(code) && !code.trimmed_end().ends_with('{')
            });
        // Only a deferred block kept on one line reads as an array; a longer
        // one is the header's block.
        if header.is_some_and(is_defer_header)
            && self
                .current
                .active_token()
                .is_some_and(|brace| self.matching_brace_on_current_line(brace).is_some())
        {
            BraceType::DeferArray
        } else if self.is_objc_method_line()
            || self.layout.objc.method_continuation
            || (self.current_is_blank() && self.output_ends_objc_method_header())
        {
            BraceType::Definition
        } else if current_ends_block_literal_header(self.current.trimmed_end())
            || lambda_in_block_scope
        {
            BraceType::Command
        } else if lambda_header {
            BraceType::Definition
        } else if (header_condition_closed
            || header.is_none() && is_conditional_header_line(self.current.trimmed_start()))
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(
                    BraceType::Command
                        | BraceType::NonStatement
                        | BraceType::Definition
                        | BraceType::DeferArray
                )
            )
        {
            BraceType::Command
        } else if self.current_ends_compound_literal_type() {
            BraceType::CompoundLiteral
        } else if current_namespace_header || previous_namespace_header {
            BraceType::Namespace
        } else if self.layout.command_state.previous_command_char == Some('=')
            || trailing_word(self.current.trimmed_end()) == language::RETURN
            || self
                .layout
                .nesting
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| {
                    matches!(brace_type, BraceType::Array | BraceType::CompoundLiteral)
                })
            || (self.layout.command_state.previous_command_char == Some('{')
                && !self.token_input.token_begins_source_line
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    Some(BraceType::NonStatement)
                ))
        {
            BraceType::Array
        } else if pending_extern
            || (block_word.as_deref() == Some("extern") && !self.current_ends_definition_header())
        {
            BraceType::Extern
        } else if matches!(block_word.as_deref(), Some("namespace" | "module")) {
            BraceType::Namespace
        } else if block_word.as_deref() == Some("class") {
            BraceType::Class
        } else if block_word.as_deref() == Some("interface") {
            BraceType::Interface
        } else if block_word.as_deref() == Some("struct") {
            BraceType::Struct
        } else if block_word.as_deref() == Some("union") {
            BraceType::Union
        } else if block_word.as_deref() == Some("enum") {
            BraceType::Enum
        } else if self.current.trimmed_start().starts_with(':')
            && self
                .current
                .trimmed_end()
                .chars()
                .next_back()
                .is_some_and(is_word_char)
        {
            BraceType::Array
        } else if self.brace_opens_constructor_body()
            || self.current_ends_trailing_return_definition()
            || (header.is_some()
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    None | Some(
                        BraceType::Namespace
                            | BraceType::Class
                            | BraceType::Interface
                            | BraceType::Struct
                            | BraceType::Union
                            | BraceType::Enum
                            | BraceType::Extern
                    )
                ))
        {
            BraceType::Definition
        } else if header.is_some()
            || matches!(self.current.trimmed(), "-" | "+")
            || self.current_ends_definition_header()
                && matches!(
                    self.layout.nesting.brace_type_stack.last(),
                    Some(BraceType::Command | BraceType::Definition)
                )
        {
            BraceType::Command
        } else if self.current_ends_definition_header() {
            BraceType::Definition
        } else if matches!(
            self.layout.command_state.previous_command_char,
            Some(':' | ';' | '{' | '}' | '(')
        ) || language::is_non_paren_header(trailing_word(self.current.trimmed_end()))
        {
            BraceType::Command
        } else if self.current.trimmed_start().starts_with("->")
            && unmatched_open_paren_column(self.current.trimmed_end()).is_some()
        {
            BraceType::NonStatement
        } else if self.layout.nesting.brace_type_stack.is_empty()
            && !self.current.trimmed().is_empty()
            && self.current.trimmed().chars().all(is_word_char)
            && self
                .output
                .last_line_outside_comment()
                .is_none_or(|previous| {
                    let code = self.output.code_trimmed_of(previous);
                    code.is_empty()
                        || code.ends_with_any(b";}")
                        || code.trimmed_start().starts_with('#')
                })
            && (self.token_input.token_begins_source_line
                || self
                    .token_input
                    .previous_input_whitespace
                    .as_deref()
                    .is_some_and(|whitespace| !whitespace.is_empty()))
        {
            // A macro standing for a function head at file scope.
            BraceType::Definition
        } else if self.token_input.token_begins_source_line
            && matches!(
                self.layout.nesting.brace_type_stack.last(),
                Some(BraceType::Command | BraceType::Definition)
            )
            && !self.current.trimmed().is_empty()
            && self.current.trimmed().chars().all(is_word_char)
            && self
                .output
                .last_line_outside_comment()
                .is_none_or(|previous| {
                    let code = self.output.code_trimmed_of(previous);
                    code.is_empty()
                        || code.ends_with_any(b";{}")
                        || code.trimmed_start().starts_with('#')
                })
        {
            // A macro word heading a block in code, as `SEH_TRY`.
            BraceType::Command
        } else if !self.current_is_blank()
            // An argument's brace after the line its `,` ended.
            || self.layout.command_state.previous_command_char == Some(',')
                && self.layout.nesting.paren_depth > 0
        {
            BraceType::Initializer
        } else {
            BraceType::NonStatement
        }
    }

    fn brace_opens_constructor_body(&self) -> bool {
        if self.layout.command_state.previous_command_char != Some('}') {
            return false;
        }
        let scope_allows = match self.layout.nesting.brace_type_stack.last() {
            Some(brace_type) => {
                is_class_like_brace_type(*brace_type) || *brace_type == BraceType::Namespace
            }
            None => true,
        };
        if !scope_allows {
            return false;
        }
        let trimmed = self.current.trimmed_start();
        if (trimmed.starts_with(':') && !trimmed.starts_with("::")) || trimmed.starts_with(',') {
            return true;
        }
        line_has_constructor_init_colon(self.current.trimmed())
    }

    pub(super) fn current_is_lambda_body_header(&self) -> bool {
        // A lambda's capture list closes with `]`.
        if !self.current.as_bytes().contains(&b']') {
            return false;
        }
        let head = self.current.trimmed_end();
        let arrow = self.current.last_arrow();
        is_lambda_body_header_with_arrow(head, arrow)
            || self.current.last_open_bracket().is_some_and(|index| {
                let tail = head[index..].trimmed_start();
                let skipped = head.len() - index - tail.len();
                tail.as_bytes().contains(&b']')
                    && is_lambda_body_header_with_arrow(
                        tail,
                        arrow
                            .filter(|&arrow| arrow >= index + skipped)
                            .map(|arrow| arrow - index - skipped),
                    )
            })
    }

    pub(super) fn current_ends_trailing_return_definition(&self) -> bool {
        let mut lines = Vec::new();
        for line in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trimmed().is_empty())
            .take(16)
        {
            let code = self.output.code_trimmed_of(line);
            if code.ends_with_any(b";{}") {
                break;
            }
            lines.push(code);
        }
        lines.reverse();
        if !self.current_is_blank() {
            lines.push(self.current.trimmed_end());
        }
        let source = lines.join("\n");
        let chars: Vec<char> = source.chars().collect();
        let mut depth = 0i32;
        let mut saw_parameter_close = false;
        let mut arrow_end = None;
        let mut index = 0;
        while index < chars.len() {
            let ch = chars[index];
            if arrow_end.is_none()
                && ch == '-'
                && chars.get(index + 1) == Some(&'>')
                && depth == 0
                && saw_parameter_close
            {
                arrow_end = Some(index + 2);
                index += 2;
                continue;
            }
            match ch {
                '(' | '[' => depth += 1,
                ')' | ']' => {
                    depth -= 1;
                    if depth == 0 && ch == ')' && arrow_end.is_none() {
                        saw_parameter_close = true;
                    }
                }
                _ => {}
            }
            index += 1;
        }
        arrow_end.is_some_and(|arrow_end| {
            depth == 0 && chars[arrow_end..].iter().any(|ch| !ch.is_whitespace())
        })
    }

    pub(super) fn current_ends_definition_header(&self) -> bool {
        let source = if self.current_is_blank() {
            match self.output.scoped().iter().rev().find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !is_comment_only_line(trimmed)
            }) {
                Some(line) => self.output.code_of(line),
                None => return false,
            }
        } else {
            self.current.as_str()
        };
        code_ends_definition_header(source)
    }

    fn aggregate_header_ends_with_paren_group(&self) -> bool {
        let header = if self.current_is_blank() {
            self.output.last_non_empty_scoped()
        } else {
            None
        };
        let trimmed = match header {
            Some(line) => line.trimmed_end(),
            None => self.current.trimmed_end(),
        };
        trailing_matching_parens(trimmed).is_some_and(|(_, close)| close + 1 == trimmed.len())
    }

    pub(super) fn current_ends_compound_literal_type(&self) -> bool {
        if self.current_is_blank() {
            // A directive's condition casts nothing after it.
            return self.output.last().is_some_and(|last| {
                !last.trimmed_start().starts_with('#') && line_ends_compound_literal_cast(last)
            });
        }
        if matches!(self.current.trimmed_start().chars().next(), Some(':' | ',')) {
            return false;
        }
        if !line_ends_compound_literal_cast(&self.current) {
            return false;
        }
        let trimmed = self.current.trimmed();
        let leading_paren_group = trimmed.starts_with('(')
            && trailing_matching_parens(trimmed) == Some((0, trimmed.len() - 1));
        if leading_paren_group && self.previous_output_line_ends_with_declarator() {
            return false;
        }
        true
    }

    fn previous_output_line_ends_with_declarator(&self) -> bool {
        let Some(previous) = self.output.last().map(|line| line.trimmed_end()) else {
            return false;
        };
        previous.chars().next_back().is_some_and(is_word_char)
            && trailing_word(previous) != language::RETURN
    }

    pub(crate) fn track_extern_c_guard(&mut self, token: &Token) {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => return,
            Token::Preprocessor(line) => {
                if self.extern_c_guard == ExternCGuard::Idle && is_cplusplus_conditional(&line.text)
                {
                    self.extern_c_guard = ExternCGuard::CplusplusConditional;
                }
                return;
            }
            Token::Word(word)
                if word == "extern"
                    && self.extern_c_guard == ExternCGuard::CplusplusConditional =>
            {
                self.extern_c_guard = ExternCGuard::Extern;
                return;
            }
            Token::StringLiteral(literal)
                if literal == "\"C\"" && self.extern_c_guard == ExternCGuard::Extern =>
            {
                self.extern_c_guard = ExternCGuard::ExternC;
                return;
            }
            Token::Symbol('{') => return,
            _ => {}
        }
        if self.extern_c_guard == ExternCGuard::ExternC {
            self.extern_c_guard = ExternCGuard::Idle;
        }
    }
}

/// Progress through a `#ifdef __cplusplus` / `extern "C" {` guard.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ExternCGuard {
    Idle,
    CplusplusConditional,
    Extern,
    ExternC,
    InsideBlock,
}

pub(super) fn brace_indent_applies(brace_type: BraceType) -> bool {
    matches!(
        brace_type,
        BraceType::Command
            | BraceType::NonStatement
            | BraceType::Extern
            | BraceType::Class
            | BraceType::Interface
            | BraceType::Struct
            | BraceType::Union
            | BraceType::Enum
            | BraceType::Definition
            | BraceType::Array
            | BraceType::CompoundLiteral
    )
}

impl FormatEngine<'_> {
    /// Kind of the brace group that the `{` being pushed opens.
    pub(super) fn pushed_brace_kind(&self) -> Option<BlockKind> {
        let brace = self.current.active_token()?;
        let group = self.tree.groups.opened_at(brace)?;
        self.tree.blocks.kind(group)
    }

    pub(crate) fn in_initializer_brace(&self) -> bool {
        self.layout
            .nesting
            .brace_type_stack
            .iter()
            .any(|brace_type| matches!(brace_type, BraceType::Array | BraceType::CompoundLiteral))
    }

    pub(crate) fn innermost_init_block_brace(&self) -> bool {
        matches!(
            self.layout.nesting.brace_type_stack.last(),
            Some(BraceType::Initializer)
        ) && self.current_inline_array_column().is_none()
    }

    pub(crate) fn in_aggregate_declaration_brace(&self) -> bool {
        self.layout
            .nesting
            .brace_type_stack
            .last()
            .is_some_and(|brace_type| {
                matches!(
                    brace_type,
                    BraceType::Struct | BraceType::Union | BraceType::Enum
                )
            })
    }

    pub(crate) fn in_enum_declaration_brace(&self) -> bool {
        self.layout
            .nesting
            .brace_type_stack
            .last()
            .is_some_and(|brace_type| *brace_type == BraceType::Enum)
    }

    pub(crate) fn innermost_brace_is_compound_literal(&self) -> bool {
        matches!(
            self.layout.nesting.brace_type_stack.last(),
            Some(BraceType::CompoundLiteral)
        )
    }

    pub(crate) fn enclosed_in_compound_literal(&self) -> bool {
        self.layout
            .nesting
            .brace_type_stack
            .iter()
            .any(|brace_type| matches!(brace_type, BraceType::CompoundLiteral))
    }
}

pub(super) fn block_indent_extra(
    header: Option<&str>,
    brace_type: BraceType,
    options: &FormatOptions,
) -> usize {
    if brace_type == BraceType::Definition
        && options.brace_style == BraceStyle::Gnu
        && header.is_some_and(|header| {
            matches!(
                header,
                "try" | "catch" | "@try" | "@catch" | "@finally" | "__except" | "__finally"
            )
        })
    {
        return 1;
    }
    if brace_type != BraceType::Command {
        return 0;
    }
    if matches!(options.brace_style, BraceStyle::Vtk | BraceStyle::Ratliff)
        && header == Some("switch")
    {
        return usize::from(!options.indent_switches);
    }
    if options.indent_blocks && header.is_some_and(|header| !matches!(header, "case" | "default")) {
        1
    } else {
        0
    }
}

pub(crate) fn is_class_like_brace_type(brace_type: BraceType) -> bool {
    matches!(
        brace_type,
        BraceType::Class | BraceType::Interface | BraceType::Struct | BraceType::Union
    )
}

fn line_has_constructor_init_colon(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut depth = 0i32;
    let mut previous_significant: Option<char> = None;
    for (index, &ch) in chars.iter().enumerate() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ':' if depth == 0
                && chars.get(index + 1) != Some(&':')
                && previous_significant == Some(')') =>
            {
                return true;
            }
            _ => {}
        }
        if !ch.is_whitespace() {
            previous_significant = Some(ch);
        }
    }
    false
}

pub(crate) fn contains_one_line_block(line: &str) -> bool {
    let Some(open) = line.find('{') else {
        return false;
    };
    line[open + 1..].contains('}')
}

pub(crate) fn lambda_header_has_trailing_return(line: &str) -> bool {
    line.match_indices("->")
        .any(|(index, _)| line[..index].trimmed_end().ends_with(')'))
}

pub(super) fn line_ends_lambda_parameter_list(line: &str) -> bool {
    let current = line.trimmed_end();
    let Some((open_pos, _)) = trailing_matching_parens(current) else {
        return false;
    };
    current[..open_pos].trimmed_end().ends_with(']')
}

pub(crate) fn code_ends_definition_header(source: &str) -> bool {
    let mut rest = source.trimmed_end();
    loop {
        if rest.ends_with(')') {
            return true;
        }
        let stripped = rest.trim_end_matches('&').trimmed_end();
        if stripped.len() != rest.len() {
            rest = stripped;
            continue;
        }
        let word = trailing_word(rest);
        if word.is_empty()
            || !(language::PRE_COMMAND_QUALIFIERS.contains(&word) || is_macro_like_word(word))
        {
            return false;
        }
        let candidate = rest[..rest.len() - word.len()].trimmed_end();
        if candidate.is_empty() {
            return false;
        }
        rest = candidate;
    }
}
