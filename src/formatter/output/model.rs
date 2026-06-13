use super::super::{ContinuationIndent, LineKind};

pub(super) struct LineReplayLayout {
    pub(super) input_continuation_indent: Option<ContinuationIndent>,
    pub(super) closed_delimiter_continuation_indent: Option<usize>,
    pub(super) constructor_lambda_header_indent_spaces: Option<usize>,
    pub(super) inline_body_owner_indent_spaces: Option<usize>,
    pub(super) lisp_attached_suffix_indent_spaces: Option<usize>,
    pub(super) header_operator_indent_spaces: Option<usize>,
    pub(super) closed_lambda_parameter_list: bool,
    pub(super) closed_split_lambda_parameter_list: bool,
    pub(super) lambda_parameter_indent_spaces: Option<usize>,
}

pub(super) struct LineLayout {
    pub(super) line_kind: LineKind,
    pub(super) normal_indent: usize,
    pub(super) indent: usize,
    pub(super) exact_indent_spaces: Option<usize>,
    pub(super) class_scope_label: bool,
    pub(super) else_while_brace: bool,
}

pub(super) struct PostEmissionLayout {
    pub(super) restore_objc_message_align: Option<usize>,
    pub(super) next_sibling_statement_indent_spaces: Option<usize>,
    pub(super) split_condition_body_indent_spaces: Option<usize>,
    pub(super) ternary_call_clear_indent_spaces: Option<usize>,
    pub(super) else_while_brace: bool,
}

pub(super) struct AlignedLineLayout {
    pub(super) layout: LineLayout,
    pub(super) restore_objc_message_align: Option<usize>,
    pub(super) case_unindent_closing_line: bool,
}

pub(super) struct ContextualLineLayout {
    pub(super) layout: LineLayout,
    pub(super) output_spaces: usize,
    pub(super) split_else_state_active: bool,
    pub(super) next_sibling_statement_indent_spaces: Option<usize>,
}

pub(super) enum LineRoute<T> {
    Published,
    Layout(T),
}
