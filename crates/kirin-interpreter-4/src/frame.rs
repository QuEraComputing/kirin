use kirin_ir::{CompileStage, ResultValue, SSAValue, SpecializedFunction};
use rustc_hash::FxHashMap;

/// A call frame for one [`SpecializedFunction`] invocation.
///
/// Stores the callee identity, per-frame SSA value bindings, and
/// `caller_results` — the result slots where the return value should
/// be written when the frame is popped.
#[derive(Debug)]
pub struct Frame<V> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    caller_results: Vec<ResultValue>,
}

impl<V> Frame<V> {
    pub fn new(
        callee: SpecializedFunction,
        stage: CompileStage,
        caller_results: Vec<ResultValue>,
    ) -> Self {
        Self {
            callee,
            stage,
            values: FxHashMap::default(),
            caller_results,
        }
    }

    pub fn callee(&self) -> SpecializedFunction {
        self.callee
    }

    pub fn stage(&self) -> CompileStage {
        self.stage
    }

    pub fn caller_results(&self) -> &[ResultValue] {
        &self.caller_results
    }

    pub fn values(&self) -> &FxHashMap<SSAValue, V> {
        &self.values
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
    pub fn into_parts(
        self,
    ) -> (
        SpecializedFunction,
        CompileStage,
        FxHashMap<SSAValue, V>,
        Vec<ResultValue>,
    ) {
        (self.callee, self.stage, self.values, self.caller_results)
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
        let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
        let (spec, block, c0_result) = pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
            let sf = b.staged_function().new().unwrap();
            let c0 = Constant::<ArithValue, ArithType>::new(b, ArithValue::I64(0));
            let c0_result = c0.result;
            let ret = Return::<ArithType>::new(b, vec![c0_result.into()]);
            let block = b.block().stmt(c0).terminator(ret).new();
            let region = b.region().add_block(block).new();
            let body = FunctionBody::<ArithType>::new(
                b,
                region,
                kirin_ir::Signature::new(vec![], ArithType::default(), ()),
            );
            let spec = b.specialize().staged_func(sf).body(body).new().unwrap();
            (spec, block, c0_result)
        });
        (pipeline, stage_id, spec, block, c0_result)
    }

    #[test]
    fn test_frame_read_returns_none_for_missing() {
        let (_pipeline, stage_id, callee, _block, _result) = build_fixture();
        let frame: Frame<i32> = Frame::new(callee, stage_id, vec![]);

        let bogus = SSAValue::from(TestSSAValue(9999));
        assert!(frame.read(bogus).is_none());
    }

    #[test]
    fn test_frame_write_overwrites() {
        let (_pipeline, stage_id, callee, _block, result) = build_fixture();
        let mut frame: Frame<i32> = Frame::new(callee, stage_id, vec![]);

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
        let mut frame: Frame<i32> = Frame::new(callee, stage_id, vec![result]);
        frame.write(result, 42);

        let (got_callee, got_stage, got_values, got_caller_results) = frame.into_parts();
        assert_eq!(got_callee, callee);
        assert_eq!(got_stage, stage_id);
        assert_eq!(got_caller_results, vec![result]);
        let ssa: SSAValue = result.into();
        assert_eq!(got_values.get(&ssa), Some(&42));
    }

    #[test]
    fn test_frame_caller_results() {
        let (_pipeline, stage_id, callee, _block, result) = build_fixture();
        let frame: Frame<i32> = Frame::new(callee, stage_id, vec![result]);

        assert_eq!(frame.caller_results(), &[result]);
    }
}
