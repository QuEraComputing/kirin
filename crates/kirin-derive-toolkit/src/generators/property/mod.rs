mod context;
mod emit;
mod scan;
mod statement;

/// Generates boolean property trait implementations.
///
/// Reads `#[kirin(terminator)]`, `#[kirin(constant)]`, `#[kirin(pure)]`,
/// and `#[kirin(speculatable)]` attributes to emit trait impls that
/// return `true` or `false` per variant.
pub use context::DeriveProperty;

/// Which property trait to generate.
pub use context::PropertyKind;

pub use context::{BareAttrReader, PropertyValueReader};
