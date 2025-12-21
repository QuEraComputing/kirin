/// Kirin's helper attribute definitions and property parsing
pub mod attrs;

/// Common extra information for statement definitions
pub mod extra;
/// the `fn` builder options
pub mod builder;
/// derive macro for field iterators such as `HasArguments`, `HasArgumentMut` etc.
pub mod field;
// /// derive macro for setting statement text format
// pub mod format;
/// derive macro for marker traits such as `Dialect`
pub mod marker;
/// derive macro for getting the name of an instruction or dialect
pub mod name;
/// derive macro for accessing properties such as `IsConstant`, `IsPure` etc.
pub mod property;

pub mod prelude {
    pub use super::builder::Builder;
    pub use super::field::FieldsIter;
    pub use super::marker::DialectMarker;
    pub use super::property::{IsConstant, IsPure, IsTerminator, Property, SearchProperty};
}
