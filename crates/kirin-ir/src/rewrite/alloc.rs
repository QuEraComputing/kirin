//! Statement allocation shared by every insert: block splices,
//! `insert_terminator`, and `DiGraph` and `UnGraph` members. Each insert
//! checks and places the statement its own way; these do the common part,
//! including adopting the bodies the new statement names.
//!
//! TODO: when the insertion API is updated later, move the body checks and
//! the adoption here into its single statement-creation path, and check
//! enclosure against its insertion point rather than the parent passed in.

use crate::arena::GetInfo;
use crate::node::linked_list::LinkedListNode;
use crate::{Dialect, SSAValue, Statement, StatementInfo, StatementParent, Use};

use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{RewriteError, Rewriter, add_use};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Check that `definition` can be inserted under `parent`, and return its
    /// results and operands.
    ///
    /// Each result must be a fresh [`Rewriter::new_result`] value
    /// ([`RewriteError::ResultNotFresh`]), and each operand must be live
    /// ([`RewriteError::UnknownValue`]). Each body the definition names must be
    /// live ([`RewriteError::UnknownBody`]), owned by nothing
    /// ([`RewriteError::BodyOwnedElsewhere`]), created by this `Rewriter`
    /// ([`RewriteError::BodyNotDetached`]), named only once
    /// ([`RewriteError::BodyListedTwice`]), and must not contain `parent`
    /// ([`RewriteError::BodyEnclosesOwner`]).
    pub(super) fn prepare_insert(
        &self,
        definition: &L,
        parent: StatementParent,
    ) -> Result<(Vec<SSAValue>, Vec<SSAValue>), RewriteError> {
        let results = self.fresh_results(definition)?;
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }
        self.check_new_bodies(&bodies_of(definition), &[], parent)?;
        Ok((results, operands))
    }

    /// Allocate `definition` as a statement under `parent` with chain links
    /// `prev`/`next`, bind `results`, and record `operands` as its uses — the
    /// two lists from [`prepare_insert`](Rewriter::prepare_insert), and make it
    /// the parent of the bodies it names. Placing it (block chain, terminator
    /// slot, graph node) is left to the caller.
    pub(super) fn alloc_statement(
        &mut self,
        definition: L,
        parent: StatementParent,
        prev: Option<Statement>,
        next: Option<Statement>,
        results: Vec<SSAValue>,
        operands: &[SSAValue],
    ) -> Statement {
        let bodies = bodies_of(&definition);
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
        for body in bodies {
            self.adopt_body(body, stmt);
        }
        stmt
    }
}
