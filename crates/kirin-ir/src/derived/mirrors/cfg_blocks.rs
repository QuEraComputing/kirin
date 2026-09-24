use crate::arena::Id;
use crate::derived::chain::{ChainScan, derive_chains};
use crate::{
    Block, BlockParent, CFG, ChainDefect, ChainFinding, DanglingParent, Dialect, Finding,
    LinkedList, Mismatch, StageInfo, derived::SlotMap,
};

/// The expected `CFGInfo::blocks` summary for every CFG. `None` means the
/// CFG's block chain could not be derived.
pub(in crate::derived) type CFGBlocksMap = SlotMap<CFG, Option<LinkedList<Block>>>;

pub(in crate::derived) fn derive<L: Dialect>(
    stage: &StageInfo<L>,
    findings: &mut Vec<Finding>,
) -> CFGBlocksMap {
    let mut cfg_blocks_table = CFGBlocksMap::sized_like(&stage.nodes.cfgs);

    let mut chain_scan: ChainScan<Block, CFG> =
        ChainScan::new(stage.nodes.blocks.len(), stage.nodes.cfgs.len());
    for (raw, item) in stage.nodes.blocks.items.iter().enumerate() {
        if item.deleted() {
            continue;
        }

        // Blocks not inside CFGs do not interest us
        let Some(BlockParent::CFG(parent)) = item.parent else {
            continue;
        };
        let block = Block::from(Id(raw));

        // A block whose CFG is gone would otherwise be filed under a container
        // that no longer exists.
        if !is_live_cfg(stage, parent) {
            findings.push(Finding::DanglingParent(DanglingParent::BlockInCFG {
                block,
                parent,
            }));
            continue;
        }

        chain_scan.record(block, item.data.node.prev, item.data.node.next, parent);
    }

    let mut defects: Vec<(CFG, ChainDefect<Block>)> = Vec::new();
    let derived_chains = derive_chains(&chain_scan, &mut defects);

    for (cfg, defect) in defects {
        findings.push(Finding::Chain(ChainFinding::BlocksInCFG(defect, cfg)));
    }

    for (cfg, mirror) in derived_chains.iter() {
        let blocks = cfg_blocks_table
            .get_mut(cfg)
            .expect("both tables are sized like the cfg arena");
        *blocks = *mirror;
    }

    cfg_blocks_table
}

impl CFGBlocksMap {
    /// Write the block list into every CFG whose chain was successfully derived. Finalization only.
    ///
    /// A CFG that did not derive keeps whatever mirror it already had.
    /// This will only occur when using [`install_and_derive_mirrors_unchecked`](crate::derived::install_and_derive_mirrors_unchecked)
    pub(in crate::derived) fn install<L: Dialect>(self, stage: &mut StageInfo<L>) {
        assert!(self.len() == stage.nodes.cfgs.len());
        let stage_cfgs = stage.nodes.cfgs.items.iter_mut();
        for (item, cfg_blocks_opt) in stage_cfgs.zip(self.into_iter()) {
            let Some(cfg_blocks) = cfg_blocks_opt else {
                continue;
            };
            item.blocks = cfg_blocks;
        }
    }

    /// Compare the installed block list with this derivation, appending one
    /// [`Mismatch`] per disagreeing CFG. Never writes to the stage.
    pub(in crate::derived) fn verify<L: Dialect>(
        &self,
        stage: &StageInfo<L>,
        out: &mut Vec<Mismatch>,
    ) {
        for (raw, item) in stage.nodes.cfgs.items.iter().enumerate() {
            if item.deleted() {
                continue;
            }
            let cfg = CFG::from(Id(raw));

            // A cfg whose body could not be derived has nothing to compare
            // against; the defect was already reported as a `Finding`.
            let Some(derived) = self.get(cfg).and_then(Option::as_ref) else {
                continue;
            };

            if *derived != item.blocks {
                out.push(Mismatch::CFGBlocks {
                    cfg,
                    installed: item.blocks,
                    derived: *derived,
                });
            }
        }
    }
}

/// Whether `cfg` resolves to a live CFG of `stage`.
fn is_live_cfg<L: Dialect>(stage: &StageInfo<L>, cfg: CFG) -> bool {
    stage
        .nodes
        .cfgs
        .get(cfg)
        .is_some_and(|item| !item.deleted())
}
