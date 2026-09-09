use kirin_ir::{Product, SSAValue};

use crate::{EnvIndex, Interp, InterpreterError};

/// The engine capability for *using* an environment: reading values out of one,
/// writing values into one, and binding a list of values positionally.
///
/// This is the layer where mechanism becomes policy. [`EnvStore`](crate::EnvStore) is
/// storage — it maps a context key to an environment and holds facts. This
/// trait is what an engine exposes on top of that storage, and each engine
/// decides what its own accesses *mean*: concrete execution reports an unbound
/// SSA read as an error, while a sparse-forward analysis logs the read and
/// treats an absent binding as bottom.
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
    /// Read an SSA value from an activation.
    fn env_read(&self, index: EnvIndex, value: SSAValue) -> Result<Self::Value, Self::Error>;
    /// Write an SSA value into an activation.
    fn env_write(
        &mut self,
        index: EnvIndex,
        value: SSAValue,
        data: Self::Value,
    ) -> Result<(), Self::Error>;

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
    /// It writes through [`env_write`](Self::env_write) rather than reaching
    /// into storage, so an engine's logging and absence policy apply to bound
    /// values exactly as they do to a dialect rule's writes.
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
