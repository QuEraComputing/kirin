//! Dialect-neutral body topology enumeration: the implementation behind the
//! [`BodyTopologyQuery`](super::query::BodyTopologyQuery) IR query.
//!
//! Backward analyses need the *shape* of a body: which blocks and graph
//! nodes exist (including bodies nested inside structured statements), each
//! block's statements, each block's *feeders* — the statements whose rules can
//! translate demand on that block's parameters (terminators targeting it,
//! statements owning it) — and each graph port's *boundary* (the statement
//! owning the graph, and the port's slot index). This is topology only —
//! uses/defs/edge-argument *semantics* stay in dialect
//! [`Interpretable`](crate::Interpretable) rules; the enumeration consumes the
//! generic [`HasSuccessors`]/[`HasBlocks`]/[`HasCFG`]/[`HasDigraphs`]/
//! [`HasUngraphs`] contract every dialect derives.
//!
//! Note what is deliberately *not* here: the forward CFG successor relation.
//! That is a local IR query — `stmt.definition(stage).successors()` on a
//! block's terminator answers it — so materializing a copy would duplicate IR
//! state that can go stale. `feeders` is the *reverse* relation and is not
//! obtainable from a block alone (finding who targets it requires sweeping the
//! whole body), which is why this prepass materializes that one and not the
//! forward one.

use std::collections::{HashMap, HashSet};

use kirin_ir::{
    Block, CFG, DiGraph, Dialect, GetInfo, HasBlocks, HasCFG, HasDigraphs, HasSuccessors,
    HasUngraphs, Port, SSAValue, StageInfo, Statement, UnGraph,
};

use crate::{Body, PortBoundary};

/// The shape of one block: which block it is and the statements it contains.
#[derive(Clone, Debug)]
pub struct BlockTopology {
    pub block: Block,
    /// Statements in program order; the terminator, if any, is last.
    pub stmts: Vec<Statement>,
    /// `true` for blocks nested inside a statement (structured bodies),
    /// `false` for the analyzed body's own top-level blocks.
    pub nested: bool,
}

/// The shape of a body: all blocks and graph parts (the analyzed body's own
/// plus structured bodies, recursively), the block-feeder index, and the
/// graph-port boundary index.
///
/// Graph parts contribute only their node statements, flattened: nothing
/// consumes per-graph grouping, the graph handle, or a nested flag, so none is
/// recorded. Order is enumeration order, not a schedule — scheduling is the
/// walker's job, and backward prepasses only need *all* statements.
#[derive(Clone, Debug, Default)]
pub struct BodyTopology {
    pub blocks: Vec<BlockTopology>,
    /// Position of each block within `blocks`, so a lookup by [`Block`] is O(1)
    /// rather than a scan. Built during collection; holds no statements of its
    /// own.
    block_index: HashMap<Block, usize>,
    graph_stmts: Vec<Statement>,
    feeders: HashMap<Block, Vec<Statement>>,
    port_boundary: HashMap<SSAValue, PortBoundary>,
}

impl BodyTopology {
    /// The statements whose rules can translate demand on `block`'s parameters:
    /// terminators with an edge into `block`, plus statements owning `block`
    /// as a structured body.
    pub fn feeders(&self, block: Block) -> &[Statement] {
        self.feeders.get(&block).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The analyzed body's own top-level blocks (excluding nested bodies).
    pub fn cfg_blocks(&self) -> impl Iterator<Item = &BlockTopology> {
        self.blocks.iter().filter(|block| !block.nested)
    }

    /// The statements of `block` in program order (terminator last), if this
    /// topology enumerated it.
    ///
    /// O(1) via `block_index`. Worth indexing: the dense backward engine asks
    /// this once per block-owner analysis, and owners are re-analyzed until the
    /// fixpoint converges — a scan here is O(blocks) per iteration.
    pub fn block_statements(&self, block: Block) -> Option<&[Statement]> {
        self.block_index
            .get(&block)
            .map(|&position| self.blocks[position].stmts.as_slice())
    }

    /// Where `port` sits on its owning statement's boundary, if the port
    /// belongs to a graph enumerated by this topology.
    pub fn port_boundary(&self, port: impl Into<SSAValue>) -> Option<&PortBoundary> {
        self.port_boundary.get(&port.into())
    }

    /// Every statement enumerated by this topology: block statements first,
    /// then graph node statements.
    pub fn statements(&self) -> impl Iterator<Item = Statement> + '_ {
        self.blocks
            .iter()
            .flat_map(|block| block.stmts.iter().copied())
            .chain(self.graph_stmts.iter().copied())
    }
}

/// Enumerate the topology of `body` in the finalized `stage`.
pub fn body_topology<L>(stage: &StageInfo<L>, body: Body) -> BodyTopology
where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let mut topology = BodyTopology::default();
    let mut visited = HashSet::new();
    match body {
        Body::CFG(cfg) => {
            for block in cfg.blocks(stage) {
                collect_block(stage, block, false, &mut topology, &mut visited);
            }
        }
        Body::Block(block) => {
            collect_block(stage, block, false, &mut topology, &mut visited);
        }
        Body::DiGraph(graph) => {
            collect_digraph(stage, graph, &mut topology, &mut visited);
        }
        Body::UnGraph(graph) => {
            collect_ungraph(stage, graph, &mut topology, &mut visited);
        }
    }
    topology
}

fn collect_block<L>(
    stage: &StageInfo<L>,
    block: Block,
    nested: bool,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    if !visited.insert(block) {
        return;
    }

    let mut stmts: Vec<Statement> = block.statements(stage).collect();
    if let Some(terminator) = block.terminator(stage) {
        stmts.push(terminator);
    }

    // Record the *reverse* edge only: a statement with an edge into `target`
    // is one of `target`'s feeders. The forward direction is left to the IR.
    for &stmt in &stmts {
        for successor in stmt.definition(stage).successors() {
            topology
                .feeders
                .entry(successor.target())
                .or_default()
                .push(stmt);
        }
    }

    topology.block_index.insert(block, topology.blocks.len());
    topology.blocks.push(BlockTopology {
        block,
        stmts: stmts.clone(),
        nested,
    });

    for &stmt in &stmts {
        collect_owned_bodies(stage, stmt, topology, visited);
    }
}

fn collect_digraph<L>(
    stage: &StageInfo<L>,
    graph: DiGraph,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let info = graph.expect_info(stage);
    let stmts: Vec<Statement> = info.graph().node_weights().copied().collect();
    record_ports(info.parent(), info.ports(), topology);
    topology.graph_stmts.extend(stmts.iter().copied());
    for stmt in stmts {
        collect_owned_bodies(stage, stmt, topology, visited);
    }
}

fn collect_ungraph<L>(
    stage: &StageInfo<L>,
    graph: UnGraph,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let info = graph.expect_info(stage);
    let stmts: Vec<Statement> = info.graph().node_weights().copied().collect();
    record_ports(info.parent(), info.ports(), topology);
    topology.graph_stmts.extend(stmts.iter().copied());
    for stmt in stmts {
        collect_owned_bodies(stage, stmt, topology, visited);
    }
}

/// Descend into every body owned by `stmt`: structured blocks/cfgs (the
/// owning statement feeds each owned block) and owned graphs (their ports'
/// boundary is recorded).
fn collect_owned_bodies<L>(
    stage: &StageInfo<L>,
    stmt: Statement,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCFG<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let definition = stmt.definition(stage);
    let owned_blocks: Vec<Block> = definition.blocks().copied().collect();
    let owned_cfgs: Vec<CFG> = definition.cfgs().copied().collect();
    let owned_digraphs: Vec<DiGraph> = definition.digraphs().copied().collect();
    let owned_ungraphs: Vec<UnGraph> = definition.ungraphs().copied().collect();
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
    for owned in owned_digraphs {
        collect_digraph(stage, owned, topology, visited);
    }
    for owned in owned_ungraphs {
        collect_ungraph(stage, owned, topology, visited);
    }
}

fn record_ports(owner: Option<Statement>, ports: &[Port], topology: &mut BodyTopology) {
    let Some(owner) = owner else { return };
    for (index, &port) in ports.iter().enumerate() {
        topology
            .port_boundary
            .insert(SSAValue::from(port), PortBoundary { owner, index });
    }
}
