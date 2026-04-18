use std::convert::Infallible;

use kirin_ir::{Block, SpecializedFunction};

pub use crate::interp::Interp;

/// Concrete (cursor-stack) execution environment.
///
/// Implemented by `ConcreteInterp`. Dialect ops for structured control flow
/// (SCF `If`, `For`) are constrained on `E: ConcreteEnv`.
///
/// The relationship between `Self::Ext` and `Self::Cursor` is:
///   `Self::Ext = ControlExt<Self::Cursor>` — not expressible as a trait
///   equality in Rust, but enforced by the `ConcreteInterp` impl which
///   concretely sets both.
pub trait ConcreteEnv: Interp {
    /// The cursor coproduct type for this language.
    ///
    /// E.g. `HighLevelCursor<V>` composed from `BlockCursor<V, HighLevel>`
    /// and `SCFCursor<V, HighLevel>`.
    type Cursor;

    /// Take the pending yield value, if any.
    ///
    /// Called by SCF cursors (e.g. `IfCursor`) after a body block has finished
    /// to collect the value produced by `scf.yield`.
    fn take_pending_yield(&mut self) -> Option<Self::Value>;
}

/// Abstract (worklist fixpoint) execution environment.
///
/// `Ext = Infallible` proves at the type level that abstract execution never
/// produces cursor push/pop events.
///
/// Implemented by `AbstractInterp`. SCF ops that need abstract semantics are
/// constrained on `E: AbstractEnv`.
pub trait AbstractEnv: Interp<Ext = Infallible> {
    /// Enqueue a block for (re-)analysis with the given entry arguments.
    fn enqueue_block(&mut self, block: Block, args: Vec<Self::Value>);

    /// Record a return/yield value from the current function.
    fn record_return(&mut self, v: Self::Value) -> Result<(), Self::Error>;

    /// The function currently being analyzed.
    fn current_function(&self) -> SpecializedFunction;
}

/// Dialect op semantic contract.
///
/// Implemented by dialect op types (e.g. `Arith<T>`, `ControlFlow<T>`) and
/// by dialect wrapper enums (e.g. `LowLevel`, `HighLevel`).
///
/// # Effect types by layer
///
/// - **Pure value ops** (`Arith`, `Cmp`, `Bitwise`, `Constant`): `type Effect = ()`
///   — only side-effect is writing SSA results. The wrapper converts `()` to
///   `Control::Advance` via `op.interpret(env).map(Control::from)`.
///
/// - **Flat CF ops** (`ControlFlow`): `type Effect = Control<E::Value, E::Ext>`
///   — produces `Jump` or `Fork` but never `Ext(...)`. Works for both abstract
///   and concrete modes with a single impl.
///
/// - **Function ops** (`Call`, `Return`): `type Effect = Control<E::Value, E::Ext>`
///   — same as flat CF.
///
/// - **SCF ops (concrete)**: `type Effect = Control<E::Value, E::Ext>` where
///   `E::Ext = ControlExt<E::Cursor>` — produces `Ext(Push(cursor))`.
///   Requires `E: ConcreteEnv`.
///
/// - **SCF ops (abstract)**: `type Effect = Control<E::Value, Infallible>`
///   — produces `Fork` or `Jump`. Requires `E: AbstractEnv`.
///
/// - **Dialect wrappers**: `type Effect = Control<E::Value, E::Ext>` — converts
///   inner `()` to `Control::Advance`, maps `Control<V, Infallible>` to
///   `Control<V, E::Ext>` via `map_ext(Into::into)`.
pub trait Interpretable<E: Interp> {
    /// The effect produced by this op.
    type Effect;
    fn interpret(&self, env: &mut E) -> Result<Self::Effect, E::Error>;
}
