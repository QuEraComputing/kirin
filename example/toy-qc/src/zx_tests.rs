use crate::stage::Stage;
use crate::types::QubitType;
use crate::zx::{Wire, ZX};
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;

fn parse(text: &str) -> Pipeline<Stage> {
    let mut ir: Pipeline<Stage> = Pipeline::new();
    ir.parse(text).unwrap();
    ir
}

/// An edge statement inserted after the node that reads it prints in member
/// order, after that node, and still reparses to the same graph.
#[test]
fn a_wire_inserted_after_its_reader_roundtrips() {
    let text = std::fs::read_to_string(format!(
        "{}/programs/bell_pair_zx.kirin",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut ir = parse(&text);
    let id = ir.stage_by_name("zx").unwrap();
    let Some(Stage::ZX(stage)) = ir.stage_mut(id) else {
        panic!("@zx is a ZX stage");
    };
    let ug = stage.ungraph_arena().iter().next().unwrap().id();
    let h_box = ug
        .expect_info(stage)
        .nodes()
        .find(|&node| matches!(node.definition(stage), ZX::HBox(_)))
        .unwrap();

    let mut rw = Rewriter::new(stage);
    let res = rw.new_result(QubitType::Qubit);
    let wire = rw.insert_ungraph_edge(ug, ZX::Wire(Wire { res })).unwrap();
    rw.replace_operand(h_box, 1, res).unwrap();
    drop(rw);
    kirin::ir::verify_derived(stage).unwrap();
    // The new wire is the last member, after `h_box`, which reads it.
    let members: Vec<Statement> = ug.expect_info(stage).members().map(|(s, _)| s).collect();
    assert_eq!(members.last(), Some(&wire));

    let printed = ir.sprint();
    let reparsed = parse(&printed);
    assert_eq!(reparsed.sprint(), printed);
    for stage in reparsed.stages() {
        if let Stage::ZX(stage) = stage {
            kirin::ir::verify_derived(stage).unwrap();
            let info = stage.ungraph_arena().iter().next().unwrap();
            assert_eq!(info.edge_statements().count(), 3);
            // %w0 joins z_spider and h_box; %w1 and the new wire have one
            // reader each.
            assert_eq!(info.graph().edge_count(), 1);
        }
    }
}
