//! Visit orders of the `walk` traversal orders over nested bodies.

mod common;

use common::{BuilderDialect, new_stage};
use kirin_ir::walk::{
    BFS, KeepOrderReverseBFS, PostOrder, PreOrder, ReverseBFS, WalkCursor, WalkError, WalkOrder,
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
fn erasing_the_visited_statement_breaks_the_walk() {
    for (a, result) in [
        erase_while_visiting::<PreOrder>(),
        erase_while_visiting::<PostOrder>(),
        erase_while_visiting::<BFS>(),
        erase_while_visiting::<ReverseBFS>(),
        erase_while_visiting::<KeepOrderReverseBFS>(),
    ] {
        assert_eq!(result, Err(WalkError::VisitedErased(a)));
    }
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
