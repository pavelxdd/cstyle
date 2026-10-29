use crate::config::FormatOptions;
use crate::formatter::braces::classification::is_lambda_capture_header;
use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::engine::FormatEngine;
use crate::formatter::lexer::Token;
use crate::formatter::state::BraceType;
use crate::formatter::state::frame::{ConstructorInitializerFrame, ConstructorInitializerLayout};
use crate::formatter::structure::blocks::{BlockKind, is_code_token};
use crate::formatter::structure::groups::Delimiter;
use crate::formatter::syntax::{language, scoped_name_is_constructor};
use crate::formatter::text::columns::column_after;
use crate::formatter::text::columns::{leading_visual_width, visual_width_from};
use crate::formatter::text::line_scan::ContainsAnyByte;
use crate::formatter::text::line_scan::{
    after_leading_comments, has_unmatched_open_brace, inline_brace_pair_range,
    is_comment_only_line, line_paren_imbalance, preprocessor_directive,
    trailing_comment_split_limit, unmatched_open_paren_columns,
};
use crate::formatter::text::line_view::LineView;
use crate::formatter::text::trim::Trimmed;
use crate::source::lex::{is_identifier_continue, is_identifier_start};

pub(crate) struct MaxLengthConstructorReplay {
    has_constructor_initializer: bool,
    in_constructor_initializer: bool,
    lambda_call_indent: Option<ContinuationIndent>,
    structural_level: Option<usize>,
}

impl MaxLengthConstructorReplay {
    /// Whether the replayed line holds a constructor initializer list.
    pub(crate) fn has_constructor_initializer(&self) -> bool {
        self.has_constructor_initializer
    }

    fn head_enters_constructor_initializer(&self, head: &str) -> bool {
        self.has_constructor_initializer
            && head.match_indices(')').any(|(close, _)| {
                head[close + 1..]
                    .trimmed_start()
                    .strip_prefix(':')
                    .is_some_and(|tail| !tail.starts_with(':'))
            })
    }

    pub(crate) fn structural_level(&self) -> Option<usize> {
        self.structural_level
    }
}

fn paren_depth_delta(line: &str) -> isize {
    if !line.contains_any_byte(b"()") {
        return 0;
    }
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

pub(crate) fn has_inline_constructor_initializer_colon(line: &str) -> bool {
    // The colon follows a closing paren.
    if !line.contains(')') || !line.contains(':') {
        return false;
    }
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
            && constructor_signature_ends_with_parameter_list(&line[..index])
            && line[index + ch.len_utf8()..].starts_with(' ')
        {
            let before = line[..index].trimmed_start();
            let Some(open) = before.find('(') else {
                return false;
            };
            // Specifiers or a macro may lead the name.
            return before[..open].split_whitespace().all(|word| {
                word.chars()
                    .all(|ch| is_identifier_continue(ch) || matches!(ch, '~' | ':'))
                    && !word.ends_with(':')
                    && !language::is_header(word)
                    && !matches!(word, "return" | "else" | "do" | "goto")
            });
        }
    }
    false
}

fn constructor_signature_ends_with_parameter_list(line: &str) -> bool {
    let mut rest = line.trimmed_end();
    loop {
        if rest.ends_with(')') {
            return true;
        }
        if let Some(stripped) = rest.strip_suffix("&&").or_else(|| rest.strip_suffix('&')) {
            rest = stripped.trimmed_end();
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
            rest = rest[..rest.len() - word.len()].trimmed_end();
            continue;
        }
        return false;
    }
}

impl FormatEngine<'_> {
    /// Where an argument split off an initializer's open paren stands, for
    /// a constructor head at `head_indent`: one indent past the head plus the
    /// continuation indent, or two indents past it when that exceeds the
    /// maximum.
    pub(crate) fn initializer_argument_indent(&self, head_indent: usize) -> usize {
        let width = self.options.indent_width;
        let past_head = width + self.options.continuation_indent * width;
        if past_head > self.options.max_continuation_indent {
            head_indent + width * 2
        } else {
            head_indent + past_head
        }
    }

    pub(crate) fn constructor_initializer_prefix_level(&self, structural_level: usize) -> usize {
        let width = self.options.indent_width.max(1);
        self.layout
            .frame_stack
            .active_constructor_initializer()
            .map_or(structural_level, |frame| {
                structural_level.max(frame.colon_line_indent_spaces / width)
            })
    }

    pub(crate) fn replayed_constructor_lambda_header_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        if self.options.max_code_length.is_none()
            || self
                .layout
                .frame_stack
                .active_constructor_initializer()
                .is_none()
        {
            return None;
        }
        let current = line.trimmed_start();
        let open = current.find('(')?;
        if !is_lambda_capture_header(current[..open].trimmed_end()) {
            return None;
        }
        let previous = self.output.last_line_outside_comment()?;
        let previous = self.output.code_of(previous).trimmed_start();
        let open = self.open_paren_column_of(previous)?;
        let target = leading_visual_width(previous, self.options.tab_width) + open + 1;
        (target == self.token_input.input_source_indent).then_some(target)
    }

    /// Whether the last constructor's initializers may still be read: its
    /// frame stays recorded until its body closes, and the body opens
    /// outside any function.
    pub(crate) fn in_constructor_initializer(&self) -> bool {
        self.layout
            .frame_stack
            .active_constructor_initializer()
            .is_some()
            && !self
                .layout
                .nesting
                .brace_type_stack
                .iter()
                .any(|brace_type| matches!(brace_type, BraceType::Command | BraceType::Definition))
    }

    pub(crate) fn record_constructor_initializer_frame(&mut self, function_try: bool) {
        let layout = if self.current.trimmed().is_empty() {
            ConstructorInitializerLayout::Split
        } else {
            ConstructorInitializerLayout::SameLine
        };
        let colon_line_indent_spaces = if layout == ConstructorInitializerLayout::SameLine
            && self.paren_closes_of(self.current.trimmed_end()) > 0
        {
            self.layout
                .frame_stack
                .line_closed_delimiter_line_indent_spaces()
                .unwrap_or_else(|| self.current_line_indent_spaces())
        } else {
            self.current_line_indent_spaces()
        };
        self.layout
            .frame_stack
            .push_constructor_initializer(ConstructorInitializerFrame {
                colon_line_indent_spaces,
                layout,
                function_try,
            });
    }

    pub(crate) fn output_has_constructor_initializer_colon(&self) -> bool {
        let key = (self.output.len(), self.output.version());
        if let Some((cached, colon)) = self.constructor_colon_cache.get()
            && cached == key
        {
            return colon;
        }
        let colon = self.scan_output_constructor_initializer_colon();
        self.constructor_colon_cache.set(Some((key, colon)));
        colon
    }

    fn scan_output_constructor_initializer_colon(&self) -> bool {
        for index in (0..self.output.len()).rev().take(64) {
            // Comments leading a line take no part in it.
            let trimmed = after_leading_comments(self.output.code_trimmed(index));
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return false;
            }
            if trimmed.ends_with(':') && !trimmed.starts_with_any(b"?:") {
                return trimmed.contains('(');
            }
            if trimmed.starts_with(':') && !trimmed.starts_with("::") {
                return true;
            }
            if trimmed.contains(':')
                && trimmed.match_indices(')').any(|(close, _)| {
                    let Some(tail) = trimmed[close + 1..].trimmed_start().strip_prefix(':') else {
                        return false;
                    };
                    if tail.starts_with(':') {
                        return false;
                    }
                    // The `:` of a ternary follows its `?` outside the just-closed
                    // paren, so a question mark at depth zero marks a ternary colon.
                    let mut depth = 1usize;
                    for ch in trimmed[..close].chars().rev() {
                        match ch {
                            ')' => depth += 1,
                            '(' => depth = depth.saturating_sub(1),
                            '?' if depth == 0 => return false,
                            ';' | '{' | '}' => break,
                            _ => {}
                        }
                    }
                    true
                })
            {
                return true;
            }
        }
        false
    }

    pub(crate) fn same_line_constructor_initializer_base_indent_spaces(&self) -> Option<usize> {
        for index in (0..self.output.len()).rev().take(16) {
            let raw = &self.output[index];
            let trimmed = self.output.code_trimmed(index);
            if trimmed.contains(" : ") && trimmed.contains('(') && !trimmed.starts_with("case ") {
                return Some(
                    leading_visual_width(raw, self.options.tab_width)
                        + self.options.indent_width
                        + self.case_unindent_spaces(),
                );
            }
            if trimmed.ends_with(';') || trimmed.ends_with('{') || trimmed.ends_with('}') {
                return None;
            }
        }
        None
    }

    fn constructor_initializer_frame_base_indent_spaces(&self) -> Option<usize> {
        let frame = self.layout.frame_stack.active_constructor_initializer()?;
        if !self.output_has_constructor_initializer_colon() {
            return None;
        }
        match frame.layout {
            ConstructorInitializerLayout::SameLine => {
                Some(frame.colon_line_indent_spaces + self.options.indent_width)
            }
            ConstructorInitializerLayout::Split => {
                for scan_index in self.output.scoped_range().rev().take(64) {
                    let raw = &self.output[scan_index];
                    let trimmed = self.output.code_body(scan_index);
                    if trimmed.starts_with(':') && !trimmed.starts_with("::") {
                        if trimmed == ":" {
                            return Some(
                                frame.colon_line_indent_spaces
                                    + usize::from(frame.function_try) * self.options.indent_width,
                            );
                        }
                        let spaces_after_colon =
                            trimmed[1..].len() - trimmed[1..].trimmed_start().len();
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

    pub(crate) fn constructor_initializer_base_indent_spaces(&self) -> Option<usize> {
        let scan = self.constructor_initializer_scan();
        match scan {
            ConstructorScan::Colon { index, after_colon } => {
                let leading = leading_visual_width(&self.output[index], self.options.tab_width);
                match after_colon {
                    Some(spaces) => Some(leading + 1 + spaces),
                    None => {
                        let function_try = self
                            .layout
                            .frame_stack
                            .active_constructor_initializer()
                            .is_some_and(|frame| frame.function_try);
                        Some(leading + usize::from(function_try) * self.options.indent_width)
                    }
                }
            }
            ConstructorScan::Head { index } => Some(
                leading_visual_width(&self.output[index], self.options.tab_width)
                    + self.options.indent_width,
            ),
            ConstructorScan::Stop => None,
            ConstructorScan::Exhausted => self.constructor_initializer_frame_base_indent_spaces(),
        }
    }

    /// Looks back from the last output line for the line that starts a
    /// constructor initializer list.
    /// The look back for the line that starts a constructor initializer
    /// list. Lines pushed since the last look back are read first; with no
    /// answer among them, the last answer holds while its line is within
    /// reach, and the look back finds nothing once it is not, as the lines
    /// after it decide nothing.
    fn constructor_initializer_scan(&self) -> ConstructorScan {
        let (len, version) = (self.output.len(), self.output.version());
        let floor = len.saturating_sub(CONSTRUCTOR_SCAN_REACH);
        let cached = self
            .constructor_scan_cache
            .get()
            .filter(|cached| cached.version == version && cached.len <= len);
        let read_from = cached.map_or(floor, |cached| cached.len.max(floor));
        let mut found = (read_from..len)
            .rev()
            .find_map(|index| Some((self.constructor_scan_step(index)?, Some(index))));
        if found.is_none() {
            found = Some(match cached {
                Some(cached) if cached.decided_at.is_some_and(|at| at >= floor) => {
                    (cached.scan, cached.decided_at)
                }
                _ => (ConstructorScan::Exhausted, None),
            });
        }
        let (scan, decided_at) = found.expect("an answer");
        self.constructor_scan_cache.set(Some(ConstructorScanCache {
            len,
            version,
            scan,
            decided_at,
        }));
        scan
    }

    /// What output line `index` decides for the look back, if anything.
    /// Whether a lambda's body closes among the constructor's initializers
    /// before the token at `index`: astyle reads the rows after it as the
    /// constructor's.
    pub(crate) fn lambda_closes_among_initializers_before(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        let mut cursor = index;
        while let Some(before) = self.tree.previous_code_token(cursor) {
            if let Some(group) = groups.closed_at(before) {
                let open = groups.get(group).open;
                if (open..=before).any(|inner| {
                    matches!(tokens[inner], Token::Symbol('}'))
                        && groups.closed_at(inner).is_some_and(|body| {
                            self.tree.blocks.kind(body) == Some(BlockKind::Lambda)
                        })
                }) {
                    return true;
                }
                cursor = open;
                continue;
            }
            if matches!(tokens[before], Token::Symbol(':' | ';' | '{' | '}')) {
                return false;
            }
            cursor = before;
        }
        false
    }

    /// Whether the code of output line `index` ends with the brace of a
    /// braced initializer, which neither opens nor closes a body.
    fn line_ends_initializer_brace(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        self.output
            .line_tokens(index)
            .and_then(|span| {
                (span.first..=span.last)
                    .rev()
                    .find(|&at| is_code_token(&tokens[at]))
            })
            .and_then(|last| self.tree.groups.delimited_by(last))
            .is_some_and(|group| self.tree.blocks.kind(group) == Some(BlockKind::Initializer))
    }

    fn constructor_scan_step(&self, index: usize) -> Option<ConstructorScan> {
        // The body of a block comment holds no code.
        if self.output.comment_start_index(index) != index {
            return None;
        }
        let code = self.output.code_before_comment(index).trimmed_end();
        let trimmed = self.output.code_body(index);
        let braced_initializer = || self.line_ends_initializer_brace(index);
        // A conditional's colon neither starts nor ends the list.
        if trimmed.starts_with(':')
            && !trimmed.starts_with("::")
            && !self.colon_line_is_ternary_arm(index)
        {
            if code.ends_with(';')
                || (code.ends_with('{') || code.ends_with('}')) && !braced_initializer()
            {
                return Some(ConstructorScan::Stop);
            }
            if self.colon_line_stands_in_parens(index) {
                return Some(ConstructorScan::Stop);
            }
            let after_colon =
                (trimmed != ":").then(|| trimmed[1..].len() - trimmed[1..].trimmed_start().len());
            return Some(ConstructorScan::Colon { index, after_colon });
        }
        if trimmed.ends_with(':') && !trimmed.starts_with_any(b"?:") && trimmed.contains('(') {
            if trimmed.contains('?') || self.colon_line_is_ternary_arm(index) {
                return Some(ConstructorScan::Stop);
            }
            return Some(ConstructorScan::Head { index });
        }
        if trimmed.ends_with(';')
            || (trimmed.ends_with('{') || trimmed.ends_with('}')) && !braced_initializer()
        {
            return Some(ConstructorScan::Stop);
        }
        None
    }

    /// Whether output line `index` holds the colon after a function's
    /// parameters, and their qualifiers, that opens its initializers.
    fn line_holds_initializer_colon(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        let groups = &self.tree.groups;
        self.output.line_tokens(index).is_some_and(|span| {
            (span.first..=span.last).any(|colon| {
                matches!(tokens[colon], Token::Symbol(':'))
                    && std::iter::successors(self.tree.previous_code_token(colon), |&before| {
                        matches!(tokens[before], Token::Word(_))
                            .then(|| self.tree.previous_code_token(before))
                            .flatten()
                    })
                    .find(|&before| !matches!(tokens[before], Token::Word(_)))
                    .and_then(|close| groups.closed_at(close))
                    .is_some_and(|params| self.tree.functions.is_parameter_list(params))
            })
        })
    }

    /// Whether the colon leading output line `index` stands inside parens,
    /// as an asm statement's operands do, where no initializer list starts.
    fn colon_line_stands_in_parens(&self, index: usize) -> bool {
        let tokens = &self.tree.tokens;
        self.output
            .line_tokens(index)
            .and_then(|span| (span.first..=span.last).find(|&at| is_code_token(&tokens[at])))
            .filter(|&colon| matches!(tokens[colon], Token::Symbol(':')))
            .and_then(|colon| self.tree.groups.enclosing(colon))
            .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Paren)
    }

    fn colon_line_is_ternary_arm(&self, colon_index: usize) -> bool {
        if let Some(span) = self.output.line_tokens(colon_index)
            && let Some(colon) =
                (span.first..=span.last).find(|&index| is_code_token(&self.tree.tokens[index]))
            && matches!(self.tree.tokens[colon], Token::Symbol(':'))
        {
            let group = self.tree.groups.enclosing(colon);
            return self
                .tree
                .groups
                .members_before(group, colon)
                .take_while(|&index| {
                    !matches!(self.tree.tokens[index], Token::Symbol(';' | '{' | '}'))
                })
                .any(|index| matches!(self.tree.tokens[index], Token::Symbol('?')));
        }
        for index in (0..colon_index).rev() {
            let code = self.output.code_before_comment(index).trimmed_end();
            let trimmed = self.output.code_body(index);
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

    pub(crate) fn constructor_initializer_header_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with(':') || trimmed.starts_with("::") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let code = self.output.code_trimmed_of(previous);
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
        let (closes, opens) = self.paren_imbalance_of(code);
        if !opens.is_empty() || closes != 0 {
            return None;
        }
        let first_word = code
            .trimmed_start()
            .split(|ch: char| !is_identifier_continue(ch))
            .find(|word| !word.is_empty())?;
        if language::is_header(first_word) {
            return None;
        }
        Some(leading_visual_width(previous, self.options.tab_width) + self.options.indent_width)
    }

    pub(crate) fn constructor_initializer_continuation_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#:,{}") {
            return None;
        }
        // Directives between the members leave the member before them.
        let range = self.output.scoped_range();
        let previous_index = self.output.last_line_looked(
            &self.initializer_member_look,
            range.start,
            range.end,
            |index| {
                let head = self.output.trimmed(index);
                !head.is_empty()
                    && !head.starts_with("//")
                    && !head.starts_with('#')
                    && self.output.directive_of_continuation(index).is_none()
            },
        )?;
        let previous = &self.output[previous_index];
        let previous_code = self.output.code_trimmed_of(previous);
        // So do the arguments of a member whose name ends the row before.
        let member_arguments =
            trimmed.starts_with('(') && previous_code.ends_with(is_identifier_continue);
        if !previous_code.ends_with(',')
            && !(previous_code.trimmed() == ":"
                && self.line_holds_initializer_colon(previous_index))
            && !member_arguments
        {
            return None;
        }
        // The constructor's body may open on the last member's row.
        if self
            .open_paren_column_of(previous_code.trimmed_start())
            .is_some()
            || has_unmatched_open_brace(previous_code)
            || has_unmatched_open_brace(trimmed) && !self.pending_line_ends_opening_function_body()
        {
            return None;
        }
        // A head whose initializers run on after its colon stands a level
        // out of them.
        let base_indent = match self.constructor_initializer_base_indent_spaces() {
            Some(base) => base,
            None if member_arguments && self.line_holds_initializer_colon(previous_index) => {
                leading_visual_width(previous, self.options.tab_width) + self.options.indent_width
            }
            None => return None,
        };
        // A row that closes the parens it starts in is still an argument.
        let starts_in_parens = match self.output.pending_tokens() {
            Some(span) => self
                .tree
                .groups
                .enclosing(span.first)
                .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Paren),
            None => self.layout.nesting.paren_depth > 0,
        };
        if starts_in_parens
            && previous_code.ends_with(',')
            && self.paren_closes_of(previous_code) == 0
        {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        Some(base_indent)
    }

    /// Whether the line being laid out ends with the brace of a function's
    /// body.
    fn pending_line_ends_opening_function_body(&self) -> bool {
        self.output.pending_tokens().is_some_and(|span| {
            matches!(self.tree.tokens[span.last], Token::Symbol('{'))
                && self.tree.groups.opened_at(span.last).is_some_and(|group| {
                    self.tree.blocks.kind(group) == Some(BlockKind::FunctionBody)
                })
        })
    }

    pub(crate) fn constructor_initializer_preprocessor_branch_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || !trimmed.starts_with(',') || self.layout.nesting.paren_depth > 0 {
            return None;
        }
        if !self
            .output
            .last_non_empty_scoped()?
            .trimmed_start()
            .starts_with('#')
        {
            return None;
        }
        let mut saw_member = false;
        for scan_index in self.output.scoped_range().rev().skip(1).take(64) {
            let raw = &self.output[scan_index];
            let previous = self.output.code_body(scan_index);
            if previous.is_empty() || previous.starts_with('#') {
                continue;
            }
            if previous.starts_with(':')
                && !previous.starts_with("::")
                && !previous.ends_with(';')
                && !previous.contains_any_byte(b"{}")
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

    pub(crate) fn constructor_initializer_open_paren_arg_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#:,{})") {
            return None;
        }
        // The look back reads at most 64 lines in scope, past blank lines
        // and the bodies of block comments.
        let range = self.output.scoped_range();
        let floor = range.start.max(range.end.saturating_sub(64));
        let lines = self.open_paren_arg_lines(floor);
        let comma = lines.comma.filter(|&comma| comma >= floor);
        let hard = lines.hard.filter(|&(hard, _)| hard >= floor);
        // A comma line nearer than any other decides when the list has a
        // base; one that also stops is the nearest hard line too.
        let decided = match (comma, hard) {
            (Some(comma), hard)
                if hard.is_none_or(|(hard, _)| comma >= hard)
                    && self.constructor_initializer_base_indent_spaces().is_some() =>
            {
                Some(comma)
            }
            (_, Some((hard, true))) => Some(hard),
            _ => None,
        }?;
        // Parens a later line leaves open hold the row as any call's, and
        // one closing the parens the decided line leaves open ends them.
        let (_, decided_opens) = line_paren_imbalance(self.output.code_before_comment(decided));
        let mut decided_open = decided_opens.len();
        let mut depth = 0usize;
        for index in decided + 1..self.output.len() {
            let (closes, opens) = line_paren_imbalance(self.output.code_before_comment(index));
            let closes_later = closes.min(depth);
            decided_open = decided_open.saturating_sub(closes - closes_later);
            depth = depth - closes_later + opens.len();
        }
        if depth > 0 || !decided_opens.is_empty() && decided_open == 0 {
            return None;
        }
        let code = self.output.code_before_comment_trimmed(decided);
        self.arguments_after_open_paren_spaces(code)
    }

    /// Whether the last line in scope starts inside parens an earlier line
    /// opened: an argument row, not an initializer's.
    fn last_line_starts_in_parens(&self) -> bool {
        self.output
            .last_non_empty_index()
            .and_then(|index| self.output.line_tokens(index))
            .and_then(|span| self.tree.groups.enclosing(span.first))
            .is_some_and(|group| self.tree.groups.get(group).delimiter == Delimiter::Paren)
    }

    /// Where the arguments of the call the line `code` leaves open stand:
    /// past the last paren its text follows, a level further for a paren
    /// nested in it that ends the line.
    fn arguments_after_open_paren_spaces(&self, code: &str) -> Option<usize> {
        let open = self.open_paren_column_of(code)?;
        let spaces_after_open = code[open + 1..].len() - code[open + 1..].trimmed_start().len();
        let nested_at_end = code.ends_with('(') && open + 1 < code.len();
        Some(
            visual_width_from(&code[..open + 1], 0, self.options.tab_width)
                + spaces_after_open
                + usize::from(nested_at_end) * self.options.indent_width,
        )
    }

    /// The nearest lines from `floor` on that decide the look back of
    /// [`Self::constructor_initializer_open_paren_arg_indent_spaces`], read
    /// on from the last answer as lines come.
    fn open_paren_arg_lines(&self, floor: usize) -> OpenParenArgLines {
        let (len, version) = (self.output.len(), self.output.version());
        let cached = self.open_paren_arg_cache.get().filter(|cached| {
            cached.version == version && cached.len <= len && cached.from <= floor
        });
        let (mut lines, read_from) = match cached {
            Some(cached) => (cached, cached.len),
            None => (
                OpenParenArgLines {
                    len,
                    version,
                    from: floor,
                    hard: None,
                    comma: None,
                },
                floor,
            ),
        };
        for index in read_from..len {
            match self.open_paren_arg_line(index) {
                OpenParenArgLine::Pass => {}
                OpenParenArgLine::Colon => lines.hard = Some((index, true)),
                OpenParenArgLine::Comma { stops } => {
                    lines.comma = Some(index);
                    if stops {
                        lines.hard = Some((index, false));
                    }
                }
                OpenParenArgLine::Stop => lines.hard = Some((index, false)),
            }
        }
        lines.len = len;
        self.open_paren_arg_cache.set(Some(lines));
        lines
    }

    /// How output line `index` reads to the look back of
    /// [`Self::constructor_initializer_open_paren_arg_indent_spaces`].
    fn open_paren_arg_line(&self, index: usize) -> OpenParenArgLine {
        if self.output.comment_start_index(index) != index || self.output.trimmed(index).is_empty()
        {
            return OpenParenArgLine::Pass;
        }
        let code = self.output.code_before_comment_trimmed(index);
        let body = self.output.code_body(index);
        // A member a leading comma starts opens its arguments as the first
        // one does.
        let colon = body.starts_with(':') && !body.starts_with("::") || body.starts_with(',');
        let comma = body.ends_with(',');
        let opens = (colon || comma) && self.open_paren_column_of(code).is_some();
        let stops = body.starts_with(')')
            || body.ends_with(';')
            || body.ends_with('{')
            || body.ends_with('}');
        if colon && opens {
            OpenParenArgLine::Colon
        } else if comma && opens {
            OpenParenArgLine::Comma { stops }
        } else if stops {
            OpenParenArgLine::Stop
        } else {
            OpenParenArgLine::Pass
        }
    }

    pub(crate) fn constructor_initializer_argument_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if trimmed.is_empty() || trimmed.starts_with_any(b"#:,{}") {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if !previous_code.ends_with('(') || self.last_line_starts_in_parens() {
            return None;
        }
        let base = self.constructor_initializer_base_indent_spaces()?;
        Some(
            self.arguments_after_open_paren_spaces(previous_code)
                .unwrap_or(base + self.options.indent_width),
        )
    }

    pub(crate) fn constructor_initializer_closing_paren_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with(')')
            || !(self.constructor_initializer_base_indent_spaces().is_some()
                || self.output_has_constructor_initializer_colon())
        {
            return None;
        }
        let mut pending = trimmed.chars().take_while(|ch| *ch == ')').count();
        for scan_index in self.output.scoped_range().rev().take(128) {
            let raw = &self.output[scan_index];
            let code = self.output.code_before_comment(scan_index).trimmed_end();
            if code.trimmed().is_empty() {
                continue;
            }
            for (index, ch) in code.char_indices().rev() {
                match ch {
                    ')' => pending += 1,
                    '(' => {
                        pending = pending.saturating_sub(1);
                        if pending == 0 {
                            return Some(if code[..index].trimmed().is_empty() {
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

    pub(crate) fn constructor_initializer_ternary_arm_indent_spaces(
        &self,
        line: &LineView<'_>,
    ) -> Option<usize> {
        let trimmed = line.trimmed_start();
        if !trimmed.starts_with_any(b"?:")
            || self.constructor_initializer_base_indent_spaces().is_none()
        {
            return None;
        }
        let previous = self.output.last_non_empty_scoped()?;
        let previous_code = self.output.code_trimmed_of(previous);
        if trimmed.starts_with(':') && previous_code.trimmed_start().starts_with('?') {
            return Some(leading_visual_width(previous, self.options.tab_width));
        }
        if trimmed.starts_with('?')
            && let Some(open) = self.open_paren_column_of(previous_code)
        {
            // Indenting after parens sets the arm a continuation past its
            // paren's line.
            return Some(if self.options.indent_after_parens {
                leading_visual_width(previous, self.options.tab_width)
                    + self.options.continuation_indent * self.options.indent_width
            } else {
                column_after(previous_code, open, self.options.tab_width)
            });
        }
        None
    }

    pub(crate) fn constructor_initializer_context_indent(
        &self,
        current: &str,
        natural: usize,
    ) -> Option<usize> {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        if current.is_empty() || current.starts_with_any(b"#{}") {
            return None;
        }
        let mut initializer = None;
        let len = self.output.len();
        let floor = len.saturating_sub(32);
        // A macro's body continues nothing after it.
        let after_macro = len
            .checked_sub(1)
            .is_none_or(|last| !self.output.trimmed(last).ends_with('\\'));
        let stops = |index: usize| {
            let trimmed = self.output.code_trimmed(index);
            trimmed == "{"
                || trimmed == "}"
                || trimmed.ends_with(';')
                || after_macro && preprocessor_directive(trimmed) == Some("define")
                || after_macro
                    && self
                        .output
                        .directive_of_continuation(index)
                        .is_some_and(|directive| {
                            preprocessor_directive(self.output.trimmed(directive)) == Some("define")
                        })
                || trimmed.starts_with("*/")
                || trimmed.ends_with("*/")
                || self.output.code_has(index, b'{')
                || self.output.code_has(index, b'}')
        };
        let colon_line = |index: usize| {
            let trimmed = self.output.code_trimmed(index);
            self.output.code_has(index, b':')
                && (trimmed.starts_with(':') && !trimmed.starts_with("::")
                    || self.output.code_has(index, b')')
                        && has_inline_constructor_initializer_colon(self.output.code(index)))
        };
        // The look back passes the lines that neither stop it nor hold a
        // colon that may start an initializer.
        let mut end = len;
        while let Some(index) = if end == len {
            self.output
                .last_line_looked(&self.constructor_context_look, floor, len, |index| {
                    stops(index) || colon_line(index)
                })
        } else {
            (floor..end)
                .rev()
                .find(|&index| stops(index) || colon_line(index))
        } {
            end = index;
            if stops(index) {
                break;
            }
            let code = self.output.code(index);
            let trimmed = self.output.code_trimmed(index);
            let colon_start = trimmed.starts_with(':') && !trimmed.starts_with("::");
            let inline_colon =
                self.output.code_has(index, b')') && has_inline_constructor_initializer_colon(code);
            let offset = len - 1 - index;
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
            if (starts_initializer || inline_initializer) && self.output.code_has(index, b'(') {
                let member_indent = if starts_initializer {
                    let leading = self.output.lead_width(index, tab_width);
                    if trimmed == ":" {
                        leading
                    } else {
                        let spaces_after_colon =
                            trimmed[1..].len() - trimmed[1..].trimmed_start().len();
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
                    Some(self.initializer_argument_indent(self.output.lead_width(index, tab_width)))
                } else {
                    None
                };
                initializer = Some((member_indent, arg_indent, index));
                break;
            }
        }
        let (member_indent, arg_indent, initializer_line) = initializer?;
        // Parens the lines from the initializer's on leave open.
        let open_depth: isize = (initializer_line..self.output.len())
            .map(|index| paren_depth_delta(self.output.code(index)))
            .sum();
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

    pub(crate) fn split_constructor_member_call_indent(&self, current: &str) -> Option<usize> {
        let width = self.options.indent_width;
        let tab_width = self.options.tab_width;
        if current.is_empty()
            || current.starts_with('#')
            || current == "{"
            || current == "}"
            || current.starts_with("};")
            // A member's call splits at a `(` alone on its line.
            || current != "("
                && !self
                    .output
                    .has_open_paren_line_from(self.output.len().saturating_sub(32))
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

    pub(crate) fn constructor_member_line_base_indent_spaces(&self) -> Option<usize> {
        self.layout.frame_stack.active_constructor_initializer()?;
        self.current
            .trimmed_start()
            .chars()
            .next()
            .filter(|ch| is_identifier_start(*ch))?;
        self.output
            .scoped()
            .iter()
            .rev()
            .find(|line| {
                let trimmed = line.trimmed_start();
                !trimmed.is_empty() && !is_comment_only_line(trimmed)
            })
            .filter(|line| {
                let code = self.output.code_trimmed_of(line);
                code.ends_with(',') && self.open_paren_column_of(code).is_none()
            })?;
        self.constructor_initializer_base_indent_spaces()
    }
}

/// Where a part split inside the initializers goes: one past the argument
/// the open paren holds, or past the head with no paren open or too deep.
fn initializer_split_indent(
    options: &FormatOptions,
    head: &str,
    base_indent_width: usize,
    arguments_indent: ContinuationIndent,
) -> ContinuationIndent {
    let head_indent = base_indent_width + options.indent_width;
    if unmatched_open_paren_columns(head).is_empty() {
        return ContinuationIndent::Spaces(head_indent);
    }
    let held = arguments_indent.columns(options.indent_width);
    if head.trimmed_end().ends_with('(') {
        return arguments_indent;
    }
    let held = held + options.indent_width;
    if held.saturating_sub(base_indent_width) <= options.max_continuation_indent {
        ContinuationIndent::Spaces(held)
    } else {
        arguments_indent
    }
}

pub(crate) fn start_max_length_constructor_replay(
    options: &FormatOptions,
    line: &str,
    head: &str,
    tail: &str,
    base_indent_width: usize,
    structural_level: usize,
    mut next_indent: ContinuationIndent,
) -> (MaxLengthConstructorReplay, ContinuationIndent) {
    let has_constructor_initializer = line.find('(').is_some_and(|open| {
        scoped_name_is_constructor(line[..open].trimmed_end()) && line[open + 1..].contains(':')
    });
    let mut replay = MaxLengthConstructorReplay {
        has_constructor_initializer,
        in_constructor_initializer: false,
        lambda_call_indent: None,
        structural_level: None,
    };
    replay.in_constructor_initializer = replay.head_enters_constructor_initializer(head);
    let split_ends_lambda_parameter_opener = head
        .trimmed_end()
        .strip_suffix('(')
        .is_some_and(|head| is_lambda_capture_header(head.trimmed_end()));
    let tail_starts_lambda_capture = tail
        .trimmed_start()
        .find('(')
        .is_some_and(|open| is_lambda_capture_header(tail.trimmed_start()[..open].trimmed_end()));
    let constructor_lambda_tail = replay.in_constructor_initializer && tail_starts_lambda_capture;
    if constructor_lambda_tail && let Some(open) = unmatched_open_paren_columns(head).last() {
        next_indent = ContinuationIndent::Spaces(base_indent_width + open + 1);
    } else if replay.in_constructor_initializer && !split_ends_lambda_parameter_opener {
        next_indent = initializer_split_indent(options, head, base_indent_width, next_indent);
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

pub(crate) fn advance_max_length_constructor_replay(
    options: &FormatOptions,
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
        following_indent = ContinuationIndent::Spaces(base_indent_width + options.indent_width);
    } else if replay.in_constructor_initializer
        && let Some(target) = unmatched_open_paren_columns(head)
            .into_iter()
            .rev()
            .map(|open| next_indent.columns(options.indent_width) + open + 1)
            .find(|target| {
                target.saturating_sub(base_indent_width) <= options.max_continuation_indent
            })
    {
        following_indent = ContinuationIndent::Spaces(target);
    } else if following_indent.columns(options.indent_width)
        < next_indent.columns(options.indent_width)
    {
        following_indent = next_indent;
    }
    replay.in_constructor_initializer |= enters_constructor_initializer;
    following_indent
}

pub(crate) fn constructor_initializer_name_indent_from_line(
    options: &FormatOptions,
    line: &str,
) -> Option<usize> {
    let code = line[..trailing_comment_split_limit(line)].trimmed_end();
    let leading = leading_visual_width(code, options.tab_width);
    let trimmed = code.trimmed_start();
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

/// How many output lines the look back for a constructor initializer list
/// reads.
const CONSTRUCTOR_SCAN_REACH: usize = 64;

/// How a line reads to the look back for the open paren of a constructor
/// initializer's argument.
#[derive(Clone, Copy)]
enum OpenParenArgLine {
    /// A line the look back passes.
    Pass,
    /// A line the list's `:` leads that leaves a paren open.
    Colon,
    /// A line ending with `,` that leaves a paren open; it decides when the
    /// list has a base and otherwise passes, or stops the look back when
    /// `stops`.
    Comma { stops: bool },
    /// A line that stops the look back.
    Stop,
}

/// The nearest lines that decide the look back for the open paren of a
/// constructor initializer's argument, read from line `from` up to `len`
/// at output `version`: the nearest `Colon` or stopping line, with whether
/// it is a `Colon`, and the nearest `Comma` line.
#[derive(Clone, Copy)]
pub(crate) struct OpenParenArgLines {
    len: usize,
    version: u64,
    from: usize,
    hard: Option<(usize, bool)>,
    comma: Option<usize>,
}

/// The last look back for a constructor initializer list: the line count
/// and version it read, its answer, and the line that decided it.
#[derive(Clone, Copy)]
pub(crate) struct ConstructorScanCache {
    len: usize,
    version: u64,
    scan: ConstructorScan,
    decided_at: Option<usize>,
}

/// What the look back for a constructor initializer list found.
#[derive(Clone, Copy)]
pub(crate) enum ConstructorScan {
    /// A line the list's `:` leads, with the spaces after a colon that code
    /// follows.
    Colon {
        index: usize,
        after_colon: Option<usize>,
    },
    /// A constructor head ending in the list's `:`.
    Head { index: usize },
    /// A line that ends the search empty-handed.
    Stop,
    /// No line decided within reach.
    Exhausted,
}
