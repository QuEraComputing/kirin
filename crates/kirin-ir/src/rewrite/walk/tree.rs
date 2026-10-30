//! Navigation over the statement nesting tree, one step at a time.
//!
//! A statement's children are the statements in the bodies it owns, in a
//! fixed order: its `Block`s, then its `CFG`s (blocks in layout order), then
//! its `DiGraph`s and `UnGraph`s (members in member order). Within a block,
//! the terminator comes last.
//!
//! Siblings follow the IR's own lists: a block's statement list, a `CFG`'s
//! block layout list, and a `DiGraph`'s or `UnGraph`'s member list.
//! Terminator successors and graph connectivity are not read, so across
//! blocks and graph members the order is deterministic but not program
//! order.
//!
//! Each step reads the current links, so it sees every edit made before it.

use crate::{BlockParent, Body, Dialect, GetInfo, StageInfo, Statement, StatementParent};

/// `stmt`'s first child, or `None` if it has none or is not live.
pub(super) fn first_child<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Option<Statement> {
    first_in_bodies_after(stage, stmt, None)
}

/// The child of `stmt`'s parent that comes after `stmt`, possibly in a later
/// body of that parent. `None` if `stmt` is the last child, is a top-level
/// statement, or is not live.
///
/// O(1) within a block or graph. Leaving a body rescans the parent's bodies
/// to find the next one, which costs O(number of bodies); ops own only a
/// few, so this is effectively constant.
pub(super) fn next_sibling<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Option<Statement> {
    match stmt.get_info(stage)?.parent? {
        StatementParent::Block(block) => {
            if let Some(next) = block.next_statement(stage, stmt) {
                return Some(next);
            }
            match block.get_info(stage)?.parent()? {
                BlockParent::CFG(cfg) => {
                    let mut later = block.get_info(stage)?.node().next;
                    while let Some(next_block) = later {
                        if let Some(first) = next_block.first_statement(stage) {
                            return Some(first);
                        }
                        later = next_block.get_info(stage)?.node().next;
                    }
                    let owner = cfg.get_info(stage)?.parent?;
                    first_in_bodies_after(stage, owner, Some(Body::CFG(cfg)))
                }
                BlockParent::Statement(owner) => {
                    first_in_bodies_after(stage, owner, Some(Body::Block(block)))
                }
            }
        }
        StatementParent::DiGraph(graph) => {
            let info = graph.get_info(stage)?;
            if let Some(next) = info.member_after(stmt) {
                return Some(next);
            }
            first_in_bodies_after(stage, info.parent()?, Some(Body::DiGraph(graph)))
        }
        StatementParent::UnGraph(graph) => {
            let info = graph.get_info(stage)?;
            if let Some(next) = info.member_after(stmt) {
                return Some(next);
            }
            first_in_bodies_after(stage, info.parent()?, Some(Body::UnGraph(graph)))
        }
    }
}

/// The statement owning the body `stmt` is in, or `None` for a top-level or
/// dead statement.
pub(super) fn parent<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> Option<Statement> {
    match stmt.get_info(stage)?.parent? {
        StatementParent::Block(block) => match block.get_info(stage)?.parent()? {
            BlockParent::CFG(cfg) => cfg.get_info(stage)?.parent,
            BlockParent::Statement(owner) => Some(owner),
        },
        StatementParent::DiGraph(graph) => graph.get_info(stage)?.parent(),
        StatementParent::UnGraph(graph) => graph.get_info(stage)?.parent(),
    }
}

/// Whether `stmt` is live and owns any body, empty or not.
pub(super) fn owns_bodies<L: Dialect>(stage: &StageInfo<L>, stmt: Statement) -> bool {
    stmt.get_info(stage).is_some_and(|info| {
        let definition = info.definition();
        definition.blocks().next().is_some()
            || definition.cfgs().next().is_some()
            || definition.digraphs().next().is_some()
            || definition.ungraphs().next().is_some()
    })
}

/// Append `stmt`'s children to `dest`. Returns `false`, appending nothing, if
/// `stmt` is not live.
pub(super) fn append_children<L: Dialect>(
    stage: &StageInfo<L>,
    stmt: Statement,
    dest: &mut Vec<Statement>,
) -> bool {
    if stmt.get_info(stage).is_none() {
        return false;
    }
    let mut child = first_child(stage, stmt);
    while let Some(current) = child {
        dest.push(current);
        child = next_sibling(stage, current);
    }
    true
}

/// The first statement in `owner`'s bodies that come after `after` (all of
/// them for `None`), skipping empty bodies.
fn first_in_bodies_after<L: Dialect>(
    stage: &StageInfo<L>,
    owner: Statement,
    after: Option<Body>,
) -> Option<Statement> {
    let definition = owner.get_info(stage)?.definition();
    let bodies = definition
        .blocks()
        .map(|&block| Body::Block(block))
        .chain(definition.cfgs().map(|&cfg| Body::CFG(cfg)))
        .chain(definition.digraphs().map(|&graph| Body::DiGraph(graph)))
        .chain(definition.ungraphs().map(|&graph| Body::UnGraph(graph)));
    let mut skipping = after;
    for body in bodies {
        if let Some(target) = skipping {
            if body == target {
                skipping = None;
            }
            continue;
        }
        if let Some(first) = first_statement(stage, body) {
            return Some(first);
        }
    }
    None
}

/// The first statement of `body`, or `None` if it is empty.
fn first_statement<L: Dialect>(stage: &StageInfo<L>, body: Body) -> Option<Statement> {
    match body {
        Body::Block(block) => block.first_statement(stage),
        Body::CFG(cfg) => cfg
            .blocks(stage)
            .find_map(|block| block.first_statement(stage)),
        Body::DiGraph(graph) => graph.get_info(stage)?.first_member(),
        Body::UnGraph(graph) => graph.get_info(stage)?.first_member(),
    }
}
