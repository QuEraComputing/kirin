use kirin_ir::{Block, CompileStage, Dialect, Pipeline, SSAValue, StageInfo, Statement};

use super::SingleStage;
use crate::{
    BlockSeed, ConsumeEffect, InterpreterError, Lift, Machine, StageAccess, ValueStore,
    control::{Breakpoint, Breakpoints, Directive, Fuel, Interrupt, Location},
    cursor::{ExecutionCursor, InternalSeed},
    interpreter::{
        Driver, Exec, Interpreter, Invoke, Position, ResolveCallee, TypedStage, callee, exec_block,
    },
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

// ---------------------------------------------------------------------------
// Machine<'ir> — SingleStage is a machine whose effect is Directive
// ---------------------------------------------------------------------------

impl<'ir, L, V, M, E> Machine<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    type Effect = Directive<<M as Machine<'ir>>::Stop, <M as Machine<'ir>>::Seed>;
    type Stop = <M as Machine<'ir>>::Stop;
    type Seed = <M as Machine<'ir>>::Seed;
}

// ---------------------------------------------------------------------------
// ConsumeEffect<'ir, ()> — terminal consumer of Directive effects
// ---------------------------------------------------------------------------

impl<'ir, L, V, M, E> ConsumeEffect<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: Clone + 'ir,
    M: Machine<'ir, Seed = BlockSeed<V>> + 'ir,
    E: From<InterpreterError> + 'ir,
{
    type Error = E;

    fn consume_effect(&mut self, directive: Self::Effect) -> Result<(), Self::Error> {
        match directive {
            Directive::Advance => {
                let stage = self.active_stage();
                let stage = self.stage_info_for(stage);
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                let cursor = activation
                    .cursor_stack
                    .last_mut()
                    .ok_or(InterpreterError::InvalidControl(
                        "advance requires an active cursor",
                    ))
                    .map_err(E::from)?;
                cursor.advance(stage);
                Ok(())
            }
            Directive::Stay => Ok(()),
            Directive::Push(seed) => {
                let (block, args) = seed.into_parts();
                let stage = self.active_stage();
                let internal_seed: InternalSeed = block.into();
                let next = ExecutionCursor::from_seed(self.stage_info_for(stage), internal_seed);
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                activation.cursor_stack.push(next);
                self.bind_block_args(block, args)?;
                Ok(())
            }
            Directive::Replace(seed) => {
                let (block, args) = seed.into_parts();
                self.replace_current_seed(block.into()).map_err(E::from)?;
                self.bind_block_args(block, args)?;
                Ok(())
            }
            Directive::Pop => {
                let activation = self.current_activation_mut().map_err(E::from)?;
                activation.after_statement = None;
                activation
                    .cursor_stack
                    .pop()
                    .map(|_| ())
                    .ok_or(InterpreterError::InvalidControl(
                        "pop requires an active cursor",
                    ))
                    .map_err(E::from)
            }
            Directive::Stop(stop) => {
                self.last_stop = Some(stop);
                self.clear_frames();
                self.skip_finish_step = false;
                Ok(())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Driver<'ir>
// ---------------------------------------------------------------------------

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
    M: Machine<'ir, Seed = BlockSeed<V>>
        + ConsumeEffect<'ir, Directive<<M as Machine<'ir>>::Stop, BlockSeed<V>>, Error: Into<E>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
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

    fn take_stop(&mut self) -> Option<Self::Stop> {
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

// ---------------------------------------------------------------------------
// Interpreter<'ir> — interpret_current encapsulates the full pipeline
// ---------------------------------------------------------------------------

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
    M: Machine<'ir, Seed = BlockSeed<V>>
        + ConsumeEffect<'ir, Directive<<M as Machine<'ir>>::Stop, BlockSeed<V>>, Error: Into<E>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
{
    fn interpret_current(&mut self) -> Result<Self::Effect, E> {
        let stage = self.stage_info_for(self.active_stage());
        let stmt = self.current_statement_result().map_err(E::from)?;
        let definition = stmt.definition(stage);
        let dialect_effect = definition.interpret(self).map_err(Into::into)?;
        let directive = self
            .machine
            .consume_effect(dialect_effect)
            .map_err(Into::into)?;
        Ok(Lift::lift(directive))
    }
}

// ---------------------------------------------------------------------------
// Exec, Invoke, ResolveCallee
// ---------------------------------------------------------------------------

impl<'ir, L, V, M, E> Exec<'ir, BlockSeed<V>> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect
        + 'ir
        + crate::Interpretable<
            'ir,
            SingleStage<'ir, L, V, M, E>,
            Effect = <M as Machine<'ir>>::Effect,
        >,
    V: Clone + crate::ProductValue + 'ir,
    M: Machine<'ir, Seed = BlockSeed<V>>
        + ConsumeEffect<'ir, Directive<<M as Machine<'ir>>::Stop, BlockSeed<V>>, Error: Into<E>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
{
    fn exec(&mut self, seed: BlockSeed<V>) -> Result<Option<V>, E> {
        exec_block(self, seed)
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
    M: Machine<'ir, Seed = BlockSeed<V>>
        + ConsumeEffect<'ir, Directive<<M as Machine<'ir>>::Stop, BlockSeed<V>>, Error: Into<E>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
{
    fn invoke(
        &mut self,
        callee: kirin_ir::SpecializedFunction,
        args: &[Self::Value],
        results: &[kirin_ir::ResultValue],
    ) -> Result<(), E> {
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
    ) -> Result<Directive<Self::Value, Self::Seed>, E> {
        let frame = self.frames.pop::<InterpreterError>().map_err(E::from)?;
        let (_, _, _, activation) = frame.into_parts();
        let Some(continuation) = activation.continuation else {
            self.skip_finish_step = true;
            return Ok(Directive::Stop(value));
        };

        self.restore_caller(&continuation).map_err(E::from)?;
        self.write_return_product(continuation.results(), value)?;
        self.skip_finish_step = true;
        Ok(Directive::Stay)
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
    M: Machine<'ir, Seed = BlockSeed<V>>
        + ConsumeEffect<'ir, Directive<<M as Machine<'ir>>::Stop, BlockSeed<V>>, Error: Into<E>>
        + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as crate::Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
{
    fn resolve_query(
        &self,
        query: callee::Query,
        args: &[Self::Value],
    ) -> Result<kirin_ir::SpecializedFunction, E> {
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
                Err(InterpreterError::message(format!(
                    "{policy:?} callee resolution is not yet implemented in SingleStage",
                ))
                .into())
            }
        }
    }
}
