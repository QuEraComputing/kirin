use crate::arena::GetInfo;
use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};
use crate::rewrite::walk::tree;
use crate::{Dialect, Statement, StatementParent};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Move `stmt` to immediately before `anchor`, which may be in any block
    /// body.
    ///
    /// The statement keeps its id, results, uses and nested bodies; only its
    /// place changes. As with [`Rewriter::insert_before`], `anchor` may be a
    /// block's terminator: `stmt` then goes to the end of that block's
    /// statement chain. Moving a statement before itself does nothing.
    ///
    /// Not checked, as with [`Rewriter::insert_before`]: that every use of
    /// `stmt`'s results still comes after it.
    ///
    /// Rejects, leaving the stage untouched, if `stmt` or `anchor` is unknown
    /// or not in a block body ([`RewriteError::NotInBlockBody`]), if `stmt` is
    /// a terminator ([`RewriteError::UnexpectedTerminator`]), or if `anchor` is
    /// nested in `stmt`'s own bodies ([`RewriteError::MovedIntoItself`]).
    ///
    /// Records [`MutationEvent::MovedStatement`]. A walk visiting `stmt`
    /// reports the move (see [`crate::walk`]).
    pub fn move_before(&mut self, stmt: Statement, anchor: Statement) -> Result<(), RewriteError> {
        self.move_relative(stmt, anchor, false)
    }

    /// Move `stmt` to immediately after `anchor`, which may be in any block
    /// body.
    ///
    /// As [`Rewriter::move_before`], except that `anchor` must not be a
    /// terminator ([`RewriteError::AnchorIsTerminator`]): nothing follows one.
    pub fn move_after(&mut self, stmt: Statement, anchor: Statement) -> Result<(), RewriteError> {
        self.move_relative(stmt, anchor, true)
    }

    fn move_relative(
        &mut self,
        stmt: Statement,
        anchor: Statement,
        after: bool,
    ) -> Result<(), RewriteError> {
        let block = {
            let item = self.live_statement(stmt)?;
            let Some(StatementParent::Block(block)) = item.parent else {
                return Err(RewriteError::NotInBlockBody(stmt));
            };
            if item.definition.is_terminator() {
                return Err(RewriteError::UnexpectedTerminator);
            }
            block
        };
        // Checks the anchor; the place itself is read again once `stmt` is
        // unlinked, since `stmt` may be one of its neighbours.
        self.slot_at(anchor, after)?;
        if stmt == anchor {
            return Ok(());
        }
        // Walk up the statements whose bodies enclose `anchor`. If `stmt` is
        // one of them, the move would put `stmt` inside its own body.
        let mut owner = tree::parent(self.stage, anchor);
        while let Some(current) = owner {
            if current == stmt {
                return Err(RewriteError::MovedIntoItself { stmt, anchor });
            }
            owner = tree::parent(self.stage, current);
        }

        self.unlink_from_block(stmt, block);
        let (block, prev, next) = self
            .slot_at(anchor, after)
            .expect("the anchor was checked above and moving `stmt` does not touch it");
        debug_assert!(stmt.get_info(self.stage).is_some());
        self.link_into_block(stmt, block, prev, next);
        self.events.push(MutationEvent::MovedStatement { stmt });
        Ok(())
    }
}
