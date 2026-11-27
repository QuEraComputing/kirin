//! The analysis `Database`: caching per analysis and key, dependencies and
//! cycles, requirements, and dropping results when a pass edits the IR.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::panic::{AssertUnwindSafe, catch_unwind};

use kirin::prelude::*;
use kirin_arith::Arith;
use kirin_rewrite::{
    Analysis, AnalysisCache, AnalysisError, AnalysisKind, Database, DriverError, Requirement,
    RewriteDriver, RuleSet, RunPassError,
};
use kirin_test_languages::ArithFunctionLanguage as L;
use kirin_wildcard::rewrite_rule;

type Db = Database<StageInfo<L>>;

/// `-(-x)` is `x`, so a pass running it edits any stage with a double
/// negation.
#[rewrite_rule(language = L)]
fn double_neg() {
    let a = op![Arith::neg(x)];
    let r = op![Arith::neg(a)];
    rewrite!(r, {
        replace!(r, x);
    })
}

/// Two stages, each with a double negation for a pass to remove.
const TWO_STAGES: &str = "stage @a fn @f(i64) -> i64;\n\
     stage @b fn @f(i64) -> i64;\n\
     specialize @a fn @f(i64) -> i64 cfg {\n  ^entry(%x: i64) {\n    \
       %n = neg %x -> i64;\n    %m = neg %n -> i64;\n    ret %m;\n  }\n}\n\
     specialize @b fn @f(i64) -> i64 cfg {\n  ^entry(%y: i64) {\n    \
       %n = neg %y -> i64;\n    %m = neg %n -> i64;\n    ret %m;\n  }\n}\n";

fn database() -> (Db, CompileStage, CompileStage) {
    let mut program: Program<StageInfo<L>> = Program::new();
    program.parse(TWO_STAGES).expect("the program parses");
    let a = program.stage_by_name("a").expect("stage @a exists");
    let b = program.stage_by_name("b").expect("stage @b exists");
    (Database::new(program), a, b)
}

thread_local! {
    /// How many times each test analysis has run, by name.
    static NUM_RUNS: RefCell<HashMap<&'static str, usize>> = RefCell::default();
}

fn count_run(name: &'static str) {
    NUM_RUNS.with(|num_runs| *num_runs.borrow_mut().entry(name).or_default() += 1);
}

fn num_runs(name: &'static str) -> usize {
    NUM_RUNS.with(|num_runs| num_runs.borrow().get(name).copied().unwrap_or(0))
}

/// The number of live SSA values in a stage.
struct ValueCount;

impl AnalysisKind for ValueCount {
    type Key = CompileStage;
    type Output = usize;
}

impl Analysis<StageInfo<L>> for ValueCount {
    fn run(db: &Db, stage: &CompileStage) -> Result<usize, AnalysisError> {
        count_run("ValueCount");
        let info = db
            .program()
            .stage(*stage)
            .ok_or(AnalysisError::UnknownStage)?;
        Ok(info.ssa_arena().iter().count())
    }
}

/// An analysis that depends on another: its `run` asks the database for
/// `ValueCount` mid-run, which should come from the cache when already
/// computed. The result is twice `ValueCount`'s, so a test can check it.
struct Doubled;

impl AnalysisKind for Doubled {
    type Key = CompileStage;
    type Output = usize;
}

impl Analysis<StageInfo<L>> for Doubled {
    fn run(db: &Db, stage: &CompileStage) -> Result<usize, AnalysisError> {
        count_run("Doubled");
        Ok(2 * *db.get::<ValueCount>(stage)?)
    }
}

/// Returns its key, so a test can see which key a result came from.
struct Echo;

impl AnalysisKind for Echo {
    type Key = BTreeMap<(CompileStage, Function), u32>;
    type Output = Self::Key;
}

impl Analysis<StageInfo<L>> for Echo {
    fn run(_db: &Db, key: &Self::Key) -> Result<Self::Key, AnalysisError> {
        count_run("Echo");
        Ok(key.clone())
    }
}

/// Asks for itself, for the same key.
struct SelfDependent;

impl AnalysisKind for SelfDependent {
    type Key = CompileStage;
    type Output = ();
}

impl Analysis<StageInfo<L>> for SelfDependent {
    fn run(db: &Db, stage: &CompileStage) -> Result<(), AnalysisError> {
        db.get::<SelfDependent>(stage).map(drop)
    }
}

/// Asks for itself for a smaller key, down to 0. That is recursion, not a
/// cycle: a cycle asks for the same analysis *and* key that is still being
/// computed, while here each key is new and the chain ends at 0.
struct Countdown;

impl AnalysisKind for Countdown {
    type Key = u32;
    type Output = u32;
}

impl Analysis<StageInfo<L>> for Countdown {
    fn run(db: &Db, n: &u32) -> Result<u32, AnalysisError> {
        match n {
            0 => Ok(0),
            n => Ok(1 + *db.get::<Countdown>(&(n - 1))?),
        }
    }
}

/// Reads `Left` and `Right`, which both read `Shared`.
struct Top;
struct Left;
struct Right;
struct Shared;

macro_rules! unit_analysis {
    ($name:ident, |$db:ident| $body:expr) => {
        impl AnalysisKind for $name {
            type Key = ();
            type Output = ();
        }

        impl Analysis<StageInfo<L>> for $name {
            fn run($db: &Db, _key: &()) -> Result<(), AnalysisError> {
                count_run(stringify!($name));
                $body
            }
        }
    };
}

unit_analysis!(Top, |db| {
    db.get::<Left>(&())?;
    db.get::<Right>(&())?;
    Ok(())
});
unit_analysis!(Left, |db| db.get::<Shared>(&()).map(drop));
unit_analysis!(Right, |db| db.get::<Shared>(&()).map(drop));
unit_analysis!(Shared, |_db| Ok(()));

/// Always fails.
struct Failing;

impl AnalysisKind for Failing {
    type Key = ();
    type Output = ();
}

impl Analysis<StageInfo<L>> for Failing {
    fn run(_db: &Db, _key: &()) -> Result<(), AnalysisError> {
        count_run("Failing");
        Err(AnalysisError::failed::<Failing>("always fails"))
    }
}

/// Panics on its first run and succeeds afterwards.
struct PanicsOnce;

impl AnalysisKind for PanicsOnce {
    type Key = ();
    type Output = ();
}

impl Analysis<StageInfo<L>> for PanicsOnce {
    fn run(_db: &Db, _key: &()) -> Result<(), AnalysisError> {
        count_run("PanicsOnce");
        if num_runs("PanicsOnce") == 1 {
            panic!("first run");
        }
        Ok(())
    }
}

/// A pass that removes the stage's double negation.
fn fold(rewriter: &mut Rewriter<L>, _cache: &AnalysisCache) -> Result<(), DriverError> {
    let rules = RuleSet::build(|stage| vec![double_neg(stage)]).expect("the rule checks");
    RewriteDriver::new(&rules).run(rewriter).map(drop)
}

/// A pass that reads and edits nothing.
fn read_only(_rewriter: &mut Rewriter<L>, _cache: &AnalysisCache) -> Result<(), DriverError> {
    Ok(())
}

#[test]
fn get_computes_once_then_reuses() {
    let (db, a, _) = database();
    let first = db.get::<ValueCount>(&a).unwrap();
    let second = db.get::<ValueCount>(&a).unwrap();
    assert_eq!(first, second);
    assert_eq!(num_runs("ValueCount"), 1);
}

#[test]
fn get_cached_never_computes() {
    let (db, a, _) = database();
    assert!(db.get_cached::<ValueCount>(&a).is_none());
    assert_eq!(num_runs("ValueCount"), 0);
    let computed = db.get::<ValueCount>(&a).unwrap();
    assert_eq!(db.get_cached::<ValueCount>(&a), Some(computed));
}

#[test]
fn distinct_keys_get_distinct_results() {
    let (db, a, b) = database();
    db.get::<ValueCount>(&a).unwrap();
    db.get::<ValueCount>(&b).unwrap();
    assert_eq!(num_runs("ValueCount"), 2);
}

#[test]
fn the_same_roots_in_two_orders_are_one_key() {
    let (db, a, b) = database();
    let f = db
        .program()
        .lookup_function_by_name("f")
        .expect("@f exists");
    let mut forward = BTreeMap::new();
    forward.insert((a, f), 1);
    forward.insert((b, f), 2);
    let mut backward = BTreeMap::new();
    backward.insert((b, f), 2);
    backward.insert((a, f), 1);
    let first = db.get::<Echo>(&forward).unwrap();
    let second = db.get::<Echo>(&backward).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert_eq!(num_runs("Echo"), 1);
}

#[test]
fn an_analysis_gets_its_dependencies_through_the_database() {
    let (db, a, _) = database();
    let count = *db.get::<ValueCount>(&a).unwrap();
    assert_eq!(*db.get::<Doubled>(&a).unwrap(), 2 * count);
    // `Doubled` read the cached `ValueCount` rather than running it again.
    assert_eq!(num_runs("ValueCount"), 1);
}

#[test]
fn a_dependency_shared_by_two_others_runs_once() {
    let (db, _, _) = database();
    db.get::<Top>(&()).unwrap();
    assert_eq!(num_runs("Left"), 1);
    assert_eq!(num_runs("Right"), 1);
    assert_eq!(num_runs("Shared"), 1);
}

#[test]
fn asking_for_the_same_key_again_is_a_cycle() {
    let (db, a, _) = database();
    assert!(matches!(
        db.get::<SelfDependent>(&a),
        Err(AnalysisError::Cycle { .. })
    ));
}

#[test]
fn asking_for_the_same_analysis_with_another_key_is_not_a_cycle() {
    let (db, _, _) = database();
    assert_eq!(*db.get::<Countdown>(&3).unwrap(), 3);
}

#[test]
fn a_failure_is_not_cached() {
    let (db, _, _) = database();
    assert!(matches!(
        db.get::<Failing>(&()),
        Err(AnalysisError::Failed { .. })
    ));
    assert!(matches!(
        db.get::<Failing>(&()),
        Err(AnalysisError::Failed { .. })
    ));
    assert_eq!(num_runs("Failing"), 2);
}

#[test]
fn a_panicking_run_leaves_no_cycle_behind() {
    let (db, _, _) = database();
    let panicked = catch_unwind(AssertUnwindSafe(|| db.get::<PanicsOnce>(&())));
    assert!(panicked.is_err());
    assert!(db.get::<PanicsOnce>(&()).is_ok());
    assert_eq!(num_runs("PanicsOnce"), 2);
}

#[test]
fn get_computes_an_unregistered_analysis() {
    let (db, a, _) = database();
    assert!(db.get::<ValueCount>(&a).is_ok());
}

#[test]
fn run_pass_computes_every_requirement_first() {
    let (mut db, a, b) = database();
    db.register::<ValueCount>();
    let requires = [
        Requirement::of::<ValueCount>(a),
        Requirement::of::<ValueCount>(b),
    ];
    db.run_pass(a, requires, |_rewriter: &mut Rewriter<L>, cache| {
        assert!(cache.get::<ValueCount>(&a).is_some());
        assert!(cache.get::<ValueCount>(&b).is_some());
        Ok::<_, DriverError>(())
    })
    .unwrap();
    assert_eq!(num_runs("ValueCount"), 2);
}

#[test]
fn an_unregistered_requirement_fails_before_the_pass() {
    let (mut db, a, _) = database();
    let mut ran = false;
    let result = db.run_pass(
        a,
        [Requirement::of::<ValueCount>(a)],
        |_rewriter: &mut Rewriter<L>, _| {
            ran = true;
            Ok::<_, DriverError>(())
        },
    );
    assert!(matches!(
        result,
        Err(RunPassError::RequirementUnsatisfied(
            AnalysisError::Unregistered { .. }
        ))
    ));
    assert!(!ran);
    // The stage is untouched and still usable.
    db.run_pass(a, [], read_only).unwrap();
}

#[test]
fn a_pass_with_no_edits_keeps_every_result() {
    let (mut db, a, b) = database();
    db.get::<ValueCount>(&a).unwrap();
    db.get::<ValueCount>(&b).unwrap();
    db.run_pass(a, [], read_only).unwrap();
    assert_eq!(db.revision(), 0);
    assert!(db.get_cached::<ValueCount>(&a).is_some());
    assert!(db.get_cached::<ValueCount>(&b).is_some());
}

#[test]
fn a_pass_with_an_edit_drops_every_result_whatever_its_key() {
    let (mut db, a, b) = database();
    db.get::<ValueCount>(&a).unwrap();
    db.get::<ValueCount>(&b).unwrap();
    db.run_pass(a, [], fold).unwrap();
    assert_eq!(db.revision(), 1);
    // `b`'s result is dropped too, though the pass edited only `a`.
    assert!(db.get_cached::<ValueCount>(&a).is_none());
    assert!(db.get_cached::<ValueCount>(&b).is_none());
}

#[test]
fn a_failed_pass_drops_every_result() {
    let (mut db, a, b) = database();
    db.get::<ValueCount>(&b).unwrap();
    let result = db.run_pass(a, [], |_rewriter: &mut Rewriter<L>, _| {
        Err::<(), _>(std::io::Error::other("the pass fails"))
    });
    assert!(matches!(
        result,
        Err(RunPassError::PassFailedOrNotRun(StagePassError::PassFailed))
    ));
    assert_eq!(db.revision(), 1);
    assert!(db.get_cached::<ValueCount>(&b).is_none());
}

#[test]
fn program_mut_drops_every_result() {
    let (mut db, a, _) = database();
    db.get::<ValueCount>(&a).unwrap();
    db.program_mut();
    assert_eq!(db.revision(), 1);
    assert!(db.get_cached::<ValueCount>(&a).is_none());
}

#[test]
fn a_pass_reads_the_results_computed_before_it() {
    let (mut db, a, _) = database();
    db.register::<ValueCount>();
    let before = *db.get::<ValueCount>(&a).unwrap();
    db.run_pass(a, [Requirement::of::<ValueCount>(a)], |rewriter, cache| {
        fold(rewriter, cache)?;
        // Still the result from before the edit: a snapshot.
        assert_eq!(*cache.get::<ValueCount>(&a).unwrap(), before);
        Ok::<_, DriverError>(())
    })
    .unwrap();
    assert_ne!(*db.get::<ValueCount>(&a).unwrap(), before);
}
