//! [`InsertionPoint`], and the [`Rewriter::create`] and [`Rewriter::insert`]
//! entry points: check an insertion point, then build or take the statement
//! and place it.

use crate::arena::GetInfo;
use crate::{Block, BlockParent, DiGraph, Dialect, Statement, UnGraph};

use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{RewriteError, Rewriter};

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
    /// the graph get none. A detached statement's results may already be read
    /// by members; each such slot gets an edge from the new node.
    DiGraph(DiGraph),
    /// A new member of the `UnGraph`: an edge statement if the definition is
    /// one ([`IsEdge`](crate::IsEdge)), a node otherwise. Neither may be a
    /// terminator or carry successors, and an edge statement has no operands
    /// ([`RewriteError::EdgeStatementWithOperands`]): its results are new
    /// wires. Each operand of a node that is a wire of the graph becomes an
    /// endpoint of that wire, and a wire read by two node slots is an edge,
    /// including the wires of a detached edge statement that nodes already
    /// read. A wire may end up with more than two endpoints; that is invalid
    /// IR, left for [`verify_derived`](crate::verify_derived) to report, and
    /// such a wire has no edge.
    UnGraph(UnGraph),
}

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Create a statement from `definition` at `insertion_pt`, returning its id.
    ///
    /// Every operand must be live ([`RewriteError::UnknownValue`]) and every
    /// result a fresh [`Rewriter::new_result`] value
    /// ([`RewriteError::ResultNotFresh`]), which becomes a result of the new
    /// statement. The statement must also fit `insertion_pt` (see
    /// [`InsertionPoint`]); if it does not, the error is
    /// [`RewriteError::CannotInsert`], naming `insertion_pt` and the reason.
    ///
    /// The new statement becomes the parent of every body `definition` names,
    /// such as blocks from [`Rewriter::new_block`] or a CFG from
    /// [`Rewriter::clone_cfg`]. Each must be live
    /// ([`RewriteError::UnknownBody`]), owned by nothing
    /// ([`RewriteError::BodyOwnedElsewhere`]; a block in a `CFG` counts as
    /// owned), named once ([`RewriteError::BodyListedTwice`]), and must not
    /// contain the block or graph `insertion_pt` is in, which would make the
    /// statement its own ancestor ([`RewriteError::BodyEnclosesOwner`],
    /// wrapped in [`RewriteError::CannotInsert`]).
    ///
    /// Rejects, leaving the stage untouched, if a check fails. Records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// (covering the adopted bodies), then for a terminator
    /// [`MutationEvent::ChangedTerminator`](crate::MutationEvent::ChangedTerminator)
    /// and for a graph member
    /// [`MutationEvent::ChangedDiGraphTopology`](crate::MutationEvent::ChangedDiGraphTopology)
    /// or
    /// [`MutationEvent::ChangedUnGraphTopology`](crate::MutationEvent::ChangedUnGraphTopology).
    pub fn create(
        &mut self,
        insertion_pt: InsertionPoint,
        definition: L,
    ) -> Result<Statement, RewriteError> {
        let placement = self.check_insertion_pt(insertion_pt, &definition)?;
        // A body containing `insertion_pt` is a placement error, so it names
        // the insertion point. The other body errors are about the body alone.
        self.check_new_bodies(&bodies_of(&definition), &[], placement.parent())
            .map_err(|reason| match reason {
                RewriteError::BodyEnclosesOwner(_) => RewriteError::CannotInsert {
                    insertion_pt,
                    reason: Box::new(reason),
                },
                reason => reason,
            })?;
        let stmt = self.build_statement(definition)?;
        self.place(stmt, placement);
        Ok(stmt)
    }

    /// Insert the detached `stmt` at `insertion_pt`.
    ///
    /// A detached statement is one this `Rewriter` made but has not placed,
    /// such as a copy; it has no parent yet. It must fit `insertion_pt` as for
    /// [`Rewriter::create`].
    ///
    /// A detached statement's results can be read before it is placed, but
    /// one never placed is tombstoned when the `Rewriter` is dropped, leaving
    /// the statements reading its results with dangling operands.
    /// [`verify_derived`](crate::verify_derived) reports them, so
    /// [`Program::run_pass`](crate::Program::run_pass) quarantines the
    /// stage.
    ///
    /// Rejects, leaving the stage untouched, if `stmt` is unknown, is not a
    /// detached statement of this `Rewriter` ([`RewriteError::NotDetached`]),
    /// or does not fit `insertion_pt`. `insertion_pt` must not be nested in
    /// `stmt`'s own bodies ([`RewriteError::InsertedIntoItself`], wrapped in
    /// [`RewriteError::CannotInsert`]). Records the events of
    /// [`Rewriter::create`].
    pub fn insert(
        &mut self,
        insertion_pt: InsertionPoint,
        stmt: Statement,
    ) -> Result<(), RewriteError> {
        let item = self.live_statement(stmt)?;
        if !self.detached.contains(&stmt) {
            return Err(RewriteError::NotDetached(stmt));
        }
        let placement = self.check_insertion_pt(insertion_pt, &item.definition)?;
        if self.encloses(stmt, self.insertion_pt_owner(insertion_pt)) {
            return Err(RewriteError::CannotInsert {
                insertion_pt,
                reason: Box::new(RewriteError::InsertedIntoItself(stmt)),
            });
        }
        self.place(stmt, placement);
        Ok(())
    }

    /// The statement whose body holds `insertion_pt`, or `None` at the top
    /// level. `insertion_pt` has passed [`Rewriter::check_insertion_pt`].
    fn insertion_pt_owner(&self, insertion_pt: InsertionPoint) -> Option<Statement> {
        match insertion_pt {
            InsertionPoint::Before(anchor) | InsertionPoint::After(anchor) => {
                anchor.owner(self.stage)
            }
            InsertionPoint::Terminator(block) => match block.get_info(self.stage)?.parent()? {
                BlockParent::CFG(cfg) => cfg.get_info(self.stage)?.parent,
                BlockParent::Statement(owner) => Some(owner),
            },
            InsertionPoint::DiGraph(graph) => graph.get_info(self.stage)?.parent(),
            InsertionPoint::UnGraph(graph) => graph.get_info(self.stage)?.parent(),
        }
    }
}
