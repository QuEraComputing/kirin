//! Creating bodies owned by nothing.

use crate::{Block, Body, Dialect, Rewriter};

impl<L: Dialect> Rewriter<'_, L> {
    /// Create an empty block with arguments of `arg_types`, owned by nothing.
    ///
    /// The block is meant to become a statement's body, such as a branch of
    /// an `scf.if`: fill it, then name it in a definition passed to an insert
    /// (such as [`Rewriter::insert_before`]) or to
    /// [`Rewriter::replace_statement`], which makes the statement its parent.
    /// Statements can be added to it like any block, starting with its
    /// terminator ([`Rewriter::insert_terminator`]).
    ///
    /// Until the block is attached or erased, its statements count as uses of
    /// the values they read: such a value cannot be erased, dead-code
    /// elimination keeps it, and [`Rewriter::replace_all_uses`] rewrites those
    /// uses too. To abandon the block early, call [`Rewriter::erase_body`].
    ///
    /// A block no statement owns when the `Rewriter` is dropped is tombstoned
    /// together with everything it holds, unless the drop follows a panic, in
    /// which case it is left for inspection. Unlike [`Rewriter::erase_body`],
    /// the drop does not check that nothing outside still refers to the block:
    /// a branch to it, or a use of a value it defines, is left dangling. When
    /// a pass run by [`Program::run_pass`](crate::Program::run_pass) succeeds,
    /// [`verify_derived`](crate::verify_derived) then finds it and the stage is
    /// quarantined. When the pass fails, or the `Rewriter` is used directly,
    /// nothing reports it.
    ///
    /// Records no event: a block owned by nothing is not part of the IR yet.
    /// Edits inside it record their usual events. The statement that adopts it
    /// records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    pub fn new_block(&mut self, arg_types: impl IntoIterator<Item = L::Type>) -> Block {
        let block = self.alloc_block(None, arg_types);
        self.unowned_bodies.insert(Body::Block(block));
        block
    }
}
