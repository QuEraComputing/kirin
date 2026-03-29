mod activation;
mod frame_ops;
mod traits;

use std::marker::PhantomData;

use kirin_ir::{CompileStage, Dialect, Pipeline, StageInfo};
use rustc_hash::FxHashSet;

use crate::{FrameStack, Machine, control::Breakpoint};

/// Minimal concrete single-stage shell for the new machine design.
pub struct SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    pipeline: &'ir Pipeline<StageInfo<L>>,
    root_stage: CompileStage,
    machine: M,
    frames: FrameStack<V, activation::Activation>,
    breakpoints: FxHashSet<Breakpoint>,
    fuel: Option<u64>,
    interrupt_requested: bool,
    last_stop: Option<<M as Machine<'ir>>::Stop>,
    skip_finish_step: bool,
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
            root_stage: stage,
            machine,
            frames: FrameStack::new(),
            breakpoints: FxHashSet::default(),
            fuel: None,
            interrupt_requested: false,
            last_stop: None,
            skip_finish_step: false,
            _error: PhantomData,
        }
    }

    #[must_use]
    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    /// Access the inner dialect machine.
    pub fn machine(&self) -> &M {
        &self.machine
    }

    /// Mutably access the inner dialect machine.
    pub fn machine_mut(&mut self) -> &mut M {
        &mut self.machine
    }
}

// Projection from interpreter shell to inner machine.
impl<'ir, L, V, M, E> crate::ProjectMachine<M> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn project(&self) -> &M {
        &self.machine
    }
}

impl<'ir, L, V, M, E> crate::ProjectMachineMut<M> for SingleStage<'ir, L, V, M, E>
where
    L: Dialect + 'ir,
    V: 'ir,
    M: Machine<'ir> + 'ir,
    E: 'ir,
{
    fn project_mut(&mut self) -> &mut M {
        &mut self.machine
    }
}

#[cfg(test)]
mod tests {
    use super::SingleStage;
    use crate::{
        BlockSeed, ConsumeEffect, Interpretable, InterpreterError, control::Directive,
        interpreter::Position,
    };
    use kirin_ir::{CompileStage, GetInfo, Pipeline, StageInfo};
    use kirin_test_languages::CompositeLanguage;
    use kirin_test_utils::ir_fixtures::{build_linear_program, first_statement_of_specialization};

    #[derive(Debug, Default)]
    struct TestMachine;

    impl<'ir> crate::Machine<'ir> for TestMachine {
        type Effect = Directive<&'static str, BlockSeed<i64>>;
        type Stop = &'static str;
        type Seed = BlockSeed<i64>;
    }

    impl<'ir> crate::ConsumeEffect<'ir, Directive<&'static str, BlockSeed<i64>>> for TestMachine {
        type Error = InterpreterError;

        fn consume_effect(
            &mut self,
            effect: Self::Effect,
        ) -> Result<Directive<&'static str, BlockSeed<i64>>, Self::Error> {
            Ok(effect)
        }
    }

    type TestInterp<'ir> = SingleStage<'ir, CompositeLanguage, i64, TestMachine, InterpreterError>;

    impl<'ir> Interpretable<'ir, TestInterp<'ir>> for CompositeLanguage {
        type Effect = Directive<&'static str, BlockSeed<i64>>;
        type Error = InterpreterError;

        fn interpret(&self, _: &mut TestInterp<'ir>) -> Result<Self::Effect, Self::Error> {
            unreachable!("tests do not interpret statements")
        }
    }

    #[test]
    fn push_specialization_seeds_first_statement() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;
        let expected = first_statement_of_specialization(&pipeline, stage_id, spec_fn);

        let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);

        interp.push_specialization(spec_fn).unwrap();

        assert_eq!(interp.cursor_depth(), 1);
        assert_eq!(interp.current_statement(), expected);
    }

    #[test]
    fn consume_effect_advance_walks_block_and_then_exhausts() {
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

        let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
        interp.push_specialization(spec_fn).unwrap();

        assert_eq!(interp.current_statement(), Some(first));

        interp.consume_effect(Directive::Advance).unwrap();
        assert_eq!(interp.current_statement(), second);

        interp.consume_effect(Directive::Advance).unwrap();
        assert_eq!(interp.current_statement(), third);

        interp.consume_effect(Directive::Advance).unwrap();
        assert_eq!(interp.current_statement(), terminator);

        interp.consume_effect(Directive::Advance).unwrap();
        assert_eq!(interp.current_statement(), None);
    }

    #[test]
    fn stop_clears_cursor_stack_and_records_stop() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let spec_fn = build_linear_program(&mut pipeline, stage_id).0;

        let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
        interp.push_specialization(spec_fn).unwrap();

        interp.consume_effect(Directive::Stop("done")).unwrap();

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
        let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);

        let error = interp
            .consume_effect(Directive::Replace(block.into()))
            .unwrap_err();

        assert!(matches!(
            error,
            InterpreterError::NoFrame
                | InterpreterError::InvalidControl("replace requires an active cursor")
        ));
    }

    #[test]
    fn pop_without_active_cursor_errors() {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
        let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);

        let error = interp.consume_effect(Directive::Pop).unwrap_err();

        assert!(matches!(
            error,
            InterpreterError::NoFrame
                | InterpreterError::InvalidControl("pop requires an active cursor")
        ));
    }
}
