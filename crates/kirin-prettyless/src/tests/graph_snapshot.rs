// ============================================================================
// Snapshot tests for digraph and ungraph pretty printing
// ============================================================================

use kirin_ir::{BuilderSSAKind, ResultValue, SSAValue};
use kirin_test_languages::{
    UngraphCompound, UngraphEdge, UngraphLanguage, UngraphNodeA, UngraphNodeB,
};

// --- PrettyPrint impl for UngraphLanguage ---

impl PrettyPrint for UngraphEdge {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        self.res.pretty_print(doc) + doc.text(" = wire")
    }
}

impl PrettyPrint for UngraphNodeA {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        let mut result = doc.text("node_a(") + self.param.pretty_print(doc);
        for port in &self.ports {
            result = result + doc.text(", ") + port.pretty_print(doc);
        }
        result + doc.text(")")
    }
}

impl PrettyPrint for UngraphNodeB {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        let mut result = doc.text("node_b(")
            + self.param0.pretty_print(doc)
            + doc.text(", ")
            + self.param1.pretty_print(doc);
        for port in &self.ports {
            result = result + doc.text(", ") + port.pretty_print(doc);
        }
        result + doc.text(")")
    }
}

impl PrettyPrint for UngraphCompound {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        _namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        let mut header = self.res.pretty_print(doc) + doc.text(" = compound(");
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                header = header + doc.text(", ");
            }
            header = header + arg.pretty_print(doc);
        }
        header = header + doc.text(") ");
        header + doc.print_ungraph(&self.body)
    }
}

impl PrettyPrint for UngraphLanguage {
    fn namespaced_pretty_print<'a, L: Dialect + PrettyPrint>(
        &self,
        doc: &'a Document<'a, L>,
        namespace: &[&str],
    ) -> ArenaDoc<'a>
    where
        L::Type: std::fmt::Display,
    {
        match self {
            UngraphLanguage::Edge(inner) => inner.namespaced_pretty_print(doc, namespace),
            UngraphLanguage::NodeA(inner) => inner.namespaced_pretty_print(doc, namespace),
            UngraphLanguage::NodeB(inner) => inner.namespaced_pretty_print(doc, namespace),
            UngraphLanguage::Compound(inner) => inner.namespaced_pretty_print(doc, namespace),
        }
    }
}

// --- Digraph snapshot tests (using SimpleLanguage) ---

#[test]
fn print_digraph_simple() {
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();

    let port_ref = stage.graph_port().index(0);

    let a = SimpleLanguage::op_constant(&mut stage, 1.0);
    let b = SimpleLanguage::op_add(&mut stage, port_ref, a.result);

    let dg = stage
        .digraph()
        .port(SimpleType::F64)
        .port_name("p0")
        .node(a.id)
        .node(b.id)
        .yield_value(b.result.into())
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_digraph(&dg);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn print_digraph_with_captures() {
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();

    let port0_ref = stage.graph_port().index(0);
    let port1_ref = stage.graph_port().index(1);
    let capture_ref = stage.graph_capture().index(0);

    let a = SimpleLanguage::op_add(&mut stage, port0_ref, capture_ref);
    let b = SimpleLanguage::op_add(&mut stage, a.result, port1_ref);

    let dg = stage
        .digraph()
        .port(SimpleType::F64)
        .port_name("p0")
        .port(SimpleType::F64)
        .port_name("p1")
        .capture(SimpleType::F64)
        .capture_name("theta")
        .node(a.id)
        .node(b.id)
        .yield_value(b.result.into())
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_digraph(&dg);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn print_digraph_named() {
    let mut stage: kirin_ir::StageInfo<SimpleLanguage> = kirin_ir::StageInfo::default();

    let port_ref = stage.graph_port().index(0);

    let a = SimpleLanguage::op_constant(&mut stage, 42.0);
    let b = SimpleLanguage::op_add(&mut stage, port_ref, a.result);

    let dg = stage
        .digraph()
        .name("my_graph")
        .port(SimpleType::F64)
        .port_name("x")
        .node(a.id)
        .node(b.id)
        .yield_value(b.result.into())
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_digraph(&dg);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

// --- Ungraph snapshot tests (using UngraphLanguage) ---

fn ungraph_stage() -> kirin_ir::StageInfo<UngraphLanguage> {
    kirin_ir::StageInfo::default()
}

fn make_edge(stage: &mut kirin_ir::StageInfo<UngraphLanguage>) -> (kirin_ir::Statement, SSAValue) {
    let result_id: ResultValue = stage.ssa_arena().next_id().into();
    let stmt = stage
        .statement()
        .definition(UngraphEdge { res: result_id })
        .new();
    let wire_ssa = stage
        .ssa()
        .ty(SimpleType::Any)
        .kind(BuilderSSAKind::Result(stmt, 0))
        .new();
    (stmt, wire_ssa)
}

fn make_node_a(
    stage: &mut kirin_ir::StageInfo<UngraphLanguage>,
    param: SSAValue,
    ports: Vec<SSAValue>,
) -> kirin_ir::Statement {
    stage
        .statement()
        .definition(UngraphNodeA { param, ports })
        .new()
}

fn make_node_b(
    stage: &mut kirin_ir::StageInfo<UngraphLanguage>,
    param0: SSAValue,
    param1: SSAValue,
    ports: Vec<SSAValue>,
) -> kirin_ir::Statement {
    stage
        .statement()
        .definition(UngraphNodeB {
            param0,
            param1,
            ports,
        })
        .new()
}

#[test]
fn print_ungraph_simple() {
    let mut stage = ungraph_stage();

    let port_ref = stage.graph_port().index(0);

    let (edge_stmt, wire_ssa) = make_edge(&mut stage);
    let n0 = make_node_a(&mut stage, port_ref, vec![wire_ssa]);
    let n1 = make_node_a(&mut stage, port_ref, vec![wire_ssa]);

    let ug = stage
        .ungraph()
        .port(SimpleType::Any)
        .port_name("p0")
        .edge(edge_stmt)
        .node(n0)
        .node(n1)
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_ungraph(&ug);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn print_ungraph_with_captures() {
    let mut stage = ungraph_stage();

    let port_ref = stage.graph_port().index(0);
    let capture_ref = stage.graph_capture().index(0);

    let (edge0_stmt, wire0) = make_edge(&mut stage);
    let (edge1_stmt, wire1) = make_edge(&mut stage);

    let n0 = make_node_a(&mut stage, port_ref, vec![wire0, wire1]);
    let n1 = make_node_b(&mut stage, capture_ref, port_ref, vec![wire0]);

    let ug = stage
        .ungraph()
        .port(SimpleType::Any)
        .port_name("p0")
        .capture(SimpleType::F64)
        .capture_name("theta")
        .edge(edge0_stmt)
        .edge(edge1_stmt)
        .node(n0)
        .node(n1)
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_ungraph(&ug);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn print_ungraph_named() {
    let mut stage = ungraph_stage();

    let port_ref = stage.graph_port().index(0);

    let (edge_stmt, wire_ssa) = make_edge(&mut stage);
    let n0 = make_node_a(&mut stage, port_ref, vec![wire_ssa]);
    let n1 = make_node_a(&mut stage, port_ref, vec![wire_ssa]);

    let ug = stage
        .ungraph()
        .name("my_ug")
        .port(SimpleType::Any)
        .port_name("p0")
        .edge(edge_stmt)
        .node(n0)
        .node(n1)
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_ungraph(&ug);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}

#[test]
fn print_ungraph_nested_compound() {
    let mut stage = ungraph_stage();

    // Inner ungraph port placeholder
    let inner_port_ref = stage.graph_port().index(0);

    let (inner_edge, inner_wire) = make_edge(&mut stage);
    let inner_node = make_node_b(&mut stage, inner_port_ref, inner_port_ref, vec![inner_wire]);
    let inner_ug = stage
        .ungraph()
        .port(SimpleType::Any)
        .port_name("ip0")
        .edge(inner_edge)
        .node(inner_node)
        .new();

    // Outer ungraph port/capture placeholders
    let outer_port_ref = stage.graph_port().index(0);
    let outer_capture_ref = stage.graph_capture().index(0);

    // Compound wrapping inner ungraph, takes outer port as arg
    let compound_res: ResultValue = stage.ssa_arena().next_id().into();
    let compound_stmt = stage
        .statement()
        .definition(UngraphCompound {
            args: vec![outer_port_ref],
            body: inner_ug,
            res: compound_res,
        })
        .new();
    let _compound_ssa = stage
        .ssa()
        .ty(SimpleType::Any)
        .kind(BuilderSSAKind::Result(compound_stmt, 0))
        .new();

    // Outer node uses capture and an edge
    let (outer_edge, outer_wire) = make_edge(&mut stage);
    let outer_node = make_node_a(&mut stage, outer_capture_ref, vec![outer_wire]);

    let ug = stage
        .ungraph()
        .port(SimpleType::Any)
        .port_name("op0")
        .capture(SimpleType::F64)
        .capture_name("phi")
        .edge(outer_edge)
        .node(outer_node)
        .node(compound_stmt)
        .new();

    let doc = Document::new(Default::default(), &stage);
    let arena_doc = doc.print_ungraph(&ug);
    let mut buf = String::new();
    arena_doc
        .render_fmt(doc.config().max_width, &mut buf)
        .unwrap();
    insta::assert_snapshot!(buf);
}
