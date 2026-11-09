use std::collections::HashSet;

use smallvec::SmallVec;

use crate::arena::{GetInfo, GetInfoMut, Id};
use crate::node::linked_list::{LinkedList, LinkedListNode};
use crate::node::{CFGInfo, GraphMember, Port, PortParent};
use crate::{
    Block, BlockInfo, BlockParent, Body, CFG, DiGraph, DiGraphInfo, Dialect, SSAInfo, SSAKind,
    SSAValue, Statement, StatementInfo, StatementParent, Successor, UnGraph, UnGraphInfo, Use,
};

use super::IRMap;
use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{RewriteError, Rewriter, add_predecessor, add_use};

/// What the second walk of a copy made, for the last step to finish: the
/// copied statements, whose operand uses are recorded once every result is
/// bound, and the copied graphs, whose topology follows those uses.
#[derive(Default)]
struct Copied {
    statements: Vec<Statement>,
    /// Each original `DiGraph` with its copy, whose yields are set last.
    digraphs: Vec<(DiGraph, DiGraph)>,
    ungraphs: Vec<UnGraph>,
}

/// Deep copies. A copy takes two passes over the original: the first
/// allocates every CFG, graph, block, block argument, port and result the copy
/// needs, recording them in the [`IRMap`]; the second copies the statements,
/// whose operands and branch targets may name any of them, since a block may
/// branch to, or read a value from, a block listed after it. Last, the copied
/// statements' operand uses are recorded, and the copied graphs' topology is
/// built from them.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Copy `stmt` with everything nested in it, giving the copy fresh ids, and
    /// return the copy, detached: it has no parent until
    /// [`Rewriter::insert`] places it. Its results can be read before then,
    /// but a copy never placed leaves their readers dangling (see
    /// [`Rewriter::insert`]).
    ///
    /// Every operand and branch target inside the copy is mapped through
    /// `map`. An operand reading a value from outside the copy keeps reading it
    /// unless `map` gives a replacement; a block argument that `map` already
    /// gives a value for is not copied, and the copy reads that value. Results
    /// are always copied. Names are not copied. Afterwards `map` holds every
    /// value, block and statement the copy made.
    ///
    /// Rejects, leaving the stage untouched, if `stmt` is unknown, is a
    /// terminator ([`RewriteError::UnexpectedTerminator`]: it could not be
    /// inserted), or if the copy would read a value from `map` that is not
    /// live ([`RewriteError::UnknownValue`]). Records no event: nothing
    /// reachable changes until the insert.
    pub fn clone_statement(
        &mut self,
        stmt: Statement,
        map: &mut IRMap,
    ) -> Result<Statement, RewriteError> {
        if self.live_statement(stmt)?.definition.is_terminator() {
            return Err(RewriteError::UnexpectedTerminator);
        }
        self.check_replacements_live(stmt, map)?;

        self.reserve_statement(stmt, map);
        let mut copied = Copied::default();
        let copy = self.fill_statement(stmt, None, map, &mut copied);
        self.finish_copy(copied, map);
        self.detached.insert(copy);
        Ok(copy)
    }

    /// Copy `source` with everything nested in it, giving the copy fresh ids,
    /// and return the copy, detached: no statement owns it.
    ///
    /// Place its blocks with [`Rewriter::splice_cfg_after`], or make it the
    /// body of a new statement with [`Rewriter::create`]. A copy still
    /// detached when the `Rewriter` is dropped, including one a splice left
    /// empty, is tombstoned, leaving any reader of the values in its blocks
    /// dangling (see [`Rewriter::insert`]). Values are mapped as for
    /// [`Rewriter::clone_statement`]: in particular, mapping the arguments of
    /// `source`'s entry block gives a copy whose entry block has none.
    ///
    /// Rejects, leaving the stage untouched, if `source` is unknown
    /// ([`RewriteError::UnknownCfg`]), or if the copy would read a value from
    /// `map` that is not live ([`RewriteError::UnknownValue`]). Records no
    /// event.
    pub fn clone_cfg(&mut self, source: CFG, map: &mut IRMap) -> Result<CFG, RewriteError> {
        if source.get_info(self.stage).is_none() {
            return Err(RewriteError::UnknownCfg(source));
        }
        for stmt in self.body_statements(Body::CFG(source)) {
            self.check_replacements_live(stmt, map)?;
        }

        let copy = self.reserve_cfg(source, map);
        let mut copied = Copied::default();
        self.fill_body(Body::CFG(source), map, &mut copied);
        self.finish_copy(copied, map);
        self.detached_cfgs.insert(copy);
        Ok(copy)
    }

    /// Reject `stmt` if the copy would read a value from `map` that is not
    /// live, in `stmt` or anything nested in it.
    fn check_replacements_live(&self, stmt: Statement, map: &IRMap) -> Result<(), RewriteError> {
        let definition = &stmt.expect_info(self.stage).definition;
        for &operand in definition.arguments() {
            if let Some(replacement) = map.value(operand)
                && replacement.get_info(self.stage).is_none()
            {
                return Err(RewriteError::UnknownValue(replacement));
            }
        }
        // A `DiGraph`'s yields are read too.
        for &graph in definition.digraphs() {
            for &value in graph.expect_info(self.stage).yields() {
                if let Some(replacement) = map.value(value)
                    && replacement.get_info(self.stage).is_none()
                {
                    return Err(RewriteError::UnknownValue(replacement));
                }
            }
        }
        for body in bodies_of(definition) {
            for nested in self.body_statements(body) {
                self.check_replacements_live(nested, map)?;
            }
        }
        Ok(())
    }

    /// First pass for `stmt`: allocate its results, unbound, and its bodies.
    fn reserve_statement(&mut self, stmt: Statement, map: &mut IRMap) {
        let definition = &stmt.expect_info(self.stage).definition;
        let results: Vec<SSAValue> = definition.results().map(|&r| r.into()).collect();
        let bodies = bodies_of(definition);
        for result in results {
            let ty = result.expect_info(self.stage).ty().clone();
            let copy = self.stage.ssas.alloc(None);
            self.unbound_ssa_results.insert(copy, ty);
            map.map_value(result, copy);
        }
        for body in bodies {
            match body {
                Body::Block(block) => {
                    self.reserve_block(block, None, map);
                }
                Body::CFG(cfg) => {
                    self.reserve_cfg(cfg, map);
                }
                Body::DiGraph(graph) => self.reserve_digraph(graph, map),
                Body::UnGraph(graph) => self.reserve_ungraph(graph, map),
            }
        }
    }

    /// First pass for `cfg`: allocate a CFG with no parent and its blocks.
    fn reserve_cfg(&mut self, cfg: CFG, map: &mut IRMap) -> CFG {
        let copy = self
            .stage
            .nodes
            .cfgs
            .alloc_with_id(|id| CFGInfo::builder().id(id).blocks(LinkedList::new()).new());
        map.map_cfg(cfg, copy);
        for block in self.cfg_blocks(cfg) {
            let block_copy = self.reserve_block(block, Some(BlockParent::CFG(copy)), map);
            self.push_block(copy, block_copy);
        }
        copy
    }

    /// First pass for `graph`: allocate a graph with no parent, no members and
    /// copies of its ports, then reserve its members.
    fn reserve_digraph(&mut self, graph: DiGraph, map: &mut IRMap) {
        let (ports, edge_count) = {
            let info = graph.expect_info(self.stage);
            (info.ports().to_vec(), info.edge_count())
        };
        let copy = self.stage.nodes.digraphs.alloc_with_id(|id| {
            DiGraphInfo::from_parts(id, None, None, Vec::new(), edge_count, Vec::new())
        });
        let ports = self.copy_ports(&ports, PortParent::DiGraph(copy), map);
        if let Some(info) = copy.get_info_mut(self.stage) {
            info.ports = ports;
        }
        map.map_digraph(graph, copy);
        for stmt in self.body_statements(Body::DiGraph(graph)) {
            self.reserve_statement(stmt, map);
        }
    }

    /// First pass for `graph`, as for a `DiGraph`.
    fn reserve_ungraph(&mut self, graph: UnGraph, map: &mut IRMap) {
        let (ports, edge_count) = {
            let info = graph.expect_info(self.stage);
            (info.ports().to_vec(), info.edge_count())
        };
        let copy =
            self.stage.nodes.ungraphs.alloc_with_id(|id| {
                UnGraphInfo::from_parts(id, None, None, Vec::new(), edge_count)
            });
        let ports = self.copy_ports(&ports, PortParent::UnGraph(copy), map);
        if let Some(info) = copy.get_info_mut(self.stage) {
            info.ports = ports;
        }
        map.map_ungraph(graph, copy);
        for stmt in self.body_statements(Body::UnGraph(graph)) {
            self.reserve_statement(stmt, map);
        }
    }

    /// Allocate a copy of each of `ports`, in order, as ports of `parent`.
    fn copy_ports(&mut self, ports: &[Port], parent: PortParent, map: &mut IRMap) -> Vec<Port> {
        let mut copies = Vec::with_capacity(ports.len());
        for (index, &port) in ports.iter().enumerate() {
            let port = SSAValue::from(port);
            let ty = port.expect_info(self.stage).ty().clone();
            let copy = self
                .stage
                .ssas
                .alloc_with_id(|id| Some(SSAInfo::new(id, None, ty, SSAKind::Port(parent, index))));
            map.map_value(port, copy);
            copies.push(Port::from(Id::from(copy)));
        }
        copies
    }

    /// First pass for `block`: allocate an empty block under `parent` with the
    /// arguments `map` does not already give values for, then reserve its
    /// statements.
    fn reserve_block(
        &mut self,
        block: Block,
        parent: Option<BlockParent>,
        map: &mut IRMap,
    ) -> Block {
        let copy = self.stage.nodes.blocks.alloc_with_id(|id| {
            BlockInfo::builder()
                .maybe_parent(parent)
                .node(LinkedListNode::new(id))
                .arguments(Vec::new())
                .predecessors(SmallVec::new())
                .new()
        });
        let arguments: Vec<SSAValue> = block
            .expect_info(self.stage)
            .arguments
            .iter()
            .map(|&arg| arg.into())
            .collect();
        let mut copied_arguments = Vec::new();
        for argument in arguments {
            if map.value(argument).is_some() {
                continue;
            }
            let ty = argument.expect_info(self.stage).ty().clone();
            let copied = self.alloc_block_argument(copy, copied_arguments.len(), ty);
            map.map_value(argument, copied);
            copied_arguments.push(copied);
        }
        if let Some(info) = copy.get_info_mut(self.stage) {
            info.arguments = copied_arguments;
        }
        map.map_block(block, copy);
        for stmt in self.block_statements(block) {
            self.reserve_statement(stmt, map);
        }
        copy
    }

    /// Second pass for `stmt`: allocate its copy under `parent` with every id
    /// mapped, then copy its bodies' contents. Every statement copied is
    /// pushed onto `copied`, for its operand uses to be recorded once all
    /// results are bound.
    fn fill_statement(
        &mut self,
        stmt: Statement,
        parent: Option<StatementParent>,
        map: &mut IRMap,
        copied: &mut Copied,
    ) -> Statement {
        let original = stmt.expect_info(self.stage).definition.clone();
        let mut definition = original.clone();
        for operand in definition.arguments_mut() {
            *operand = map.lookup_or_original(*operand);
        }
        let mut results = Vec::new();
        for result in definition.results_mut() {
            let copy = map.value(*result).expect("results are reserved first");
            *result = copy.into();
            results.push(copy);
        }
        for successor in definition.successors_mut() {
            let target = successor.target();
            *successor = Successor::from_block(map.block(target).unwrap_or(target));
        }
        for block in definition.blocks_mut() {
            *block = map.block(*block).expect("bodies are reserved first");
        }
        for cfg in definition.cfgs_mut() {
            *cfg = map.cfg(*cfg).expect("bodies are reserved first");
        }
        for graph in definition.digraphs_mut() {
            *graph = map.digraph(*graph).expect("bodies are reserved first");
        }
        for graph in definition.ungraphs_mut() {
            *graph = map.ungraph(*graph).expect("bodies are reserved first");
        }
        let bodies = bodies_of(&definition);

        let copy = self
            .stage
            .statement_arena_mut()
            .alloc_with_id(|id| StatementInfo {
                node: LinkedListNode::new(id),
                parent,
                definition,
            });
        self.bind_results(copy, results);
        map.map_statement(stmt, copy);
        copied.statements.push(copy);
        // The copy owns the copied bodies, reserved empty by the first pass.
        for body in bodies {
            self.adopt_body(body, copy);
        }
        // Walk the original bodies to fill those copies, found through `map`.
        for body in bodies_of(&original) {
            self.fill_body(body, map, copied);
        }
        copy
    }

    /// Second pass for the original `body`: fill its copy.
    fn fill_body(&mut self, body: Body, map: &mut IRMap, copied: &mut Copied) {
        match body {
            Body::Block(block) => self.fill_block(block, map, copied),
            Body::CFG(cfg) => {
                for block in self.cfg_blocks(cfg) {
                    self.fill_block(block, map, copied);
                }
            }
            Body::DiGraph(graph) => {
                let copy = map.digraph(graph).expect("graphs are reserved first");
                for stmt in self.body_statements(body) {
                    let stmt_copy = self.fill_statement(
                        stmt,
                        Some(StatementParent::DiGraph(copy)),
                        map,
                        copied,
                    );
                    if let Some(info) = copy.get_info_mut(self.stage) {
                        info.add_node(stmt_copy);
                    }
                }
                copied.digraphs.push((graph, copy));
            }
            Body::UnGraph(graph) => {
                let copy = map.ungraph(graph).expect("graphs are reserved first");
                let members: Vec<(Statement, GraphMember)> =
                    graph.expect_info(self.stage).members().collect();
                for (stmt, member) in members {
                    let stmt_copy = self.fill_statement(
                        stmt,
                        Some(StatementParent::UnGraph(copy)),
                        map,
                        copied,
                    );
                    if let Some(info) = copy.get_info_mut(self.stage) {
                        match member {
                            GraphMember::Node(_) => info.add_node(stmt_copy),
                            GraphMember::Edge => info.add_edge_member(stmt_copy),
                        };
                    }
                }
                copied.ungraphs.push(copy);
            }
        }
    }

    /// Second pass for the original `block`: copy its statements into its copy
    /// in order, then its terminator, recording the terminator's edges.
    fn fill_block(&mut self, block: Block, map: &mut IRMap, copied: &mut Copied) {
        let copy = map.block(block).expect("blocks are reserved first");
        let (head, terminator) = {
            let info = block.expect_info(self.stage);
            (info.statements.head, info.terminator)
        };
        let mut prev = None;
        for stmt in self.chain_from(head) {
            let stmt_copy =
                self.fill_statement(stmt, Some(StatementParent::Block(copy)), map, copied);
            self.link_into_block(stmt_copy, copy, prev, None);
            prev = Some(stmt_copy);
        }
        let Some(terminator) = terminator else {
            return;
        };
        let terminator_copy =
            self.fill_statement(terminator, Some(StatementParent::Block(copy)), map, copied);
        let targets: HashSet<Block> = terminator_copy
            .expect_info(self.stage)
            .definition
            .successors()
            .map(|successor| successor.target())
            .collect();
        for target in targets {
            add_predecessor(self.stage, target, copy);
        }
        if let Some(info) = copy.get_info_mut(self.stage) {
            info.terminator = Some(terminator_copy);
        }
    }

    /// Last step of a copy: build each copied graph's topology and record the
    /// operand uses of every statement copied, now that every result is
    /// bound.
    fn finish_copy(&mut self, copied: Copied, map: &IRMap) {
        // `DiGraph`s: for each operand of a member that is another member's
        // result, add an edge from that member. This reads only the operands,
        // so it does not need the uses yet. Then copy the yields, mapping them
        // like operands, and record their uses.
        for (graph, copy) in copied.digraphs {
            for node in self.body_statements(Body::DiGraph(copy)) {
                let operands: Vec<SSAValue> = node
                    .expect_info(self.stage)
                    .definition
                    .arguments()
                    .copied()
                    .collect();
                for operand in operands {
                    self.add_operand_edge(copy, node, operand);
                }
            }
            let yields: Vec<SSAValue> = graph
                .expect_info(self.stage)
                .yields()
                .iter()
                .map(|&value| map.lookup_or_original(value))
                .collect();
            for (index, &value) in yields.iter().enumerate() {
                add_use(self.stage, value, Use::DiGraphYield { graph: copy, index });
            }
            if let Some(info) = copy.get_info_mut(self.stage) {
                info.extra.yields = yields;
            }
        }

        // `UnGraph`s: an edge is a wire read by two node slots, found from the
        // recorded uses. So snapshot each copied graph's wires, record the
        // operand uses, then resync the wires to add the edges.
        let wires: Vec<_> = copied
            .ungraphs
            .iter()
            .map(|&graph| {
                (
                    graph,
                    self.snapshot_wires(graph, self.wire_candidates(graph)),
                )
            })
            .collect();
        for &stmt in &copied.statements {
            let operands: Vec<SSAValue> = stmt
                .expect_info(self.stage)
                .definition
                .arguments()
                .copied()
                .collect();
            for (index, operand) in operands.into_iter().enumerate() {
                add_use(self.stage, operand, Use::StatementOperand { stmt, index });
            }
        }
        for (graph, wires) in wires {
            self.resync_wires(graph, wires);
        }
    }

    /// The values that may be wires of `graph`: its edge statements' results
    /// and its ports. `snapshot_wires` keeps only the wires among them.
    fn wire_candidates(&self, graph: UnGraph) -> Vec<SSAValue> {
        let info = graph.expect_info(self.stage);
        let ports = info.ports().iter().map(|&port| SSAValue::from(port));
        let results = info.edge_statements().flat_map(|stmt| {
            stmt.expect_info(self.stage)
                .definition
                .results()
                .map(|&result| SSAValue::from(result))
                .collect::<Vec<_>>()
        });
        ports.chain(results).collect()
    }

    /// Append the unlinked `block` to the end of `cfg`'s block list.
    fn push_block(&mut self, cfg: CFG, block: Block) {
        let tail = cfg.expect_info(self.stage).blocks.tail;
        match tail {
            Some(tail) => {
                if let Some(info) = tail.get_info_mut(self.stage) {
                    info.node.next = Some(block);
                }
            }
            None => {
                if let Some(info) = cfg.get_info_mut(self.stage) {
                    info.blocks.head = Some(block);
                }
            }
        }
        if let Some(info) = block.get_info_mut(self.stage) {
            info.node.prev = tail;
        }
        if let Some(info) = cfg.get_info_mut(self.stage) {
            info.blocks.tail = Some(block);
            info.blocks.len += 1;
        }
    }

    /// The statements directly in `body`: a block's chain then its
    /// terminator, every block's in order for a CFG, and a graph's members in
    /// member order.
    fn body_statements(&self, body: Body) -> Vec<Statement> {
        match body {
            Body::Block(block) => self.block_statements(block),
            Body::CFG(cfg) => self
                .cfg_blocks(cfg)
                .into_iter()
                .flat_map(|block| self.block_statements(block))
                .collect(),
            Body::DiGraph(graph) => graph
                .expect_info(self.stage)
                .members()
                .map(|(stmt, _)| stmt)
                .collect(),
            Body::UnGraph(graph) => graph
                .expect_info(self.stage)
                .members()
                .map(|(stmt, _)| stmt)
                .collect(),
        }
    }

    /// `cfg`'s blocks, in order.
    fn cfg_blocks(&self, cfg: CFG) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut current = cfg.get_info(self.stage).and_then(|info| info.blocks.head);
        while let Some(block) = current {
            blocks.push(block);
            current = block.get_info(self.stage).and_then(|info| info.node.next);
        }
        blocks
    }
}
