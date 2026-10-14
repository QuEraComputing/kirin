use kirin_ir::{Product, SSAValue};

use crate::{EnvIndex, Interp, InterpreterError, LatticeAnchor};

/// The engine capability for *using* an environment: reading a fact out of one
/// and writing a fact into one, at whichever [`Anchor`](Env::Anchor) family the
/// engine attaches facts to.
///
/// This is the layer where mechanism becomes policy. [`EnvStore`](crate::EnvStore) is
/// storage — it maps a context key to an environment and holds facts. This
/// trait is what an engine exposes on top of that storage, and each engine
/// decides what its own accesses *mean*: concrete execution reports an unbound
/// SSA read as an error, while a sparse-forward analysis logs the read and
/// treats an absent binding as bottom.
///
/// **The access interface is anchor-generic.** A sparse engine anchors facts to
/// [`SSAValue`]s; a dense engine anchors them to
/// [`ProgramPoint`](crate::ProgramPoint)s. Both *use* an environment the same
/// way — read a fact, write a fact — so that shared vocabulary must not name one
/// anchor family. Operations that are genuinely SSA-shaped live on
/// [`SSABinding`] instead, which pins `Anchor = SSAValue` and is
/// blanket-implemented, so an SSA-anchored engine gets them for free and a
/// point-anchored engine is never asked for them.
///
/// **Environment *lifetime* is deliberately not here.** Allocating and freeing
/// an activation belongs to the call boundary, so `alloc_env`/`free_env` live on
/// [`CallServices`](crate::CallServices) with `resolve_callee`/`discover_body`, and a
/// [`CallFrame`](crate::CallFrame) owns pairing them correctly. Keeping them off
/// this trait is what lets a frame that only reads and writes — `ScfForFrame`,
/// `BlockCursor::write_child_results` — bound exactly what it consumes, and
/// what lets an abstract dataflow engine expose storage access without a call
/// convention it never performs.
///
/// Selecting a *context key* is not here either: that is an analysis policy
/// decision, so keyed allocation
/// ([`EnvStore::get_or_allocate`](crate::EnvStore::get_or_allocate)) stays internal to the
/// engine that has a policy, and never appears on this shared surface.
pub trait Env: Interp {
    /// Where this engine's environments attach facts: [`SSAValue`] for the
    /// sparse shapes, [`ProgramPoint`](crate::ProgramPoint) for the dense ones.
    ///
    /// It is the same anchor the engine's
    /// [`EnvStore<_, Anchor, _>`](crate::EnvStore) is parameterized by, which is
    /// why it carries only [`LatticeAnchor`]'s `Clone + Eq + Hash`.
    type Anchor: LatticeAnchor;

    /// Read the fact anchored at `anchor` in an activation.
    fn env_read(&self, index: EnvIndex, anchor: Self::Anchor) -> Result<Self::Value, Self::Error>;
    /// Write the fact anchored at `anchor` in an activation.
    fn env_write(
        &mut self,
        index: EnvIndex,
        anchor: Self::Anchor,
        data: Self::Value,
    ) -> Result<(), Self::Error>;
}

/// Positional SSA binding, for engines whose environments are anchored on
/// [`SSAValue`].
///
/// Split out of [`Env`] rather than defaulted on it: binding a *list* of values
/// to a *list* of slots is meaningful only where the anchor is an SSA value, so
/// it is bounded `Env<Anchor = SSAValue>` and blanket-implemented. That keeps
/// [`Env`]'s own vocabulary free of one anchor family while every SSA-anchored
/// engine still gets this for free — no engine implements it, and no dense
/// engine is asked to.
pub trait SSABinding: Env<Anchor = SSAValue> {
    /// Positionally bind runtime values to SSA slots in an **explicitly
    /// selected** activation, checking arity.
    ///
    /// The explicitly-addressed counterpart of
    /// [`SparseForwardInterp::write_results`](crate::SparseForwardInterp::write_results),
    /// which always binds into the engine's *current* activation
    /// ([`Interp::index`]). Frames need this one: a frame binds results into the
    /// activation it owns, which is not necessarily the one a dialect rule is
    /// executing in. The two differ by *which activation*, not by what they do —
    /// hence neither name mentions the [`Product`] container.
    ///
    /// It writes through [`Env::env_write`] rather than reaching into storage,
    /// so an engine's logging and absence policy apply to bound values exactly
    /// as they do to a dialect rule's writes.
    fn bind_values(
        &mut self,
        index: EnvIndex,
        slots: &[SSAValue],
        values: Product<Self::Value>,
    ) -> Result<(), Self::Error> {
        if slots.len() != values.len() {
            return Err(Self::Error::from(InterpreterError::ProductArityMismatch {
                expected: slots.len(),
                actual: values.len(),
            }));
        }
        for (slot, value) in slots.iter().copied().zip(values) {
            self.env_write(index, slot, value)?;
        }
        Ok(())
    }
}

impl<T: Env<Anchor = SSAValue>> SSABinding for T {}
