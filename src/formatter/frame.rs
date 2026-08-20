#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) struct FrameId(usize);

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum ParenRole {
    Call,
    SemicolonlessMacroCall,
    Header,
    ObjCTypeGroup,
    CastOrGroup,
}

impl ParenRole {
    pub(super) fn is_call_like(self) -> bool {
        matches!(self, Self::Call | Self::SemicolonlessMacroCall)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum CommaRole {
    CallArgument,
    Declaration,
    InitializerSibling,
    CompoundLiteralArgument,
    Other,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ArgumentFrame {
    pub(super) role: CommaRole,
    pub(super) owner: Option<FrameId>,
    pub(super) index: usize,
    pub(super) sibling_anchor_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum ColonRole {
    Ternary,
    Label,
    ClassInitializer,
    ClassBase,
    EnumUnderlyingType,
    RangeFor,
    BitField,
    ObjCSelector,
    ObjCInterface,
    AsmOperand,
    AlignedContinuation,
    Other,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum TernaryOwnerRole {
    Assignment,
    Return,
    Other,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct TernaryFrame {
    pub(super) owner_role: TernaryOwnerRole,
    pub(super) parent_delimiter: Option<FrameId>,
    pub(super) question_indent_spaces: usize,
    pub(super) branch_anchor_column: Option<usize>,
    pub(super) colon_role: Option<ColonRole>,
    pub(super) colon_output_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum ConstructorInitializerLayout {
    SameLine,
    Split,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ConstructorInitializerFrame {
    pub(super) colon_line_indent_spaces: usize,
    pub(super) layout: ConstructorInitializerLayout,
    pub(super) function_try: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct HeaderFrame {
    pub(super) header: String,
    pub(super) line_indent_spaces: usize,
    pub(super) body_indent_spaces: usize,
    pub(super) parent_delimiter: Option<FrameId>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct BracelessHeaderFrame {
    pub(super) header: String,
    pub(super) header_indent_spaces: usize,
    pub(super) can_match_else: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum CommentFrameKind {
    Line,
    Block,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct CommentFrame {
    pub(super) kind: CommentFrameKind,
    pub(super) output_column: usize,
    pub(super) multiline: bool,
    pub(super) continuation_anchor_column: Option<usize>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct StringContinuationFrame {
    pub(super) output_line: usize,
    pub(super) line_indent_spaces: usize,
    pub(super) literal_start_column: usize,
    pub(super) line_starts_with_chain_operator: bool,
    pub(super) has_opening_context: bool,
    pub(super) has_open_brace_before_literal: bool,
    pub(super) has_stream_context: bool,
    pub(super) inside_delimiter_context: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum PointerRole {
    DeclarationPointer,
    DeclarationReference,
    UnaryOperator,
    BinaryOperator,
    CastTypeGroup,
    FunctionPointer,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct DeclarationFrame {
    pub(super) pointer_role: PointerRole,
    pub(super) continuation_anchor_column: Option<usize>,
    pub(super) closing_anchor_column: Option<usize>,
    pub(super) is_typedef: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct LogicalFrame {
    pub(super) operator: LogicalOperator,
    pub(super) operator_output_column: usize,
    pub(super) operator_output_line: usize,
    pub(super) line_indent_spaces: usize,
    pub(super) operator_starts_output_line: bool,
    pub(super) line_has_positive_paren_delta: bool,
    pub(super) line_ends_with_close_paren: bool,
    pub(super) line_unmatched_open_paren_column: Option<usize>,
    pub(super) return_value_column: Option<usize>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum LogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct StreamFrame {
    pub(super) operator_output_column: usize,
    pub(super) operator_output_line: usize,
    pub(super) line_indent_spaces: usize,
    pub(super) operator_ends_output_line: bool,
    pub(super) line_contains_nested_brace: bool,
    pub(super) line_has_unmatched_open_paren: bool,
    pub(super) line_ends_with_close_paren: bool,
    pub(super) line_has_positive_paren_delta: bool,
    pub(super) chain_anchor_column: usize,
    pub(super) assignment_value_start_column: Option<usize>,
    pub(super) after_multiline_braced_operand: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum BraceSemanticKind {
    Command,
    Definition,
    Array,
    CompoundLiteral,
    Lambda,
    Initializer,
    DeferArray,
    Aggregate,
    Namespace,
    Extern,
    NonStatement,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct BraceFrame {
    pub(super) semantic_kind: BraceSemanticKind,
    pub(super) formatter_type: super::state::FormatterBraceType,
    pub(super) header: Option<String>,
    pub(super) label_block: bool,
    pub(super) case_block: bool,
    pub(super) case_header_pending: bool,
    pub(super) nested_case_label: bool,
    pub(super) class_base: bool,
    pub(super) header_indent_column: usize,
    pub(super) body_indent_column: usize,
    pub(super) sibling_indent_column: usize,
    pub(super) split_header: bool,
    pub(super) close_output_line: Option<usize>,
    pub(super) close_ends_output_line: bool,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct CallFrame {
    pub(super) first_argument_column: Option<usize>,
    pub(super) next_argument_index: usize,
    pub(super) logical_operand_indent_column: usize,
    pub(super) logical_operand_indent_tracks_opener: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub(super) enum BracketRole {
    Other,
    ObjectiveCMessage,
    ObjectiveCCollection,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct BracketFrame {
    pub(super) opener_output_column: usize,
    pub(super) opener_output_line: usize,
    pub(super) line_indent_spaces: usize,
    pub(super) role: BracketRole,
    pub(super) parent_objc_message_align: Option<usize>,
    pub(super) opens_after_selector: bool,
}

impl BracketFrame {
    pub(super) fn objc_continuation_indent_column(&self) -> Option<usize> {
        self.parent_objc_message_align.map(|parent| {
            if self.opens_after_selector {
                parent + 1
            } else {
                self.opener_output_column + 1
            }
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct DelimiterFrame {
    pub(super) role: ParenRole,
    pub(super) lambda_parameter_list: bool,
    pub(super) opener_output_column: usize,
    pub(super) opener_output_line: usize,
    pub(super) line_indent_spaces: usize,
    pub(super) continuation_indent_column: Option<usize>,
    pub(super) call: Option<CallFrame>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct DelimiterEntry {
    id: FrameId,
    frame: DelimiterFrame,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct ClosedDelimiterFrame {
    opener_output_column: usize,
    opener_output_line: usize,
    line_indent_spaces: usize,
}

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct FrameStack {
    next_id: usize,
    delimiters: Vec<DelimiterEntry>,
    brackets: Vec<BracketFrame>,
    last_argument: Option<ArgumentFrame>,
    ternary_frames: Vec<TernaryFrame>,
    ternary_colon_output_lines: Vec<usize>,
    open_ternary_line_ends: Vec<usize>,
    logical_frames: Vec<LogicalFrame>,
    stream_frames: Vec<StreamFrame>,
    constructor_initializer_frame: Option<ConstructorInitializerFrame>,
    header_frame: Option<HeaderFrame>,
    braceless_header_frames: Vec<BracelessHeaderFrame>,
    comment_frame: Option<CommentFrame>,
    declaration_frames: Vec<DeclarationFrame>,
    string_continuation_frames: Vec<StringContinuationFrame>,
    brace_frames: Vec<BraceFrame>,
    closed_delimiter_frames: Vec<ClosedDelimiterFrame>,
    line_closed_delimiter_continuation_indent: Option<usize>,
    line_closed_delimiter_line_indent_spaces: Option<usize>,
    line_closed_call_logical_operand_indent: Option<(usize, usize)>,
    line_closed_lambda_parameter_list: bool,
    line_closed_brackets: Vec<BracketFrame>,
    closed_brace_frames: Vec<BraceFrame>,
}

fn shift_column_for_indent(column: usize, old_indent: usize, new_indent: usize) -> usize {
    if new_indent >= old_indent {
        column + (new_indent - old_indent)
    } else {
        column.saturating_sub(old_indent - new_indent)
    }
}

impl FrameStack {
    pub(super) fn push_delimiter(&mut self, frame: DelimiterFrame) -> FrameId {
        let id = FrameId(self.next_id);
        self.next_id += 1;
        self.delimiters.push(DelimiterEntry { id, frame });
        id
    }

    pub(super) fn pop_delimiter(&mut self, current_output_line: usize) {
        let Some(entry) = self.delimiters.pop() else {
            return;
        };
        self.ternary_frames
            .retain(|frame| frame.parent_delimiter != Some(entry.id));
        if entry.frame.opener_output_line < current_output_line
            && self.line_closed_delimiter_continuation_indent.is_none()
        {
            self.line_closed_delimiter_continuation_indent = entry.frame.continuation_indent_column;
            self.line_closed_delimiter_line_indent_spaces = Some(entry.frame.line_indent_spaces);
        }
        if entry.frame.opener_output_line < current_output_line
            && let Some(call) = entry.frame.call.as_ref()
        {
            self.line_closed_call_logical_operand_indent =
                Some((current_output_line, call.logical_operand_indent_column));
        }
        self.line_closed_lambda_parameter_list |= entry.frame.lambda_parameter_list;
        if !self.stream_frames.is_empty() || !self.string_continuation_frames.is_empty() {
            self.closed_delimiter_frames.push(ClosedDelimiterFrame {
                opener_output_column: entry.frame.opener_output_column,
                opener_output_line: entry.frame.opener_output_line,
                line_indent_spaces: entry.frame.line_indent_spaces,
            });
        }
    }

    pub(super) fn push_bracket(&mut self, frame: BracketFrame) {
        self.brackets.push(frame);
    }

    pub(super) fn pop_bracket(&mut self) {
        if let Some(frame) = self.brackets.pop() {
            self.line_closed_brackets.push(frame);
        }
    }

    pub(super) fn take_line_closed_brackets(&mut self) -> Vec<BracketFrame> {
        std::mem::take(&mut self.line_closed_brackets)
    }

    pub(super) fn active_bracket(&self) -> Option<&BracketFrame> {
        self.brackets.last()
    }

    pub(super) fn bracket_depth(&self) -> usize {
        self.brackets.len()
    }

    pub(super) fn truncate_brackets(&mut self, len: usize) {
        self.brackets.truncate(len);
    }

    pub(super) fn has_objc_alignment_bracket(&self) -> bool {
        self.brackets
            .iter()
            .any(|frame| frame.role != BracketRole::Other)
    }

    pub(super) fn active_delimiter(&self) -> Option<&DelimiterFrame> {
        self.delimiters.last().map(|entry| &entry.frame)
    }

    pub(super) fn active_delimiter_with_id(&self) -> Option<(FrameId, &DelimiterFrame)> {
        self.delimiters.last().map(|entry| (entry.id, &entry.frame))
    }

    pub(super) fn delimiter_by_id(&self, id: FrameId) -> Option<&DelimiterFrame> {
        self.delimiters
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| &entry.frame)
    }

    pub(super) fn active_delimiter_mut(&mut self) -> Option<(FrameId, &mut DelimiterFrame)> {
        self.delimiters
            .last_mut()
            .map(|entry| (entry.id, &mut entry.frame))
    }

    pub(super) fn line_closed_delimiter_line_indent_spaces(&self) -> Option<usize> {
        self.line_closed_delimiter_line_indent_spaces
    }

    pub(super) fn take_line_closed_delimiter_continuation_indent(&mut self) -> Option<usize> {
        self.line_closed_delimiter_line_indent_spaces = None;
        self.line_closed_delimiter_continuation_indent.take()
    }

    pub(super) fn take_line_closed_call_logical_operand_indent(
        &mut self,
        output_line: usize,
    ) -> Option<usize> {
        self.line_closed_call_logical_operand_indent
            .take()
            .and_then(|(line, indent)| (line == output_line).then_some(indent))
    }

    pub(super) fn take_line_closed_lambda_parameter_list(&mut self) -> bool {
        std::mem::take(&mut self.line_closed_lambda_parameter_list)
    }

    fn delimiter_output_positions(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.delimiters
            .iter()
            .map(|entry| {
                (
                    entry.frame.opener_output_line,
                    entry.frame.opener_output_column,
                )
            })
            .chain(
                self.closed_delimiter_frames
                    .iter()
                    .map(|frame| (frame.opener_output_line, frame.opener_output_column)),
            )
    }

    pub(super) fn delimiter_count_after_output_column(&self, line: usize, column: usize) -> usize {
        self.delimiter_output_positions()
            .filter(|&(frame_line, frame_column)| frame_line == line && frame_column > column)
            .count()
    }

    pub(super) fn last_delimiter_column_after_output_column(
        &self,
        line: usize,
        column: usize,
    ) -> Option<usize> {
        self.delimiter_output_positions()
            .filter(|&(frame_line, frame_column)| frame_line == line && frame_column > column)
            .map(|(_, frame_column)| frame_column)
            .max()
    }

    pub(super) fn first_delimiter_column_after_output_column(
        &self,
        line: usize,
        column: usize,
    ) -> Option<usize> {
        self.delimiter_output_positions()
            .filter(|&(frame_line, frame_column)| frame_line == line && frame_column > column)
            .map(|(_, frame_column)| frame_column)
            .min()
    }

    pub(super) fn last_argument(&self) -> Option<&ArgumentFrame> {
        self.last_argument.as_ref()
    }

    pub(super) fn set_last_argument(&mut self, frame: ArgumentFrame) {
        self.last_argument = Some(frame);
    }

    pub(super) fn push_ternary(&mut self, frame: TernaryFrame) {
        self.ternary_frames.push(frame);
    }

    pub(super) fn active_ternary(&self) -> Option<&TernaryFrame> {
        self.ternary_frames
            .iter()
            .rev()
            .find(|frame| frame.colon_role.is_none())
            .or_else(|| self.ternary_frames.last())
    }

    pub(super) fn active_ternary_mut(&mut self) -> Option<&mut TernaryFrame> {
        self.ternary_frames
            .iter_mut()
            .rev()
            .find(|frame| frame.colon_role.is_none())
    }

    pub(super) fn last_ternary_with_colon(&self) -> Option<&TernaryFrame> {
        self.ternary_frames
            .iter()
            .rev()
            .find(|frame| frame.colon_role.is_some())
    }

    pub(super) fn mark_last_ternary_colon_output_line(&mut self, line: usize) {
        if self
            .ternary_colon_output_lines
            .last()
            .is_none_or(|last| *last != line)
        {
            self.ternary_colon_output_lines.push(line);
        }
    }

    pub(super) fn last_ternary_colon_output_line(&self) -> Option<usize> {
        self.ternary_colon_output_lines.last().copied()
    }

    pub(super) fn has_open_ternary(&self) -> bool {
        !self.ternary_frames.is_empty()
    }

    pub(super) fn mark_line_ended_open_ternary(&mut self, line: usize) {
        if self
            .open_ternary_line_ends
            .last()
            .is_none_or(|last| *last != line)
        {
            self.open_ternary_line_ends.push(line);
        }
    }

    pub(super) fn line_ended_open_ternary(&self, line: usize) -> bool {
        self.open_ternary_line_ends.contains(&line)
    }

    pub(super) fn pop_active_ternary(&mut self) {
        if let Some(index) = self
            .ternary_frames
            .iter()
            .rposition(|frame| frame.colon_role.is_none())
        {
            self.ternary_frames.remove(index);
        }
    }

    pub(super) fn pop_completed_ternaries(&mut self) {
        self.ternary_frames
            .retain(|frame| frame.colon_role.is_none());
    }

    pub(super) fn push_logical(&mut self, frame: LogicalFrame) {
        self.logical_frames.push(frame);
    }

    pub(super) fn active_logical_on_output_line(&self, line_index: usize) -> Option<&LogicalFrame> {
        self.logical_frames
            .iter()
            .rev()
            .find(|frame| frame.operator_output_line == line_index)
    }

    pub(super) fn logical_before_output_line_with_return(
        &self,
        line: usize,
    ) -> Option<&LogicalFrame> {
        self.logical_frames
            .iter()
            .rev()
            .find(|frame| frame.operator_output_line < line && frame.return_value_column.is_some())
    }

    pub(super) fn mark_logical_line_context(
        &mut self,
        line: usize,
        unmatched_open_paren_column: Option<usize>,
        ends_with_close_paren: bool,
        has_positive_paren_delta: bool,
    ) {
        for frame in self
            .logical_frames
            .iter_mut()
            .filter(|frame| frame.operator_output_line == line)
        {
            frame.line_unmatched_open_paren_column = unmatched_open_paren_column;
            frame.line_ends_with_close_paren = ends_with_close_paren;
            frame.line_has_positive_paren_delta = has_positive_paren_delta;
        }
    }

    pub(super) fn mark_logical_line_output_indent(&mut self, line: usize, indent_spaces: usize) {
        for frame in self
            .logical_frames
            .iter_mut()
            .filter(|frame| frame.operator_output_line == line)
        {
            frame.operator_output_column = shift_column_for_indent(
                frame.operator_output_column,
                frame.line_indent_spaces,
                indent_spaces,
            );
            if let Some(column) = frame.line_unmatched_open_paren_column {
                frame.line_unmatched_open_paren_column = Some(shift_column_for_indent(
                    column,
                    frame.line_indent_spaces,
                    indent_spaces,
                ));
            }
            if let Some(column) = frame.return_value_column {
                frame.return_value_column = Some(shift_column_for_indent(
                    column,
                    frame.line_indent_spaces,
                    indent_spaces,
                ));
            }
            frame.line_indent_spaces = indent_spaces;
            frame.operator_starts_output_line = frame.operator_output_column == indent_spaces;
        }
    }

    pub(super) fn clear_logical_frames(&mut self) {
        self.logical_frames.clear();
    }

    pub(super) fn push_stream(&mut self, frame: StreamFrame) {
        self.stream_frames.push(frame);
    }

    pub(super) fn push_constructor_initializer(&mut self, frame: ConstructorInitializerFrame) {
        self.constructor_initializer_frame = Some(frame);
    }

    pub(super) fn active_constructor_initializer(&self) -> Option<&ConstructorInitializerFrame> {
        self.constructor_initializer_frame.as_ref()
    }

    pub(super) fn push_header(&mut self, frame: HeaderFrame) {
        self.header_frame = Some(frame);
    }

    pub(super) fn active_header(&self) -> Option<&HeaderFrame> {
        self.header_frame.as_ref()
    }

    pub(super) fn clear_header(&mut self) {
        self.header_frame = None;
    }

    pub(super) fn push_braceless_header(&mut self, frame: BracelessHeaderFrame) {
        self.braceless_header_frames.push(frame);
    }

    pub(super) fn pop_braceless_header(&mut self) {
        self.braceless_header_frames.pop();
    }

    pub(super) fn active_braceless_header(&self) -> Option<&BracelessHeaderFrame> {
        self.braceless_header_frames.last()
    }

    pub(super) fn take_matching_braceless_else_indent(&mut self) -> Option<usize> {
        let index = self
            .braceless_header_frames
            .iter()
            .rposition(|frame| frame.can_match_else)?;
        let indent = self.braceless_header_frames[index].header_indent_spaces;
        self.braceless_header_frames.truncate(index);
        Some(indent)
    }

    pub(super) fn push_comment(&mut self, frame: CommentFrame) {
        self.comment_frame = Some(frame);
    }

    pub(super) fn active_comment(&self) -> Option<&CommentFrame> {
        self.comment_frame.as_ref()
    }

    pub(super) fn active_comment_mut(&mut self) -> Option<&mut CommentFrame> {
        self.comment_frame.as_mut()
    }

    pub(super) fn clear_comments(&mut self) {
        self.comment_frame = None;
    }

    pub(super) fn set_string_continuation(&mut self, frame: StringContinuationFrame) {
        self.string_continuation_frames.push(frame);
    }

    pub(super) fn string_continuation_before_output_line(
        &self,
        line: usize,
    ) -> Option<&StringContinuationFrame> {
        self.string_continuation_frames
            .iter()
            .rev()
            .find(|frame| frame.output_line < line)
    }

    pub(super) fn string_continuation_on_output_line(
        &self,
        line: usize,
    ) -> Option<&StringContinuationFrame> {
        self.string_continuation_frames
            .iter()
            .rev()
            .find(|frame| frame.output_line == line)
    }

    pub(super) fn clear_string_continuations(&mut self) {
        self.string_continuation_frames.clear();
        self.clear_closed_delimiters_if_unused();
    }

    pub(super) fn push_declaration(&mut self, frame: DeclarationFrame) {
        self.declaration_frames.push(frame);
    }

    pub(super) fn active_declaration_mut(&mut self) -> Option<&mut DeclarationFrame> {
        self.declaration_frames.last_mut()
    }

    pub(super) fn active_typedef_function_pointer_declaration(&self) -> Option<&DeclarationFrame> {
        self.declaration_frames
            .iter()
            .rev()
            .find(|frame| frame.pointer_role == PointerRole::FunctionPointer && frame.is_typedef)
    }

    pub(super) fn active_typedef_function_pointer_declaration_mut(
        &mut self,
    ) -> Option<&mut DeclarationFrame> {
        self.declaration_frames
            .iter_mut()
            .rev()
            .find(|frame| frame.pointer_role == PointerRole::FunctionPointer && frame.is_typedef)
    }

    pub(super) fn clear_declarations(&mut self) {
        self.declaration_frames.clear();
    }

    pub(super) fn active_stream(&self) -> Option<&StreamFrame> {
        self.stream_frames.last()
    }

    pub(super) fn active_stream_on_output_line(&self, line_index: usize) -> Option<&StreamFrame> {
        self.stream_frames
            .iter()
            .rev()
            .find(|frame| frame.operator_output_line == line_index)
    }

    pub(super) fn first_stream_on_output_line(&self, line_index: usize) -> Option<&StreamFrame> {
        self.stream_frames
            .iter()
            .find(|frame| frame.operator_output_line == line_index)
    }

    pub(super) fn mark_stream_line_context(
        &mut self,
        line: usize,
        ends_with_stream_operator: bool,
        contains_nested_brace: bool,
        has_unmatched_open_paren: bool,
        ends_with_close_paren: bool,
        has_positive_paren_delta: bool,
    ) {
        for frame in self
            .stream_frames
            .iter_mut()
            .filter(|frame| frame.operator_output_line == line)
        {
            frame.operator_ends_output_line = ends_with_stream_operator;
            frame.line_contains_nested_brace = contains_nested_brace;
            frame.line_has_unmatched_open_paren = has_unmatched_open_paren;
            frame.line_ends_with_close_paren = ends_with_close_paren;
            frame.line_has_positive_paren_delta = has_positive_paren_delta;
        }
    }

    pub(super) fn stream_before_output_line(&self, line: usize) -> Option<&StreamFrame> {
        self.stream_frames
            .iter()
            .rev()
            .find(|frame| frame.operator_output_line < line)
    }

    pub(super) fn stream_before_output_line_with_unmatched_open_paren(
        &self,
        line: usize,
    ) -> Option<&StreamFrame> {
        self.stream_frames
            .iter()
            .rev()
            .find(|frame| frame.operator_output_line < line && frame.line_has_unmatched_open_paren)
    }

    pub(super) fn mark_stream_line_output_indent(&mut self, line: usize, indent_spaces: usize) {
        for frame in self
            .stream_frames
            .iter_mut()
            .filter(|frame| frame.operator_output_line == line)
        {
            let old_operator_column = frame.operator_output_column;
            frame.operator_output_column = shift_column_for_indent(
                frame.operator_output_column,
                frame.line_indent_spaces,
                indent_spaces,
            );
            if frame.chain_anchor_column == old_operator_column {
                frame.chain_anchor_column = frame.operator_output_column;
            }
            if let Some(column) = frame.assignment_value_start_column {
                frame.assignment_value_start_column = Some(shift_column_for_indent(
                    column,
                    frame.line_indent_spaces,
                    indent_spaces,
                ));
            }
            frame.line_indent_spaces = indent_spaces;
        }
    }

    pub(super) fn mark_delimiter_line_output_indent(&mut self, line: usize, indent_spaces: usize) {
        for entry in &mut self.delimiters {
            let frame = &mut entry.frame;
            if frame.opener_output_line == line {
                frame.opener_output_column = shift_column_for_indent(
                    frame.opener_output_column,
                    frame.line_indent_spaces,
                    indent_spaces,
                );
                if let Some(column) = frame.continuation_indent_column.as_mut() {
                    *column =
                        shift_column_for_indent(*column, frame.line_indent_spaces, indent_spaces);
                }
                if let Some(call) = frame.call.as_mut()
                    && call.logical_operand_indent_tracks_opener
                {
                    call.logical_operand_indent_column = shift_column_for_indent(
                        call.logical_operand_indent_column,
                        frame.line_indent_spaces,
                        indent_spaces,
                    );
                }
                frame.line_indent_spaces = indent_spaces;
            }
        }
        for frame in &mut self.closed_delimiter_frames {
            if frame.opener_output_line == line {
                frame.opener_output_column = shift_column_for_indent(
                    frame.opener_output_column,
                    frame.line_indent_spaces,
                    indent_spaces,
                );
                frame.line_indent_spaces = indent_spaces;
            }
        }
        for frame in &mut self.brackets {
            if frame.opener_output_line == line {
                frame.opener_output_column = shift_column_for_indent(
                    frame.opener_output_column,
                    frame.line_indent_spaces,
                    indent_spaces,
                );
                frame.line_indent_spaces = indent_spaces;
            }
        }
    }

    pub(super) fn clear_stream_frames(&mut self) {
        self.stream_frames.clear();
        self.clear_closed_delimiters_if_unused();
    }

    fn clear_closed_delimiters_if_unused(&mut self) {
        if self.stream_frames.is_empty() && self.string_continuation_frames.is_empty() {
            self.closed_delimiter_frames.clear();
        }
    }

    pub(super) fn push_brace(&mut self, frame: BraceFrame) {
        self.closed_brace_frames.clear();
        self.brace_frames.push(frame);
    }

    pub(super) fn clear_closed_braces(&mut self) {
        self.closed_brace_frames.clear();
    }

    pub(super) fn pop_brace(&mut self) {
        let Some(frame) = self.brace_frames.pop() else {
            return;
        };
        if frame.semantic_kind == BraceSemanticKind::Command {
            self.header_frame = None;
        }
        self.closed_brace_frames.push(frame);
    }

    #[cfg(test)]
    pub(super) fn brace_depth(&self) -> usize {
        self.brace_frames.len()
    }

    pub(super) fn active_brace(&self) -> Option<&BraceFrame> {
        self.brace_frames.last()
    }

    pub(super) fn active_brace_mut(&mut self) -> Option<&mut BraceFrame> {
        self.brace_frames.last_mut()
    }

    pub(super) fn enclosing_brace(&self) -> Option<&BraceFrame> {
        self.brace_before_top(1)
    }

    pub(super) fn brace_before_top(&self, skip: usize) -> Option<&BraceFrame> {
        self.brace_frames
            .len()
            .checked_sub(skip + 1)
            .and_then(|index| self.brace_frames.get(index))
    }

    pub(super) fn first_closed_brace(&self) -> Option<&BraceFrame> {
        self.closed_brace_frames.first()
    }

    pub(super) fn last_closed_brace(&self) -> Option<&BraceFrame> {
        self.closed_brace_frames.last()
    }

    pub(super) fn last_closed_brace_mut(&mut self) -> Option<&mut BraceFrame> {
        self.closed_brace_frames.last_mut()
    }

    pub(super) fn mark_last_closed_brace_output_position(&mut self, line: usize) {
        if let Some(frame) = self.last_closed_brace_mut() {
            frame.close_output_line = Some(line);
            frame.close_ends_output_line = false;
        }
    }

    pub(super) fn mark_last_closed_brace_line_end(&mut self, line: usize, ends_with_brace: bool) {
        if let Some(frame) = self.last_closed_brace_mut()
            && frame.close_output_line == Some(line)
        {
            frame.close_ends_output_line = ends_with_brace;
        }
    }

    pub(super) fn enclosing_delimiter(&self) -> Option<&DelimiterFrame> {
        self.delimiters
            .len()
            .checked_sub(2)
            .and_then(|index| self.delimiters.get(index))
            .map(|entry| &entry.frame)
    }
}

#[cfg(test)]
mod tests {
    use super::super::state::FormatterBraceType;
    use super::*;

    fn delimiter_frame() -> DelimiterFrame {
        DelimiterFrame {
            role: ParenRole::CastOrGroup,
            lambda_parameter_list: false,
            opener_output_column: 0,
            opener_output_line: 0,
            line_indent_spaces: 0,
            continuation_indent_column: None,
            call: None,
        }
    }

    #[test]
    fn delimiter_frame_keeps_call_role_and_columns() {
        let mut stack = FrameStack::default();
        let id = stack.push_delimiter(DelimiterFrame {
            role: ParenRole::Call,
            lambda_parameter_list: false,
            opener_output_column: 11,
            opener_output_line: 3,
            line_indent_spaces: 4,
            continuation_indent_column: Some(8),
            call: Some(CallFrame {
                first_argument_column: Some(12),
                next_argument_index: 0,
                logical_operand_indent_column: 8,
                logical_operand_indent_tracks_opener: true,
            }),
        });

        let delimiter = stack.delimiter_by_id(id).expect("delimiter");
        assert_eq!(delimiter.role, ParenRole::Call);
        assert_eq!(delimiter.opener_output_column, 11);
        assert_eq!(
            delimiter
                .call
                .as_ref()
                .and_then(|call| call.first_argument_column),
            Some(12)
        );
        assert_eq!(
            delimiter
                .call
                .as_ref()
                .map(|call| call.logical_operand_indent_column),
            Some(8)
        );
    }

    #[test]
    fn closed_delimiters_without_dependent_frames_are_discarded() {
        let mut stack = FrameStack::default();
        let mut frame = delimiter_frame();
        frame.opener_output_column = 4;
        stack.push_delimiter(frame);

        stack.pop_delimiter(1);

        assert_eq!(stack.delimiter_count_after_output_column(0, 0), 0);
    }

    #[test]
    fn delimiter_column_queries_use_source_order_across_open_and_closed_frames() {
        let mut stack = FrameStack::default();
        let mut first = delimiter_frame();
        first.opener_output_column = 5;
        first.opener_output_line = 2;
        stack.push_delimiter(first);

        let mut second = delimiter_frame();
        second.opener_output_column = 15;
        second.opener_output_line = 2;
        stack.push_delimiter(second);
        stack.set_string_continuation(StringContinuationFrame {
            output_line: 2,
            line_indent_spaces: 0,
            literal_start_column: 20,
            line_starts_with_chain_operator: false,
            has_opening_context: true,
            has_open_brace_before_literal: false,
            has_stream_context: true,
            inside_delimiter_context: true,
        });
        stack.pop_delimiter(3);

        assert_eq!(
            stack.first_delimiter_column_after_output_column(2, 0),
            Some(5)
        );
        assert_eq!(
            stack.last_delimiter_column_after_output_column(2, 0),
            Some(15)
        );

        stack.clear_string_continuations();
        stack.pop_delimiter(3);
        assert_eq!(stack.delimiter_count_after_output_column(2, 0), 0);
    }

    #[test]
    fn enclosing_delimiter_returns_parent_delimiter_frame() {
        let mut stack = FrameStack::default();
        stack.push_delimiter(DelimiterFrame {
            role: ParenRole::Call,
            lambda_parameter_list: false,
            opener_output_column: 4,
            opener_output_line: 1,
            line_indent_spaces: 0,
            continuation_indent_column: None,
            call: None,
        });
        stack.push_delimiter(DelimiterFrame {
            role: ParenRole::CastOrGroup,
            lambda_parameter_list: false,
            opener_output_column: 9,
            opener_output_line: 2,
            line_indent_spaces: 4,
            continuation_indent_column: None,
            call: None,
        });

        let parent = stack.enclosing_delimiter().expect("parent delimiter");
        assert_eq!(parent.role, ParenRole::Call);
        assert_eq!(parent.opener_output_column, 4);
    }

    #[test]
    fn argument_frame_tracks_role_and_anchor() {
        let mut stack = FrameStack::default();
        stack.set_last_argument(ArgumentFrame {
            role: CommaRole::Declaration,
            owner: None,
            index: 1,
            sibling_anchor_column: Some(4),
        });

        let argument = stack.last_argument().expect("argument");
        assert_eq!(argument.role, CommaRole::Declaration);
        assert_eq!(argument.sibling_anchor_column, Some(4));
    }

    #[test]
    fn ternary_frame_tracks_question_and_colon_anchors() {
        let mut stack = FrameStack::default();
        stack.push_ternary(TernaryFrame {
            owner_role: TernaryOwnerRole::Assignment,
            parent_delimiter: None,
            question_indent_spaces: 4,
            branch_anchor_column: Some(6),
            colon_role: None,
            colon_output_column: None,
        });
        let frame = stack.active_ternary_mut().expect("ternary");
        frame.colon_role = Some(ColonRole::Ternary);
        frame.colon_output_column = Some(11);

        let frame = stack.active_ternary().expect("ternary");
        assert_eq!(frame.question_indent_spaces, 4);
        assert_eq!(frame.colon_role, Some(ColonRole::Ternary));
        assert_eq!(frame.colon_output_column, Some(11));
    }

    #[test]
    fn string_continuation_frame_tracks_literal_anchor() {
        let mut stack = FrameStack::default();
        stack.set_string_continuation(StringContinuationFrame {
            output_line: 2,
            line_indent_spaces: 8,
            literal_start_column: 17,
            line_starts_with_chain_operator: true,
            has_opening_context: true,
            has_open_brace_before_literal: false,
            has_stream_context: true,
            inside_delimiter_context: false,
        });

        stack.set_string_continuation(StringContinuationFrame {
            output_line: 4,
            line_indent_spaces: 12,
            literal_start_column: 21,
            line_starts_with_chain_operator: false,
            has_opening_context: false,
            has_open_brace_before_literal: true,
            has_stream_context: false,
            inside_delimiter_context: true,
        });

        let frame = stack.string_continuation_on_output_line(4).expect("string");
        assert_eq!(frame.output_line, 4);
        assert_eq!(frame.literal_start_column, 21);
        let previous = stack
            .string_continuation_before_output_line(4)
            .expect("previous string");
        assert_eq!(previous.output_line, 2);
        assert!(previous.line_starts_with_chain_operator);
        stack.clear_string_continuations();
        assert!(stack.string_continuation_on_output_line(4).is_none());
    }

    #[test]
    fn stream_frame_tracks_operator_anchor_and_line_context() {
        let mut stack = FrameStack::default();
        stack.push_stream(StreamFrame {
            operator_output_column: 8,
            operator_output_line: 3,
            line_indent_spaces: 4,
            operator_ends_output_line: false,
            line_contains_nested_brace: false,
            line_has_unmatched_open_paren: false,
            line_ends_with_close_paren: false,
            line_has_positive_paren_delta: false,
            chain_anchor_column: 8,
            assignment_value_start_column: None,
            after_multiline_braced_operand: true,
        });

        let frame = stack.active_stream().expect("stream");
        assert_eq!(frame.operator_output_column, 8);
        assert_eq!(stack.active_stream_on_output_line(3), Some(frame));
        stack.mark_stream_line_context(3, true, true, true, false, true);
        let frame = stack.active_stream().expect("stream");
        assert!(frame.operator_ends_output_line);
        assert!(frame.line_contains_nested_brace);
        assert!(frame.line_has_unmatched_open_paren);
        assert!(frame.line_has_positive_paren_delta);
    }

    #[test]
    fn brace_frame_tracks_semantic_kind_and_formatter_type() {
        let mut stack = FrameStack::default();
        stack.push_brace(BraceFrame {
            semantic_kind: BraceSemanticKind::Lambda,
            formatter_type: FormatterBraceType::Command,
            header: None,
            label_block: false,
            case_block: false,
            case_header_pending: false,
            nested_case_label: false,
            class_base: false,
            header_indent_column: 7,
            body_indent_column: 11,
            sibling_indent_column: 7,
            split_header: false,
            close_output_line: None,
            close_ends_output_line: false,
        });

        let frame = stack.active_brace().expect("brace");
        assert_eq!(frame.semantic_kind, BraceSemanticKind::Lambda);
        assert_eq!(frame.formatter_type, FormatterBraceType::Command);
        assert_eq!(frame.header_indent_column, 7);
        stack.pop_brace();
        stack.mark_last_closed_brace_output_position(3);
        stack.mark_last_closed_brace_line_end(3, true);
        let closed = stack.last_closed_brace().expect("closed brace");
        assert_eq!(closed.close_output_line, Some(3));
        assert!(closed.close_ends_output_line);
    }

    #[test]
    fn constructor_initializer_frame_tracks_indent_and_layout() {
        let mut stack = FrameStack::default();
        stack.push_constructor_initializer(ConstructorInitializerFrame {
            colon_line_indent_spaces: 4,
            layout: ConstructorInitializerLayout::SameLine,
            function_try: false,
        });

        let frame = stack
            .active_constructor_initializer()
            .expect("constructor initializer");
        assert_eq!(frame.colon_line_indent_spaces, 4);
        assert_eq!(frame.layout, ConstructorInitializerLayout::SameLine);
    }

    #[test]
    fn header_and_braceless_frames_track_indent_and_matching() {
        let mut stack = FrameStack::default();
        stack.push_header(HeaderFrame {
            header: "if".to_string(),
            line_indent_spaces: 4,
            body_indent_spaces: 8,
            parent_delimiter: None,
        });
        stack.push_braceless_header(BracelessHeaderFrame {
            header: "if".to_string(),
            header_indent_spaces: 4,
            can_match_else: true,
        });

        assert_eq!(stack.active_header().expect("header").header, "if");
        let braceless = stack.active_braceless_header().expect("braceless header");
        assert!(braceless.can_match_else);
        stack.pop_braceless_header();
        assert!(stack.active_braceless_header().is_none());
    }

    #[test]
    fn comment_frame_tracks_column_and_continuation_anchor() {
        let mut stack = FrameStack::default();
        stack.push_comment(CommentFrame {
            kind: CommentFrameKind::Block,
            output_column: 10,
            multiline: true,
            continuation_anchor_column: Some(10),
        });

        let frame = stack.active_comment().expect("comment");
        assert_eq!(frame.kind, CommentFrameKind::Block);
        assert_eq!(frame.output_column, 10);
        assert_eq!(frame.continuation_anchor_column, Some(10));
        stack.clear_comments();
        assert!(stack.active_comment().is_none());
    }

    #[test]
    fn declaration_frame_tracks_pointer_role_and_anchors() {
        let mut stack = FrameStack::default();
        stack.push_declaration(DeclarationFrame {
            pointer_role: PointerRole::FunctionPointer,
            continuation_anchor_column: Some(20),
            closing_anchor_column: Some(16),
            is_typedef: true,
        });

        let frame = stack.active_declaration_mut().expect("declaration");
        assert_eq!(frame.pointer_role, PointerRole::FunctionPointer);
        assert_eq!(frame.continuation_anchor_column, Some(20));
        assert_eq!(frame.closing_anchor_column, Some(16));
        stack.clear_declarations();
        assert!(stack.active_declaration_mut().is_none());
    }
}
