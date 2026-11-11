use crate::arena::{GetInfo, GetInfoMut};
use crate::node::linked_list::LinkedList;
use crate::rewrite::rewriter::{MutationEvent, RewriteError, Rewriter};
use crate::rewrite::walk::tree;
use crate::{Block, Dialect, SSAValue, Statement, StatementParent};

impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Move `stmt` to immediately before `anchor`, which may be in any block
    /// body.
    ///
    /// The statement keeps its id, results, uses and nested bodies; only its
    /// place changes. As with [`Rewriter::insert_before`], `anchor` may be a
    /// block's terminator: `stmt` then goes to the end of that block's
    /// statement chain. Moving a statement to where it already is (before
    /// itself, or before the statement that already follows it) does nothing
    /// and records no event.
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
        let (_, prev, next) = self.slot_at(anchor, after)?;
        // `stmt` already borders the place (this includes `stmt == anchor`),
        // so unlinking it and linking it back would put it where it is.
        if prev == Some(stmt) || next == Some(stmt) {
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

        self.unlink(stmt, block);
        let (block, prev, next) = self
            .slot_at(anchor, after)
            .expect("the anchor was checked above and moving `stmt` does not touch it");
        debug_assert!(stmt.get_info(self.stage).is_some());
        self.link(LinkedList::single(stmt), block, prev, next);
        self.events.push(MutationEvent::MovedStatement { stmt });
        Ok(())
    }

    /// Move every statement of `source`, except its terminator, to
    /// immediately before `anchor`, which may be in any block body, and
    /// redirect every use of `source`'s `i`-th argument to
    /// `arg_replacements[i]`.
    ///
    /// The statements keep their order, ids, results, uses and nested bodies.
    /// `source` keeps its arguments and terminator and is not erased. `anchor`
    /// follows the rules of [`Rewriter::move_before`]: before a terminator
    /// means at the end of that block's statement chain. A replacement may be
    /// one of `source`'s own arguments.
    ///
    /// Rejects, leaving the stage untouched, if `source` or `anchor` is
    /// unknown, if `anchor` is not in a block body
    /// ([`RewriteError::NotInBlockBody`]), if `anchor` is in `source` or
    /// nested in one of its statements ([`RewriteError::SplicedIntoItself`]),
    /// if the replacement count differs from `source`'s argument count
    /// ([`RewriteError::BlockArgumentArityMismatch`]), or if a replacement is
    /// not live.
    ///
    /// Records the [`Rewriter::replace_all_uses`] events for the argument
    /// redirection, then [`MutationEvent::SplicedBlock`]. A walk visiting a
    /// statement of `source` reports the splice (see [`crate::walk`]).
    pub fn splice_block_before(
        &mut self,
        source: Block,
        anchor: Statement,
        arg_replacements: &[SSAValue],
    ) -> Result<(), RewriteError> {
        self.splice_relative(source, anchor, arg_replacements, false)
    }

    /// Move every statement of `source`, except its terminator, to
    /// immediately after `anchor`, and redirect `source`'s arguments.
    ///
    /// As [`Rewriter::splice_block_before`], except that `anchor` must not be
    /// a terminator ([`RewriteError::AnchorIsTerminator`]): nothing follows
    /// one.
    pub fn splice_block_after(
        &mut self,
        source: Block,
        anchor: Statement,
        arg_replacements: &[SSAValue],
    ) -> Result<(), RewriteError> {
        self.splice_relative(source, anchor, arg_replacements, true)
    }

    fn splice_relative(
        &mut self,
        source: Block,
        anchor: Statement,
        arg_replacements: &[SSAValue],
        after: bool,
    ) -> Result<(), RewriteError> {
        let args: Vec<SSAValue> = self
            .live_block(source)?
            .arguments
            .iter()
            .map(|&arg| arg.into())
            .collect();
        let (dest, prev, next) = self.slot_at(anchor, after)?;
        // Check `anchor` and every statement whose body encloses it. If one
        // of them is a statement of the block `source`, it would be spliced
        // next to or inside itself.
        let mut current = Some(anchor);
        while let Some(stmt) = current {
            if *stmt.parent(self.stage) == Some(StatementParent::Block(source)) {
                return Err(RewriteError::SplicedIntoItself { source, anchor });
            }
            current = tree::parent(self.stage, stmt);
        }
        if arg_replacements.len() != args.len() {
            return Err(RewriteError::BlockArgumentArityMismatch {
                block: source,
                expected: args.len(),
                found: arg_replacements.len(),
            });
        }
        for &value in arg_replacements {
            if value.get_info(self.stage).is_none() {
                return Err(RewriteError::UnknownValue(value));
            }
        }

        let pairs: Vec<(SSAValue, SSAValue)> = args
            .into_iter()
            .zip(arg_replacements.iter().copied())
            .filter(|(old, new)| old != new)
            .collect();
        self.redirect_uses(&pairs);

        let chain = match source.get_info_mut(self.stage) {
            Some(info) => std::mem::take(&mut info.statements),
            None => LinkedList::new(),
        };
        self.link(chain, dest, prev, next);
        self.events
            .push(MutationEvent::SplicedBlock { source, dest });
        Ok(())
    }
}
