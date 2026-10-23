use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::ops::{Deref, DerefMut};

use indexmap::IndexMap;

use crate::arena::Arena;
use crate::builder::ungraph::UnGraphRole;
use crate::derived::{derive_mirrors, install_and_derive_mirrors_unchecked, install_mirrors};
use crate::node::ssa::{BuilderSSAInfo, BuilderSSAKind, SSAValue};
use crate::node::stmt::StatementParent;
use crate::stage::arenas::Arenas;
use crate::{Dialect, Finding, StageInfo, node::*};

/// Trait for types that provide mutable access to a [`BuilderStageInfo`].
///
/// Only [`BuilderStageInfo`] implements this. The trait exists so that
/// derive-generated builder functions can use it as a bound and produce a
/// helpful compiler error when someone accidentally passes `&mut StageInfo`
/// instead of `&mut BuilderStageInfo`.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a builder stage — cannot construct IR on finalized `StageInfo`",
    note = "use `stage.with_builder(|b| {{ ... }})` to get a `&mut BuilderStageInfo` for construction"
)]
pub trait AsBuildStage<L: Dialect> {
    /// Get a mutable reference to the underlying [`BuilderStageInfo`].
    fn as_build_stage(&mut self) -> &mut BuilderStageInfo<L>;
}

impl<L: Dialect> AsBuildStage<L> for BuilderStageInfo<L> {
    fn as_build_stage(&mut self) -> &mut BuilderStageInfo<L> {
        self
    }
}

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
    /// The IR being finalized has dangling references. These are all reported as [`Finding`]s
    MalformedIR(Vec<Finding>),
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
            FinalizeError::MalformedIR(findings) => {
                write!(f, "IR is malformed: ({} findings)", findings.len())?;
                for finding in findings {
                    write!(f, "\n  - {finding}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for FinalizeError {}

/// Builder for constructing IR within a single compilation stage.
///
/// `BuilderStageInfo` holds the same node arenas as [`StageInfo`] (blocks,
/// statements, cfgs, graphs, etc.) but uses [`BuilderSSAInfo`] for the SSA
/// arena — allowing `Option<L::Type>` and [`BuilderSSAKind`] placeholders during
/// construction.
///
/// # Lifecycle
///
/// 1. Create: `BuilderStageInfo::default()` or `BuilderStageInfo::from(stage_info)`
/// 2. Build: use builder methods to construct IR nodes
/// 3. Finalize: call [`finalize()`](Self::finalize) to validate and convert to [`StageInfo`]
///
/// # Building blocks
///
/// Statements and SSA values:
/// ```ignore
/// let mut stage = BuilderStageInfo::<MyDialect>::default();
///
/// // Create a statement (the dialect enum value is the "definition")
/// let stmt = stage.statement().definition(MyDialect::Nop).new();
///
/// // Create a typed SSA value
/// let ssa = stage.ssa().name("x").ty(MyType::I32).kind(BuilderSSAKind::Result(stmt, 0)).new();
/// ```
///
/// Blocks with arguments and placeholder substitution:
/// ```ignore
/// // Placeholders reference block arguments by index — resolved when the block is built
/// let arg0 = stage.block_argument().index(0);
/// let arg1 = stage.block_argument().index(1);
///
/// let add = stage.statement().definition(MyDialect::Add(arg0, arg1)).new();
/// let ret = stage.statement().definition(MyDialect::Return).new();
///
/// let block = stage
///     .block()
///     .argument(MyType::I32).arg_name("x")
///     .argument(MyType::I64).arg_name("y")
///     .stmt(add)
///     .terminator(ret)
///     .new();
/// ```
///
/// CFG (containers of blocks):
/// ```ignore
/// let entry = stage.block().new();
/// let exit = stage.block().new();
/// let cfg = stage.cfg().add_block(entry).add_block(exit).new();
/// ```
///
/// # Finalization
///
/// [`finalize()`](Self::finalize) validates that all SSA values have types and
/// resolved kinds, then converts the arena from [`BuilderSSAInfo`] to
/// [`SSAInfo`](crate::node::ssa::SSAInfo):
///
/// ```ignore
/// let finalized: StageInfo<MyDialect> = stage.finalize().unwrap();
///
/// // SSAInfo on finalized StageInfo has non-optional type and clean kind:
/// let ssa_info = some_ssa.expect_info(&finalized);
/// let ty: &MyType = ssa_info.ty();       // not Option
/// let kind: &SSAKind = ssa_info.kind();  // not BuilderSSAKind
/// ```
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
            ssas: stage.ssas.map(|opt| match opt {
                Some(info) => BuilderSSAInfo::from(info),
                None => {
                    // Deleted/tombstoned item — create a minimal placeholder
                    // BuilderSSAInfo. The arena slot is already marked deleted,
                    // so this value is never accessed through normal APIs.
                    BuilderSSAInfo::new(
                        SSAValue::from(crate::arena::Id(0)),
                        None,
                        None,
                        BuilderSSAKind::Test,
                    )
                }
            }),
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
    ///
    /// Deleted SSA slots become `None` tombstones in the resulting arena.
    pub fn finalize(self) -> Result<StageInfo<L>, FinalizeError> {
        // Validate all live SSAs first
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
        // Deleted SSAs become `None` tombstones (safe, no zeroed memory).
        let ssas = self.ssas.map_live(
            |info| {
                Some(
                    info.finalize()
                        .expect("finalize: SSA validation passed but conversion failed"),
                )
            },
            |_info| None,
        );
        let mut stage = StageInfo {
            nodes: self.nodes,
            ssas,
        };
        let mirrors = derive_mirrors(&stage)
            .map_err(|e| FinalizeError::MalformedIR(e.findings().to_vec()))?;
        install_mirrors(&mut stage, mirrors);
        Ok(stage)
    }

    /// Convert to [`StageInfo`] without validation.
    ///
    /// This is a `pub(crate)` escape hatch used by [`StageInfo::with_builder`]
    /// to round-trip through the builder. SSAs with unresolved kinds get a
    /// best-effort default. SSAs with missing types and deleted items become
    /// `None` tombstones. Not part of the public API.
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
                    Some(ty) => Some(crate::node::ssa::SSAInfo {
                        id,
                        name,
                        ty,
                        kind,
                        uses,
                    }),
                    // Type-less SSAs cannot produce a valid SSAInfo<L>.
                    // Tombstone them — callers should ensure types are set
                    // before finalization (e.g., via #[kirin(type = ...)]
                    // annotations or explicit builder calls).
                    None => None,
                }
            },
            // Deleted items become None tombstones — safe, no zeroed memory.
            |_info| None,
        );
        let mut stage = StageInfo {
            nodes: self.nodes,
            ssas,
        };
        install_and_derive_mirrors_unchecked(&mut stage);
        stage
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
        let _ = self.blocks.delete(real);
    }

    /// Attach node statements and yield values to an existing digraph, which
    /// must have no members yet.
    pub fn attach_nodes_to_digraph(
        &mut self,
        dg: DiGraph,
        nodes: &[Statement],
        yields: &[SSAValue],
    ) {
        for &stmt_id in nodes {
            let info = &self.statements[stmt_id];
            assert!(
                !info.definition.is_terminator(),
                "Cannot add terminator statement {:?} as a digraph node",
                info.definition
            );
            assert!(
                info.definition.successors().next().is_none(),
                "Cannot add statement {:?} with successors as a digraph node",
                info.definition
            );
        }

        let dg_info = &mut self.digraphs[dg];
        assert!(
            dg_info.members().next().is_none(),
            "Cannot attach nodes to digraph {dg:?}, which already has members"
        );
        dg_info.extra.yields = yields.to_vec();
        self.add_digraph_nodes(dg, nodes);
    }

    /// Make `nodes` members and nodes of `dg`, with one edge per operand slot
    /// reading another member's result. The graph's own statement → node map
    /// is the only index built.
    pub(crate) fn add_digraph_nodes(&mut self, dg: DiGraph, nodes: &[Statement]) {
        let parent = Some(StatementParent::DiGraph(dg));
        let graph = &mut self.nodes.digraphs[dg];
        for &stmt in nodes {
            self.nodes.statements[stmt].parent = parent;
            graph.add_node(stmt);
        }
        for &consumer in nodes {
            for &operand in self.nodes.statements[consumer].definition.arguments() {
                let ssa_info = self.ssas.get(operand).unwrap_or_else(|| {
                    panic!(
                        "SSAValue {operand} not found in stage (a placeholder already \
                         resolved by another graph is deleted and cannot be reused)"
                    )
                });
                if let BuilderSSAKind::Result(producer, _) = ssa_info.kind
                    && graph.node_idx_of(producer).is_some()
                {
                    graph.add_edge(producer, consumer, operand);
                }
            }
        }
    }

    /// Make `edge_stmts` and `node_stmts` members of `ug`, with one edge per
    /// wire (an edge statement's result or a boundary port) read by two node
    /// slots. The graph's own statement → node map is the only index built,
    /// and doubles as the BFS visited set.
    ///
    /// Members are stored in BFS order from the boundary: nodes reading a
    /// boundary port first, then the edge statements and nodes their wires
    /// reach, then the remaining nodes and edge statements in the order given.
    ///
    /// # Panics
    ///
    /// If a wire is read by more than two node operand slots.
    pub(crate) fn add_ungraph_members(
        &mut self,
        ug: UnGraph,
        edge_stmts: &[Statement],
        node_stmts: &[Statement],
    ) {
        let parent = Some(StatementParent::UnGraph(ug));
        for &stmt_id in node_stmts.iter().chain(edge_stmts) {
            self.nodes.statements[stmt_id].parent = parent;
        }
        let statements = &self.nodes.statements;
        let graph = &mut self.nodes.ungraphs[ug];

        let boundary_ssa_set: HashSet<SSAValue> =
            graph.edge_ports().iter().map(|&port| port.into()).collect();
        let mut ssa_to_edge_stmt: HashMap<SSAValue, Statement> = HashMap::new();
        for &edge_stmt in edge_stmts {
            for &result in statements[edge_stmt].definition.results() {
                ssa_to_edge_stmt.insert(result.into(), edge_stmt);
            }
        }

        // Edge SSA → the node statements reading it, one entry per operand slot.
        let mut edge_ssa_to_nodes: IndexMap<SSAValue, Vec<Statement>> = IndexMap::new();
        for &node_stmt in node_stmts {
            for &operand in statements[node_stmt].definition.arguments() {
                if ssa_to_edge_stmt.contains_key(&operand) || boundary_ssa_set.contains(&operand) {
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
                    "UnGraph constraint violated: edge SSAValue {ssa} is used by {} node statements (max 2 allowed for undirected graph edges)",
                    nodes.len()
                );
            }
        }

        // BFS from the nodes reading a boundary port, across edge-statement
        // wires. `add_node` and `add_edge_member` are false for a member
        // already added, so the graph's own map is the visited set.
        let mut queue: VecDeque<Statement> = VecDeque::new();
        for &node_stmt in node_stmts {
            let uses_boundary = statements[node_stmt]
                .definition
                .arguments()
                .any(|operand| boundary_ssa_set.contains(operand));
            if uses_boundary && graph.add_node(node_stmt) {
                queue.push_back(node_stmt);
            }
        }
        while let Some(node_stmt) = queue.pop_front() {
            for &operand in statements[node_stmt].definition.arguments() {
                let Some(&edge_stmt) = ssa_to_edge_stmt.get(&operand) else {
                    continue;
                };
                if !graph.add_edge_member(edge_stmt) {
                    continue;
                }
                for &other_stmt in &edge_ssa_to_nodes[&operand] {
                    if graph.add_node(other_stmt) {
                        queue.push_back(other_stmt);
                    }
                }
            }
        }
        // The rest, in the order given.
        for &node_stmt in node_stmts {
            graph.add_node(node_stmt);
        }
        for &edge_stmt in edge_stmts {
            graph.add_edge_member(edge_stmt);
        }

        for (&ssa, nodes) in &edge_ssa_to_nodes {
            if let &[n0, n1] = nodes.as_slice() {
                graph.add_edge(n0, n1, ssa);
            }
        }
    }

    /// Assert `stmt` may be an `UnGraph` member in `role`: an edge statement
    /// with no operands, or a node; never a terminator or a statement with
    /// successors.
    pub(crate) fn assert_ungraph_member(&self, stmt: Statement, role: UnGraphRole) {
        let definition = &self.statements[stmt].definition;
        match role {
            UnGraphRole::Edge => assert!(
                definition.is_edge(),
                "Cannot add non-edge statement {definition:?} as an ungraph edge"
            ),
            UnGraphRole::Node => assert!(
                !definition.is_edge(),
                "Cannot add edge statement {definition:?} as an ungraph node"
            ),
        }
        assert!(
            !definition.is_terminator(),
            "Cannot add terminator statement {definition:?} to an ungraph"
        );
        assert!(
            definition.successors().next().is_none(),
            "Cannot add statement {definition:?} with successors to an ungraph"
        );
        if role == UnGraphRole::Edge {
            assert!(
                definition.arguments().next().is_none(),
                "Cannot add edge statement {definition:?} with operands to an ungraph"
            );
        }
    }

    /// Attach edge and node statements to an existing ungraph, which must
    /// have no members yet.
    pub fn attach_nodes_to_ungraph(
        &mut self,
        ug: UnGraph,
        edge_stmts: &[Statement],
        node_stmts: &[Statement],
    ) {
        assert!(
            self.ungraphs[ug].members().next().is_none(),
            "Cannot attach statements to ungraph {ug:?}, which already has members"
        );
        for &stmt in edge_stmts {
            self.assert_ungraph_member(stmt, UnGraphRole::Edge);
        }
        for &stmt in node_stmts {
            self.assert_ungraph_member(stmt, UnGraphRole::Node);
        }

        self.add_ungraph_members(ug, edge_stmts, node_stmts);
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
