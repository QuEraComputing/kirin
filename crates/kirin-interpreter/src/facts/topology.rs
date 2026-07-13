//! Dialect-neutral body topology enumeration.
//!
//! Backward analyses need the *shape* of a body: which blocks and graph
//! nodes exist (including bodies nested inside structured statements), each
//! block's statements, the CFG successor relation, each block's *feeders* —
//! the statements whose rules can translate demand on that block's
//! parameters (terminators targeting it, statements owning it) — and each
//! graph port's *boundary* (the statement owning the graph, and the port's
//! slot index). This is topology only — uses/defs/edge-argument *semantics*
//! stay in dialect [`Interpretable`](crate::Interpretable) rules; the
//! enumeration consumes the generic [`HasSuccessors`]/[`HasBlocks`]/
//! [`HasCfgs`]/[`HasDigraphs`]/[`HasUngraphs`] contract every dialect
//! derives.

use std::collections::{HashMap, HashSet};

use kirin_ir::{
    Block, Cfg, DiGraph, Dialect, GetInfo, HasBlocks, HasCfgs, HasDigraphs, HasSuccessors,
    HasUngraphs, Port, SSAValue, StageInfo, Statement, UnGraph,
};

use crate::Body;

/// The shape of one block: its statements and CFG successors.
#[derive(Clone, Debug)]
pub struct BlockTopology {
    pub block: Block,
    /// Statements in program order; the terminator, if any, is last.
    pub stmts: Vec<Statement>,
    /// CFG successor blocks (targets of the block's terminator).
    pub successors: Vec<Block>,
    /// `true` for blocks nested inside a statement (structured bodies),
    /// `false` for the analyzed body's own top-level blocks.
    pub nested: bool,
}

/// The shape of one graph body: its node statements, in declaration order.
///
/// Order is enumeration order, not a schedule — execution scheduling is the
/// walker's job, and backward prepasses only need *all* statements.
#[derive(Clone, Debug)]
pub struct GraphTopology {
    /// `Body::DiGraph(..)` or `Body::UnGraph(..)`.
    pub graph: Body,
    pub stmts: Vec<Statement>,
    /// `true` for graphs nested inside a statement, `false` for the analyzed
    /// body itself.
    pub nested: bool,
}

/// Where a graph port sits on its owning statement's boundary.
///
/// This is a **location**, not a value mapping: the owning statement's
/// dialect rule translates port/index into its operands, captures, or
/// results — for values (forward) and demand (backward) alike.
#[derive(Clone, Copy, Debug)]
pub struct PortBoundary {
    /// The statement that owns the graph.
    pub owner: Statement,
    /// Which boundary slot this port occupies.
    pub index: usize,
}

/// The shape of a body: all blocks and graph parts (the analyzed body's own
/// plus structured bodies, recursively), the block-feeder index, and the
/// graph-port boundary index.
#[derive(Clone, Debug, Default)]
pub struct BodyTopology {
    pub blocks: Vec<BlockTopology>,
    pub graphs: Vec<GraphTopology>,
    feeders: HashMap<Block, Vec<Statement>>,
    port_boundary: HashMap<SSAValue, PortBoundary>,
}

/// Deprecated name for [`BodyTopology`]; kept for one release.
pub type CfgTopology = BodyTopology;

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
            .chain(self.graphs.iter().flat_map(|g| g.stmts.iter().copied()))
    }
}

/// Enumerate the topology of `body` in the finalized `stage`.
pub fn body_topology<L>(stage: &StageInfo<L>, body: Body) -> BodyTopology
where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let mut topology = BodyTopology::default();
    let mut visited = HashSet::new();
    match body {
        Body::Cfg(cfg) => {
            for block in cfg.blocks(stage) {
                collect_block(stage, block, false, &mut topology, &mut visited);
            }
        }
        Body::Block(block) => {
            collect_block(stage, block, false, &mut topology, &mut visited);
        }
        Body::DiGraph(graph) => {
            collect_digraph(stage, graph, false, &mut topology, &mut visited);
        }
        Body::UnGraph(graph) => {
            collect_ungraph(stage, graph, false, &mut topology, &mut visited);
        }
    }
    topology
}

/// Enumerate the topology of `cfg` in the finalized `stage`.
///
/// Deprecated spelling of [`body_topology`] over a `Cfg`; kept for one
/// release.
pub fn cfg_topology<L>(stage: &StageInfo<L>, cfg: &Cfg) -> BodyTopology
where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    body_topology(stage, Body::Cfg(*cfg))
}

fn collect_block<L>(
    stage: &StageInfo<L>,
    block: Block,
    nested: bool,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
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

    for &stmt in &stmts {
        collect_owned_bodies(stage, stmt, topology, visited);
    }
}

fn collect_digraph<L>(
    stage: &StageInfo<L>,
    graph: DiGraph,
    nested: bool,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let info = graph.expect_info(stage);
    let stmts: Vec<Statement> = info.graph().node_weights().copied().collect();
    record_ports(info.parent(), info.ports(), topology);
    topology.graphs.push(GraphTopology {
        graph: Body::DiGraph(graph),
        stmts: stmts.clone(),
        nested,
    });
    for stmt in stmts {
        collect_owned_bodies(stage, stmt, topology, visited);
    }
}

fn collect_ungraph<L>(
    stage: &StageInfo<L>,
    graph: UnGraph,
    nested: bool,
    topology: &mut BodyTopology,
    visited: &mut HashSet<Block>,
) where
    L: Dialect,
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let info = graph.expect_info(stage);
    let stmts: Vec<Statement> = info.graph().node_weights().copied().collect();
    record_ports(info.parent(), info.ports(), topology);
    topology.graphs.push(GraphTopology {
        graph: Body::UnGraph(graph),
        stmts: stmts.clone(),
        nested,
    });
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
    for<'a> L: HasSuccessors<'a> + HasBlocks<'a> + HasCfgs<'a> + HasDigraphs<'a> + HasUngraphs<'a>,
{
    let definition = stmt.definition(stage);
    let owned_blocks: Vec<Block> = definition.blocks().copied().collect();
    let owned_cfgs: Vec<Cfg> = definition.cfgs().copied().collect();
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
        collect_digraph(stage, owned, true, topology, visited);
    }
    for owned in owned_ungraphs {
        collect_ungraph(stage, owned, true, topology, visited);
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
