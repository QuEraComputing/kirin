use std::collections::HashSet;

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;

use super::{FunctionOps, InlineError, InlineReport, NotInlined, inline_calls};
use crate::{Call, CallLike, Function, Lambda, Return};

/// A statement owning a block, like an `scf.if` arm or a loop body.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(builders, type = ArithType)]
struct Nest {
    body: Block,
}

/// A language wrapping this crate's function operations with branches and
/// arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(type = ArithType)]
enum TestLang {
    #[wraps]
    Function(Function<ArithType>),
    #[wraps]
    Call(Call<ArithType>),
    #[wraps]
    Return(Return<ArithType>),
    #[wraps]
    Cf(ControlFlow<ArithType>),
    #[wraps]
    Arith(Arith<ArithType>),
    #[wraps]
    Lambda(Lambda<ArithType>),
    #[wraps]
    Nest(Nest),
}

impl FunctionOps<ArithType> for TestLang {
    fn as_call(&self) -> Option<&Call<ArithType>> {
        match self {
            TestLang::Call(call) => Some(call),
            _ => None,
        }
    }

    fn as_return(&self) -> Option<&Return<ArithType>> {
        match self {
            TestLang::Return(ret) => Some(ret),
            _ => None,
        }
    }
}

fn branch(target: Block, args: Vec<SSAValue>) -> TestLang {
    TestLang::Cf(ControlFlow::Branch {
        target: Successor::from_block(target),
        args,
    })
}

type Builder = BuilderStageInfo<TestLang>;

fn signature(arity: usize) -> Signature<ArithType> {
    Signature::new(vec![ArithType::I64; arity], ArithType::I64, ())
}

/// Make `definition` the only specialization of a new staged function.
fn specialize(b: &mut Builder, definition: Statement, arity: usize) -> SpecializedFunction {
    let staged = b
        .staged_function()
        .signature(signature(arity))
        .new()
        .unwrap();
    b.specialize()
        .staged_func(staged)
        .definition(definition)
        .new()
        .unwrap()
}

/// A function owning `body`, made a specialization.
fn define(b: &mut Builder, body: CFG, arity: usize) -> SpecializedFunction {
    let definition = Function::<ArithType>::new(b, body, signature(arity));
    specialize(b, definition.into(), arity)
}

/// `fn @f(%p) { ^e(%p): %s = add %p, %p; ret %s }`
fn add_twice(b: &mut Builder) -> SpecializedFunction {
    let p = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(b, p, p);
    let ret = Return::<ArithType>::new(b, vec![add.result.into()]);
    let entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(add)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(entry).new();
    define(b, body, 1)
}

/// `fn @h(%p, %q) { ^e(%p, %q): cond_br %p then=^t(%p) else=^f(%q);
/// ^t(%a): ret %a; ^f(%b): ret %b }`
fn choose(b: &mut Builder) -> SpecializedFunction {
    let a = b.block_argument().index(0);
    let ret_a = Return::<ArithType>::new(b, vec![a]);
    let t = b.block().argument(ArithType::I64).terminator(ret_a).new();
    let bb = b.block_argument().index(0);
    let ret_b = Return::<ArithType>::new(b, vec![bb]);
    let f = b.block().argument(ArithType::I64).terminator(ret_b).new();
    let p = b.block_argument().index(0);
    let q = b.block_argument().index(1);
    let cond_br = b
        .statement()
        .definition(TestLang::Cf(ControlFlow::ConditionalBranch {
            condition: p,
            true_target: Successor::from_block(t),
            true_args: vec![p],
            false_target: Successor::from_block(f),
            false_args: vec![q],
        }))
        .new();
    let entry = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .terminator(cond_br)
        .new();
    let body = b.cfg().add_block(entry).add_block(t).add_block(f).new();
    define(b, body, 2)
}

/// The report of a run that inlined calls to `callees` and skipped none.
fn inlined<const N: usize>(callees: [SpecializedFunction; N]) -> Result<InlineReport, InlineError> {
    Ok(InlineReport {
        inlined: HashSet::from(callees),
        skipped: vec![],
    })
}

/// The report of a run that inlined nothing and skipped one call, for `reason`.
fn skipped(reason: NotInlined) -> Result<InlineReport, InlineError> {
    Ok(InlineReport {
        inlined: HashSet::new(),
        skipped: vec![reason],
    })
}

fn definition(stage: &StageInfo<TestLang>, stmt: Statement) -> &TestLang {
    stmt.expect_info(stage).definition()
}

fn arg(stage: &StageInfo<TestLang>, block: Block, index: usize) -> SSAValue {
    block.expect_info(stage).arguments()[index].into()
}

fn result(stage: &StageInfo<TestLang>, stmt: Statement) -> SSAValue {
    (*definition(stage, stmt).results().next().unwrap()).into()
}

/// Every call left anywhere in `body`'s blocks.
fn calls(stage: &StageInfo<TestLang>, body: CFG) -> Vec<Statement> {
    body.blocks(stage)
        .flat_map(|block| block.statements(stage).collect::<Vec<_>>())
        .filter(|&stmt| definition(stage, stmt).as_call().is_some())
        .collect()
}

#[test]
fn a_single_block_callee_is_inlined_in_place() {
    // ^g(%x): %c = call @f(%x); %d = add %c, %c; ret %d
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let x = b.block_argument().index(0);
    let call = Call::<ArithType>::build(&mut b)
        .specialized(f)
        .args(vec![x])
        .results(1)
        .insert();
    let c: SSAValue = call.results[0].into();
    let d = Arith::<ArithType>::op_add(&mut b, c, c);
    let d_id = d.id;
    let ret = Return::<ArithType>::new(&mut b, vec![d.result.into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(call)
        .stmt(d)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();
    let x = arg(&stage, g, 0);
    let d = d_id;

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([f]));
    drop(rw);

    // ^g(%x): %s = add %x, %x; %d = add %s, %s; ret %d
    let stmts: Vec<Statement> = g.statements(&stage).collect();
    assert_eq!(stmts.len(), 2);
    assert_eq!(stmts[1], d);
    let s = result(&stage, stmts[0]);
    assert!(matches!(
        definition(&stage, stmts[0]),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == x && *rhs == x
    ));
    assert!(matches!(
        definition(&stage, d),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == s && *rhs == s
    ));
    assert_eq!(body.blocks(&stage).count(), 1);
    verify_derived(&stage).unwrap();
}

/// `fn @id(%p) { ^e(%p): ret %p }`
fn identity(b: &mut Builder) -> SpecializedFunction {
    let p = b.block_argument().index(0);
    let ret = Return::<ArithType>::new(b, vec![p]);
    let entry = b.block().argument(ArithType::I64).terminator(ret).new();
    let body = b.cfg().add_block(entry).new();
    define(b, body, 1)
}

#[test]
fn a_chain_of_calls_rewrites_each_use_once() {
    // ^g(%x): %c1 = call @id(%x); ...; %c4 = call @id(%c3);
    //         %u1 = add %c4, %c4; ...; %u4 = add %c4, %c4; ret
    const N: usize = 4;
    let mut b = Builder::default();
    let id = identity(&mut b);
    let x = b.block_argument().index(0);
    let mut calls = Vec::new();
    let mut last = x;
    for _ in 0..N {
        let call = Call::<ArithType>::build(&mut b)
            .specialized(id)
            .args(vec![last])
            .results(1)
            .insert();
        last = call.results[0].into();
        calls.push(call);
    }
    let uses: Vec<_> = (0..N)
        .map(|_| Arith::<ArithType>::op_add(&mut b, last, last))
        .collect();
    let use_ids: Vec<Statement> = uses.iter().map(|add| add.id).collect();
    let ret = Return::<ArithType>::new(&mut b, vec![]);
    let mut g = b.block().argument(ArithType::I64);
    for call in calls {
        g = g.stmt(call);
    }
    for add in uses {
        g = g.stmt(add);
    }
    let g = g.terminator(ret).new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();
    let x = arg(&stage, g, 0);

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([id]));
    // Each use is rewritten once, straight to `%x`, not once per call.
    for &stmt in &use_ids {
        let rewrites = rw
            .events()
            .iter()
            .filter(|event| **event == MutationEvent::ChangedOperands { stmt })
            .count();
        assert_eq!(rewrites, 1, "{stmt:?} was rewritten {rewrites} times");
    }
    drop(rw);

    // ^g(%x): %u1 = add %x, %x; ...; %u4 = add %x, %x; ret
    assert_eq!(g.statements(&stage).collect::<Vec<_>>(), use_ids);
    for stmt in use_ids {
        assert!(matches!(
            definition(&stage, stmt),
            TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == x && *rhs == x
        ));
    }
    verify_derived(&stage).unwrap();
}

#[test]
fn an_id_call_on_a_multi_block_calls_result_reads_the_second_half() {
    // ^g(%x, %y): %c = call @h(%x, %y); %d = call @id(%c); %e = add %d, %d;
    //             ret %e
    let mut b = Builder::default();
    let h = choose(&mut b);
    let id = identity(&mut b);
    let x = b.block_argument().index(0);
    let y = b.block_argument().index(1);
    let first = Call::<ArithType>::build(&mut b)
        .specialized(h)
        .args(vec![x, y])
        .results(1)
        .insert();
    let c: SSAValue = first.results[0].into();
    let second = Call::<ArithType>::build(&mut b)
        .specialized(id)
        .args(vec![c])
        .results(1)
        .insert();
    let d: SSAValue = second.results[0].into();
    let e = Arith::<ArithType>::op_add(&mut b, d, d);
    let e_id = e.id;
    let ret = Return::<ArithType>::new(&mut b, vec![e.result.into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(first)
        .stmt(second)
        .stmt(e)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 2);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([id, h]));
    drop(rw);

    // `%d` is replaced by `%c`, and `%c` by the second half's argument.
    // ^g(%x, %y): br ^e'(%x, %y)   ... copies of @h's blocks ...
    // ^c(%r): %e = add %r, %r; ret %e
    let blocks: Vec<Block> = body.blocks(&stage).collect();
    let &[.., cont] = blocks.as_slice() else {
        unreachable!()
    };
    let r = arg(&stage, cont, 0);
    assert_eq!(cont.statements(&stage).collect::<Vec<_>>(), [e_id]);
    assert!(matches!(
        definition(&stage, e_id),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == r && *rhs == r
    ));
    assert!(calls(&stage, body).is_empty());
    verify_derived(&stage).unwrap();
}

#[test]
fn a_call_reading_an_earlier_blocks_inlined_call_reads_the_inlined_value() {
    // ^g(%x): %c = call @f(%x); br ^b
    // ^b: %d = call @f(%c); ret %d
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let x = b.block_argument().index(0);
    let first = Call::<ArithType>::build(&mut b)
        .specialized(f)
        .args(vec![x])
        .results(1)
        .insert();
    let c: SSAValue = first.results[0].into();
    let second = Call::<ArithType>::build(&mut b)
        .specialized(f)
        .args(vec![c])
        .results(1)
        .insert();
    let ret = Return::<ArithType>::new(&mut b, vec![second.results[0].into()]);
    let ret_id = ret.id;
    let bb = b.block().stmt(second).terminator(ret).new();
    let br = b.statement().definition(branch(bb, vec![])).new();
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(first)
        .terminator(br)
        .new();
    let body = b.cfg().add_block(g).add_block(bb).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();
    let x = arg(&stage, g, 0);

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([f]));
    drop(rw);

    // ^g(%x): %s = add %x, %x; br ^b
    // ^b: %t = add %s, %s; ret %t
    let [s_stmt] = g.statements(&stage).collect::<Vec<_>>()[..] else {
        panic!("expected one statement in ^g");
    };
    let s = result(&stage, s_stmt);
    assert!(matches!(
        definition(&stage, s_stmt),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == x && *rhs == x
    ));
    let [t_stmt] = bb.statements(&stage).collect::<Vec<_>>()[..] else {
        panic!("expected one statement in ^b");
    };
    let t = result(&stage, t_stmt);
    assert!(matches!(
        definition(&stage, t_stmt),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == s && *rhs == s
    ));
    assert_eq!(
        definition(&stage, ret_id)
            .arguments()
            .copied()
            .collect::<Vec<_>>(),
        [t]
    );
    verify_derived(&stage).unwrap();
}

#[test]
fn a_multi_block_callee_is_inlined_between_the_halves_of_the_block() {
    // ^g(%x, %y): %c = call @h(%x, %y); %d = add %c, %c; ret %d
    let mut b = Builder::default();
    let h = choose(&mut b);
    let x = b.block_argument().index(0);
    let y = b.block_argument().index(1);
    let call = Call::<ArithType>::build(&mut b)
        .specialized(h)
        .args(vec![x, y])
        .results(1)
        .insert();
    let c: SSAValue = call.results[0].into();
    let d = Arith::<ArithType>::op_add(&mut b, c, c);
    let d_id = d.id;
    let ret = Return::<ArithType>::new(&mut b, vec![d.result.into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(call)
        .stmt(d)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 2);
    let mut stage = b.finalize().unwrap();
    let (x, y) = (arg(&stage, g, 0), arg(&stage, g, 1));
    let d = d_id;

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([h]));
    drop(rw);

    // ^g(%x, %y): br ^e'(%x, %y)
    // ^e'(%p, %q): cond_br %p then=^t'(%p) else=^f'(%q)
    // ^t'(%a): br ^c(%a)     ^f'(%b): br ^c(%b)
    // ^c(%r): %d = add %r, %r; ret %d
    let blocks: Vec<Block> = body.blocks(&stage).collect();
    let [g2, e, t, f, cont] = blocks[..] else {
        panic!("expected five blocks, found {}", blocks.len());
    };
    assert_eq!(g2, g);
    assert!(g.statements(&stage).next().is_none());
    assert_eq!(
        *definition(&stage, g.terminator(&stage).unwrap()),
        branch(e, vec![x, y])
    );
    let (p, q) = (arg(&stage, e, 0), arg(&stage, e, 1));
    assert!(matches!(
        definition(&stage, e.terminator(&stage).unwrap()),
        TestLang::Cf(ControlFlow::ConditionalBranch { condition, true_args, false_args, .. })
            if *condition == p && *true_args == [p] && *false_args == [q]
    ));
    for block in [t, f] {
        assert_eq!(
            *definition(&stage, block.terminator(&stage).unwrap()),
            branch(cont, vec![arg(&stage, block, 0)])
        );
    }
    let r = arg(&stage, cont, 0);
    assert_eq!(cont.statements(&stage).collect::<Vec<_>>(), [d]);
    assert!(matches!(
        definition(&stage, d),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == r && *rhs == r
    ));
    let preds: HashSet<Block> = cont
        .expect_info(&stage)
        .predecessors()
        .iter()
        .copied()
        .collect();
    assert_eq!(preds, HashSet::from([t, f]));
    verify_derived(&stage).unwrap();
}

#[test]
fn a_callee_whose_entry_is_a_loop_header_keeps_its_entry_arguments() {
    // fn @l(%n) { ^e(%i): %j = add %i, %i; cond_br %j then=^b(%j) else=^x(%j);
    // ^b(%k): br ^e(%k); ^x(%r): ret %r }
    let mut b = Builder::default();
    let r = b.block_argument().index(0);
    let ret_r = Return::<ArithType>::new(&mut b, vec![r]);
    let exit = b.block().argument(ArithType::I64).terminator(ret_r).new();
    // `^b`'s branch back to `^e` is created below, once `^e` exists.
    let back = b.block().argument(ArithType::I64).new();
    let i = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(&mut b, i, i);
    let j: SSAValue = add.result.into();
    let cond_br = b
        .statement()
        .definition(TestLang::Cf(ControlFlow::ConditionalBranch {
            condition: j,
            true_target: Successor::from_block(back),
            true_args: vec![j],
            false_target: Successor::from_block(exit),
            false_args: vec![j],
        }))
        .new();
    let entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(add)
        .terminator(cond_br)
        .new();
    let l_body = b
        .cfg()
        .add_block(entry)
        .add_block(back)
        .add_block(exit)
        .new();
    let l = define(&mut b, l_body, 1);
    // ^g(%x): %c = call @l(%x); ret %c
    let x = b.block_argument().index(0);
    let call = Call::<ArithType>::build(&mut b)
        .specialized(l)
        .args(vec![x])
        .results(1)
        .insert();
    let ret = Return::<ArithType>::new(&mut b, vec![call.results[0].into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(call)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();
    let x = arg(&stage, g, 0);

    let mut rw = Rewriter::new(&mut stage);
    let k = arg(rw.stage(), back, 0);
    rw.create(InsertionPoint::Terminator(back), branch(entry, vec![k]))
        .unwrap();
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([l]));
    drop(rw);

    // ^g(%x): br ^e'(%x)
    // ^e'(%i'): %j' = add %i', %i'; cond_br %j' then=^b'(%j') else=^x'(%j')
    // ^b'(%k'): br ^e'(%k')     ^x'(%r'): br ^c(%r')
    // ^c(%c'): ret %c'
    let blocks: Vec<Block> = body.blocks(&stage).collect();
    let [_, e2, b2, _, _] = blocks[..] else {
        panic!("expected five blocks, found {}", blocks.len());
    };
    assert_eq!(
        *definition(&stage, g.terminator(&stage).unwrap()),
        branch(e2, vec![x])
    );
    assert_eq!(
        *definition(&stage, b2.terminator(&stage).unwrap()),
        branch(e2, vec![arg(&stage, b2, 0)])
    );
    let preds: HashSet<Block> = e2
        .expect_info(&stage)
        .predecessors()
        .iter()
        .copied()
        .collect();
    assert_eq!(preds, HashSet::from([g, b2]));
    verify_derived(&stage).unwrap();
}

#[test]
fn calls_the_inlined_code_brings_in_are_left_for_another_run() {
    // fn @k(%p) { ^e(%p): %c = call @f(%p); ret %c }
    // ^g(%x): %c = call @k(%x); %d = call @h(%c, %x); ret %d
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let h = choose(&mut b);
    let p = b.block_argument().index(0);
    let inner = Call::<ArithType>::build(&mut b)
        .specialized(f)
        .args(vec![p])
        .results(1)
        .insert();
    let ret = Return::<ArithType>::new(&mut b, vec![inner.results[0].into()]);
    let k_entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(inner)
        .terminator(ret)
        .new();
    let k_body = b.cfg().add_block(k_entry).new();
    let k = define(&mut b, k_body, 1);
    let x = b.block_argument().index(0);
    let first = Call::<ArithType>::build(&mut b)
        .specialized(k)
        .args(vec![x])
        .results(1)
        .insert();
    let second = Call::<ArithType>::build(&mut b)
        .specialized(h)
        .args(vec![first.results[0].into(), x])
        .results(1)
        .insert();
    let ret = Return::<ArithType>::new(&mut b, vec![second.results[0].into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(first)
        .stmt(second)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    // One level: `@k` and `@h` are inlined, the call to `@f` `@k` brings in is
    // left.
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([h, k]));
    let left = calls(rw.stage(), body);
    assert_eq!(left.len(), 1);
    assert!(matches!(
        definition(rw.stage(), left[0]).as_call(),
        Some(Call::Specialized(call)) if call.target() == f
    ));
    // A second run inlines it, and a third finds nothing left.
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([f]));
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([]));
    drop(rw);

    assert!(calls(&stage, body).is_empty());
    // `@k`'s body, with `@f`'s inside it, is in ^g before the split.
    let x = arg(&stage, g, 0);
    let stmts: Vec<Statement> = g.statements(&stage).collect();
    assert_eq!(stmts.len(), 1);
    assert!(matches!(
        definition(&stage, stmts[0]),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == x && *rhs == x
    ));
    verify_derived(&stage).unwrap();
}

#[test]
fn several_multi_block_calls_in_one_block_are_all_inlined() {
    // ^g(%x, %y): %c1 = call @h(%x, %y); %c2 = call @h(%c1, %y);
    //             %d = add %c1, %c2; ret %d
    let mut b = Builder::default();
    let h = choose(&mut b);
    let x = b.block_argument().index(0);
    let y = b.block_argument().index(1);
    let c1 = Call::<ArithType>::build(&mut b)
        .specialized(h)
        .args(vec![x, y])
        .results(1)
        .insert();
    let c2 = Call::<ArithType>::build(&mut b)
        .specialized(h)
        .args(vec![c1.results[0].into(), y])
        .results(1)
        .insert();
    let d = Arith::<ArithType>::op_add(
        &mut b,
        SSAValue::from(c1.results[0]),
        SSAValue::from(c2.results[0]),
    );
    let ret = Return::<ArithType>::new(&mut b, vec![d.result.into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(c1)
        .stmt(c2)
        .stmt(d)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 2);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([h]));
    drop(rw);

    assert!(calls(&stage, body).is_empty());
    // ^g, two copies of @h's three blocks, and a second half after each call.
    assert_eq!(body.blocks(&stage).count(), 1 + 2 * 3 + 2);
    verify_derived(&stage).unwrap();
}

#[test]
fn a_call_with_several_results_takes_every_returned_value() {
    // fn @m(%p) { ^e(%p): %s = add %p, %p; ret %s, %p }
    // ^g(%x): %c0, %c1 = call @m(%x); %d = add %c0, %c1; ret %d
    let mut b = Builder::default();
    let p = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(&mut b, p, p);
    let ret = Return::<ArithType>::new(&mut b, vec![add.result.into(), p]);
    let entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(add)
        .terminator(ret)
        .new();
    let m_body = b.cfg().add_block(entry).new();
    let m = define(&mut b, m_body, 1);
    let x = b.block_argument().index(0);
    let call = Call::<ArithType>::build(&mut b)
        .specialized(m)
        .args(vec![x])
        .results(2)
        .insert();
    let d = Arith::<ArithType>::op_add(
        &mut b,
        SSAValue::from(call.results[0]),
        SSAValue::from(call.results[1]),
    );
    let d_id = d.id;
    let ret = Return::<ArithType>::new(&mut b, vec![d.result.into()]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(call)
        .stmt(d)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(g).new();
    define(&mut b, body, 1);
    let mut stage = b.finalize().unwrap();
    let x = arg(&stage, g, 0);
    let d = d_id;

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, body, branch), inlined([m]));
    drop(rw);

    let s = result(&stage, g.statements(&stage).next().unwrap());
    assert!(matches!(
        definition(&stage, d),
        TestLang::Arith(Arith::Add { lhs, rhs, .. }) if *lhs == s && *rhs == x
    ));
    verify_derived(&stage).unwrap();
}

/// A block calling `target` with `args` and `results`, in a body of its own.
fn calling(b: &mut Builder, target: SpecializedFunction, results: usize) -> (CFG, Statement) {
    let x = b.block_argument().index(0);
    let call = Call::<ArithType>::build(b)
        .specialized(target)
        .args(vec![x, x])
        .results(results)
        .insert();
    let id = call.id;
    let ret = Return::<ArithType>::new(b, vec![]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(call)
        .terminator(ret)
        .new();
    (b.cfg().add_block(g).new(), id)
}

#[test]
fn calls_that_do_not_match_their_callee_are_rejected_before_any_edit() {
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let h = choose(&mut b);
    // `@f` takes one argument, `calling` passes two.
    let (too_many_args, call_f) = calling(&mut b, f, 1);
    // `@h` returns one value, the call takes two.
    let (too_many_results, call_h) = calling(&mut b, h, 2);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        inline_calls(&mut rw, too_many_args, branch),
        Err(InlineError::ArgumentCountMismatch {
            call: call_f,
            parameters: 1,
            arguments: 2
        })
    );
    assert_eq!(
        inline_calls(&mut rw, too_many_results, branch),
        Err(InlineError::ReturnCountMismatch {
            call: call_h,
            results: 2,
            returned: 1
        })
    );
    assert!(rw.events().is_empty());
}

#[test]
fn calls_to_a_callee_with_no_cfg_body_or_an_empty_one_are_left_untouched() {
    let mut b = Builder::default();
    // A specialization whose definition has no body.
    let x = b.block_argument().index(0);
    let plain = Arith::<ArithType>::op_add(&mut b, x, x);
    let plain_id = plain.id;
    let ret = Return::<ArithType>::new(&mut b, vec![]);
    let _holder = b
        .block()
        .argument(ArithType::I64)
        .stmt(plain)
        .terminator(ret)
        .new();
    let bodiless = specialize(&mut b, plain_id, 1);
    // A function whose body is a CFG with no blocks.
    let empty_body = b.cfg().new();
    let empty = define(&mut b, empty_body, 1);
    let (no_body, call_bodiless) = calling(&mut b, bodiless, 1);
    let (empty_callee, call_empty) = calling(&mut b, empty, 1);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        inline_calls(&mut rw, no_body, branch),
        skipped(NotInlined::CalleeBodyNotCfg(call_bodiless))
    );
    assert_eq!(
        inline_calls(&mut rw, empty_callee, branch),
        skipped(NotInlined::EmptyCalleeBody(call_empty))
    );
    assert!(rw.events().is_empty());
}

/// `nest { ret %v }`, and the return in it.
fn nest_returning(b: &mut Builder, v: SSAValue) -> (Statement, Statement) {
    let ret = Return::<ArithType>::new(b, vec![v]);
    let ret_id: Statement = ret.into();
    let body = b.block().terminator(ret_id).new();
    let nest = b
        .statement()
        .definition(TestLang::Nest(Nest { body }))
        .new();
    (nest, ret_id)
}

#[test]
fn calls_to_a_callee_with_a_return_nested_in_a_body_are_left_untouched() {
    let mut b = Builder::default();
    // fn @f(%p, %q) { ^e(%p, %q): %s = add %p, %p; nest { ret %s }; ret %p }:
    // one block, one top-level return, so it would be inlined in place.
    let p = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(&mut b, p, p);
    let (nest, in_place_ret) = nest_returning(&mut b, add.result.into());
    let ret = Return::<ArithType>::new(&mut b, vec![p]);
    let entry = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(add)
        .stmt(nest)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(entry).new();
    let in_place = define(&mut b, body, 2);
    // fn @h(%p, %q) { ^e(%p, %q): %s = add %p, %p; nest { ret %s }; br ^t(%q);
    // ^t(%a): ret %a }
    let a = b.block_argument().index(0);
    let ret_a = Return::<ArithType>::new(&mut b, vec![a]);
    let t = b.block().argument(ArithType::I64).terminator(ret_a).new();
    let p = b.block_argument().index(0);
    let q = b.block_argument().index(1);
    let add = Arith::<ArithType>::op_add(&mut b, p, p);
    let (nest, blocks_ret) = nest_returning(&mut b, add.result.into());
    let br = b.statement().definition(branch(t, vec![q])).new();
    let entry = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(add)
        .stmt(nest)
        .terminator(br)
        .new();
    let body = b.cfg().add_block(entry).add_block(t).new();
    let as_blocks = define(&mut b, body, 2);
    let (calls_in_place, call_in_place) = calling(&mut b, in_place, 1);
    let (calls_as_blocks, call_as_blocks) = calling(&mut b, as_blocks, 1);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        inline_calls(&mut rw, calls_in_place, branch),
        skipped(NotInlined::NestedReturn {
            call: call_in_place,
            ret: in_place_ret
        })
    );
    assert_eq!(
        inline_calls(&mut rw, calls_as_blocks, branch),
        skipped(NotInlined::NestedReturn {
            call: call_as_blocks,
            ret: blocks_ret
        })
    );
    assert!(rw.events().is_empty());
}

#[test]
fn a_return_nested_in_a_lambda_of_the_callee_is_copied_as_is() {
    // fn @f(%p, %q) { ^e(%p, %q):
    //   %l = lambda { ^b(%y): %z = add %y, %y; nest { ret %z }; ret %y };
    //   ret %p }
    let mut b = Builder::default();
    let y = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(&mut b, y, y);
    let (nest, _) = nest_returning(&mut b, add.result.into());
    let ret_y = Return::<ArithType>::new(&mut b, vec![y]);
    let lambda_entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(add)
        .stmt(nest)
        .terminator(ret_y)
        .new();
    let lambda_body = b.cfg().add_block(lambda_entry).new();
    let name = b.symbol_table_mut().intern("l".to_string());
    let lambda = Lambda::<ArithType>::new(&mut b, name, vec![], lambda_body);
    let p = b.block_argument().index(0);
    let ret = Return::<ArithType>::new(&mut b, vec![p]);
    let entry = b
        .block()
        .argument(ArithType::I64)
        .argument(ArithType::I64)
        .stmt(lambda)
        .terminator(ret)
        .new();
    let body = b.cfg().add_block(entry).new();
    let f = define(&mut b, body, 2);
    let (caller, _) = calling(&mut b, f, 1);
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(inline_calls(&mut rw, caller, branch), inlined([f]));
    drop(rw);

    // ^g(%x): %l = lambda { ^b(%y): %z = add %y, %y; nest { ret %z }; ret %y }; ret
    let g = caller.blocks(&stage).next().unwrap();
    let stmts: Vec<Statement> = g.statements(&stage).collect();
    assert_eq!(stmts.len(), 1);
    let TestLang::Lambda(copy) = definition(&stage, stmts[0]) else {
        panic!("the lambda is copied in front of the call");
    };
    let Some(Body::CFG(copy_body)) = copy.callable_body() else {
        panic!("the copied lambda has a CFG body");
    };
    let copy_entry = copy_body.blocks(&stage).next().unwrap();
    let nest_copy = copy_entry.statements(&stage).nth(1).unwrap();
    let TestLang::Nest(Nest { body: nest_body }) = definition(&stage, nest_copy) else {
        panic!("the lambda's nest is copied");
    };
    let nested_ret = nest_body.terminator(&stage).unwrap();
    assert!(definition(&stage, nested_ret).as_return().is_some());
    verify_derived(&stage).unwrap();
}

/// `^g(%x): %first = call @first(..); %second = call @second(..); ret`, where
/// each call passes `%x` as many times as its `args` says. Returns the CFG,
/// `^g`, and the two calls.
fn two_calls(
    b: &mut Builder,
    first: (SpecializedFunction, usize),
    second: (SpecializedFunction, usize),
) -> (CFG, Block, Statement, Statement) {
    let x = b.block_argument().index(0);
    let call = |b: &mut Builder, (target, args): (SpecializedFunction, usize)| {
        Call::<ArithType>::build(b)
            .specialized(target)
            .args(vec![x; args])
            .results(1)
            .insert()
    };
    let first = call(b, first);
    let second = call(b, second);
    let (first_id, second_id) = (first.id, second.id);
    let ret = Return::<ArithType>::new(b, vec![]);
    let g = b
        .block()
        .argument(ArithType::I64)
        .stmt(first)
        .stmt(second)
        .terminator(ret)
        .new();
    (b.cfg().add_block(g).new(), g, first_id, second_id)
}

#[test]
fn a_call_left_as_a_call_does_not_stop_the_others_being_inlined() {
    // fn @n(%p) { ^e(%p): %s = add %p, %p; nest { ret %s }; ret %p }
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let p = b.block_argument().index(0);
    let add = Arith::<ArithType>::op_add(&mut b, p, p);
    let (nest, nested_ret) = nest_returning(&mut b, add.result.into());
    let ret = Return::<ArithType>::new(&mut b, vec![p]);
    let entry = b
        .block()
        .argument(ArithType::I64)
        .stmt(add)
        .stmt(nest)
        .terminator(ret)
        .new();
    let n_body = b.cfg().add_block(entry).new();
    let n = define(&mut b, n_body, 1);
    // ^g(%x): %a = call @f(%x); %b = call @n(%x); ret
    let (body, g, _, call_n) = two_calls(&mut b, (f, 1), (n, 1));
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        inline_calls(&mut rw, body, branch),
        Ok(InlineReport {
            inlined: HashSet::from([f]),
            skipped: vec![NotInlined::NestedReturn {
                call: call_n,
                ret: nested_ret
            }],
        })
    );
    drop(rw);

    // ^g(%x): %s = add %x, %x; %b = call @n(%x); ret
    let stmts: Vec<Statement> = g.statements(&stage).collect();
    assert_eq!(stmts.len(), 2);
    assert!(matches!(
        definition(&stage, stmts[0]),
        TestLang::Arith(Arith::Add { .. })
    ));
    assert_eq!(calls(&stage, body), vec![call_n]);
}

#[test]
fn a_call_that_does_not_match_its_callee_stops_the_run_before_any_edit() {
    // ^g(%x): %a = call @f(%x, %x); %b = call @f(%x); ret. The calls are
    // inlined last to first, so `%b`, which matches, comes before `%a`.
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let (body, _, call_a, _) = two_calls(&mut b, (f, 2), (f, 1));
    let mut stage = b.finalize().unwrap();

    let mut rw = Rewriter::new(&mut stage);
    assert_eq!(
        inline_calls(&mut rw, body, branch),
        Err(InlineError::ArgumentCountMismatch {
            call: call_a,
            parameters: 1,
            arguments: 2
        })
    );
    assert!(rw.events().is_empty());
}
