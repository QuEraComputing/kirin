//! Undirected-graph test language for exercising graph-body derive macros.
//!
//! Provides generic node, edge, and compound-node types backed by `UnGraph`.
//! Tests model ZX-calculus-style patterns (spiders, wires, nested diagrams)
//! but the types are intentionally general-purpose so they can be reused in
//! parser, printer, interpreter, and analysis tests.
//!
//! Based on the text format in `docs/design/graph-ir-node.md`.

use kirin_ir::{Dialect, Placeholder, ResultValue, SSAKind, SSAValue, UnGraph};
use kirin_test_types::SimpleType;

/// Edge operation — creates an edge SSAValue in an ungraph.
///
/// Corresponds to `edge %w0 = wire -> EdgeType;` in the text format.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(edge, builders, type = SimpleType, crate = kirin_ir)]
pub struct UngraphEdge {
    pub res: ResultValue,
}

/// Unary node — one captured parameter plus variable edge connections.
///
/// Models operations like ZX spiders: `node_a(%param, %e0, %e1, ...);`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = SimpleType, crate = kirin_ir)]
pub struct UngraphNodeA {
    pub param: SSAValue,
    pub ports: Vec<SSAValue>,
}

/// Binary node — two captured parameters plus variable edge connections.
///
/// Models operations with two parameters: `node_b(%p0, %p1, %e0, ...);`
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = SimpleType, crate = kirin_ir)]
pub struct UngraphNodeB {
    pub param0: SSAValue,
    pub param1: SSAValue,
    pub ports: Vec<SSAValue>,
}

/// Compound node — contains an ungraph body, can be nested.
///
/// Operands map positionally to the inner ungraph's `[edge_ports ++ captures]`.
///
/// ```text
/// %out = compound(%e0, %e1, %captured) {
///   ungraph ^ug0(%ip0: T, %ip1: T) capture(%c: T) {
///     ...
///     yield %result;
///   }
/// } -> T;
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = SimpleType, crate = kirin_ir)]
pub struct UngraphCompound {
    pub args: Vec<SSAValue>,
    pub body: UnGraph,
    pub res: ResultValue,
}

/// Undirected-graph test language — composes edge, node, and compound types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = SimpleType, crate = kirin_ir)]
#[wraps]
pub enum UngraphLanguage {
    Edge(UngraphEdge),
    NodeA(UngraphNodeA),
    NodeB(UngraphNodeB),
    Compound(UngraphCompound),
}

#[cfg(test)]
mod tests {
    use super::*;
    use kirin_ir::*;

    fn new_stage() -> BuilderStageInfo<UngraphLanguage> {
        BuilderStageInfo::default()
    }

    /// Create an edge statement that produces a ResultValue.
    fn make_edge(stage: &mut BuilderStageInfo<UngraphLanguage>) -> (Statement, SSAValue) {
        let result_id: ResultValue = stage.ssa_arena().next_id().into();
        let stmt = stage
            .statement()
            .definition(UngraphEdge { res: result_id })
            .new();
        let wire_ssa = stage
            .ssa()
            .ty(SimpleType::Any)
            .kind(SSAKind::Result(stmt, 0))
            .new();
        (stmt, wire_ssa)
    }

    /// Create a NodeA statement with placeholder operands.
    fn make_node_a(stage: &mut BuilderStageInfo<UngraphLanguage>, n_ports: usize) -> Statement {
        let placeholder = stage.ssa().ty(SimpleType::Any).kind(SSAKind::Test).new();
        stage
            .statement()
            .definition(UngraphNodeA {
                param: placeholder,
                ports: vec![placeholder; n_ports],
            })
            .new()
    }

    /// Create a NodeB statement with placeholder operands.
    fn make_node_b(stage: &mut BuilderStageInfo<UngraphLanguage>, n_ports: usize) -> Statement {
        let placeholder = stage.ssa().ty(SimpleType::Any).kind(SSAKind::Test).new();
        stage
            .statement()
            .definition(UngraphNodeB {
                param0: placeholder,
                param1: placeholder,
                ports: vec![placeholder; n_ports],
            })
            .new()
    }

    /// Two nodes connected by an edge, with one boundary port and one capture.
    #[test]
    fn simple_ungraph() {
        let mut stage = new_stage();

        let (edge_stmt, _) = make_edge(&mut stage);
        let n1 = make_node_a(&mut stage, 1);
        let n2 = make_node_a(&mut stage, 1);

        let ug = stage
            .ungraph()
            .port(SimpleType::Any)
            .capture(SimpleType::F64)
            .edge(edge_stmt)
            .node(n1)
            .node(n2)
            .new();

        let info = ug.expect_info(&stage);
        assert_eq!(info.edge_ports().len(), 1);
        assert_eq!(info.capture_ports().len(), 1);
        assert_eq!(info.edge_statements().len(), 1);
        assert_eq!(info.graph().node_count(), 2);
    }

    /// Compound node wrapping an ungraph body — verifies HasUngraphs and
    /// HasArguments delegation through the language enum.
    #[test]
    fn compound_wrapping_ungraph() {
        let mut stage = new_stage();

        let (edge_stmt, _) = make_edge(&mut stage);
        let na = make_node_a(&mut stage, 1);
        let nb = make_node_b(&mut stage, 1);

        let ug = stage
            .ungraph()
            .port(SimpleType::Any)
            .port(SimpleType::Any)
            .capture(SimpleType::F64)
            .edge(edge_stmt)
            .node(na)
            .node(nb)
            .new();

        let compound_res: ResultValue = stage.ssa_arena().next_id().into();
        let placeholder = stage.ssa().ty(SimpleType::Any).kind(SSAKind::Test).new();
        let compound_stmt = stage
            .statement()
            .definition(UngraphCompound {
                args: vec![placeholder, placeholder],
                body: ug,
                res: compound_res,
            })
            .new();
        let _compound_ssa = stage
            .ssa()
            .ty(SimpleType::Any)
            .kind(SSAKind::Result(compound_stmt, 0))
            .new();

        let def = compound_stmt.definition(&stage);
        let ungraphs: Vec<_> = def.ungraphs().collect();
        assert_eq!(ungraphs.len(), 1, "compound should have one ungraph body");
        assert_eq!(*ungraphs[0], ug);

        let args: Vec<_> = def.arguments().collect();
        assert_eq!(args.len(), 2, "compound should have 2 args");
    }

    /// Edge operation has `is_edge() == true`.
    #[test]
    fn edge_is_edge() {
        let test_ssa: SSAValue = TestSSAValue(0).into();
        let edge = UngraphEdge {
            res: test_ssa.into(),
        };
        assert!(edge.is_edge(), "edge op should be an edge");
        assert!(!edge.is_terminator(), "edge op should not be a terminator");
    }

    /// Node operations are neither edge nor terminator.
    #[test]
    fn node_properties() {
        let test_ssa: SSAValue = TestSSAValue(0).into();
        let a = UngraphNodeA {
            param: test_ssa,
            ports: vec![],
        };
        assert!(!a.is_edge());
        assert!(!a.is_terminator());

        let b = UngraphNodeB {
            param0: test_ssa,
            param1: test_ssa,
            ports: vec![],
        };
        assert!(!b.is_edge());
        assert!(!b.is_terminator());
    }

    /// `#[wraps]` on the language enum correctly delegates all trait queries.
    #[test]
    fn language_wraps_delegation() {
        let test_ssa: SSAValue = TestSSAValue(0).into();
        let test_res: ResultValue = test_ssa.into();

        let edge: UngraphLanguage = UngraphEdge { res: test_res }.into();
        assert!(edge.is_edge(), "Edge variant should delegate is_edge");

        let a: UngraphLanguage = UngraphNodeA {
            param: test_ssa,
            ports: vec![TestSSAValue(1).into(), TestSSAValue(2).into()],
        }
        .into();
        assert!(!a.is_edge());
        let args: Vec<_> = a.arguments().collect();
        assert_eq!(args.len(), 3, "NodeA has param + 2 ports = 3 arguments");

        let mut stage = new_stage();
        let ug = stage.ungraph().new();
        let compound: UngraphLanguage = UngraphCompound {
            args: vec![test_ssa],
            body: ug,
            res: test_res,
        }
        .into();
        let ungraphs: Vec<_> = compound.ungraphs().collect();
        assert_eq!(ungraphs.len(), 1, "Compound should expose ungraph body");
    }

    /// Nested compound node — inner ungraph inside an outer ungraph.
    #[test]
    fn nested_compound() {
        let mut stage = new_stage();

        // Inner ungraph: one NodeB with an edge
        let (inner_edge, _) = make_edge(&mut stage);
        let inner_node = make_node_b(&mut stage, 1);
        let inner_ug = stage
            .ungraph()
            .port(SimpleType::Any)
            .edge(inner_edge)
            .node(inner_node)
            .new();

        // Wrap inner ungraph in a compound statement
        let compound_res: ResultValue = stage.ssa_arena().next_id().into();
        let placeholder = stage.ssa().ty(SimpleType::Any).kind(SSAKind::Test).new();
        let compound_stmt = stage
            .statement()
            .definition(UngraphCompound {
                args: vec![placeholder],
                body: inner_ug,
                res: compound_res,
            })
            .new();
        let _compound_ssa = stage
            .ssa()
            .ty(SimpleType::Any)
            .kind(SSAKind::Result(compound_stmt, 0))
            .new();

        // Outer ungraph: NodeA + the nested compound
        let (outer_edge, _) = make_edge(&mut stage);
        let outer_node = make_node_a(&mut stage, 1);

        let outer_ug = stage
            .ungraph()
            .port(SimpleType::Any)
            .port(SimpleType::Any)
            .capture(SimpleType::F64)
            .edge(outer_edge)
            .node(outer_node)
            .node(compound_stmt)
            .new();

        let outer_info = outer_ug.expect_info(&stage);
        assert_eq!(
            outer_info.graph().node_count(),
            2,
            "outer has NodeA + compound"
        );

        let inner_info = inner_ug.expect_info(&stage);
        assert_eq!(inner_info.graph().node_count(), 1, "inner has NodeB");
    }

    /// Compound node embedded in a block — mixing sequential and graph IR.
    #[test]
    fn compound_in_block() {
        let mut stage = new_stage();

        let (edge_stmt, _) = make_edge(&mut stage);
        let node = make_node_a(&mut stage, 1);

        let ug = stage
            .ungraph()
            .port(SimpleType::Any)
            .capture(SimpleType::F64)
            .edge(edge_stmt)
            .node(node)
            .new();

        let compound_res: ResultValue = stage.ssa_arena().next_id().into();
        let placeholder = stage.ssa().ty(SimpleType::Any).kind(SSAKind::Test).new();
        let compound_stmt = stage
            .statement()
            .definition(UngraphCompound {
                args: vec![placeholder],
                body: ug,
                res: compound_res,
            })
            .new();

        let block = stage.block().stmt(compound_stmt).new();

        let stmts: Vec<_> = block.statements(&stage).collect();
        assert_eq!(stmts.len(), 1);
        let def = stmts[0].definition(&stage);
        assert_eq!(def.ungraphs().count(), 1, "block stmt has ungraph body");
    }
}
