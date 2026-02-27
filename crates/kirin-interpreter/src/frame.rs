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
