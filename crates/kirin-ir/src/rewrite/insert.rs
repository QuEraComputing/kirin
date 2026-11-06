//! Creating and inserting statements at an [`InsertionPoint`].
//!
//! [`Rewriter::create`] builds a new statement and places it in one step.
//! [`Rewriter::insert`] places a detached statement, such as a copy. Both
//! check the insertion point against the definition before changing anything,
//! so a rejection leaves the stage untouched.

use std::collections::HashSet;

use crate::arena::{GetInfo, GetInfoMut};
use crate::node::linked_list::LinkedListNode;
use crate::{
    Block, DiGraph, Dialect, SSAValue, Statement, StatementInfo, StatementParent, UnGraph, Use,
};

use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter, add_predecessor, add_use};

/// Where [`Rewriter::create`] or [`Rewriter::insert`] places a statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertionPoint {
    /// Immediately before the anchor, a statement in a block body. Before the
    /// block's terminator means at the end of its statement chain. The
    /// statement must be a non-terminator with no successors.
    Before(Statement),
    /// Immediately after the anchor, a non-terminator in a block body
    /// ([`RewriteError::AnchorIsTerminator`] otherwise: nothing follows a
    /// terminator). The statement must be a non-terminator with no
    /// successors.
    After(Statement),
    /// The terminator slot of a block that has none
    /// ([`RewriteError::BlockHasTerminator`] otherwise), in a CFG or a
    /// statement-owned body. The statement must be a terminator
    /// ([`RewriteError::ExpectedTerminator`]) with no results
    /// ([`RewriteError::CannotInsertTerminatorWithResults`]) and live
    /// successors. The predecessor mirror gains an entry per successor.
    Terminator(Block),
    /// A new member and node of the `DiGraph`. The statement must be a
    /// non-terminator with no successors. Each operand slot reading a result
    /// of another member gets an edge from that member; operands from outside
    /// the graph get none.
    DiGraph(DiGraph),
    /// A new member of the `UnGraph`: an edge statement if the definition is
    /// one ([`IsEdge`](crate::IsEdge)), a node otherwise. Neither may be a
    /// terminator or carry successors, and an edge statement has no operands
    /// ([`RewriteError::EdgeStatementWithOperands`]): its results are new
    /// wires. Each operand of a node that is a wire of the graph becomes an
    /// endpoint of that wire, and a wire read by two node slots is an edge. A
    /// wire may end up with more than two endpoints; that is invalid IR, left
    /// for [`verify_derived`](crate::verify_derived) to report, and such a
    /// wire has no edge.
    UnGraph(UnGraph),
}

/// An [`InsertionPoint`] checked against a definition, ready to place.
enum Placement {
    Chain {
        block: Block,
        prev: Option<Statement>,
        next: Option<Statement>,
    },
    Terminator {
        block: Block,
        targets: HashSet<Block>,
    },
    DiGraph(DiGraph),
    UnGraphNode(UnGraph),
    UnGraphEdge(UnGraph),
}

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Create a statement from `definition` at `point`, returning its id.
    ///
    /// Every operand must be live ([`RewriteError::UnknownValue`]) and every
    /// result a fresh [`Rewriter::new_result`] value
    /// ([`RewriteError::ResultNotFresh`]), which becomes a result of the new
    /// statement. The statement must also fit `point` (see
    /// [`InsertionPoint`]).
    ///
    /// Rejects, leaving the stage untouched, if a check fails. Records
    /// [`MutationEvent::InsertedStatement`], then for a terminator
    /// [`MutationEvent::ChangedTerminator`] and for a graph member
    /// [`MutationEvent::ChangedDiGraphTopology`] or
    /// [`MutationEvent::ChangedUnGraphTopology`].
    pub fn create(
        &mut self,
        point: InsertionPoint,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        let placement = self.check_point(point, &definition)?;
        let stmt = self.build_statement(definition)?;
        self.place(stmt, placement);
        Ok(stmt)
    }

    /// Insert the detached `stmt` at `point`.
    ///
    /// A detached statement is one this `Rewriter` made but has not placed,
    /// such as a copy; it has no parent yet. It must fit `point` as for
    /// [`Rewriter::create`].
    ///
    /// Rejects, leaving the stage untouched, if `stmt` is unknown, is not a
    /// detached statement of this `Rewriter` ([`RewriteError::NotDetached`]),
    /// or does not fit `point`. Records the events of [`Rewriter::create`].
    pub fn insert(&mut self, point: InsertionPoint, stmt: Statement) -> Result<(), RewriteError> {
        let item = self.live_statement(stmt)?;
        if !self.detached.contains(&stmt) {
            return Err(RewriteError::NotDetached(stmt));
        }
        let placement = self.check_point(point, &item.definition)?;
        self.place(stmt, placement);
        Ok(())
    }

    /// Check that a statement with `definition` fits `point`.
    fn check_point(
        &self,
        point: InsertionPoint,
        definition: &L,
    ) -> Result<Placement, RewriteError> {
        // Block chains and graphs take only non-terminators, which never carry
        // successors.
        let check_non_terminator = || {
            if definition.is_terminator() {
                Err(RewriteError::UnexpectedTerminator)
            } else if definition.successors().next().is_some() {
                Err(RewriteError::NonTerminatorWithSuccessors)
            } else {
                Ok(())
            }
        };
        match point {
            InsertionPoint::Before(anchor) | InsertionPoint::After(anchor) => {
                let after = matches!(point, InsertionPoint::After(_));
                let (block, prev, next) = self.slot_at(anchor, after)?;
                check_non_terminator()?;
                Ok(Placement::Chain { block, prev, next })
            }
            InsertionPoint::Terminator(block) => {
                if self.live_block(block)?.terminator.is_some() {
                    return Err(RewriteError::BlockHasTerminator(block));
                }
                if !definition.is_terminator() {
                    return Err(RewriteError::ExpectedTerminator);
                }
                if definition.results().next().is_some() {
                    return Err(RewriteError::CannotInsertTerminatorWithResults);
                }
                let targets: HashSet<Block> = definition.successors().map(|s| s.target()).collect();
                for &target in &targets {
                    self.live_block(target)?;
                }
                Ok(Placement::Terminator { block, targets })
            }
            InsertionPoint::DiGraph(graph) => {
                self.live_digraph(graph)?;
                check_non_terminator()?;
                Ok(Placement::DiGraph(graph))
            }
            InsertionPoint::UnGraph(graph) => {
                self.live_ungraph(graph)?;
                check_non_terminator()?;
                if !definition.is_edge() {
                    return Ok(Placement::UnGraphNode(graph));
                }
                if definition.arguments().next().is_some() {
                    return Err(RewriteError::EdgeStatementWithOperands);
                }
                Ok(Placement::UnGraphEdge(graph))
            }
        }
    }

    /// Place the detached `stmt` as `placement` says, and record the events.
    fn place(&mut self, stmt: Statement, placement: Placement) {
        self.detached.remove(&stmt);
        let operands: Vec<SSAValue> = stmt
            .expect_info(self.stage)
            .definition
            .arguments()
            .copied()
            .collect();
        let parent = match placement {
            Placement::Chain { block, .. } | Placement::Terminator { block, .. } => {
                StatementParent::Block(block)
            }
            Placement::DiGraph(graph) => StatementParent::DiGraph(graph),
            Placement::UnGraphNode(graph) | Placement::UnGraphEdge(graph) => {
                StatementParent::UnGraph(graph)
            }
        };
        if let Some(info) = stmt.get_info_mut(self.stage) {
            info.parent = Some(parent);
        }
        self.events.push(MutationEvent::InsertedStatement { stmt });

        match placement {
            Placement::Chain { block, prev, next } => {
                self.link_into_block(stmt, block, prev, next);
            }
            Placement::Terminator { block, targets } => {
                // Only terminators carry successors and the block had none, so
                // every target gains exactly one entry for `block`.
                for target in targets {
                    add_predecessor(self.stage, target, block);
                }
                if let Some(info) = block.get_info_mut(self.stage) {
                    info.terminator = Some(stmt);
                }
                self.events.push(MutationEvent::ChangedTerminator { block });
            }
            Placement::DiGraph(graph) => {
                graph
                    .get_info_mut(self.stage)
                    .expect("graph is live in this stage")
                    .add_node(stmt);
                for operand in operands {
                    self.add_operand_edge(graph, stmt, operand);
                }
                self.events
                    .push(MutationEvent::ChangedDiGraphTopology { graph });
            }
            Placement::UnGraphNode(graph) => {
                // `stmt` already uses its operands, but endpoints only count
                // graph nodes, so the snapshot does not see it until it is
                // added.
                let wires = self.snapshot_wires(graph, operands);
                graph
                    .get_info_mut(self.stage)
                    .expect("graph is live in this stage")
                    .add_node(stmt);
                self.resync_wires(graph, wires);
                self.events
                    .push(MutationEvent::ChangedUnGraphTopology { graph });
            }
            Placement::UnGraphEdge(graph) => {
                graph
                    .get_info_mut(self.stage)
                    .expect("graph is live in this stage")
                    .add_edge_member(stmt);
                self.events
                    .push(MutationEvent::ChangedUnGraphTopology { graph });
            }
        }
    }

    /// Build `definition` as a detached statement: it has no parent and no
    /// place yet. Its results are bound and its operand uses recorded. A
    /// statement built but never inserted is tombstoned when the `Rewriter` is
    /// dropped.
    ///
    /// Rejects, allocating nothing, if an operand is not live
    /// ([`RewriteError::UnknownValue`]) or a result is not a fresh
    /// [`Rewriter::new_result`] value ([`RewriteError::ResultNotFresh`]).
    pub(super) fn build_statement(&mut self, definition: L) -> Result<Statement, RewriteError> {
        let results = self.fresh_results(&definition)?;
        let operands: Vec<SSAValue> = definition.arguments().copied().collect();
        for &operand in &operands {
            if operand.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(operand));
            }
        }

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
        self.detached.insert(stmt);
        Ok(stmt)
    }
}
