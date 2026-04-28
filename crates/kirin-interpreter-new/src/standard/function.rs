use std::marker::PhantomData;

use kirin_ir::{
    Dialect, Function, GetInfo, HasStageInfo, SpecializedFunction, StagedFunction, TryLiftFrom,
    UniqueLiveSpecializationError,
};

use crate::{
    ConcreteInterpreter, Env, EnvIndex, Frame, FrameEffect, HasLocation, InterpreterError,
    Location, Position, StandardCompletion, Traversal,
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
    E: From<InterpreterError>,
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
            .ok_or(InterpreterError::MissingFunction(function))?;
        info.staged_function(stage)
            .ok_or(InterpreterError::MissingStagedFunction { function, stage })
            .map_err(E::from)
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
            Err(UniqueLiveSpecializationError::NoSpecialization) => {
                Err(InterpreterError::MissingSpecialization(function).into())
            }
            Err(UniqueLiveSpecializationError::Ambiguous { count }) => {
                Err(InterpreterError::AmbiguousSpecialization { function, count }.into())
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
            .map_err(E::from)
    }
}

pub trait FunctionBodyDispatch<L: Dialect, F, E, V> {
    fn dispatch_function_body(
        &mut self,
        location: Location,
        body: kirin_ir::Statement,
        env: EnvIndex,
        args: Vec<V>,
    ) -> Result<F, E>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionFrame<L, V> {
    pub location: Location,
    pub function: Function,
    pub args: Vec<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> FunctionFrame<L, V> {
    pub fn new(stage: kirin_ir::CompileStage, function: Function, args: Vec<V>) -> Self {
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
    F: From<StagedFunctionFrame<L, V>>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let staged = interp.staged_function(self.location.stage, self.function)?;
        Ok(FrameEffect::Continue(
            StagedFunctionFrame::<L, V>::new(self.location.stage, staged, self.args).into(),
        ))
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
    pub args: Vec<V>,
    _marker: PhantomData<fn() -> L>,
}

impl<L, V> StagedFunctionFrame<L, V> {
    pub fn new(stage: kirin_ir::CompileStage, function: StagedFunction, args: Vec<V>) -> Self {
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
    F: From<SpecializedFunctionFrame<L, V>>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let specialized = interp.specialized_function(self.location.stage, self.function)?;
        Ok(FrameEffect::Continue(
            SpecializedFunctionFrame::<L, V>::new(self.location.stage, specialized, self.args)
                .into(),
        ))
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
    pub fn new(stage: kirin_ir::CompileStage, function: SpecializedFunction, args: Vec<V>) -> Self {
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
    Entry { args: Vec<V> },
    Active { env: EnvIndex },
}

impl<I, L, F, C, E, V> Frame<I, F, C, E> for SpecializedFunctionFrame<L, V>
where
    I: Env<V, Error = E> + FunctionAccess<L, Error = E> + FunctionBodyDispatch<L, F, E, V>,
    L: Dialect,
    F: From<SpecializedFunctionFrame<L, V>>,
    C: TryLiftFrom<StandardCompletion<V>> + crate::ProjectOrSelf<StandardCompletion<V>>,
    E: From<InterpreterError> + From<<C as TryLiftFrom<StandardCompletion<V>>>::Error>,
{
    fn step(self, interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        let location = self.location;
        let function = self.function;
        let SpecializedFunctionState::Entry { args } = self.state else {
            return Err(InterpreterError::UnexpectedCompletion {
                location,
                completion: "active specialized function frame stepped",
            }
            .into());
        };

        let env = interp.alloc();
        let body = interp.function_body(location.stage, function)?;
        let child = interp.dispatch_function_body(location, body, env, args)?;
        Ok(FrameEffect::Push {
            parent: Self::active(location, function, env).into(),
            child,
        })
    }

    fn resume_done(self, _interp: &mut I) -> Result<FrameEffect<F, C>, E> {
        Err(InterpreterError::FunctionBodyFellThrough(self.location).into())
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
            ) => Err(InterpreterError::FunctionBodyFellThrough(self.location).into()),
            Err(completion) => Ok(FrameEffect::Complete(completion)),
        }
    }
}
