//! Classification of the token that starts the next source line.

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(crate) struct NextLineState {
    pub(crate) leads_with_assignment: bool,
    pub(crate) leads_with_class_init: bool,
    pub(crate) leads_with_class_base: bool,
    pub(crate) leads_with_comma: bool,
    pub(crate) leads_with_open_brace: bool,
    pub(crate) leads_with_close_brace: bool,
    pub(crate) leads_with_else: bool,
    pub(crate) word_followed_by_open_paren: bool,
    pub(crate) leads_with_noexcept: bool,
    pub(crate) leads_with_open_paren: bool,
}
