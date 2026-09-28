//! Line emission: indentation layout, vertical spacing, and the output buffer.

mod blank_lines;
pub(crate) mod block_spacing;
pub(crate) mod buffer;
mod emission;
mod finish;
mod layout;
pub(crate) mod line_adjust;
pub(crate) mod member_spacing;
mod model;
mod replay;
mod retab;
mod routing;
pub(crate) mod source_indent;
mod whitespace;
