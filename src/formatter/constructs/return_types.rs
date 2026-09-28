use crate::formatter::constructs::headers::is_header;
use crate::formatter::engine::FormatEngine;
use crate::formatter::structure::TokenSpan;
use crate::formatter::structure::functions::FunctionHead;
use crate::formatter::syntax::function_name_start;
use crate::formatter::syntax::language::{self, is_non_type_keyword, is_type_like_pointer_word};
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::{
    find_outside_quotes, line_ends_with_comment, line_paren_imbalance,
    reverse_scan_skips_block_comment, trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::source::lex::is_identifier_continue;

impl FormatEngine<'_> {
    pub(crate) fn split_return_type_pointer_name_indent_spaces(&self, line: &str) -> Option<usize> {
        if !is_pointer_prefixed_function_part(line.trim_start()) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        is_return_type_line(previous.trim())
            .then(|| leading_visual_width(previous, self.options.tab_width))
    }

    pub(crate) fn split_trailing_return_arrow_indent_spaces(&self, line: &str) -> Option<usize> {
        let current = line.trim_start();
        if !current.starts_with("->") || current.starts_with("->*") {
            return None;
        }

        let mut close_pending = 0usize;
        let mut in_block_comment = false;
        for previous in self
            .output
            .iter()
            .rev()
            .filter(|line| !line.trim().is_empty())
            .take(16)
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if reverse_scan_skips_block_comment(code, &mut in_block_comment) {
                continue;
            }
            if close_pending == 0 && !code.ends_with(')') {
                return None;
            }
            let (closes, mut opens) = line_paren_imbalance(code);
            if close_pending > 0
                && let Some(&column) = opens.last()
                && code[column..].starts_with('(')
            {
                let before = code[..column].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                if is_parameter_return_type_prefix(return_type)
                    && !name.is_empty()
                    && !is_header(self.options, name)
                {
                    return Some(leading_visual_width(previous, self.options.tab_width));
                }
            }
            let cancel = close_pending.min(opens.len());
            for _ in 0..cancel {
                opens.pop();
            }
            close_pending = close_pending - cancel + closes;
            if close_pending == 0
                && (code.ends_with(';') || code.ends_with('{') || code.ends_with('}'))
            {
                return None;
            }
        }
        None
    }

    fn recent_base_trailing_return_function_header_index(&self) -> Option<usize> {
        if self.layout.indentation.indent() == 0 {
            return None;
        }
        let mut closed_blocks = 0usize;
        for (index, line) in self.output.iter().enumerate().rev().take(24) {
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('}') {
                closed_blocks += 1;
            }
            if code.ends_with('{') {
                if closed_blocks > 0 {
                    closed_blocks -= 1;
                    continue;
                }
                if code.contains(") ->") && !trimmed.starts_with("template ") {
                    return Some(index);
                }
                return None;
            }
        }
        None
    }

    pub(crate) fn recent_base_trailing_return_function_header(&self) -> bool {
        self.recent_base_trailing_return_function_header_index()
            .is_some()
    }

    pub(crate) fn recent_trailing_return_function_after_multiline_template_declaration(
        &self,
    ) -> bool {
        let Some(brace_index) = self.recent_base_trailing_return_function_header_index() else {
            return false;
        };
        let mut signature_start = brace_index;
        let mut index = brace_index;
        while index > 0 {
            index -= 1;
            let line = &self.output[index];
            let code = line[..trailing_comment_split_limit(line)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with("template ")
                || code.ends_with(';')
                || code.ends_with('{')
                || code.ends_with('}')
            {
                break;
            }
            signature_start = index;
            if code.contains('(') {
                break;
            }
        }
        self.output_closes_multiline_template_declaration_before(signature_start)
    }

    pub(crate) fn trailing_return_function_parameter_tail_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let current = line.trim_start();
        if !current.contains("= {}") || !current.contains(") ->") || !current.ends_with('{') {
            return None;
        }
        for (previous_index, previous) in self
            .output
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, line)| !line.trim().is_empty())
            .take(8)
        {
            let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
            if let Some(open) = unmatched_open_paren_column(previous_code) {
                let before = previous_code[..open].trim_end();
                let name_start = function_name_start(before)?;
                let return_type = before[..name_start].trim_end();
                let name = before[name_start..].trim_start();
                let prefixed_return_type = return_type
                    .split_whitespace()
                    .any(language::is_macro_like_word)
                    && is_parameter_return_type_prefix(return_type);
                if leading_visual_width(previous, self.options.tab_width) == 0
                    && (prefixed_return_type
                        || self.output_closes_multiline_template_declaration_before(previous_index))
                    && !name.is_empty()
                    && !is_header(self.options, name)
                {
                    return Some(0);
                }
                return None;
            }
            if previous_code.ends_with([';', '{', '}']) {
                return None;
            }
        }
        None
    }

    pub(crate) fn function_signature_parameter_continuation_indent_spaces(
        &self,
        current_line: &str,
        signature_line: &str,
        open_paren: usize,
    ) -> Option<usize> {
        let before = signature_line[..open_paren].trim_end();
        let name_start = function_name_start(before)?;
        let return_type = before[..name_start].trim_end();
        let name = before[name_start..].trim_start();
        if !is_parameter_return_type_prefix(return_type)
            || name.is_empty()
            || is_header(self.options, name)
        {
            return None;
        }
        let after_paren = &signature_line[open_paren + 1..];
        let after_paren_indent = after_paren.len() - after_paren.trim_start().len();
        let visual_open =
            visual_width_from(&signature_line[..open_paren], 0, self.options.tab_width);
        let visual_after = visual_width_from(
            &after_paren[..after_paren_indent],
            visual_open + 1,
            self.options.tab_width,
        );
        let spaces = if current_line.starts_with(')') {
            visual_open
        } else {
            visual_open + 1 + visual_after
        };
        (spaces <= self.options.max_continuation_indent).then_some(spaces)
    }

    /// Whether a return type option applies to `head`: `definition_option`
    /// for definitions, `declaration_option` for declarations.
    fn return_type_option_applies(
        head: &FunctionHead,
        definition_option: bool,
        declaration_option: bool,
    ) -> bool {
        if head.body.is_some() {
            definition_option
        } else {
            declaration_option
        }
    }

    /// Whether the return type of `head` can move to or from its own line:
    /// the name must sit at the same nesting as the return type, which rules
    /// out `void (*f(int))(void)` and `int (name)(...)`.
    fn has_movable_return_type(&self, head: &FunctionHead) -> bool {
        head.has_return_type()
            && self.tree.groups.enclosing(head.name) == self.tree.groups.enclosing(head.start)
    }

    /// Joins a function name line to the return type on the previous output
    /// line (`--attach-return-type`, `--attach-return-type-decl`).
    pub(crate) fn try_publish_attached_return_type(&mut self, line: &str) -> bool {
        let Some(span) = self.output.pending_tokens() else {
            return false;
        };
        let Some(head) = self.tree.functions.named_at(span.first).cloned() else {
            return false;
        };
        if !Self::return_type_option_applies(
            &head,
            self.options.attach_return_type,
            self.options.attach_return_type_decl,
        ) || !self.has_movable_return_type(&head)
        {
            return false;
        }
        let Some(previous_index) = self.output.len().checked_sub(1) else {
            return false;
        };
        let previous_is_return_type =
            self.output
                .line_tokens(previous_index)
                .is_some_and(|previous| {
                    previous.first == head.start
                        && self.tree.previous_code_token(head.name_start) == Some(previous.last)
                });
        let previous = &self.output[previous_index];
        // AStyle attaches only return types it recognizes as types, and never
        // a split `struct Type *`.
        let previous_trimmed = previous.trim();
        if !previous_is_return_type
            || line_ends_with_comment(previous)
            || !is_return_type_line(previous_trimmed)
            || (previous_trimmed.starts_with("struct ") && previous_trimmed.ends_with('*'))
        {
            return false;
        }
        let previous = self.output.pop().expect("previous line exists");
        let previous_trimmed = previous.trim();
        let previous_prefix = &previous[..previous.len() - previous.trim_start().len()];
        let separator = if previous_trimmed.ends_with(['*', '&', '^']) {
            ""
        } else {
            " "
        };
        self.output.set_pending_tokens(Some(TokenSpan {
            first: head.start,
            last: span.last,
        }));
        self.adjust_and_publish_line(format!(
            "{previous_prefix}{previous_trimmed}{separator}{}",
            line.trim_start()
        ));
        true
    }

    /// Splits the return type of a function head onto its own line
    /// (`--break-return-type`, `--break-return-type-decl`).
    pub(crate) fn try_publish_split_return_type(
        &mut self,
        line: &str,
        indent: usize,
        exact_indent_spaces: Option<usize>,
    ) -> bool {
        let Some(span) = self.output.pending_tokens() else {
            return false;
        };
        let Some(head) = self.tree.functions.starting_at(span.first).cloned() else {
            return false;
        };
        if !Self::return_type_option_applies(
            &head,
            self.options.break_return_type && !self.options.attach_return_type,
            self.options.break_return_type_decl && !self.options.attach_return_type_decl,
        ) || !self.has_movable_return_type(&head)
            || !span.contains(head.name_start)
        {
            return false;
        }
        let params_open = self.tree.groups.get(head.params).open;
        let (Some(name_offset), Some(params_offset)) = (
            self.tree
                .token_offset_in_line(line, span.first, head.name_start),
            self.tree
                .token_offset_in_line(line, span.first, params_open),
        ) else {
            return false;
        };
        let return_type = line[..name_offset].trim_end().to_string();
        let function_part = format!(
            "{}{}",
            line[name_offset..params_offset].trim_end(),
            &line[params_offset..]
        );
        let return_type_last = self
            .tree
            .previous_code_token(head.name_start)
            .unwrap_or(span.first);
        let return_type_span = TokenSpan {
            first: span.first,
            last: return_type_last,
        };
        let function_span = TokenSpan {
            first: head.name_start,
            last: span.last,
        };
        self.output.set_pending_tokens(Some(return_type_span));
        if let Some(spaces) = exact_indent_spaces {
            self.push_formatted_line_exact(&return_type, indent, spaces);
            self.output.set_pending_tokens(Some(function_span));
            self.push_formatted_line_exact(&function_part, indent, spaces);
        } else {
            self.push_formatted_line(&return_type, indent);
            self.output.set_pending_tokens(Some(function_span));
            self.push_formatted_line(&function_part, indent);
        }
        true
    }
}

pub(crate) fn is_return_type_line(line: &str) -> bool {
    if line.is_empty()
        || line.contains("//")
        || line.contains("/*")
        || line.contains("*/")
        || line.starts_with('#')
    {
        return false;
    }
    let mut angle_depth: i32 = 0;
    for ch in line.chars() {
        match ch {
            '{' | '}' | ';' => return false,
            '<' => angle_depth += 1,
            '>' => angle_depth = (angle_depth - 1).max(0),
            '(' | ')' | '=' | ',' if angle_depth == 0 => return false,
            _ => {}
        }
    }
    if angle_depth != 0 {
        return false;
    }
    line.split(|ch: char| !is_identifier_continue(ch))
        .any(|part| !part.is_empty() && is_type_like_pointer_word(part))
}

pub(crate) fn is_parameter_return_type_prefix(line: &str) -> bool {
    let first_word = line
        .split(|ch: char| !is_identifier_continue(ch))
        .find(|word| !word.is_empty());
    if first_word.is_some_and(|word| {
        is_non_type_keyword(word)
            || matches!(
                word,
                "co_return" | "alignof" | "noexcept" | "typeid" | "requires" | "decltype"
            )
    }) {
        return false;
    }
    is_return_type_line(line)
        || (!line.trim().is_empty()
            && line.chars().all(|ch| {
                ch.is_whitespace()
                    || is_identifier_continue(ch)
                    || matches!(ch, ':' | '<' | '>' | '*' | '&')
            }))
}

fn is_pointer_prefixed_function_part(line: &str) -> bool {
    let rest =
        line.trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '*' | '&' | '^'));
    if rest == line {
        return false;
    }
    let Some(open_paren) = find_outside_quotes(rest, "(") else {
        return false;
    };
    let before = rest[..open_paren].trim_end();
    !before.is_empty()
        && !language::is_header(before)
        && function_name_start(before).is_some_and(|start| start == 0)
}
