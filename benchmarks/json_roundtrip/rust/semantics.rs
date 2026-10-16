//! Execute the actual Kirin concrete interpreter; this is outside all timers.
use super::{IR, spec};
use kirin_interpreter::{ConcreteInterpreter, InterpreterError, expect_single};
use serde_json::{Value, json};

pub fn validate(
    original: &IR,
    restored: &IR,
    suite: &spec::Suite,
    family: &spec::Family,
    size: usize,
) -> Value {
    let mut covered = std::collections::BTreeSet::new();
    let mut results = Vec::new();
    for input in &suite.samples {
        assert!(input.x.abs() <= suite.max_abs_input && input.y.abs() <= suite.max_abs_input);
        let (path, expected) = family.expected(size, input);
        covered.insert(path);
        let run = |ir: &IR| {
            let mut interpreter: ConcreteInterpreter<'_, _, i64, InterpreterError> =
                ConcreteInterpreter::new(ir);
            let args = [
                input.x,
                input.y,
                i64::from(input.outer),
                i64::from(input.inner),
            ];
            let actual: i64 = expect_single::<i64, InterpreterError>(
                interpreter.call_by_name("test", "benchmark", args).unwrap(),
            )
            .unwrap();
            assert_eq!(
                actual, expected,
                "real Rust runtime differs from semantic oracle for {path}"
            );
            actual
        };
        results.push(json!({"input": {"x": input.x, "y": input.y, "outer": input.outer, "inner": input.inner},
                           "path": path, "expected": expected, "original": run(original), "decoded": run(restored)}));
    }
    assert_eq!(
        covered.len(),
        family.paths.len(),
        "samples must cover every semantic path"
    );
    json!(results)
}
