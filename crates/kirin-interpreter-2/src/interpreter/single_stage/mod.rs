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
        type Seed = kirin_ir::Block;
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
            InterpreterError::NoFrame
                | InterpreterError::InvalidControl("replace requires an active cursor")
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
            InterpreterError::NoFrame
                | InterpreterError::InvalidControl("pop requires an active cursor")
        ));
    }
}
