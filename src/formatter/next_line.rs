//! Classification of the token that starts the next source line.

#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub(super) struct NextLineState {
    pub(super) leads_with_assignment: bool,
    pub(super) leads_with_class_init: bool,
    pub(super) leads_with_class_base: bool,
    pub(super) leads_with_comma: bool,
    pub(super) leads_with_open_brace: bool,
    pub(super) leads_with_close_brace: bool,
    pub(super) word_followed_by_open_paren: bool,
    pub(super) leads_with_noexcept: bool,
    pub(super) leads_with_open_paren: bool,
}
