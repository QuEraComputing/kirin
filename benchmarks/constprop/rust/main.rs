//! Constant-propagation benchmark; see benchmarks/constprop/README.md.
use std::{hint::black_box, io::Read, time::Instant};

use kirin::prelude::*;
use kirin_constprop::{ConstProp, ConstPropValue};
use kirin_interpreter::{InterpreterError, expect_single};
use serde::Deserialize;
use serde_json::{Value, json};

#[allow(dead_code)]
#[path = "../../common/rust/mod.rs"]
mod common;
use common::{BenchmarkLanguage, workload};

type IR = Pipeline<StageInfo<BenchmarkLanguage>>;
type V = ConstPropValue<i64>;

/// An abstract argument tuple; `null` is an unknown (top) argument.
#[derive(Deserialize)]
struct Input {
    x: Option<i64>,
    y: Option<i64>,
    outer: Option<bool>,
    inner: Option<bool>,
}

#[derive(Deserialize)]
struct Request {
    family: String,
    size: usize,
    warmups: usize,
    repeats: usize,
    check: Vec<Input>,
    time: Vec<Input>,
}

impl Input {
    fn args(&self) -> [V; 4] {
        let int = |v: Option<i64>| v.map_or(V::Top, V::Const);
        let bool = |v: Option<bool>| v.map_or(V::Top, |b| V::Const(i64::from(b)));
        [int(self.x), int(self.y), bool(self.outer), bool(self.inner)]
    }
}

fn project(result: &Result<V, InterpreterError>) -> Value {
    match result {
        Ok(V::Const(c)) => json!(c),
        Ok(V::Top) => json!("top"),
        Ok(V::Bottom) => json!("bottom"),
        Ok(_) => json!("other"),
        Err(_) => json!("error"),
    }
}

fn main() {
    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request).unwrap();
    let request: Request = serde_json::from_str(&request).unwrap();
    let mut ir = IR::new();
    ir.parse(&workload::workload(&request.family, request.size))
        .expect("generated workload parses");
    // Returns the analysis with its result so fact-store destruction stays outside the timer.
    let analyze = |args: [V; 4]| {
        let mut analysis = ConstProp::<_, i64, InterpreterError>::new(&ir);
        let result = analysis
            .analyze_by_name("test", "benchmark", args)
            .and_then(expect_single::<V, InterpreterError>);
        (analysis, result)
    };
    let check: Vec<_> = request
        .check
        .iter()
        .map(|input| project(&analyze(input.args()).1))
        .collect();
    let time: Vec<_> = request
        .time
        .iter()
        .map(|input| {
            let result = project(&analyze(input.args()).1);
            for _ in 0..request.warmups {
                drop(black_box(analyze(input.args())));
            }
            let samples: Vec<_> = (0..request.repeats)
                .map(|_| {
                    let args = input.args();
                    let start = Instant::now();
                    let output = black_box(analyze(args));
                    let elapsed = start.elapsed().as_secs_f64();
                    drop(output);
                    elapsed
                })
                .collect();
            json!({"result": result, "samples_seconds": samples})
        })
        .collect();
    println!("{}", json!({"check": check, "time": time}));
}
