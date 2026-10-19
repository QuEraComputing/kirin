use super::*;

#[test]
fn original_and_decoded_payloads_execute_against_oracle() {
    let suite = spec::Suite::load();
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
        let cases = semantics::validate(&ir, &restored, &suite, suite.family(family, 4), 4);
        assert_eq!(cases.as_array().unwrap().len(), 7);
    }
}

#[test]
fn malformed_branch_argument_arity_is_rejected() {
    let original = workload::workload("diamond", 4);
    let text = original.replace("br ^merge(%left1)", "br ^merge()");
    assert_ne!(text, original);
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

#[test]
fn arithmetic_with_boolean_operand_is_rejected() {
    let original = workload::workload("chain", 4);
    let text = original.replace("add %x, %y", "add %outer, %y");
    assert_ne!(text, original);
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

#[test]
fn structurally_valid_wrong_arithmetic_fails_real_runtime_oracle() {
    let original = workload::workload("chain", 4);
    let text = original.replace("add %x, %y", "sub %x, %y");
    assert_ne!(text, original);
    let mut ir = IR::new();
    ir.parse(&text).unwrap();
    shape::verify(&ir.stages()[0]);
    let restored: IR = serde_json::from_str(&serde_json::to_string(&ir).unwrap()).unwrap();
    let suite = spec::Suite::load();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            semantics::validate(&ir, &restored, &suite, suite.family("chain", 4), 4);
        }))
        .is_err()
    );
}
