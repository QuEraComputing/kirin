use kirin_ir::{Block, CompileStage, Dialect, Pipeline, SSAValue, StageInfo, Statement};

use super::SingleStage;
use crate::{
    ConsumeEffect, InterpreterError, Machine, StageAccess, ValueStore,
    control::{Breakpoint, Breakpoints, Fuel, Interrupt, Location, Shell},
    interpreter::{Driver, Interpreter, Invoke, Position, ResolveCallee, TypedStage, callee},
};

impl<'ir, L, V, M, E> Fuel for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn fuel(&self) -> Option<u64> {
        self.fuel
    }

    fn set_fuel(&mut self, fuel: Option<u64>) {
        self.fuel = fuel;
    }
}

impl<'ir, L, V, M, E> Breakpoints for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn add_breakpoint(&mut self, breakpoint: Breakpoint) -> bool {
        self.breakpoints.insert(breakpoint)
    }

    fn remove_breakpoint(&mut self, breakpoint: &Breakpoint) -> bool {
        self.breakpoints.remove(breakpoint)
    }

    fn has_breakpoint(&self, breakpoint: &Breakpoint) -> bool {
        self.breakpoints.contains(breakpoint)
    }
}

impl<'ir, L, V, M, E> Interrupt for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn request_interrupt(&mut self) {
        self.interrupt_requested = true;
    }

    fn clear_interrupt(&mut self) {
        self.interrupt_requested = false;
    }

    fn interrupt_requested(&self) -> bool {
        self.interrupt_requested
    }
}

impl<'ir, L, V, M, E> Position<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn cursor_depth(&self) -> usize {
        self.frames
            .current::<InterpreterError>()
            .map(|frame| frame.extra().cursor_stack.len())
            .unwrap_or(0)
    }

    fn current_block(&self) -> Option<Block> {
        self.frames
            .current::<InterpreterError>()
            .ok()
            .and_then(|frame| frame.extra().cursor_stack.last())
            .and_then(crate::cursor::ExecutionCursor::current_block)
    }

    fn current_statement(&self) -> Option<Statement> {
        self.frames
            .current::<InterpreterError>()
            .ok()
            .and_then(|frame| frame.extra().cursor_stack.last())
            .and_then(crate::cursor::ExecutionCursor::current)
    }

    fn current_location(&self) -> Option<Location> {
        self.frames
            .current::<InterpreterError>()
            .ok()
            .and_then(|frame| {
                frame
                    .extra()
                    .after_statement
                    .map(Location::AfterStatement)
                    .or_else(|| self.current_statement().map(Location::BeforeStatement))
            })
    }
}

impl<'ir, L, V, M, E> TypedStage<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    type Dialect = L;

    fn stage_info(&self) -> &'ir StageInfo<Self::Dialect> {
        self.stage_info_for(self.active_stage())
    }
}

impl<'ir, L, V, M, E> ValueStore for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: Clone + 'ir,
    M: Machine<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
{
    type Value = V;
    type Error = E;

    fn read(&self, value: SSAValue) -> Result<Self::Value, Self::Error> {
        self.frames.read(value).cloned()
    }

    fn write(
        &mut self,
        target: impl Into<SSAValue>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        self.frames.write_ssa(target.into(), value)
    }
}

impl<'ir, L, V, M, E> StageAccess<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    type StageInfo = StageInfo<L>;

    fn pipeline(&self) -> &'ir Pipeline<Self::StageInfo> {
        self.pipeline
    }

    fn active_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.root_stage)
    }
}

impl<'ir, L, V, M, E> Driver<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect
        + 'ir
        + crate::Interpretable<
            'ir,
            SingleStage<'ir, L, V, M, E>,
            Effect = <M as Machine<'ir>>::Effect,
        >,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    fn poll_execution_gate(&mut self) -> Result<Option<Statement>, crate::result::Suspension> {
        loop {
            let Some(location) = self.current_location() else {
                return Ok(None);
            };

            if self.has_breakpoint(&Breakpoint::new(self.active_stage(), location)) {
                return Err(crate::result::Suspension::Breakpoint);
            }

            match location {
                Location::AfterStatement(_) => self.clear_after_statement(),
                Location::BeforeStatement(statement) => {
                    if matches!(self.fuel, Some(0)) {
                        return Err(crate::result::Suspension::FuelExhausted);
                    }

                    if self.interrupt_requested {
                        return Err(crate::result::Suspension::HostInterrupt);
                    }

                    return Ok(Some(statement));
                }
            }
        }
    }

    fn stop_pending(&self) -> bool {
        self.last_stop.is_some()
    }

    fn take_stop(&mut self) -> Option<<Self::Machine as Machine<'ir>>::Stop> {
        self.last_stop.take()
    }

    fn finish_step(&mut self, statement: Statement) {
        if self.skip_finish_step {
            self.skip_finish_step = false;
            return;
        }

        if let Ok(activation) = self.current_activation_mut() {
            activation.after_statement = Some(statement);
        }
    }
}

impl<'ir, L, V, M, E> Interpreter<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect
        + 'ir
        + crate::Interpretable<
            'ir,
            SingleStage<'ir, L, V, M, E>,
            Effect = <M as Machine<'ir>>::Effect,
        >,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    type Machine = M;
    type Error = E;

    fn machine(&self) -> &Self::Machine {
        &self.machine
    }

    fn machine_mut(&mut self) -> &mut Self::Machine {
        &mut self.machine
    }

    fn interpret_current(
        &mut self,
    ) -> Result<<Self::Machine as Machine<'ir>>::Effect, <Self as Interpreter<'ir>>::Error> {
        let stage = self.stage_info_for(self.active_stage());
        let stmt = self.current_statement_result().map_err(E::from)?;
        let definition = stmt.definition(stage);
        definition.interpret(self).map_err(Into::into)
    }

    fn consume_effect(
        &mut self,
        effect: <Self::Machine as Machine<'ir>>::Effect,
    ) -> Result<Shell<<Self::Machine as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>
    {
        self.machine.consume_effect(effect).map_err(Into::into)
    }

    fn consume_control(
        &mut self,
        control: Shell<<Self::Machine as Machine<'ir>>::Stop>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        self.apply_control(control).map_err(Into::into)
    }
}

impl<'ir, L, V, M, E> Invoke<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect
        + 'ir
        + crate::Interpretable<
            'ir,
            SingleStage<'ir, L, V, M, E>,
            Effect = <M as Machine<'ir>>::Effect,
        >,
    V: Clone + crate::ProductValue + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    Self: ValueStore<Value = V, Error = E>,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    fn invoke(
        &mut self,
        callee: kirin_ir::SpecializedFunction,
        args: &[Self::Value],
        results: &[kirin_ir::ResultValue],
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        let continuation = self.prepare_invoke(results).map_err(E::from)?;
        let stage = self.active_stage();
        let entry = self.entry_block(callee).map_err(E::from)?;
        self.push_frame(callee, stage, entry.into(), Some(continuation))
            .map_err(E::from)?;

        if let Err(error) = self.bind_block_args(entry, args.iter().cloned()) {
            let _ = self.frames.pop::<InterpreterError>();
            return Err(error);
        }

        self.skip_finish_step = true;
        Ok(())
    }

    fn return_current(
        &mut self,
        value: Self::Value,
    ) -> Result<crate::effect::Flow<Self::Value>, <Self as Interpreter<'ir>>::Error> {
        let frame = self.frames.pop::<InterpreterError>().map_err(E::from)?;
        let (_, _, _, activation) = frame.into_parts();
        let Some(continuation) = activation.continuation else {
            self.skip_finish_step = true;
            return Ok(crate::effect::Flow::Stop(value));
        };

        self.restore_caller(&continuation).map_err(E::from)?;
        self.write_return_product(continuation.results(), value)?;
        self.skip_finish_step = true;
        Ok(crate::effect::Flow::Stay)
    }
}

impl<'ir, L, V, M, E> ResolveCallee<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect
        + 'ir
        + crate::Interpretable<
            'ir,
            SingleStage<'ir, L, V, M, E>,
            Effect = <M as Machine<'ir>>::Effect,
        >,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    fn resolve_query(
        &self,
        query: callee::Query,
        args: &[Self::Value],
    ) -> Result<kirin_ir::SpecializedFunction, <Self as Interpreter<'ir>>::Error> {
        let stage_id = match query.stage() {
            callee::Stage::Current => self.active_stage(),
            callee::Stage::Exact(stage) => stage,
        };

        let staged_function = match query.target() {
            callee::Target::Specialized(callee) => return Ok(callee),
            callee::Target::Staged(staged) => staged,
            callee::Target::Function(function) => match query.staged() {
                callee::StagedPolicy::ExactStage => {
                    self.lookup_staged(function, stage_id).map_err(E::from)?
                }
            },
            callee::Target::Symbol(symbol) => {
                let function = self.lookup_function(symbol, stage_id).map_err(E::from)?;
                match query.staged() {
                    callee::StagedPolicy::ExactStage => {
                        self.lookup_staged(function, stage_id).map_err(E::from)?
                    }
                }
            }
        };

        match query.specialization() {
            callee::SpecializationPolicy::UniqueLive => self
                .select_unique_live_specialization(staged_function, stage_id)
                .map_err(E::from),
            policy => {
                let _ = args;
                Err(InterpreterError::custom(std::io::Error::other(format!(
                    "{policy:?} callee resolution is not yet implemented in SingleStage",
                )))
                .into())
            }
        }
    }
}
