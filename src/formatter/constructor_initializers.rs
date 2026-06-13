use super::brace_classification::is_lambda_capture_header;
use super::columns::{leading_visual_width, visual_width_from};
use super::frame::{ConstructorInitializerFrame, ConstructorInitializerLayout};
use super::language;
use super::line_scan::{
    has_unmatched_open_brace, inline_brace_pair_range, unmatched_open_paren_columns,
};
use super::line_scan::{is_comment_only_line, line_paren_imbalance};
use super::syntax::scoped_name_is_constructor;
use super::{
    ContinuationIndent, FormatEngine, trailing_comment_split_limit, unmatched_open_paren_column,
};
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(super) struct MaxLengthConstructorReplay {
    has_constructor_initializer: bool,
    in_constructor_initializer: bool,
    lambda_call_indent: Option<ContinuationIndent>,
    structural_level: Option<usize>,
}

impl MaxLengthConstructorReplay {
    fn head_enters_constructor_initializer(&self, head: &str) -> bool {
        self.has_constructor_initializer
            && head.match_indices(')').any(|(close, _)| {
                head[close + 1..]
                    .trim_start()
                    .strip_prefix(':')
                    .is_some_and(|tail| !tail.starts_with(':'))
            })
    }

    pub(super) fn structural_level(&self) -> Option<usize> {
        self.structural_level
    }
}

fn paren_depth_delta(line: &str) -> isize {
    let mut depth = 0;
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && (in_string || in_char) {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_char {
            in_string = !in_string;
            continue;
        }
        if ch == '\'' && !in_string {
            in_char = !in_char;
            continue;
        }
        if in_string || in_char {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
    }
    depth
}

pub(super) fn has_inline_constructor_initializer_colon(line: &str) -> bool {
    let mut in_string = false;
    let mut in_char = false;
    let mut escaped = false;
    let mut saw_question = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && (in_string || in_char) {
            escaped = true;
            continue;
        }
        if ch == '"' && !in_char {
            in_string = !in_string;
            continue;
        }
        if ch == '\'' && !in_string {
            in_char = !in_char;
            continue;
        }
        if in_string || in_char {
            continue;
        }
        if ch == '?' {
            saw_question = true;
            continue;
        }
        if ch == ':'
            && !saw_question
            && line[..index].trim_end().ends_with(')')
            && line[index + ch.len_utf8()..].starts_with(' ')
        {
            let before = line[..index].trim_start();
            let open = before.find('(').unwrap_or(usize::MAX);
            let first_space = before.find(char::is_whitespace).unwrap_or(usize::MAX);
            return open < first_space;
        }
    }
    false
}

fn constructor_signature_ends_with_parameter_list(line: &str) -> bool {
    let mut rest = line.trim_end();
    loop {
        if rest.ends_with(')') {
            return true;
        }
        if let Some(stripped) = rest.strip_suffix("&&").or_else(|| rest.strip_suffix('&')) {
            rest = stripped.trim_end();
            continue;
        }
        let word = rest
            .rsplit(|ch: char| !is_identifier_continue(ch))
            .next()
            .unwrap_or_default();
        if matches!(
            word,
            "const" | "volatile" | "noexcept" | "override" | "final" | "mutable"
        ) {
            rest = rest[..rest.len() - word.len()].trim_end();
            continue;
        }
        return false;
    }
}

impl FormatEngine<'_> {
    pub(super) fn constructor_initializer_prefix_level(&self, structural_level: usize) -> usize {
        let width = self.options.indent_width.max(1);
        self.frame_stack
            .active_constructor_initializer()
            .map_or(structural_level, |frame| {
                structural_level.max(frame.colon_line_indent_spaces / width)
            })
    }

    pub(super) fn replayed_constructor_lambda_header_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        if self.options.max_code_length.is_none()
            || self.frame_stack.active_constructor_initializer().is_none()
        {
            return None;
        }
        let current = line.trim_start();
        let open = current.find('(')?;
        if !is_lambda_capture_header(current[..open].trim_end()) {
            return None;
        }
        let previous = self.output.last_non_empty_line()?;
        let previous = previous[..trailing_comment_split_limit(previous)].trim_start();
        let open = unmatched_open_paren_column(previous)?;
        let target = leading_visual_width(previous, self.options.tab_width) + open + 1;
        (target == self.token_input.input_source_indent).then_some(target)
    }

    pub(super) fn start_max_length_constructor_replay(
        &self,
        line: &str,
        head: &str,
        tail: &str,
        base_indent_width: usize,
        structural_level: usize,
        mut next_indent: ContinuationIndent,
    ) -> (MaxLengthConstructorReplay, ContinuationIndent) {
        let has_constructor_initializer = line.find('(').is_some_and(|open| {
            scoped_name_is_constructor(line[..open].trim_end()) && line[open + 1..].contains(':')
        });
        let mut replay = MaxLengthConstructorReplay {
            has_constructor_initializer,
            in_constructor_initializer: false,
            lambda_call_indent: None,
            structural_level: None,
        };
        replay.in_constructor_initializer = replay.head_enters_constructor_initializer(head);
        let split_ends_lambda_parameter_opener = head
            .trim_end()
            .strip_suffix('(')
            .is_some_and(|head| is_lambda_capture_header(head.trim_end()));
        let tail_starts_lambda_capture = tail
            .trim_start()
            .find('(')
            .is_some_and(|open| is_lambda_capture_header(tail.trim_start()[..open].trim_end()));
        let constructor_lambda_tail =
            replay.in_constructor_initializer && tail_starts_lambda_capture;
        if constructor_lambda_tail && let Some(open) = unmatched_open_paren_columns(head).last() {
            next_indent = ContinuationIndent::Spaces(base_indent_width + open + 1);
        } else if replay.in_constructor_initializer && !split_ends_lambda_parameter_opener {
            next_indent = ContinuationIndent::Spaces(base_indent_width + self.options.indent_width);
        }
        replay.lambda_call_indent = if constructor_lambda_tail {
            Some(next_indent)
        } else if replay.in_constructor_initializer && split_ends_lambda_parameter_opener {
            unmatched_open_paren_columns(head)
                .into_iter()
                .rev()
                .nth(1)
                .map(|open| ContinuationIndent::Spaces(base_indent_width + open + 1))
        } else {
            None
        };
        if replay.in_constructor_initializer
            && (split_ends_lambda_parameter_opener || constructor_lambda_tail)
        {
            replay.structural_level = Some(structural_level.max(1));
        }
        (replay, next_indent)
    }

    pub(super) fn advance_max_length_constructor_replay(
        &self,
        replay: &mut MaxLengthConstructorReplay,
        head: &str,
        base_indent_width: usize,
        next_indent: ContinuationIndent,
        mut following_indent: ContinuationIndent,
    ) -> ContinuationIndent {
        let enters_constructor_initializer = replay.head_enters_constructor_initializer(head);
        if replay.in_constructor_initializer
            && inline_brace_pair_range(head).is_some()
            && let Some(owner) = replay.lambda_call_indent
        {
            following_indent = owner;
        } else if enters_constructor_initializer {
            following_indent =
                ContinuationIndent::Spaces(base_indent_width + self.options.indent_width);
        } else if replay.in_constructor_initializer
            && let Some(target) = unmatched_open_paren_columns(head)
                .into_iter()
                .rev()
                .map(|open| next_indent.columns(self.options.indent_width) + open + 1)
                .find(|target| {
                    target.saturating_sub(base_indent_width) <= self.options.max_continuation_indent
                })
        {
            following_indent = ContinuationIndent::Spaces(target);
        } else if following_indent.columns(self.options.indent_width)
            < next_indent.columns(self.options.indent_width)
        {
            following_indent = next_indent;
        }
        replay.in_constructor_initializer |= enters_constructor_initializer;
        following_indent
    }

    pub(super) fn record_constructor_initializer_frame(&mut self, function_try: bool) {
        let layout = if self.current.trim().is_empty() {
            ConstructorInitializerLayout::Split
        } else {
            ConstructorInitializerLayout::SameLine
        };
        let colon_line_indent_spaces = if layout == ConstructorInitializerLayout::SameLine
            && line_paren_imbalance(self.current.trim_end()).0 > 0
        {
            self.frame_stack
                .line_closed_delimiter_line_indent_spaces()
                .unwrap_or_else(|| self.current_line_indent_spaces())
        } else {
            self.current_line_indent_spaces()
        };
        self.frame_stack
            .push_constructor_initializer(ConstructorInitializerFrame {
                colon_line_indent_spaces,
                layout,
                function_try,
            });
    }

    pub(super) fn output_has_constructor_initializer_colon(&self) -> bool {
        for index in (0..self.output.len()).rev().take(64) {
            let trimmed = self.output.code_trimmed(index);
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
            if trimmed.ends_with(':') && !trimmed.starts_with(['?', ':']) {
                return trimmed.contains('(');
            }
            if trimmed.starts_with(':') && !trimmed.starts_with("::") {
                return true;
            }
            if trimmed.match_indices(')').any(|(close, _)| {
                trimmed[close + 1..]
                    .trim_start()
                    .strip_prefix(':')
                    .is_some_and(|tail| !tail.starts_with(':'))
            }) {
                return true;
            }
        }
        false
    }

    pub(super) fn same_line_constructor_initializer_base_indent_spaces(&self) -> Option<usize> {
        for index in (0..self.output.len()).rev().take(16) {
            let raw = &self.output[index];
            let trimmed = self.output.code_trimmed(index);
            if trimmed.contains(" : ") && trimmed.contains('(') && !trimmed.starts_with("case ") {
                return Some(
                    leading_visual_width(raw, self.options.tab_width) + self.options.indent_width,
                );
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
        }
        None
    }

    fn constructor_initializer_frame_base_indent_spaces(&self) -> Option<usize> {
        let frame = self.frame_stack.active_constructor_initializer()?;
        if !self.output_has_constructor_initializer_colon() {
            return None;
        }
        match frame.layout {
            ConstructorInitializerLayout::SameLine => {
                Some(frame.colon_line_indent_spaces + self.options.indent_width)
            }
            ConstructorInitializerLayout::Split => {
                for raw in self.output.iter().rev().take(64) {
                    let code = raw[..trailing_comment_split_limit(raw)].trim_end();
                    let trimmed = code.trim_start();
                    if trimmed.starts_with(':') && !trimmed.starts_with("::") {
                        if trimmed == ":" {
                            return Some(
                                frame.colon_line_indent_spaces
                                    + usize::from(frame.function_try) * self.options.indent_width,
                            );
                        }
                        let spaces_after_colon =
                            trimmed[1..].len() - trimmed[1..].trim_start().len();
                        return Some(
                            leading_visual_width(raw, self.options.tab_width)
                                + 1
                                + spaces_after_colon,
                        );
                    }
                    if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                        break;
                    }
                }
                Some(frame.colon_line_indent_spaces)
            }
        }
    }

    pub(super) fn constructor_initializer_base_indent_spaces(&self) -> Option<usize> {
        let total = self.output.len();
        for offset in 0..total.min(64) {
            let index = total - 1 - offset;
            let raw = &self.output[index];
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let trimmed = code.trim_start();
            if trimmed.starts_with(':') && !trimmed.starts_with("::") {
                if code.ends_with('{') || code.ends_with('}') {
                    return None;
                }
                if self.colon_line_is_ternary_arm(index) {
                    return None;
                }
                let leading = leading_visual_width(raw, self.options.tab_width);
                if trimmed == ":" {
                    let function_try = self
                        .frame_stack
                        .active_constructor_initializer()
                        .is_some_and(|frame| frame.function_try);
                    return Some(leading + usize::from(function_try) * self.options.indent_width);
                }
                let spaces_after_colon = trimmed[1..].len() - trimmed[1..].trim_start().len();
                return Some(leading + 1 + spaces_after_colon);
            }
            if trimmed.ends_with(':') && !trimmed.starts_with(['?', ':']) && trimmed.contains('(') {
                if trimmed.contains('?') || self.colon_line_is_ternary_arm(index) {
                    return None;
                }
                return Some(
                    leading_visual_width(raw, self.options.tab_width) + self.options.indent_width,
                );
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
        }
        self.constructor_initializer_frame_base_indent_spaces()
    }

    fn colon_line_is_ternary_arm(&self, colon_index: usize) -> bool {
        for index in (0..colon_index).rev() {
            let code =
                self.output[index][..trailing_comment_split_limit(&self.output[index])].trim_end();
            let trimmed = code.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed.starts_with('?') || code.ends_with('?') {
                return true;
            }
            if code.ends_with(';') || code.ends_with('{') || code.ends_with('}') {
                return false;
            }
        }
        false
    }

    pub(super) fn constructor_initializer_header_indent_spaces(&self, line: &str) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(':') || trimmed.starts_with("::") {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !constructor_signature_ends_with_parameter_list(code) || code.ends_with(';') {
            return None;
        }
        let mut depth = 0i32;
        let mut pending_question = 0i32;
        let bytes = code.as_bytes();
        for (index, &byte) in bytes.iter().enumerate() {
            match byte {
                b'(' | b'[' => depth += 1,
                b')' | b']' => depth -= 1,
                b'?' if depth == 0 => pending_question += 1,
                b':' if depth == 0
                    && bytes.get(index + 1) != Some(&b':')
                    && (index == 0 || bytes[index - 1] != b':') =>
                {
                    pending_question -= 1;
                }
                _ => {}
            }
        }
        if pending_question > 0 {
            return None;
        }
        let (closes, opens) = line_paren_imbalance(code);
        if !opens.is_empty() || closes != 0 {
            return None;
        }
        let first_word = code
            .trim_start()
            .split(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty())?;
        if language::is_header(first_word) {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(super) fn constructor_initializer_continuation_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', ':', ',', '{', '}']) {
            return None;
        }
        let previous = self.output.iter().rev().find(|line| {
            let trimmed = line.trim_start();
            !trimmed.is_empty() && !trimmed.starts_with("//")
        })?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with(',') && previous_code.trim() != ":" {
            return None;
        }
        if unmatched_open_paren_column(previous_code.trim_start()).is_some()
            || has_unmatched_open_brace(previous_code)
            || has_unmatched_open_brace(trimmed)
        {
            return None;
        }
        let base_indent = self.constructor_initializer_base_indent_spaces()?;
        if self.stack_state.paren_depth > 0
            && previous_code.ends_with(',')
            && line_paren_imbalance(previous_code).0 == 0
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        Some(base_indent)
    }

    pub(super) fn constructor_initializer_preprocessor_branch_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || !trimmed.starts_with(',') || self.stack_state.paren_depth > 0 {
            return None;
        }
        if !self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?
            .trim_start()
            .starts_with('#')
        {
            return None;
        }
        let mut saw_member = false;
        for raw in self.output.iter().rev().skip(1).take(64) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            let previous = code.trim_start();
            if previous.is_empty() || previous.starts_with('#') {
                continue;
            }
            if previous.starts_with(':')
                && !previous.starts_with("::")
                && !previous.ends_with(';')
                && !previous.contains(['{', '}'])
            {
                return Some(leading_visual_width(raw, self.options.tab_width));
            }
            if previous.ends_with(';') || previous.ends_with('{') || previous.ends_with('}') {
                return None;
            }
            if saw_member || previous.starts_with(',') || previous.contains('(') {
                saw_member = true;
                continue;
            }
            return None;
        }
        None
    }

    pub(super) fn constructor_initializer_open_paren_arg_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', ':', ',', '{', '}', ')']) {
            return None;
        }
        let base_indent = self.constructor_initializer_base_indent_spaces();
        for previous in self
            .output
            .iter()
            .rev()
            .take(64)
            .filter(|line| !line.trim().is_empty())
        {
            let code = previous[..trailing_comment_split_limit(previous)].trim_end();
            let previous_trimmed = code.trim_start();
            if ((previous_trimmed.starts_with(':') && !previous_trimmed.starts_with("::"))
                || (base_indent.is_some() && previous_trimmed.ends_with(',')))
                && let Some(open) = unmatched_open_paren_column(code)
            {
                let spaces_after_open =
                    code[open + 1..].len() - code[open + 1..].trim_start().len();
                return Some(
                    visual_width_from(&code[..open + 1], 0, self.options.tab_width)
                        + spaces_after_open,
                );
            }
            if previous_trimmed.starts_with(')')
                || previous_trimmed.ends_with(';')
                || previous_trimmed.ends_with('{')
                || previous_trimmed.ends_with('}')
            {
                return None;
            }
        }
        None
    }

    pub(super) fn constructor_initializer_argument_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with(['#', ':', ',', '{', '}']) {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if !previous_code.ends_with('(') {
            return None;
        }
        self.constructor_initializer_base_indent_spaces()
            .map(|spaces| spaces + self.options.indent_width)
    }

    pub(super) fn constructor_initializer_closing_paren_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(')')
            || !(self.constructor_initializer_base_indent_spaces().is_some()
                || self.output_has_constructor_initializer_colon())
        {
            return None;
        }
        let mut pending = trimmed.chars().take_while(|ch| *ch == ')').count();
        for raw in self.output.iter().rev().take(128) {
            let code = raw[..trailing_comment_split_limit(raw)].trim_end();
            if code.trim().is_empty() {
                continue;
            }
            for (index, ch) in code.char_indices().rev() {
                match ch {
                    ')' => pending += 1,
                    '(' => {
                        pending = pending.saturating_sub(1);
                        if pending == 0 {
                            return Some(if code[..index].trim().is_empty() {
                                leading_visual_width(raw, self.options.tab_width)
                            } else {
                                visual_width_from(&code[..index], 0, self.options.tab_width)
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    pub(super) fn constructor_initializer_ternary_arm_indent_spaces(
        &self,
        line: &str,
    ) -> Option<usize> {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(['?', ':'])
            || self.constructor_initializer_base_indent_spaces().is_none()
        {
            return None;
        }
        let previous = self
            .output
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())?;
        let previous_code = previous[..trailing_comment_split_limit(previous)].trim_end();
        if trimmed.starts_with(':') && previous_code.trim_start().starts_with('?') {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if trimmed.starts_with('?')
            && let Some(open) = unmatched_open_paren_column(previous_code)
        {
            return Some(open + 1);
        }
        None
    }

    pub(super) fn constructor_initializer_context_indent(
        &self,
        current: &str,
        natural: usize,
    ) -> Option<usize> {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        let mut paren_depth = 0isize;
        let mut initializer = None;
        for (offset, index) in (0..self.output.len()).rev().take(32).enumerate() {
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            if trimmed == "{"
                || trimmed == "}"
                || trimmed.ends_with(';')
                || trimmed.starts_with("*/")
                || trimmed.ends_with("*/")
                || code.contains('{')
                || code.contains('}')
            {
                break;
            }
            paren_depth += paren_depth_delta(code);
            let colon_start = trimmed.starts_with(':') && !trimmed.starts_with("::");
            let inline_colon = has_inline_constructor_initializer_colon(code);
            if !colon_start && !inline_colon {
                continue;
            }
            let previous_statement_has_question = (0..self.output.len())
                .rev()
                .skip(offset + 1)
                .take_while(|&index| {
                    let trimmed = self.output.trimmed(index);
                    !trimmed.ends_with(';') && trimmed != "{" && trimmed != "}"
                })
                .any(|index| self.output[index].contains('?'));
            let starts_initializer = colon_start && !previous_statement_has_question;
            let inline_initializer = inline_colon && !previous_statement_has_question;
            if (starts_initializer || inline_initializer) && code.contains('(') {
                let member_indent = if starts_initializer {
                    let leading = self.output.lead_width(index, tab_width);
                    if trimmed == ":" {
                        leading
                    } else {
                        let spaces_after_colon =
                            trimmed[1..].len() - trimmed[1..].trim_start().len();
                        leading + 1 + spaces_after_colon
                    }
                } else {
                    self.output.lead_width(index, tab_width) + width
                };
                let arg_indent = if inline_initializer
                    && code.ends_with(',')
                    && !self.output[index][code.len()..].contains("//")
                {
                    Some(natural + width)
                } else if has_inline_constructor_initializer_colon(code) && code.ends_with('(') {
                    Some(self.output.lead_width(index, tab_width) + width * 2)
                } else {
                    None
                };
                initializer = Some((member_indent, arg_indent, paren_depth));
                break;
            }
        }
        let (member_indent, arg_indent, open_depth) = initializer?;
        if current.is_empty() || current.starts_with(['#', '{', '}']) {
            return None;
        }
        if let Some(arg_indent) = arg_indent
            && open_depth > 0
        {
            return Some(arg_indent);
        }
        if current.chars().next().is_some_and(is_identifier_start) && open_depth <= 0 {
            return Some(member_indent);
        }
        None
    }

    pub(super) fn split_constructor_member_call_indent(&self, current: &str) -> Option<usize> {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        if current.is_empty()
            || current.starts_with('#')
            || current == "{"
            || current == "}"
            || current.starts_with("};")
        {
            return None;
        }
        let lines = &self.output;
        for index in (0..lines.len()).rev().take(32) {
            let member = self.output.trimmed(index);
            if member == "{" || member == "}" || member.ends_with(';') {
                break;
            }
            if member == "("
                && let Some(before_index) = index.checked_sub(1)
            {
                let before_code = self.output.code(before_index);
                if has_inline_constructor_initializer_colon(before_code)
                    && before_code
                        .chars()
                        .last()
                        .is_some_and(is_identifier_continue)
                {
                    let member_indent = self.output.lead_width(before_index, tab_width) + width;
                    return Some(if current.starts_with(')') {
                        member_indent
                    } else {
                        member_indent + width
                    });
                }
            }
            if member.is_empty()
                || member.contains(|ch: char| !(ch == '_' || ch.is_ascii_alphanumeric()))
            {
                continue;
            }
            let next_is_open = (index + 1 < lines.len() && self.output.trimmed(index + 1) == "(")
                || (index + 1 == lines.len() && current == "(");
            if !next_is_open {
                continue;
            }
            let in_initializer = (0..index)
                .rev()
                .take(32)
                .map(|previous| self.output.trimmed(previous))
                .take_while(|trimmed| !trimmed.ends_with(';') && *trimmed != "{" && *trimmed != "}")
                .any(|trimmed| trimmed.starts_with(':') || trimmed.contains(" : "));
            if !in_initializer {
                continue;
            }
            let member_indent = self.output.lead_width(index, tab_width);
            let mut depth = 0usize;
            for line_index in index + 1..lines.len() {
                let trimmed = self.output.trimmed(line_index);
                if trimmed == "(" {
                    depth += 1;
                } else if trimmed.starts_with(')') {
                    depth = depth.saturating_sub(1);
                }
            }
            if depth == 0 && current != "(" {
                continue;
            }
            return Some(if current.starts_with(')') {
                member_indent + width * depth.saturating_sub(1)
            } else if current == "(" {
                member_indent + width * depth
            } else {
                member_indent + width * depth.max(1)
            });
        }
        None
    }

    pub(super) fn constructor_initializer_name_indent_from_line(
        &self,
        line: &str,
    ) -> Option<usize> {
        let code = line[..trailing_comment_split_limit(line)].trim_end();
        let leading = leading_visual_width(code, self.options.tab_width);
        let trimmed = code.trim_start();
        let punctuation = trimmed.chars().next()?;
        if !matches!(punctuation, ':' | ',')
            || (punctuation == ':' && trimmed[punctuation.len_utf8()..].starts_with(':'))
        {
            return None;
        }
        let name_start = trimmed[punctuation.len_utf8()..].find(|ch: char| !ch.is_whitespace())?
            + punctuation.len_utf8();
        Some(leading + name_start)
    }

    pub(super) fn constructor_member_line_base_indent_spaces(&self) -> Option<usize> {
        self.frame_stack.active_constructor_initializer()?;
        self.current
            .trim_start()
            .chars()
            .next()
            .filter(|ch| is_identifier_start(*ch))?;
        self.output
            .iter()
            .rev()
            .find(|line| {
                let trimmed = line.trim_start();
                !trimmed.is_empty() && !is_comment_only_line(trimmed)
            })
            .filter(|line| {
                let code = line[..trailing_comment_split_limit(line)].trim_end();
                code.ends_with(',') && unmatched_open_paren_column(code).is_none()
            })?;
        self.constructor_initializer_base_indent_spaces()
    }
}
