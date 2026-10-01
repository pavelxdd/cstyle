use crate::config::{BraceStyle, FormatOptions};
use crate::formatter::constructs::headers::{line_is_control_body_header, starts_header_word};
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::{Token, raw_strings, token_text};
use crate::formatter::preprocessor::is_conditional_preprocessor;
use crate::formatter::state::frame::BraceSemanticKind;
use crate::formatter::state::indentation::LineKind;
use crate::formatter::text::columns::{leading_visual_width, leading_whitespace_len};
use crate::formatter::text::line_scan::{
    advance_quoted_literal, is_comment_line, preprocessor_directive, trailing_comment_split_limit,
    unmatched_open_paren_column,
};
use crate::source::lex::{is_digit_separator, is_identifier_continue, is_identifier_start};

pub(crate) fn find_case_colon(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    if !(is_case_label_start(trimmed) || is_default_label_start(trimmed)) {
        return None;
    }
    let start = line.len() - trimmed.len();
    find_case_colon_from(line, start)
}

pub(crate) fn split_switch_label_statement(line: &str) -> Option<(String, String)> {
    let colon = find_case_colon(line)?;
    let statement = line[colon + 1..].trim_start();
    if statement.is_empty()
        || statement.starts_with('{')
        || statement.starts_with("//")
        || statement.starts_with("/*")
    {
        return None;
    }
    Some((line[..=colon].to_string(), statement.to_string()))
}

pub(crate) fn case_label_with_trailing_comment(line: &str) -> bool {
    let comment = trailing_comment_split_limit(line);
    if comment == line.len() {
        return false;
    }
    let code = line[..comment].trim_end();
    (find_case_colon(code).is_some() || code == "default:") && code.ends_with(':')
}

pub(crate) fn multiline_switch_label_colon(
    tokens: &[Token],
    line_start: usize,
    line_end: usize,
) -> Option<(usize, bool)> {
    let line = tokens[line_start..line_end]
        .iter()
        .filter(|token| !matches!(token, Token::Newline))
        .map(token_text)
        .collect::<String>();
    let colon = find_case_colon(&line)?;
    if !line[..colon].contains('\n') {
        return None;
    }
    let has_action = split_switch_label_statement(&line).is_some();

    let mut offset = 0usize;
    for (relative, token) in tokens[line_start..line_end].iter().enumerate() {
        if matches!(token, Token::Newline) {
            continue;
        }
        if offset == colon && matches!(token, Token::Symbol(':')) {
            return Some((line_start + relative, has_action));
        }
        offset += token_text(token).len();
    }
    None
}

pub(crate) fn is_case_label_start(line: &str) -> bool {
    line.strip_prefix("case")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| !is_identifier_continue(ch))
}

pub(crate) fn is_default_label_start(line: &str) -> bool {
    line.strip_prefix("default").is_some_and(|rest| {
        rest.chars()
            .next()
            .is_none_or(|ch| !is_identifier_continue(ch))
    })
}

fn find_case_colon_from(line: &str, start: usize) -> Option<usize> {
    let chars = line.char_indices().collect::<Vec<_>>();
    let code_chars = line.chars().collect::<Vec<_>>();
    let mut index = chars.partition_point(|(byte_index, _)| *byte_index < start);
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut ternary_contexts = Vec::new();

    while let Some(&(byte_index, ch)) = chars.get(index) {
        let next = chars.get(index + 1).map(|(_, ch)| *ch);

        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
            index += 1;
            continue;
        }

        if ch == '/' && next == Some('/') {
            return None;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if let Some(end) = raw_strings::end(line, byte_index) {
            index = chars.partition_point(|(index, _)| *index < end);
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&code_chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '?' => ternary_contexts.push((paren_depth, bracket_depth, brace_depth)),
            ':' if next == Some(':') => {
                index += 2;
                continue;
            }
            ':' if ternary_contexts.last().copied()
                == Some((paren_depth, bracket_depth, brace_depth)) =>
            {
                ternary_contexts.pop();
            }
            ':' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                return Some(byte_index);
            }
            _ => {}
        }
        index += 1;
    }

    None
}

fn is_one_line_block_reached(line: &str, start: usize) -> bool {
    let braces = code_delimiters(line, start);
    let Some(open) = braces.iter().position(|(_, ch)| *ch == '{') else {
        return false;
    };
    braces[open + 1..].iter().any(|(_, ch)| *ch == '}')
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct CodeDelimiterState {
    in_block_comment: bool,
    quote: Option<char>,
    escaped: bool,
    raw_delimiter: Option<String>,
}

fn code_delimiters(line: &str, start: usize) -> Vec<(usize, char)> {
    code_delimiters_stateful(line, start, &mut CodeDelimiterState::default())
}

fn code_delimiters_stateful(
    line: &str,
    start: usize,
    state: &mut CodeDelimiterState,
) -> Vec<(usize, char)> {
    let chars = line.char_indices().collect::<Vec<_>>();
    let code_chars = line.chars().collect::<Vec<_>>();
    let mut index = chars.partition_point(|(byte_index, _)| *byte_index < start);
    let mut braces = Vec::new();

    while let Some(&(byte_index, ch)) = chars.get(index) {
        let next = chars.get(index + 1).map(|(_, ch)| *ch);

        if let Some(delimiter) = state.raw_delimiter.clone() {
            let Some(end) = raw_strings::closing_end(line, byte_index, &delimiter) else {
                break;
            };
            state.raw_delimiter = None;
            index = chars.partition_point(|(index, _)| *index < end);
            continue;
        }

        if state.in_block_comment {
            if ch == '*' && next == Some('/') {
                state.in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }

        if let Some(quote_char) = state.quote {
            if state.escaped {
                state.escaped = false;
            } else if ch == '\\' {
                state.escaped = true;
            } else if ch == quote_char {
                state.quote = None;
            }
            index += 1;
            continue;
        }

        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            state.in_block_comment = true;
            index += 2;
            continue;
        }
        if let Some(raw) = raw_strings::start(line, byte_index) {
            if let Some(end) = raw.end {
                index = chars.partition_point(|(index, _)| *index < end);
            } else {
                state.raw_delimiter = Some(raw.delimiter);
                break;
            }
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&code_chars, index)) {
            state.quote = Some(ch);
            state.escaped = false;
            index += 1;
            continue;
        }
        if matches!(ch, '(' | ')' | '{' | '}') {
            braces.push((byte_index, ch));
        }
        index += 1;
    }

    braces
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct SwitchCaseObserver {
    switch_depth: usize,
    switch_stack: Vec<usize>,
    brace_depth: usize,
    pending_switch: bool,
    pending_switch_paren_depth: usize,
    delimiter_state: CodeDelimiterState,
    looking_for_case_brace: bool,
    unindent_next_line: bool,
}

impl SwitchCaseObserver {
    pub(crate) fn observe_line(&mut self, line: &str, mut kind: LineKind) -> LineKind {
        let trimmed = line.trim_start();
        if kind == LineKind::Normal
            && !self.switch_stack.is_empty()
            && (is_case_label_start(trimmed) || is_default_label_start(trimmed))
        {
            kind = LineKind::SwitchLabel;
        }

        if kind == LineKind::SwitchLabel {
            self.looking_for_case_brace = true;
            self.unindent_next_line = true;
        } else if !trimmed.is_empty() {
            self.unindent_next_line = false;
        }

        let starts_in_opaque_text = self.delimiter_state.in_block_comment
            || self.delimiter_state.quote.is_some()
            || self.delimiter_state.raw_delimiter.is_some();
        if !starts_in_opaque_text
            && trimmed.strip_prefix("switch").is_some_and(|rest| {
                rest.chars()
                    .next()
                    .is_none_or(|ch| ch == '(' || ch.is_whitespace())
            })
        {
            self.pending_switch = true;
            self.pending_switch_paren_depth = 0;
        }
        for (_, delimiter) in code_delimiters_stateful(trimmed, 0, &mut self.delimiter_state) {
            match delimiter {
                '(' if self.pending_switch => self.pending_switch_paren_depth += 1,
                ')' if self.pending_switch => {
                    self.pending_switch_paren_depth =
                        self.pending_switch_paren_depth.saturating_sub(1);
                }
                '{' => {
                    self.brace_depth += 1;
                    if self.pending_switch && self.pending_switch_paren_depth == 0 {
                        self.switch_stack.push(self.brace_depth);
                        self.switch_depth += 1;
                        self.pending_switch = false;
                    }
                    self.looking_for_case_brace = false;
                }
                '}' => {
                    self.brace_depth = self.brace_depth.saturating_sub(1);
                    while self
                        .switch_stack
                        .last()
                        .is_some_and(|depth| *depth > self.brace_depth)
                    {
                        self.switch_stack.pop();
                        self.switch_depth = self.switch_depth.saturating_sub(1);
                    }
                }
                _ => {}
            }
        }

        kind
    }

    pub(crate) fn switch_depth(&self) -> usize {
        self.switch_depth
    }
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct CaseBlockState {
    switch_brace_count: usize,
    unindent_depth: usize,
    unindent_case: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct SwitchCaseLineTransformer {
    case_block_state: CaseBlockState,
    case_stack: Vec<CaseBlockState>,
    brace_depth: usize,
    switch_depth: usize,
    looking_for_case_brace: bool,
    unindent_next_line: bool,
    should_unindent_line: bool,
    should_unindent_comment: bool,
    in_continued_preprocessor: bool,
    in_block_comment: bool,
    in_quote: bool,
    quote_char: char,
    raw_string_delimiter: Option<String>,
    marked_label_colon: Option<usize>,
    line_number: usize,
    tab_width: usize,
    indent_width: usize,
    indent_cases: bool,
    indent_preproc_define: bool,
    empty_line_fill: bool,
    body_level_braces: bool,
}

impl SwitchCaseLineTransformer {
    pub(crate) fn new(options: &FormatOptions) -> Self {
        Self {
            case_block_state: CaseBlockState::default(),
            case_stack: Vec::new(),
            brace_depth: 0,
            switch_depth: 0,
            looking_for_case_brace: false,
            unindent_next_line: false,
            should_unindent_line: true,
            should_unindent_comment: false,
            in_continued_preprocessor: false,
            in_block_comment: false,
            in_quote: false,
            quote_char: '\'',
            raw_string_delimiter: None,
            marked_label_colon: None,
            line_number: 0,
            tab_width: options.tab_width,
            indent_width: options.indent_width,
            indent_cases: options.indent_cases,
            indent_preproc_define: options.indent_preproc_define,
            empty_line_fill: options.empty_line_fill,
            body_level_braces: options.brace_style == BraceStyle::Whitesmith,
        }
    }

    pub(crate) fn begin_line(&mut self) {
        self.line_number += 1;
    }

    pub(crate) fn mark_label_colon(&mut self, byte_index: usize) {
        self.marked_label_colon = Some(byte_index);
    }

    pub(crate) fn raw_literal_suffix_start(&self, line: &str) -> Option<usize> {
        self.raw_string_delimiter
            .as_deref()
            .and_then(|delimiter| raw_strings::closing_end(line, 0, delimiter))
    }

    pub(crate) fn scan_raw_literal_line(&mut self, line: &str) {
        let mut scan = line.to_string();
        let is_preprocessor = scan.trim_start().starts_with('#');
        self.parse_line(&mut scan, is_preprocessor);
    }

    pub(crate) fn transform_line(&mut self, mut line: String) -> String {
        self.should_unindent_line = true;
        self.should_unindent_comment = false;

        if line.is_empty() && !self.empty_line_fill {
            return line;
        }

        if self.unindent_next_line {
            self.case_block_state.unindent_depth += 1;
            self.case_block_state.unindent_case = true;
            self.unindent_next_line = false;
        }

        let is_preprocessor = self.in_continued_preprocessor || line.trim_start().starts_with('#');
        self.in_continued_preprocessor = is_preprocessor && line.trim_end().ends_with('\\');
        self.parse_line(&mut line, is_preprocessor);

        let unindent_depth = self.total_unindent_depth();
        if self.should_unindent_comment && unindent_depth > 0 {
            self.unindent_line(&mut line, unindent_depth - 1);
        } else if self.should_unindent_line && unindent_depth > 0 {
            self.unindent_line(&mut line, unindent_depth);
        }
        line
    }

    fn parse_line(&mut self, line: &mut String, is_preprocessor: bool) {
        let scan = line.clone();
        let chars: Vec<(usize, char)> = scan.char_indices().collect();
        let code_chars = scan.chars().collect::<Vec<_>>();
        let mut index = 0;

        while let Some(&(byte_index, ch)) = chars.get(index) {
            if let Some(delimiter) = self.raw_string_delimiter.clone() {
                let Some(end) = raw_strings::closing_end(&scan, byte_index, &delimiter) else {
                    break;
                };
                self.raw_string_delimiter = None;
                index = chars.partition_point(|(index, _)| *index < end);
                continue;
            }

            if self.marked_label_colon == Some(byte_index) {
                self.marked_label_colon = None;
                self.looking_for_case_brace = true;
                index += 1;
                continue;
            }

            if matches!(ch, ' ' | '\t') {
                index += 1;
                continue;
            }

            if self.in_block_comment {
                if self.case_block_state.switch_brace_count == 1
                    && self.case_block_state.unindent_case
                {
                    self.should_unindent_comment = true;
                }
                if starts_with_at(&scan, byte_index, "*/") {
                    self.in_block_comment = false;
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }

            if self.in_quote {
                if ch == '\\' {
                    index += 2;
                    continue;
                }
                if ch == self.quote_char {
                    self.in_quote = false;
                }
                index += 1;
                continue;
            }

            if starts_with_at(&scan, byte_index, "\\\\") {
                index += 2;
                continue;
            }
            if ch == '\\' {
                index += 2;
                continue;
            }

            if let Some(raw) = raw_strings::start(&scan, byte_index) {
                if let Some(end) = raw.end {
                    index = chars.partition_point(|(index, _)| *index < end);
                } else {
                    self.raw_string_delimiter = Some(raw.delimiter);
                    break;
                }
                continue;
            }

            if ch == '"' || (ch == '\'' && !is_digit_separator(&code_chars, index)) {
                self.in_quote = true;
                self.quote_char = ch;
                index += 1;
                continue;
            }

            if starts_with_at(&scan, byte_index, "//") {
                if has_windows_line_marker_after_line_comment(&scan, byte_index) {
                    self.line_number = self.line_number.saturating_sub(1);
                }
                if first_non_ws_byte(&scan) == Some(byte_index)
                    && self.case_block_state.switch_brace_count == 1
                    && self.case_block_state.unindent_case
                {
                    self.should_unindent_comment = true;
                }
                break;
            }
            if starts_with_at(&scan, byte_index, "/*") {
                if self.case_block_state.switch_brace_count == 1
                    && self.case_block_state.unindent_case
                {
                    self.should_unindent_comment = true;
                }
                self.in_block_comment = true;
                index += 2;
                continue;
            }

            if ch == '{' {
                self.brace_depth += 1;
            }
            if ch == '}' {
                self.brace_depth = self.brace_depth.saturating_sub(1);
            }

            let is_potential_keyword = is_identifier_start(ch);
            if is_potential_keyword && keyword_at(&scan, byte_index, "switch") {
                self.switch_depth += 1;
                self.case_stack.push(self.case_block_state.clone());
                self.case_block_state = CaseBlockState::default();
                index = skip_identifier(&chars, index);
                continue;
            }

            if self.indent_cases
                || self.switch_depth == 0
                || (is_preprocessor && !self.indent_preproc_define)
            {
                if is_potential_keyword {
                    index = skip_identifier(&chars, index);
                } else {
                    index += 1;
                }
                continue;
            }

            index = self.process_switch_block(&scan, &chars, index, line) + 1;
        }
        self.marked_label_colon = None;
    }

    fn process_switch_block(
        &mut self,
        scan: &str,
        chars: &[(usize, char)],
        index: usize,
        line: &mut String,
    ) -> usize {
        let (byte_index, ch) = chars[index];
        let is_potential_keyword = is_identifier_start(ch);

        if ch == '{' {
            self.case_block_state.switch_brace_count += 1;
            if self.looking_for_case_brace {
                if !self.body_level_braces {
                    self.case_block_state.unindent_case = true;
                    self.case_block_state.unindent_depth += 1;
                }
                self.looking_for_case_brace = false;
            }
            return index;
        }
        self.looking_for_case_brace = false;

        if ch == '}' {
            self.case_block_state.switch_brace_count =
                self.case_block_state.switch_brace_count.saturating_sub(1);
            if self.case_block_state.switch_brace_count == 0 && self.switch_depth > 0 {
                let mut line_unindent = self.total_unindent_depth();
                if first_non_ws_byte(scan) == Some(byte_index) && !self.case_stack.is_empty() {
                    line_unindent = self.stack_unindent_depth();
                }
                if self.should_unindent_line {
                    if line_unindent > 0 {
                        self.unindent_line(line, line_unindent);
                    }
                    self.should_unindent_line = false;
                }
                self.switch_depth = self.switch_depth.saturating_sub(1);
                self.case_block_state = self.case_stack.pop().unwrap_or_default();
            }
            return index;
        }

        if is_potential_keyword
            && (keyword_at(scan, byte_index, "case") || keyword_at(scan, byte_index, "default"))
        {
            if self.case_block_state.unindent_case {
                self.case_block_state.unindent_case = false;
                self.case_block_state.unindent_depth =
                    self.case_block_state.unindent_depth.saturating_sub(1);
            }

            let Some(colon) = find_case_colon_from(scan, byte_index) else {
                return index;
            };
            let mut next = char_index_after_byte(chars, colon);
            while chars
                .get(next)
                .is_some_and(|(_, ch)| matches!(ch, ' ' | '\t'))
            {
                next += 1;
            }
            if chars.get(next).is_some_and(|(_, ch)| *ch == '{') {
                self.brace_depth += 1;
                self.case_block_state.switch_brace_count += 1;
                if !self.body_level_braces && !is_one_line_block_reached(scan, chars[next].0) {
                    self.unindent_next_line = true;
                }
                return next;
            }
            self.looking_for_case_brace = true;
            return next.saturating_sub(1);
        }

        if is_potential_keyword {
            return skip_identifier(chars, index).saturating_sub(1);
        }
        index
    }

    pub(crate) fn total_unindent_depth(&self) -> usize {
        self.case_block_state.unindent_depth + self.stack_unindent_depth()
    }

    pub(crate) fn next_line_unindent_depth(&self) -> usize {
        self.total_unindent_depth() + usize::from(self.unindent_next_line)
    }

    pub(crate) fn unindent_depth_for_line(&self, line: &str) -> usize {
        if !self.indent_cases
            && first_non_ws_byte(line).is_some_and(|index| line[index..].starts_with('}'))
            && self.case_block_state.switch_brace_count == 1
            && self.switch_depth > 0
            && !self.case_stack.is_empty()
        {
            return self.stack_unindent_depth();
        }
        self.next_line_unindent_depth()
            + usize::from(
                !self.indent_cases
                    && !self.body_level_braces
                    && self.looking_for_case_brace
                    && line.trim_start().starts_with('{'),
            )
    }

    pub(crate) fn pending_unindent_depth(&self) -> usize {
        let total = self.total_unindent_depth();
        if self.case_block_state.unindent_case {
            total.saturating_sub(1)
        } else {
            total
        }
    }

    fn stack_unindent_depth(&self) -> usize {
        self.case_stack
            .iter()
            .map(|state| state.unindent_depth)
            .sum()
    }

    fn unindent_line(&self, line: &mut String, levels: usize) -> usize {
        if line.is_empty() && !self.empty_line_fill {
            return 0;
        }

        let whitespace = leading_whitespace_len(line);
        if whitespace == 0 {
            return 0;
        }

        let erase = levels * self.indent_width;
        if erase > whitespace {
            return 0;
        }
        line.replace_range(0..erase, "");
        erase
    }
}

fn starts_with_at(line: &str, byte_index: usize, needle: &str) -> bool {
    line.get(byte_index..)
        .is_some_and(|rest| rest.starts_with(needle))
}

fn first_non_ws_byte(line: &str) -> Option<usize> {
    line.char_indices()
        .find(|(_, ch)| !matches!(ch, ' ' | '\t'))
        .map(|(index, _)| index)
}

fn has_windows_line_marker_after_line_comment(line: &str, byte_index: usize) -> bool {
    line.get(byte_index + 2..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(|ch| ch > '\u{f0}')
}

fn keyword_at(line: &str, byte_index: usize, keyword: &str) -> bool {
    let Some(rest) = line.get(byte_index..) else {
        return false;
    };
    if !rest.starts_with(keyword) {
        return false;
    }
    let before = line[..byte_index].chars().next_back();
    let after = rest[keyword.len()..].chars().next();
    before.is_none_or(|ch| !is_identifier_continue(ch))
        && after.is_none_or(|ch| !is_identifier_continue(ch))
}

fn skip_identifier(chars: &[(usize, char)], index: usize) -> usize {
    let mut next = index;
    while chars
        .get(next)
        .is_some_and(|(_, ch)| is_identifier_continue(*ch))
    {
        next += 1;
    }
    next
}

fn char_index_after_byte(chars: &[(usize, char)], byte_index: usize) -> usize {
    chars.partition_point(|(index, _)| *index <= byte_index)
}

fn starts_inline_case_statement(line: &str) -> bool {
    let line = line.trim_start();
    if !(starts_header_word(line, "case") || starts_header_word(line, "default")) {
        return false;
    }
    let bytes = line.as_bytes();
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut question_depth = 0usize;
    while let Some(&byte) = bytes.get(index) {
        if let Some(quote_byte) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote_byte {
                quote = None;
            }
            index += 1;
            continue;
        }
        match byte {
            b'"' | b'\'' => quote = Some(byte),
            b'?' => question_depth += 1,
            b':' if bytes.get(index.wrapping_sub(1)) == Some(&b':')
                || bytes.get(index + 1) == Some(&b':') => {}
            b':' if question_depth > 0 => question_depth -= 1,
            b':' => return !line[index + 1..].trim().is_empty(),
            _ => {}
        }
        index += 1;
    }
    false
}

fn is_braced_switch_label_line(line: &str) -> bool {
    let code = line[..trailing_comment_split_limit(line)].trim_end();
    let trimmed = code.trim_start();
    code.ends_with('{') && (find_case_colon(trimmed).is_some() || trimmed == "default: {")
}

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(crate) struct SwitchCaseLayoutState {
    body_brace_depths: Vec<usize>,
    pending_label_brace: bool,
    preprocessor_brace_depths: Vec<usize>,
    unindent_brace_depths: Vec<usize>,
    closing_line_needs_unindent: bool,
    /// Switch body depth where a case block closed since the last label:
    /// astyle lays out the rest of a Whitesmith case at its label.
    case_block_closed_depth: Option<usize>,
}

pub(crate) struct CaseBlockBodyLayout {
    pub(crate) exact_indent_spaces: usize,
}

struct ActiveCaseLayout {
    indent_spaces: usize,
    opens_block: bool,
}

impl FormatEngine<'_> {
    pub(crate) fn split_else_header_operator_case_compensation_indent_spaces(
        &self,
        line: &str,
        current_spaces: Option<usize>,
        header_operator_spaces: Option<usize>,
    ) -> Option<usize> {
        let spaces = current_spaces?;
        if spaces == 0
            || self.layout.line_adjuster.next_line_case_unindent_depth()
                <= self.layout.line_adjuster.total_case_unindent_depth()
            || self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| is_braced_switch_label_line(previous))
            || !self.line_aligns_to_open_paren_content(line)
        {
            return None;
        }
        let trimmed = line.trim_start();
        let header_operator_indent = self.split_else_body_indent_active()
            && (trimmed.starts_with("&&") || trimmed.starts_with("||"))
            && header_operator_spaces == Some(spaces);
        (!header_operator_indent).then_some(spaces + self.options.indent_width)
    }

    pub(crate) fn split_else_switch_comment_indent_spaces(
        &self,
        line: &str,
        split_else_output_context: bool,
    ) -> Option<usize> {
        if !split_else_output_context
            || !(is_comment_line(line.trim_start()) || line.trim_start().starts_with("/*"))
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.trim_start().starts_with("switch") || !previous_code.ends_with('{') {
            return None;
        }
        let case_unindent = self
            .layout
            .line_adjuster
            .next_line_case_unindent_depth()
            .max(self.layout.line_adjuster.total_case_unindent_depth())
            * self.options.indent_width;
        Some(leading_visual_width(previous, self.options.tab_width) + case_unindent)
    }

    pub(crate) fn split_else_adjusted_case_indent_floor(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_context: bool,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !split_else_context
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with('#')
            || self.layout.line_adjuster.total_case_unindent_depth() == 0
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let previous_trimmed = previous_code.trim_start();
        let adjusted_delta = self.adjusted_line_indent_delta(previous);
        let target = if previous_code.ends_with('{')
            && !line.trim_start().starts_with('}')
            && !previous_trimmed.starts_with("case ")
            && !previous_trimmed.starts_with("default:")
            && !previous_trimmed.starts_with("switch")
            && adjusted_delta > 0
        {
            leading_visual_width(previous, self.options.tab_width)
                + self.options.indent_width
                + adjusted_delta
        } else if previous_code.ends_with(',')
            && !line.trim_start().starts_with(['#', '}', ')'])
            && adjusted_delta > 0
        {
            leading_visual_width(previous, self.options.tab_width) + adjusted_delta
        } else {
            return None;
        };
        (current_spaces.unwrap_or(0) <= target).then_some(target)
    }

    pub(crate) fn split_else_switch_label_indent_spaces(
        &self,
        line_kind: LineKind,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context || line_kind != LineKind::SwitchLabel {
            return None;
        }
        let switch_line = self.output.scoped().iter().rev().find(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            code.trim_start().starts_with("switch")
        })?;
        let body_indent = usize::from(
            self.options.indent_switches || self.options.brace_style == BraceStyle::Ratliff,
        ) * self.options.indent_width;
        Some(leading_visual_width(switch_line, self.options.tab_width) + body_indent)
    }

    pub(crate) fn split_else_case_body_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        split_else_context: bool,
    ) -> Option<usize> {
        if !split_else_context
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
        {
            return None;
        }
        for previous in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.starts_with("case ") || trimmed.starts_with("default:") {
                let follows_comment = self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .skip_while(|line| line.as_str() != previous.as_str())
                    .skip(1)
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|line| is_comment_line(line.trim_start()));
                if code.ends_with('{') && !follows_comment {
                    return None;
                }
                let case_unindent = usize::from(code.ends_with('{') && follows_comment)
                    * self.layout.line_adjuster.next_line_case_unindent_depth()
                    * self.options.indent_width;
                return Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width
                        + case_unindent
                        + usize::from(!code.ends_with('{'))
                            * self.layout.line_adjuster.total_case_unindent_depth()
                            * self.options.indent_width,
                );
            }
            if trimmed.starts_with("switch") || code.ends_with('{') || trimmed.starts_with('}') {
                break;
            }
        }
        None
    }

    pub(crate) fn split_else_case_closed_block_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if self.options.indent_cases {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if previous.trim() != "}" {
            return None;
        }
        let target = leading_visual_width(previous, self.options.tab_width)
            + self
                .layout
                .line_adjuster
                .total_case_unindent_depth()
                .max(self.layout.line_adjuster.next_line_case_unindent_depth())
                * self.options.indent_width;
        if line_kind == LineKind::Normal
            && !line.trim_start().starts_with(['#', '{', '}'])
            && line.trim() != "break;"
            && self
                .layout
                .nesting
                .brace_header_stack
                .iter()
                .any(|header| header.as_deref() == Some("case"))
            && self.layout.nesting.last_closed_brace_header.as_deref() != Some("switch")
        {
            return (current_spaces.unwrap_or(0) < target).then_some(target);
        }
        if line.trim() != "break;" {
            return None;
        }
        let nearest_case = self.output.scoped().iter().rev().find(|line| {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            trimmed.starts_with("case ") || trimmed.starts_with("default:")
        });
        let closes_switch = self.layout.nesting.last_closed_brace_header.as_deref()
            == Some("switch")
            || self
                .output
                .current_closing_brace_open(self.options.tab_width)
                .is_some_and(|(_, _, trimmed)| starts_header_word(trimmed, "switch"));
        if closes_switch
            || self.layout.nesting.last_closed_brace_header.as_deref() != Some("switch")
                && nearest_case.is_some_and(|line| {
                    line[..trailing_comment_split_limit(line)]
                        .trim_end()
                        .ends_with('{')
                })
        {
            return (current_spaces.unwrap_or(0) < target).then_some(target);
        }
        if self.layout.nesting.last_closed_brace_header.as_deref() != Some("switch")
            && nearest_case.is_some_and(|line| {
                !line[..trailing_comment_split_limit(line)]
                    .trim_end()
                    .ends_with('{')
            })
        {
            let previous_indent = leading_visual_width(previous, self.options.tab_width);
            return current_spaces
                .is_none_or(|spaces| spaces > previous_indent)
                .then_some(previous_indent);
        }
        None
    }

    pub(crate) fn split_else_case_completed_call_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        normal_indent: usize,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if !self.preprocessor.split_else.extra_indent
            || line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}'])
        {
            return None;
        }
        let current = current_spaces?;
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        if case_unindent == 0 {
            return None;
        }
        let body_spaces = normal_indent * self.options.indent_width;
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let call_indent = self
            .output
            .scoped()
            .iter()
            .rev()
            .skip(1)
            .take(8)
            .take_while(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                let trimmed = code.trim_start();
                !trimmed.starts_with("case ")
                    && !trimmed.starts_with("default:")
                    && trimmed != "}"
                    && !trimmed.ends_with('{')
            })
            .find_map(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                (unmatched_open_paren_column(code).is_some() && !code.ends_with(';'))
                    .then(|| leading_visual_width(line, self.options.tab_width))
            })?;
        let previous_indent = leading_visual_width(previous, self.options.tab_width);
        if !previous_code.ends_with(';')
            || previous_code.trim() == "};"
            || previous_code.trim_start().starts_with(");")
            || unmatched_open_paren_column(previous_code).is_some()
            || !(previous_indent.saturating_sub(body_spaces)
                <= self.options.max_continuation_indent
                || previous_indent.saturating_sub(call_indent)
                    <= self.options.max_continuation_indent)
        {
            return None;
        }
        let target = call_indent.max(body_spaces) + case_unindent;
        (target != current).then_some(target)
    }

    pub(crate) fn case_parenthesized_block_indent_spaces(
        &self,
        line: &str,
        current_spaces: usize,
    ) -> Option<usize> {
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        (case_unindent > 0
            && line.trim_start().starts_with(')')
            && line[..trailing_comment_split_limit(line)]
                .trim_end()
                .ends_with('{'))
        .then_some(current_spaces + case_unindent)
    }

    pub(crate) fn case_control_indent_floor(
        &self,
        line: &str,
        normal_indent: usize,
        current_spaces: usize,
    ) -> Option<usize> {
        if self.layout.line_adjuster.total_case_unindent_depth() == 0 {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        let trimmed = line.trim_start();
        let owns_case_floor = previous_code.ends_with(") {")
            || trimmed == "}"
                && self
                    .output
                    .current_closing_brace_open(self.options.tab_width)
                    .is_some_and(|(_, _, open)| starts_header_word(open, "switch"))
            || trimmed.starts_with("break;") && previous_code.trim() == "}"
            || trimmed.starts_with("} else")
            || trimmed.starts_with("}else")
            || previous_code.trim_start().starts_with("} else") && previous_code.ends_with('{');
        owns_case_floor.then(|| current_spaces.max(normal_indent * self.options.indent_width))
    }

    pub(crate) fn case_post_comment_sibling_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || line.trim_start().starts_with(['#', '{', '}', '/'])
            || self.layout.line_adjuster.total_case_unindent_depth() == 0
            || self
                .layout
                .nesting
                .brace_header_stack
                .last()
                .is_none_or(|header| header.as_deref() != Some("case"))
        {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        if !is_comment_line(previous.trim_start()) {
            return None;
        }
        let previous_indent = self
            .output
            .scoped()
            .iter()
            .rev()
            .take_while(|line| is_comment_line(line.trim_start()))
            .find(|line| line.trim_start().starts_with("/*"))
            .map_or_else(
                || leading_visual_width(previous, self.options.tab_width),
                |line| leading_visual_width(line, self.options.tab_width),
            );
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        current_spaces
            .is_some_and(|spaces| spaces <= previous_indent)
            .then_some(previous_indent + case_unindent)
    }

    pub(crate) fn logical_case_unindent_adjusted_spaces(
        &self,
        line: &str,
        current_spaces: usize,
        normal_indent: usize,
    ) -> Option<usize> {
        let case_unindent =
            self.layout.line_adjuster.total_case_unindent_depth() * self.options.indent_width;
        let trimmed = line.trim_start();
        (case_unindent > 0
            && (trimmed.starts_with("&&") || trimmed.starts_with("||"))
            && current_spaces <= normal_indent * self.options.indent_width + case_unindent)
            .then_some(current_spaces + case_unindent)
    }

    pub(crate) fn has_pending_case_label_brace(&self) -> bool {
        self.layout.switch_case_layout.pending_label_brace
    }

    pub(crate) fn case_closing_line_needs_unindent(&self) -> bool {
        self.layout.switch_case_layout.closing_line_needs_unindent
    }

    fn has_case_body_at_current_depth(&self) -> bool {
        let current = self.layout.nesting.brace_header_stack.len();
        self.layout
            .switch_case_layout
            .body_brace_depths
            .contains(&current)
    }

    pub(crate) fn has_case_body_indent(&self) -> bool {
        !self.layout.switch_case_layout.body_brace_depths.is_empty()
    }

    pub(crate) fn register_attached_case_label_brace(&mut self) {
        if !self.layout.switch_case_layout.pending_label_brace {
            return;
        }
        self.layout
            .switch_case_layout
            .unindent_brace_depths
            .push(self.layout.nesting.brace_header_stack.len());
        self.layout.switch_case_layout.pending_label_brace = false;
    }

    pub(crate) fn prepare_case_closing_brace(&mut self) {
        self.layout.switch_case_layout.closing_line_needs_unindent = self
            .layout
            .switch_case_layout
            .unindent_brace_depths
            .last()
            .is_some_and(|depth| *depth == self.layout.nesting.brace_header_stack.len());
    }

    pub(crate) fn clear_case_body_indent_if_past_switch(&mut self) {
        let current = self.layout.nesting.brace_header_stack.len();
        while self
            .layout
            .switch_case_layout
            .body_brace_depths
            .last()
            .is_some_and(|depth| current < *depth)
        {
            self.layout.switch_case_layout.body_brace_depths.pop();
        }
        while self
            .layout
            .switch_case_layout
            .preprocessor_brace_depths
            .last()
            .is_some_and(|depth| current + 1 < *depth)
        {
            self.layout
                .switch_case_layout
                .preprocessor_brace_depths
                .pop();
        }
    }

    pub(crate) fn direct_switch_body_indent_spaces(&self) -> Option<usize> {
        if !self.options.indent_switches && self.options.brace_style != BraceStyle::Vtk {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.header.as_deref() == Some("switch"))?;
        Some(frame.body_indent_column + self.options.indent_width)
    }

    pub(crate) fn case_body_indent_extra(&self, line_kind: LineKind) -> usize {
        if !self.options.indent_switches {
            return 0;
        }
        let current = self.layout.nesting.brace_header_stack.len();
        match line_kind {
            // A Whitesmith case block's body stands at its brace, the case
            // body column; other blocks nest in the case body.
            LineKind::Normal if self.options.brace_style == BraceStyle::Whitesmith => {
                let layout = &self.layout.switch_case_layout;
                layout
                    .body_brace_depths
                    .iter()
                    .filter(|&&depth| {
                        if depth < current {
                            return !layout.unindent_brace_depths.contains(&(depth + 1));
                        }
                        depth == current
                            && (layout.closing_line_needs_unindent
                                || !layout.unindent_brace_depths.contains(&(depth + 1))
                                    && layout.case_block_closed_depth != Some(depth))
                    })
                    .count()
            }
            LineKind::Normal => self
                .layout
                .switch_case_layout
                .body_brace_depths
                .iter()
                .filter(|depth| **depth <= current)
                .count(),
            LineKind::SwitchLabel => self
                .layout
                .switch_case_layout
                .body_brace_depths
                .iter()
                .filter(|depth| **depth < current)
                .count(),
            LineKind::Label => 0,
        }
    }

    pub(crate) fn case_preprocessor_body_indent_extra(
        &self,
        line_kind: LineKind,
        line: &str,
    ) -> usize {
        if line_kind != LineKind::Normal {
            return 0;
        }
        if line.trim() == "}"
            && !self.layout.switch_case_layout.closing_line_needs_unindent
            && self
                .output
                .last_line_outside_comment()
                .is_some_and(|previous| {
                    let trimmed = previous.trim_start();
                    trimmed.starts_with("break;")
                })
        {
            return 0;
        }
        let current = self.layout.nesting.brace_header_stack.len();
        self.layout
            .switch_case_layout
            .preprocessor_brace_depths
            .iter()
            .filter(|depth| current + 1 >= **depth)
            .count()
    }

    pub(crate) fn isolated_opening_brace_is_switch_label(&self) -> bool {
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes;
            if meta.opens > depth {
                let mut trimmed = self.output.code_trimmed(index);
                // Ratliff closes a case block at its body, past the label.
                while self.options.brace_style != BraceStyle::Ratliff
                    && let Some(rest) = trimmed
                        .strip_prefix("/*")
                        .and_then(|rest| rest.split_once("*/"))
                {
                    trimmed = rest.1.trim_start();
                }
                return trimmed.ends_with('{')
                    && (trimmed.starts_with("case ") || trimmed.starts_with("default:"));
            }
            depth -= meta.opens;
        }
        false
    }

    fn nearest_open_switch_indent_spaces(&self) -> Option<usize> {
        let tab_width = self.options.tab_width;
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes;
            if meta.opens > depth {
                let trimmed = self.output.code_trimmed(index);
                if trimmed.starts_with("switch ") || trimmed.starts_with("switch(") {
                    return Some(self.output.lead_width(index, tab_width));
                }
            }
            depth = depth.saturating_sub(meta.opens);
        }
        None
    }

    fn active_emitted_case_layout(&self) -> Option<ActiveCaseLayout> {
        let tab_width = self.options.tab_width;
        let mut closing_indents = Vec::new();
        for index in (0..self.output.len()).rev() {
            let trimmed = self.output.code_trimmed(index);
            if trimmed.is_empty() {
                continue;
            }
            let code = self.output.code(index);
            if trimmed.starts_with("case ") || trimmed.starts_with("default:") {
                let indent_spaces = self.output.lead_width(index, tab_width);
                if !closing_indents
                    .iter()
                    .any(|closing| *closing <= indent_spaces)
                {
                    return Some(ActiveCaseLayout {
                        indent_spaces,
                        opens_block: code.ends_with('{'),
                    });
                }
                return None;
            }
            if trimmed.starts_with("switch") {
                return None;
            }
            if code.ends_with('{') {
                let open_indent = self.output.lead_width(index, tab_width);
                if let Some(index) = closing_indents
                    .iter()
                    .position(|closing| *closing <= open_indent)
                {
                    closing_indents.remove(index);
                } else {
                    return None;
                }
            }
            if trimmed == "}" {
                closing_indents.push(self.output.lead_width(index, tab_width));
            }
        }
        None
    }

    pub(crate) fn initial_switch_case_indent_spaces(
        &self,
        line: &str,
        line_kind: LineKind,
        mut exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line_kind == LineKind::SwitchLabel
            && let Some(spaces) = self.nearest_open_switch_indent_spaces()
        {
            let switch_body_indent = usize::from(
                self.options.indent_switches || self.options.brace_style == BraceStyle::Ratliff,
            ) * self.options.indent_width;
            exact_indent_spaces = Some(
                spaces
                    + switch_body_indent
                    + self.layout.line_adjuster.pending_case_unindent() * self.options.indent_width,
            );
        }

        let trimmed = line.trim_start();
        if self.layout.line_adjuster.switch_depth() == 0
            || trimmed.starts_with('#')
            || trimmed.starts_with("case ")
            || trimmed.starts_with("default")
        {
            return exact_indent_spaces;
        }

        let indent_width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let Some(case_layout) = self.active_emitted_case_layout() else {
            return exact_indent_spaces;
        };
        let case_indent = case_layout.indent_spaces;
        let case_body_extra = usize::from(self.preprocessor.split_else.extra_indent) * indent_width;
        let target = if trimmed.starts_with('}') && !case_layout.opens_block {
            None
        } else if trimmed.starts_with('}') {
            self.output.last_non_empty_index().and_then(|index| {
                let previous_indent = self.output.lead_width(index, tab_width);
                if previous_indent >= case_indent + case_body_extra + indent_width {
                    Some(
                        if previous_indent > case_indent + case_body_extra + indent_width {
                            previous_indent - indent_width
                        } else {
                            case_indent + case_body_extra
                        },
                    )
                } else if previous_indent >= case_indent + indent_width {
                    Some(case_indent)
                } else {
                    None
                }
            })
        } else {
            Some(case_indent + case_body_extra + indent_width)
        };
        if let Some(target) = target
            && (trimmed.starts_with('}') || exact_indent_spaces.unwrap_or(0) < target)
        {
            exact_indent_spaces = Some(target);
        }
        exact_indent_spaces
    }

    pub(crate) fn emitted_case_body_indent_spaces(
        &self,
        line: &str,
        current_spaces: Option<usize>,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if self.layout.line_adjuster.switch_depth() == 0
            || trimmed.starts_with('#')
            || trimmed.starts_with("case ")
            || trimmed.starts_with("default")
            || trimmed.starts_with("} while")
        {
            return None;
        }
        let case_layout = self.active_emitted_case_layout()?;
        let case_indent = case_layout.indent_spaces;
        let tab_width = self.options.tab_width;
        let target = if trimmed.starts_with('}') && !case_layout.opens_block {
            None
        } else if trimmed.starts_with('}') {
            let mut closed = 0usize;
            (0..self.output.len()).rev().find_map(|index| {
                let previous_trimmed = self.output.code_trimmed(index);
                if previous_trimmed.is_empty() {
                    return None;
                }
                let code = self.output.code(index);
                if previous_trimmed.starts_with("case ") || previous_trimmed.starts_with("default:")
                {
                    return Some(case_indent);
                }
                if previous_trimmed.starts_with('}') {
                    closed += 1;
                    return None;
                }
                if code.ends_with('{') {
                    if closed > 0 {
                        closed -= 1;
                        return None;
                    }
                    let indent = self.output.lead_width(index, tab_width);
                    if indent > case_indent {
                        if !(line_is_control_body_header(previous_trimmed)
                            || starts_header_word(previous_trimmed, "for")
                            || starts_header_word(previous_trimmed, "while")
                            || starts_header_word(previous_trimmed, "if")
                            || starts_header_word(previous_trimmed, "do"))
                            && let Some(header) = (0..index).rev().take(8).find(|header| {
                                let trimmed = self.output.code_trimmed(*header);
                                line_is_control_body_header(trimmed)
                                    || starts_header_word(trimmed, "for")
                                    || starts_header_word(trimmed, "while")
                                    || starts_header_word(trimmed, "if")
                            })
                        {
                            return Some(self.output.lead_width(header, tab_width));
                        }
                        return Some(indent);
                    }
                }
                None
            })
        } else {
            Some(case_indent + self.options.indent_width)
        }?;
        let target = target
            + self.layout.line_adjuster.next_line_case_unindent_depth() * self.options.indent_width;
        (trimmed.starts_with('}') || current_spaces.unwrap_or(0) < target).then_some(target)
    }

    pub(crate) fn immediate_case_brace_indent_spaces(
        &self,
        line: &str,
        closing_line_needs_unindent: bool,
    ) -> Option<usize> {
        if !closing_line_needs_unindent || self.options.indent_cases {
            return None;
        }
        // A switch body's closer lines up however a comment trails it, as
        // does a case block closer whose `{` stands alone at its label; one
        // closing a block opened on its label, or VTK's and Horstmann's
        // indented case block, keeps astyle's extra level.
        let closes_switch = self
            .output
            .pending_tokens()
            .and_then(|span| self.tree.groups.closed_at(span.first))
            .and_then(|group| self.tree.blocks.owner(group))
            .is_some_and(
                |owner| matches!(&self.tree.tokens[owner], Token::Word(word) if word == "switch"),
            );
        let opened_alone = !matches!(
            self.options.brace_style,
            BraceStyle::Vtk | BraceStyle::Horstmann
        ) && self
            .output
            .current_closing_brace_open(self.options.tab_width)
            .is_some_and(|(_, _, opener)| opener == "{");
        let code = line[..trailing_comment_split_limit(line)].trim();
        if line.trim() == "}" || ((closes_switch || opened_alone) && code == "}") {
            return Some(self.layout.indentation.indent() * self.options.indent_width);
        }

        let after_brace = line.trim_start().strip_prefix("} ")?.trim_start();
        let indent =
            if starts_header_word(after_brace, "case") || after_brace.starts_with("default:") {
                self.layout.indentation.indent().saturating_sub(1)
            } else {
                self.layout.indentation.indent() + 1
            };
        Some(indent * self.options.indent_width)
    }

    pub(crate) fn case_label_block_indent_override(
        &self,
        line: &str,
        structural_indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.starts_with("case ") || trimmed.starts_with("default:") {
            return None;
        }
        let (open_spaces, _, open_trimmed) = self
            .output
            .current_closing_brace_open(self.options.tab_width)?;
        if !(open_trimmed.starts_with("case ") || open_trimmed.starts_with("default:")) {
            return None;
        }

        let case_unindent_depth = self.layout.line_adjuster.next_line_case_unindent_depth();
        if matches!(trimmed, "};" | "},") {
            return Some(open_spaces + case_unindent_depth * self.options.indent_width);
        }
        if case_unindent_depth == 0 {
            return None;
        }

        let target = if line.trim() == "}" {
            open_spaces
        } else {
            open_spaces + self.options.indent_width
        };
        let current = exact_indent_spaces.unwrap_or(structural_indent * self.options.indent_width);
        (current <= target).then_some(target + case_unindent_depth * self.options.indent_width)
    }

    pub(crate) fn active_case_control_closing_indent_override(
        &self,
        line: &str,
        structural_indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let case_unindent_depth = self.layout.line_adjuster.next_line_case_unindent_depth();
        if case_unindent_depth == 0 {
            return None;
        }

        let mut target = None;
        if !self
            .output
            .last_line_outside_comment()
            .is_some_and(|previous| {
                let trimmed = previous.trim();
                trimmed.starts_with('#') || matches!(trimmed, "}" | "};" | "break;")
            })
            && let Some(open_spaces) = self.recent_same_line_else_open_indent_spaces()
        {
            target = Some(open_spaces + case_unindent_depth * self.options.indent_width);
        }
        if let Some((open_spaces, _, open_trimmed)) = self
            .output
            .current_closing_brace_open(self.options.tab_width)
            && (starts_header_word(open_trimmed, "if")
                || starts_header_word(open_trimmed, "for")
                || starts_header_word(open_trimmed, "while")
                || starts_header_word(open_trimmed, "do")
                || open_trimmed.starts_with("else"))
        {
            let control_target = open_spaces + case_unindent_depth * self.options.indent_width;
            target = Some(target.map_or(control_target, |current| current.max(control_target)));
        }

        target.map(|target| {
            exact_indent_spaces
                .unwrap_or(structural_indent * self.options.indent_width)
                .max(target)
        })
    }

    fn recent_same_line_else_open_indent_spaces(&self) -> Option<usize> {
        let tab_width = self.options.tab_width;
        for line in self.output.scoped().iter().rev().take(32) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.starts_with("case ") || trimmed.starts_with("default:") {
                break;
            }
            if (trimmed.starts_with("} else") || trimmed.starts_with("}else"))
                && code.ends_with('{')
            {
                return Some(leading_visual_width(line, tab_width));
            }
        }
        None
    }

    pub(crate) fn compound_case_label_indent_override(&self, line: &str) -> Option<usize> {
        if self.options.indent_cases {
            return None;
        }
        let after_brace = line.trim_start().strip_prefix("} ")?.trim_start();
        if !(starts_header_word(after_brace, "case") || after_brace.starts_with("default:")) {
            return None;
        }
        let case_unindent_depth = self.layout.line_adjuster.total_case_unindent_depth();
        (case_unindent_depth > 0).then_some(
            self.layout
                .indentation
                .indent()
                .saturating_sub(case_unindent_depth)
                * self.options.indent_width,
        )
    }

    pub(crate) fn split_switch_closing_indent_override(&self, line: &str) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let frame = self.layout.frame_stack.last_closed_brace()?;
        (frame.semantic_kind == BraceSemanticKind::Command
            && frame.header.as_deref() == Some("switch")
            && frame.split_header)
            .then_some(frame.sibling_indent_column)
    }

    pub(crate) fn post_block_case_body_indent_override(
        &self,
        line: &str,
        line_kind: LineKind,
        closes_outer_delimiter: bool,
        has_owned_continuation: bool,
        is_closing_header: bool,
    ) -> Option<usize> {
        if line_kind != LineKind::Normal
            || closes_outer_delimiter
            || has_owned_continuation
            || is_closing_header
            || line
                .trim_start()
                .starts_with(['#', '{', '}', '/', ')', ']'])
            || self
                .output
                .last_line_outside_comment()
                .is_none_or(|previous| previous.trim() != "}")
            || !self.has_case_body_at_current_depth()
            || self
                .layout
                .frame_stack
                .last_closed_brace()
                .is_none_or(|frame| frame.case_block)
        {
            return None;
        }

        self.active_case_label_indent_spaces()
            .map(|spaces| spaces + self.options.indent_width)
    }

    pub(crate) fn nested_case_label_indent_override(
        &mut self,
        line_kind: LineKind,
    ) -> Option<usize> {
        if line_kind != LineKind::SwitchLabel {
            return None;
        }
        let indent_width = self.options.indent_width;
        let frame = self
            .layout
            .frame_stack
            .active_brace_mut()
            .filter(|frame| frame.case_block)?;
        if frame.case_header_pending {
            frame.case_header_pending = false;
            return None;
        }
        frame.nested_case_label = true;
        Some(frame.header_indent_column + indent_width)
    }

    pub(crate) fn active_case_block_body_layout(
        &self,
        line: &str,
        line_kind: LineKind,
        uses_normal_indent: bool,
        closes_outer_delimiter: bool,
        has_owned_continuation: bool,
        exact_indent_spaces: Option<usize>,
    ) -> Option<CaseBlockBodyLayout> {
        let previous_line = self.output.iter().rposition(|line| !line.trim().is_empty());
        let follows_ternary_arm = previous_line.is_some_and(|previous_line| {
            self.layout
                .frame_stack
                .line_ended_open_ternary(previous_line)
        });
        if line_kind != LineKind::Normal
            || !uses_normal_indent
            || closes_outer_delimiter
            || has_owned_continuation
            || follows_ternary_arm
            || line.trim_start().starts_with([')', ']', '}'])
            || self.pending_line_continues_statement()
        {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.case_block)?;
        let target = if line.trim_start().starts_with('{') {
            frame.sibling_indent_column
        } else if frame.nested_case_label {
            frame.header_indent_column + 2 * self.options.indent_width
        } else {
            frame.body_indent_column
        };
        let target = target
            + self.layout.line_adjuster.case_unindent_depth_for_line(line)
                * self.options.indent_width;
        let exact_indent_spaces = if self.preprocessor.split_else.extra_levels > 0 {
            exact_indent_spaces.map_or(target, |current| current.max(target))
        } else {
            target
        };
        Some(CaseBlockBodyLayout {
            exact_indent_spaces,
        })
    }

    pub(crate) fn switch_case_frame_closing_indent_override(
        &self,
        line: &str,
        exact_indent_spaces: Option<usize>,
    ) -> Option<usize> {
        if line.trim() != "}" {
            return None;
        }
        let frame = self
            .layout
            .frame_stack
            .last_closed_brace()
            .filter(|frame| frame.case_block || frame.header.as_deref() == Some("switch"))?;
        let target = if frame.case_block && frame.nested_case_label {
            frame.header_indent_column
                + if matches!(
                    self.options.brace_style,
                    BraceStyle::Whitesmith | BraceStyle::Vtk | BraceStyle::Ratliff
                ) {
                    2 * self.options.indent_width
                } else {
                    self.options.indent_width
                }
        } else if self.options.brace_style == BraceStyle::Ratliff
            && frame.header.as_deref() == Some("switch")
        {
            frame.body_indent_column
        } else {
            frame.sibling_indent_column
        };
        let target = target
            + self.layout.line_adjuster.case_unindent_depth_for_line(line)
                * self.options.indent_width;
        Some(
            if frame.case_block && self.preprocessor.split_else.extra_levels > 0 {
                exact_indent_spaces.map_or(target, |current| current.max(target))
            } else {
                target
            },
        )
    }

    pub(crate) fn active_case_label_indent_spaces(&self) -> Option<usize> {
        let tab_width = self.options.tab_width;
        let mut depth = 0usize;
        for index in (0..self.output.len()).rev() {
            let meta = self.output.brace_meta(index);
            depth += meta.closes;
            if depth == 0 {
                let code = self.output.code_trimmed(index);
                if code.starts_with("case ") || code.starts_with("default:") {
                    return Some(self.output.lead_width(index, tab_width));
                }
            }
            if meta.opens > depth {
                return None;
            }
            depth -= meta.opens;
        }
        None
    }

    pub(crate) fn update_case_body_indent(&mut self, line_kind: LineKind) {
        if line_kind == LineKind::SwitchLabel {
            let current = self.layout.nesting.brace_header_stack.len();
            if self.layout.switch_case_layout.body_brace_depths.last() != Some(&current) {
                self.layout
                    .switch_case_layout
                    .body_brace_depths
                    .push(current);
            }
        }
    }

    pub(crate) fn update_case_brace_unindent(&mut self, line_kind: LineKind, line: &str) {
        if self.options.indent_cases {
            self.layout.switch_case_layout.pending_label_brace = false;
            self.layout.switch_case_layout.closing_line_needs_unindent = false;
            return;
        }

        let layout = &mut self.layout.switch_case_layout;
        if layout.closing_line_needs_unindent {
            if let Some(depth) = layout.unindent_brace_depths.pop() {
                layout.case_block_closed_depth = Some(depth - 1);
            }
            layout.closing_line_needs_unindent = false;
        }

        let current = self.layout.nesting.brace_header_stack.len();
        while let Some(&depth) = layout.unindent_brace_depths.last()
            && current < depth
        {
            layout.unindent_brace_depths.pop();
            layout.case_block_closed_depth = Some(depth - 1);
        }

        if line_kind == LineKind::SwitchLabel {
            layout.case_block_closed_depth = None;
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            if code.ends_with('{') {
                self.layout
                    .switch_case_layout
                    .unindent_brace_depths
                    .push(self.layout.nesting.brace_header_stack.len());
                self.layout.switch_case_layout.pending_label_brace = false;
            } else {
                self.layout.switch_case_layout.pending_label_brace = true;
            }
            return;
        }

        if line_kind == LineKind::Normal && !line.trim().is_empty() {
            let trimmed = line.trim_start();
            if self.layout.switch_case_layout.pending_label_brace && trimmed.starts_with('{') {
                let current = self.layout.nesting.brace_header_stack.len();
                self.layout
                    .switch_case_layout
                    .unindent_brace_depths
                    .push(current + 1);
                if self
                    .output
                    .scoped()
                    .iter()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .is_some_and(|previous| {
                        let previous = previous.trim_start();
                        previous.starts_with('#')
                            && !preprocessor_directive(previous)
                                .is_some_and(is_conditional_preprocessor)
                    })
                {
                    self.layout
                        .switch_case_layout
                        .preprocessor_brace_depths
                        .push(current);
                }
                self.layout.switch_case_layout.pending_label_brace = false;
            } else if !is_comment_line(trimmed) && !trimmed.starts_with('#') {
                self.layout.switch_case_layout.pending_label_brace = false;
            }
            if trimmed.starts_with("break;") {
                self.layout
                    .switch_case_layout
                    .preprocessor_brace_depths
                    .pop();
            }
        }
    }

    pub(crate) fn case_comment_following_indent_spaces(&self, line: &str) -> Option<usize> {
        if line.trim_start().starts_with(['#', '{', '}', '/'])
            || find_case_colon(line).is_some()
            || self.pending_line_is_label(line)
            || self
                .layout
                .nesting
                .brace_header_stack
                .last()
                .is_none_or(|header| header.as_deref() != Some("case"))
        {
            return None;
        }
        if let Some(frame) = self
            .layout
            .frame_stack
            .active_brace()
            .filter(|frame| frame.case_block)
        {
            return Some(if frame.nested_case_label {
                frame.header_indent_column + 2 * self.options.indent_width
            } else {
                frame.body_indent_column
            });
        }
        let mut comment_indent = self
            .layout
            .previous_pre_adjust_line
            .as_deref()
            .filter(|line| is_comment_line(line.trim_start()))
            .map(|line| leading_visual_width(line, self.options.tab_width));
        for previous in self
            .output
            .scoped()
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
        {
            if is_comment_line(previous.trim_start()) {
                comment_indent = Some(
                    leading_visual_width(previous, self.options.tab_width)
                        + self.options.indent_width,
                );
                continue;
            }
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let trimmed = code.trim_start();
            if code.ends_with('{')
                && (trimmed.starts_with("case ") || trimmed.starts_with("default:"))
            {
                return comment_indent;
            }
            break;
        }
        None
    }
}

pub(crate) fn replayed_inline_case_body_indent_spaces(
    options: &FormatOptions,
    previous: &str,
    delimiter_replayed: bool,
) -> Option<usize> {
    if options.max_code_length.is_none()
        || !delimiter_replayed
        || !starts_inline_case_statement(previous)
    {
        return None;
    }
    let trimmed = previous.trim_start();
    unmatched_open_paren_column(trimmed).map(|open| {
        leading_visual_width(previous, options.tab_width) + open + 1 + options.indent_width
    })
}

pub(crate) fn max_length_inline_case_body_indent_extra(
    options: &FormatOptions,
    line: &str,
) -> Option<usize> {
    starts_inline_case_statement(line).then_some(options.indent_width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observe(state: &mut SwitchCaseObserver, line: &str) -> LineKind {
        let kind = if find_case_colon(line).is_some() {
            LineKind::SwitchLabel
        } else {
            LineKind::Normal
        };
        state.observe_line(line, kind)
    }

    #[test]
    fn finds_case_colons_outside_comments_and_literals() {
        assert_eq!(find_case_colon("case 1:"), Some(6));
        assert_eq!(find_case_colon("case ':' :"), Some(9));
        assert_eq!(find_case_colon("case \"x:y\" :"), Some(11));
        assert_eq!(find_case_colon("case value /* : */:"), Some(18));
        assert_eq!(find_case_colon("case 1'000:"), Some(10));
        assert_eq!(find_case_colon("case value // :"), None);
        assert_eq!(find_case_colon("default_value:"), None);
    }

    #[test]
    fn splits_switch_label_statements_before_max_length_wrapping() {
        assert_eq!(
            split_switch_label_statement("case A: write_log(foo);").unwrap(),
            ("case A:".to_string(), "write_log(foo);".to_string())
        );
    }

    #[test]
    fn observer_tracks_nested_switches() {
        let mut state = SwitchCaseObserver::default();

        assert_eq!(observe(&mut state, "switch (x)"), LineKind::Normal);
        assert_eq!(observe(&mut state, "{"), LineKind::Normal);
        assert_eq!(state.switch_depth(), 1);
        assert_eq!(observe(&mut state, "case 1:"), LineKind::SwitchLabel);
        assert!(state.looking_for_case_brace);
        assert!(state.unindent_next_line);
        assert_eq!(observe(&mut state, "{"), LineKind::Normal);
        assert!(!state.looking_for_case_brace);
        assert_eq!(observe(&mut state, "switch (y)"), LineKind::Normal);
        assert_eq!(observe(&mut state, "{"), LineKind::Normal);
        assert_eq!(state.switch_depth(), 2);
        assert_eq!(observe(&mut state, "case 2:"), LineKind::SwitchLabel);
        assert_eq!(observe(&mut state, "}"), LineKind::Normal);
        assert_eq!(observe(&mut state, "}"), LineKind::Normal);
        assert_eq!(state.switch_depth(), 1);
        assert_eq!(observe(&mut state, "}"), LineKind::Normal);
        assert_eq!(state.switch_depth(), 0);
    }

    #[test]
    fn observer_ignores_expression_braces_and_counts_inline_blocks() {
        let mut state = SwitchCaseObserver::default();

        assert_eq!(
            observe(&mut state, "switch ([] { return 1; }()) {"),
            LineKind::Normal
        );
        assert_eq!(state.switch_depth(), 1);
        assert_eq!(
            observe(&mut state, "if (ready) { call(); }"),
            LineKind::Normal
        );
        assert_eq!(
            observe(&mut state, "case hash(R\"(a)\"):"),
            LineKind::SwitchLabel
        );
        assert_eq!(observe(&mut state, "}"), LineKind::Normal);
        assert_eq!(state.switch_depth(), 0);
    }

    #[test]
    fn finds_one_line_blocks_outside_comments_and_literals() {
        assert!(is_one_line_block_reached("{ printf(\"}\"); }", 0));
        assert!(is_one_line_block_reached("{ /* } */ return; }", 0));
        assert!(is_one_line_block_reached("{ int x = 1'000; }", 0));
        assert!(!is_one_line_block_reached("{ printf(\"}\");", 0));
    }

    #[test]
    fn windows_line_markers_do_not_advance_case_line_number() {
        let options = FormatOptions::default();
        let mut transformer = SwitchCaseLineTransformer::new(&options);

        transformer.begin_line();
        assert_eq!(
            transformer.transform_line("//\u{f1}".to_string()),
            "//\u{f1}"
        );
        assert_eq!(transformer.line_number, 0);
        transformer.begin_line();
        assert_eq!(
            transformer.transform_line("int value;".to_string()),
            "int value;"
        );
        assert_eq!(transformer.line_number, 1);
    }
}
