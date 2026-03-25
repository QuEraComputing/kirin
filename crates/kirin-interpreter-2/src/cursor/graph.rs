use kirin_ir::{DiGraph, Dialect, GetInfo, StageInfo, Statement, UnGraph};

/// Directed-graph shell cursor following stored graph node order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DiGraphCursor {
    digraph: DiGraph,
    nodes: Vec<Statement>,
    current: usize,
}

impl DiGraphCursor {
    pub(crate) fn new<L: Dialect>(stage: &StageInfo<L>, digraph: DiGraph) -> Self {
        let info = digraph.expect_info(stage);
        let nodes = info
            .graph()
            .node_indices()
            .map(|node| info.graph()[node])
            .collect();
        Self {
            digraph,
            nodes,
            current: 0,
        }
    }

    pub(crate) fn current(&self) -> Option<Statement> {
        self.nodes.get(self.current).copied()
    }

    pub(crate) fn advance(&mut self) {
        if self.current().is_some() {
            self.current += 1;
        }
    }
}

/// Undirected-graph shell cursor following stored BFS-canonical node order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnGraphCursor {
    ungraph: UnGraph,
    nodes: Vec<Statement>,
    current: usize,
}

impl UnGraphCursor {
    pub(crate) fn new<L: Dialect>(stage: &StageInfo<L>, ungraph: UnGraph) -> Self {
        let info = ungraph.expect_info(stage);
        let nodes = info
            .graph()
            .node_indices()
            .map(|node| info.graph()[node])
            .collect();
        Self {
            ungraph,
            nodes,
            current: 0,
        }
    }

    pub(crate) fn current(&self) -> Option<Statement> {
        self.nodes.get(self.current).copied()
    }

    pub(crate) fn advance(&mut self) {
        if self.current().is_some() {
            self.current += 1;
        }
    }
}
