use kirin::prelude::*;
use kirin_test_languages::SimpleType;
use kirin_test_utils::roundtrip;

/// Edge operation — creates an edge SSAValue.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(edge, builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "$wire")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct Wire {
    #[kirin(type = SimpleType::Any)]
    res: ResultValue,
}

/// Unary node — one captured parameter plus one edge port.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "$node_a({param}, {port})")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct NodeA {
    param: SSAValue,
    port: SSAValue,
}

/// Function whose body is an UnGraph of `wire` edges and `node_a` nodes.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[chumsky(format = "fn {:name}{sig} {body}")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
struct UngraphFunction {
    body: UnGraph,
    sig: Signature<SimpleType>,
}

/// Ungraph test language composing edge, node, and function.
#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint)]
#[kirin(builders, type = SimpleType, crate = kirin::ir)]
#[chumsky(crate = kirin::parsers)]
#[wraps]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum UngraphTestLanguage {
    Wire(Wire),
    NodeA(NodeA),
    UngraphFunction(UngraphFunction),
}

// --- Roundtrip tests ---

#[test]
fn test_ungraph_wire_roundtrip() {
    roundtrip::assert_statement_roundtrip::<UngraphTestLanguage>("%w = wire", &[]);
}

#[test]
fn test_ungraph_node_roundtrip() {
    roundtrip::assert_statement_roundtrip::<UngraphTestLanguage>(
        "node_a(%p, %e0)",
        &[("p", SimpleType::F64), ("e0", SimpleType::Any)],
    );
}

// --- Pipeline-level UnGraph tests ---

/// An UnGraph-bodied function: `%x` and `%y` are boundary ports (wires), each
/// read once by `%s`; `%t` reads `%s`, which is not a wire.
const UNGRAPH_FN: &str = r#"
stage @test fn @by_ungraph(i64, i64) -> i64;
specialize @test fn @by_ungraph(i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  %t = mul %s, %s -> i64;
}
"#;

type GraphPipeline = Pipeline<StageInfo<kirin_test_languages::GraphFunctionLanguage>>;

fn parse_graph_pipeline(text: &str) -> GraphPipeline {
    let mut pipeline = GraphPipeline::new();
    pipeline.parse(text).expect("pipeline parses");
    pipeline
}

/// `pipeline`'s `@test` stage and the one UnGraph in it (asserting there is
/// exactly one).
fn test_stage_and_ungraph(
    pipeline: &GraphPipeline,
) -> (
    &StageInfo<kirin_test_languages::GraphFunctionLanguage>,
    UnGraph,
) {
    let stage = pipeline
        .stage(pipeline.stage_by_name("test").unwrap())
        .unwrap();
    let graphs: Vec<UnGraph> = stage.ungraph_arena().iter().map(|info| info.id()).collect();
    assert_eq!(graphs.len(), 1);
    (stage, graphs[0])
}

/// The graph has one edge, carried by its first boundary port, and every
/// node resolves to its own petgraph node.
fn assert_port_edge(pipeline: &GraphPipeline) {
    let (stage, ug) = test_stage_and_ungraph(pipeline);
    let info = ug.expect_info(stage);
    let port = SSAValue::from(info.edge_ports()[0]);
    let weights: Vec<SSAValue> = info.graph().edge_weights().copied().collect();
    assert_eq!(weights, [port]);
    for node in info.graph().node_indices() {
        assert_eq!(info.node_idx_of(info.graph()[node]), Some(node));
    }
    kirin::ir::verify_derived(stage).unwrap();
}

#[cfg(feature = "serde")]
#[test]
fn test_ungraph_json_restores_graph_mirror() {
    let pipeline = parse_graph_pipeline(UNGRAPH_FN);

    // The graph mirror is serialized like the other mirrors and trusted on
    // load; verification confirms it still agrees with the IR.
    let json = serde_json::to_string(&pipeline).unwrap();
    let restored: GraphPipeline = serde_json::from_str(&json).unwrap();

    let (stage, ug) = test_stage_and_ungraph(&pipeline);
    let (restored_stage, restored_ug) = test_stage_and_ungraph(&restored);
    let restored_info = restored_ug.expect_info(restored_stage);
    assert_eq!(
        restored_info.members().collect::<Vec<_>>(),
        ug.expect_info(stage).members().collect::<Vec<_>>()
    );
    assert_eq!(restored_info.graph().node_count(), 2);
    kirin::ir::verify_derived(restored_stage).unwrap();
}

#[test]
fn test_ungraph_text_roundtrip_after_rewrite() {
    let mut pipeline = parse_graph_pipeline(UNGRAPH_FN);
    let stage_id = pipeline.stage_by_name("test").unwrap();
    let (stage, ug) = test_stage_and_ungraph(&pipeline);
    let info = ug.expect_info(stage);
    let x = SSAValue::from(info.edge_ports()[0]);
    // `%t` is the node that does not read `%x`.
    let t = info
        .nodes()
        .find(|node| !node.arguments(stage).any(|&arg| arg == x))
        .unwrap();

    // `%t` now reads `%x` too, so the port gains a second reader: an edge.
    let stage = pipeline.stage_mut(stage_id).unwrap();
    kirin::ir::Rewriter::new(stage)
        .replace_operand(t, 0, x)
        .unwrap();
    assert_port_edge(&pipeline);

    // Print → parse → print is valid and stable after one round, and the
    // reparsed graph has the same edge.
    let printed = pipeline.sprint();
    let reparsed = parse_graph_pipeline(&printed);
    assert_port_edge(&reparsed);
    let printed_again = reparsed.sprint();
    assert_eq!(parse_graph_pipeline(&printed_again).sprint(), printed_again);
}

#[test]
fn test_ungraph_body_with_a_terminator_is_a_parse_error() {
    // An UnGraph has no terminator; parsing one must report an error, not
    // panic in the builder.
    let input = r#"
stage @test fn @g(i64, i64) -> i64;
specialize @test fn @g(i64, i64) -> i64 ungraph ^u0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  ret %s;
}
"#;
    let error = GraphPipeline::new()
        .parse(input)
        .expect_err("a terminator is not an ungraph member");
    assert!(
        format!("{error:?}").contains("cannot contain a terminator"),
        "unexpected error: {error:?}"
    );
}

/// Two nodes sharing `%w0`; `%w1` has one reader per node.
const WIRED_FN: &str = r#"
stage @test fn @wired() -> any;
specialize @test fn @wired() -> any ungraph ^ug0 {
  edge %w0 = wire;
  edge %w1 = wire;
  node_a(%w0, %w1);
  node_a(%w0, %w1);
}
"#;

type WirePipeline = Pipeline<StageInfo<UngraphTestLanguage>>;

fn parse_wire_pipeline(text: &str) -> WirePipeline {
    let mut pipeline = WirePipeline::new();
    pipeline.parse(text).expect("pipeline parses");
    pipeline
}

/// An edge statement inserted after the node that reads it prints in member
/// order, after that node, and still reparses to the same graph.
#[test]
fn test_ungraph_wire_inserted_after_its_reader_roundtrips() {
    let mut pipeline = parse_wire_pipeline(WIRED_FN);
    let stage_id = pipeline.stage_by_name("test").unwrap();
    let stage = pipeline.stage_mut(stage_id).unwrap();
    let ug = stage.ungraph_arena().iter().next().unwrap().id();
    let reader = ug.expect_info(stage).nodes().last().unwrap();

    let mut rw = kirin::ir::Rewriter::new(stage);
    let res = rw.new_result(SimpleType::Any);
    let wire = rw
        .insert_ungraph_edge(ug, UngraphTestLanguage::Wire(Wire { res }))
        .unwrap();
    rw.replace_operand(reader, 1, res).unwrap();
    drop(rw);
    kirin::ir::verify_derived(stage).unwrap();
    // The new wire is the last member, after `reader`, which reads it.
    let members: Vec<Statement> = ug.expect_info(stage).members().map(|(s, _)| s).collect();
    assert_eq!(members.last(), Some(&wire));

    let printed = pipeline.sprint();
    let reparsed = parse_wire_pipeline(&printed);
    assert_eq!(reparsed.sprint(), printed);
    let stage = reparsed
        .stage(reparsed.stage_by_name("test").unwrap())
        .unwrap();
    kirin::ir::verify_derived(stage).unwrap();
    let info = stage.ungraph_arena().iter().next().unwrap();
    assert_eq!(info.edge_statements().count(), 3);
    // %w0 joins both nodes; %w1 and the new wire have one reader each.
    assert_eq!(info.graph().edge_count(), 1);
}
