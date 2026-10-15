use super::*;

#[test]
fn benchmark_cfgs_are_valid_before_and_after_json() {
    for family in ["chain", "diamond", "nested_cfg"] {
        let mut ir = IR::new();
        ir.parse(&workload::workload(family, 4)).unwrap();
        shape::verify(&ir.stages()[0]);
        let restored: IR = serde_json::from_str(&serde_json::to_string(&ir).unwrap()).unwrap();
        shape::verify(&restored.stages()[0]);
        assert_eq!(
            shape::describe(&ir.stages()[0]),
            shape::describe(&restored.stages()[0])
        );
        assert_eq!(ir.sprint(), restored.sprint());
    }
}

#[test]
fn malformed_branch_argument_arity_is_rejected() {
    let text = workload::workload("diamond", 4).replace("br ^merge(%l1)", "br ^merge()");
    let mut ir = IR::new();
    // The parser may reject this earlier; otherwise our fixture verifier must.
    if ir.parse(&text).is_ok() {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                shape::verify(&ir.stages()[0]);
            }))
            .is_err()
        );
    }
}

#[test]
fn arithmetic_with_boolean_operand_is_rejected() {
    let text = workload::workload("chain", 4).replace("add %x, %x", "add %cond, %x");
    let mut ir = IR::new();
    if ir.parse(&text).is_ok() {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                shape::verify(&ir.stages()[0]);
            }))
            .is_err()
        );
    }
}
