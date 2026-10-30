//! Visit orders of the `walk` traversal orders over nested bodies.

mod common;

use common::{BuilderDialect, new_stage};
use kirin_ir::walk::{
    BFS, KeepOrderReverseBFS, PostOrder, PreOrder, ReverseBFS, WalkCursor, WalkError, WalkOrder,
    WalkResult, WalkStatus,
};
use kirin_ir::*;

/// The statements of a two-level tree, by name.
struct Tree {
    stage: StageInfo<BuilderDialect>,
    root: Statement,
    a: Statement,
    owner: Statement,
    ret: Statement,
    then_nop: Statement,
    then_ret: Statement,
    else_nop: Statement,
    else_ret: Statement,
}

/// ```text
/// root = own_cfg {
///   a = nop
///   owner = own_blocks { then_nop; then_ret } { else_nop; else_ret }
///   ret
/// }
/// ```
fn tree() -> Tree {
    let mut stage = new_stage();
    let then_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let then_ret = stage.statement().definition(BuilderDialect::Return).new();
    let then_block = stage.block().stmt(then_nop).terminator(then_ret).new();
    let else_nop = stage.statement().definition(BuilderDialect::Nop).new();
    let else_ret = stage.statement().definition(BuilderDialect::Return).new();
    let else_block = stage.block().stmt(else_nop).terminator(else_ret).new();

    let a = stage.statement().definition(BuilderDialect::Nop).new();
    let owner = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(then_block, else_block))
        .new();
    let ret = stage.statement().definition(BuilderDialect::Return).new();
    let entry = stage.block().stmt(a).stmt(owner).terminator(ret).new();
    let cfg = stage.cfg().add_block(entry).new();
    let root = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(cfg))
        .new();

    Tree {
        stage: stage.finalize().unwrap(),
        root,
        a,
        owner,
        ret,
        then_nop,
        then_ret,
        else_nop,
        else_ret,
    }
}

#[test]
fn pre_order_visits_a_statement_before_its_bodies() {
    let t = tree();
    assert_eq!(
        PreOrder::schedule(&t.stage, t.root),
        [
            t.root, t.a, t.owner, t.then_nop, t.then_ret, t.else_nop, t.else_ret, t.ret
        ]
    );
}

#[test]
fn post_order_visits_a_statement_after_its_bodies() {
    let t = tree();
    assert_eq!(
        PostOrder::schedule(&t.stage, t.root),
        [
            t.a, t.then_nop, t.then_ret, t.else_nop, t.else_ret, t.owner, t.ret, t.root
        ]
    );
}

#[test]
fn bfs_visits_level_by_level() {
    let t = tree();
    assert_eq!(
        BFS::schedule(&t.stage, t.root),
        [
            t.root, t.a, t.owner, t.ret, t.then_nop, t.then_ret, t.else_nop, t.else_ret
        ]
    );
}

#[test]
fn reverse_bfs_is_bfs_backwards() {
    let t = tree();
    assert_eq!(
        ReverseBFS::schedule(&t.stage, t.root),
        [
            t.else_ret, t.else_nop, t.then_ret, t.then_nop, t.ret, t.owner, t.a, t.root
        ]
    );
}

#[test]
fn keep_order_reverse_bfs_visits_the_deepest_level_first_in_program_order() {
    let t = tree();
    assert_eq!(
        KeepOrderReverseBFS::schedule(&t.stage, t.root),
        [
            t.then_nop, t.then_ret, t.else_nop, t.else_ret, t.a, t.owner, t.ret, t.root
        ]
    );
}

#[test]
fn a_leaf_root_schedules_only_itself() {
    let t = tree();
    assert_eq!(PreOrder::schedule(&t.stage, t.a), [t.a]);
    assert_eq!(PostOrder::schedule(&t.stage, t.a), [t.a]);
    assert_eq!(BFS::schedule(&t.stage, t.a), [t.a]);
    assert_eq!(ReverseBFS::schedule(&t.stage, t.a), [t.a]);
    assert_eq!(KeepOrderReverseBFS::schedule(&t.stage, t.a), [t.a]);
}

#[test]
fn an_erased_subtree_is_not_scheduled() {
    let mut t = tree();
    Rewriter::new(&mut t.stage)
        .erase_statement(t.owner)
        .expect("owner has no results and is not a terminator");

    assert_eq!(PreOrder::schedule(&t.stage, t.root), [t.root, t.a, t.ret]);
    assert_eq!(PostOrder::schedule(&t.stage, t.root), [t.a, t.ret, t.root]);
    assert!(PreOrder::schedule(&t.stage, t.owner).is_empty());
    for nested in [t.then_nop, t.then_ret, t.else_nop, t.else_ret] {
        assert!(
            nested.get_info(&t.stage).is_none(),
            "erasing a statement tombstones what is nested in it"
        );
    }
}

#[test]
fn depth_first_orders_skip_empty_bodies() {
    // a { b { d } {} } { c }
    let mut stage = new_stage();
    let d = stage.statement().definition(BuilderDialect::Nop).new();
    let with_d = stage.block().stmt(d).new();
    let empty = stage.block().new();
    let b = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(with_d, empty))
        .new();
    let with_b = stage.block().stmt(b).new();
    let c = stage.statement().definition(BuilderDialect::Nop).new();
    let with_c = stage.block().stmt(c).new();
    let a = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(with_b, with_c))
        .new();
    let stage = stage.finalize().unwrap();

    assert_eq!(PreOrder::schedule(&stage, a), [a, b, d, c]);
    assert_eq!(PostOrder::schedule(&stage, a), [d, b, c, a]);
    assert_eq!(BFS::schedule(&stage, a), [a, b, c, d]);
    assert_eq!(ReverseBFS::schedule(&stage, a), [d, c, b, a]);
    assert_eq!(KeepOrderReverseBFS::schedule(&stage, a), [d, b, c, a]);
}

#[test]
fn walks_cross_cfg_blocks_and_graph_bodies() {
    // root = own_cfg { ^empty {} ^bb1 { x = own_graphs [n1, n2] [u1] } ^bb2 { y } }
    let mut stage = new_stage();
    let n1 = stage.statement().definition(BuilderDialect::Nop).new();
    let n2 = stage.statement().definition(BuilderDialect::Nop).new();
    let digraph = stage.digraph().node(n1).node(n2).new();
    let u1 = stage.statement().definition(BuilderDialect::Nop).new();
    let ungraph = stage.ungraph().node(u1).new();
    let x = stage
        .statement()
        .definition(BuilderDialect::OwnGraphs(digraph, ungraph))
        .new();
    let y = stage.statement().definition(BuilderDialect::Nop).new();
    let empty = stage.block().new();
    let bb1 = stage.block().stmt(x).new();
    let bb2 = stage.block().stmt(y).new();
    let cfg = stage
        .cfg()
        .add_block(empty)
        .add_block(bb1)
        .add_block(bb2)
        .new();
    let root = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(cfg))
        .new();
    let stage = stage.finalize().unwrap();

    assert_eq!(PreOrder::schedule(&stage, root), [root, x, n1, n2, u1, y]);
    assert_eq!(PostOrder::schedule(&stage, root), [n1, n2, u1, x, y, root]);
}

#[test]
fn a_nested_root_walks_only_its_subtree() {
    let t = tree();
    let subtree = [t.then_nop, t.then_ret, t.else_nop, t.else_ret];
    let mut pre = vec![t.owner];
    pre.extend(subtree);
    assert_eq!(PreOrder::schedule(&t.stage, t.owner), pre);
    let mut post = subtree.to_vec();
    post.push(t.owner);
    assert_eq!(PostOrder::schedule(&t.stage, t.owner), post);
}

/// Walk `O` under `t.root`, running `edit` at each statement, and return the
/// statements visited.
fn walk_editing<O: WalkOrder>(
    t: &mut Tree,
    mut edit: impl FnMut(&mut Rewriter<BuilderDialect>, Statement),
) -> Result<Vec<Statement>, WalkError> {
    let mut rewriter = Rewriter::new(&mut t.stage);
    let mut cursor = O::start(rewriter.stage(), t.root);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage())? {
        visited.push(stmt);
        edit(&mut rewriter, stmt);
    }
    Ok(visited)
}

#[test]
fn pre_order_walks_children_inserted_while_visiting() {
    let mut t = tree();
    let (owner, then_ret) = (t.owner, t.then_ret);
    let mut inserted = None;
    let visited = walk_editing::<PreOrder>(&mut t, |rewriter, stmt| {
        if stmt == owner {
            inserted = Some(
                rewriter
                    .insert_before(then_ret, BuilderDialect::Nop)
                    .unwrap(),
            );
        }
    })
    .unwrap();
    let inserted = inserted.unwrap();
    assert_eq!(
        visited,
        [
            t.root, t.a, t.owner, t.then_nop, inserted, t.then_ret, t.else_nop, t.else_ret, t.ret
        ]
    );
}

#[test]
fn post_order_walks_siblings_inserted_after_the_visited_statement() {
    let mut t = tree();
    let a = t.a;
    let mut inserted = None;
    let visited = walk_editing::<PostOrder>(&mut t, |rewriter, stmt| {
        if stmt == a {
            inserted = Some(rewriter.insert_after(a, BuilderDialect::Nop).unwrap());
        }
    })
    .unwrap();
    assert_eq!(visited[..2], [t.a, inserted.unwrap()]);
}

/// The number of statements `O` visits under `tree()`'s root when a
/// statement is inserted after `a` while visiting `a`.
fn visits_with_insert_after_a<O: WalkOrder>() -> usize {
    let mut t = tree();
    let a = t.a;
    walk_editing::<O>(&mut t, |rewriter, stmt| {
        if stmt == a {
            rewriter.insert_after(a, BuilderDialect::Nop).unwrap();
        }
    })
    .unwrap()
    .len()
}

#[test]
fn reverse_bfs_orders_do_not_walk_statements_inserted_after_they_start() {
    assert_eq!(visits_with_insert_after_a::<ReverseBFS>(), 8);
    assert_eq!(visits_with_insert_after_a::<KeepOrderReverseBFS>(), 8);
}

/// `root = own_cfg { a = own_blocks { x } {}; b = own_blocks { y } {}; c }`,
/// and its statements `[root, a, b, c, x, y]`.
fn two_owners() -> (StageInfo<BuilderDialect>, [Statement; 6]) {
    let mut stage = new_stage();
    let x = stage.statement().definition(BuilderDialect::Nop).new();
    let with_x = stage.block().stmt(x).new();
    let empty_a = stage.block().new();
    let a = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(with_x, empty_a))
        .new();
    let y = stage.statement().definition(BuilderDialect::Nop).new();
    let with_y = stage.block().stmt(y).new();
    let empty_b = stage.block().new();
    let b = stage
        .statement()
        .definition(BuilderDialect::OwnBlocks(with_y, empty_b))
        .new();
    let c = stage.statement().definition(BuilderDialect::Nop).new();
    let entry = stage.block().stmt(a).stmt(b).stmt(c).new();
    let cfg = stage.cfg().add_block(entry).new();
    let root = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(cfg))
        .new();
    (stage.finalize().unwrap(), [root, a, b, c, x, y])
}

#[test]
fn bfs_walks_statements_inserted_at_the_current_level_and_below() {
    let (mut stage, [root, a, b, c, x, y]) = two_owners();
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = BFS::start(rewriter.stage(), root);
    let (mut n, mut z, mut h) = (None, None, None);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage()).unwrap() {
        visited.push(stmt);
        if stmt == a {
            // Ahead of the cursor at the current level, and one level down.
            n = Some(rewriter.insert_after(b, BuilderDialect::Nop).unwrap());
            z = Some(rewriter.insert_after(x, BuilderDialect::Nop).unwrap());
        }
        if stmt == y {
            // At a level that is already finished.
            h = Some(rewriter.insert_after(a, BuilderDialect::Nop).unwrap());
        }
    }
    let (n, z, h) = (n.unwrap(), z.unwrap(), h.unwrap());
    assert_eq!(visited, [root, a, b, n, c, x, z, y]);
    assert!(!visited.contains(&h));
}

#[test]
fn bfs_skips_statements_erased_ahead_of_it() {
    let (mut stage, [root, a, b, c, x, _y]) = two_owners();
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = BFS::start(rewriter.stage(), root);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage()).unwrap() {
        visited.push(stmt);
        if stmt == a {
            // A sibling ahead, and a whole subtree waiting to be walked.
            rewriter.erase_statement(c).unwrap();
            rewriter.erase_statement(b).unwrap();
        }
    }
    assert_eq!(visited, [root, a, x]);
}

/// Erase `t.a` while visiting it under order `O`.
fn erase_while_visiting<O: WalkOrder>() -> (Statement, Result<Vec<Statement>, WalkError>) {
    let mut t = tree();
    let a = t.a;
    let result = walk_editing::<O>(&mut t, |rewriter, stmt| {
        if stmt == a {
            rewriter.erase_statement(a).unwrap();
        }
    });
    (a, result)
}

#[test]
fn erasing_the_visited_statement_alone_breaks_link_following_walks() {
    for (a, result) in [
        erase_while_visiting::<PreOrder>(),
        erase_while_visiting::<PostOrder>(),
        erase_while_visiting::<BFS>(),
    ] {
        assert_eq!(result, Err(WalkError::VisitedErased(a)));
    }
}

#[test]
fn reverse_bfs_orders_go_on_after_the_visited_statement_is_erased() {
    let t = tree();
    let (_, reverse) = erase_while_visiting::<ReverseBFS>();
    assert_eq!(reverse.unwrap(), ReverseBFS::schedule(&t.stage, t.root));
    let (_, keep_order) = erase_while_visiting::<KeepOrderReverseBFS>();
    assert_eq!(
        keep_order.unwrap(),
        KeepOrderReverseBFS::schedule(&t.stage, t.root)
    );
}

#[test]
fn reverse_bfs_orders_skip_the_rest_of_an_ancestor_erased_while_visiting() {
    let mut t = tree();
    let (owner, then_nop, then_ret) = (t.owner, t.then_nop, t.then_ret);
    let reverse = walk_editing::<ReverseBFS>(&mut t, |rewriter, stmt| {
        if stmt == then_ret {
            rewriter.erase_statement(owner).unwrap();
        }
    });
    assert_eq!(
        reverse.unwrap(),
        [t.else_ret, t.else_nop, t.then_ret, t.ret, t.a, t.root]
    );

    let mut t = tree();
    let keep_order = walk_editing::<KeepOrderReverseBFS>(&mut t, |rewriter, stmt| {
        if stmt == then_nop {
            rewriter.erase_statement(owner).unwrap();
        }
    });
    assert_eq!(keep_order.unwrap(), [t.then_nop, t.a, t.ret, t.root]);
}

#[test]
fn bfs_goes_on_with_the_next_parent_after_the_parent_is_erased() {
    let (mut stage, [root, a, b, c, x, y]) = two_owners();
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = BFS::start(rewriter.stage(), root);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage()).unwrap() {
        visited.push(stmt);
        if stmt == x {
            rewriter.erase_statement(a).unwrap();
        }
    }
    assert_eq!(visited, [root, a, b, c, x, y]);
}

/// ```text
/// root = own_cfg {
///   g = own_blocks { p = own_blocks { a } {} } {}
///   h = own_blocks { q = own_blocks { d } {} } {}
/// }
/// ```
/// and its statements `[root, g, h, p, q, a, d]`, in BFS order.
fn bfs_three_levels() -> (StageInfo<BuilderDialect>, [Statement; 7]) {
    let mut stage = new_stage();
    let owner_of = |stage: &mut BuilderStageInfo<BuilderDialect>, child: Statement| {
        let body = stage.block().stmt(child).new();
        let empty = stage.block().new();
        stage
            .statement()
            .definition(BuilderDialect::OwnBlocks(body, empty))
            .new()
    };
    let a = stage.statement().definition(BuilderDialect::Nop).new();
    let p = owner_of(&mut stage, a);
    let g = owner_of(&mut stage, p);
    let d = stage.statement().definition(BuilderDialect::Nop).new();
    let q = owner_of(&mut stage, d);
    let h = owner_of(&mut stage, q);
    let entry = stage.block().stmt(g).stmt(h).new();
    let cfg = stage.cfg().add_block(entry).new();
    let root = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(cfg))
        .new();
    (stage.finalize().unwrap(), [root, g, h, p, q, a, d])
}

#[test]
fn bfs_goes_on_outside_a_grandparent_erased_while_visiting() {
    let (mut stage, [root, g, h, p, q, a, d]) = bfs_three_levels();
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = BFS::start(rewriter.stage(), root);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage()).unwrap() {
        visited.push(stmt);
        if stmt == a {
            // Erases `p` and `a` with it; `p` is the front parent.
            rewriter.erase_statement(g).unwrap();
        }
    }
    assert_eq!(visited, [root, g, h, p, q, a, d]);
}

#[test]
fn pre_order_skips_children_and_siblings_erased_while_visiting() {
    let (mut stage, [root, a, b, c, x, y]) = two_owners();
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = PreOrder::start(rewriter.stage(), root);
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(rewriter.stage()).unwrap() {
        visited.push(stmt);
        if stmt == a {
            // Its only child, and a sibling ahead of the cursor.
            rewriter.erase_statement(x).unwrap();
            rewriter.erase_statement(c).unwrap();
        }
    }
    assert_eq!(visited, [root, a, b, y]);
}

/// Walk `O` under `root`, returning `result` when visiting `at` and
/// `Advance` elsewhere; the statements visited and how the walk ended.
fn steered<O: WalkOrder>(
    stage: &StageInfo<BuilderDialect>,
    root: Statement,
    at: Statement,
    result: WalkResult,
) -> (Vec<Statement>, WalkStatus) {
    let mut visited = Vec::new();
    let status = O::walk(stage, root, |_, stmt| {
        visited.push(stmt);
        if stmt == at {
            result
        } else {
            WalkResult::Advance
        }
    });
    (visited, status)
}

/// `steered` for `O`, with `Skip`, checked to complete.
fn skipping<O: WalkOrder>(t: &Tree, at: Statement) -> Vec<Statement> {
    let (visited, status) = steered::<O>(&t.stage, t.root, at, WalkResult::Skip);
    assert_eq!(status, WalkStatus::Completed);
    visited
}

#[test]
fn advancing_everywhere_walks_the_whole_schedule() {
    let t = tree();
    let (visited, status) = steered::<PreOrder>(&t.stage, t.root, t.owner, WalkResult::Advance);
    assert_eq!(visited, PreOrder::schedule(&t.stage, t.root));
    assert_eq!(status, WalkStatus::Completed);
}

#[test]
fn skip_leaves_out_the_subtree_in_pre_order_and_bfs() {
    let t = tree();
    assert_eq!(
        skipping::<PreOrder>(&t, t.owner),
        [t.root, t.a, t.owner, t.ret]
    );
    assert_eq!(skipping::<BFS>(&t, t.owner), [t.root, t.a, t.owner, t.ret]);
}

#[test]
fn skip_has_no_effect_in_orders_visiting_children_first() {
    let t = tree();
    assert_eq!(
        skipping::<PostOrder>(&t, t.owner),
        PostOrder::schedule(&t.stage, t.root)
    );
    assert_eq!(
        skipping::<ReverseBFS>(&t, t.owner),
        ReverseBFS::schedule(&t.stage, t.root)
    );
    assert_eq!(
        skipping::<KeepOrderReverseBFS>(&t, t.owner),
        KeepOrderReverseBFS::schedule(&t.stage, t.root)
    );
}

#[test]
fn skipping_a_statement_without_bodies_changes_nothing() {
    let t = tree();
    assert_eq!(
        skipping::<PreOrder>(&t, t.a),
        PreOrder::schedule(&t.stage, t.root)
    );
    assert_eq!(skipping::<BFS>(&t, t.a), BFS::schedule(&t.stage, t.root));
}

#[test]
fn skipping_the_root_visits_only_the_root_in_pre_order_and_bfs() {
    let t = tree();
    assert_eq!(skipping::<PreOrder>(&t, t.root), [t.root]);
    assert_eq!(skipping::<BFS>(&t, t.root), [t.root]);
}

#[test]
fn skip_in_bfs_leaves_out_only_the_skipped_subtree() {
    let (stage, [root, a, b, c, _x, y]) = two_owners();
    let (visited, _) = steered::<BFS>(&stage, root, a, WalkResult::Skip);
    assert_eq!(visited, [root, a, b, c, y]);
}

/// `steered` for `O`, interrupting at `t.owner`: the walk ends right after
/// it, having visited what the full schedule visits up to it.
fn check_interrupts<O: WalkOrder>(t: &Tree) {
    let (visited, status) = steered::<O>(&t.stage, t.root, t.owner, WalkResult::Interrupt);
    assert_eq!(status, WalkStatus::Interrupted);
    let schedule = O::schedule(&t.stage, t.root);
    let end = schedule.iter().position(|&stmt| stmt == t.owner).unwrap() + 1;
    assert_eq!(visited, schedule[..end]);
}

#[test]
fn interrupt_ends_the_walk_after_the_visited_statement() {
    let t = tree();
    check_interrupts::<PreOrder>(&t);
    check_interrupts::<PostOrder>(&t);
    check_interrupts::<BFS>(&t);
    check_interrupts::<ReverseBFS>(&t);
    check_interrupts::<KeepOrderReverseBFS>(&t);
}

/// Walk `O` with `walk_mut`, inserting a statement into `t.owner`'s first
/// body while visiting it and skipping it; the statements visited.
fn skip_after_inserting<O: WalkOrder>() -> (Tree, Statement, Vec<Statement>) {
    let mut t = tree();
    let (root, owner, then_ret) = (t.root, t.owner, t.then_ret);
    let mut visited = Vec::new();
    let mut inserted = None;
    let status = O::walk_mut(&mut Rewriter::new(&mut t.stage), root, |rewriter, stmt| {
        visited.push(stmt);
        if stmt != owner {
            return WalkResult::Advance;
        }
        inserted = Some(
            rewriter
                .insert_before(then_ret, BuilderDialect::Nop)
                .unwrap(),
        );
        WalkResult::Skip
    })
    .unwrap();
    assert_eq!(status, WalkStatus::Completed);
    (t, inserted.unwrap(), visited)
}

#[test]
fn skip_leaves_out_children_inserted_while_visiting() {
    let (t, inserted, visited) = skip_after_inserting::<PreOrder>();
    assert_eq!(visited, [t.root, t.a, t.owner, t.ret]);
    assert!(!visited.contains(&inserted));
    let (t, inserted, visited) = skip_after_inserting::<BFS>();
    assert_eq!(visited, [t.root, t.a, t.owner, t.ret]);
    assert!(!visited.contains(&inserted));
}

#[test]
fn walk_mut_fails_when_the_visited_statement_is_erased() {
    let mut t = tree();
    let (root, a) = (t.root, t.a);
    let mut rewriter = Rewriter::new(&mut t.stage);
    let result = PreOrder::walk_mut(&mut rewriter, root, |rewriter, stmt| {
        if stmt == a {
            rewriter.erase_statement(a).unwrap();
        }
        WalkResult::Advance
    });
    assert_eq!(result, Err(WalkError::VisitedErased(a)));
}

#[test]
fn skip_before_the_first_step_has_no_effect() {
    let t = tree();
    let mut cursor = PreOrder::start(&t.stage, t.root);
    cursor.skip();
    let mut visited = Vec::new();
    while let Some(stmt) = cursor.next(&t.stage).unwrap() {
        visited.push(stmt);
    }
    assert_eq!(visited, PreOrder::schedule(&t.stage, t.root));
}

/// The ids of [`merge_cfg`]'s blocks and statements.
#[derive(Clone, Copy)]
struct MergeCfg {
    root: Statement,
    b1: Block,
    b2: Block,
    z: Statement,
    br: Statement,
}

/// ```text
/// root = own_cfg {
///   ^b2 { z; ret }
///   ^b3 { y; ret }
///   ^b1 { x; br ^b2 }
/// }
/// ```
fn merge_cfg() -> (StageInfo<BuilderDialect>, MergeCfg) {
    let mut stage = new_stage();
    let z = stage.statement().definition(BuilderDialect::Nop).new();
    let b2_ret = stage.statement().definition(BuilderDialect::Return).new();
    let b2 = stage.block().stmt(z).terminator(b2_ret).new();
    let y = stage.statement().definition(BuilderDialect::Nop).new();
    let b3_ret = stage.statement().definition(BuilderDialect::Return).new();
    let b3 = stage.block().stmt(y).terminator(b3_ret).new();
    let x = stage.statement().definition(BuilderDialect::Nop).new();
    let br = stage
        .statement()
        .definition(BuilderDialect::Branch(Successor::from_block(b2)))
        .new();
    let b1 = stage.block().stmt(x).terminator(br).new();
    let cfg = stage.cfg().add_block(b2).add_block(b3).add_block(b1).new();
    let root = stage
        .statement()
        .definition(BuilderDialect::OwnCFG(cfg))
        .new();
    let ids = MergeCfg {
        root,
        b1,
        b2,
        z,
        br,
    };
    (stage.finalize().unwrap(), ids)
}

/// Walk `O` over [`merge_cfg`], running `edit` when visiting `z`; the walk's
/// schedule before the edit, and the statements it visited.
fn edit_at_z<O: WalkOrder>(
    edit: impl Fn(&mut Rewriter<BuilderDialect>, MergeCfg),
) -> (Vec<Statement>, Result<Vec<Statement>, WalkError>) {
    let (mut stage, m) = merge_cfg();
    let schedule = O::schedule(&stage, m.root);
    let mut rewriter = Rewriter::new(&mut stage);
    let mut cursor = O::start(rewriter.stage(), m.root);
    let mut visited = Vec::new();
    let result = loop {
        match cursor.next(rewriter.stage()) {
            Ok(Some(stmt)) => {
                visited.push(stmt);
                if stmt == m.z {
                    edit(&mut rewriter, m);
                }
            }
            Ok(None) => break Ok(visited),
            Err(error) => break Err(error),
        }
    };
    (schedule, result)
}

fn merge_b2_into_b1(rewriter: &mut Rewriter<BuilderDialect>, m: MergeCfg) {
    rewriter.merge_blocks(m.b1, m.b2, &[]).unwrap();
}

#[test]
fn merging_the_visited_statements_block_away_breaks_link_following_walks() {
    let (_, m) = merge_cfg();
    for (_, result) in [
        edit_at_z::<PreOrder>(merge_b2_into_b1),
        edit_at_z::<PostOrder>(merge_b2_into_b1),
        edit_at_z::<BFS>(merge_b2_into_b1),
    ] {
        assert_eq!(result, Err(WalkError::VisitedMoved(m.z)));
    }
}

#[test]
fn reverse_bfs_orders_visit_statements_where_they_were_before_a_merge() {
    let (_, m) = merge_cfg();
    // `br` comes before `z` in `ReverseBFS`, so it is visited before the
    // merge erases it; in `KeepOrderReverseBFS` it comes after and is skipped.
    let (schedule, result) = edit_at_z::<ReverseBFS>(merge_b2_into_b1);
    assert_eq!(result.unwrap(), schedule);

    let (schedule, result) = edit_at_z::<KeepOrderReverseBFS>(merge_b2_into_b1);
    let without_br: Vec<Statement> = schedule.into_iter().filter(|&s| s != m.br).collect();
    assert_eq!(result.unwrap(), without_br);
}

#[test]
fn splitting_the_visited_statements_block_keeps_the_walk() {
    fn split_at_z(rewriter: &mut Rewriter<BuilderDialect>, m: MergeCfg) {
        rewriter.split_block(m.z).unwrap();
    }
    for (schedule, result) in [
        edit_at_z::<PreOrder>(split_at_z),
        edit_at_z::<PostOrder>(split_at_z),
        edit_at_z::<BFS>(split_at_z),
    ] {
        assert_eq!(result.unwrap(), schedule);
    }
}
