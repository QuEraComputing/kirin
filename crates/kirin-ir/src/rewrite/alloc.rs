//! Statement allocation shared by every insert: block splices,
//! `insert_terminator`, and `DiGraph` nodes. Each insert checks and places
//! the statement its own way; these do the common part.

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use crate::{Dialect, SSAValue, Statement, StatementInfo, StatementParent, Use};

use crate::rewrite::rewriter::{RewriteError, Rewriter, add_use};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Check `definition`'s results and operands for insertion, returning
    /// both: every result must be a fresh [`Rewriter::new_result`] value
    /// ([`RewriteError::ResultNotFresh`]) and every operand live
    /// ([`RewriteError::UnknownValue`]).
    pub(super) fn prepare_insert(
        &self,
        definition: &L,
    ) -> Result<(Vec<SSAValue>, Vec<SSAValue>), RewriteError> {
        let results = self.fresh_results(definition)?;
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }
        Ok((results, operands))
    }

    /// Allocate `definition` as a statement under `parent` with chain links
    /// `prev`/`next`, bind `results`, and record `operands` as its uses — the
    /// two lists from [`prepare_insert`](Rewriter::prepare_insert). Placing it
    /// (block chain, terminator slot, graph node) is left to the caller.
    pub(super) fn alloc_statement(
        &mut self,
        definition: L,
        parent: StatementParent,
        prev: Option<Statement>,
        next: Option<Statement>,
        results: Vec<SSAValue>,
        operands: &[SSAValue],
    ) -> Statement {
        let stmt = self
            .stage
            .statement_arena_mut()
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode {
                    ptr: id,
                    next,
                    prev,
                },
                parent: Some(parent),
                definition,
            });
        self.bind_results(stmt, results);
        for (index, &operand) in operands.iter().enumerate() {
            add_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        stmt
    }
}
