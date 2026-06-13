pub(super) fn visual_width_from(text: &str, start_column: usize, tab_width: usize) -> usize {
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

pub(super) fn leading_visual_width(line: &str, tab_width: usize) -> usize {
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

pub(super) fn drop_leading_columns(line: &str, drop: usize, tab_width: usize) -> &str {
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

pub(super) fn visual_column_at(chars: &[char], index: usize, tab_width: usize) -> usize {
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
