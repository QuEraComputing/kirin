use kirin_ir::{Block, Dialect, GetInfo, Rewriter, SSAValue, StageInfo, Statement};

use super::{DriverError, RewriteDriver};

impl<L: Dialect> RewriteDriver<'_, L> {
    /// Erase the statements left dead under `root`, and return how many
    /// were erased.
    ///
    /// Rules never erase their root, so a rule that replaces its root's
    /// results leaves the root, and often its producers, behind with no
    /// uses. Erasing them after each sweep keeps later sweeps from walking
    /// and matching them again. This also removes dead statements no rule
    /// touched.
    ///
    /// A statement is dead if it is pure, is not a terminator, and none of
    /// its results is used. Each block is walked from its end, and nested
    /// blocks before the statement owning them, so removing one dead
    /// statement exposes its operands' producers before they are reached and
    /// a dead chain goes in one pass. Graph bodies are left alone.
    pub(super) fn clean_up_dead_code(
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<usize, DriverError> {
        let mut erased = 0;
        for block in nested_blocks(rewriter.stage(), root) {
            erased += erase_dead_in_block(rewriter, block)?;
        }
        Ok(erased)
    }
}

/// Erase the dead statements of `block` and the blocks nested in it.
fn erase_dead_in_block<L: Dialect>(
    rewriter: &mut Rewriter<'_, L>,
    block: Block,
) -> Result<usize, DriverError> {
    let mut erased = 0;
    let mut current = block.last_statement(rewriter.stage());
    while let Some(stmt) = current {
        let previous = block.previous_statement(rewriter.stage(), stmt);
        for nested in nested_blocks(rewriter.stage(), stmt) {
            erased += erase_dead_in_block(rewriter, nested)?;
        }
        if is_dead(rewriter.stage(), stmt) {
            rewriter
                .erase_statement(stmt)
                .map_err(|error| DriverError::DeadCode { stmt, error })?;
            erased += 1;
        }
        current = previous;
    }
    Ok(erased)
}

/// The blocks of `stmt`'s `Block` and `CFG` bodies.
fn nested_blocks<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Vec<Block> {
    let Some(info) = stmt.get_info(stage) else {
        return Vec::new();
    };
    let definition = info.definition();
    definition
        .blocks()
        .copied()
        .chain(definition.cfgs().flat_map(|cfg| cfg.blocks(stage)))
        .collect()
}

fn is_dead<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> bool {
    let Some(info) = stmt.get_info(stage) else {
        return false;
    };
    let definition = info.definition();
    !definition.is_terminator()
        && definition.is_pure()
        && definition.results().all(|&result| {
            SSAValue::from(result)
                .get_info(stage)
                .is_none_or(|value| value.uses().is_empty())
        })
}
