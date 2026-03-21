pub trait CompileTimeValue:
    Clone + std::fmt::Debug + std::fmt::Display + std::hash::Hash + PartialEq
{
}

/// A type that can produce a placeholder value for use before type inference.
///
/// Unlike `Default`, which implies a semantically meaningful "zero" value,
/// `Placeholder` explicitly marks a value as temporary — it will be replaced
/// by a real type during inference or lowering.
///
/// # When is this needed?
///
/// `ResultValue` fields in dialect structs/enums represent SSA outputs that
/// need a type at construction time. When a `ResultValue` field has no
/// explicit `#[kirin(type = ...)]` annotation, the derive macro automatically
/// uses `T::placeholder()` as the default type expression and adds
/// `T: Placeholder` to the generated builder's `where` clause.
///
/// Dialect authors **do not** need to write `+ Placeholder` on their struct
/// definitions or interpreter impls — the bound only appears in
/// derive-generated code (builders and parsers).
///
/// Use explicit `#[kirin(type = expr)]` to override the default when the
/// result type is computed from other fields (e.g., `#[kirin(type = value.type_of())]`
/// in [`Constant`](crate::Typeof)).
///
/// # Example
///
/// ```ignore
/// // The derive auto-infers T::placeholder() for `result` and adds
/// // T: Placeholder to the generated builder's where clause.
/// #[derive(Dialect)]
/// #[kirin(builders, type = T)]
/// pub enum Arith<T: CompileTimeValue> {
///     Add {
///         lhs: SSAValue,
///         rhs: SSAValue,
///         result: ResultValue,  // no #[kirin(type = ...)] needed
///         #[kirin(default)]
///         marker: PhantomData<T>,
///     },
/// }
/// ```
pub trait Placeholder: CompileTimeValue {
    fn placeholder() -> Self;
}

pub trait Typeof<Ty> {
    fn type_of(&self) -> Ty;
}

impl<T> CompileTimeValue for T where
    T: Clone + std::fmt::Debug + std::fmt::Display + std::hash::Hash + PartialEq
{
}
