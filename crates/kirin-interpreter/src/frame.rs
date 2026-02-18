use fxhash::FxHashMap;
use kirin_ir::{ResultValue, SSAValue, SpecializedFunction, Statement};

/// A call frame for one [`SpecializedFunction`] invocation.
///
/// Stores the callee identity, per-frame SSA value bindings, and
/// interpreter-specific extra state `X`:
///
/// - [`StackInterpreter`](crate::StackInterpreter) uses
///   `Frame<V, Option<Statement>>` (instruction cursor).
/// - [`AbstractInterpreter`](crate::AbstractInterpreter) uses
///   `Frame<V, FixpointState<V>>` (worklist + block entry states).
#[derive(Debug)]
pub struct Frame<V, X> {
    callee: SpecializedFunction,
    values: FxHashMap<SSAValue, V>,
    extra: X,
}

// -- Common methods ---------------------------------------------------------

impl<V, X> Frame<V, X> {
    pub fn new(callee: SpecializedFunction, extra: X) -> Self {
        Self {
            callee,
            values: FxHashMap::default(),
            extra,
        }
    }

    pub fn with_values(
        callee: SpecializedFunction,
        values: FxHashMap<SSAValue, V>,
        extra: X,
    ) -> Self {
        Self {
            callee,
            values,
            extra,
        }
    }

    pub fn callee(&self) -> SpecializedFunction {
        self.callee
    }

    pub fn extra(&self) -> &X {
        &self.extra
    }

    pub fn extra_mut(&mut self) -> &mut X {
        &mut self.extra
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
    pub fn into_parts(self) -> (SpecializedFunction, FxHashMap<SSAValue, V>, X) {
        (self.callee, self.values, self.extra)
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
