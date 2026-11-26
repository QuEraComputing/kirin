use kirin::prelude::*;

use super::check::{Callee, Checked, check_call};
use super::ops::{InlinableCall, inlinable_call};
use super::redirects::Redirects;
use super::{FunctionOps, InlineError, NotInlined};

/// What a run of [`inline_calls`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InlineReport {
    /// How many calls were inlined.
    pub inlined: usize,
    /// The calls to a function in this stage left as calls, and why, in the
    /// order they were met. Calls of another kind, to another stage, or nested
    /// in statements' bodies are not listed.
    pub skipped: Vec<NotInlined>,
}

/// Inline every specialized function call in `cfg`'s blocks, and report how
/// many were inlined and which were left as calls. Only calls to a function
/// in this stage whose body is a CFG are inlined.
///
/// A callee that is a single block ending in a return has its statements
/// copied in front of the call, and the call's results are replaced by the
/// returned values. Any other callee is copied as blocks: the call's block is
/// split after the call, the copied blocks go in between and are entered with
/// a branch passing the call's arguments, each copied return becomes a branch
/// to the second half with the returned values, and that half takes the call's
/// results as its arguments.
/// `branch` builds the language's branch to a block with arguments.
///
/// One level: the calls the inlined code brings in are left as calls, for
/// another run to inline. Calls of another kind, to another stage, or nested
/// in statements' bodies are left as they are too.
///
/// Every call is checked before any edit. A call this inliner does not
/// handle is left as a call, untouched, and reported (see [`NotInlined`]);
/// the others are still inlined. For example, a callee with a return nested
/// in a statement's body, such as an `scf.if` arm or a loop body, is not
/// inlined: that return cannot become a branch, and copied as is it would
/// return from the caller. Returns of a callable nested in the callee, such as
/// a lambda, are copied as they are: they still return from it.
///
/// It is the caller's responsibility to ensure there is no recursion, self or
/// mutual, among the calls inlined.
///
/// Each block's calls are inlined from its last to its first, so a split only
/// moves statements after the call, which no earlier split moved, and reads
/// none of the statements before it. The calls' results are redirected once
/// every call is inlined, each straight to the value it ends up as, so a use
/// is rewritten once even when one call's result is replaced by another's, as
/// along a chain of calls to a function that returns its parameter. So a run
/// is linear in `cfg`'s size plus the size of the callees it copies. Running
/// this on each callee's body earlier than on its callers' bodies inlines
/// everything with one run per function, since a callee with no calls left
/// brings none in.
///
/// Rejects, leaving the stage untouched, if a call's callee is not a live
/// specialization, or the call's argument or result count does not match the
/// callee's signature (see [`InlineError`]).
pub fn inline_calls<L, T>(
    rewriter: &mut Rewriter<'_, L>,
    cfg: CFG,
    branch: impl Fn(Block, Vec<SSAValue>) -> L,
) -> Result<InlineReport, InlineError>
where
    L: Dialect + HasCallableBody + FunctionOps<T>,
    T: CompileTimeValue,
{
    // Check every call before any edit, each block from its last statement to
    // its first: the order they are inlined in.
    let stage = rewriter.stage();
    let mut report = InlineReport::default();
    let mut accepted = Vec::new();
    for block in cfg.blocks(stage) {
        let stmts: Vec<Statement> = block.statements(stage).collect();
        for stmt in stmts.into_iter().rev() {
            let Some(call) = inlinable_call::<L, T>(stage, stmt) else {
                continue;
            };
            match check_call::<L, T>(stage, stmt, &call)? {
                Checked::Inline(callee) => accepted.push((stmt, call, callee)),
                Checked::Skip(reason) => report.skipped.push(reason),
            }
        }
    }
    // Inlining a call copies its callee and never edits one, and redirects
    // and erases nothing until every call is inlined, so what was read above
    // still holds: each call's arguments and results are still live.
    let mut redirects = Redirects::default();
    for (stmt, call, callee) in accepted {
        inline_call(rewriter, stmt, call, callee, &branch, &mut redirects)
            .expect("`check_call` accepted the call, so inlining it cannot fail");
        report.inlined += 1;
    }
    redirects
        .apply(rewriter)
        .expect("the inlined calls' results and replacements are live");
    Ok(report)
}

/// Inline `stmt`, the call `call` to `callee`, which [`check_call`] accepted,
/// leaving the call in place and recording in `redirects` what replaces its
/// results.
fn inline_call<L>(
    rewriter: &mut Rewriter<'_, L>,
    stmt: Statement,
    call: InlinableCall,
    callee: Callee,
    branch: &impl Fn(Block, Vec<SSAValue>) -> L,
    redirects: &mut Redirects,
) -> Result<(), RewriteError>
where
    L: Dialect,
{
    let stage = rewriter.stage();
    let Callee {
        body: callee,
        blocks,
        entry,
        entry_args,
        returns,
    } = callee;
    let in_place = blocks.len() == 1 && returns.len() == 1;
    let mut map = IRMap::default();

    // A callee that is one block ending in a return needs no new blocks: copy
    // its statements in front of the call, reading the call's arguments where
    // they read the callee's; the call's results are replaced by the returned
    // values, mapped like the copied operands.
    if in_place {
        for (&arg, &value) in entry_args.iter().zip(&call.args) {
            map.map_value(arg, value);
        }
        let chain: Vec<Statement> = entry.statements(stage).collect();
        let returned: Vec<SSAValue> = returns[0]
            .expect_info(stage)
            .definition()
            .arguments()
            .copied()
            .collect();
        for original in chain {
            let copy = rewriter.clone_statement(original, &mut map)?;
            rewriter.insert(InsertionPoint::Before(stmt), copy)?;
        }
        redirects.record(
            stmt,
            &call.results,
            returned
                .into_iter()
                .map(|value| map.lookup_or_original(value)),
        );
        return Ok(());
    }

    // Split the call's block after the call: the second half takes the call's
    // results as arguments.
    let Some(StatementParent::Block(caller_block)) = *stmt.parent(stage) else {
        unreachable!("`inline_calls` only walks the blocks of a CFG")
    };
    let after = stmt.next(stage).or_else(|| caller_block.terminator(stage));
    let continuation = match after {
        Some(at) => rewriter.split_block(at)?,
        None => rewriter.insert_block_after(caller_block, [])?,
    };
    let mut args = Vec::with_capacity(call.results.len());
    for (index, &result) in call.results.iter().enumerate() {
        let ty = result.expect_info(rewriter.stage()).ty().clone();
        args.push(rewriter.insert_block_arg(continuation, index, ty, |_, _| {
            unreachable!("the second half has no predecessors yet")
        })?);
    }
    redirects.record(stmt, &call.results, args.into_iter().map(SSAValue::from));

    // Copy the callee's blocks between the call's block and its second half,
    // branch from the call's block to the copied entry with the call's
    // arguments, and turn each copied return into a branch to the second half.
    let copy = rewriter.clone_cfg(callee, &mut map)?;
    rewriter.splice_cfg_after(copy, caller_block)?;
    let entry_copy = map
        .block(entry)
        .expect("the copy has the callee's entry block");
    rewriter.create(
        InsertionPoint::Terminator(caller_block),
        branch(entry_copy, call.args),
    )?;
    for ret in returns {
        let ret_copy = map
            .statement(ret)
            .expect("the copy has the callee's returns");
        let values: Vec<SSAValue> = ret_copy
            .expect_info(rewriter.stage())
            .definition()
            .arguments()
            .copied()
            .collect();
        rewriter.replace_statement(ret_copy, branch(continuation, values))?;
    }
    Ok(())
}
