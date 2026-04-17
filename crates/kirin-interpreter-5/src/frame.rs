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
