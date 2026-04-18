use kirin::prelude::{CompileTimeValue, Dialect};
use kirin_interpreter_6::has_cursor::HasCursor;

use crate::StructuredControlFlow;

use super::cursor::SCFCursor;

// ---------------------------------------------------------------------------
// SCF has no non-Core effects — no HasEffect impl needed.
//
// All SCF ops (If, For, Yield) produce Core effects:
//   scf.if  → Core::Push(IfCursor)
//   scf.for → Core::Push(ForCursor)
//   scf.yield → Core::Yield(v)
//
// Therefore SCF does NOT implement HasEffect. The language-level effect type
// for a language composed with SCF has no SCF variant — only a Core variant.
//
// #[derive(ComposedEffect)] will skip SCF accordingly.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// HasCursor impl: SCF contributes SCFCursor<V, L> to the language cursor coproduct.
//
// The L parameter is the *containing language* — SCFCursor<V, L> creates
// BlockCursor<V, L> for body block execution.
// ---------------------------------------------------------------------------

impl<T: CompileTimeValue, L: Dialect> HasCursor<L> for StructuredControlFlow<T> {
    /// The cursor coproduct for SCF within language L.
    ///
    /// #[derive(ComposedCursor)] reads this to add an SCF(SCFCursor<V, L>) variant
    /// to the language cursor coproduct. Written manually until the derive exists.
    type Cursor<V> = SCFCursor<V, L>;
}

// ---------------------------------------------------------------------------
// Core Lift impls for SCF cursors
//
// These allow the language cursor coproduct to satisfy
// `Core<V, LangCursor<V>>: Lift<SCFCursor<V, L>>`, which is used when the
// language cursor coproduct's `Lift<SCFCursor<V, L>> for LangCursor<V>` impl
// generates `LangCursor::SCF(cursor)` and the driver loop wraps it in
// `Core::Push(LangCursor::SCF(cursor))`.
//
// There is no `Lift<SCFEffect> for Core` — SCF has no dialect-specific effects.
// ---------------------------------------------------------------------------

// No Core Lift for a dialect effect here; SCF ops construct Core effects directly.
// See interpret.rs for how If/For/Yield ops return Core::Push / Core::Yield.
