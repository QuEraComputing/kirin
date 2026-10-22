use kirin_ir::{
    Block, DiGraph, Dialect, GetInfo, Rewriter, SSAKind, SSAValue, StageInfo, Statement,
};

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
    /// its results is used. Bodies nested in a statement are cleaned before
    /// the statement itself, and erasing a dead statement can only make its
    /// operands' producers dead, so each body is cleaned in one pass:
    ///
    /// - a block is walked from its end, so producers come after their users;
    /// - a `DiGraph` is walked from its sinks, the members none of whose
    ///   results is used (by another member or a yield), and a member's
    ///   producers are queued when it is erased.
    ///
    /// `UnGraph` bodies are left alone: their nodes usually have no results
    /// and mean something through their wires, so "no result is used" does
    /// not make them dead.
    pub(super) fn clean_up_dead_code(
        rewriter: &mut Rewriter<'_, L>,
        root: Statement,
    ) -> Result<usize, DriverError> {
        erase_dead_nested_in(rewriter, root)
    }
}

/// Erase the dead statements in the `Block`, `CFG` and `DiGraph` bodies of
/// `stmt`, and in the bodies nested in those.
fn erase_dead_nested_in<L: Dialect>(
    rewriter: &mut Rewriter<'_, L>,
    stmt: Statement,
) -> Result<usize, DriverError> {
    let Some(info) = stmt.get_info(rewriter.stage()) else {
        return Ok(0);
    };
    let definition = info.definition();
    let stage = rewriter.stage();
    let blocks: Vec<Block> = definition
        .blocks()
        .copied()
        .chain(definition.cfgs().flat_map(|cfg| cfg.blocks(stage)))
        .collect();
    let digraphs: Vec<DiGraph> = definition.digraphs().copied().collect();

    let mut erased = 0;
    for block in blocks {
        erased += erase_dead_in_block(rewriter, block)?;
    }
    for graph in digraphs {
        erased += erase_dead_in_digraph(rewriter, graph)?;
    }
    Ok(erased)
}

/// Erase the dead statements of `block`, walking it from its end.
fn erase_dead_in_block<L: Dialect>(
    rewriter: &mut Rewriter<'_, L>,
    block: Block,
) -> Result<usize, DriverError> {
    let mut erased = 0;
    let mut current = block.last_statement(rewriter.stage());
    while let Some(stmt) = current {
        let previous = block.previous_statement(rewriter.stage(), stmt);
        erased += erase_dead_nested_in(rewriter, stmt)?;
        if is_dead(rewriter.stage(), stmt) {
            erase(rewriter, stmt)?;
            erased += 1;
        }
        current = previous;
    }
    Ok(erased)
}

/// Erase the dead members of `graph`, starting from its sinks.
///
/// Erasing a member queues its producers. A producer still used by another
/// consumer is skipped when popped, and queued again when that consumer is
/// erased, so each is queued once per erased consumer: O(members + edges).
fn erase_dead_in_digraph<L: Dialect>(
    rewriter: &mut Rewriter<'_, L>,
    graph: DiGraph,
) -> Result<usize, DriverError> {
    let members: Vec<Statement> = graph
        .get_info(rewriter.stage())
        .map(|info| info.nodes().collect())
        .unwrap_or_default();

    let mut erased = 0;
    for &member in &members {
        erased += erase_dead_nested_in(rewriter, member)?;
    }

    // The sinks: members no other member or yield uses.
    let mut worklist: Vec<Statement> = members
        .into_iter()
        .filter(|&member| results_unused(rewriter.stage(), member))
        .collect();
    while let Some(node) = worklist.pop() {
        if !is_dead(rewriter.stage(), node) {
            continue;
        }
        let producers = producers_in(rewriter.stage(), node, graph);
        erase(rewriter, node)?;
        erased += 1;
        worklist.extend(producers);
    }
    Ok(erased)
}

/// The members of `graph` producing `node`'s operands, once per operand.
fn producers_in<L: Dialect>(
    stage: &StageInfo<L>,
    node: Statement,
    graph: DiGraph,
) -> Vec<Statement> {
    let mut producers = Vec::new();
    let Some(info) = node.get_info(stage) else {
        return producers;
    };
    for &operand in info.definition().arguments() {
        let Some(SSAKind::Result(producer, _)) = operand.get_info(stage).map(|v| *v.kind()) else {
            continue;
        };
        let in_graph = graph
            .get_info(stage)
            .is_some_and(|info| info.member(producer).is_some());
        if in_graph {
            producers.push(producer);
        }
    }
    producers
}

fn erase<L: Dialect>(rewriter: &mut Rewriter<'_, L>, stmt: Statement) -> Result<(), DriverError> {
    rewriter
        .erase_statement(stmt)
        .map_err(|error| DriverError::DeadCode { stmt, error })
}

fn is_dead<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> bool {
    // `None` for an erased statement: lookups only return live ones.
    let Some(info) = stmt.get_info(stage) else {
        return false;
    };
    let definition = info.definition();
    !definition.is_terminator() && definition.is_pure() && results_unused(stage, stmt)
}

/// Whether `stmt` is live and none of its results is used.
fn results_unused<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> bool {
    stmt.get_info(stage).is_some_and(|info| {
        info.definition().results().all(|&result| {
            SSAValue::from(result)
                .get_info(stage)
                .is_none_or(|value| value.uses().is_empty())
        })
    })
}
