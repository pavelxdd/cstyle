use crate::formatter::continuation::ContinuationIndent;
use crate::formatter::state::indentation::LineKind;

pub(crate) struct LineReplayLayout {
    pub(crate) input_continuation_indent: Option<ContinuationIndent>,
    pub(crate) closed_delimiter_continuation_indent: Option<usize>,
    pub(crate) constructor_lambda_header_indent_spaces: Option<usize>,
    pub(crate) inline_body_owner_indent_spaces: Option<usize>,
    pub(crate) lisp_attached_suffix_indent_spaces: Option<usize>,
    pub(crate) header_operator_indent_spaces: Option<usize>,
    pub(crate) closed_lambda_parameter_list: bool,
    pub(crate) closed_split_lambda_parameter_list: bool,
    pub(crate) lambda_parameter_indent_spaces: Option<usize>,
}

pub(crate) struct LineLayout {
    pub(crate) line_kind: LineKind,
    pub(crate) normal_indent: usize,
    pub(crate) indent: usize,
    pub(crate) exact_indent_spaces: Option<usize>,
    pub(crate) class_scope_label: bool,
    pub(crate) else_while_brace: bool,
}

pub(super) struct PostEmissionLayout {
    pub(crate) restore_objc_message_align: Option<usize>,
    pub(super) split_condition_body_indent_spaces: Option<usize>,
    pub(crate) ternary_call_clear_indent_spaces: Option<usize>,
    pub(crate) else_while_brace: bool,
}

pub(crate) struct AlignedLineLayout {
    pub(crate) layout: LineLayout,
    pub(crate) restore_objc_message_align: Option<usize>,
    pub(crate) case_unindent_closing_line: bool,
}

pub(crate) struct ContextualLineLayout {
    pub(crate) layout: LineLayout,
    pub(crate) output_spaces: usize,
    pub(crate) split_else_state_active: bool,
}

pub(crate) enum LineRoute<T> {
    Published,
    Layout(T),
}
