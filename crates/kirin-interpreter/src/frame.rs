use kirin_ir::{CompileStage, ResultValue, SSAValue, SpecializedFunction, Statement};
use rustc_hash::FxHashMap;

/// A call frame for one [`SpecializedFunction`] invocation.
///
/// Stores the callee identity, per-frame SSA value bindings, and
/// interpreter-specific extra state `X`:
///
/// - [`StackInterpreter`](crate::StackInterpreter) uses
///   `Frame<V, Option<Statement>>` (instruction cursor).
/// - [`AbstractInterpreter`](crate::AbstractInterpreter) uses
///   `Frame<V, FixpointState>` (worklist + block arg tracking).
#[derive(Debug)]
pub struct Frame<V, X> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    extra: X,
}

// -- Common methods ---------------------------------------------------------

impl<V, X> Frame<V, X> {
    pub fn new(callee: SpecializedFunction, stage: CompileStage, extra: X) -> Self {
        Self {
            callee,
            stage,
            values: FxHashMap::default(),
            extra,
        }
    }

    pub fn with_values(
        callee: SpecializedFunction,
        stage: CompileStage,
        values: FxHashMap<SSAValue, V>,
        extra: X,
    ) -> Self {
        Self {
            callee,
            stage,
            values,
            extra,
        }
    }

    pub fn callee(&self) -> SpecializedFunction {
        self.callee
    }

    pub fn stage(&self) -> CompileStage {
        self.stage
    }

    pub fn extra(&self) -> &X {
        &self.extra
    }

    pub fn extra_mut(&mut self) -> &mut X {
        &mut self.extra
    }

    pub fn values(&self) -> &FxHashMap<SSAValue, V> {
        &self.values
    }

    /// Simultaneous mutable access to the values map and extra state.
    pub fn values_and_extra_mut(&mut self) -> (&mut FxHashMap<SSAValue, V>, &mut X) {
        (&mut self.values, &mut self.extra)
    }

    pub fn read(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }

    pub fn write(&mut self, result: ResultValue, value: V) -> Option<V> {
        self.values.insert(result.into(), value)
    }

    /// Write a value keyed by an arbitrary [`SSAValue`] (e.g. block arguments).
    pub fn write_ssa(&mut self, ssa: SSAValue, value: V) -> Option<V> {
        self.values.insert(ssa, value)
    }

    /// Consume the frame, returning its constituent parts.
    pub fn into_parts(self) -> (SpecializedFunction, CompileStage, FxHashMap<SSAValue, V>, X) {
        (self.callee, self.stage, self.values, self.extra)
    }
}

// -- Cursor methods for StackInterpreter ------------------------------------

impl<V> Frame<V, Option<Statement>> {
    pub fn cursor(&self) -> Option<Statement> {
        self.extra
    }

    pub fn set_cursor(&mut self, cursor: Option<Statement>) {
        self.extra = cursor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_arith::{ArithType, ArithValue};
    use kirin_constant::Constant;
    use kirin_function::{FunctionBody, Return};
    use kirin_ir::{Pipeline, StageInfo, TestSSAValue};
    use kirin_test_languages::CompositeLanguage;

    fn build_fixture() -> (
        Pipeline<StageInfo<CompositeLanguage>>,
        CompileStage,
        SpecializedFunction,
        kirin_ir::Block,
        ResultValue,
    ) {
        let mut pipeline: Pipeline<StageInfo<CompositeLanguage>> = Pipeline::new();
        let stage_id = pipeline.add_stage(StageInfo::default(), None::<&str>);
        let (spec, block, c0_result) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let sf = b.staged_function(None, None, None, None).unwrap();
            let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
            let c0_result = c0.result;
            let ret = Return::<ArithType>::new(b, c0_result);
            let block = b.block().stmt(c0).terminator(ret).new();
            let region = b.region().add_block(block).new();
            let body = FunctionBody::<ArithType>::new(b, region);
            let spec = b.specialize(sf, None, body, None).unwrap();
            (spec, block, c0_result)
        });
        (pipeline, stage_id, spec, block, c0_result)
    }

    #[test]
    fn test_frame_read_returns_none_for_missing() {
        let (_pipeline, stage_id, callee, _block, _result) = build_fixture();
        let frame: Frame<i32, ()> = Frame::new(callee, stage_id, ());

        let bogus = SSAValue::from(TestSSAValue(9999));
        assert!(frame.read(bogus).is_none());
    }

    #[test]
    fn test_frame_write_overwrites() {
        let (_pipeline, stage_id, callee, _block, result) = build_fixture();
        let mut frame: Frame<i32, ()> = Frame::new(callee, stage_id, ());

        let old = frame.write(result, 10);
        assert!(old.is_none(), "first write should return None");

        let old = frame.write(result, 20);
        assert_eq!(old, Some(10), "second write should return previous value");

        let ssa: SSAValue = result.into();
        assert_eq!(frame.read(ssa), Some(&20));
    }

    #[test]
    fn test_frame_into_parts() {
        let (_pipeline, stage_id, callee, _block, result) = build_fixture();
        let mut frame: Frame<i32, String> = Frame::new(callee, stage_id, "extra".to_string());
        frame.write(result, 42);

        let (got_callee, got_stage, got_values, got_extra) = frame.into_parts();
        assert_eq!(got_callee, callee);
        assert_eq!(got_stage, stage_id);
        assert_eq!(got_extra, "extra");
        let ssa: SSAValue = result.into();
        assert_eq!(got_values.get(&ssa), Some(&42));
    }

    #[test]
    fn test_frame_cursor_methods() {
        let (pipeline, stage_id, callee, block, _result) = build_fixture();
        let mut frame: Frame<i32, Option<Statement>> = Frame::new(callee, stage_id, None);

        assert_eq!(frame.cursor(), None);

        let stage = pipeline.stage(stage_id).unwrap();
        let stmt = block
            .first_statement(stage)
            .expect("fixture has statements");
        frame.set_cursor(Some(stmt));
        assert_eq!(frame.cursor(), Some(stmt));

        frame.set_cursor(None);
        assert_eq!(frame.cursor(), None);
    }
}
