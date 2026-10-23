use super::*;
use kirin_interpreter::ConcreteInterpreter;

#[test]
fn new_workloads_execute_against_independent_oracle() {
    let suite = common::spec::Suite::load();
    for name in ["loop", "calls", "calls_same", "recursion"] {
        let mut ir = IR::new();
        ir.parse(&workload::workload(name, 4)).unwrap();
        let family = suite.family(name, 4);
        for input in &suite.samples {
            let mut interpreter: ConcreteInterpreter<'_, _, i64, InterpreterError> =
                ConcreteInterpreter::new(&ir);
            let args = [
                input.x,
                input.y,
                i64::from(input.outer),
                i64::from(input.inner),
            ];
            let actual = expect_single::<i64, InterpreterError>(
                interpreter.call_by_name("test", "benchmark", args).unwrap(),
            )
            .unwrap();
            let (path, expected) = family.expected(4, input);
            assert_eq!(actual, expected, "{name}/{path} differs from the oracle");
        }
    }
}
