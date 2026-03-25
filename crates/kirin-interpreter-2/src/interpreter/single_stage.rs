use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, Pipeline, SSAValue, SpecializedFunction, StageInfo,
    Statement,
};
use rustc_hash::{FxHashMap, FxHashSet};

use super::{Driver, Interpreter, Position};
use crate::{
    BlockSeed, ConsumeEffect, ExecutionSeed, Interpretable, InterpreterError, Machine, StageAccess,
    ValueStore,
    control::{Breakpoint, Breakpoints, Fuel, Interrupt, Location, Shell},
    cursor::ExecutionCursor,
};

/// Minimal concrete single-stage shell for the new machine design.
pub struct SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    pipeline: &'ir Pipeline<StageInfo<L>>,
    stage: CompileStage,
    machine: M,
    values: FxHashMap<SSAValue, V>,
    cursor_stack: Vec<ExecutionCursor>,
    after_statement: Option<Statement>,
    breakpoints: FxHashSet<Breakpoint>,
    fuel: Option<u64>,
    interrupt_requested: bool,
    last_stop: Option<<M as Machine<'ir>>::Stop>,
    _error: PhantomData<fn() -> E>,
}

impl<'ir, L, V, M, E> SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    pub fn new(pipeline: &'ir Pipeline<StageInfo<L>>, stage: CompileStage, machine: M) -> Self {
        Self {
            pipeline,
            stage,
            machine,
            values: FxHashMap::default(),
            cursor_stack: Vec::new(),
            after_statement: None,
            breakpoints: FxHashSet::default(),
            fuel: None,
            interrupt_requested: false,
            last_stop: None,
            _error: PhantomData,
        }
    }

    pub fn with_values(
        pipeline: &'ir Pipeline<StageInfo<L>>,
        stage: CompileStage,
        machine: M,
        values: FxHashMap<SSAValue, V>,
    ) -> Self {
        Self {
            pipeline,
            stage,
            machine,
            values,
            cursor_stack: Vec::new(),
            after_statement: None,
            breakpoints: FxHashSet::default(),
            fuel: None,
            interrupt_requested: false,
            last_stop: None,
            _error: PhantomData,
        }
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    pub fn stage_info(&self) -> &'ir StageInfo<L> {
        self.pipeline
            .stage(self.stage)
            .expect("single-stage interpreter points at a missing stage")
    }

    pub fn last_stop(&self) -> Option<&<M as Machine<'ir>>::Stop> {
        self.last_stop.as_ref()
    }

    pub fn clear_values(&mut self) {
        self.values.clear();
    }

    pub fn take_value_bindings(&mut self) -> Vec<(SSAValue, V)> {
        self.values.drain().collect()
    }

    pub fn replace_value_bindings(&mut self, bindings: Vec<(SSAValue, V)>) -> Vec<(SSAValue, V)> {
        let current = self.take_value_bindings();
        self.values.extend(bindings);
        current
    }

    pub fn clear_cursor_stack(&mut self) {
        self.cursor_stack.clear();
        self.after_statement = None;
    }

    pub fn resume_seed_after_current(&self) -> Result<ExecutionSeed, InterpreterError> {
        let stage = self.stage_info();
        let statement = self
            .current_statement()
            .ok_or(InterpreterError::NoCurrentStatement)?;
        let block = self
            .current_block()
            .ok_or(InterpreterError::InvalidControl(
                "current statement is not block-local",
            ))?;
        let next = (*statement.next(stage)).or_else(|| block.terminator(stage));

        Ok(match next {
            Some(statement) => BlockSeed::at_statement(block, statement).into(),
            None => BlockSeed::exhausted(block).into(),
        })
    }

    fn clear_after_statement(&mut self) {
        self.after_statement = None;
    }

    pub fn push_block(&mut self, block: Block) {
        self.push_seed(block.into());
    }

    pub fn push_seed(&mut self, seed: ExecutionSeed) {
        self.clear_after_statement();
        self.cursor_stack
            .push(ExecutionCursor::from_seed(self.stage_info(), seed));
    }

    pub fn replace_seed(&mut self, seed: ExecutionSeed) -> Result<(), InterpreterError> {
        let next_cursor = ExecutionCursor::from_seed(self.stage_info(), seed);

        self.clear_after_statement();
        let cursor = self
            .cursor_stack
            .last_mut()
            .ok_or(InterpreterError::InvalidControl(
                "replace requires an active cursor",
            ))?;

        *cursor = next_cursor;
        Ok(())
    }

    pub fn push_specialization(
        &mut self,
        callee: SpecializedFunction,
    ) -> Result<(), InterpreterError> {
        let block = self.entry_block(callee)?;
        self.push_block(block);
        Ok(())
    }

    pub fn entry_block(&self, callee: SpecializedFunction) -> Result<Block, InterpreterError> {
        let stage = self.stage_info();
        let spec_info = callee.expect_info(stage);
        let body = *spec_info.body();
        let region = body
            .regions(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)?;

        region
            .blocks(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)
    }

    pub fn bind_block_args(&mut self, block: Block, args: &[V]) -> Result<(), E>
    where
        V: Clone,
        E: From<InterpreterError>,
    {
        let stage = self.stage_info();
        let block_info = block.expect_info(stage);
        if block_info.arguments.len() != args.len() {
            return Err(InterpreterError::ArityMismatch {
                expected: block_info.arguments.len(),
                got: args.len(),
            }
            .into());
        }

        for (argument, value) in block_info.arguments.iter().zip(args.iter()) {
            self.write(SSAValue::from(*argument), value.clone())?;
        }

        Ok(())
    }

    pub fn start_specialization(&mut self, callee: SpecializedFunction, args: &[V]) -> Result<(), E>
    where
        V: Clone,
        E: From<InterpreterError>,
    {
        self.clear_values();
        self.clear_cursor_stack();
        self.last_stop = None;

        let entry = self.entry_block(callee)?;
        self.push_block(entry);
        self.bind_block_args(entry, args)
    }

    pub fn apply_control(
        &mut self,
        control: Shell<<M as Machine<'ir>>::Stop>,
    ) -> Result<(), InterpreterError> {
        self.clear_after_statement();
        match control {
            Shell::Advance => {
                let stage = self.stage_info();
                let cursor =
                    self.cursor_stack
                        .last_mut()
                        .ok_or(InterpreterError::InvalidControl(
                            "advance requires an active cursor",
                        ))?;
                cursor.advance(stage);
                Ok(())
            }
            Shell::Stay => Ok(()),
            Shell::Push(seed) => {
                self.push_seed(seed);
                Ok(())
            }
            Shell::Replace(seed) => self.replace_seed(seed),
            Shell::Pop => {
                self.cursor_stack
                    .pop()
                    .map(|_| ())
                    .ok_or(InterpreterError::InvalidControl(
                        "pop requires an active cursor",
                    ))
            }
            Shell::Stop(stop) => {
                self.last_stop = Some(stop);
                self.cursor_stack.clear();
                Ok(())
            }
        }
    }

    pub fn run_specialization(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<crate::result::Run<<M as Machine<'ir>>::Stop>, E>
    where
        V: Clone,
        M: ConsumeEffect<'ir, Error = E>,
        L: Interpretable<'ir, Self, Machine = M, Error = E>,
        E: From<InterpreterError>,
    {
        self.start_specialization(callee, args)?;
        <Self as Driver<'ir>>::run(self)
    }
}

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
        self.cursor_stack.len()
    }

    fn current_block(&self) -> Option<Block> {
        self.cursor_stack
            .last()
            .and_then(ExecutionCursor::current_block)
    }

    fn current_statement(&self) -> Option<Statement> {
        self.cursor_stack.last().and_then(ExecutionCursor::current)
    }

    fn current_location(&self) -> Option<Location> {
        self.after_statement
            .map(Location::AfterStatement)
            .or_else(|| self.current_statement().map(Location::BeforeStatement))
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
        self.values
            .get(&value)
            .cloned()
            .ok_or_else(|| InterpreterError::UnboundValue(value).into())
    }

    fn write(
        &mut self,
        target: impl Into<SSAValue>,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        self.values.insert(target.into(), value);
        Ok(())
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
        self.stage
    }
}

impl<'ir, L, V, M, E> Driver<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir + Interpretable<'ir, SingleStage<'ir, L, V, M, E>, Machine = M>,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
    <M as ConsumeEffect<'ir>>::Error: Into<E>,
{
    fn poll_execution_gate(&mut self) -> Result<Option<Statement>, crate::result::Suspension> {
        loop {
            let Some(location) = self.current_location() else {
                return Ok(None);
            };

            if self.has_breakpoint(&Breakpoint::new(self.stage, location)) {
                return Err(crate::result::Suspension::Breakpoint);
            }

            match location {
                Location::AfterStatement(_) => {
                    self.clear_after_statement();
                }
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
        self.after_statement = Some(statement);
    }
}

impl<'ir, L, V, M, E> Interpreter<'ir> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir + Interpretable<'ir, SingleStage<'ir, L, V, M, E>, Machine = M>,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as Interpretable<'ir, SingleStage<'ir, L, V, M, E>>>::Error: Into<E>,
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
        let stage = self.stage_info();
        let stmt = self
            .current_statement()
            .ok_or(InterpreterError::NoCurrentStatement)
            .map_err(E::from)?;
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

#[cfg(test)]
mod tests {
    use super::SingleStage;
    use crate::{InterpreterError, control::Shell, interpreter::Position};
    use kirin_ir::{CompileStage, GetInfo, Pipeline, StageInfo};
    use kirin_test_languages::CompositeLanguage;
    use kirin_test_utils::ir_fixtures::{build_linear_program, first_statement_of_specialization};

    #[derive(Debug, Default)]
    struct TestMachine;

    impl<'ir> crate::Machine<'ir> for TestMachine {
        type Effect = ();
        type Stop = &'static str;
    }

    #[test]
    fn push_specialization_seeds_first_statement() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;
        let expected = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

        let mut interp =
            SingleStage::<_, i64, _, InterpreterError>::new(&pipeline, stage_id, TestMachine);

        interp.push_specialization(spec_fn).unwrap();

        assert_eq!(interp.cursor_depth(), 1);
        assert_eq!(interp.current_statement(), expected);
    }

    #[test]
    fn apply_control_advance_walks_block_and_then_exhausts() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;
        let stage = pipeline.stage(stage_id).unwrap();
        let first = first_statement_of_specialization(&pipeline, stage_id, spec_fn).unwrap();
        let second = *first.next(stage);
        let third = second.and_then(|stmt| *stmt.next(stage));
        let terminator = {
            let spec_info = spec_fn.expect_info(stage);
            let body = *spec_info.body();
            let region = body.regions(stage).next().unwrap();
            let block = region.blocks(stage).next().unwrap();
            block.terminator(stage)
        };

        let mut interp =
            SingleStage::<_, i64, _, InterpreterError>::new(&pipeline, stage_id, TestMachine);
        interp.push_specialization(spec_fn).unwrap();

        assert_eq!(interp.current_statement(), Some(first));

        interp.apply_control(Shell::Advance).unwrap();
        assert_eq!(interp.current_statement(), second);

        interp.apply_control(Shell::Advance).unwrap();
        assert_eq!(interp.current_statement(), third);

        interp.apply_control(Shell::Advance).unwrap();
        assert_eq!(interp.current_statement(), terminator);

        interp.apply_control(Shell::Advance).unwrap();
        assert_eq!(interp.current_statement(), None);
    }

    #[test]
    fn stop_clears_cursor_stack_and_records_stop() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

        let mut interp =
            SingleStage::<_, i64, _, InterpreterError>::new(&pipeline, stage_id, TestMachine);
        interp.push_specialization(spec_fn).unwrap();

        interp.apply_control(Shell::Stop("done")).unwrap();

        assert_eq!(interp.cursor_depth(), 0);
        assert_eq!(interp.last_stop(), Some(&"done"));
    }

    #[test]
    fn replace_without_active_cursor_errors() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let block = pipeline
            .stage_mut(stage_id)
            .unwrap()
            .with_builder(|b| b.block().new());
        let mut interp =
            SingleStage::<_, i64, _, InterpreterError>::new(&pipeline, stage_id, TestMachine);

        let error = interp
            .apply_control(Shell::Replace(block.into()))
            .unwrap_err();

        assert!(matches!(
            error,
            InterpreterError::InvalidControl("replace requires an active cursor")
        ));
    }

    #[test]
    fn pop_without_active_cursor_errors() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let mut interp =
            SingleStage::<_, i64, _, InterpreterError>::new(&pipeline, stage_id, TestMachine);

        let error = interp.apply_control(Shell::Pop).unwrap_err();

        assert!(matches!(
            error,
            InterpreterError::InvalidControl("pop requires an active cursor")
        ));
    }
}
