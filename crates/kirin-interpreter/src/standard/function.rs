use std::marker::PhantomData;

use kirin_ir::{
    Dialect, Function, GetInfo, HasStageInfo, Product, SpecializedFunction, StagedFunction,
    UniqueLiveSpecializationError,
};

use crate::{
    AbstractInterpreterWithStore, ConcreteInterpreter, Env, EnvIndex, FixpointProfile, Frame,
    FrameEffect, HasLocation, InterpreterError, InterpreterProfile, Location, Position,
    StageAccess, StandardCompletion, StandardFixpointInterpreter, Traversal,
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

impl<'ir, P, L> FunctionAccess<L> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| P::Error::from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(P::Error::from)
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
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(P::Error::from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(P::Error::from(InterpreterError::AmbiguousSpecialization {
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
            .map_err(P::Error::from)
    }
}

impl<'ir, P, Store, L> FunctionAccess<L> for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| P::Error::from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(P::Error::from)
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
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(P::Error::from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(P::Error::from(InterpreterError::AmbiguousSpecialization {
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
            .map_err(P::Error::from)
    }
}

impl<'ir, P, Store, Deps, L> FunctionAccess<L> for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    P::Stage: HasStageInfo<L>,
    L: Dialect,
    P::Error: From<InterpreterError>,
{
    type Error = P::Error;

    fn staged_function(
        &self,
        stage: kirin_ir::CompileStage,
        function: Function,
    ) -> Result<StagedFunction, Self::Error> {
        let info = self
            .pipeline()
            .function_info(function)
            .ok_or_else(|| P::Error::from(InterpreterError::MissingFunction(function)))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(P::Error::from)
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
            Err(UniqueLiveSpecializationError::NoSpecialization) => Err(P::Error::from(
                InterpreterError::MissingSpecialization(function),
            )),
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(P::Error::from(InterpreterError::AmbiguousSpecialization {
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
            .map_err(P::Error::from)
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

impl<'ir, P, L, F> FunctionBodyDispatch<L, F, P::Error, P::Value> for ConcreteInterpreter<'ir, P>
where
    P: InterpreterProfile,
    L: Dialect,
    L: FunctionEntry<L, Self, F, P::Error, P::Value>,
    P::Stage: HasStageInfo<L>,
    P::Error: From<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = StageAccess::<L>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

impl<'ir, P, L, F, Store> FunctionBodyDispatch<L, F, P::Error, P::Value>
    for AbstractInterpreterWithStore<'ir, P, Store>
where
    P: InterpreterProfile,
    L: Dialect,
    L: FunctionEntry<L, Self, F, P::Error, P::Value>,
    P::Stage: HasStageInfo<L>,
    P::Error: From<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
        let location = Location::new(location.stage, Position::Statement { statement: body });
        let definition = {
            let stage = StageAccess::<L>::stage_info(self, location.stage)?;
            body.definition(stage).clone()
        };
        definition.enter_function_body(location, env, self, args)
    }
}

impl<'ir, P, L, F, Store, Deps> FunctionBodyDispatch<L, F, P::Error, P::Value>
    for StandardFixpointInterpreter<'ir, P, Store, Deps>
where
    P: FixpointProfile,
    L: Dialect,
    L: FunctionEntry<L, Self, F, P::Error, P::Value>,
    P::Stage: HasStageInfo<L>,
    P::Error: From<InterpreterError>,
{
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Product<P::Value>,
    ) -> Result<F, P::Error> {
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
    F: TryFrom<StagedFunctionFrame<L, V>>,
    E: From<<F as TryFrom<StagedFunctionFrame<L, V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let staged = interp.staged_function(self.location.stage, self.function)?;
        StagedFunctionFrame::<L, V>::new(self.location.stage, staged, self.args)
            .try_into()
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
    F: TryFrom<SpecializedFunctionFrame<L, V>>,
    E: From<<F as TryFrom<SpecializedFunctionFrame<L, V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let specialized = interp.specialized_function(self.location.stage, self.function)?;
        SpecializedFunctionFrame::<L, V>::new(self.location.stage, specialized, self.args)
            .try_into()
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
    F: TryFrom<SpecializedFunctionFrame<L, V>>,
    C: TryFrom<StandardCompletion<V>> + crate::ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError>
        + From<<F as TryFrom<SpecializedFunctionFrame<L, V>>>::Error>
        + From<<C as TryFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let location = self.location;
        let function = self.function;
        let SpecializedFunctionState::Entry { args } = self.state else {
            return Err(E::from(InterpreterError::UnexpectedCompletion {
                location,
                completion: "active specialized function frame stepped",
            }));
        };

        let env = interp.alloc();
        let body = interp.function_body(location.stage, function)?;
        let child = interp.dispatch_function_body(location, body, env, args)?;
        Ok(FrameEffect::Push {
            parent: Self::active(location, function, env).try_into()?,
            child,
        })
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(E::from(InterpreterError::FunctionBodyFellThrough(
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
                C::try_from(StandardCompletion::FunctionReturned(value))?,
            )),
            Ok(
                StandardCompletion::BlockDone
                | StandardCompletion::RegionDone
                | StandardCompletion::GraphDone,
            ) => Err(E::from(InterpreterError::FunctionBodyFellThrough(
                self.location,
            ))),
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}
