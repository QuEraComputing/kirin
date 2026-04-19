use std::collections::{HashMap, VecDeque};

use kirin_ir::{Block, Dialect, StageInfo, Statement};

// ---------------------------------------------------------------------------
// User-facing trait
// ---------------------------------------------------------------------------

/// Trait implemented by a backward dataflow domain.
///
/// The user provides one method — [`transfer_block`] — which computes the
/// live-in state for a block given its live-out state.  The framework
/// handles CFG traversal, join at block exit boundaries, and convergence.
pub trait BlockTransferBackward<'ir> {
    /// The abstract domain element (e.g. `HashSet<SSAValue>`).
    type Domain: Clone + PartialEq;

    /// Combine two domain elements at a control-flow join (e.g. set union for liveness).
    fn join(a: &Self::Domain, b: &Self::Domain) -> Self::Domain;

    /// The bottom element — used to initialise every block's live-out.
    fn bottom() -> Self::Domain;

    /// Compute the live-in state for `block` given `live_out`.
    ///
    /// The implementation should process the block's statements in **reverse**
    /// order (terminator first, then body statements from last to first) and
    /// compute the standard gen/kill transformation.
    fn transfer_block<L: Dialect>(
        &self,
        block: Block,
        stage: &StageInfo<L>,
        live_out: Self::Domain,
    ) -> Self::Domain;
}

// ---------------------------------------------------------------------------
// Framework struct
// ---------------------------------------------------------------------------

/// Generic backward-worklist fixpoint engine.
///
/// Construct with [`BackwardFixpoint::new`] and call [`BackwardFixpoint::analyze`]
/// to run the analysis.  Returns a map from each [`Block`] to its
/// `(live_in, live_out)` pair at the fixed point.
pub struct BackwardFixpoint<T> {
    transfer: T,
}

impl<T> BackwardFixpoint<T> {
    /// Create a new fixpoint engine wrapping the given transfer function.
    pub fn new(transfer: T) -> Self {
        Self { transfer }
    }
}

impl<T> BackwardFixpoint<T> {
    /// Run the backward fixpoint analysis over a function body.
    ///
    /// `body_stmt` is the statement that *owns* the function body (i.e. the
    /// statement whose first region contains all of the function's blocks).
    /// `stage` is the [`StageInfo`] for the dialect being analysed.
    ///
    /// Returns a [`HashMap`] mapping each [`Block`] to `(live_in, live_out)`
    /// at the fixed point.
    pub fn analyze<'ir, L>(
        &self,
        body_stmt: Statement,
        stage: &'ir StageInfo<L>,
    ) -> HashMap<Block, (T::Domain, T::Domain)>
    where
        T: BlockTransferBackward<'ir>,
        L: Dialect,
    {
        // ------------------------------------------------------------------
        // 1. Collect all blocks and build the successor map.
        // ------------------------------------------------------------------
        let region = body_stmt
            .regions(stage)
            .next()
            .expect("function body must have a region");
        let blocks: Vec<Block> = region.blocks(stage).collect();

        let mut succs: HashMap<Block, Vec<Block>> = HashMap::new();
        for &blk in &blocks {
            let mut blk_succs = Vec::new();
            if let Some(term) = blk.terminator(stage) {
                for succ in term.successors(stage) {
                    blk_succs.push(succ.target());
                }
            }
            succs.insert(blk, blk_succs);
        }

        // ------------------------------------------------------------------
        // 2. Build predecessor map (invert successor map).
        // ------------------------------------------------------------------
        let mut preds: HashMap<Block, Vec<Block>> = HashMap::new();
        for &blk in &blocks {
            preds.entry(blk).or_default();
        }
        for (&blk, blk_succs) in &succs {
            for &s in blk_succs {
                preds.entry(s).or_default().push(blk);
            }
        }

        // ------------------------------------------------------------------
        // 3. Initialise live_in / live_out to bottom.
        // ------------------------------------------------------------------
        let mut live_in: HashMap<Block, T::Domain> =
            blocks.iter().map(|&b| (b, T::bottom())).collect();
        let mut live_out: HashMap<Block, T::Domain> =
            blocks.iter().map(|&b| (b, T::bottom())).collect();

        // ------------------------------------------------------------------
        // 4. Backward worklist until convergence.
        // ------------------------------------------------------------------
        let mut worklist: VecDeque<Block> = blocks.iter().copied().collect();
        // O(1) deduplication set so we never enqueue the same block twice.
        let mut in_worklist: HashMap<Block, bool> = blocks.iter().map(|&b| (b, true)).collect();

        while let Some(blk) = worklist.pop_front() {
            in_worklist.insert(blk, false);

            // live_out[blk] = ∪ live_in[s] for each successor s of blk
            let new_out: T::Domain = succs[&blk]
                .iter()
                .fold(T::bottom(), |acc, s| T::join(&acc, &live_in[s]));

            // live_in[blk] = transfer_block(blk, new_out)
            let new_in = self.transfer.transfer_block(blk, stage, new_out.clone());

            let changed = new_in != live_in[&blk] || new_out != live_out[&blk];
            live_in.insert(blk, new_in);
            live_out.insert(blk, new_out);

            if changed {
                // Re-enqueue predecessors of blk.
                for &pred in &preds[&blk] {
                    if !in_worklist.get(&pred).copied().unwrap_or(false) {
                        in_worklist.insert(pred, true);
                        worklist.push_back(pred);
                    }
                }
            }
        }

        // ------------------------------------------------------------------
        // 5. Assemble the result map.
        // ------------------------------------------------------------------
        blocks
            .iter()
            .map(|&b| {
                let li = live_in.remove(&b).unwrap_or_else(T::bottom);
                let lo = live_out.remove(&b).unwrap_or_else(T::bottom);
                (b, (li, lo))
            })
            .collect()
    }
}
