//! `ConstPropDriver`: capability trait for interpreters that can run a
//! constant-propagation fixpoint analysis from a single function entry.
//!
//! Implemented automatically for any [`StandardFixpointInterpreter`] whose
//! owner is [`ConstPropOwner`] and whose summary is [`ConstPropSummary`].
//! Using the trait gives callers a one-line analysis entry point on top of
//! the lower-level [`solve`](StandardFixpointInterpreter::solve) API.

use core::convert::Infallible;

use kirin_interpreter_new::{
    AbstractValue, Env, Frame, FunctionEntryTarget, FunctionInvocationDispatch, HasProductValue,
    InterpreterError, OwnerSummaryDeps, ProjectOrSelf, StageBlockDispatch,
    StandardFixpointInterpreter, SummaryDependencyIndex,
};
use kirin_ir::{CompileStage, Function, HasBottom, HasTop, LiftFrom, Pipeline, StageMeta};

use crate::{
    AdvanceableLocationSummary, ConstPropFunctionOwner, ConstPropOwner, ConstPropSummary,
    DefaultConstPropCompletion, DefaultConstPropSemantics,
};

/// Default fixpoint interpreter type alias used by [`ConstPropDriver`]
/// impls. Wraps [`StandardFixpointInterpreter`] with the standard owner
/// dependency index.
pub type ConstPropFixpointInterpreter<
    'ir,
    Stage,
    K,
    F,
    C,
    E,
    S,
    Store,
    Deps = OwnerSummaryDeps<K>,
> = StandardFixpointInterpreter<'ir, Stage, K, F, C, E, S, Store, Deps>;

/// Value domain compatible with the standard const-prop framework.
///
/// `ConstPropDomain` is the value-level companion to [`ConstPropDriver`]:
/// abstract lattice operations plus tuple-product semantics for SSA results.
pub trait ConstPropDomain: AbstractValue + HasProductValue {}

impl<T> ConstPropDomain for T where T: AbstractValue + HasProductValue {}

/// Capability trait for interpreters that can drive a const-prop fixpoint.
///
/// `ConstPropDriver` is the single user-facing trait for running a
/// constant-propagation analysis. It carries both the marker semantics for
/// const-prop SSA environments and the convenience method
/// [`analyze_function`](Self::analyze_function) for single-entry analyses.
///
/// Use [`solve`](StandardFixpointInterpreter::solve) directly when you need
/// multi-entry analyses or custom dependency wiring.
pub trait ConstPropDriver<V>: Env<V> {
    /// Pipeline stage type. Used by name-based lookup methods.
    type Stage;

    /// Access the underlying pipeline.
    fn pipeline(&self) -> &Pipeline<Self::Stage>;

    /// Run the fixpoint from a single function entry and return the
    /// function's inferred result value.
    fn analyze_function<A>(
        &mut self,
        stage: CompileStage,
        target: FunctionEntryTarget,
        args: A,
    ) -> Result<V, <Self as Env<V>>::Error>
    where
        A: IntoIterator<Item = V>;

    /// Convenience for [`analyze_function`](Self::analyze_function) taking a
    /// raw [`Function`].
    fn analyze_function_id<A>(
        &mut self,
        stage: CompileStage,
        function: Function,
        args: A,
    ) -> Result<V, <Self as Env<V>>::Error>
    where
        A: IntoIterator<Item = V>,
    {
        self.analyze_function(stage, FunctionEntryTarget::Function(function), args)
    }

    /// Resolve a stage and function by name and run the analysis.
    fn analyze_function_by_name<A>(
        &mut self,
        stage_name: &str,
        function_name: &str,
        args: A,
    ) -> Result<V, <Self as Env<V>>::Error>
    where
        Self::Stage: StageMeta,
        <Self as Env<V>>::Error: LiftFrom<InterpreterError>,
        A: IntoIterator<Item = V>,
    {
        let stage = self
            .pipeline()
            .stage_by_name(stage_name)
            .ok_or_else(|| InterpreterError::MissingStageName(stage_name.into()))
            .map_err(<Self as Env<V>>::Error::lift_from)?;
        let function = self
            .pipeline()
            .lookup_function_by_name(function_name)
            .ok_or_else(|| InterpreterError::MissingFunctionName(function_name.into()))
            .map_err(<Self as Env<V>>::Error::lift_from)?;
        self.analyze_function_id(stage, function, args)
    }
}

impl<'ir, Stage, F, C, E, V, Loc, Store, Deps> ConstPropDriver<V>
    for StandardFixpointInterpreter<
        'ir,
        Stage,
        ConstPropOwner,
        F,
        C,
        E,
        ConstPropSummary<V, Loc>,
        Store,
        Deps,
    >
where
    V: HasBottom + HasTop + Clone + PartialEq,
    Loc: AdvanceableLocationSummary<V>,
    F: Frame<Self, F, C, E>,
    C: DefaultConstPropCompletion<V> + ProjectOrSelf<Loc::Completion, Error = Infallible>,
    Self: FunctionInvocationDispatch<F, E, V> + StageBlockDispatch<F, E, V> + Env<V, Error = E>,
    Deps: SummaryDependencyIndex<ConstPropOwner>,
    E: LiftFrom<InterpreterError> + LiftFrom<Infallible> + LiftFrom<Deps::Error>,
{
    type Stage = Stage;

    fn pipeline(&self) -> &Pipeline<Self::Stage> {
        StandardFixpointInterpreter::pipeline(self)
    }

    fn analyze_function<A>(
        &mut self,
        stage: CompileStage,
        target: FunctionEntryTarget,
        args: A,
    ) -> Result<V, E>
    where
        A: IntoIterator<Item = V>,
    {
        let function_owner = ConstPropFunctionOwner::new(stage, target);
        let owner = ConstPropOwner::Function(function_owner);
        let mut semantics: DefaultConstPropSemantics<V, Loc> =
            DefaultConstPropSemantics::new(function_owner, args);

        self.solve(&mut semantics, owner)?;
        Ok(self
            .summary(&owner)
            .and_then(ConstPropSummary::function_value)
            .cloned()
            .unwrap_or_else(V::bottom))
    }
}
