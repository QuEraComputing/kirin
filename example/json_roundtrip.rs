//! JSON-only round-trip benchmark; see docs/benchmarks/json-roundtrip.md.
use std::{hint::black_box, time::Instant};

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_cf::ControlFlow;
use kirin_function::Return;
use kirin_prettyless::PipelinePrintExt;
use kirin_test_languages::SimpleType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Dialect, HasParser, PrettyPrint, Serialize, Deserialize)]
#[kirin(builders, type = SimpleType)]
enum BenchmarkLanguage {
    #[chumsky(format = "fn {:name}{signature} {body}")]
    Function {
        body: CFG,
        signature: Signature<SimpleType>,
    },
    #[wraps]
    Arith(Arith<SimpleType>),
    #[wraps]
    Cf(ControlFlow<SimpleType>),
    #[wraps]
    Return(Return<SimpleType>),
}

#[path = "json_roundtrip/shape.rs"]
mod shape;
#[path = "json_roundtrip/workload.rs"]
mod workload;

type IR = Pipeline<StageInfo<BenchmarkLanguage>>;

fn main() {
    let mut args = std::env::args().skip(1);
    let family = args.next().expect("family: chain, diamond or nested_cfg");
    let size: usize = args.next().expect("number of adds").parse().unwrap();
    let repeats: usize = args.next().unwrap_or("30".into()).parse().unwrap();
    let warmups: usize = args.next().unwrap_or("5".into()).parse().unwrap();
    assert!(size > 0 && repeats > 0 && size.is_multiple_of(4));
    let text = workload::workload(&family, size);
    let mut ir = IR::new();
    ir.parse(&text).expect("generated workload parses");
    shape::verify(&ir.stages()[0]);
    let printed = ir.sprint();
    let json = serde_json::to_string(&ir).expect("serialize complete IR");
    let restored: IR = serde_json::from_str(&json).expect("deserialize complete IR");
    assert_eq!(printed, restored.sprint());
    assert_eq!(
        serde_json::to_value(&ir).unwrap(),
        serde_json::to_value(&restored).unwrap()
    );
    let stage = &restored.stages()[0];
    shape::verify(stage);
    assert_eq!(
        restored.lookup_function_by_name("benchmark"),
        ir.lookup_function_by_name("benchmark")
    );
    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for stmt in stage.statement_arena().iter() {
        let name = match stmt.definition() {
            BenchmarkLanguage::Function { .. } => continue,
            BenchmarkLanguage::Arith(Arith::Add { .. }) => "add",
            BenchmarkLanguage::Cf(ControlFlow::Branch { .. }) => "br",
            BenchmarkLanguage::Cf(ControlFlow::ConditionalBranch { .. }) => "cond_br",
            BenchmarkLanguage::Return(_) => "ret",
            other => panic!("unexpected benchmark statement: {other:?}"),
        };
        *counts.entry(name).or_default() += 1;
    }
    assert_eq!(counts["add"], size);
    let shape = shape::describe(stage);
    assert_eq!(shape, shape::describe(&ir.stages()[0]));
    let roundtrip = || {
        let json = serde_json::to_string(black_box(&ir)).unwrap();
        let restored: IR = serde_json::from_str(black_box(&json)).unwrap();
        (json, restored)
    };
    for _ in 0..warmups {
        drop(black_box(roundtrip()));
    }
    let mut samples = Vec::with_capacity(repeats);
    for _ in 0..repeats {
        let start = Instant::now();
        let output = black_box(roundtrip());
        samples.push(start.elapsed().as_secs_f64());
        drop(output); // returned IR and JSON destruction is outside the timer
    }
    println!(
        "{}",
        serde_json::json!({
            "implementation": "rust", "family": family, "adds": size,
            "operations": counts, "blocks": stage.block_arena().iter().count(), "shape": shape,
            "json_bytes": json.len(), "warmups": warmups, "samples_seconds": samples,
        })
    );
}

#[cfg(test)]
#[path = "json_roundtrip/tests.rs"]
mod tests;
