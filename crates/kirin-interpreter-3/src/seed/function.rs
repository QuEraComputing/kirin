use kirin_ir::{Dialect, Function, Product, ResultValue, SpecializedFunction};
use smallvec::SmallVec;

use crate::{
    Effect, Execute, InterpreterError, Machine, PipelineAccess, ProductValue, ResolutionPolicy,
};

use super::super::runtime::SingleStage;
use super::RegionSeed;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSeed<V> {
    pub callee: SpecializedFunction,
    pub args: SmallVec<[V; 2]>,
    pub results: Product<ResultValue>,
}

impl<V> FunctionSeed<V> {
    #[must_use]
    pub fn new(
        callee: SpecializedFunction,
        args: impl Into<SmallVec<[V; 2]>>,
        results: Product<ResultValue>,
    ) -> Self {
        Self {
            callee,
            args: args.into(),
            results,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedFunctionSeed<V> {
    pub function: Function,
    pub args: SmallVec<[V; 2]>,
    pub results: Product<ResultValue>,
    pub policy: ResolutionPolicy,
}

impl<V> StagedFunctionSeed<V> {
    #[must_use]
    pub fn new(
        function: Function,
        args: impl Into<SmallVec<[V; 2]>>,
        results: Product<ResultValue>,
    ) -> Self {
        Self {
            function,
            args: args.into(),
            results,
            policy: ResolutionPolicy::UniqueLive,
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: ResolutionPolicy) -> Self {
        self.policy = policy;
        self
    }
}

impl<'ir, L, V, M, S> Execute<SingleStage<'ir, L, V, M, S>> for FunctionSeed<V>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    type Output = Effect<V, M::Effect>;

    fn execute(
        self,
        interp: &mut SingleStage<'ir, L, V, M, S>,
    ) -> Result<Self::Output, <SingleStage<'ir, L, V, M, S> as Machine>::Error> {
        interp.push_specialization_frame(self.callee)?;
        let region = interp.specialization_entry_region(self.callee)?;
        let terminal = RegionSeed::new(region, self.args).execute(interp);
        let pop_result = interp.pop_current_frame();

        match (terminal, pop_result) {
            (Ok(Effect::Return(value)), Ok(())) => {
                Ok(Effect::BindProduct(self.results, value).then(Effect::Advance))
            }
            (Ok(_), Ok(())) => {
                Err(InterpreterError::Unsupported("expected return from callee".to_owned()).into())
            }
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), Err(_)) => Err(error),
        }
    }
}

impl<'ir, L, V, M, S> Execute<SingleStage<'ir, L, V, M, S>> for StagedFunctionSeed<V>
where
    L: Dialect + crate::Interpretable<SingleStage<'ir, L, V, M, S>>,
    V: Clone + ProductValue,
    M: Machine,
    M::Effect: crate::Lift<L::Effect>,
    M::Error: crate::Lift<L::Error>,
    S: kirin_ir::HasStageInfo<L>,
{
    type Output = Effect<V, M::Effect>;

    fn execute(
        self,
        interp: &mut SingleStage<'ir, L, V, M, S>,
    ) -> Result<Self::Output, <SingleStage<'ir, L, V, M, S> as Machine>::Error> {
        let callee = interp.resolve_callee(self.function, &self.args, self.policy)?;

        FunctionSeed::new(callee, self.args, self.results).execute(interp)
    }
}
