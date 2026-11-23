//! Fresh result values for built statements.
//!
//! A definition names its results by id, so the ids must exist before the
//! definition can be written, while the statement that defines them does not
//! exist until it is built. [`Rewriter::new_result`] bridges the gap: it
//! allocates an id whose arena slot stays empty (so the value reads as not live
//! and cannot be used yet) and keeps its type until [`Rewriter::create`] binds
//! it. Dropping the `Rewriter` tombstones every value it never bound, so once
//! it is gone each result slot is either bound or a real tombstone.
//! [`Rewriter::into_events`], or the drop, also tombstones the detached code
//! nothing placed.

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

    /// Tombstone every detached statement no [`insert`](Rewriter::insert)
    /// placed, with its results, operand uses and bodies, every
    /// [`new_block`](Rewriter::new_block) block no statement owns, and every
    /// detached CFG (such as a [`clone_cfg`](Rewriter::clone_cfg) copy) no
    /// statement adopted. Records
    /// [`MutationEvent::ErasedStatement`](crate::MutationEvent::ErasedStatement)
    /// for each statement tombstoned. Run by [`Rewriter::into_events`] before it takes
    /// the log, and by the drop.
    ///
    /// Does nothing after a panic: a panic part-way through an edit can leave
    /// detached code inconsistent, so tombstoning it could panic again. Two
    /// cases are covered. When this `Rewriter` is dropped while a panic
    /// unwinds, a second panic would abort the process. When the pass boundary
    /// has caught the panic, the unwind is over, so it sets
    /// `keep_detached_on_drop` instead; a second panic would escape the
    /// boundary and lose the quarantined stage, which keeps the detached code
    /// for inspection.
    pub(super) fn tombstone_detached(&mut self) {
        if self.keep_detached_on_drop || std::thread::panicking() {
            return;
        }
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
}

impl<'a, L: Dialect> Drop for Rewriter<'a, L> {
    /// Tombstone the detached code [`Rewriter::into_events`] has not already
    /// tombstoned (statements no [`insert`](Rewriter::insert) placed,
    /// [`new_block`](Rewriter::new_block) blocks and detached CFGs nothing
    /// adopted), unless the drop follows a panic, and every
    /// [`new_result`](Rewriter::new_result) value no statement bound, so
    /// nothing detached or empty outlives the `Rewriter`.
    fn drop(&mut self) {
        self.tombstone_detached();
        for (value, _) in self.unbound_ssa_results.drain() {
            let _ = self.stage.ssas.delete(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use crate::arena::GetInfo;
    use crate::testing::dialect::{TestLang, TestType, new_stage};
    use crate::{
        Block, InsertionPoint, MutationEvent, Rewriter, SSAValue, StageInfo, Statement,
        StatementParent, Use, verify_derived,
    };

    /// A stage whose only block has one argument, returned as a live value
    /// for detached statements to read.
    fn stage_with_a_value() -> (StageInfo<TestLang>, SSAValue) {
        let mut stage = new_stage();
        let block = stage.block().argument(TestType::I32).new();
        let stage = stage.finalize().unwrap();
        let x = SSAValue::from(block.expect_info(&stage).arguments()[0]);
        (stage, x)
    }

    fn uses(stage: &StageInfo<TestLang>, value: SSAValue) -> Vec<Use> {
        value.expect_info(stage).uses().to_vec()
    }

    /// Detached statements built and never inserted, one per kind of thing
    /// they hold.
    struct Detached {
        /// Reads `x`.
        reader: Statement,
        /// Owns `body`, which holds `inner`.
        owner: Statement,
        body: Block,
        inner: Statement,
        /// Defines `result`.
        def: Statement,
        result: SSAValue,
    }

    fn build_detached(rw: &mut Rewriter<'_, TestLang>, x: SSAValue) -> Detached {
        let reader = rw.build_statement(TestLang::Use(x)).unwrap();
        let body = rw.new_block([]);
        let inner = rw
            .create(InsertionPoint::Terminator(body), TestLang::Return)
            .unwrap();
        let owner = rw.build_statement(TestLang::Region(body)).unwrap();
        let result = rw.new_result(TestType::I32);
        let def = rw.build_statement(TestLang::Def(result)).unwrap();
        Detached {
            reader,
            owner,
            body,
            inner,
            def,
            result: result.into(),
        }
    }

    #[test]
    fn dropping_tombstones_detached_statements_with_their_uses_results_and_bodies() {
        let (mut stage, x) = stage_with_a_value();
        let mut rw = Rewriter::new(&mut stage);
        let d = build_detached(&mut rw, x);
        // Built, so live, but placed nowhere.
        assert_eq!(
            uses(rw.stage(), x),
            [Use::StatementOperand {
                stmt: d.reader,
                index: 0
            }]
        );
        assert_eq!(
            *d.inner.parent(rw.stage()),
            Some(StatementParent::Block(d.body))
        );
        assert!(d.result.get_info(rw.stage()).is_some());

        drop(rw);
        for stmt in [d.reader, d.owner, d.inner, d.def] {
            assert!(
                stmt.get_info(&stage).is_none(),
                "{stmt:?} outlived the drop"
            );
        }
        assert!(d.body.get_info(&stage).is_none());
        assert!(d.result.get_info(&stage).is_none());
        // `x` stays, with the detached reader's use removed.
        assert_eq!(uses(&stage, x), []);
        verify_derived(&stage).unwrap();
    }

    /// As [`dropping_during_a_panic_leaves_unowned_blocks_in_place`], for
    /// detached statements: they keep their uses, results and bodies.
    #[test]
    fn dropping_during_a_panic_leaves_detached_statements_in_place() {
        let (mut stage, x) = stage_with_a_value();
        let mut built: Option<Detached> = None;

        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut rw = Rewriter::new(&mut stage);
            built = Some(build_detached(&mut rw, x));
            panic!("rule gave up")
        }));
        std::panic::set_hook(previous_hook);

        assert!(result.is_err());
        let d = built.expect("the closure built statements before panicking");
        for stmt in [d.reader, d.owner, d.inner, d.def] {
            assert!(stmt.get_info(&stage).is_some(), "{stmt:?} was tombstoned");
        }
        assert!(d.body.get_info(&stage).is_some());
        assert!(d.result.get_info(&stage).is_some());
        assert_eq!(
            uses(&stage, x),
            [Use::StatementOperand {
                stmt: d.reader,
                index: 0
            }]
        );
    }

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

    /// After the pass boundary catches a panic, `into_events` leaves detached
    /// code in place and records no erasure for it.
    #[test]
    fn ending_after_a_caught_panic_keeps_detached_code_and_records_nothing() {
        let mut stage = new_stage().finalize().unwrap();
        let mut rw = Rewriter::new(&mut stage);
        let block = rw.new_block([]);
        let ret = rw
            .create(InsertionPoint::Terminator(block), TestLang::Return)
            .unwrap();
        rw.keep_detached_on_drop();
        let events = rw.into_events();

        assert_eq!(
            events,
            [
                MutationEvent::CreatedStatement { stmt: ret },
                MutationEvent::ChangedTerminator { block },
            ]
        );
        assert!(block.get_info(&stage).is_some());
        assert!(ret.get_info(&stage).is_some());
    }
}
