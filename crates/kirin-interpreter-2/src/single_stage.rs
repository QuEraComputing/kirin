use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, GetInfo, Pipeline, SSAValue, SpecializedFunction, StageInfo,
    Statement,
};
use rustc_hash::FxHashMap;

use crate::{
    ConsumeEffect, Control, ExecutionSeed, Interpretable, Interpreter, InterpreterError, Machine,
    StageAccess, ValueStore, cursor::BlockCursor,
};

/// Minimal concrete single-stage shell for the new machine design.
pub struct SingleStageInterpreter<'ir, L, V, M, E>
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
    cursor_stack: Vec<BlockCursor>,
    last_stop: Option<<M as Machine<'ir>>::Stop>,
    _error: PhantomData<fn() -> E>,
}

impl<'ir, L, V, M, E> SingleStageInterpreter<'ir, L, V, M, E>
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
            last_stop: None,
            _error: PhantomData,
        }
    }

    pub fn stage_info(&self) -> &'ir StageInfo<L> {
        self.pipeline
            .stage(self.stage)
            .expect("single-stage interpreter points at a missing stage")
    }

    pub fn cursor_depth(&self) -> usize {
        self.cursor_stack.len()
    }

    pub fn current_block(&self) -> Option<Block> {
        self.cursor_stack.last().map(BlockCursor::block)
    }

    pub fn current_statement(&self) -> Option<Statement> {
        self.cursor_stack.last().and_then(BlockCursor::current)
    }

    pub fn last_stop(&self) -> Option<&<M as Machine<'ir>>::Stop> {
        self.last_stop.as_ref()
    }

    pub fn take_stop(&mut self) -> Option<<M as Machine<'ir>>::Stop> {
        self.last_stop.take()
    }

    pub fn clear_values(&mut self) {
        self.values.clear();
    }

    pub fn clear_cursor_stack(&mut self) {
        self.cursor_stack.clear();
    }

    pub fn push_block(&mut self, block: Block) {
        self.cursor_stack
            .push(BlockCursor::new(self.stage_info(), block));
    }

    pub fn push_seed(&mut self, seed: ExecutionSeed) {
        match seed {
            ExecutionSeed::Block(seed) => self.push_block(seed.block()),
        }
    }

    pub fn replace_seed(&mut self, seed: ExecutionSeed) -> Result<(), InterpreterError> {
        let next_cursor = match seed {
            ExecutionSeed::Block(seed) => BlockCursor::new(self.stage_info(), seed.block()),
        };

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
        let stage = self.stage_info();
        let spec_info = callee.expect_info(stage);
        let body = *spec_info.body();
        let region = body
            .regions(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)?;
        let block = region
            .blocks(stage)
            .next()
            .ok_or_else(InterpreterError::missing_entry_block)?;
        self.push_block(block);
        Ok(())
    }

    pub fn apply_control(
        &mut self,
        control: Control<<M as Machine<'ir>>::Stop>,
    ) -> Result<(), InterpreterError> {
        match control {
            Control::Advance => {
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
            Control::Stay => Ok(()),
            Control::Push(seed) => {
                self.push_seed(seed);
                Ok(())
            }
            Control::Replace(seed) => self.replace_seed(seed),
            Control::Pop => {
                self.cursor_stack
                    .pop()
                    .map(|_| ())
                    .ok_or(InterpreterError::InvalidControl(
                        "pop requires an active cursor",
                    ))
            }
            Control::Stop(stop) => {
                self.last_stop = Some(stop);
                self.cursor_stack.clear();
                Ok(())
            }
        }
    }
}

impl<'ir, L, V, M, E> ValueStore for SingleStageInterpreter<'ir, L, V, M, E>
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

impl<'ir, L, V, M, E> StageAccess<'ir> for SingleStageInterpreter<'ir, L, V, M, E>
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

impl<'ir, L, V, M, E> Interpreter<'ir> for SingleStageInterpreter<'ir, L, V, M, E>
where
    L: Dialect + 'ir + Interpretable<'ir, SingleStageInterpreter<'ir, L, V, M, E>, Machine = M>,
    V: Clone + 'ir,
    M: Machine<'ir> + ConsumeEffect<'ir> + 'ir,
    E: From<InterpreterError> + 'ir,
    <L as Interpretable<'ir, SingleStageInterpreter<'ir, L, V, M, E>>>::Error: Into<E>,
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
    ) -> Result<Control<<Self::Machine as Machine<'ir>>::Stop>, <Self as Interpreter<'ir>>::Error>
    {
        self.machine.consume_effect(effect).map_err(Into::into)
    }

    fn consume_control(
        &mut self,
        control: Control<<Self::Machine as Machine<'ir>>::Stop>,
    ) -> Result<(), <Self as Interpreter<'ir>>::Error> {
        self.apply_control(control).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::SingleStageInterpreter;
    use crate::{Control, InterpreterError};
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

        let mut interp = SingleStageInterpreter::<_, i64, _, InterpreterError>::new(
            &pipeline,
            stage_id,
            TestMachine,
        );

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

        let mut interp = SingleStageInterpreter::<_, i64, _, InterpreterError>::new(
            &pipeline,
            stage_id,
            TestMachine,
        );
        interp.push_specialization(spec_fn).unwrap();

        assert_eq!(interp.current_statement(), Some(first));

        interp.apply_control(Control::Advance).unwrap();
        assert_eq!(interp.current_statement(), second);

        interp.apply_control(Control::Advance).unwrap();
        assert_eq!(interp.current_statement(), third);

        interp.apply_control(Control::Advance).unwrap();
        assert_eq!(interp.current_statement(), terminator);

        interp.apply_control(Control::Advance).unwrap();
        assert_eq!(interp.current_statement(), None);
    }

    #[test]
    fn stop_clears_cursor_stack_and_records_stop() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

        let mut interp = SingleStageInterpreter::<_, i64, _, InterpreterError>::new(
            &pipeline,
            stage_id,
            TestMachine,
        );
        interp.push_specialization(spec_fn).unwrap();

        interp.apply_control(Control::Stop("done")).unwrap();

        assert_eq!(interp.cursor_depth(), 0);
        assert_eq!(interp.last_stop(), Some(&"done"));
    }
}
