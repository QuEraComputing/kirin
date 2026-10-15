//! Fresh result values for inserted statements.
//!
//! A definition names its results by id, so the ids must exist before the
//! definition can be built, while the statement that defines them does not
//! exist until it is inserted. [`Rewriter::new_result`] bridges the gap: it
//! allocates an id whose arena slot stays empty (so the value reads as not
//! live and cannot be used yet) and keeps its type until an insert binds it.

use crate::{Dialect, ResultValue, SSAInfo, SSAKind, SSAValue, Statement};

use crate::rewrite::rewriter::{RewriteError, Rewriter};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Allocate a result value of type `ty` for a statement not inserted yet.
    ///
    /// Put it in a result slot of the definition passed to an insert; the
    /// insert makes it a result of the new statement. Until then the value is
    /// not live: using it as an operand is rejected with
    /// [`RewriteError::UnknownValue`]. A value this `Rewriter` never binds
    /// stays dead.
    pub fn new_result(&mut self, ty: L::Type) -> ResultValue {
        let value = self.stage.ssas.alloc(None);
        self.unbound_ssa_results.insert(value, ty);
        value.into()
    }

    /// The result values of `definition`, checked bindable: each one came from
    /// this `Rewriter`'s [`new_result`](Rewriter::new_result), is not bound
    /// yet, and appears once. Otherwise [`RewriteError::ResultNotFresh`].
    pub(super) fn fresh_results(&self, definition: &L) -> Result<Vec<SSAValue>, RewriteError> {
        let mut results: Vec<SSAValue> = Vec::new();
        for result in definition.results() {
            let value = SSAValue::from(*result);
            if !self.unbound_ssa_results.contains_key(&value) || results.contains(&value) {
                return Err(RewriteError::ResultNotFresh(value));
            }
            results.push(value);
        }
        Ok(results)
    }

    /// Make `results` (from [`fresh_results`](Rewriter::fresh_results)) the
    /// results of the newly allocated `stmt`, in order.
    pub(super) fn bind_results(&mut self, stmt: Statement, results: Vec<SSAValue>) {
        for (index, value) in results.into_iter().enumerate() {
            let ty = self
                .unbound_ssa_results
                .remove(&value)
                .expect("result checked fresh before binding");
            let slot = self
                .stage
                .ssas
                .get_mut(value)
                .expect("unbound result has an arena slot");
            slot.data = Some(SSAInfo::new(value, None, ty, SSAKind::Result(stmt, index)));
        }
    }
}
