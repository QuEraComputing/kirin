//! Building a definition into a detached statement, and reading a
//! statement's operands and results.

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use crate::{Dialect, SSAValue, Statement, StatementInfo, Use};

use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{RewriteError, Rewriter, add_use};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// `stmt`'s operands, in order.
    pub(super) fn operands_of(&self, stmt: Statement) -> Vec<SSAValue> {
        stmt.expect_info(self.stage)
            .definition
            .arguments()
            .copied()
            .collect()
    }

    /// `stmt`'s results, in order.
    pub(super) fn results_of(&self, stmt: Statement) -> Vec<SSAValue> {
        stmt.expect_info(self.stage)
            .definition
            .results()
            .map(|&result| result.into())
            .collect()
    }

    /// Build `definition` as a detached statement: it has no parent and no
    /// place yet. Its results are bound, its operand uses recorded, and it
    /// becomes the parent of the bodies it names, which the caller has checked
    /// it can adopt ([`Rewriter::check_new_bodies`]). A statement built but
    /// never inserted is tombstoned with those bodies when the `Rewriter` is
    /// dropped.
    ///
    /// Rejects, allocating nothing, if an operand is not live
    /// ([`RewriteError::UnknownValue`]) or a result is not a fresh
    /// [`Rewriter::new_result`] value ([`RewriteError::ResultNotFresh`]).
    pub(in crate::rewrite) fn build_statement(
        &mut self,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        let results = self.fresh_results(&definition)?;
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }

        let bodies = bodies_of(&definition);
        let stmt = self
            .stage
            .statement_arena_mut()
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode::new(id),
                parent: None,
                definition,
            });
        self.bind_results(stmt, results);
        for (index, &operand) in operands.iter().enumerate() {
            add_use(self.stage, operand, Use::StatementOperand { stmt, index });
        }
        for body in bodies {
            self.adopt_body(body, stmt);
        }
        self.detached.insert(stmt);
        Ok(stmt)
    }
}
