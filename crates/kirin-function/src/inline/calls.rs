use kirin::prelude::*;

use super::ops::{InlinableCall, inlinable_call};
use super::{FunctionOps, InlineError};

/// Inline every specialized function call in `cfg`'s blocks, and return how
/// many were inlined. Only calls to a function in this stage whose body is a
/// CFG are inlined.
///
/// A callee that is a single block ending in a return has its statements
/// copied in front of the call, and the call's results are replaced by the
/// returned values. Any other callee is copied as blocks: the call's block is split
/// after the call, the copied blocks go in between and are entered with a
/// branch, each copied return becomes a branch to the second half with the
/// returned values, and that half takes the call's results as its arguments.
/// `branch` builds the language's branch to a block with arguments.
///
/// One level: the calls the inlined code brings in are left as calls, for
/// another run to inline. Calls of another kind, to another stage, or nested
/// in statements' bodies are left as they are too.
///
/// Each block is walked from its last statement to its first, so a split only
/// moves statements after the call, which no earlier split moved: a run is
/// linear in `cfg`'s size plus the size of the callees it copies. Running this
/// on each callee's body earlier than on its callers' bodies inlines everything
/// with one run per function, since a callee with no calls left brings none in.
///
/// Stops at the first call it cannot inline (see [`InlineError`]), leaving
/// the calls inlined before it inlined.
pub fn inline_calls<L, T>(
    rewriter: &mut Rewriter<'_, L>,
    cfg: CFG,
    branch: impl Fn(Block, Vec<SSAValue>) -> L,
) -> Result<usize, InlineError>
where
    L: Dialect + HasCallableBody + FunctionOps<T>,
    T: CompileTimeValue,
{
    let mut inlined = 0;
    let blocks: Vec<Block> = cfg.blocks(rewriter.stage()).collect();
    for block in blocks {
        let mut cursor = block
            .expect_info(rewriter.stage())
            .statements()
            .tail()
            .copied();
        while let Some(stmt) = cursor {
            // Read before inlining: the statement before the call, whatever
            // inlining puts in between.
            cursor = *stmt.prev(rewriter.stage());
            if let Some(call) = inlinable_call::<L, T>(rewriter.stage(), stmt) {
                inline_call(rewriter, stmt, call, &branch)?;
                inlined += 1;
            }
        }
    }
    Ok(inlined)
}

/// Inline `stmt`, the call `call`, after checking that it can be.
fn inline_call<L, T>(
    rewriter: &mut Rewriter<'_, L>,
    stmt: Statement,
    call: InlinableCall,
    branch: &impl Fn(Block, Vec<SSAValue>) -> L,
) -> Result<(), InlineError>
where
    L: Dialect + HasCallableBody + FunctionOps<T>,
    T: CompileTimeValue,
{
    let stage = rewriter.stage();
    let definition = call
        .target
        .get_info(stage)
        .map(|info| *info.definition())
        .filter(|definition| definition.get_info(stage).is_some())
        .ok_or(InlineError::UnknownCallee(stmt))?;
    let Some(Body::CFG(callee)) = definition.expect_info(stage).definition().callable_body() else {
        return Err(InlineError::CalleeBodyNotCfg(stmt));
    };
    let blocks: Vec<Block> = callee.blocks(stage).collect();
    let &[entry, ..] = blocks.as_slice() else {
        return Err(InlineError::CalleeBodyNotCfg(stmt));
    };
    let entry_args: Vec<SSAValue> = entry
        .expect_info(stage)
        .arguments()
        .iter()
        .map(|&arg| arg.into())
        .collect();
    if entry_args.len() != call.args.len() {
        return Err(InlineError::ArgumentCountMismatch {
            call: stmt,
            parameters: entry_args.len(),
            arguments: call.args.len(),
        });
    }
    // Every return of the callee, by its block.
    let mut returns = Vec::new();
    for &block in &blocks {
        let Some(terminator) = block.terminator(stage) else {
            continue;
        };
        if let Some(ret) = terminator.expect_info(stage).definition().as_return() {
            let returned = ret.arguments().count();
            if returned != call.results.len() {
                return Err(InlineError::ReturnCountMismatch {
                    call: stmt,
                    results: call.results.len(),
                    returned,
                });
            }
            returns.push(terminator);
        }
    }
    let in_place = blocks.len() == 1 && returns.len() == 1;
    // The callee reads the call's arguments where it read its own.
    let mut map = IRMap::default();
    for (&arg, &value) in entry_args.iter().zip(&call.args) {
        map.map_value(arg, value);
    }

    // A callee that is one block ending in a return needs no new blocks: copy
    // its statements in front of the call, replace the call's results with
    // the returned values, mapped like the copied operands, and erase the call.
    if in_place {
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
        for (result, value) in call.results.into_iter().zip(returned) {
            rewriter.replace_all_uses(result, map.value(value).unwrap_or(value))?;
        }
        rewriter.erase_statement(stmt)?;
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
    for (index, &result) in call.results.iter().enumerate() {
        let ty = result.expect_info(rewriter.stage()).ty().clone();
        let arg = rewriter.insert_block_arg(continuation, index, ty, |_, _| {
            unreachable!("the second half has no predecessors yet")
        })?;
        rewriter.replace_all_uses(result, arg)?;
    }

    // Copy the callee's blocks between the call's block and its second half,
    // branch from the call's block to the copied entry, and turn each copied
    // return into a branch to the second half.
    let copy = rewriter.clone_cfg(callee, &mut map)?;
    rewriter.splice_cfg_after(copy, caller_block)?;
    let entry_copy = map
        .block(entry)
        .expect("the copy has the callee's entry block");
    rewriter.create(
        InsertionPoint::Terminator(caller_block),
        branch(entry_copy, Vec::new()),
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
    rewriter.erase_statement(stmt)?;
    Ok(())
}
