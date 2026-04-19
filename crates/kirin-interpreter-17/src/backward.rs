use std::collections::{HashMap, VecDeque};

use kirin_ir::{Block, Dialect, StageInfo, Statement};

pub trait BlockTransferBackward<'ir> {
    type Domain: Clone + PartialEq;

    fn join(a: &Self::Domain, b: &Self::Domain) -> Self::Domain;

    fn bottom() -> Self::Domain;

    fn transfer_block<L: Dialect>(
        &self,
        block: Block,
        stage: &StageInfo<L>,
        live_out: Self::Domain,
    ) -> Self::Domain;
}

pub struct BackwardFixpoint<T> {
    transfer: T,
}

impl<T> BackwardFixpoint<T> {
    pub fn new(transfer: T) -> Self {
        Self { transfer }
    }
}

impl<T> BackwardFixpoint<T> {
    pub fn analyze<'ir, L>(
        &self,
        body_stmt: Statement,
        stage: &'ir StageInfo<L>,
    ) -> HashMap<Block, (T::Domain, T::Domain)>
    where
        T: BlockTransferBackward<'ir>,
        L: Dialect,
    {
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

        let mut preds: HashMap<Block, Vec<Block>> = HashMap::new();
        for &blk in &blocks {
            preds.entry(blk).or_default();
        }
        for (&blk, blk_succs) in &succs {
            for &s in blk_succs {
                preds.entry(s).or_default().push(blk);
            }
        }

        let mut live_in: HashMap<Block, T::Domain> =
            blocks.iter().map(|&b| (b, T::bottom())).collect();
        let mut live_out: HashMap<Block, T::Domain> =
            blocks.iter().map(|&b| (b, T::bottom())).collect();

        let mut worklist: VecDeque<Block> = blocks.iter().copied().collect();
        let mut in_worklist: HashMap<Block, bool> = blocks.iter().map(|&b| (b, true)).collect();

        while let Some(blk) = worklist.pop_front() {
            in_worklist.insert(blk, false);

            let new_out: T::Domain = succs[&blk]
                .iter()
                .fold(T::bottom(), |acc, s| T::join(&acc, &live_in[s]));

            let new_in = self.transfer.transfer_block(blk, stage, new_out.clone());

            let changed = new_in != live_in[&blk] || new_out != live_out[&blk];
            live_in.insert(blk, new_in);
            live_out.insert(blk, new_out);

            if changed {
                for &pred in &preds[&blk] {
                    if !in_worklist.get(&pred).copied().unwrap_or(false) {
                        in_worklist.insert(pred, true);
                        worklist.push_back(pred);
                    }
                }
            }
        }

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
