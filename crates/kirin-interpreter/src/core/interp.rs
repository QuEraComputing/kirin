use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, InterpreterError, SemanticKey, SparseForwardEffect, SparseForwardSemantic};

// An engine names its semantics through [`Interp::Semantics`], a
// [`SemanticKey`] from [`semantics`](crate::semantics): [`ForwardEval`](crate::ForwardEval) is
// forward evaluation (concrete execution, constprop, interval),
// [`StrongDemand`](crate::StrongDemand) is per-value backward demand, and
// [`ClassicLiveness`](crate::ClassicLiveness) is per-point backward liveness.
// The key's [`Shape`](SemanticKey::Shape) fixes the solver mechanics.

/// The current statement location an engine is interpreting.
///
/// Engines stash this before dispatching a dialect rule so the rule can read it
/// back through [`Interp::stage`]/[`Interp::statement`]/[`Interp::index`] (and,
/// for forward rules, the SSA helpers on [`SparseForwardInterp`]).
#[derive(Clone, Copy, Debug)]
pub struct InterpLocation {
    pub stage: CompileStage,
    pub statement: Statement,
    pub index: EnvIndex,
}

/// Shared engine contract for concrete execution and analyses.
///
/// `Interp` names the value domain, error type, statement effect, and the
/// [`Semantics`](Interp::Semantics) key of an engine, and exposes the current
/// statement location. SSA storage access lives on [`Env`], and traversal lives
/// in frame types.
pub trait Interp: Sized {
    /// The value domain: concrete values for execution, lattice elements for
    /// abstract interpretation.
    type Value: Clone;
    /// The total error type of the interpreter run.
    type Error: From<InterpreterError>;
    /// The per-statement effect/result produced by this analysis.
    type Effect;
    /// The semantic key this engine runs — e.g. [`ForwardEval`](crate::ForwardEval). Dialect rules
    /// are selected by matching their [`Interpretable`](crate::Interpretable)
    /// `Semantics` parameter against this; the key's
    /// [`Shape`](SemanticKey::Shape) fixes the solver mechanics.
    type Semantics: SemanticKey;

    /// The stage the current statement belongs to.
    fn stage(&self) -> CompileStage;
    /// The statement currently being interpreted.
    fn statement(&self) -> Statement;
    /// The current SSA activation.
    fn index(&self) -> EnvIndex;
}

/// Marker trait for lattice-valued abstract interpretation engines.
///
/// This intentionally does not require forward env access, widening, or a
/// universal join API; those belong to concrete engine specializations. The
/// value domain only needs to be `Clone`; lattice operations like
/// [`HasBottom`](kirin_ir::HasBottom)/[`HasTop`](kirin_ir::HasTop) are required
/// by concrete engine specializations, not by this marker.
pub trait AbstractInterpreter: Interp {}

/// SSA storage access used by forward engines.
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
}

/// [`SparseForwardShape`](crate::SparseForwardShape)-engine flavor: env
/// access plus [`SparseForwardEffect`]. This is the *shape-generic* engine
/// surface: env/read/write are how any sparse-forward semantics executes
/// statements, so the blanket impl below covers every engine whose
/// [`Semantics`](Interp::Semantics) is a [`SparseForwardSemantic`] —
/// [`ForwardEval`](crate::ForwardEval) today, downstream keys (e.g. a qubit-address analysis)
/// tomorrow. Rules pick their key in the `Interpretable` tag.
///
/// Dialect rules (`impl Interpretable<I, ForwardEval>`, or another
/// sparse-forward key) bound on this trait and use its SSA helpers —
/// [`read`](Self::read), [`read_many`](Self::read_many),
/// [`write`](Self::write), [`write_results`](Self::write_results) — which
/// operate on the engine's *current* activation ([`Interp::index`]). The
/// associated frame type is exposed only because [`SparseForwardEffect::Push`]
/// carries a frame; ordinary dialects do not name it.
pub trait SparseForwardInterp:
    Env + Interp<Effect = SparseForwardEffect<<Self as Interp>::Value, Self::Frame>>
{
    /// The engine's total frame type, carried by [`SparseForwardEffect::Push`].
    type Frame;

    /// Read one SSA value from the current activation.
    fn read(&self, value: impl Into<SSAValue>) -> Result<Self::Value, Self::Error> {
        self.env_read(self.index(), value.into())
    }

    /// Read a list of SSA values into a [`Product`].
    fn read_many(&self, values: &[SSAValue]) -> Result<Product<Self::Value>, Self::Error> {
        values.iter().map(|value| self.read(*value)).collect()
    }

    /// Write one SSA value into the current activation.
    fn write(&mut self, value: impl Into<SSAValue>, data: Self::Value) -> Result<(), Self::Error> {
        let index = self.index();
        self.env_write(index, value.into(), data)
    }

    /// Destructure a [`Product`] into result slots, checking arity.
    fn write_results<T: Into<SSAValue> + Copy>(
        &mut self,
        values: &[T],
        data: Product<Self::Value>,
    ) -> Result<(), Self::Error> {
        if values.len() != data.len() {
            return Err(Self::Error::from(InterpreterError::ProductArityMismatch {
                expected: values.len(),
                actual: data.len(),
            }));
        }
        for (value, data) in values.iter().zip(data) {
            self.write(*value, data)?;
        }
        Ok(())
    }
}

impl<V, F, I> SparseForwardInterp for I
where
    I: Env + Interp<Value = V, Effect = SparseForwardEffect<V, F>>,
    I::Semantics: SparseForwardSemantic,
{
    type Frame = F;
}
