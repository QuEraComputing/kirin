use kirin_ir::{CompileStage, ResultValue, SSAValue, SpecializedFunction};
use rustc_hash::FxHashMap;

/// Per-invocation storage for stage-local shells.
#[derive(Debug)]
pub struct Frame<V, X> {
    callee: SpecializedFunction,
    stage: CompileStage,
    values: FxHashMap<SSAValue, V>,
    extra: X,
}

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

    pub fn values_and_extra_mut(&mut self) -> (&mut FxHashMap<SSAValue, V>, &mut X) {
        (&mut self.values, &mut self.extra)
    }

    pub fn read(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }

    pub fn write(&mut self, result: ResultValue, value: V) -> Option<V> {
        self.values.insert(result.into(), value)
    }

    pub fn write_ssa(&mut self, value: SSAValue, result: V) -> Option<V> {
        self.values.insert(value, result)
    }

    pub fn into_parts(self) -> (SpecializedFunction, CompileStage, FxHashMap<SSAValue, V>, X) {
        (self.callee, self.stage, self.values, self.extra)
    }
}
