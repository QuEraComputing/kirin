//! Creating bodies owned by nothing.

use crate::node::CFGInfo;
use crate::node::linked_list::LinkedList;
use crate::{Block, Body, CFG, Dialect, Rewriter};

impl<L: Dialect> Rewriter<'_, L> {
    /// Create an empty block with arguments of `arg_types`, owned by nothing.
    ///
    /// The block is meant to become a statement's body, such as a branch of
    /// an `scf.if`. Add statements to it like any block, starting with its
    /// terminator ([`Rewriter::insert_terminator`]). Then name the block in
    /// the definition of a new statement ([`Rewriter::insert_before`] and
    /// similar) or of a replacement ([`Rewriter::replace_statement`]); that
    /// statement becomes its parent.
    ///
    /// Its statements count as uses of the values they read as soon as they
    /// are added, even while the block is owned by nothing: such a value
    /// cannot be erased while they read it. [`Rewriter::erase_body`] discards
    /// the block early.
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
    /// The statement that adopts it records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    /// Edits inside it still record their usual events for now.
    pub fn new_block(&mut self, arg_types: impl IntoIterator<Item = L::Type>) -> Block {
        let block = self.alloc_block(None, arg_types);
        self.unowned_bodies.insert(Body::Block(block));
        block
    }

    /// Create an empty CFG, owned by nothing.
    ///
    /// The CFG is meant to become a statement's body, such as a lambda's.
    /// Create its blocks with [`Rewriter::append_block`],
    /// [`Rewriter::insert_block_before`] or [`Rewriter::insert_block_after`],
    /// which link them into its block list ([`Rewriter::new_block`] blocks
    /// cannot be moved into a CFG), and fill them. Then name the CFG in the
    /// definition of a new statement ([`Rewriter::insert_before`] and similar)
    /// or of a replacement ([`Rewriter::replace_statement`]); that statement
    /// becomes its parent.
    ///
    /// The rest is as for a [`Rewriter::new_block`] block. Its statements
    /// count as uses of the values they read as soon as they are added, even
    /// while the CFG is owned by nothing; [`Rewriter::erase_body`] discards
    /// it early. A CFG no statement owns when the `Rewriter` is dropped is
    /// tombstoned together with everything it holds, unless the drop follows a
    /// panic, without the checks [`Rewriter::erase_body`] makes.
    ///
    /// Records no event: a CFG owned by nothing is not part of the IR yet.
    /// The statement that adopts it records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    /// Edits inside it still record their usual events.
    pub fn new_cfg(&mut self) -> CFG {
        let cfg = self.stage.nodes.cfgs.alloc_with_id(|id| {
            CFGInfo::builder()
                .id(id)
                .blocks(LinkedList::default())
                .new()
        });
        self.unowned_bodies.insert(Body::CFG(cfg));
        cfg
    }
}
