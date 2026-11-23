//! Checking an [`InsertionPoint`] against a definition, and placing a
//! detached statement there with its edges and events.

use std::collections::HashSet;

use crate::arena::{GetInfo, GetInfoMut};
use crate::node::linked_list::LinkedList;
use crate::{Block, DiGraph, Dialect, Statement, StatementParent, UnGraph, Use};

use super::InsertionPoint;
use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter, Side, add_predecessor};

/// An [`InsertionPoint`] checked against a definition, ready to place.
pub(super) enum Placement {
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

impl Placement {
    /// The block or graph the placed statement goes in.
    pub(super) fn parent(&self) -> StatementParent {
        match *self {
            Placement::Chain { block, .. } | Placement::Terminator { block, .. } => {
                StatementParent::Block(block)
            }
            Placement::DiGraph(graph) => StatementParent::DiGraph(graph),
            Placement::UnGraphNode(graph) | Placement::UnGraphEdge(graph) => {
                StatementParent::UnGraph(graph)
            }
        }
    }
}

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Check that a statement with `definition` fits `insertion_pt`. A
    /// rejection is wrapped in [`RewriteError::CannotInsert`], naming
    /// `insertion_pt`.
    pub(super) fn check_insertion_pt(
        &self,
        insertion_pt: InsertionPoint,
        definition: &L,
    ) -> Result<Placement, RewriteError> {
        self.placement_at(insertion_pt, definition)
            .map_err(|reason| RewriteError::CannotInsert {
                insertion_pt,
                reason: Box::new(reason),
            })
    }

    /// Where a statement with `definition` goes at `insertion_pt`, or why it
    /// does not fit there.
    fn placement_at(
        &self,
        insertion_pt: InsertionPoint,
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
        match insertion_pt {
            InsertionPoint::Before(anchor) | InsertionPoint::After(anchor) => {
                let side = match insertion_pt {
                    InsertionPoint::After(_) => Side::After,
                    _ => Side::Before,
                };
                let (block, prev, next) = self.slot_at(anchor, side)?;
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

    /// Place the detached `stmt` as `placement` says, and record the events
    /// the placement implies. The caller records `stmt`'s own event first.
    pub(super) fn place(&mut self, stmt: Statement, placement: Placement) {
        self.detached.remove(&stmt);
        let parent = placement.parent();
        if let Some(info) = stmt.get_info_mut(self.stage) {
            info.parent = Some(parent);
        }

        match placement {
            Placement::Chain { block, prev, next } => {
                self.link(LinkedList::single(stmt), block, prev, next);
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
                for operand in self.operands_of(stmt) {
                    self.add_operand_edge(graph, stmt, operand);
                }
                // A detached statement's results are usable before it is
                // placed, so members may already read them: add an edge to
                // each of their slots. `stmt`'s own slots got theirs above.
                for result in self.results_of(stmt) {
                    let consumers: Vec<Statement> = result
                        .expect_info(self.stage)
                        .uses()
                        .iter()
                        .filter_map(|site| match *site {
                            Use::StatementOperand { stmt: consumer, .. } if consumer != stmt => {
                                Some(consumer)
                            }
                            _ => None,
                        })
                        .collect();
                    let info = graph
                        .get_info_mut(self.stage)
                        .expect("graph is live in this stage");
                    for consumer in consumers {
                        if info.node_idx_of(consumer).is_some() {
                            info.add_edge(stmt, consumer, result);
                        }
                    }
                }
                self.events
                    .push(MutationEvent::ChangedDiGraphTopology { graph });
            }
            Placement::UnGraphNode(graph) => {
                // `stmt` already uses its operands, but endpoints only count
                // graph nodes, so the snapshot does not see it until it is
                // added.
                let wires = self.snapshot_wires(graph, self.operands_of(stmt));
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
                // A detached statement's results are usable before it is
                // placed, so nodes may already read its wires: resync each
                // from no endpoints, making a wire with two an edge.
                let wires = self
                    .results_of(stmt)
                    .into_iter()
                    .map(|wire| (wire, Default::default()))
                    .collect();
                self.resync_wires(graph, wires);
                self.events
                    .push(MutationEvent::ChangedUnGraphTopology { graph });
            }
        }
    }
}
