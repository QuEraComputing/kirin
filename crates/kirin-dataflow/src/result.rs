//! The result of a liveness analysis: live sets at every program point.

use std::collections::HashMap;

use kirin_ir::{Block, Statement};

use crate::LiveSet;

/// Liveness facts for one analysed function.
///
/// The maps cover top-level CFG blocks *and* nested structured-control-flow
/// body blocks, so a caller can query any program point. A point that was
/// never reached by the analysis is simply absent from the relevant map.
#[derive(Clone, Debug, Default)]
pub struct Liveness {
    /// Values live on entry to each block (for a successor of a branch, the
    /// live block parameters report which edge args the predecessor must keep
    /// live; see [`crate::analyze_function`]).
    pub block_in: HashMap<Block, LiveSet>,
    /// Values live on exit from each block (after its terminator transfers
    /// control). Computed as the union of the successor edge transfers.
    pub block_out: HashMap<Block, LiveSet>,
    /// Values live immediately before each statement executes.
    pub stmt_before: HashMap<Statement, LiveSet>,
    /// Values live immediately after each statement executes.
    pub stmt_after: HashMap<Statement, LiveSet>,
}

impl Liveness {
    /// Values live immediately before `statement`.
    pub fn live_before(&self, statement: Statement) -> Option<&LiveSet> {
        self.stmt_before.get(&statement)
    }

    /// Values live immediately after `statement`.
    pub fn live_after(&self, statement: Statement) -> Option<&LiveSet> {
        self.stmt_after.get(&statement)
    }

    /// Values live on entry to `block`.
    pub fn block_live_in(&self, block: Block) -> Option<&LiveSet> {
        self.block_in.get(&block)
    }

    /// Values live on exit from `block`.
    pub fn block_live_out(&self, block: Block) -> Option<&LiveSet> {
        self.block_out.get(&block)
    }
}
