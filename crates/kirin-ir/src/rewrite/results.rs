//! Fresh result values for built statements.
//!
//! A definition names its results by id, so the ids must exist before the
//! definition can be written, while the statement that defines them does not
//! exist until it is built. [`Rewriter::new_result`] bridges the gap: it
//! allocates an id whose arena slot stays empty (so the value reads as not live
//! and cannot be used yet) and keeps its type until [`Rewriter::create`] binds
//! it. Dropping the `Rewriter` tombstones every value it never bound, so once
//! it is gone each result slot is either bound or a real tombstone. It also
//! tombstones every detached statement it never inserted.

use crate::{Body, Dialect, ResultValue, SSAInfo, SSAKind, SSAValue, Statement};

use crate::rewrite::rewriter::{RewriteError, Rewriter};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Allocate a result value of type `ty` for a statement not built yet.
    ///
    /// Put it in a result slot of the definition passed to
    /// [`Rewriter::create`], which makes it a result of the new statement.
    /// Until then the value is not live: using it as an operand is rejected
    /// with [`RewriteError::UnknownValue`]. A value this `Rewriter` never binds
    /// is tombstoned when the `Rewriter` is dropped.
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

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Leave detached code in place when this `Rewriter` drops. Called by the
    /// pass boundary after it catches a panic.
    pub(crate) fn keep_detached_on_drop(&mut self) {
        self.keep_detached_on_drop = true;
    }
}

impl<'a, L: Dialect> Drop for Rewriter<'a, L> {
    /// Tombstone every detached statement no [`insert`](Rewriter::insert)
    /// placed, with its results, operand uses and bodies, every
    /// [`new_block`](Rewriter::new_block) block no statement owns, every
    /// detached CFG (such as a [`clone_cfg`](Rewriter::clone_cfg) copy) no
    /// statement adopted, and every
    /// [`new_result`](Rewriter::new_result) value no statement bound, so
    /// nothing detached or empty outlives the `Rewriter`.
    ///
    /// After a panic the detached statements and bodies are left in place: a
    /// panic part-way through an edit can leave them inconsistent, so
    /// tombstoning them could panic again. Two cases are covered. When this
    /// `Rewriter` is dropped while a panic unwinds, a second panic would abort
    /// the process. When the pass boundary has caught the panic, the unwind is
    /// over before this drop runs, so it sets `keep_detached_on_drop` instead;
    /// a second panic would escape the boundary and lose the quarantined
    /// stage, which keeps the detached code for inspection.
    fn drop(&mut self) {
        if !self.keep_detached_on_drop && !std::thread::panicking() {
            for stmt in std::mem::take(&mut self.detached) {
                self.tombstone_statement(stmt, None);
            }
            for block in std::mem::take(&mut self.unowned_blocks) {
                self.tombstone_block(block);
            }
            for cfg in std::mem::take(&mut self.detached_cfgs) {
                self.tombstone_body(Body::CFG(cfg));
            }
        }
        for (value, _) in self.unbound_ssa_results.drain() {
            let _ = self.stage.ssas.delete(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::arena::GetInfo;
    use crate::testing::dialect::{TestLang, new_stage};
    use crate::{Block, InsertionPoint, Rewriter};

    /// A `Rewriter` used outside the pass boundary, dropped while a panic
    /// unwinds, leaves its half-built block alone instead of panicking again,
    /// which would abort the process.
    #[test]
    fn dropping_during_a_panic_leaves_unowned_blocks_in_place() {
        let mut stage = new_stage().finalize().unwrap();
        let mut detached: Option<Block> = None;

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut rw = Rewriter::new(&mut stage);
            let block = rw.new_block([]);
            let ret = rw
                .create(InsertionPoint::Terminator(block), TestLang::Return)
                .unwrap();
            let nop = rw
                .create(InsertionPoint::Before(ret), TestLang::Nop)
                .unwrap();
            // Leave the block half-built, as a panic part-way through an edit
            // could: its chain still names a statement whose slot is gone.
            let _ = rw.stage.statement_arena_mut().delete(nop);
            detached = Some(block);
            panic!("rule gave up")
        }));
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        let block = detached.expect("the closure made a block before panicking");
        assert!(block.get_info(&stage).is_some());
    }
}
