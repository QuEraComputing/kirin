//! Compile-time benchmark for `kirin-rewrite`'s analysis database; see
//! README.md.
use std::collections::HashSet;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use kirin::prelude::*;
use kirin_interpreter::{Body, Callee, Linker, SameStageLinker};
use kirin_rewrite::{AnalysisCache, Database, DriverError, RewriteDriver, RewriteSummary, RuleSet};
use serde_json::{Value, json};
use toy_lang::analysis_clients::constprop::{
    ConstPropFacts, StageConstants, fold_constants, fold_constants_with,
};
use toy_lang::analysis_clients::demand::{
    DemandFacts, StageDemand, free_undemanded, free_undemanded_with,
};
use toy_lang::analysis_clients::log;
use toy_lang::interpreter::run_i64;
use toy_lang::language::LowLevel;
use toy_lang::stage::Stage;

mod workload;
use workload::{Case, Shape};

#[derive(Parser)]
struct Args {
    #[arg(long, num_args = 1.., default_values = ["independent", "call_chain"])]
    shapes: Vec<String>,
    /// Numbers of functions.
    #[arg(long, num_args = 1.., default_values_t = [10, 100])]
    sizes: Vec<usize>,
    /// Three-statement units per function.
    #[arg(long, default_value_t = 8)]
    units: usize,
    #[arg(long, num_args = 1.., default_values = ["cold", "edit_first", "edit_last", "clean"])]
    cases: Vec<String>,
    /// Rounds of `fold` then `free`; the last must edit nothing.
    #[arg(long, default_value_t = 3)]
    rounds: usize,
    #[arg(long, default_value_t = 10)]
    repeats: usize,
    #[arg(long, default_value_t = 2)]
    warmups: usize,
    /// Print each pass of the median sample.
    #[arg(long)]
    per_pass: bool,
    /// Also write the full report here.
    #[arg(long)]
    json: Option<PathBuf>,
    /// Passed by `cargo bench`.
    #[arg(long, hide = true)]
    bench: bool,
}

/// Run `rules` to a fixpoint, recording the driver's time and whether it
/// edited in `timed`.
fn drive(
    rules: &RuleSet<LowLevel>,
    rewriter: &mut Rewriter<'_, LowLevel>,
    cache: &AnalysisCache,
    timed: &mut (Duration, bool),
) -> Result<RewriteSummary, DriverError> {
    let start = Instant::now();
    let summary = RewriteDriver::new(rules).with_cache(cache).run(rewriter);
    *timed = (start.elapsed(), rewriter.edited());
    summary
}

/// How analyses reach the passes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `Database::run_pass`: a result is reused until a pass edits the
    /// program.
    Database,
    /// `Program::run_pass` with no database: every pass computes its analysis
    /// first, with no revision and no cache.
    Direct,
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::Direct => "direct",
        }
    }
}

/// The program being compiled, in the form `Mode` runs passes on.
enum Compiler {
    Database(Database<Stage>),
    Direct(Program<Stage>),
}

impl Compiler {
    fn new(text: &str, mode: Mode) -> (Self, CompileStage) {
        let mut program: Program<Stage> = Program::new();
        ParseProgramText::parse(&mut program, text).expect("the workload parses");
        let stage = program.stage_by_name("lowered").expect("the stage exists");
        let compiler = match mode {
            Mode::Database => {
                let mut db = Database::new(program);
                db.register::<ConstPropFacts>().register::<DemandFacts>();
                Self::Database(db)
            }
            Mode::Direct => Self::Direct(program),
        };
        (compiler, stage)
    }

    fn program(&self) -> &Program<Stage> {
        match self {
            Self::Database(db) => db.program(),
            Self::Direct(program) => program,
        }
    }

    /// Run `kind` on `stage`: its summary, the driver's time, and whether it
    /// edited the program.
    fn run(&mut self, stage: CompileStage, kind: PassKind) -> (RewriteSummary, Duration, bool) {
        let mut timed = (Duration::ZERO, false);
        let summary = match self {
            Self::Database(db) => {
                let rules = kind.rules(stage);
                db.run_pass(stage, rules.required(), |rewriter, cache| {
                    drive(&rules, rewriter, cache, &mut timed)
                })
                .expect("the pass succeeds")
            }
            Self::Direct(program) => {
                let rules = match kind {
                    PassKind::Fold => fold_constants_with(Arc::new(
                        StageConstants::compute(program, stage).expect("constprop runs"),
                    )),
                    PassKind::Free => free_undemanded_with(Arc::new(
                        StageDemand::compute(program, stage).expect("demand runs"),
                    )),
                };
                let empty = AnalysisCache::default();
                program
                    .run_pass(stage, |rewriter| {
                        drive(&rules, rewriter, &empty, &mut timed)
                    })
                    .expect("the pass succeeds")
            }
        };
        let (driver, edited) = timed;
        (summary, driver, edited)
    }
}

#[derive(Clone, Copy)]
enum PassKind {
    Fold,
    Free,
}

impl PassKind {
    fn name(self) -> &'static str {
        match self {
            Self::Fold => "fold",
            Self::Free => "free",
        }
    }

    fn rules(self, stage: CompileStage) -> RuleSet<LowLevel> {
        match self {
            Self::Fold => fold_constants(stage),
            Self::Free => free_undemanded(stage),
        }
    }
}

/// What one pass did and how long its parts took.
#[derive(Clone)]
struct PassRecord {
    pass: &'static str,
    /// Analysis runs before the pass; 0 is a cache hit.
    analysis_runs: usize,
    /// Roots those runs analyzed.
    roots: usize,
    analysis: Duration,
    /// The part of `analysis` spent on roots that reach the edited function.
    affected_analysis: Duration,
    driver: Duration,
    /// Time the rules spent reading analysis results, inside `driver`.
    lookup: Duration,
    lookups: usize,
    /// The whole pass: its analyses, building its rule set, and `run_pass`.
    run_pass: Duration,
    summary: RewriteSummary,
    edited: bool,
}

impl PassRecord {
    /// `run_pass` minus the analyses and the driver: building the rule set,
    /// taking the stage out and back, and the post-pass `verify_derived`.
    fn overhead(&self) -> Duration {
        self.run_pass.saturating_sub(self.analysis + self.driver)
    }
}

/// `rounds` rounds of `fold` then `free`, and whether the last edited
/// nothing.
fn pipeline(
    compiler: &mut Compiler,
    stage: CompileStage,
    rounds: usize,
    affected: &HashSet<SpecializedFunction>,
) -> (Vec<PassRecord>, bool) {
    let mut records = Vec::new();
    let mut converged = false;
    for _ in 0..rounds {
        let mut edited = false;
        for kind in [PassKind::Fold, PassKind::Free] {
            let record = run_one(compiler, stage, kind, affected);
            edited |= record.edited;
            records.push(record);
        }
        converged = !edited;
    }
    (records, converged)
}

fn run_one(
    compiler: &mut Compiler,
    stage: CompileStage,
    kind: PassKind,
    affected: &HashSet<SpecializedFunction>,
) -> PassRecord {
    log::take();
    log::take_lookups();
    let start = Instant::now();
    let (summary, driver, edited) = compiler.run(stage, kind);
    let run_pass = start.elapsed();
    let runs = log::take();
    let lookups = log::take_lookups();
    PassRecord {
        pass: kind.name(),
        analysis_runs: runs.len(),
        roots: runs.iter().map(|run| run.roots.len()).sum(),
        analysis: runs.iter().map(|run| run.total).sum(),
        affected_analysis: runs
            .iter()
            .flat_map(|run| &run.roots)
            .filter(|(root, _)| affected.contains(root))
            .map(|(_, time)| *time)
            .sum(),
        driver,
        lookup: lookups.time,
        lookups: lookups.count,
        run_pass,
        summary,
        edited,
    }
}

fn specialization(
    program: &Program<Stage>,
    stage: CompileStage,
    name: &str,
) -> SpecializedFunction {
    let function = Callee::Function(program.lookup_function_by_name(name).expect("defined"));
    SameStageLinker
        .resolve(program, stage, &function)
        .expect("resolves")
        .specialization
}

/// Every statement of the program's functions, terminators included.
fn statements(program: &Program<Stage>, stage: CompileStage, size: usize) -> usize {
    let Stage::Lowered(info) = program.stage(stage).expect("the stage is live") else {
        panic!("the lowered stage holds the lowered language")
    };
    let names = (0..size).map(|i| format!("f{i}")).chain(["leaf".into()]);
    names
        .map(|name| {
            let function = Callee::Function(program.lookup_function_by_name(&name).unwrap());
            let target = SameStageLinker.resolve(program, stage, &function).unwrap();
            let Body::CFG(cfg) = target.body(program).unwrap() else {
                panic!("{name} has a CFG body")
            };
            cfg.blocks(info)
                .map(|block| {
                    block.statements(info).count() + usize::from(block.terminator(info).is_some())
                })
                .sum::<usize>()
        })
        .sum()
}

const SAMPLES: [[i64; 2]; 3] = [[0, 0], [3, -1], [-7, 5]];

fn evaluate(program: &Program<Stage>) -> Vec<i64> {
    SAMPLES
        .iter()
        .map(|args| run_i64(program, "lowered", "f0", args).expect("f0 runs"))
        .collect()
}

/// Totals over one sample's passes.
struct Totals {
    total: Duration,
    analysis: Duration,
    affected_analysis: Duration,
    driver: Duration,
    lookup: Duration,
    overhead: Duration,
}

fn totals(records: &[PassRecord]) -> Totals {
    let sum = |f: fn(&PassRecord) -> Duration| records.iter().map(f).sum();
    Totals {
        total: sum(|r| r.run_pass),
        analysis: sum(|r| r.analysis),
        affected_analysis: sum(|r| r.affected_analysis),
        driver: sum(|r| r.driver),
        lookup: sum(|r| r.lookup),
        overhead: sum(PassRecord::overhead),
    }
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}

fn ms(seconds: f64) -> String {
    format!("{:.3}", seconds * 1e3)
}

fn summary_json(records: &[PassRecord]) -> Value {
    let count = |f: fn(&PassRecord) -> usize| records.iter().map(f).sum::<usize>();
    json!({
        "passes": records.len(),
        "analysis_runs": count(|r| r.analysis_runs),
        "cache_hits": count(|r| usize::from(r.analysis_runs == 0)),
        "roots_analyzed": count(|r| r.roots),
        "lookups": count(|r| r.lookups),
        "applied": count(|r| r.summary.applied),
        "erased": count(|r| r.summary.erased),
        "sweeps": count(|r| r.summary.sweeps),
        "applied_by_pass": {
            "fold": records.iter().filter(|r| r.pass == "fold").map(|r| r.summary.applied).sum::<usize>(),
            "free": records.iter().filter(|r| r.pass == "free").map(|r| r.summary.applied).sum::<usize>(),
        },
    })
}

/// One (shape, size, case, mode): check the first run, then time samples.
fn measure(args: &Args, shape: Shape, size: usize, case: Case, mode: Mode) -> Value {
    let text = workload::text(shape, case, size, args.units);
    let affected_of = |program: &Program<Stage>, stage| -> HashSet<SpecializedFunction> {
        match case.edited(size) {
            Some(edited) => shape
                .affected(edited)
                .iter()
                .map(|name| specialization(program, stage, name))
                .collect(),
            None => HashSet::new(),
        }
    };

    // Check.
    let (mut compiler, stage) = Compiler::new(&text, mode);
    let before = statements(compiler.program(), stage, size);
    let values_before = evaluate(compiler.program());
    let affected = affected_of(compiler.program(), stage);
    let (records, converged) = pipeline(&mut compiler, stage, args.rounds, &affected);
    let after = statements(compiler.program(), stage, size);
    let values_after = evaluate(compiler.program());
    let dirty = (0..size).filter(|&i| case.is_dirty(size, i)).count();
    let summary = summary_json(&records);
    let mut failures = Vec::new();
    if !converged {
        failures.push(format!("round {} still edited", args.rounds));
    }
    if values_before != values_after {
        failures.push(format!("values {values_before:?} became {values_after:?}"));
    }
    if before - after != dirty * args.units {
        failures.push(format!(
            "expected {} fewer statements, got {before} -> {after}",
            dirty * args.units
        ));
    }
    if summary["applied_by_pass"]["fold"] != dirty * args.units {
        failures.push(format!(
            "fold applied {}",
            summary["applied_by_pass"]["fold"]
        ));
    }
    if summary["applied_by_pass"]["free"] != dirty {
        failures.push(format!(
            "free applied {}",
            summary["applied_by_pass"]["free"]
        ));
    }
    drop(compiler);

    // Time.
    let sample = || {
        let (mut compiler, stage) = Compiler::new(&text, mode);
        let affected = affected_of(compiler.program(), stage);
        let (records, _) = black_box(pipeline(&mut compiler, stage, args.rounds, &affected));
        drop(compiler);
        records
    };
    for _ in 0..args.warmups {
        sample();
    }
    let samples: Vec<Vec<PassRecord>> = (0..args.repeats).map(|_| sample()).collect();
    let all_totals: Vec<Totals> = samples.iter().map(|records| totals(records)).collect();
    let med = |f: fn(&Totals) -> Duration| {
        median(all_totals.iter().map(|t| f(t).as_secs_f64()).collect())
    };
    let median_total = med(|t| t.total);
    let median_sample = samples
        .iter()
        .min_by(|a, b| {
            let distance =
                |r: &Vec<PassRecord>| (totals(r).total.as_secs_f64() - median_total).abs();
            distance(a).total_cmp(&distance(b))
        })
        .expect("at least one repeat");
    let per_pass: Vec<Value> = median_sample
        .iter()
        .map(|r| {
            json!({
                "pass": r.pass,
                "analysis_runs": r.analysis_runs,
                "roots": r.roots,
                "analysis_seconds": r.analysis.as_secs_f64(),
                "affected_analysis_seconds": r.affected_analysis.as_secs_f64(),
                "driver_seconds": r.driver.as_secs_f64(),
                "lookup_seconds": r.lookup.as_secs_f64(),
                "lookups": r.lookups,
                "overhead_seconds": r.overhead().as_secs_f64(),
                "run_pass_seconds": r.run_pass.as_secs_f64(),
                "sweeps": r.summary.sweeps,
                "applied": r.summary.applied,
                "erased": r.summary.erased,
                "edited": r.edited,
            })
        })
        .collect();
    json!({
        "shape": shape.name(),
        "size": size,
        "units": args.units,
        "case": case.name(),
        "mode": mode.name(),
        "check": {
            "failures": failures,
            "statements_before": before,
            "statements_after": after,
            "values": values_after,
            "summary": summary,
        },
        "median_seconds": {
            "total": median_total,
            "analysis": med(|t| t.analysis),
            "affected_analysis": med(|t| t.affected_analysis),
            "driver": med(|t| t.driver),
            "lookup": med(|t| t.lookup),
            "overhead": med(|t| t.overhead),
        },
        "per_pass": per_pass,
    })
}

fn print_row(row: &Value, per_pass: bool) {
    let m = &row["median_seconds"];
    let s = &row["check"]["summary"];
    let f = |v: &Value| v.as_f64().unwrap();
    let affected = if row["case"] == "cold" {
        "-".to_string()
    } else {
        ms(f(&m["affected_analysis"]))
    };
    let failures = row["check"]["failures"].as_array().unwrap();
    println!(
        "{:<12} {:>5} {:<10} {:<9} | {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} | {:>6} {:>5} {:>4} {:>6} {:>8} {:>7} {:>6} {:>6} {}",
        row["shape"].as_str().unwrap(),
        row["size"],
        row["case"].as_str().unwrap(),
        row["mode"].as_str().unwrap(),
        ms(f(&m["total"])),
        ms(f(&m["analysis"])),
        affected,
        ms(f(&m["driver"])),
        ms(f(&m["lookup"])),
        ms(f(&m["overhead"])),
        s["passes"],
        s["analysis_runs"],
        s["cache_hits"],
        s["roots_analyzed"],
        s["lookups"],
        s["applied"],
        s["erased"],
        s["sweeps"],
        if failures.is_empty() {
            "ok".to_string()
        } else {
            format!("FAIL {failures:?}")
        },
    );
    if per_pass {
        for p in row["per_pass"].as_array().unwrap() {
            println!(
                "    {:<5} runs={} roots={:<5} analysis={}ms affected={}ms driver={}ms lookup={}ms ({} reads) overhead={}ms sweeps={} applied={} erased={} edited={}",
                p["pass"].as_str().unwrap(),
                p["analysis_runs"],
                p["roots"],
                ms(f(&p["analysis_seconds"])),
                ms(f(&p["affected_analysis_seconds"])),
                ms(f(&p["driver_seconds"])),
                ms(f(&p["lookup_seconds"])),
                p["lookups"],
                ms(f(&p["overhead_seconds"])),
                p["sweeps"],
                p["applied"],
                p["erased"],
                p["edited"],
            );
        }
    }
}

fn main() {
    let args = Args::parse();
    println!(
        "{:<12} {:>5} {:<10} {:<9} | {:>10} {:>10} {:>10} {:>10} {:>10} {:>10} | {:>6} {:>5} {:>4} {:>6} {:>8} {:>7} {:>6} {:>6} check",
        "shape",
        "size",
        "case",
        "mode",
        "total ms",
        "analysis",
        "affected",
        "driver",
        "lookup",
        "overhead",
        "passes",
        "runs",
        "hits",
        "roots",
        "lookups",
        "applied",
        "erased",
        "sweeps",
    );
    let mut report = Vec::new();
    let mut failed = false;
    for shape in args.shapes.iter().map(|s| Shape::parse(s)) {
        for &size in &args.sizes {
            for case in args.cases.iter().map(|c| Case::parse(c)) {
                for mode in [Mode::Database, Mode::Direct] {
                    let row = measure(&args, shape, size, case, mode);
                    failed |= !row["check"]["failures"].as_array().unwrap().is_empty();
                    print_row(&row, args.per_pass);
                    report.push(row);
                }
            }
        }
    }
    if let Some(path) = &args.json {
        std::fs::write(path, serde_json::to_string_pretty(&report).unwrap()).unwrap();
    }
    if failed {
        std::process::exit(1);
    }
}
