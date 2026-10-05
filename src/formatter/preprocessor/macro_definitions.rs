use crate::config::{BraceStyle, FormatOptions, MinConditionalIndent};
use crate::formatter::continuation::{min_conditional_indent_spaces, operator_chains};
use crate::formatter::engine::FormatEngine;
use crate::formatter::text::columns::{leading_visual_width, visual_column_at, visual_width_from};
use crate::formatter::text::line_scan::{
    advance_quoted_literal, trailing_comment_split_limit, unmatched_open_paren_columns,
};
use crate::source::lex::{
    is_digit_separator, is_identifier_continue, is_identifier_start, leading_identifier,
};

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
    trailing_header: bool,
    opens: usize,
    closes: usize,
    leading_close: bool,
    is_case_label: bool,
    ends_semicolon: bool,
    /// The closing `;` ends the statement rather than a clause of a
    /// `for` header whose parens stay open.
    semicolon_ends_statement: bool,
    paren_balance: isize,
    is_header: bool,
    is_command_header: bool,
    is_switch_header: bool,
    has_embedded_default_label: bool,
}

fn is_define_header_keyword(line: &str) -> bool {
    matches!(
        leading_identifier(line),
        "if" | "else" | "for" | "while" | "do" | "switch"
    )
}

fn is_define_case_label(line: &str) -> bool {
    let word = leading_identifier(line);
    if word != "case" && word != "default" {
        return false;
    }
    line.trim_start()[word.len()..]
        .split(';')
        .next()
        .is_some_and(|head| head.contains(':'))
}

fn is_define_user_label(line: &str) -> bool {
    let word = leading_identifier(line);
    !word.is_empty()
        && !matches!(
            word,
            "case" | "default" | "public" | "private" | "protected"
        )
        && line.trim_start()[word.len()..]
            .trim_start()
            .strip_prefix(':')
            .is_some_and(|rest| !rest.starts_with(':'))
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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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
    let mut paren_balance = 0isize;

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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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
            '(' => paren_balance += 1,
            ')' => paren_balance -= 1,
            _ => {}
        }
        if !ch.is_whitespace() {
            last_code = Some(ch);
        }
        index += 1;
    }

    let ends_semicolon = last_code == Some(';');
    let semicolon_ends_statement = ends_semicolon && paren_balance <= 0;
    let is_command_header = is_define_header_keyword(content);
    let is_header = is_command_header && opens == 0 && !semicolon_ends_statement;
    // A header after a brace, as in `} else` or `{ if (x)`, with its body on
    // the next row.
    let trailing_header = !ends_semicolon
        && content
            .rfind(['{', '}'])
            .is_some_and(|brace| is_define_header_keyword(&content[brace + 1..]));
    DefineBodyLineInfo {
        trailing_header,
        opens,
        closes,
        leading_close: content.trim_start().starts_with('}'),
        is_case_label: is_define_case_label(content),
        ends_semicolon,
        semicolon_ends_statement,
        paren_balance,
        is_header,
        is_command_header,
        is_switch_header: leading_identifier(content) == "switch",
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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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
    (in_comment, visual_column_at(&chars, open_column, tab_width))
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
    opens_header_block: bool,
    starts_with_assignment: bool,
) {
    if info.closes > info.opens {
        for _ in 0..(info.closes - info.opens) {
            frames.pop();
        }
        while frames.last().copied().is_some_and(is_define_header_frame) {
            frames.pop();
        }
        if info.trailing_header {
            frames.push(DefineFrame::Header);
        }
    } else if info.opens > info.closes {
        for slot in 0..(info.opens - info.closes) {
            let pending_header = if slot == 0
                && opens_header_block
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
        if info.trailing_header {
            frames.push(DefineFrame::Header);
        }
    } else if opens_header_block
        && info.opens > 0
        && frames.last().copied().is_some_and(is_define_header_frame)
    {
        // A one-line block is the whole body of the header.
        frames.pop();
    } else if info.is_header {
        frames.push(if info.is_switch_header {
            DefineFrame::SwitchHeader
        } else {
            DefineFrame::Header
        });
    } else if info.semicolon_ends_statement {
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
            // A brace, as in a statement expression, starts statements.
            '{' => parens.clear(),
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
            // A control header starts statements, not an expression.
            && !["if", "for", "while", "switch", "do", "else"].iter().any(|header| {
                trimmed.strip_prefix(header).is_some_and(|rest| {
                    rest.is_empty() || rest.starts_with([' ', '\t', '('])
                })
            })
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

/// A designator row ends with its comma, or is the last row and ends
/// with its value.
fn define_complete_designated_initializer_row(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.starts_with('.') || trimmed.starts_with('['))
        && (trimmed.ends_with(',')
            || !(trimmed.ends_with(['=', '(', '[', '{'])
                || crate::formatter::tokens::operators::head_ends_binary_operator(trimmed)))
}

fn define_run_in_designated_initializer_column(line: &str, tab_width: usize) -> Option<usize> {
    line.find("{ .")
        .or_else(|| line.find("{ ["))
        .map(|column| visual_width_from(&line[..column + 2], 0, tab_width))
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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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
                let previous = index.checked_sub(1).and_then(|i| chars.get(i).copied());
                let is_comparison = next == Some('=')
                    || matches!(previous, Some('=') | Some('!') | Some('<') | Some('>'));
                if !is_comparison {
                    let mut after = index + 1;
                    while chars.get(after).is_some_and(|ch| ch.is_whitespace()) {
                        after += 1;
                    }
                    return (after < chars.len())
                        .then(|| visual_column_at(&chars, after, tab_width));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// The column the rows after a statement row with an open paren align at:
/// the first character after the last open paren, a continuing backslash
/// included, but at least the minimum conditional indent within a header.
fn define_row_paren_continuation(
    line: &str,
    level_spaces: usize,
    options: &FormatOptions,
) -> Option<usize> {
    let (body, _) = strip_define_backslash(line);
    let open = *unmatched_open_paren_columns(body).last()?;
    let full: Vec<char> = line.trim_end().chars().collect();
    let open = body[..open].chars().count();
    let next = full[open + 1..]
        .iter()
        .position(|ch| !ch.is_whitespace())
        .map_or(open + 1, |offset| open + 1 + offset);
    let row_indent = leading_visual_width(line, options.tab_width);
    let mut column = visual_column_at(&full, next, options.tab_width);
    let min = min_conditional_indent_spaces(options);
    if is_define_header_keyword(line) && column < level_spaces + min {
        column = row_indent + min;
    }
    Some(capped_define_continuation(
        column,
        row_indent,
        level_spaces,
        options,
    ))
}

/// The column of the value a `return` leading a row continues past its
/// line.
fn define_return_value_column(row: &str, tab_width: usize) -> Option<usize> {
    let (body, _) = strip_define_backslash(row);
    let trimmed = body.trim_start();
    let rest = trimmed.strip_prefix("return")?;
    if !rest.starts_with(char::is_whitespace) || body.trim_end().ends_with(';') {
        return None;
    }
    let value = body.len() - rest.trim_start().len();
    (value < body.trim_end().len()).then(|| visual_width_from(&body[..value], 0, tab_width))
}

/// The column rows continuing at `column` take: past the maximum from the
/// indent level of the rows, two indents past the row with the paren.
fn capped_define_continuation(
    column: usize,
    row_indent: usize,
    level_spaces: usize,
    options: &FormatOptions,
) -> usize {
    if column > level_spaces + options.max_continuation_indent {
        2 * options.indent_width + row_indent
    } else {
        column
    }
}

/// The `close` of an anchor that an assignment registers rather than a
/// paren.
const ASSIGNMENT_ANCHOR: usize = usize::MAX;

/// The column a row continuing at the top of `anchors` takes: a row led
/// by `)` stands where its paren saved, others at the top anchor.
fn define_row_anchor(anchors: &[(usize, usize)], row: &str) -> Option<usize> {
    if row.trim_start().starts_with(')') {
        anchors
            .iter()
            .rev()
            .find(|&&(close, _)| close != ASSIGNMENT_ANCHOR)
            .map(|&(close, _)| close)
    } else {
        anchors.last().map(|&(_, align)| align)
    }
}

/// Replays astyle's continuation stack over a row: each open paren and
/// each assignment records the column the rows after it align at, and for
/// a paren the column a row starting with its `)` takes, apart from the
/// `extra` spaces the rows take after a control header. A `)` drops what
/// its paren recorded, a `,` the assignments within the paren. Returns
/// whether `line` closes a control header with code after it, which
/// indents the rows after it by a level, the columns the parens align at
/// included.
fn update_define_expression_paren_anchors(
    line: &str,
    anchors: &mut Vec<(usize, usize)>,
    level_spaces: usize,
    extra: usize,
    options: &FormatOptions,
) -> bool {
    let tab_width = options.tab_width;
    let row_indent = leading_visual_width(line, tab_width).saturating_sub(extra);
    let mut header_depth = None;
    let mut header_close = None;
    let paren_count = |anchors: &Vec<(usize, usize)>| {
        anchors
            .iter()
            .filter(|&&(close, _)| close != ASSIGNMENT_ANCHOR)
            .count()
    };
    // The maximum counts from the indent level of the rows; past it, rows
    // fall back to two indents past the row with the paren.
    let limit = level_spaces + options.max_continuation_indent;
    let fallback = 2 * options.indent_width + row_indent;
    let (body, _) = strip_define_backslash(line);
    let chars: Vec<char> = body.chars().collect();
    let full: Vec<char> = line.trim_end().chars().collect();
    // astyle aligns at the first character after the paren or the
    // assignment, a continuing backslash included.
    let register = |anchors: &Vec<(usize, usize)>, index: usize| {
        let previous = anchors.last().map(|&(_, align)| align);
        let next = full[index + 1..]
            .iter()
            .position(|ch| !ch.is_whitespace())
            .map(|offset| index + 1 + offset);
        match next {
            Some(next) if !options.indent_after_parens => {
                let align = visual_column_at(&full, next, tab_width).saturating_sub(extra);
                let align = if align > limit { fallback } else { align };
                (
                    visual_column_at(&full, index, tab_width).saturating_sub(extra),
                    align.max(previous.unwrap_or(0)),
                )
            }
            _ => {
                let previous = previous.unwrap_or(row_indent);
                let align = options.continuation_indent * options.indent_width + previous;
                let align = if align > limit { fallback } else { align };
                (previous, align)
            }
        }
    };
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
        if quote.is_some() {
            advance_quoted_literal(ch, &mut quote, &mut escaped);
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
        let previous_char = index.checked_sub(1).map(|before| chars[before]);
        match ch {
            '(' => {
                let word_end = chars[..index]
                    .iter()
                    .rposition(|ch| !ch.is_whitespace())
                    .map_or(0, |end| end + 1);
                let word_start = chars[..word_end]
                    .iter()
                    .rposition(|ch| !ch.is_alphanumeric() && *ch != '_')
                    .map_or(0, |start| start + 1);
                let word: String = chars[word_start..word_end].iter().collect();
                if header_depth.is_none() && matches!(word.as_str(), "if" | "while" | "for") {
                    header_depth = Some(paren_count(anchors));
                }
                let anchor = register(anchors, index);
                anchors.push(anchor);
            }
            ')' => {
                while anchors
                    .pop()
                    .is_some_and(|(close, _)| close == ASSIGNMENT_ANCHOR)
                {}
                if header_depth == Some(paren_count(anchors)) {
                    header_depth = None;
                    header_close = Some(index);
                }
            }
            ',' | ';' => {
                if ch == ';' || paren_count(anchors) > 0 {
                    while anchors
                        .last()
                        .is_some_and(|&(close, _)| close == ASSIGNMENT_ANCHOR)
                    {
                        anchors.pop();
                    }
                }
            }
            '=' if next != Some('=') && !matches!(previous_char, Some('=' | '!' | '<' | '>')) => {
                let (_, align) = register(anchors, index);
                anchors.push((ASSIGNMENT_ANCHOR, align));
            }
            // Indenting after parens stacks `return` like an assignment.
            'r' if options.indent_after_parens
                && !previous_char.is_some_and(|ch| ch.is_alphanumeric() || ch == '_')
                && chars[index..].starts_with(&['r', 'e', 't', 'u', 'r', 'n'])
                && !chars
                    .get(index + 6)
                    .is_some_and(|ch| ch.is_alphanumeric() || *ch == '_') =>
            {
                let (_, align) = register(anchors, index + 5);
                anchors.push((ASSIGNMENT_ANCHOR, align));
                index += 6;
                continue;
            }
            _ => {}
        }
        index += 1;
    }
    header_close.is_some_and(|close| chars[close + 1..].iter().any(|ch| !ch.is_whitespace()))
}

impl FormatEngine<'_> {
    pub(crate) fn finish_define_line(&mut self, line: &str) {
        let line_start = line.trim_start();
        if !line_start.starts_with("#define") {
            return;
        }
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        if code.ends_with('\\') {
            return;
        }
        self.layout.continuation_indent.clear_next_line();
        self.layout.nesting.clear_continuation_indents();
        operator_chains::clear_operator_chain_state(
            &mut self.layout.frame_stack,
            &mut self.layout.continuation_indent.logical_chain_indent_spaces,
        );
    }

    pub(super) fn push_multiline_define(&mut self, parts: &[&str]) {
        let Some((first, body_parts)) = parts.split_first() else {
            return;
        };
        let define_base = self.preprocessor_base_level();
        let define_prefix = self.options.indent_prefix(define_base);
        let first_line = format!("{define_prefix}{}", first.trim_start());
        self.adjust_and_publish_line(first_line.clone());

        // astyle indents no body past a `#define` whose parameter list
        // continues on the next line.
        let body_level = define_base
            + if define_base > 0 && self.options.indent_preproc_block
                || open_parens_after(0, first) > 0
            {
                0
            } else {
                1
            };
        if let Some(spaces) =
            define_expression_continuation_spaces(first.trim_start(), self.options.tab_width)
                .map(|spaces| spaces + define_base * self.options.indent_width)
            && define_body_is_expression_continuation(body_parts)
        {
            self.push_define_rows_aligned_to_header(&first_line, body_parts, body_level, spaces);
            return;
        }

        if define_body_is_expression_continuation(body_parts) {
            self.push_define_expression_rows(body_parts, body_level);
            return;
        }

        self.push_define_statement_rows(first, &first_line, body_parts, body_level);
    }

    /// Lays out an expression body whose rows align to the expression that starts
    /// on the `#define` line.
    fn push_define_rows_aligned_to_header(
        &mut self,
        first_line: &str,
        body_parts: &[&str],
        body_level: usize,
        spaces: usize,
    ) {
        let mut paren_anchors = Vec::new();
        let level_spaces = leading_visual_width(first_line, self.options.tab_width);
        let mut extra = 0;
        if update_define_expression_paren_anchors(
            first_line,
            &mut paren_anchors,
            level_spaces,
            extra,
            self.options,
        ) {
            extra = self.options.indent_width;
        }
        let mut line_spaces = paren_anchors
            .last()
            .map_or(spaces, |&(_, align)| align + extra);
        let mut previous_line = None;
        for (index, part) in body_parts.iter().enumerate() {
            if index > 0
                && let Some(previous) = previous_line.as_deref()
            {
                let current = strip_define_backslash(part).0.trim_start();
                line_spaces = if let Some(anchor) = define_row_anchor(&paren_anchors, current) {
                    anchor + extra
                } else {
                    next_define_expression_indent(previous, spaces, self.options)
                };
            }
            let prefix = self
                .options
                .continuation_indent_prefix(body_level, line_spaces);
            let line = format!("{prefix}{}", part.trim_start());
            self.adjust_and_publish_line(line.clone());
            if update_define_expression_paren_anchors(
                &line,
                &mut paren_anchors,
                level_spaces,
                extra,
                self.options,
            ) {
                extra = self.options.indent_width;
            }
            previous_line = Some(line.trim_end_matches('\\').trim_end().to_string());
        }
    }

    fn push_define_expression_rows(&mut self, body_parts: &[&str], body_level: usize) {
        let base_spaces = body_level * self.options.indent_width;
        let mut paren_anchors = Vec::new();
        let mut line_spaces = base_spaces;
        for (index, part) in body_parts.iter().enumerate() {
            let current = strip_define_backslash(part).0.trim_start();
            let current_starts_assignment =
                current.starts_with('=') && current.as_bytes().get(1) != Some(&b'=');
            // Inside parens a leading `=` stands at them like any row.
            if current_starts_assignment && paren_anchors.is_empty() {
                line_spaces = base_spaces + self.options.indent_width;
            } else if index > 0 {
                line_spaces = define_row_anchor(&paren_anchors, current).unwrap_or(base_spaces);
            }
            let prefix = self
                .options
                .continuation_indent_prefix(body_level, line_spaces);
            let line = format!("{prefix}{}", part.trim_start());
            self.adjust_and_publish_line(line.clone());
            update_define_expression_paren_anchors(
                &line,
                &mut paren_anchors,
                base_spaces,
                0,
                self.options,
            );
        }
    }

    /// Whether a block stays open past the row, which indents a VTK
    /// closing brace.
    fn vtk_closing_row_leaves_block_open(
        &self,
        frames: &[DefineFrame],
        info: &DefineBodyLineInfo,
        opens_header_block: bool,
        starts_with_assignment: bool,
    ) -> bool {
        let mut after = frames.to_vec();
        apply_define_frame_transition(&mut after, info, opens_header_block, starts_with_assignment);
        after
            .iter()
            .any(|frame| !is_define_header_frame(*frame) && *frame != DefineFrame::InitializerBrace)
    }

    fn push_define_statement_rows(
        &mut self,
        first: &str,
        first_line: &str,
        body_parts: &[&str],
        body_level: usize,
    ) {
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
        let first_indent = leading_visual_width(first_line, self.options.tab_width);
        // The column rows continued at before a paren opened, which they
        // resume once it closes.
        let mut column_before_parens = None;
        let mut continuation_column =
            define_expression_continuation_spaces(first_line, self.options.tab_width)
                .or_else(|| {
                    // An assignment the replacement starts registers its value.
                    (!first_replacement.contains(['{', ';']))
                        .then(|| {
                            let (body, _) = strip_define_backslash(first_line);
                            define_assignment_align_column(body.trim_end(), self.options.tab_width)
                        })
                        .flatten()
                })
                .map(|column| {
                    capped_define_continuation(column, first_indent, first_indent, self.options)
                });
        let mut open_parens = 0isize;
        // The parameters a `#define` line leaves open, which its body follows.
        let mut open_parameters = open_parens_after(0, first);
        // The parens open across the rows, for rows that close inner ones.
        let mut paren_anchors = Vec::new();
        let mut in_comment = false;
        let mut comment_source_open_column = 0usize;
        let mut comment_output_open_column = 0usize;
        let mut comment_structural_level = base_level;
        for (part_index, part) in body_parts.iter().enumerate() {
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
                        comment_output_open_column + 1
                    } else {
                        let rel = source_col as isize - comment_source_open_column as isize;
                        (comment_output_open_column as isize + rel)
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

            // The replacement starts after the row closing the parameters.
            let parameters_before = open_parameters;
            let closes_parameters = open_parameters > 0 && {
                open_parameters = open_parens_after(open_parameters, content);
                open_parameters == 0
            };
            let mut info = scan_define_body_line(if closes_parameters {
                parameters_tail(parameters_before, content)
            } else {
                content
            });
            info.semicolon_ends_statement &= open_parens + info.paren_balance <= 0;
            let starts_with_open = content.starts_with('{');
            // A `{` ending the condition of a header opens the header's block.
            let opens_header_block = starts_with_open
                || open_parens > 0 && frames.last().copied().is_some_and(is_define_header_frame);
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
                        && self.vtk_closing_row_leaves_block_open(
                            &frames,
                            &info,
                            opens_header_block,
                            starts_with_assignment,
                        )
                        && frames.last().is_some_and(|frame| {
                            is_define_command_frame(*frame) || *frame == DefineFrame::CaseBrace
                        }))
                        // In code, the brace closing the define's block
                        // stands at its body, unless it ends the define
                        // alone.
                        || (info.leading_close
                            && self.layout.indentation.indent() > 0
                            && (part_index + 1 < body_parts.len()
                                || !content
                                    .trim_end_matches('\\')
                                    .trim_end()
                                    .trim_start_matches('}')
                                    .trim()
                                    .is_empty())
                            && frames.len() == 1
                            && frames.last().copied().is_some_and(is_define_command_frame))
                        // A header's brace at the top of the body stands at
                        // the header.
                        || (starts_with_open
                            && frames.last().copied().is_some_and(is_define_header_frame)
                            && frames.iter().any(|frame| !is_define_header_frame(*frame)))))
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
            // Labels stand at the body of the define, or a level out with
            // indented labels.
            let structural_level = if is_define_user_label(content) {
                if self.options.indent_labels {
                    structural_level.saturating_sub(1).max(base_level)
                } else {
                    base_level
                }
            } else {
                structural_level
            };

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
                    || (content.starts_with(':') && !content.starts_with("::")));
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
                        || continued_designated_initializer_row
                        || open_parens > 0 && !info.leading_close && !starts_with_open
                        // A row that only closes a block after its code
                        // continues the code.
                        || info.opens == 0 && !info.leading_close)
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
                comment_source_open_column = source_lead + open_column;
                comment_output_open_column =
                    visual_width_from(&prefix, 0, self.options.tab_width) + open_column;
                comment_structural_level = prefix_structural_level;
            }

            apply_define_frame_transition(
                &mut frames,
                &info,
                opens_header_block,
                starts_with_assignment,
            );
            let parens_before = open_parens;
            open_parens = open_parens_after(open_parens, content);
            if open_parens > 0 {
                let row = format!("{prefix}{display}");
                let level_spaces = structural_level * self.options.indent_width;
                let depth = paren_anchors.len();
                update_define_expression_paren_anchors(
                    &row,
                    &mut paren_anchors,
                    level_spaces,
                    0,
                    self.options,
                );
                // A header's condition continues at least at the minimum
                // conditional indent, unless parens indent after them.
                let min = min_conditional_indent_spaces(self.options);
                if !self.options.indent_after_parens
                    && is_define_header_keyword(content)
                    && let Some((_, align)) = paren_anchors.get_mut(depth)
                    && *align < level_spaces + min
                {
                    *align = leading_visual_width(&row, self.options.tab_width) + min;
                }
            } else {
                paren_anchors.clear();
            }

            let line_open_paren = !unmatched_open_paren_columns(&emitted).is_empty();
            if parens_before <= 0 && open_parens > 0 {
                column_before_parens = continuation_column;
            }
            let parens_closed = parens_before > 0 && open_parens <= 0;
            continuation_column = if closes_parameters {
                None
            } else if starts_with_assignment && info.opens > info.closes {
                emitted.find('{').map(|column| {
                    visual_width_from(&emitted[..column + 1], 0, self.options.tab_width) + 1
                })
            } else if let Some(column) =
                define_run_in_designated_initializer_column(&emitted, self.options.tab_width)
            {
                Some(column)
            } else if is_structural {
                None
            } else if info.ends_semicolon && !line_open_paren {
                // Inside the parentheses of a `for` header the next row keeps
                // their column.
                if continuation_column.is_some() && open_parens > 0 {
                    continuation_column
                } else {
                    None
                }
            } else if line_open_paren && self.options.indent_after_parens {
                paren_anchors.last().map(|&(_, align)| align)
            } else if line_open_paren {
                define_row_paren_continuation(
                    &format!("{prefix}{display}"),
                    structural_level * self.options.indent_width,
                    self.options,
                )
            } else if open_parens <= 0
                && content.ends_with(')')
                && frames.last().copied().is_some_and(is_define_header_frame)
            {
                // The header's condition closed: its body follows.
                None
            } else if parens_closed && column_before_parens.is_some() {
                column_before_parens.take()
            } else if let Some(&(_, align)) = paren_anchors.last() {
                Some(align)
            } else if continuation_column.is_some() {
                continuation_column
            } else if define_complete_designated_initializer_row(content) {
                None
            } else {
                // An assignment registers the column after it, a continuing
                // backslash included.
                let row = format!("{prefix}{display}");
                let row_indent = leading_visual_width(&row, self.options.tab_width);
                define_assignment_align_column(row.trim_end(), self.options.tab_width)
                    .or_else(|| define_return_value_column(row.trim_end(), self.options.tab_width))
                    .map(|column| {
                        if self.options.indent_after_parens {
                            row_indent
                                + self.options.continuation_indent * self.options.indent_width
                        } else {
                            capped_define_continuation(
                                column,
                                row_indent,
                                structural_level * self.options.indent_width,
                                self.options,
                            )
                        }
                    })
            };
        }
    }
}

/// The text after the parenthesis that closes `open` continued parameter
/// parentheses.
fn parameters_tail(open: isize, text: &str) -> &str {
    let mut balance = open;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => balance += 1,
            ')' => {
                balance -= 1;
                if balance == 0 {
                    return text[index + 1..].trim_start();
                }
            }
            _ => {}
        }
    }
    text
}

/// The parentheses left open after a define body row, starting from
/// `open`; a brace starts statements that no outer parenthesis continues.
fn open_parens_after(open: isize, text: &str) -> isize {
    let mut balance = open;
    let mut quote = None;
    let mut escaped = false;
    for ch in text.chars() {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' => quote = Some(ch),
            '(' => balance += 1,
            ')' => balance = (balance - 1).max(0),
            '{' | '}' => balance = 0,
            _ => {}
        }
    }
    balance
}
