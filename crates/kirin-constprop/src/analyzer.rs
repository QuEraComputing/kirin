use core::convert::Infallible;

use kirin_interpreter_new::{
    AbstractEnvStore, Frame, FunctionEntryTarget, InterpreterError, OwnerSemantics,
};
use kirin_ir::{CompileStage, Function, HasBottom, HasTop, LiftFrom, Product};

use crate::{
    ConstPropFixpointInterpreter, ConstPropLocationSummary, ConstPropOwner, ConstPropSummary,
    ConstPropValue,
};

pub type ConstPropFunctionFixpoint<'ir, Stage, F, C, E, V = ConstPropValue, L = ()> =
    ConstPropFixpointInterpreter<
        'ir,
        Stage,
        ConstPropOwner,
        F,
        C,
        E,
        ConstPropSummary<V, L>,
        AbstractEnvStore<V>,
    >;

pub struct ConstPropAnalyzerHelper<'ir, Stage, V = ConstPropValue> {
    pipeline: &'ir kirin_ir::Pipeline<Stage>,
    stage: Option<CompileStage>,
    target: Option<FunctionEntryTarget>,
    args: Product<V>,
}

impl<'ir, Stage, V> ConstPropAnalyzerHelper<'ir, Stage, V> {
    pub fn new(pipeline: &'ir kirin_ir::Pipeline<Stage>) -> Self {
        Self {
            pipeline,
            stage: None,
            target: None,
            args: Product::new(),
        }
    }

    pub fn stage(mut self, stage: CompileStage) -> Self {
        self.stage = Some(stage);
        self
    }

    pub fn target(mut self, target: FunctionEntryTarget) -> Self {
        self.target = Some(target);
        self
    }

    pub fn function(self, function: Function) -> Self {
        self.target(FunctionEntryTarget::Function(function))
    }

    pub fn args<A>(mut self, args: A) -> Self
    where
        A: IntoIterator<Item = V>,
    {
        self.args = args.into_iter().collect();
        self
    }

    pub fn analyze_return_with<F, C, E, L, Sem>(
        self,
        make_semantics: impl FnOnce(&Product<V>) -> Sem,
    ) -> Result<V, E>
    where
        V: HasBottom + HasTop + Clone + PartialEq,
        L: ConstPropLocationSummary<V>,
        F: Frame<ConstPropFunctionFixpoint<'ir, Stage, F, C, E, V, L>, F, C, E>,
        Sem: OwnerSemantics<
                ConstPropFunctionFixpoint<'ir, Stage, F, C, E, V, L>,
                ConstPropOwner,
                ConstPropSummary<V, L>,
                F,
                C,
                E,
            >,
        E: LiftFrom<InterpreterError> + LiftFrom<Infallible>,
    {
        let stage = self
            .stage
            .ok_or_else(|| E::lift_from(InterpreterError::Custom("missing constprop stage")))?;
        let target = self
            .target
            .ok_or_else(|| E::lift_from(InterpreterError::Custom("missing constprop target")))?;
        let owner = ConstPropOwner::function(stage, target);
        let mut interp = ConstPropFunctionFixpoint::new(self.pipeline, AbstractEnvStore::new(), ());
        let mut semantics = make_semantics(&self.args);

        interp.solve(&mut semantics, owner)?;
        Ok(interp
            .summary(&owner)
            .and_then(ConstPropSummary::function_value)
            .cloned()
            .unwrap_or_else(V::bottom))
    }
}
