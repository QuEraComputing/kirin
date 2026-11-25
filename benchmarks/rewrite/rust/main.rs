//! Rewrite benchmark; see benchmarks/rewrite/README.md.
use std::{hint::black_box, io::Read, time::Instant};

use kirin::prelude::*;
use kirin_interpreter::{ConcreteInterpreter, InterpreterError, expect_single};
use kirin_ir::walk::PreOrder;
use kirin_rewrite::{RewriteDriver, RuleSet};
use serde::Deserialize;
use serde_json::{Value, json};

mod backward;
#[allow(dead_code)]
#[path = "../../common/rust/mod.rs"]
mod common;
mod rules;
mod unroll;
mod workload;
use common::BenchmarkLanguage as L;

type IR = Program<StageInfo<L>>;

#[derive(Deserialize)]
struct Sample {
    x: i64,
    y: i64,
    outer: bool,
    inner: bool,
}

#[derive(Deserialize)]
struct Request {
    task: String,
    shape: String,
    size: usize,
    warmups: usize,
    repeats: usize,
    samples: Vec<Sample>,
}

/// What one variant runs inside `run_pass`.
enum Edit {
    Driver(Box<RuleSet<L>>),
    Primitive(String),
}

/// The Rust variants of a task: driver rule forms, or the raw edit.
fn variants(task: &str, shape: &str) -> Vec<(&'static str, Edit)> {
    let driver = |name: &'static str| (name, Edit::Driver(Box::new(rules::rule_set(name))));
    match task {
        "peephole" | "no_match" => vec![driver("pattern"), driver("native")],
        "dce" => vec![driver("cleanup")],
        "backward_cascade" => {
            let match_cost = shape.rsplit('_').next().unwrap().parse().unwrap();
            vec![(
                "native",
                Edit::Driver(Box::new(backward::rules(match_cost))),
            )]
        }
        "cascade" => vec![driver("unroll")],
        "bypass" | "insert" | "move" | "erase" => {
            vec![("rewriter", Edit::Primitive(task.to_string()))]
        }
        _ => panic!("unknown task {task}"),
    }
}

fn parse(text: &str) -> (IR, CompileStage) {
    let mut ir = IR::new();
    ir.parse(text).expect("generated workload parses");
    let stage = ir.stage_by_name("test").expect("the stage exists");
    (ir, stage)
}

/// Apply `edit` in one pass: its summary, the pass closure's time and the
/// whole `run_pass` time, which adds the post-pass `verify_derived`.
fn apply(ir: &mut IR, stage: CompileStage, edit: &Edit) -> (Value, f64, f64) {
    let mut inner = 0.0;
    let start = Instant::now();
    let summary = match edit {
        Edit::Driver(rule_set) => ir.run_pass(stage, |rewriter| {
            let start = Instant::now();
            let summary = RewriteDriver::new(rule_set).run::<PreOrder>(rewriter);
            inner = start.elapsed().as_secs_f64();
            summary.map(rules::summary)
        }),
        Edit::Primitive(op) => ir.run_pass(stage, |rewriter| {
            let start = Instant::now();
            let edits = rules::primitive(op, rewriter);
            inner = start.elapsed().as_secs_f64();
            edits.map(|edits| json!({"edits": edits}))
        }),
    };
    let total = start.elapsed().as_secs_f64();
    let summary = summary.unwrap_or_else(|error| {
        panic!(
            "rewrite pass failed: {error:?}, cause: {:?}",
            ir.quarantined(stage).map(|q| q.cause())
        )
    });
    (summary, inner, total)
}

/// Every statement of `@benchmark`'s body, terminators included.
fn statements(ir: &IR, stage: CompileStage) -> usize {
    let info = ir.stage(stage).expect("the stage is live");
    rules::body(info)
        .blocks(info)
        .map(|block| block.statements(info).count() + usize::from(block.terminator(info).is_some()))
        .sum()
}

fn evaluate(ir: &IR, samples: &[Sample]) -> Vec<i64> {
    samples
        .iter()
        .map(|s| {
            let mut interpreter: ConcreteInterpreter<'_, _, i64, InterpreterError> =
                ConcreteInterpreter::new(ir);
            let args = [s.x, s.y, i64::from(s.outer), i64::from(s.inner)];
            let result = interpreter.call_by_name("test", "benchmark", args).unwrap();
            expect_single::<i64, InterpreterError>(result).unwrap()
        })
        .collect()
}

fn main() {
    let mut request = String::new();
    std::io::stdin().read_to_string(&mut request).unwrap();
    let request: Request = serde_json::from_str(&request).unwrap();
    let text = workload::text(&request.shape, request.size);
    let output: serde_json::Map<String, Value> = variants(&request.task, &request.shape)
        .into_iter()
        .map(|(name, edit)| {
            let (mut ir, stage) = parse(&text);
            let before = statements(&ir, stage);
            let (summary, _, _) = apply(&mut ir, stage, &edit);
            let check = json!({
                "statements_before": before,
                "statements_after": statements(&ir, stage),
                "values": evaluate(&ir, &request.samples),
                "summary": summary,
            });
            // Parsing and dropping the IR stay outside the timer.
            let sample = || {
                let (mut ir, stage) = parse(&text);
                let (_, inner, total) = black_box(apply(&mut ir, stage, &edit));
                drop(ir);
                (inner, total)
            };
            for _ in 0..request.warmups {
                sample();
            }
            let (pass, run_pass): (Vec<f64>, Vec<f64>) =
                (0..request.repeats).map(|_| sample()).unzip();
            let timing = json!({"pass_seconds": pass, "run_pass_seconds": run_pass});
            (name.to_string(), json!({"check": check, "timing": timing}))
        })
        .collect();
    println!("{}", Value::Object(output));
}
