use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;
use kirin_function::{Bind, Call, Return};

// ---------------------------------------------------------------------------
// Language definitions
// ---------------------------------------------------------------------------

/// Higher-level language: structured control flow (`if`) and lexical
/// lambdas that capture variables from the enclosing scope.
///
/// Block/Region-containing dialect types (SCF, Lambda) are inlined rather
/// than composed via `#[wraps]` because the recursive AST types overflow
/// trait resolution (E0275).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
enum HighLevel {
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[chumsky(format = "{res:name} = lambda {name} captures({captures}) {body} -> {res:type}")]
    Lambda {
        name: Symbol,
        captures: Vec<SSAValue>,
        body: Region,
        #[kirin(type = ArithType::default())]
        res: ResultValue,
    },

    #[chumsky(format = "if {condition} then {then_body} else {else_body}")]
    If {
        condition: SSAValue,
        then_body: Block,
        else_body: Block,
    },

    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
}

/// Lower-level language: unstructured control flow (`br`/`cond_br`/`ret`)
/// and lifted function bindings instead of inline lambdas.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type = ArithType)]
enum LowLevel {
    #[chumsky(format = "{body}")]
    Function { body: Region },

    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
    #[wraps]
    Bind(Bind<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
}

// ---------------------------------------------------------------------------
// Two-stage pipeline
// ---------------------------------------------------------------------------

/// A compilation pipeline with two stages backed by different dialects.
///
/// `@source` uses the higher-level IR (structured control flow, lambdas).
/// `@lowered` uses the lower-level IR (unstructured control flow, bind).
#[derive(Debug, StageMeta, RenderStage)]
enum Stage {
    #[stage(name = "source")]
    Source(StageInfo<HighLevel>),
    #[stage(name = "lowered")]
    Lowered(StageInfo<LowLevel>),
}

// ---------------------------------------------------------------------------
// Program text: both stages in a single pipeline
// ---------------------------------------------------------------------------

/// A full two-stage program.
///
/// **`@source`** — a single function `@main` using structured `if` and an
/// inline `lambda`.
///
/// **`@lowered`** — the same computation after lambda-lifting: `@adder` is a
/// standalone function and `@main` uses `bind` to capture the environment.
const PROGRAM: &str = r#"
stage @source fn @main(i64, i64) -> i64;

specialize @source fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    %doubled = add %x, %x -> i64;
    if %cond then ^then() {
      %r = add %doubled, %doubled -> i64;
    } else ^else() {
      %r2 = sub %doubled, %doubled -> i64;
    };
    %f = lambda @adder captures(%doubled) {
      ^bb0(%a: i64) {
        %sum = add %a, %a -> i64;
        ret %sum;
      }
    } -> i64;
    %result = call @adder(%x) -> i64;
    ret %result;
  }
}

stage @lowered fn @main(i64, i64) -> i64;
stage @lowered fn @adder(i64, i64) -> i64;

specialize @lowered fn @adder(i64, i64) -> i64 {
  ^entry(%capture: i64, %a: i64) {
    %sum = add %a, %capture -> i64;
    ret %sum;
  }
}

specialize @lowered fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %cond: i64) {
    %doubled = add %x, %x -> i64;
    %f = bind @adder captures(%doubled) -> i64;
    %result = call @adder(%doubled, %x) -> i64;
    ret %result;
  }
}
"#;

// ---------------------------------------------------------------------------
// Roundtrip: parse → print → re-parse → print → compare
// ---------------------------------------------------------------------------

fn main() {
    // Parse the full two-stage program into a single pipeline.
    let mut pipeline: Pipeline<Stage> = Pipeline::new();
    pipeline.parse(PROGRAM).expect("parse failed");

    // Render every function across all its stages.
    let rendered = pipeline.sprint();

    println!("{rendered}");

    // Re-parse the rendered text and render again.
    let mut pipeline2: Pipeline<Stage> = Pipeline::new();
    pipeline2
        .parse(&rendered)
        .expect("re-parse of rendered output failed");

    let rendered2 = pipeline2.sprint();

    assert_eq!(
        rendered.trim_end(),
        rendered2.trim_end(),
        "roundtrip mismatch"
    );
    println!("roundtrip OK");
}
