use super::columns::{leading_visual_width, visual_width_from};
use super::{
    FormatEngine, operator_chains, trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::source::lex::{is_digit_separator, is_identifier_continue, is_identifier_start};

#[derive(Clone, Copy, PartialEq)]
enum DefineFrame {
    Brace,
    CaseBrace,
    CommandBrace,
    InitializerBrace,
    SwitchBrace,
    Header,
    SwitchHeader,
}

struct DefineBodyLineInfo {
    opens: usize,
    closes: usize,
    leading_close: bool,
    is_case_label: bool,
    ends_semicolon: bool,
    is_header: bool,
    is_command_header: bool,
    is_switch_header: bool,
    has_embedded_default_label: bool,
}

fn define_body_first_word(line: &str) -> &str {
    let trimmed = line.trim_start();
    let end = trimmed
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(trimmed.len());
    &trimmed[..end]
}

fn is_define_header_keyword(line: &str) -> bool {
    matches!(
        define_body_first_word(line),
        "if" | "else" | "for" | "while" | "do" | "switch"
    )
}

fn is_define_case_label(line: &str) -> bool {
    let word = define_body_first_word(line);
    if word != "case" && word != "default" {
        return false;
    }
    line.trim_start()[word.len()..]
        .split(';')
        .next()
        .is_some_and(|head| head.contains(':'))
}

fn has_embedded_default_label(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let first_code = chars
        .iter()
        .position(|ch| !ch.is_whitespace())
        .unwrap_or(chars.len());
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        if is_identifier_start(ch) {
            let start = index;
            index += 1;
            while chars
                .get(index)
                .is_some_and(|ch| is_identifier_continue(*ch))
            {
                index += 1;
            }
            if start > first_code && chars[start..index].iter().collect::<String>() == "default" {
                let mut after = index;
                while chars.get(after).is_some_and(|ch| ch.is_whitespace()) {
                    after += 1;
                }
                if chars.get(after) == Some(&':') {
                    return true;
                }
            }
            continue;
        }
        index += 1;
    }
    false
}

fn scan_define_body_line(content: &str) -> DefineBodyLineInfo {
    let chars: Vec<char> = content.chars().collect();
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut last_code = None;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            last_code = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '{' => opens += 1,
            '}' => closes += 1,
            _ => {}
        }
        if !ch.is_whitespace() {
            last_code = Some(ch);
        }
        index += 1;
    }

    let ends_semicolon = last_code == Some(';');
    let is_command_header = is_define_header_keyword(content);
    let is_header = is_command_header && opens == 0 && !ends_semicolon;
    DefineBodyLineInfo {
        opens,
        closes,
        leading_close: content.trim_start().starts_with('}'),
        is_case_label: is_define_case_label(content),
        ends_semicolon,
        is_header,
        is_command_header,
        is_switch_header: define_body_first_word(content) == "switch",
        has_embedded_default_label: has_embedded_default_label(content),
    }
}

fn define_block_comment_state(line: &str, mut in_comment: bool, tab_width: usize) -> (bool, usize) {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut open_column = 0usize;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_comment {
            if ch == '*' && next == Some('/') {
                in_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_comment = true;
            open_column = index;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        index += 1;
    }
    (
        in_comment,
        visual_width_chars(&chars, open_column, tab_width),
    )
}

fn strip_define_backslash(line: &str) -> (&str, bool) {
    let trimmed = line.trim_end();
    if let Some(body) = trimmed.strip_suffix('\\') {
        (body.trim_end(), true)
    } else {
        (trimmed, false)
    }
}

fn define_replacement_text(first_line: &str) -> &str {
    let (body, _) = strip_define_backslash(first_line);
    let trimmed = body.trim_start();
    let rest = match trimmed.strip_prefix('#') {
        Some(after_pound) => after_pound.trim_start(),
        None => return "",
    };
    let directive_end = rest
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(rest.len());
    if &rest[..directive_end] != "define" {
        return "";
    }
    let after_directive = rest[directive_end..].trim_start();
    let name_end = after_directive
        .find(|ch: char| !is_identifier_continue(ch))
        .unwrap_or(after_directive.len());
    let after_name = &after_directive[name_end..];
    let after_params = if after_name.starts_with('(') {
        let mut depth = 0usize;
        let mut end = after_name.len();
        for (index, ch) in after_name.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        &after_name[end..]
    } else {
        after_name
    };
    after_params.trim_start()
}

fn is_define_header_frame(frame: DefineFrame) -> bool {
    matches!(frame, DefineFrame::Header | DefineFrame::SwitchHeader)
}

fn is_define_command_frame(frame: DefineFrame) -> bool {
    matches!(frame, DefineFrame::CommandBrace | DefineFrame::SwitchBrace)
}

fn apply_define_frame_transition(
    frames: &mut Vec<DefineFrame>,
    info: &DefineBodyLineInfo,
    starts_with_open: bool,
    starts_with_assignment: bool,
) {
    if info.closes > info.opens {
        for _ in 0..(info.closes - info.opens) {
            frames.pop();
        }
        while frames.last().copied().is_some_and(is_define_header_frame) {
            frames.pop();
        }
    } else if info.opens > info.closes {
        for slot in 0..(info.opens - info.closes) {
            let pending_header = if slot == 0
                && starts_with_open
                && frames.last().copied().is_some_and(is_define_header_frame)
            {
                frames.pop()
            } else {
                None
            };
            if slot == 0 && info.is_case_label {
                frames.push(DefineFrame::CaseBrace);
            } else if slot == 0 && starts_with_assignment {
                frames.push(DefineFrame::InitializerBrace);
            } else if slot == 0
                && (info.is_switch_header || pending_header == Some(DefineFrame::SwitchHeader))
            {
                frames.push(DefineFrame::SwitchBrace);
            } else if slot == 0
                && (info.is_command_header || pending_header == Some(DefineFrame::Header))
            {
                frames.push(DefineFrame::CommandBrace);
            } else {
                frames.push(DefineFrame::Brace);
            }
        }
    } else if info.is_header {
        frames.push(if info.is_switch_header {
            DefineFrame::SwitchHeader
        } else {
            DefineFrame::Header
        });
    } else if info.ends_semicolon {
        while frames.last().copied().is_some_and(is_define_header_frame) {
            frames.pop();
        }
    }
}

fn define_expression_continuation_spaces(line: &str, tab_width: usize) -> Option<usize> {
    let trimmed = line.trim_end();
    let source_body = trimmed.strip_suffix('\\').unwrap_or(trimmed);
    let body = source_body.trim_end();
    let mut parens = Vec::new();
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => parens.push(index),
            ')' => {
                parens.pop();
            }
            _ => {}
        }
    }
    parens.last().map(|index| {
        let after_open = index + 1;
        let anchor = if source_body[after_open..].chars().all(char::is_whitespace) {
            source_body.len()
        } else {
            after_open
        };
        visual_width_from(&source_body[..anchor], 0, tab_width)
    })
}

fn define_body_is_expression_continuation(parts: &[&str]) -> bool {
    parts.iter().all(|part| {
        let (body, _) = strip_define_backslash(part);
        let trimmed = body.trim_start();
        !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("//")
            && !trimmed.starts_with("/*")
            && !trimmed.contains(['{', '}', ';'])
    })
}

fn next_define_expression_indent(line: &str, base_spaces: usize, options: &FormatOptions) -> usize {
    let Some(open_paren) = line.rfind('(') else {
        return leading_visual_width(line, options.tab_width);
    };
    let aligned = visual_width_from(&line[..open_paren + 1], 0, options.tab_width);
    if aligned <= options.max_continuation_indent {
        return aligned;
    }
    let trimmed = line.trim_start();
    let levels = if trimmed.starts_with("if ")
        || trimmed.starts_with("if(")
        || line.contains(" if ")
        || line.contains(" if(")
    {
        3
    } else {
        2
    };
    base_spaces + levels * options.indent_width
}

fn define_assignment_continuation_indent(line: &str, tab_width: usize) -> Option<usize> {
    let line = line.trim_end_matches('\\').trim_end();
    line.ends_with('=')
        .then(|| visual_width_from(line, 0, tab_width) + 1)
}

fn define_complete_designated_initializer_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with(',') && (trimmed.starts_with('.') || trimmed.starts_with('['))
}

fn define_run_in_designated_initializer_column(line: &str, tab_width: usize) -> Option<usize> {
    line.find("{ .")
        .or_else(|| line.find("{ ["))
        .map(|column| visual_width_from(&line[..column + 2], 0, tab_width))
}

fn visual_width_chars(chars: &[char], end: usize, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    chars[..end].iter().fold(0, |column, ch| {
        if *ch == '\t' {
            (column / tab_width + 1) * tab_width
        } else {
            column + 1
        }
    })
}

fn define_assignment_align_column(line: &str, tab_width: usize) -> Option<usize> {
    let chars: Vec<char> = line.chars().collect();
    let mut index = 0usize;
    let mut depth = 0i32;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;

    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '=' if depth == 0 => {
                let prev = index.checked_sub(1).and_then(|i| chars.get(i).copied());
                let is_comparison = next == Some('=')
                    || matches!(prev, Some('=') | Some('!') | Some('<') | Some('>'));
                if !is_comparison {
                    let mut after = index + 1;
                    while chars.get(after).is_some_and(|ch| ch.is_whitespace()) {
                        after += 1;
                    }
                    return (after < chars.len())
                        .then(|| visual_width_chars(&chars, after, tab_width));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn update_define_expression_paren_anchors(line: &str, anchors: &mut Vec<usize>, tab_width: usize) {
    let (body, _) = strip_define_backslash(line);
    let chars: Vec<char> = body.chars().collect();
    let mut index = 0usize;
    let mut quote = None;
    let mut escaped = false;
    let mut in_block_comment = false;
    while let Some(&ch) = chars.get(index) {
        let next = chars.get(index + 1).copied();
        if in_block_comment {
            if ch == '*' && next == Some('/') {
                in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if let Some(quote_char) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote_char {
                quote = None;
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == Some('/') {
            break;
        }
        if ch == '/' && next == Some('*') {
            in_block_comment = true;
            index += 2;
            continue;
        }
        if ch == '"' || (ch == '\'' && !is_digit_separator(&chars, index)) {
            quote = Some(ch);
            index += 1;
            continue;
        }
        match ch {
            '(' => anchors.push(visual_width_chars(&chars, index + 1, tab_width)),
            ')' => {
                anchors.pop();
            }
            _ => {}
        }
        index += 1;
    }
}

impl FormatEngine<'_> {
    pub(super) fn finish_define_line(&mut self, line: &str) {
        let line_start = line.trim_start();
        if !line_start.starts_with("#define") {
            return;
        }
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        if code.ends_with('\\') {
            return;
        }
        self.continuation_indent.next_line_indent = None;
        self.continuation_indent.next_line_indent_spaces = None;
        self.stack_state.clear_continuation_indents();
        operator_chains::clear_operator_chain_state(
            &mut self.frame_stack,
            &mut self.continuation_indent.logical_chain_indent_spaces,
        );
    }

    fn define_assignment_row_anchor(&self, line: &str) -> Option<usize> {
        define_assignment_continuation_indent(line, self.options.tab_width)
    }

    pub(super) fn push_multiline_define(&mut self, parts: &[&str]) {
        let Some((first, body_parts)) = parts.split_first() else {
            return;
        };
        let define_base = self.preprocessor_base_level();
        let define_prefix = self.options.indent_prefix(define_base);
        let first_line = format!("{define_prefix}{}", first.trim_start());
        self.adjust_and_publish_line(first_line.clone());

        let body_level = define_base
            + if define_base > 0 && self.options.indent_preproc_block {
                0
            } else {
                1
            };
        if let Some(spaces) =
            define_expression_continuation_spaces(first.trim_start(), self.options.tab_width)
                .map(|spaces| spaces + define_base * self.options.indent_width)
            && define_body_is_expression_continuation(body_parts)
        {
            let mut paren_anchors = Vec::new();
            update_define_expression_paren_anchors(
                &first_line,
                &mut paren_anchors,
                self.options.tab_width,
            );
            let mut line_spaces = spaces;
            let mut previous_line = None;
            for (index, part) in body_parts.iter().enumerate() {
                if index > 0
                    && let Some(previous) = previous_line.as_deref()
                {
                    let current = strip_define_backslash(part).0.trim_start();
                    line_spaces = if let Some(anchor) = paren_anchors
                        .last()
                        .copied()
                        .filter(|column| *column <= self.options.max_continuation_indent)
                    {
                        anchor.saturating_sub(usize::from(current.starts_with(')')))
                    } else {
                        next_define_expression_indent(previous, spaces, self.options)
                    };
                }
                let prefix = self
                    .options
                    .continuation_indent_prefix(body_level, line_spaces);
                let line = format!("{prefix}{}", part.trim_start());
                self.adjust_and_publish_line(line.clone());
                update_define_expression_paren_anchors(
                    &line,
                    &mut paren_anchors,
                    self.options.tab_width,
                );
                previous_line = Some(line.trim_end_matches('\\').trim_end().to_string());
            }
            return;
        }

        if define_body_is_expression_continuation(body_parts) {
            let base_spaces = body_level * self.options.indent_width;
            let mut paren_anchors = Vec::new();
            let mut assignment_anchor = None;
            let mut line_spaces = base_spaces;
            for (index, part) in body_parts.iter().enumerate() {
                if index > 0 {
                    let current = strip_define_backslash(part).0.trim_start();
                    let current_starts_assignment =
                        current.starts_with('=') && current.as_bytes().get(1) != Some(&b'=');
                    line_spaces = if current_starts_assignment {
                        base_spaces + self.options.indent_width
                    } else {
                        assignment_anchor
                            .or_else(|| {
                                paren_anchors.last().copied().filter(|column| {
                                    *column <= self.options.max_continuation_indent
                                })
                            })
                            .unwrap_or(base_spaces)
                    };
                }
                let prefix = self
                    .options
                    .continuation_indent_prefix(body_level, line_spaces);
                let line = format!("{prefix}{}", part.trim_start());
                self.adjust_and_publish_line(line.clone());
                if let Some(anchor) = self.define_assignment_row_anchor(&line) {
                    assignment_anchor = Some(anchor);
                }
                update_define_expression_paren_anchors(
                    &line,
                    &mut paren_anchors,
                    self.options.tab_width,
                );
            }
            return;
        }

        let base_level = body_level;
        let mut frames: Vec<DefineFrame> = Vec::new();
        let first_replacement = define_replacement_text(first);
        if !first_replacement.is_empty() {
            let info = scan_define_body_line(first_replacement);
            let starts_with_assignment = first_replacement.starts_with('=')
                && first_replacement.as_bytes().get(1) != Some(&b'=');
            apply_define_frame_transition(
                &mut frames,
                &info,
                first_replacement.starts_with('{'),
                starts_with_assignment,
            );
        }
        let mut continuation_column =
            define_expression_continuation_spaces(&first_line, self.options.tab_width);
        let mut in_comment = false;
        let mut comment_src_open_col = 0usize;
        let mut comment_out_open_col = 0usize;
        let mut comment_structural_level = base_level;
        for part in body_parts {
            let display = part.trim_start();
            let (body, had_backslash) = strip_define_backslash(part);
            let content = body.trim_start();

            if in_comment {
                let line = if display.is_empty() {
                    String::new()
                } else {
                    let source_prefix = &part[..part.len() - display.len()];
                    let source_col = visual_width_from(source_prefix, 0, self.options.tab_width);
                    let column = if display.starts_with("*/") {
                        comment_out_open_col + 1
                    } else {
                        let rel = source_col as isize - comment_src_open_col as isize;
                        (comment_out_open_col as isize + rel)
                            .max((base_level * self.options.indent_width) as isize)
                            as usize
                    };
                    format!(
                        "{}{display}",
                        self.options
                            .continuation_indent_prefix(comment_structural_level, column)
                    )
                };
                self.adjust_and_publish_line(line);
                (in_comment, _) = define_block_comment_state(content, true, self.options.tab_width);
                continue;
            }

            if content.is_empty() && !had_backslash {
                self.adjust_and_publish_line(String::new());
                continue;
            }

            let info = scan_define_body_line(content);
            let starts_with_open = content.starts_with('{');
            let starts_with_assignment =
                content.starts_with('=') && content.as_bytes().get(1) != Some(&b'=');
            let assignment_extra = usize::from(starts_with_assignment);
            let depth = base_level + frames.len();
            let switch_extra = if self.options.indent_switches
                || matches!(
                    self.options.brace_style,
                    BraceStyle::Vtk | BraceStyle::Ratliff
                ) {
                let switch_count = frames
                    .iter()
                    .enumerate()
                    .filter(|(level, frame)| *level > 0 && **frame == DefineFrame::SwitchBrace)
                    .count();
                if info.leading_close && frames.last() == Some(&DefineFrame::SwitchBrace) {
                    switch_count.saturating_sub(1)
                } else {
                    switch_count
                }
            } else {
                0
            };
            let decreases_structural_level = info.leading_close
                || info.is_case_label && frames.last() == Some(&DefineFrame::SwitchBrace)
                || starts_with_open && frames.last().copied().is_some_and(is_define_header_frame)
                || info.has_embedded_default_label;
            let command_block_extra = if self.options.indent_blocks {
                frames
                    .iter()
                    .copied()
                    .filter(|frame| is_define_command_frame(*frame))
                    .count()
            } else {
                0
            };
            let case_block_unindent = usize::from(
                self.options.brace_style == BraceStyle::Whitesmith
                    && !self.options.indent_cases
                    && frames.contains(&DefineFrame::CaseBrace),
            );
            let indented_physical_brace = (self.options.indent_braces
                && (starts_with_open || info.leading_close))
                || (self.options.brace_style == BraceStyle::Vtk
                    && ((info.leading_close
                        && frames.last().is_some_and(|frame| {
                            is_define_command_frame(*frame) || *frame == DefineFrame::CaseBrace
                        }))
                        || (starts_with_open
                            && frames.last().copied().is_some_and(is_define_header_frame))))
                || (self.options.brace_style == BraceStyle::Gnu
                    && starts_with_open
                    && frames.last().copied().is_some_and(is_define_header_frame));
            let structural_level = ((if decreases_structural_level {
                depth.saturating_sub(1).max(base_level)
            } else {
                depth
            }) + switch_extra
                + assignment_extra
                + command_block_extra
                + usize::from(indented_physical_brace))
            .saturating_sub(case_block_unindent);

            let is_structural = info.opens > 0 || info.closes > 0 || info.leading_close;
            let continued_parameter_opens_body = continuation_column.is_some()
                && info.opens > 0
                && !starts_with_open
                && content.ends_with('{')
                && content.contains(')');
            let continued_designated_initializer_row = continuation_column.is_some()
                && (content.starts_with('.') || content.starts_with('['));
            let initializer_close_column = if info.leading_close && content.starts_with('}') {
                continuation_column.map(|column| column.saturating_sub(2))
            } else {
                None
            };
            let source_prefix = &part[..part.len() - display.len()];
            let source_indent = visual_width_from(source_prefix, 0, self.options.tab_width);
            let initializer_levels = frames
                .iter()
                .filter(|frame| **frame == DefineFrame::InitializerBrace)
                .count()
                .saturating_sub(usize::from(
                    info.leading_close && frames.last() == Some(&DefineFrame::InitializerBrace),
                ));
            let initializer_closer_style_level = usize::from(
                info.leading_close
                    && self.options.indent_braces
                    && frames.last() == Some(&DefineFrame::InitializerBrace),
            );
            let continuation_levels =
                assignment_extra + initializer_levels + initializer_closer_style_level;
            let prefix_structural_level = structural_level.saturating_sub(continuation_levels);
            let structural_prefix = self.options.continuation_indent_prefix(
                prefix_structural_level,
                structural_level * self.options.indent_width,
            );
            let keep_source_indent = self.options.min_conditional_indent
                == MinConditionalIndent::Zero
                && source_indent > 0
                && (content.starts_with('?')
                    || (content.starts_with(':') && !content.starts_with("::"))
                    || content.starts_with("};"))
                || info.leading_close
                    && content == "}"
                    && source_indent > structural_level * self.options.indent_width;
            let prefix = if keep_source_indent {
                self.options
                    .continuation_indent_prefix(prefix_structural_level, source_indent)
            } else if let Some(column) = initializer_close_column {
                self.options
                    .continuation_indent_prefix(prefix_structural_level, column)
            } else if let Some(column) = continuation_column.filter(|_| {
                !content.is_empty()
                    && (!is_structural
                        || continued_parameter_opens_body
                        || continued_designated_initializer_row)
            }) {
                self.options
                    .continuation_indent_prefix(prefix_structural_level, column)
            } else {
                structural_prefix
            };
            let emitted = format!("{prefix}{content}");
            self.adjust_and_publish_line(format!("{prefix}{display}"));

            let (ends_in_comment, open_column) =
                define_block_comment_state(content, false, self.options.tab_width);
            if ends_in_comment {
                in_comment = true;
                let source_prefix = &part[..part.len() - display.len()];
                let source_lead = visual_width_from(source_prefix, 0, self.options.tab_width);
                comment_src_open_col = source_lead + open_column;
                comment_out_open_col =
                    visual_width_from(&prefix, 0, self.options.tab_width) + open_column;
                comment_structural_level = prefix_structural_level;
            }

            apply_define_frame_transition(
                &mut frames,
                &info,
                starts_with_open,
                starts_with_assignment,
            );

            let line_open_paren = unmatched_open_paren_column(&emitted)
                .map(|column| visual_width_from(&emitted[..column], 0, self.options.tab_width));
            continuation_column = if starts_with_assignment && info.opens > info.closes {
                emitted.find('{').map(|column| {
                    visual_width_from(&emitted[..column + 1], 0, self.options.tab_width) + 1
                })
            } else if let Some(column) =
                define_run_in_designated_initializer_column(&emitted, self.options.tab_width)
            {
                Some(column)
            } else if is_structural {
                None
            } else if info.ends_semicolon && line_open_paren.is_none() {
                if continuation_column.is_some() && content.starts_with('(') {
                    continuation_column
                } else {
                    None
                }
            } else if let Some(column) = line_open_paren {
                Some(column + 1)
            } else if continuation_column.is_some() {
                continuation_column
            } else if define_complete_designated_initializer_row(content) {
                None
            } else {
                define_assignment_align_column(&emitted, self.options.tab_width)
            };
        }
    }
}
