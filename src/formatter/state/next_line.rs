//! Classification of the token that starts the next source line.

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct NextLineState {
    pub(crate) leads_with_assignment: bool,
    pub(crate) leads_with_class_init: bool,
    pub(crate) leads_with_class_base: bool,
    pub(crate) leads_with_comma: bool,
    pub(crate) leads_with_open_brace: bool,
    pub(crate) leads_with_close_brace: bool,
    /// The next line's code, past comments before it, starts with `}`.
    pub(crate) code_leads_with_close_brace: bool,
    pub(crate) leads_with_else: bool,
    pub(crate) leads_with_comment: bool,
    pub(crate) word_followed_by_open_paren: bool,
    pub(crate) leads_with_noexcept: bool,
    pub(crate) leads_with_open_paren: bool,
    /// The next line opens with the brace of a braced initializer whose
    /// declarator ends the line, as `Item value` above `{ 1, 2 };`.
    pub(crate) leads_with_declarator_list: bool,
}
