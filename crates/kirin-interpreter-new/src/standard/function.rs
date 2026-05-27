use std::hash::Hash;
use std::marker::PhantomData;

use kirin_ir::{
    Dialect, Function, GetInfo, HasStageInfo, LiftFrom, Product, SpecializedFunction,
    StagedFunction, TryLift, TryLiftFrom, UniqueLiveSpecializationError,
};

use crate::{
    AbstractInterpreterWithStore, ConcreteInterpreter, Env, EnvIndex, Frame, FrameEffect,
    HasLocation, InterpreterError, Location, Position, StageAccess, StandardCompletion,
    StandardFixpointInterpreter, Summary, Traversal,
};

pub trait FunctionAccess<L: Dialect> {
    type Error;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error>;

    fn specialized_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: StagedFunction,
    ) -> Result<SpecializedFunction, Self::Error>;

    fn function_body(
        &self,
        stage: kirin_ir::CompileStage,
        function: SpecializedFunction,
    ) -> Result<kirin_ir::Statement, Self::Error>;
}

impl<'ir, S, F, C, E, V, L> FunctionAccess<L> for ConcreteInterpreter<'ir, S, F, C, E, V>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| E::lift_from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(E::lift_from)
    }

    fn specialized_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: StagedFunction,
    ) -> Result<SpecializedFunction, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        let info = function.expect_info(stage_info);
        match info.unique_live_specialization() {
            Ok(function) => Ok(function),
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(E::lift_from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(E::lift_from(InterpreterError::AmbiguousSpecialization {
                    function,
                    count,
                }))
            }
        }
    }

    fn function_body(
        &self,
        stage: kirin_ir::CompileStage,
        function: SpecializedFunction,
    ) -> Result<kirin_ir::Statement, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        function
            .get_info(stage_info)
            .map(|info| *info.body())
            .ok_or(InterpreterError::MissingFunctionBody { function, stage })
            .map_err(E::lift_from)
    }
}

impl<'ir, S, F, C, E, Store, L> FunctionAccess<L>
    for AbstractInterpreterWithStore<'ir, S, F, C, E, Store>
where
    S: HasStageInfo<L>,
    L: Dialect,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| E::lift_from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(E::lift_from)
    }

    fn specialized_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: StagedFunction,
    ) -> Result<SpecializedFunction, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        let info = function.expect_info(stage_info);
        match info.unique_live_specialization() {
            Ok(function) => Ok(function),
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(E::lift_from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(E::lift_from(InterpreterError::AmbiguousSpecialization {
                    function,
                    count,
                }))
            }
        }
    }

    fn function_body(
        &self,
        stage: kirin_ir::CompileStage,
        function: SpecializedFunction,
    ) -> Result<kirin_ir::Statement, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        function
            .get_info(stage_info)
            .map(|info| *info.body())
            .ok_or(InterpreterError::MissingFunctionBody { function, stage })
            .map_err(E::lift_from)
    }
}

impl<'ir, Stage, K, F, C, E, Sum, Store, Deps, L> FunctionAccess<L>
    for StandardFixpointInterpreter<'ir, Stage, K, F, C, E, Sum, Store, Deps>
where
    Stage: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    L: Dialect,
    Sum: Summary,
    E: LiftFrom<InterpreterError>,
{
    type Error = E;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| E::lift_from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(E::lift_from)
    }

    fn specialized_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: StagedFunction,
    ) -> Result<SpecializedFunction, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        let info = function.expect_info(stage_info);
        match info.unique_live_specialization() {
            Ok(function) => Ok(function),
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(E::lift_from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(E::lift_from(InterpreterError::AmbiguousSpecialization {
                    function,
                    count,
                }))
            }
        }
    }

    fn function_body(
        &self,
        stage: kirin_ir::CompileStage,
        function: SpecializedFunction,
    ) -> Result<kirin_ir::Statement, Self::Error> {
        let stage_info = crate::StageAccess::<L>::stage_info(self, stage)?;
        function
            .get_info(stage_info)
            .map(|info| *info.body())
            .ok_or(InterpreterError::MissingFunctionBody { function, stage })
            .map_err(E::lift_from)
    }
}

pub trait FunctionBodyDispatch<L: Dialect, F, E, V> {
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E>;
}

pub trait FunctionEntry<L: Dialect, I, F, E, V>: Dialect {
    fn enter_function_body(
        &self,
        location: Location,
        env: EnvIndex,
        interp: &mut I,
        args: Product<V>,
    ) -> Result<F, E>;
}

impl<'ir, S, L, F, C, E, V, RootF> FunctionBodyDispatch<L, F, E, V>
    for ConcreteInterpreter<'ir, S, RootF, C, E, V>
where
    L: Dialect,
    L: FunctionEntry<L, Self, F, E, V>,
    S: HasStageInfo<L>,
    E: LiftFrom<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = StageAccess::<L>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

impl<'ir, S, L, F, C, E, V, Store, RootF> FunctionBodyDispatch<L, F, E, V>
    for AbstractInterpreterWithStore<'ir, S, RootF, C, E, Store>
where
    L: Dialect,
    L: FunctionEntry<L, Self, F, E, V>,
    S: HasStageInfo<L>,
    E: LiftFrom<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = StageAccess::<L>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

impl<'ir, Stage, K, L, F, C, E, V, Sum, Store, Deps, RootF> FunctionBodyDispatch<L, F, E, V>
    for StandardFixpointInterpreter<'ir, Stage, K, RootF, C, E, Sum, Store, Deps>
where
    L: Dialect,
    L: FunctionEntry<L, Self, F, E, V>,
    Stage: HasStageInfo<L>,
    K: Clone + Eq + Hash,
    Sum: Summary,
    E: LiftFrom<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<F, E> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = StageAccess::<L>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionFrame<L, V> {
    pub location: Location,
    pub function: Function,
    pub args: Product<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> FunctionFrame<L, V> {
    pub fn new(stage: kirin_ir::CompileStage, function: Function, args: Product<V>) -> Self {
        Self {
            location: Location::new(
                stage,
                Position::Function {
                    function,
                    traversal: Traversal::Entry,
                },
            ),
            function,
            args,
            _marker: PhantomData,
        }
    }
}

impl<L, V> HasLocation for FunctionFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for FunctionFrame<L, V>
where
    I: FunctionAccess<L, Error = E>,
    L: Dialect,
    F: TryLiftFrom<StagedFunctionFrame<L, V>>,
    E: From<<F as TryLiftFrom<StagedFunctionFrame<L, V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let staged = interp.staged_function(self.location.stage, self.function)?;
        StagedFunctionFrame::<L, V>::new(self.location.stage, staged, self.args)
            .try_lift()
            .map(FrameEffect::Continue)
            .map_err(E::from)
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StagedFunctionFrame<L, V> {
    pub location: Location,
    pub function: StagedFunction,
    pub args: Product<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> StagedFunctionFrame<L, V> {
    pub fn new(stage: kirin_ir::CompileStage, function: StagedFunction, args: Product<V>) -> Self {
        Self {
            location: Location::new(
                stage,
                Position::StagedFunction {
                    function,
                    traversal: Traversal::Entry,
                },
            ),
            function,
            args,
            _marker: PhantomData,
        }
    }
}

impl<L, V> HasLocation for StagedFunctionFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for StagedFunctionFrame<L, V>
where
    I: FunctionAccess<L, Error = E>,
    L: Dialect,
    F: TryLiftFrom<SpecializedFunctionFrame<L, V>>,
    E: From<<F as TryLiftFrom<SpecializedFunctionFrame<L, V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let specialized = interp.specialized_function(self.location.stage, self.function)?;
        SpecializedFunctionFrame::<L, V>::new(self.location.stage, specialized, self.args)
            .try_lift()
            .map(FrameEffect::Continue)
            .map_err(E::from)
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Done)
    }

    fn resume(self, completion: C, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Ok(FrameEffect::Complete(completion))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecializedFunctionFrame<L, V> {
    pub location: Location,
    pub function: SpecializedFunction,
    pub state: SpecializedFunctionState<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> SpecializedFunctionFrame<L, V> {
    pub fn new(
        stage: kirin_ir::CompileStage,
        function: SpecializedFunction,
        args: Product<V>,
    ) -> Self {
        Self {
            location: Location::new(
                stage,
                Position::SpecializedFunction {
                    function,
                    traversal: Traversal::Entry,
                },
            ),
            function,
            state: SpecializedFunctionState::Entry { args },
            _marker: PhantomData,
        }
    }

    fn active(location: Location, function: SpecializedFunction, env: EnvIndex) -> Self {
        Self {
            location: Location::new(
                location.stage,
                Position::SpecializedFunction {
                    function,
                    traversal: Traversal::Exit,
                },
            ),
            function,
            state: SpecializedFunctionState::Active { env },
            _marker: PhantomData,
        }
    }
}

impl<L, V> HasLocation for SpecializedFunctionFrame<L, V> {
    fn location(&self) -> Location {
        self.location
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecializedFunctionState<V> {
    Entry { args: Product<V> },
    Active { env: EnvIndex },
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for SpecializedFunctionFrame<L, V>
where
    I: Env<V, Error = E> + FunctionAccess<L, Error = E> + FunctionBodyDispatch<L, F, E, V>,
    L: Dialect,
    F: TryLiftFrom<SpecializedFunctionFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + crate::ProjectOrSelf<StandardCompletion<V>>,
    E: LiftFrom<InterpreterError>
        + From<<F as TryLiftFrom<SpecializedFunctionFrame<L, V>>>::Error>
        + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let location = self.location;
        let function = self.function;
        let SpecializedFunctionState::Entry { args } = self.state else {
            return Err(E::lift_from(InterpreterError::UnexpectedCompletion {
                location,
                completion: "active specialized function frame stepped",
            }));
        };

        let env = interp.alloc();
        let body = interp.function_body(location.stage, function)?;
        let child = interp.dispatch_function_body(location, body, env, args)?;
        Ok(FrameEffect::Push {
            parent: Self::active(location, function, env).try_lift()?,
            child,
        })
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(E::lift_from(InterpreterError::FunctionBodyFellThrough(
            self.location,
        )))
    }

    fn resume(self, completion: C, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let SpecializedFunctionState::Active { env } = self.state else {
            return Ok(FrameEffect::Complete(completion));
        };
        interp.free(env)?;

        match completion.project_or_self() {
            Ok(StandardCompletion::FunctionReturned(value)) => Ok(FrameEffect::Complete(
                C::try_lift_from(StandardCompletion::FunctionReturned(value))?,
            )),
            Ok(
                StandardCompletion::BlockDone
                | StandardCompletion::RegionDone
                | StandardCompletion::GraphDone,
            ) => Err(E::lift_from(InterpreterError::FunctionBodyFellThrough(
                self.location,
            ))),
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}
