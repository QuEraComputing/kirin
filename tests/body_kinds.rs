//! Acceptance tests for generic interpreter bodies (issue #667): a mixed
//! language where regular Cfg-SSA code and DiGraph computational graphs
//! call into each other, plus linear (Block-bodied) callables.

use kirin::prelude::*;
use kirin_arith::{ArithConversionError, interpreter::DivisionByZero};
use kirin_interpreter::{
    ConcreteInterpreter, InterpreterError, SameStageLinker, StandardFrame, expect_single,
};
use kirin_test_languages::GraphFunctionLanguage;

/// Total error for the test engine: the framework error plus the value
/// conversion/trap errors the mixed language's rules can raise.
#[derive(Debug)]
enum TestError {
    Core(InterpreterError),
    ArithConversion(ArithConversionError),
    DivisionByZero,
}

impl From<InterpreterError> for TestError {
    fn from(error: InterpreterError) -> Self {
        Self::Core(error)
    }
}
impl From<ArithConversionError> for TestError {
    fn from(error: ArithConversionError) -> Self {
        Self::ArithConversion(error)
    }
}
impl From<DivisionByZero> for TestError {
    fn from(_: DivisionByZero) -> Self {
        Self::DivisionByZero
    }
}

type L = StageInfo<GraphFunctionLanguage>;
type Engine<'ir> =
    ConcreteInterpreter<'ir, L, i64, TestError, SameStageLinker, StandardFrame<i64, TestError>>;

fn parse(program: &str) -> Pipeline<L> {
    let mut pipeline: Pipeline<L> = Pipeline::new();
    ParsePipelineText::parse(&mut pipeline, program).expect("program parses");
    pipeline
}

fn run(pipeline: &Pipeline<L>, function: &str, args: &[i64]) -> Result<i64, TestError> {
    let mut interp: Engine<'_> = ConcreteInterpreter::new(pipeline).with_linker(SameStageLinker);
    expect_single(interp.call_by_name("test", function, args.iter().copied())?)
}

/// Entry path 1 (the call path): a Cfg-bodied `main` calls a DiGraph-bodied
/// callable; the engine builds a `DiGraphFrame`, runs the graph's arith
/// nodes in dependency order, and returns its yields.
#[test]
fn cfg_main_calls_digraph_function() {
    let pipeline = parse(
        r#"
stage @test fn @gadd(i64, i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @gadd(i64, i64) -> i64 digraph ^g0(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  yield %s;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 2 -> i64;
    %b = constant 3 -> i64;
    %r = call.named @gadd(%a, %b) -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 5);
}

/// Entry path 2 (the dialect-frame path): a statement inside a Cfg block
/// owns a DiGraph body and enters it with `Push` — the same way `scf.if`
/// enters its Block arms.
#[test]
fn cfg_statement_pushes_digraph_body() {
    let pipeline = parse(
        r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 20 -> i64;
    %b = constant 22 -> i64;
    %r = graph_eval %a, %b digraph ^g0(%x: i64, %y: i64) {
      %s = add %x, %y -> i64;
      yield %s;
    } -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 42);
}

/// A linear (single-Block) callable: the flat-instruction-list function
/// shape (QOS-style compile targets). Exits with `Return`.
#[test]
fn linear_block_callable() {
    let pipeline = parse(
        r#"
stage @test fn @ladd(i64, i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @ladd(i64, i64) -> i64 ^body(%x: i64, %y: i64) {
  %s = add %x, %y -> i64;
  ret %s;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 40 -> i64;
    %b = constant 2 -> i64;
    %r = call.named @ladd(%a, %b) -> i64;
    ret %r;
  }
}
"#,
    );
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 42);
}

/// Graph nodes run in dependency order, not declaration order: the yield
/// depends on a node declared after its operand producer.
#[test]
fn digraph_runs_in_topological_order() {
    let pipeline = parse(
        r#"
stage @test fn @g(i64) -> i64;
stage @test fn @main() -> i64;

specialize @test fn @g(i64) -> i64 digraph ^g0(%x: i64) {
  %d = mul %c, %c -> i64;
  %c = add %x, %x -> i64;
  yield %d;
}

specialize @test fn @main() -> i64 {
  ^entry() {
    %a = constant 3 -> i64;
    %r = call.named @g(%a) -> i64;
    ret %r;
  }
}
"#,
    );
    // (3 + 3)^2 = 36 — requires running `add` before `mul` despite text order.
    assert_eq!(run(&pipeline, "main", &[]).unwrap(), 36);
}
