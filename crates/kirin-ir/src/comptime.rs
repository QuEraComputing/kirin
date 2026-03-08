pub trait CompileTimeValue: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}

/// A type that can produce a placeholder value for use before type inference.
///
/// Unlike `Default`, which implies a semantically meaningful "zero" value,
/// `Placeholder` explicitly marks a value as temporary — it will be replaced
/// by a real type during inference or lowering. Dialect types that need
/// pre-inference construction (e.g., function call results whose types are
/// resolved later) should implement this trait.
///
/// Dialects with simple, fixed type systems (e.g., only `i64`) don't need
/// this trait at all — it's only required where the builder needs to create
/// SSA values without a known type (e.g., `ResultValue` fields without an
/// explicit `#[kirin(type = ...)]` annotation).
pub trait Placeholder: CompileTimeValue {
    fn placeholder() -> Self;
}

pub trait Typeof<Ty> {
    fn type_of(&self) -> Ty;
}

impl<T> CompileTimeValue for T where T: Clone + std::fmt::Debug + std::hash::Hash + PartialEq {}
