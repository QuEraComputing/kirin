//! Creating bodies owned by nothing.

use crate::node::CFGInfo;
use crate::node::linked_list::LinkedList;
use crate::{
    Block, Body, CFG, DiGraph, DiGraphInfo, Dialect, Port, PortParent, Rewriter, SSAInfo, SSAKind,
    UnGraph, UnGraphInfo,
};

impl<L: Dialect> Rewriter<'_, L> {
    /// Create an empty block with arguments of `arg_types`, owned by nothing.
    ///
    /// The block is meant to become a statement's body, such as a branch of
    /// an `scf.if`. Add statements to it like any block, starting with its
    /// terminator ([`InsertionPoint::Terminator`](crate::InsertionPoint::Terminator)).
    /// Then name the block in the definition of a new statement
    /// ([`Rewriter::create`]) or of a replacement
    /// ([`Rewriter::replace_statement`]); that statement becomes its parent.
    ///
    /// Its statements count as uses of the values they read as soon as they
    /// are added, even while the block is owned by nothing: such a value
    /// cannot be erased while they read it. [`Rewriter::erase_body`] discards
    /// the block early.
    ///
    /// A block no statement owns when the `Rewriter` is dropped is tombstoned
    /// together with everything it holds, unless the drop follows a panic, in
    /// which case it is left for inspection. Unlike [`Rewriter::erase_body`],
    /// the drop does not check that nothing outside still refers to the block:
    /// a branch to it, or a use of a value it defines, is left dangling. When
    /// a pass run by [`Program::run_pass`](crate::Program::run_pass) succeeds,
    /// [`verify_derived`](crate::verify_derived) then finds it and the stage is
    /// quarantined. When the pass fails, or the `Rewriter` is used directly,
    /// nothing reports it.
    ///
    /// Records no event: a block owned by nothing is not part of the IR yet.
    /// The statement that adopts it records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    /// Edits inside it record their usual events.
    pub fn new_block(&mut self, arg_types: impl IntoIterator<Item = L::Type>) -> Block {
        let block = self.alloc_block(None, arg_types);
        self.unowned_bodies.insert(Body::Block(block));
        block
    }

    /// Create an empty CFG, owned by nothing.
    ///
    /// The CFG is meant to become a statement's body, such as a lambda's.
    /// Create its blocks with [`Rewriter::append_block`],
    /// [`Rewriter::insert_block_before`] or [`Rewriter::insert_block_after`],
    /// which link them into its block list ([`Rewriter::new_block`] blocks
    /// cannot be moved into a CFG), and fill them. Then name the CFG in the
    /// definition of a new statement ([`Rewriter::create`]) or of a
    /// replacement ([`Rewriter::replace_statement`]); that statement becomes
    /// its parent.
    ///
    /// The rest is as for a [`Rewriter::new_block`] block. Its statements
    /// count as uses of the values they read as soon as they are added, even
    /// while the CFG is owned by nothing; [`Rewriter::erase_body`] discards
    /// it early. A CFG no statement owns when the `Rewriter` is dropped is
    /// tombstoned together with everything it holds, unless the drop follows a
    /// panic, without the checks [`Rewriter::erase_body`] makes.
    ///
    /// Records no event: a CFG owned by nothing is not part of the IR yet.
    /// The statement that adopts it records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    /// Edits inside it record their usual events.
    pub fn new_cfg(&mut self) -> CFG {
        let cfg = self.stage.nodes.cfgs.alloc_with_id(|id| {
            CFGInfo::builder()
                .id(id)
                .blocks(LinkedList::default())
                .new()
        });
        self.unowned_bodies.insert(Body::CFG(cfg));
        cfg
    }

    /// Create an empty `DiGraph` owned by nothing, with edge ports of
    /// `edge_port_types` followed by capture ports of `capture_types`.
    ///
    /// The graph is meant to become a statement's body. Read its ports with
    /// [`edge_ports`](crate::GraphInfo::edge_ports) and
    /// [`capture_ports`](crate::GraphInfo::capture_ports), and add nodes with
    /// [`Rewriter::create`] at
    /// [`InsertionPoint::DiGraph`](crate::InsertionPoint::DiGraph). Then name
    /// the graph in the definition of a new statement ([`Rewriter::create`])
    /// or of a replacement ([`Rewriter::replace_statement`]); that statement
    /// becomes its parent. The graph has no name and no yields.
    ///
    /// The rest is as for a [`Rewriter::new_block`] block. Its nodes count as
    /// uses of the values they read as soon as they are added, even while the
    /// graph is owned by nothing; [`Rewriter::erase_body`] discards it early.
    /// A graph no statement owns when the `Rewriter` is dropped is tombstoned
    /// together with everything it holds, unless the drop follows a panic,
    /// without the checks [`Rewriter::erase_body`] makes.
    ///
    /// Records no event: a graph owned by nothing is not part of the IR yet.
    /// The statement that adopts it records
    /// [`MutationEvent::InsertedStatement`](crate::MutationEvent::InsertedStatement)
    /// or [`MutationEvent::ChangedBodyNesting`](crate::MutationEvent::ChangedBodyNesting).
    /// Edits inside it record their usual events.
    pub fn new_digraph(
        &mut self,
        edge_port_types: impl IntoIterator<Item = L::Type>,
        capture_types: impl IntoIterator<Item = L::Type>,
    ) -> DiGraph {
        let graph = self.stage.nodes.digraphs.next_id();
        let (ports, edge_count) =
            self.alloc_ports(PortParent::DiGraph(graph), edge_port_types, capture_types);
        let info = DiGraphInfo::from_parts(graph, None, None, ports, edge_count, Vec::new());
        let _ = self.stage.nodes.digraphs.alloc(info);
        self.unowned_bodies.insert(Body::DiGraph(graph));
        graph
    }

    /// Create an empty `UnGraph` owned by nothing, with edge ports of
    /// `edge_port_types` followed by capture ports of `capture_types`.
    ///
    /// The graph is meant to become a statement's body. Read its ports with
    /// [`edge_ports`](crate::GraphInfo::edge_ports) and
    /// [`capture_ports`](crate::GraphInfo::capture_ports), and add nodes and
    /// edge statements with [`Rewriter::create`] at
    /// [`InsertionPoint::UnGraph`](crate::InsertionPoint::UnGraph). Then name
    /// the graph in the definition of a new statement ([`Rewriter::create`])
    /// or of a replacement ([`Rewriter::replace_statement`]); that statement
    /// becomes its parent. The graph has no name.
    ///
    /// The rest is as for [`Rewriter::new_digraph`]: its members count as uses
    /// as soon as they are added, [`Rewriter::erase_body`] discards it early,
    /// a graph no statement owns is tombstoned when the `Rewriter` drops, and
    /// creating it records no event.
    pub fn new_ungraph(
        &mut self,
        edge_port_types: impl IntoIterator<Item = L::Type>,
        capture_types: impl IntoIterator<Item = L::Type>,
    ) -> UnGraph {
        let graph = self.stage.nodes.ungraphs.next_id();
        let (ports, edge_count) =
            self.alloc_ports(PortParent::UnGraph(graph), edge_port_types, capture_types);
        let info = UnGraphInfo::from_parts(graph, None, None, ports, edge_count);
        let _ = self.stage.nodes.ungraphs.alloc(info);
        self.unowned_bodies.insert(Body::UnGraph(graph));
        graph
    }

    /// Allocate the ports of a graph `parent`: edge ports of `edge_port_types`
    /// followed by capture ports of `capture_types`, indexed in that order.
    /// Returns the ports and the number of edge ports.
    fn alloc_ports(
        &mut self,
        parent: PortParent,
        edge_port_types: impl IntoIterator<Item = L::Type>,
        capture_types: impl IntoIterator<Item = L::Type>,
    ) -> (Vec<Port>, usize) {
        let mut ports: Vec<Port> = Vec::new();
        let mut edge_count = 0;
        let types = edge_port_types
            .into_iter()
            .map(|ty| (ty, true))
            .chain(capture_types.into_iter().map(|ty| (ty, false)));
        for (index, (ty, is_edge)) in types.enumerate() {
            let port = self
                .stage
                .ssas
                .alloc_with_id(|id| Some(SSAInfo::new(id, None, ty, SSAKind::Port(parent, index))));
            ports.push(port.into());
            edge_count += usize::from(is_edge);
        }
        (ports, edge_count)
    }
}
