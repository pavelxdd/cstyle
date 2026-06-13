use super::FormatEngine;
use super::compound_literals::line_ends_compound_literal_cast;
use super::headers::is_conditional_header_line;
use super::language;
use super::language::is_macro_like_word;
use super::line_scan::{is_comment_only_line, trailing_matching_parens};
use super::line_scan::{trailing_comment_split_limit, unmatched_open_paren_column};
use super::preprocessor::is_cplusplus_conditional;
use super::rewrite::is_defer_header;
use super::state::FormatterBraceType;
use super::token::Token;
use crate::config::{BraceStyle, FormatOptions, IndentStyle};
use crate::source::lex::{is_word_char, trailing_word};

pub(super) fn line_opens_lambda_block(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let head = head.trim_end();
    is_lambda_body_header(head)
        || head
            .rfind('[')
            .is_some_and(|index| is_lambda_body_header(head[index..].trim_start()))
}

pub(super) fn is_lambda_body_header(head: &str) -> bool {
    let mut head = head.trim_end();
    if let Some(arrow) = head.rfind("->") {
        let before = head[..arrow].trim_end();
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
    let capture = head[..open].trim_end();
    if !capture.ends_with(']') {
        return false;
    }
    let Some(lb) = matching_open_index(capture, '[', ']') else {
        return false;
    };
    match capture[..lb].trim_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

pub(super) fn is_lambda_capture_header(head: &str) -> bool {
    let head = head.trim_end();
    if !head.ends_with(']') {
        return false;
    }
    let Some(lb) = matching_open_index(head, '[', ']') else {
        return false;
    };
    match head[..lb].trim_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

pub(super) fn line_opens_parameterized_lambda_block(line: &str) -> bool {
    if !line_opens_lambda_block(line) {
        return false;
    }
    let trimmed = line.trim();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let Some(capture_end) = head.rfind(']') else {
        return false;
    };
    head[capture_end + 1..].trim_start().starts_with('(')
}

pub(super) fn line_opens_lambda_or_capture_only_block(line: &str) -> bool {
    if line_opens_lambda_block(line) {
        return true;
    }
    let trimmed = line.trim();
    let Some(head) = trimmed.strip_suffix('{') else {
        return false;
    };
    let head = head.trim_end();
    let Some(capture_start) = head.rfind('[') else {
        return false;
    };
    let capture = head[capture_start..].trim();
    if !capture.ends_with(']') {
        return false;
    }
    match head[..capture_start].trim_end().chars().next_back() {
        None => true,
        Some(ch) => !(is_word_char(ch) || ch == ')' || ch == ']'),
    }
}

fn is_namespace_block_header(line: &str) -> bool {
    let trimmed = line.trim();
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
    let before = head[..open].trim_end().trim_end_matches(|ch: char| {
        is_word_char(ch) || matches!(ch, '*' | '&' | ' ' | '\t' | ':')
    });
    let Some(rest) = before.strip_suffix('^') else {
        return false;
    };
    match rest.trim_end().chars().next_back() {
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
    is_namespace_block_header(line) || line.trim_start().starts_with("module ")
}

impl FormatEngine<'_> {
    pub(super) fn exact_brace_indent_level(
        &self,
        line: &str,
        structural_level: usize,
        spaces: usize,
    ) -> usize {
        if self.options.indent_style != IndentStyle::Tabs
            || !spaces.is_multiple_of(self.options.indent_width.max(1))
            || !line[..trailing_comment_split_limit(line)]
                .trim_start()
                .starts_with(['{', '}'])
        {
            return structural_level;
        }
        structural_level.max(spaces / self.options.indent_width.max(1))
    }

    pub(super) fn classify_opening_brace(
        &mut self,
        header: Option<&str>,
        pending_extern: bool,
    ) -> FormatterBraceType {
        let block_word = self.command_state.pending_block_word.take();
        let block_word = match block_word.as_deref() {
            Some("struct" | "union" | "enum" | "class" | "interface")
                if self.aggregate_header_ends_with_paren_group()
                    || self.command_state.previous_command_char == Some(')') =>
            {
                None
            }
            _ => block_word,
        };
        let lambda_header = self.current_is_lambda_body_header()
            || is_lambda_capture_header(self.current.trim_end());
        let lambda_in_block_scope = lambda_header
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(FormatterBraceType::Command | FormatterBraceType::Definition)
            );
        let active_delimiter = self
            .frame_stack
            .active_delimiter_with_id()
            .map(|(id, _)| id);
        let header_condition_closed = header.is_some_and(|header| {
            (language::is_non_paren_header(header)
                || matches!(header, "autoreleasepool" | "@try" | "@finally")
                || self.command_state.previous_command_char == Some(')'))
                && self.frame_stack.active_header().is_some_and(|frame| {
                    frame.header == header && frame.parent_delimiter == active_delimiter
                })
        });
        let current_namespace_header = {
            let split = trailing_comment_split_limit(&self.current);
            is_namespace_block_header(&self.current[..split])
        };
        let previous_namespace_header = self.current_is_blank()
            && self
                .output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
                .is_some_and(|line| {
                    let code = &line[..trailing_comment_split_limit(line)];
                    is_namespace_block_header(code) && !code.trim_end().ends_with('{')
                });
        if header.is_some_and(is_defer_header) {
            FormatterBraceType::DeferArray
        } else if self.is_objc_method_line()
            || self.objc.method_continuation
            || (self.current_is_blank() && self.output_ends_objc_method_header())
        {
            FormatterBraceType::Definition
        } else if current_ends_block_literal_header(self.current.trim_end())
            || lambda_in_block_scope
        {
            FormatterBraceType::Command
        } else if lambda_header {
            FormatterBraceType::Definition
        } else if (header_condition_closed
            || header.is_none() && is_conditional_header_line(self.current.trim_start()))
            && matches!(
                self.stack_state.brace_type_stack.last(),
                Some(
                    FormatterBraceType::Command
                        | FormatterBraceType::NonStatement
                        | FormatterBraceType::Definition
                        | FormatterBraceType::DeferArray
                )
            )
        {
            FormatterBraceType::Command
        } else if self.current_ends_compound_literal_type() {
            FormatterBraceType::CompoundLiteral
        } else if current_namespace_header || previous_namespace_header {
            FormatterBraceType::Namespace
        } else if self.command_state.previous_command_char == Some('=')
            || trailing_word(self.current.trim_end()) == language::RETURN
            || self
                .stack_state
                .brace_type_stack
                .last()
                .is_some_and(|brace_type| *brace_type == FormatterBraceType::Array)
            || (self.command_state.previous_command_char == Some('{')
                && !self.token_input.token_begins_source_line
                && matches!(
                    self.stack_state.brace_type_stack.last(),
                    Some(FormatterBraceType::NonStatement)
                ))
        {
            FormatterBraceType::Array
        } else if pending_extern
            || (block_word.as_deref() == Some("extern") && !self.current_ends_definition_header())
        {
            FormatterBraceType::Extern
        } else if matches!(block_word.as_deref(), Some("namespace" | "module")) {
            FormatterBraceType::Namespace
        } else if block_word.as_deref() == Some("class") {
            FormatterBraceType::Class
        } else if block_word.as_deref() == Some("interface") {
            FormatterBraceType::Interface
        } else if block_word.as_deref() == Some("struct") {
            FormatterBraceType::Struct
        } else if block_word.as_deref() == Some("union") {
            FormatterBraceType::Union
        } else if block_word.as_deref() == Some("enum") {
            FormatterBraceType::Enum
        } else if self.current.trim_start().starts_with(':')
            && self
                .current
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(is_word_char)
        {
            FormatterBraceType::Array
        } else if self.brace_opens_constructor_body()
            || self.current_ends_trailing_return_definition()
            || (header.is_some()
                && matches!(
                    self.stack_state.brace_type_stack.last(),
                    None | Some(
                        FormatterBraceType::Namespace
                            | FormatterBraceType::Class
                            | FormatterBraceType::Interface
                            | FormatterBraceType::Struct
                            | FormatterBraceType::Union
                            | FormatterBraceType::Enum
                            | FormatterBraceType::Extern
                    )
                ))
        {
            FormatterBraceType::Definition
        } else if header.is_some()
            || matches!(self.current.trim(), "-" | "+")
            || self.current_ends_definition_header()
                && matches!(
                    self.stack_state.brace_type_stack.last(),
                    Some(FormatterBraceType::Command | FormatterBraceType::Definition)
                )
        {
            FormatterBraceType::Command
        } else if self.current_ends_definition_header() {
            FormatterBraceType::Definition
        } else if matches!(
            self.command_state.previous_command_char,
            Some(':' | ';' | '{' | '}' | '(')
        ) || language::is_non_paren_header(trailing_word(self.current.trim_end()))
        {
            FormatterBraceType::Command
        } else if self.current.trim_start().starts_with("->")
            && unmatched_open_paren_column(self.current.trim_end()).is_some()
        {
            FormatterBraceType::NonStatement
        } else if !self.current_is_blank() {
            FormatterBraceType::Init
        } else {
            FormatterBraceType::NonStatement
        }
    }

    pub(super) fn brace_opens_constructor_body(&self) -> bool {
        if self.command_state.previous_command_char != Some('}') {
            return false;
        }
        let scope_allows = match self.stack_state.brace_type_stack.last() {
            Some(brace_type) => {
                is_class_like_brace_type(*brace_type)
                    || *brace_type == FormatterBraceType::Namespace
            }
            None => true,
        };
        if !scope_allows {
            return false;
        }
        let trimmed = self.current.trim_start();
        if (trimmed.starts_with(':') && !trimmed.starts_with("::")) || trimmed.starts_with(',') {
            return true;
        }
        line_has_constructor_init_colon(self.current.trim())
    }

    pub(super) fn current_is_lambda_body_header(&self) -> bool {
        let head = self.current.trim_end();
        is_lambda_body_header(head)
            || head
                .rfind('[')
                .is_some_and(|index| is_lambda_body_header(head[index..].trim_start()))
    }

    pub(super) fn current_ends_trailing_return_definition(&self) -> bool {
        let mut lines = Vec::new();
        for line in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(16)
        {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            if code.ends_with([';', '{', '}']) {
                break;
            }
            lines.push(code);
        }
        lines.reverse();
        if !self.current_is_blank() {
            lines.push(self.current.trim_end());
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
            match self.output.iter().rev().find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !is_comment_only_line(trimmed)
            }) {
                Some(line) => &line[..trailing_comment_split_limit(line)],
                None => return false,
            }
        } else {
            self.current.as_str()
        };
        self.code_ends_definition_header(source)
    }

    pub(super) fn code_ends_definition_header(&self, source: &str) -> bool {
        let mut rest = source.trim_end();
        loop {
            if rest.ends_with(')') {
                return true;
            }
            let stripped = rest.trim_end_matches('&').trim_end();
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
            let candidate = rest[..rest.len() - word.len()].trim_end();
            if candidate.is_empty() {
                return false;
            }
            rest = candidate;
        }
    }

    pub(super) fn aggregate_header_ends_with_paren_group(&self) -> bool {
        let header = if self.current_is_blank() {
            self.output
                .iter()
                .rev()
                .find(|line| !line.trim().is_empty())
        } else {
            None
        };
        let trimmed = match header {
            Some(line) => line.trim_end(),
            None => self.current.trim_end(),
        };
        trailing_matching_parens(trimmed).is_some_and(|(_, close)| close + 1 == trimmed.len())
    }

    pub(super) fn current_ends_compound_literal_type(&self) -> bool {
        if self.current_is_blank() {
            return self
                .output
                .last()
                .is_some_and(|last| line_ends_compound_literal_cast(last));
        }
        if matches!(self.current.trim_start().chars().next(), Some(':' | ',')) {
            return false;
        }
        if !line_ends_compound_literal_cast(&self.current) {
            return false;
        }
        let trimmed = self.current.trim();
        let leading_paren_group = trimmed.starts_with('(')
            && trailing_matching_parens(trimmed) == Some((0, trimmed.len() - 1));
        if leading_paren_group && self.previous_output_line_ends_with_declarator() {
            return false;
        }
        true
    }

    pub(super) fn previous_output_line_ends_with_declarator(&self) -> bool {
        let Some(previous) = self.output.last().map(|line| line.trim_end()) else {
            return false;
        };
        previous.chars().next_back().is_some_and(is_word_char)
            && trailing_word(previous) != language::RETURN
    }

    pub(super) fn track_cpp_extern_c_brace(&mut self, token: &Token) {
        match token {
            Token::Whitespace(_) | Token::Newline | Token::Comment(_, _) => return,
            Token::Preprocessor(line) => {
                if self.cpp_extern_c_brace == 0 && is_cplusplus_conditional(&line.text) {
                    self.cpp_extern_c_brace = 1;
                }
                return;
            }
            Token::Word(word) if word == "extern" && self.cpp_extern_c_brace == 1 => {
                self.cpp_extern_c_brace = 2;
                return;
            }
            Token::StringLiteral(literal) if literal == "\"C\"" && self.cpp_extern_c_brace == 2 => {
                self.cpp_extern_c_brace = 3;
                return;
            }
            Token::Symbol('{') => return,
            _ => {}
        }
        if self.cpp_extern_c_brace == 3 {
            self.cpp_extern_c_brace = 0;
        }
    }
}

pub(super) fn brace_indent_applies(brace_type: FormatterBraceType) -> bool {
    matches!(
        brace_type,
        FormatterBraceType::Command
            | FormatterBraceType::NonStatement
            | FormatterBraceType::Extern
            | FormatterBraceType::Class
            | FormatterBraceType::Interface
            | FormatterBraceType::Struct
            | FormatterBraceType::Union
            | FormatterBraceType::Enum
            | FormatterBraceType::Definition
            | FormatterBraceType::Array
            | FormatterBraceType::CompoundLiteral
    )
}

impl FormatEngine<'_> {
    pub(super) fn in_initializer_brace(&self) -> bool {
        self.stack_state.brace_type_stack.iter().any(|brace_type| {
            matches!(
                brace_type,
                FormatterBraceType::Array | FormatterBraceType::CompoundLiteral
            )
        })
    }

    pub(super) fn innermost_init_block_brace(&self) -> bool {
        matches!(
            self.stack_state.brace_type_stack.last(),
            Some(FormatterBraceType::Init)
        ) && self.current_inline_array_column().is_none()
    }

    pub(super) fn in_aggregate_declaration_brace(&self) -> bool {
        self.stack_state
            .brace_type_stack
            .last()
            .is_some_and(|brace_type| {
                matches!(
                    brace_type,
                    FormatterBraceType::Struct
                        | FormatterBraceType::Union
                        | FormatterBraceType::Enum
                )
            })
    }

    pub(super) fn in_enum_declaration_brace(&self) -> bool {
        self.stack_state
            .brace_type_stack
            .last()
            .is_some_and(|brace_type| *brace_type == FormatterBraceType::Enum)
    }

    pub(super) fn innermost_brace_is_compound_literal(&self) -> bool {
        matches!(
            self.stack_state.brace_type_stack.last(),
            Some(FormatterBraceType::CompoundLiteral)
        )
    }

    pub(super) fn enclosed_in_compound_literal(&self) -> bool {
        self.stack_state
            .brace_type_stack
            .iter()
            .any(|brace_type| matches!(brace_type, FormatterBraceType::CompoundLiteral))
    }
}

pub(super) fn block_indent_extra(
    header: Option<&str>,
    brace_type: FormatterBraceType,
    options: &FormatOptions,
) -> usize {
    if brace_type == FormatterBraceType::Definition
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
    if brace_type != FormatterBraceType::Command {
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

pub(super) fn is_class_like_brace_type(brace_type: FormatterBraceType) -> bool {
    matches!(
        brace_type,
        FormatterBraceType::Class
            | FormatterBraceType::Interface
            | FormatterBraceType::Struct
            | FormatterBraceType::Union
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

pub(super) fn contains_one_line_block(line: &str) -> bool {
    let Some(open) = line.find('{') else {
        return false;
    };
    line[open + 1..].contains('}')
}

pub(super) fn lambda_header_has_trailing_return(line: &str) -> bool {
    line.match_indices("->")
        .any(|(index, _)| line[..index].trim_end().ends_with(')'))
}

pub(super) fn line_ends_lambda_parameter_list(line: &str) -> bool {
    let current = line.trim_end();
    let Some((open_pos, _)) = trailing_matching_parens(current) else {
        return false;
    };
    current[..open_pos].trim_end().ends_with(']')
}
