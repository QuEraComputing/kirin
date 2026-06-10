//! Structured control-flow lowering (`if`/`for`).
//!
//! `for`-range lowering is implemented in W4.

use std::collections::BTreeSet;

use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_scf::{For, If, Yield};

use super::BlockBuf;
use super::expr::lower_expr;
use super::stmt::lower_stmts;
use crate::ast::{Expr, RangeCall, Stmt};
use crate::error::LowerError;
use crate::language::PyLang;
use crate::scope::Frame;

/// Lower `if test: body else: orelse` to an `scf.If`.
///
/// Variables assigned in *both* branches become the `If`'s results (yielded
/// from each branch) and are rebound afterward. This is the structured-SSA
/// "phi" join. `return` inside a branch is not supported in this subset.
pub(crate) fn lower_if(
    ctx: &mut BuilderStageInfo<PyLang>,
    test: &Expr,
    body: &[Stmt],
    orelse: &[Stmt],
    frame: &mut Frame,
    buf: &mut BlockBuf,
) -> Result<(), LowerError> {
    let cond = super::expr::lower_expr(ctx, test, frame, buf)?;

    // Live-out join set, in a deterministic order: a name is carried out of the
    // `if` when it holds a defined value on *both* exit paths — assigned on that
    // path, or already defined before the `if` (so the branch that doesn't
    // assign it falls through to the prior value). Assigned-in-both subsumes the
    // no-prior-definition case (e.g. `pick`/`factorial`, where `r` is bound in
    // each branch); a name assigned in only one branch (e.g. an `if` with no
    // `else`) is carried only if it was defined beforehand.
    let mut then_assigned = BTreeSet::new();
    assigned_names(body, &mut then_assigned);
    let mut else_assigned = BTreeSet::new();
    assigned_names(orelse, &mut else_assigned);
    let joined: Vec<String> = then_assigned
        .union(&else_assigned)
        .filter(|name| {
            let defined_before = frame.lookup(name).is_some();
            (then_assigned.contains(*name) || defined_before)
                && (else_assigned.contains(*name) || defined_before)
        })
        .cloned()
        .collect();

    let then_block = lower_branch(ctx, body, &joined, frame)?;
    let else_block = lower_branch(ctx, orelse, &joined, frame)?;

    let if_stmt = If::<ArithType>::new(ctx, joined.len(), cond, then_block, else_block);
    buf.push(if_stmt.id);

    // Rebind joined names to the If results so later code sees the merged value.
    for (name, result) in joined.iter().zip(if_stmt.results) {
        frame.define(name, result.into());
    }
    Ok(())
}

/// Build one `scf` branch block: lower its statements in a nested scope and
/// terminate with a `yield` of the joined values (in `joined` order).
fn lower_branch(
    ctx: &mut BuilderStageInfo<PyLang>,
    body: &[Stmt],
    joined: &[String],
    frame: &mut Frame,
) -> Result<Block, LowerError> {
    frame.push();
    let mut branch_buf = BlockBuf::new();
    let lowered = lower_stmts(ctx, body, frame, &mut branch_buf);
    if let Err(e) = lowered {
        frame.pop();
        return Err(e);
    }
    if branch_buf.terminator.is_some() {
        frame.pop();
        return Err(LowerError::Unsupported(
            "`return` inside an if-branch".into(),
        ));
    }
    // Collect yield values before leaving the branch scope.
    let mut yield_values = Vec::with_capacity(joined.len());
    for name in joined {
        match frame.lookup(name) {
            Some(ssa) => yield_values.push(ssa),
            None => {
                frame.pop();
                return Err(LowerError::UndefinedName(name.clone()));
            }
        }
    }
    frame.pop();

    let yield_stmt = Yield::<ArithType>::new(ctx, yield_values);
    let block = ctx.block().name("branch").new();
    ctx.attach_statements_to_block(block, &branch_buf.stmts, Some(yield_stmt.id));
    Ok(block)
}

/// Collect the names bound by `Assign` statements anywhere within `stmts`
/// (recursing through nested `if`/`for` bodies).
fn assigned_names(stmts: &[Stmt], out: &mut BTreeSet<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Assign { target, .. } => {
                out.insert(target.clone());
            }
            Stmt::If { body, orelse, .. } => {
                assigned_names(body, out);
                assigned_names(orelse, out);
            }
            Stmt::For { body, .. } => assigned_names(body, out),
            Stmt::Return { .. } | Stmt::Expr(_) => {}
        }
    }
}

/// Lower `for target in range(lo, hi[, step]): body` to an `scf.For`.
///
/// Names assigned in the body that already exist before the loop become
/// loop-carried accumulators (`iter_args`): they enter as body block arguments
/// and their post-body values are `yield`ed and rebound after the loop.
/// `return` inside the loop body is not supported in this subset.
pub(crate) fn lower_for(
    ctx: &mut BuilderStageInfo<PyLang>,
    target: &str,
    iter: &RangeCall,
    body: &[Stmt],
    frame: &mut Frame,
    buf: &mut BlockBuf,
) -> Result<(), LowerError> {
    // Range bounds are evaluated before the loop, in the enclosing block.
    let lo = lower_expr(ctx, &iter.lo, frame, buf)?;
    let hi = lower_expr(ctx, &iter.hi, frame, buf)?;
    let step = match &iter.step {
        Some(e) => lower_expr(ctx, e, frame, buf)?,
        None => {
            let one = Constant::<ArithValue, ArithType>::new(ctx, ArithValue::I64(1));
            buf.push(one.id);
            one.result.into()
        }
    };

    // Loop-carried accumulators: assigned in the body and defined before the loop.
    let mut body_assigned = BTreeSet::new();
    assigned_names(body, &mut body_assigned);
    let accumulators: Vec<String> = body_assigned
        .into_iter()
        .filter(|n| frame.lookup(n).is_some())
        .collect();

    // Build the body block args-first: ^body(induction, acc...).
    //
    // The args are intentionally left UNNAMED: the lowering binds source names
    // to these SSAs by identity (below), and letting the printer assign fresh,
    // unique names avoids collisions across nested or sibling loops that reuse
    // the same Python variable (the parser uses one SSA scope per function body,
    // so two body args both named `%total` would be a duplicate definition).
    let mut builder = ctx.block().name("body");
    builder = builder.argument(ArithType::I64); // induction variable
    for _ in &accumulators {
        builder = builder.argument(ArithType::I64); // carried accumulator
    }
    let body_block = builder.new();
    let block_args: Vec<SSAValue> = ctx
        .block_arena()
        .get(body_block)
        .expect("for body block exists")
        .arguments
        .iter()
        .map(|a| SSAValue::from(Id::from(*a)))
        .collect();
    // The per-iteration induction value is the body's first block argument.
    let body_iv = block_args[0];

    // Lower the body with the induction var + accumulators bound to block args.
    frame.push();
    frame.define(target, body_iv);
    for (acc, ssa) in accumulators.iter().zip(&block_args[1..]) {
        frame.define(acc, *ssa);
    }
    let mut body_buf = BlockBuf::new();
    if let Err(e) = lower_stmts(ctx, body, frame, &mut body_buf) {
        frame.pop();
        return Err(e);
    }
    if body_buf.terminator.is_some() {
        frame.pop();
        return Err(LowerError::Unsupported(
            "`return` inside a for-loop body".into(),
        ));
    }
    let mut yield_values = Vec::with_capacity(accumulators.len());
    for acc in &accumulators {
        match frame.lookup(acc) {
            Some(ssa) => yield_values.push(ssa),
            None => {
                frame.pop();
                return Err(LowerError::UndefinedName(acc.clone()));
            }
        }
    }
    frame.pop();

    let yield_stmt = Yield::<ArithType>::new(ctx, yield_values);
    ctx.attach_statements_to_block(body_block, &body_buf.stmts, Some(yield_stmt.id));

    // Initial accumulator values (pre-loop; frame is back to the outer scope).
    let init_args: Vec<SSAValue> = accumulators
        .iter()
        .map(|acc| frame.lookup(acc).expect("accumulator defined before loop"))
        .collect();

    // The `induction_var` field is a reference slot resolved in the enclosing
    // scope (by convention the loop start); the real per-iteration value is the
    // body block argument (`body_iv`) bound above.
    let for_stmt = For::<ArithType>::new(
        ctx,
        accumulators.len(),
        lo,
        lo,
        hi,
        step,
        init_args,
        body_block,
    );
    buf.push(for_stmt.id);

    // Rebind accumulators to the loop results for code after the loop.
    for (acc, result) in accumulators.iter().zip(for_stmt.results) {
        frame.define(acc, result.into());
    }
    Ok(())
}
