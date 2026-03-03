mod context;
mod format_visitor;

#[cfg(test)]
mod tests;

pub use context::VisitorContext;
pub use format_visitor::{FormatVisitor, visit_format};
