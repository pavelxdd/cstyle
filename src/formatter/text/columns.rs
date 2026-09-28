pub(crate) fn visual_width_from(text: &str, start_column: usize, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    let mut column = start_column;
    for ch in text.chars() {
        if ch == '\t' {
            column += tab_width - (column % tab_width);
        } else {
            column += ch.len_utf8();
        }
    }
    column - start_column
}

pub(crate) fn leading_visual_width(line: &str, tab_width: usize) -> usize {
    let mut column = 0;
    for ch in line.chars() {
        match ch {
            '\t' => column += tab_width - (column % tab_width),
            ' ' => column += 1,
            _ => break,
        }
    }
    column
}

pub(crate) fn drop_leading_columns(line: &str, drop: usize, tab_width: usize) -> &str {
    let mut column = 0;
    let mut byte = 0;
    for ch in line.chars() {
        if column >= drop {
            break;
        }
        match ch {
            '\t' => column += tab_width - (column % tab_width),
            ' ' => column += 1,
            _ => break,
        }
        byte += ch.len_utf8();
    }
    &line[byte..]
}

pub(crate) fn visual_column_at(chars: &[char], index: usize, tab_width: usize) -> usize {
    let tab_width = tab_width.max(1);
    let mut column = 0;
    for &ch in &chars[..index] {
        if ch == '\t' {
            column += tab_width - (column % tab_width);
        } else {
            column += 1;
        }
    }
    column
}

pub(crate) fn leading_whitespace_len(line: &str) -> usize {
    line.char_indices()
        .take_while(|(_, ch)| matches!(ch, ' ' | '\t'))
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0)
}

/// Visual column right after the byte at `index` of `line`, which may hold
/// tabs: a byte offset is no column once tabs indent the line.
pub(crate) fn column_after(line: &str, index: usize, tab_width: usize) -> usize {
    visual_width_from(&line[..=index], 0, tab_width)
}
