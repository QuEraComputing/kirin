use kirin_ir::{CompileStage, Product, SSAValue, Statement};

use crate::{EnvIndex, ForwardEffect, InterpreterError};

/// Compile-time semantics marker for forward **evaluation**.
///
/// Used as the second type parameter of [`Interpretable`](crate::Interpretable)
/// to select the forward-evaluation rule for a dialect statement: read operands,
/// compute a semantic/lattice value, write results. This one mode covers concrete
/// execution, constant propagation, and interval analysis — they differ only in
/// the value domain, not in the shape of the rule. It is a pure type-level tag,
/// never instantiated at runtime.
///
/// It is deliberately *not* called `ForwardValue`: a future forward **type
/// inference** mode also attaches facts to SSA values, but should expose a
/// different rule API (`ForwardType` / `ForwardTypeInterp`), so the name reflects
/// the *evaluation* semantics rather than "operates on values".
///
/// Future sibling modes (not yet implemented) would each get their own marker and
/// engine trait:
/// - `ForwardType` / `ForwardTypeInterp` — forward type inference;
/// - `BackwardDataflow` / `BackwardDataflowInterp` — generic backward dataflow;
/// - [`BackwardLiveness`] / `BackwardLivenessInterp` — backward liveness.
pub struct ForwardEval;

/// Compile-time semantics marker for a future backward liveness analysis.
///
/// Reserved so a dialect can carry both an [`Interpretable<I, ForwardEval>`]
/// and an [`Interpretable<I, BackwardLiveness>`] impl without coherence
/// conflicts. Like [`ForwardEval`], it is a pure type-level tag. (Its engine
/// trait would be `BackwardLivenessInterp`; see [`ForwardEval`] for the full list
/// of planned sibling modes.)
///
/// [`Interpretable<I, ForwardEval>`]: crate::Interpretable
/// [`Interpretable<I, BackwardLiveness>`]: crate::Interpretable
pub struct BackwardLiveness;

/// The current statement location an engine is interpreting.
///
/// Engines stash this before dispatching a dialect rule so the rule can read it
/// back through [`Interp::stage`]/[`Interp::statement`]/[`Interp::index`] (and,
/// for forward rules, the SSA helpers on [`ForwardEvalInterp`]).
#[derive(Clone, Copy, Debug)]
pub struct InterpLocation {
    pub stage: CompileStage,
    pub statement: Statement,
    pub index: EnvIndex,
}

/// Shared engine contract for concrete execution and analyses.
///
/// `Interp` names the value domain, error type, statement effect, and the
/// semantics [`Kind`](Interp::Kind) of an engine, and exposes the current
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
    /// The semantics this engine runs — e.g. [`ForwardEval`]. Dialect rules are
    /// selected by matching their [`Interpretable`](crate::Interpretable) `Kind`
    /// parameter against this.
    type Kind;

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

/// Forward-evaluation engine flavor: env access plus [`ForwardEffect`].
///
/// Forward-evaluation dialect rules (`impl Interpretable<I, ForwardEval>`) bound
/// on this trait and use its SSA helpers — [`read`](Self::read), [`read_many`](Self::read_many),
/// [`write`](Self::write), [`write_results`](Self::write_results) — which operate
/// on the engine's *current* activation ([`Interp::index`]). The associated
/// frame type is exposed only because [`ForwardEffect::Push`] carries a frame;
/// ordinary dialects do not name it.
pub trait ForwardEvalInterp:
    Env + Interp<Kind = ForwardEval, Effect = ForwardEffect<<Self as Interp>::Value, Self::Frame>>
{
    /// The engine's total frame type, carried by [`ForwardEffect::Push`].
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

impl<V, F, I> ForwardEvalInterp for I
where
    I: Env + Interp<Value = V, Kind = ForwardEval, Effect = ForwardEffect<V, F>>,
{
    type Frame = F;
}
