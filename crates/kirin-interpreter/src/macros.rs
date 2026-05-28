/// Generate transitive `From<Src> for Outer` impls that route through an intermediate type.
///
/// Rust's coherence rules (specifically the blanket `impl<T> From<T> for T`) prevent `From`
/// from composing transitively: even when `Src: Into<Mid>` and `Mid: Into<Outer>`, a generic
/// blanket impl `impl<S: Into<Mid>> From<S> for Outer` would overlap with that identity impl.
/// This macro generates the per-source explicit impls so each transitive lift is concrete.
///
/// # Example
///
/// ```ignore
/// kirin_interpreter::forward_through! {
///     impl[L: Dialect, V, T] for [ToyFrame<L, V, T>] via [StandardFrame<L, V, T>]
///     from {
///         StatementFrame,
///         CallFrame<L, V>,
///         FunctionFrame<L, V>,
///     }
/// }
/// ```
///
/// Each entry expands to:
///
/// ```ignore
/// impl<L: Dialect, V, T> From<StatementFrame> for ToyFrame<L, V, T> {
///     fn from(value: StatementFrame) -> Self {
///         <Self as From<StandardFrame<L, V, T>>>::from(
///             <StandardFrame<L, V, T> as From<StatementFrame>>::from(value),
///         )
///     }
/// }
/// ```
#[macro_export]
macro_rules! forward_through {
    // Entry point with explicit generics, outer, intermediate, and source list.
    (
        impl[ $($generics:tt)* ] for [ $outer:ty ] via [ $mid:ty ]
        from { $($src:ty),+ $(,)? }
    ) => {
        $crate::__forward_through_each! {
            generics: [ $($generics)* ],
            outer: [ $outer ],
            mid: [ $mid ],
            remaining: [ $($src,)+ ],
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __forward_through_each {
    // Recursive case: peel off the first source type and emit one impl.
    (
        generics: [ $($generics:tt)* ],
        outer: [ $outer:ty ],
        mid: [ $mid:ty ],
        remaining: [ $head:ty, $($rest:tt)* ],
    ) => {
        #[automatically_derived]
        impl< $($generics)* > ::core::convert::From<$head> for $outer {
            fn from(value: $head) -> Self {
                <Self as ::core::convert::From<$mid>>::from(
                    <$mid as ::core::convert::From<$head>>::from(value),
                )
            }
        }
        $crate::__forward_through_each! {
            generics: [ $($generics)* ],
            outer: [ $outer ],
            mid: [ $mid ],
            remaining: [ $($rest)* ],
        }
    };
    // Base case: no more sources.
    (
        generics: [ $($generics:tt)* ],
        outer: [ $outer:ty ],
        mid: [ $mid:ty ],
        remaining: [],
    ) => {};
}
