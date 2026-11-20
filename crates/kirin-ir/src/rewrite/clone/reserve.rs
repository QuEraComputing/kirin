use crate::arena::{GetInfo, GetInfoMut, Id};
use crate::node::linked_list::LinkedList;
use crate::node::{CFGInfo, Port, PortParent};
use crate::{
    Block, BlockParent, Body, CFG, DiGraph, DiGraphInfo, Dialect, SSAInfo, SSAKind, SSAValue,
    Statement, UnGraph, UnGraphInfo,
};

use super::IRMap;
use crate::rewrite::bodies::bodies_of;
use crate::rewrite::rewriter::Rewriter;

/// The first pass of a deep copy: allocate everything the copy needs.
impl<'a, L: Dialect> Rewriter<'a, L> {
    /// First pass for `stmt`: allocate its results, unbound, and its bodies.
    pub(super) fn reserve_statement(&mut self, stmt: Statement, map: &mut IRMap) {
        let definition = &stmt.expect_info(self.stage).definition;
        let results: Vec<SSAValue> = definition.results().map(|&r| r.into()).collect();
        let bodies = bodies_of(definition);
        for result in results {
            let ty = result.expect_info(self.stage).ty().clone();
            let copy = self.new_result(ty);
            map.map_value(result, copy);
        }
        for body in bodies {
            match body {
                Body::Block(block) => self.reserve_block(block, None, map),
                Body::CFG(cfg) => {
                    self.reserve_cfg(cfg, map);
                }
                Body::DiGraph(graph) => self.reserve_digraph(graph, map),
                Body::UnGraph(graph) => self.reserve_ungraph(graph, map),
            }
        }
    }

    /// First pass for `cfg`: allocate a CFG with no parent and its blocks,
    /// unlinked. The second pass links them in order.
    pub(super) fn reserve_cfg(&mut self, cfg: CFG, map: &mut IRMap) -> CFG {
        let copy = self
            .stage
            .nodes
            .cfgs
            .alloc_with_id(|id| CFGInfo::builder().id(id).blocks(LinkedList::new()).new());
        map.map_cfg(cfg, copy);
        let blocks: Vec<Block> = cfg.blocks(self.stage).collect();
        for block in blocks {
            self.reserve_block(block, Some(BlockParent::CFG(copy)), map);
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

    /// First pass for `block`: allocate an empty block under `parent` with
    /// copies of its arguments, then reserve its statements.
    fn reserve_block(&mut self, block: Block, parent: Option<BlockParent>, map: &mut IRMap) {
        let types: Vec<L::Type> = block
            .expect_info(self.stage)
            .arguments
            .iter()
            .map(|&argument| {
                SSAValue::from(argument)
                    .expect_info(self.stage)
                    .ty()
                    .clone()
            })
            .collect();
        let copy = self.alloc_block(parent, types);
        let originals = &block.expect_info(self.stage).arguments;
        let copies = &copy.expect_info(self.stage).arguments;
        for (&argument, &copied) in originals.iter().zip(copies) {
            map.map_value(argument, copied);
        }
        map.map_block(block, copy);
        for stmt in self.block_statements(block) {
            self.reserve_statement(stmt, map);
        }
    }
}
