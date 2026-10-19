use crate::stage::Stage;
use kirin::prelude::*;
use kirin::pretty::PipelinePrintExt;
use kirin_test_utils::roundtrip::json_roundtrip;

#[test]
fn file_corpus_preserves_circuit_and_zx_graph_connectivity() {
    for file in ["bell_pair", "ghz", "rz_circuit", "bell_pair_zx"] {
        let text = std::fs::read_to_string(format!(
            "{}/programs/{file}.kirin",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap();
        let mut ir: Pipeline<Stage> = Pipeline::new();
        ir.parse(&text).unwrap();
        let restored = json_roundtrip(&ir);
        assert_eq!(ir.sprint(), restored.sprint(), "{file}");
        let mut reparsed: Pipeline<Stage> = Pipeline::new();
        reparsed.parse(&restored.sprint()).unwrap();
        assert_eq!(restored.sprint(), reparsed.sprint());
        for stage in restored.stages() {
            match stage {
                Stage::Circuit(stage) => kirin::ir::verify_derived(stage).unwrap(),
                Stage::ZX(stage) => {
                    kirin::ir::verify_derived(stage).unwrap();
                    assert!(
                        stage
                            .ungraph_arena()
                            .iter()
                            .any(|g| g.graph().edge_count() > 0)
                    );
                }
            }
        }
    }
}
