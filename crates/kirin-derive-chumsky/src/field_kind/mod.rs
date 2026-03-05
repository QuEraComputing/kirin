mod kind;
mod scanner;

pub use kind::{FieldKind, collect_fields};
pub use scanner::{ValueTypeScanner, fields_in_format};
