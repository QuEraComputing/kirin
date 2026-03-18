use std::collections::{HashMap, HashSet, VecDeque};

use crate::arena::{Arena, GetInfo};
use crate::node::digraph::{DiGraph, DiGraphInfo};
use crate::node::function::CompileStage;
use crate::node::region::RegionInfo;
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind};
use crate::node::stmt::StatementParent;
use crate::node::ungraph::{UnGraph, UnGraphInfo};
use crate::{Dialect, InternTable, node::*};

/// The core stage info type used during both building and finalized access.
///
/// During construction, the SSA arena contains [`BuilderSSAInfo`] values with
/// `Option<L::Type>` and [`BuilderSSAKind`]. After
/// [`BuilderStageInfo::finalize`](crate::BuilderStageInfo::finalize), the arena
/// is validated to contain only resolved, typed SSAs.
#[derive(Debug)]
pub struct StageInfo<L: Dialect> {
    /// Optional human-readable name for this compilation stage.
    pub(crate) name: Option<GlobalSymbol>,
    pub(crate) stage_id: Option<CompileStage>,
    pub(crate) staged_functions: Arena<StagedFunction, StagedFunctionInfo<L>>,
    pub(crate) staged_name_policy: StagedNamePolicy,
    pub(crate) regions: Arena<Region, RegionInfo<L>>,
    pub(crate) blocks: Arena<Block, BlockInfo<L>>,
    pub(crate) statements: Arena<Statement, StatementInfo<L>>,
    pub(crate) ssas: Arena<SSAValue, BuilderSSAInfo<L>>,
    pub(crate) digraphs: Arena<DiGraph, DiGraphInfo<L>>,
    pub(crate) ungraphs: Arena<UnGraph, UnGraphInfo<L>>,
    pub(crate) symbols: InternTable<String, Symbol>,
}

impl<L> Default for StageInfo<L>
where
    L: Dialect,
{
    fn default() -> Self {
        Self {
            name: None,
            stage_id: None,
            staged_functions: Arena::default(),
            staged_name_policy: StagedNamePolicy::default(),
            regions: Arena::default(),
            blocks: Arena::default(),
            statements: Arena::default(),
            ssas: Arena::default(),
            digraphs: Arena::default(),
            ungraphs: Arena::default(),
            symbols: InternTable::default(),
        }
    }
}

impl<L> Clone for StageInfo<L>
where
    L: Dialect,
    StatementInfo<L>: Clone,
    BuilderSSAInfo<L>: Clone,
{
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            stage_id: self.stage_id,
            staged_functions: self.staged_functions.clone(),
            staged_name_policy: self.staged_name_policy,
            regions: self.regions.clone(),
            blocks: self.blocks.clone(),
            statements: self.statements.clone(),
            ssas: self.ssas.clone(),
            digraphs: self.digraphs.clone(),
            ungraphs: self.ungraphs.clone(),
            symbols: self.symbols.clone(),
        }
    }
}

impl<L: Dialect> StageInfo<L> {
    /// Get the optional stage name for this context.
    pub fn name(&self) -> Option<GlobalSymbol> {
        self.name
    }

    /// Set the stage name for this context.
    pub fn set_name(&mut self, name: Option<GlobalSymbol>) {
        self.name = name;
    }

    /// Get the compile-stage ID assigned by the pipeline, if any.
    pub fn stage_id(&self) -> Option<CompileStage> {
        self.stage_id
    }

    /// Set the compile-stage ID for this context.
    pub fn set_stage_id(&mut self, id: Option<CompileStage>) {
        self.stage_id = id;
    }

    /// Get a reference to the statements arena.
    pub fn statement_arena(&self) -> &Arena<Statement, StatementInfo<L>> {
        &self.statements
    }

    /// Get a reference to the SSA values arena.
    pub fn ssa_arena(&self) -> &Arena<SSAValue, BuilderSSAInfo<L>> {
        &self.ssas
    }

    /// Get a mutable reference to the SSA values arena.
    pub fn ssa_arena_mut(&mut self) -> &mut Arena<SSAValue, BuilderSSAInfo<L>> {
        &mut self.ssas
    }

    /// Get a reference to the symbols intern table.
    pub fn symbol_table(&self) -> &InternTable<String, Symbol> {
        &self.symbols
    }

    /// Get a mutable reference to the symbols intern table.
    pub fn symbol_table_mut(&mut self) -> &mut InternTable<String, Symbol> {
        &mut self.symbols
    }

    /// Get a reference to the staged functions arena.
    pub fn staged_function_arena(&self) -> &Arena<StagedFunction, StagedFunctionInfo<L>> {
        &self.staged_functions
    }

    /// Get the policy controlling staged-function name/signature compatibility.
    pub fn staged_name_policy(&self) -> StagedNamePolicy {
        self.staged_name_policy
    }

    /// Set the policy controlling staged-function name/signature compatibility.
    pub fn set_staged_name_policy(&mut self, policy: StagedNamePolicy) {
        self.staged_name_policy = policy;
    }

    /// Get a reference to the regions arena.
    pub fn region_arena(&self) -> &Arena<Region, RegionInfo<L>> {
        &self.regions
    }

    /// Get a reference to the blocks arena.
    pub fn block_arena(&self) -> &Arena<Block, BlockInfo<L>> {
        &self.blocks
    }

    /// Get a reference to the directed graph arena.
    pub fn digraph_arena(&self) -> &Arena<DiGraph, DiGraphInfo<L>> {
        &self.digraphs
    }

    /// Get a reference to the undirected graph arena.
    pub fn ungraph_arena(&self) -> &Arena<UnGraph, UnGraphInfo<L>> {
        &self.ungraphs
    }

    /// Attach statements and an optional terminator to an existing block.
    pub fn attach_statements_to_block(
        &mut self,
        block: Block,
        stmts: &[Statement],
        terminator: Option<Statement>,
    ) {
        for &stmt in stmts {
            stmt.expect_info_mut(self).parent = Some(StatementParent::Block(block));
        }
        if let Some(term) = terminator {
            term.expect_info_mut(self).parent = Some(StatementParent::Block(block));
        }
        let linked = self.link_statements(stmts);
        let block_info = block.expect_info_mut(self);
        block_info.statements = linked;
        block_info.terminator = terminator;
    }

    /// Move `real` block payload into `stub`, preserving external block IDs.
    pub fn remap_block_identity(&mut self, stub: Block, real: Block) {
        let mut real_info = real.expect_info(self).clone();
        let statements: Vec<_> = real.statements(self).collect();
        let terminator = real.terminator(self);

        for stmt in statements {
            stmt.expect_info_mut(self).parent = Some(StatementParent::Block(stub));
        }
        if let Some(term) = terminator {
            term.expect_info_mut(self).parent = Some(StatementParent::Block(stub));
        }

        for (idx, arg) in real_info.arguments.iter().copied().enumerate() {
            let arg_info = arg.expect_info_mut(self);
            if let BuilderSSAKind::BlockArgument(owner, _) = arg_info.kind {
                debug_assert_eq!(
                    owner, real,
                    "unexpected block-arg owner while remapping block identity"
                );
                arg_info.kind = BuilderSSAKind::BlockArgument(stub, idx);
            }
        }

        real_info.node.ptr = stub;
        *stub.expect_info_mut(self) = real_info;
        self.blocks.delete(real);
    }

    /// Attach node statements and yield values to an existing digraph.
    pub fn attach_nodes_to_digraph(
        &mut self,
        dg: DiGraph,
        nodes: &[Statement],
        yields: &[SSAValue],
    ) {
        let dg_info = dg.expect_info(self);
        let id = dg_info.id();

        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph = petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();
        for &stmt_id in nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }
        for &stmt_id in nodes {
            let consumer_ni = stmt_to_node[&stmt_id];
            let info = stmt_id.expect_info(self);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                let ssa_info = self.ssas.get(operand).expect("SSAValue not found in stage");
                if let BuilderSSAKind::Result(producer_stmt, _) = ssa_info.kind
                    && let Some(&producer_ni) = stmt_to_node.get(&producer_stmt)
                {
                    graph.add_edge(producer_ni, consumer_ni, operand);
                }
            }
        }
        for &stmt_id in nodes {
            let info = &mut self.statements[stmt_id];
            info.parent = Some(StatementParent::DiGraph(id));
        }
        let dg_info = dg.expect_info_mut(self);
        dg_info.graph = graph;
        dg_info.yields = yields.to_vec();
    }

    /// Attach edge and node statements to an existing ungraph.
    pub fn attach_nodes_to_ungraph(
        &mut self,
        ug: UnGraph,
        edge_stmts: &[Statement],
        node_stmts: &[Statement],
    ) {
        let ug_info = ug.expect_info(self);
        let id = ug_info.id();
        let edge_count = ug_info.edge_count();
        let all_ports: Vec<crate::node::port::Port> = ug_info.ports().to_vec();

        let mut edge_ssa_set: HashSet<SSAValue> = HashSet::new();
        for &edge_stmt in edge_stmts {
            let info = edge_stmt.expect_info(self);
            for result in info.definition.results() {
                edge_ssa_set.insert((*result).into());
            }
        }
        let boundary_ssa_set: HashSet<SSAValue> = all_ports
            .iter()
            .take(edge_count)
            .map(|p| (*p).into())
            .collect();

        let mut edge_ssa_to_nodes: HashMap<SSAValue, Vec<Statement>> = HashMap::new();
        for &node_stmt in node_stmts {
            let info = node_stmt.expect_info(self);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if edge_ssa_set.contains(&operand) || boundary_ssa_set.contains(&operand) {
                    edge_ssa_to_nodes
                        .entry(operand)
                        .or_default()
                        .push(node_stmt);
                }
            }
        }
        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() > 2 {
                panic!(
                    "UnGraph constraint violated: edge SSAValue {} is used by {} node statements (max 2)",
                    ssa,
                    nodes.len()
                );
            }
        }

        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();
        for &stmt_id in node_stmts {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }
        for (ssa, nodes) in &edge_ssa_to_nodes {
            if nodes.len() == 2 {
                graph.add_edge(stmt_to_node[&nodes[0]], stmt_to_node[&nodes[1]], *ssa);
            }
        }

        let mut visited_nodes: HashSet<petgraph::graph::NodeIndex> = HashSet::new();
        let mut visited_edges: HashSet<SSAValue> = HashSet::new();
        let mut bfs_node_order: Vec<petgraph::graph::NodeIndex> = Vec::new();
        let mut bfs_edge_order: Vec<Statement> = Vec::new();
        let mut queue: VecDeque<petgraph::graph::NodeIndex> = VecDeque::new();

        let mut ssa_to_edge_stmt: HashMap<SSAValue, Statement> = HashMap::new();
        for &edge_stmt in edge_stmts {
            let info = edge_stmt.expect_info(self);
            for result in info.definition.results() {
                ssa_to_edge_stmt.insert((*result).into(), edge_stmt);
            }
        }
        for &node_stmt in node_stmts {
            let info = node_stmt.expect_info(self);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            if operands.iter().any(|op| boundary_ssa_set.contains(op)) {
                let ni = stmt_to_node[&node_stmt];
                if visited_nodes.insert(ni) {
                    queue.push_back(ni);
                    bfs_node_order.push(ni);
                }
            }
        }
        while let Some(ni) = queue.pop_front() {
            let stmt = graph[ni];
            let info = stmt.expect_info(self);
            let operands: Vec<SSAValue> = info.definition.arguments().copied().collect();
            for operand in operands {
                if !visited_edges.contains(&operand) && edge_ssa_set.contains(&operand) {
                    visited_edges.insert(operand);
                    if let Some(&edge_stmt) = ssa_to_edge_stmt.get(&operand) {
                        bfs_edge_order.push(edge_stmt);
                    }
                    if let Some(nodes) = edge_ssa_to_nodes.get(&operand) {
                        for &other_stmt in nodes {
                            let other_ni = stmt_to_node[&other_stmt];
                            if visited_nodes.insert(other_ni) {
                                queue.push_back(other_ni);
                                bfs_node_order.push(other_ni);
                            }
                        }
                    }
                }
            }
        }
        for &stmt_id in node_stmts {
            let ni = stmt_to_node[&stmt_id];
            if visited_nodes.insert(ni) {
                bfs_node_order.push(ni);
            }
        }
        let bfs_edge_set: HashSet<Statement> = bfs_edge_order.iter().copied().collect();
        for &edge_stmt in edge_stmts {
            if !bfs_edge_set.contains(&edge_stmt) {
                bfs_edge_order.push(edge_stmt);
            }
        }

        let mut new_graph =
            petgraph::Graph::<Statement, SSAValue, petgraph::Undirected>::new_undirected();
        let mut old_to_new: HashMap<petgraph::graph::NodeIndex, petgraph::graph::NodeIndex> =
            HashMap::new();
        let mut reordered_nodes = Vec::with_capacity(bfs_node_order.len());
        for &old_ni in &bfs_node_order {
            let stmt = graph[old_ni];
            let new_ni = new_graph.add_node(stmt);
            old_to_new.insert(old_ni, new_ni);
            reordered_nodes.push(stmt);
        }
        for edge in graph.edge_indices() {
            let (src, dst) = graph.edge_endpoints(edge).unwrap();
            new_graph.add_edge(old_to_new[&src], old_to_new[&dst], graph[edge]);
        }

        for &stmt_id in &reordered_nodes {
            self.statements[stmt_id].parent = Some(StatementParent::UnGraph(id));
        }
        for &stmt_id in &bfs_edge_order {
            self.statements[stmt_id].parent = Some(StatementParent::UnGraph(id));
        }
        let ug_info = ug.expect_info_mut(self);
        ug_info.graph = new_graph;
        ug_info.edge_statements = bfs_edge_order;
    }
}
