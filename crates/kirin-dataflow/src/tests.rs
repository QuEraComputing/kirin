//! Liveness tests over a combined cf + scf + arith + cmp + constant + call
//! language ([`DataflowLanguage`]). Programs are built by parsing text; the
//! `LivenessOp` forwarding impl lives here (it is the compiler-author seam).

use kirin_chumsky::ParsePipelineText;
use kirin_ir::{Block, GetInfo, IsPure, Pipeline, SSAValue, StageInfo, Statement};
use kirin_test_languages::DataflowLanguage;

use crate::{Flow, LiveSet, Liveness, LivenessOp, analyze_function, analyze_liveness_by_name};

type Stage = StageInfo<DataflowLanguage>;

// The one forwarding impl a compiler author writes for their composed language:
// delegate control ops to the leaf impls this crate provides; everything else
// is a plain use/def op.
impl LivenessOp for DataflowLanguage {
    fn flow(&self) -> Flow {
        match self {
            DataflowLanguage::ControlFlow(op) => op.flow(),
            DataflowLanguage::Structured(op) => op.flow(),
            DataflowLanguage::Return(op) => op.flow(),
            DataflowLanguage::Function { .. }
            | DataflowLanguage::Constant(_)
            | DataflowLanguage::Arith(_)
            | DataflowLanguage::Cmp(_)
            | DataflowLanguage::Call(_) => Flow::Plain,
        }
    }
}

// ---- harness ---------------------------------------------------------------

fn parse(src: &str) -> Pipeline<Stage> {
    let mut pipeline = Pipeline::new();
    pipeline.parse(src).expect("pipeline should parse");
    pipeline
}

fn body_of(pipeline: &Pipeline<Stage>, stage_name: &str, function: &str) -> Statement {
    let stage = pipeline.stage_by_name(stage_name).expect("stage");
    let info = pipeline.stage(stage).expect("stage info");
    let staged = pipeline
        .resolve_staged_function(function, stage)
        .expect("staged function");
    let specialized = staged
        .get_info(info)
        .expect("staged info")
        .unique_live_specialization()
        .expect("unique specialization");
    *specialized.get_info(info).expect("specialized info").body()
}

fn info<'a>(pipeline: &'a Pipeline<Stage>, stage_name: &str) -> &'a Stage {
    let stage = pipeline.stage_by_name(stage_name).expect("stage");
    pipeline.stage(stage).expect("stage info")
}

/// Top-level CFG blocks of a function body, in textual order.
fn blocks(info: &Stage, body: Statement) -> Vec<Block> {
    let region = *body.regions(info).next().expect("body region");
    region.blocks(info).collect()
}

/// Non-terminator statements of a block, in order.
fn stmts(info: &Stage, block: Block) -> Vec<Statement> {
    block.statements(info).collect()
}

fn terminator(info: &Stage, block: Block) -> Statement {
    block.terminator(info).expect("terminator")
}

/// Nested body blocks of a structured op (then/else for `if`, body for `for`).
fn nested_blocks(info: &Stage, stmt: Statement) -> Vec<Block> {
    stmt.blocks(info).copied().collect()
}

fn results(info: &Stage, stmt: Statement) -> Vec<SSAValue> {
    stmt.results(info).map(|r| SSAValue::from(*r)).collect()
}

/// The single result of a statement.
fn result(info: &Stage, stmt: Statement) -> SSAValue {
    let results = results(info, stmt);
    assert_eq!(results.len(), 1, "expected exactly one result");
    results[0]
}

fn params(info: &Stage, block: Block) -> Vec<SSAValue> {
    block
        .get_info(info)
        .expect("block info")
        .arguments
        .iter()
        .copied()
        .map(SSAValue::from)
        .collect()
}

fn live(values: &[SSAValue]) -> LiveSet {
    values.iter().copied().collect()
}

fn run(src: &str, function: &str) -> (Pipeline<Stage>, Liveness) {
    let pipeline = parse(src);
    let liveness = analyze_liveness_by_name(&pipeline, "test", function).expect("liveness");
    (pipeline, liveness)
}

// ---- A. straight-line -------------------------------------------------------

#[test]
fn straight_line_liveness() {
    let src = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %c1 = constant 1 -> i64;
    %c2 = constant 2 -> i64;
    %sum = add %c1, %c2 -> i64;
    ret %sum;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let body_stmts = stmts(info, entry);
    let (c1, c2, add) = (body_stmts[0], body_stmts[1], body_stmts[2]);
    let ret = terminator(info, entry);
    let (vc1, vc2, vsum) = (result(info, c1), result(info, c2), result(info, add));

    assert_eq!(live_.live_before(ret), Some(&live(&[vsum])));
    assert_eq!(live_.live_after(add), Some(&live(&[vsum])));
    assert_eq!(live_.live_before(add), Some(&live(&[vc1, vc2])));
    assert_eq!(live_.live_before(c2), Some(&live(&[vc1])));
    assert_eq!(live_.live_before(c1), Some(&live(&[])));
}

// Exercise the lower-level `analyze_function` entry point directly.
#[test]
fn analyze_function_entry_point() {
    let src = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %c1 = constant 1 -> i64;
    ret %c1;
  }
}
"#;
    let pipeline = parse(src);
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let live_ = analyze_function(info, body).expect("liveness");
    let entry = blocks(info, body)[0];
    let ret = terminator(info, entry);
    let c1 = result(info, stmts(info, entry)[0]);
    assert_eq!(live_.live_before(ret), Some(&live(&[c1])));
}

// ---- B. pure dead result does not make operands live -----------------------

#[test]
fn pure_dead_result_does_not_revive_operands() {
    let src = r#"
stage @test fn @main() -> i64;

specialize @test fn @main() -> i64 {
  ^entry() {
    %c1 = constant 1 -> i64;
    %c2 = constant 2 -> i64;
    %dead = add %c1, %c2 -> i64;
    ret %c1;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let body_stmts = stmts(info, entry);
    let (c2, dead) = (body_stmts[1], body_stmts[2]);
    let vc2 = result(info, c2);
    let vdead = result(info, dead);

    // The dead add is skipped: live before == live after, so %c2 is not revived
    // and %dead is never live.
    let before_dead = live_.live_before(dead).expect("before dead");
    assert!(!before_dead.contains(&vc2), "%c2 must not be revived");
    assert!(!before_dead.contains(&vdead));
    assert!(!live_.live_after(dead).expect("after dead").contains(&vdead));
}

// ---- C. branch condition liveness ------------------------------------------

#[test]
fn branch_condition_liveness() {
    let src = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %zero: i64) {
    %cond = lt %x, %zero -> i64;
    cond_br %cond then=^then(%x) else=^else(%x);
  }
  ^then(%a: i64) {
    ret %a;
  }
  ^else(%b: i64) {
    ret %b;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let entry_params = params(info, entry);
    let (vx, vzero) = (entry_params[0], entry_params[1]);
    let cmp = stmts(info, entry)[0];
    let vcond = result(info, cmp);
    let cond_br = terminator(info, entry);

    assert!(
        live_
            .live_before(cond_br)
            .expect("before cond_br")
            .contains(&vcond)
    );
    assert_eq!(live_.live_before(cmp), Some(&live(&[vx, vzero])));
}

// ---- D. block-argument edge transfer (mandatory) ---------------------------

#[test]
fn block_argument_edge_transfer() {
    let src = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %y: i64) {
    br ^target(%x);
  }
  ^target(%a: i64) {
    ret %a;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let cfg = blocks(info, body);
    let (entry, target) = (cfg[0], cfg[1]);
    let entry_params = params(info, entry);
    let (vx, vy) = (entry_params[0], entry_params[1]);
    let va = params(info, target)[0];
    let br = terminator(info, entry);

    // %x is live before the branch (it is the edge arg for the live target
    // param %a); %y is not.
    assert_eq!(live_.live_before(br), Some(&live(&[vx])));
    assert!(!live_.live_before(br).unwrap().contains(&vy));
    // %a is live on entry to the target block.
    assert_eq!(live_.block_live_in(target), Some(&live(&[va])));
}

// ---- E. conditional block-argument edge transfer ---------------------------

#[test]
fn conditional_block_argument_edge_transfer() {
    let src = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%cond: i64, %x: i64, %y: i64) {
    cond_br %cond then=^left(%x) else=^right(%y);
  }
  ^left(%a: i64) {
    ret %a;
  }
  ^right(%b: i64) {
    ret %b;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let cfg = blocks(info, body);
    let (entry, left, right) = (cfg[0], cfg[1], cfg[2]);
    let entry_params = params(info, entry);
    let (vcond, vx, vy) = (entry_params[0], entry_params[1], entry_params[2]);
    let cond_br = terminator(info, entry);

    assert_eq!(live_.live_before(cond_br), Some(&live(&[vcond, vx, vy])));
    // Left param maps to %x, right param maps to %y (block_out reflects both).
    assert_eq!(
        live_.block_live_in(left),
        Some(&live(&[params(info, left)[0]]))
    );
    assert_eq!(
        live_.block_live_in(right),
        Some(&live(&[params(info, right)[0]]))
    );
    assert_eq!(live_.block_live_out(entry), Some(&live(&[vx, vy])));
}

// ---- F. join point liveness ------------------------------------------------

#[test]
fn join_point_liveness() {
    let src = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%cond: i64, %x: i64, %y: i64) {
    cond_br %cond then=^left() else=^right();
  }
  ^left() {
    br ^join(%x);
  }
  ^right() {
    br ^join(%y);
  }
  ^join(%z: i64) {
    ret %z;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let cfg = blocks(info, body);
    let (entry, left, right) = (cfg[0], cfg[1], cfg[2]);
    let entry_params = params(info, entry);
    let (vcond, vx, vy) = (entry_params[0], entry_params[1], entry_params[2]);
    let cond_br = terminator(info, entry);

    assert_eq!(live_.block_live_in(left), Some(&live(&[vx])));
    assert_eq!(live_.block_live_in(right), Some(&live(&[vy])));
    assert!(live_.live_before(cond_br).unwrap().contains(&vcond));
    assert_eq!(live_.live_before(cond_br), Some(&live(&[vcond, vx, vy])));
}

// ---- G. CFG loop liveness --------------------------------------------------

#[test]
fn cfg_loop_liveness() {
    let src = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%n: i64) {
    br ^loop(%n);
  }
  ^loop(%i: i64) {
    %zero = constant 0 -> i64;
    %done = eq %i, %zero -> i64;
    cond_br %done then=^exit(%i) else=^body(%i);
  }
  ^body(%j: i64) {
    %one = constant 1 -> i64;
    %next = sub %j, %one -> i64;
    br ^loop(%next);
  }
  ^exit(%r: i64) {
    ret %r;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let cfg = blocks(info, body);
    let (entry, loop_blk, body_blk) = (cfg[0], cfg[1], cfg[2]);
    let vi = params(info, loop_blk)[0];
    let vn = params(info, entry)[0];
    let backedge = terminator(info, body_blk);
    let vnext = result(info, stmts(info, body_blk)[1]);

    // Fixpoint terminates and: %i is live across the loop header; %next is live
    // before the backedge; %n is live before entering the loop.
    assert!(live_.block_live_in(loop_blk).unwrap().contains(&vi));
    assert!(live_.live_before(backedge).unwrap().contains(&vnext));
    assert_eq!(live_.block_live_in(entry), Some(&live(&[vn])));
}

// ---- H. call liveness ------------------------------------------------------

#[test]
fn call_liveness() {
    let src = r#"
stage @test fn @main(i64, i64) -> i64;

specialize @test fn @main(i64, i64) -> i64 {
  ^entry(%x: i64, %y: i64) {
    %r = call.named @foo(%x, %y) -> i64;
    ret %r;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let call = stmts(info, entry)[0];
    let ret = terminator(info, entry);
    let vr = result(info, call);
    let entry_params = params(info, entry);
    let (vx, vy) = (entry_params[0], entry_params[1]);

    assert_eq!(live_.live_after(call), Some(&live(&[vr])));
    // call args are uses; the result def kills prior liveness of %r.
    assert_eq!(live_.live_before(call), Some(&live(&[vx, vy])));
    assert!(!live_.live_before(call).unwrap().contains(&vr));
    assert_eq!(live_.live_before(ret), Some(&live(&[vr])));
}

// ---- I. multi-result -------------------------------------------------------

#[test]
fn multi_result_liveness() {
    let src = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    %a, %b = call.named @foo(%x) -> i64, i64;
    ret %b;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let call = stmts(info, entry)[0];
    let call_results = results(info, call);
    let (va, vb) = (call_results[0], call_results[1]);

    // Only the used result %b is live after the call; %a never is.
    assert_eq!(live_.live_after(call), Some(&live(&[vb])));
    assert!(!live_.live_after(call).unwrap().contains(&va));
    assert_eq!(
        live_.live_before(call),
        Some(&live(&[params(info, entry)[0]]))
    );
}

// ---- J. unreachable block --------------------------------------------------

#[test]
fn unreachable_block_does_not_affect_entry() {
    let src = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    ret %x;
  }
  ^dead(%y: i64) {
    ret %y;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let cfg = blocks(info, body);
    let (entry, dead) = (cfg[0], cfg[1]);
    let vx = params(info, entry)[0];
    let vy = params(info, dead)[0];

    assert_eq!(live_.block_live_in(entry), Some(&live(&[vx])));
    assert!(!live_.block_live_in(entry).unwrap().contains(&vy));
}

// ---- K. SCF: scf.if --------------------------------------------------------

#[test]
fn scf_if_liveness() {
    let src = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%cond: i64, %x: i64, %y: i64) {
    %r = if %cond then ^then() {
      yield %x;
    } else ^else() {
      yield %y;
    } -> i64;
    ret %r;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let entry_params = params(info, entry);
    let (vcond, vx, vy) = (entry_params[0], entry_params[1], entry_params[2]);
    let if_stmt = stmts(info, entry)[0];
    let arms = nested_blocks(info, if_stmt);
    let (then_blk, else_blk) = (arms[0], arms[1]);

    // Condition live before the if; since %r is live after, each arm's yielded
    // value is live in that arm.
    assert_eq!(live_.live_before(if_stmt), Some(&live(&[vcond, vx, vy])));
    assert_eq!(live_.block_live_in(then_blk), Some(&live(&[vx])));
    assert_eq!(live_.block_live_in(else_blk), Some(&live(&[vy])));
}

// scf.if where the result is dead: the condition stays live, but the yielded
// values do not (their results are not live after).
#[test]
fn scf_if_dead_result() {
    let src = r#"
stage @test fn @main(i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64) -> i64 {
  ^entry(%cond: i64, %x: i64, %y: i64) {
    %r = if %cond then ^then() {
      yield %x;
    } else ^else() {
      yield %y;
    } -> i64;
    ret %cond;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let if_stmt = stmts(info, entry)[0];
    let arms = nested_blocks(info, if_stmt);

    assert!(live_.block_live_in(arms[0]).unwrap().is_empty());
    assert!(live_.block_live_in(arms[1]).unwrap().is_empty());
}

// ---- K. SCF: scf.for -------------------------------------------------------

#[test]
fn scf_for_liveness() {
    let src = r#"
stage @test fn @main(i64, i64, i64, i64) -> i64;

specialize @test fn @main(i64, i64, i64, i64) -> i64 {
  ^entry(%lo: i64, %hi: i64, %s: i64, %init: i64) {
    %sum = for %lo in %lo..%hi step %s iter_args(%init) do ^body(%iv: i64, %acc: i64) {
      %next = add %acc, %iv -> i64;
      yield %next;
    } -> i64;
    ret %sum;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let entry_params = params(info, entry);
    let (vlo, vhi, vs, vinit) = (
        entry_params[0],
        entry_params[1],
        entry_params[2],
        entry_params[3],
    );
    let for_stmt = stmts(info, entry)[0];
    let loop_body = nested_blocks(info, for_stmt)[0];
    let body_params = params(info, loop_body);
    let (viv, vacc) = (body_params[0], body_params[1]);
    let yield_stmt = terminator(info, loop_body);
    let vnext = result(info, stmts(info, loop_body)[0]);

    // Bounds, step and the live init are live before the loop.
    assert_eq!(
        live_.live_before(for_stmt),
        Some(&live(&[vlo, vhi, vs, vinit]))
    );
    // Loop-carried precision: induction var and accumulator are live in the
    // body; the yielded next value is live before the yield.
    assert!(live_.block_live_in(loop_body).unwrap().contains(&viv));
    assert!(live_.block_live_in(loop_body).unwrap().contains(&vacc));
    assert!(live_.live_before(yield_stmt).unwrap().contains(&vnext));
}

// ---- DCE-style use of liveness + purity ------------------------------------

#[test]
fn dce_style_liveness_plus_purity() {
    let src = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    %c1 = constant 1 -> i64;
    %dead = add %c1, %c1 -> i64;
    %sink = call.named @foo(%x) -> i64;
    ret %c1;
  }
}
"#;
    let (pipeline, live_) = run(src, "main");
    let info = info(&pipeline, "test");
    let body = body_of(&pipeline, "test", "main");
    let entry = blocks(info, body)[0];
    let body_stmts = stmts(info, entry);
    let (dead, sink) = (body_stmts[1], body_stmts[2]);
    let vdead = result(info, dead);
    let vsink = result(info, sink);

    // The pure add's result is dead → with its purity, a DCE pass may remove it.
    assert!(!live_.live_after(dead).unwrap().contains(&vdead));
    assert!(dead.definition(info).is_pure());

    // The call's result is also dead, but the call is impure → not removable on
    // liveness grounds alone.
    assert!(!live_.live_after(sink).unwrap().contains(&vsink));
    assert!(!sink.definition(info).is_pure());
}

// ---- unsupported control flow surfaces as an error -------------------------

#[test]
fn malformed_or_unsupported_is_reported() {
    // A `cond_br` whose edge args don't match the target block params should
    // be a clear `MalformedEdge`, not a silent guess.
    let src = r#"
stage @test fn @main(i64) -> i64;

specialize @test fn @main(i64) -> i64 {
  ^entry(%x: i64) {
    br ^target(%x);
  }
  ^target(%a: i64, %b: i64) {
    ret %a;
  }
}
"#;
    let pipeline = parse(src);
    let result = analyze_liveness_by_name(&pipeline, "test", "main");
    assert!(matches!(
        result,
        Err(crate::LivenessError::MalformedEdge { .. })
    ));
}
