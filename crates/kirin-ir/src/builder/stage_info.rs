use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::ops::{Deref, DerefMut};

use crate::arena::Arena;
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, SSAValue};
use crate::node::stmt::StatementParent;
use crate::stage::arenas::Arenas;
use crate::{Dialect, StageInfo, node::*};

/// Error returned by [`BuilderStageInfo::finalize`] when build-time SSAs
/// have not been resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeError {
    /// A `BuilderSSAKind::Unresolved` SSA was found — the builder did not resolve
    /// all placeholder references.
    UnresolvedSSA(SSAValue),
    /// A `BuilderSSAKind::Test` SSA was found — test-only SSAs must not appear in
    /// finalized IR.
    TestSSA(SSAValue),
    /// An SSA value has no type set (`ty` is `None`).
    MissingType(SSAValue),
}

impl fmt::Display for FinalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FinalizeError::UnresolvedSSA(ssa) => {
                write!(f, "unresolved SSA value {ssa} in finalized IR")
            }
            FinalizeError::TestSSA(ssa) => {
                write!(f, "test SSA value {ssa} in finalized IR")
            }
            FinalizeError::MissingType(ssa) => {
                write!(f, "SSA value {ssa} has no type set in finalized IR")
            }
        }
    }
}

impl std::error::Error for FinalizeError {}

/// A builder for constructing IR, with a separate SSA arena using [`BuilderSSAInfo`].
///
/// `BuilderStageInfo` provides the builder API surface for constructing IR:
/// creating SSA values, statements, blocks, regions, graphs, staged functions,
/// and specializations. The SSA arena holds [`BuilderSSAInfo`] (with `Option<L::Type>`
/// and [`BuilderSSAKind`]) during construction.
///
/// Call [`finalize`](BuilderStageInfo::finalize) to validate SSAs and convert to
/// a [`StageInfo`] with clean [`SSAInfo`](crate::node::ssa::SSAInfo) values.
pub struct BuilderStageInfo<L: Dialect> {
    pub(crate) nodes: Arenas<L>,
    pub(crate) ssas: Arena<SSAValue, BuilderSSAInfo<L>>,
}

impl<L: Dialect> Default for BuilderStageInfo<L> {
    fn default() -> Self {
        Self {
            nodes: Arenas::default(),
            ssas: Arena::default(),
        }
    }
}

impl<L: Dialect> fmt::Debug for BuilderStageInfo<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuilderStageInfo")
            .field("name", &self.nodes.name)
            .field("stage_id", &self.nodes.stage_id)
            .finish_non_exhaustive()
    }
}

impl<L: Dialect> From<StageInfo<L>> for BuilderStageInfo<L> {
    fn from(stage: StageInfo<L>) -> Self {
        Self {
            nodes: stage.nodes,
            ssas: stage.ssas.map(|info| BuilderSSAInfo::from(info)),
        }
    }
}

impl<L: Dialect> Deref for BuilderStageInfo<L> {
    type Target = Arenas<L>;

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<L: Dialect> DerefMut for BuilderStageInfo<L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nodes
    }
}

// ---- SSA-specific accessor methods ----

impl<L: Dialect> BuilderStageInfo<L> {
    /// Get a reference to the SSA values arena (builder variant).
    pub fn ssa_arena(&self) -> &Arena<SSAValue, BuilderSSAInfo<L>> {
        &self.ssas
    }

    /// Get a mutable reference to the SSA values arena (builder variant).
    pub fn ssa_arena_mut(&mut self) -> &mut Arena<SSAValue, BuilderSSAInfo<L>> {
        &mut self.ssas
    }
}

// ---- Finalization ----

impl<L: Dialect> BuilderStageInfo<L> {
    /// Validate the IR and return a [`StageInfo`] with clean SSA types.
    ///
    /// Checks that no `BuilderSSAKind::Unresolved` or `BuilderSSAKind::Test` values remain,
    /// and that all SSAs have types set. Converts the SSA arena from [`BuilderSSAInfo`]
    /// to [`SSAInfo`](crate::node::ssa::SSAInfo).
    pub fn finalize(self) -> Result<StageInfo<L>, FinalizeError> {
        // Validate all SSAs first
        for ssa_info in self.ssas.iter() {
            match ssa_info.builder_kind() {
                BuilderSSAKind::Unresolved(_) => {
                    return Err(FinalizeError::UnresolvedSSA(ssa_info.id()));
                }
                BuilderSSAKind::Test => {
                    return Err(FinalizeError::TestSSA(ssa_info.id()));
                }
                _ => {}
            }
            if ssa_info.ty().is_none() {
                return Err(FinalizeError::MissingType(ssa_info.id()));
            }
        }
        // All live SSAs are valid — infallible conversion.
        // Deleted SSAs may have unresolved kinds, so use map_live to handle
        // them separately. Deleted items are never accessed through public APIs.
        let ssas = self.ssas.map_live(
            |info| {
                info.finalize()
                    .expect("finalize: SSA validation passed but conversion failed")
            },
            // SAFETY: deleted items are tombstoned and never dereferenced.
            |_info| unsafe { std::mem::zeroed() },
        );
        Ok(StageInfo {
            nodes: self.nodes,
            ssas,
        })
    }

    /// Convert to [`StageInfo`] without validation.
    ///
    /// This is a `pub(crate)` escape hatch used by [`StageInfo::with_builder`]
    /// to round-trip through the builder. SSAs with missing types or unresolved
    /// kinds get best-effort defaults. Not part of the public API.
    pub(crate) fn finalize_unchecked(self) -> StageInfo<L> {
        let ssas = self.ssas.map_live(
            |info| {
                let id = info.id;
                let name = info.name;
                let kind = info
                    .kind
                    .as_resolved()
                    .unwrap_or(crate::node::ssa::SSAKind::Result(
                        crate::node::stmt::Statement(crate::arena::Id(0)),
                        0,
                    ));
                let uses = info.uses;
                match info.ty {
                    Some(ty) => crate::node::ssa::SSAInfo {
                        id,
                        name,
                        ty,
                        kind,
                        uses,
                    },
                    None => {
                        // SAFETY: The type field is zeroed. This is only used in
                        // the with_builder round-trip path where SSAs created by
                        // the parser may lack types. Deleted items are tombstoned
                        // and never dereferenced.
                        let mut ssa_info: crate::node::ssa::SSAInfo<L> =
                            unsafe { std::mem::zeroed() };
                        ssa_info.id = id;
                        ssa_info.name = name;
                        ssa_info.kind = kind;
                        ssa_info.uses = uses;
                        ssa_info
                    }
                }
            },
            // SAFETY: deleted items are tombstoned and never dereferenced.
            |_info| unsafe { std::mem::zeroed() },
        );
        StageInfo {
            nodes: self.nodes,
            ssas,
        }
    }
}

// ---- Builder methods (create/mutate nodes) ----

impl<L: Dialect> BuilderStageInfo<L> {
    /// Attach statements and an optional terminator to an existing block.
    pub fn attach_statements_to_block(
        &mut self,
        block: Block,
        stmts: &[Statement],
        terminator: Option<Statement>,
    ) {
        for &stmt in stmts {
            self.statements[stmt].parent = Some(StatementParent::Block(block));
        }
        if let Some(term) = terminator {
            self.statements[term].parent = Some(StatementParent::Block(block));
        }
        let linked = self.link_statements(stmts);
        let block_info = &mut self.blocks[block];
        block_info.statements = linked;
        block_info.terminator = terminator;
    }

    /// Move `real` block payload into `stub`, preserving external block IDs.
    pub fn remap_block_identity(&mut self, stub: Block, real: Block) {
        let mut real_info: BlockInfo<L> = (*self.blocks[real]).clone();

        // Collect statements by walking the linked list directly
        let mut statements = Vec::new();
        let mut current = real_info.statements.head().copied();
        while let Some(stmt) = current {
            statements.push(stmt);
            current = self.statements[stmt].node.next;
        }
        let terminator = real_info.terminator;

        for stmt in statements {
            self.statements[stmt].parent = Some(StatementParent::Block(stub));
        }
        if let Some(term) = terminator {
            self.statements[term].parent = Some(StatementParent::Block(stub));
        }

        for (idx, arg) in real_info.arguments.iter().copied().enumerate() {
            let arg_info = self
                .ssas
                .get_mut(arg)
                .expect("block argument SSA not found in builder stage");
            if let BuilderSSAKind::BlockArgument(owner, _) = arg_info.kind {
                debug_assert_eq!(
                    owner, real,
                    "unexpected block-arg owner while remapping block identity"
                );
                arg_info.kind = BuilderSSAKind::BlockArgument(stub, idx);
            }
        }

        real_info.node.ptr = stub;
        *self.blocks[stub] = real_info;
        self.blocks.delete(real);
    }

    /// Attach node statements and yield values to an existing digraph.
    pub fn attach_nodes_to_digraph(
        &mut self,
        dg: DiGraph,
        nodes: &[Statement],
        yields: &[SSAValue],
    ) {
        let dg_info = &self.digraphs[dg];
        let id = dg_info.id();

        let mut stmt_to_node: HashMap<Statement, petgraph::graph::NodeIndex> = HashMap::new();
        let mut graph = petgraph::Graph::<Statement, SSAValue, petgraph::Directed>::new();
        for &stmt_id in nodes {
            let ni = graph.add_node(stmt_id);
            stmt_to_node.insert(stmt_id, ni);
        }
        for &stmt_id in nodes {
            let consumer_ni = stmt_to_node[&stmt_id];
            let info = &self.statements[stmt_id];
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
        let dg_info = &mut self.digraphs[dg];
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
        let ug_info = &self.ungraphs[ug];
        let id = ug_info.id();
        let edge_count = ug_info.edge_count();
        let all_ports: Vec<crate::node::port::Port> = ug_info.ports().to_vec();

        let mut edge_ssa_set: HashSet<SSAValue> = HashSet::new();
        for &edge_stmt in edge_stmts {
            let info = &self.statements[edge_stmt];
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
            let info = &self.statements[node_stmt];
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
            let info = &self.statements[edge_stmt];
            for result in info.definition.results() {
                ssa_to_edge_stmt.insert((*result).into(), edge_stmt);
            }
        }
        for &node_stmt in node_stmts {
            let info = &self.statements[node_stmt];
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
            let info = &self.statements[stmt];
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
        let ug_info = &mut self.ungraphs[ug];
        ug_info.graph = new_graph;
        ug_info.edge_statements = bfs_edge_order;
    }

    pub fn link_statements(&mut self, ptrs: &[Statement]) -> LinkedList<Statement> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_stmt = &mut self.statements[current];
            if let Some(next_node) = current_stmt.node.next {
                let info = &self.statements[next_node];
                panic!("Statement already has a next node: {:?}", info.definition);
            }
            current_stmt.node.next = Some(next);

            let next_stmt = &mut self.statements[next];
            if let Some(prev_node) = next_stmt.node.prev {
                let info = &self.statements[prev_node];
                panic!(
                    "Statement already has a previous node: {:?}",
                    info.definition
                );
            }
            next_stmt.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }

    pub fn link_blocks(&mut self, ptrs: &[Block]) -> LinkedList<Block> {
        for window in ptrs.windows(2) {
            let current = window[0];
            let next = window[1];
            let current_block = &mut self.blocks[current];
            if let Some(next_node) = current_block.node.next {
                let info = &self.blocks[next_node];
                panic!("Block already has a next node: {:?}", info);
            }
            current_block.node.next = Some(next);

            let next_block = &mut self.blocks[next];
            if let Some(prev_node) = next_block.node.prev {
                let info = &self.blocks[prev_node];
                panic!("Block already has a previous node: {:?}", info);
            }
            next_block.node.prev = Some(current);
        }
        LinkedList {
            head: ptrs.first().copied(),
            tail: ptrs.last().copied(),
            len: ptrs.len(),
        }
    }
}
