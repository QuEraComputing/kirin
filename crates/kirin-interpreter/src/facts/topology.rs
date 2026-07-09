//! Dialect-neutral CFG topology enumeration.
//!
//! Backward analyses need the *shape* of a CFG: which blocks exist
//! (including blocks nested inside structured statements), each block's
//! statements, the CFG successor relation, and each block's *feeders* — the
//! statements whose rules can translate demand on that block's parameters
//! (terminators targeting it, statements owning it). This is topology only —
//! uses/defs/edge-argument *semantics* stay in dialect
//! [`Interpretable`](crate::Interpretable) rules; the enumeration consumes the
//! generic [`HasSuccessors`]/[`HasBlocks`]/[`HasCfgs`] contract every
//! dialect derives.

use std::collections::{HashMap, HashSet};

use kirin_ir::{Block, Cfg, Dialect, HasBlocks, HasCfgs, HasSuccessors, StageInfo, Statement};

/// The shape of one block: its statements and CFG successors.
#[derive(Clone, Debug)]
pub struct BlockTopology {
    pub block: Block,
    /// Statements in program order; the terminator, if any, is last.
    pub stmts: Vec<Statement>,
    /// CFG successor blocks (targets of the block's terminator).
    pub successors: Vec<Block>,
    /// `true` for blocks nested inside a statement (structured bodies),
    /// `false` for the analyzed CFG's own top-level blocks.
    pub nested: bool,
}

/// The shape of a CFG: all blocks (the CFG's own top-level blocks and
/// structured bodies, recursively) plus the block-feeder index.
#[derive(Clone, Debug, Default)]
pub struct CfgTopology {
    pub blocks: Vec<BlockTopology>,
    feeders: HashMap<Block, Vec<Statement>>,
}

impl CfgTopology {
    /// The statements whose rules can translate demand on `block`'s parameters:
    /// terminators with an edge into `block`, plus statements owning `block`
    /// as a structured body.
    pub fn feeders(&self, block: Block) -> &[Statement] {
        self.feeders.get(&block).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The analyzed CFG's own top-level blocks (excluding nested bodies).
    pub fn cfg_blocks(&self) -> impl Iterator<Item = &BlockTopology> {
        self.blocks.iter().filter(|block| !block.nested)
    }
}

/// Enumerate the topology of `cfg` in the finalized `stage`.
pub fn cfg_topology<L>(stage: &StageInfo<L>, cfg: &Cfg) -> CfgTopology
where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a>,
{
    let mut topology = CfgTopology::default();
    let mut visited = HashSet::new();
    for block in cfg.blocks(stage) {
        collect_block(stage, block, false, &mut topology, &mut visited);
    }
    topology
}

fn collect_block<L>(
    stage: &StageInfo<L>,
    block: Block,
    nested: bool,
    topology: &mut CfgTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a>,
{
    if !visited.insert(block) {
        return;
    }

    let mut stmts: Vec<Statement> = block.statements(stage).collect();
    if let Some(terminator) = block.terminator(stage) {
        stmts.push(terminator);
    }

    // CFG successor edges: the target's feeders include the terminator.
    let mut successors = Vec::new();
    for &stmt in &stmts {
        for successor in stmt.definition(stage).successors() {
            let target = successor.target();
            successors.push(target);
            topology.feeders.entry(target).or_default().push(stmt);
        }
    }

    topology.blocks.push(BlockTopology {
        block,
        stmts: stmts.clone(),
        successors,
        nested,
    });

    // Structured bodies: the owning statement feeds each owned block.
    for &stmt in &stmts {
        let definition = stmt.definition(stage);
        let owned_blocks: Vec<Block> = definition.blocks().copied().collect();
        let owned_cfgs: Vec<Cfg> = definition.cfgs().copied().collect();
        for owned in owned_blocks {
            topology.feeders.entry(owned).or_default().push(stmt);
            collect_block(stage, owned, true, topology, visited);
        }
        for owned_cfg in owned_cfgs {
            for owned in owned_cfg.blocks(stage) {
                topology.feeders.entry(owned).or_default().push(stmt);
                collect_block(stage, owned, true, topology, visited);
            }
        }
    }
}
