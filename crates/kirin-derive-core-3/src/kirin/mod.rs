/// Kirin's helper attribute definitions and property parsing
pub mod attrs;
/// the `fn` builder options
pub mod builder;
/// derive macro for field iterators such as `HasArguments`, `HasArgumentMut` etc.
pub mod field;

/// derive macro for accessing properties such as `IsConstant`, `IsPure` etc.
pub mod property;

/// derive macro for marker traits such as `Dialect`
pub mod marker;
