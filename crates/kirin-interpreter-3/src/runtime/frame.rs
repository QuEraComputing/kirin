use kirin_ir::{Product, ResultValue, SSAValue};
use rustc_hash::FxHashMap;

use super::cursor::ExecutionCursor;

#[derive(Debug, Clone)]
pub(crate) struct Continuation {
    resume: ExecutionCursor,
    results: Product<ResultValue>,
}

impl Continuation {
    pub(crate) const fn resume(&self) -> ExecutionCursor {
        self.resume
    }

    pub(crate) fn results(&self) -> &Product<ResultValue> {
        &self.results
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Frame<V> {
    cursor: ExecutionCursor,
    values: FxHashMap<SSAValue, V>,
    continuation: Option<Continuation>,
}

impl<V> Frame<V> {
    pub(crate) fn new(cursor: ExecutionCursor, continuation: Option<Continuation>) -> Self {
        Self {
            cursor,
            values: FxHashMap::default(),
            continuation,
        }
    }

    pub(crate) const fn cursor(&self) -> &ExecutionCursor {
        &self.cursor
    }

    pub(crate) fn cursor_mut(&mut self) -> &mut ExecutionCursor {
        &mut self.cursor
    }

    pub(crate) fn read(&self, value: SSAValue) -> Option<&V> {
        self.values.get(&value)
    }

    pub(crate) fn write_ssa(&mut self, value: SSAValue, result: V) {
        self.values.insert(value, result);
    }

    pub(crate) fn continuation(&self) -> Option<&Continuation> {
        self.continuation.as_ref()
    }
}
