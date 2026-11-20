use rustc_hash::FxHashSet;

use crate::arena::{GetInfo, GetInfoMut};
use crate::node::GraphMember;
use crate::node::linked_list::{LinkedList, LinkedListNode};
use crate::{
    Block, Body, DiGraph, Dialect, SSAValue, Statement, StatementInfo, StatementParent, Successor,
    UnGraph, Use,
};

use super::IRMap;
use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::{Rewriter, add_predecessor, add_use};

/// What the second pass of a copy made, for the last step to finish: the
/// copied statements, whose operand uses are recorded once every result is
/// bound, and the copied graphs, whose topology follows those uses.
#[derive(Default)]
pub(super) struct Copied {
    statements: Vec<Statement>,
    /// Each original `DiGraph` with its copy, whose yields are set last.
    digraphs: Vec<(DiGraph, DiGraph)>,
    ungraphs: Vec<UnGraph>,
}

/// The second pass of a deep copy, filling what the first reserved, and the
/// last step, recording uses and graph topology.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// Second pass for `stmt`: allocate its copy under `parent` with every id
    /// mapped, then copy its bodies' contents. Every statement copied is
    /// pushed onto `copied`, for its operand uses to be recorded once all
    /// results are bound.
    pub(super) fn fill_statement(
        &mut self,
        stmt: Statement,
        parent: Option<StatementParent>,
        map: &mut IRMap,
        copied: &mut Copied,
    ) -> Statement {
        let mut definition = stmt.expect_info(self.stage).definition.clone();
        // Taken before the ids are mapped: the original bodies, to fill.
        let original_bodies = bodies_of(&definition);
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
        for body in original_bodies {
            self.fill_body(body, map, copied);
        }
        copy
    }

    /// Second pass for the original `body`: fill its copy.
    pub(super) fn fill_body(&mut self, body: Body, map: &mut IRMap, copied: &mut Copied) {
        match body {
            Body::Block(block) => self.fill_block(block, map, copied),
            Body::CFG(cfg) => {
                let copy = map.cfg(cfg).expect("CFGs are reserved first");
                let mut tail = None;
                let blocks: Vec<Block> = cfg.blocks(self.stage).collect();
                for block in blocks {
                    self.fill_block(block, map, copied);
                    let block_copy = map.block(block).expect("blocks are reserved first");
                    self.link(LinkedList::single(block_copy), copy, tail, None);
                    tail = Some(block_copy);
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
            self.link(LinkedList::single(stmt_copy), copy, prev, None);
            prev = Some(stmt_copy);
        }
        let Some(terminator) = terminator else {
            return;
        };
        let terminator_copy =
            self.fill_statement(terminator, Some(StatementParent::Block(copy)), map, copied);
        let targets: FxHashSet<Block> = terminator_copy
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
    pub(super) fn finish_copy(&mut self, copied: Copied, map: &IRMap) {
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
}
