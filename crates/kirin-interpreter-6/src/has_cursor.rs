use kirin_ir::Dialect;

/// Declares the dialect-specific cursor type contributed to a composed language.
///
/// Only implement for dialects that use multi-step inline execution (cursor-based),
/// such as SCF's `IfCursor`/`ForCursor`. Dialects with no special cursors (e.g.
/// `Arith`) do NOT implement `HasCursor`.
///
/// The `L` type parameter is the *containing language*: dialect cursors need to
/// push `BlockCursor<V, L>` for body blocks, so they must know the full language
/// type `L`. This propagates through the cursor hierarchy — `SCFCursor<V, L>`
/// contains `IfCursor<V, L>` and `ForCursor<V, L>`, which create `BlockCursor<V, L>`.
///
/// The GAT `type Cursor<V>` is `V`-parameterized only (no interpreter type).
/// The `Execute<E>` behavior is added separately via trait impls.
///
/// # Example
/// ```rust,ignore
/// impl<T: CompileTimeValue, L: Dialect> HasCursor<L> for StructuredControlFlow<T> {
///     type Cursor<V> = SCFCursor<V, L>;
/// }
/// ```
///
/// # Derive support
/// `#[derive(ComposedCursor)]` reads `HasCursor<L>` for each dialect variant and
/// generates the language cursor coproduct `LangCursor<V>` with `Lift`/`Project`
/// impls and a derived `Execute<E>` impl. (derive not yet implemented — compose manually.)
pub trait HasCursor<L: Dialect> {
    type Cursor<V>;
}
