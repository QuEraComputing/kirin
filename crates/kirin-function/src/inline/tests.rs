use std::collections::HashSet;

use kirin::prelude::*;
use kirin_arith::{Arith, ArithType};
use kirin_cf::ControlFlow;

use super::{FunctionOps, InlineError, inline_calls};
use crate::{Call, CallLike, Function, Return};

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
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(1));
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
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(1));
    drop(rw);

    // ^g(%x, %y): br ^e'
    // ^e': cond_br %x then=^t'(%x) else=^f'(%y)
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
        branch(e, vec![])
    );
    assert!(e.expect_info(&stage).arguments().is_empty());
    assert!(matches!(
        definition(&stage, e.terminator(&stage).unwrap()),
        TestLang::Cf(ControlFlow::ConditionalBranch { condition, true_args, false_args, .. })
            if *condition == x && *true_args == [x] && *false_args == [y]
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
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(2));
    let left = calls(rw.stage(), body);
    assert_eq!(left.len(), 1);
    assert!(matches!(
        definition(rw.stage(), left[0]).as_call(),
        Some(Call::Specialized(call)) if call.target() == f
    ));
    // A second run inlines it, and a third finds nothing left.
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(1));
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(0));
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
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(2));
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
    assert_eq!(inline_calls(&mut rw, body, branch), Ok(1));
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
fn calls_that_cannot_be_inlined_are_rejected_before_any_edit() {
    let mut b = Builder::default();
    let f = add_twice(&mut b);
    let h = choose(&mut b);
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
    // `@f` takes one argument, `calling` passes two.
    let (too_many_args, call_f) = calling(&mut b, f, 1);
    // `@h` returns one value, the call takes two.
    let (too_many_results, call_h) = calling(&mut b, h, 2);
    let (no_body, call_bodiless) = calling(&mut b, bodiless, 1);
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
    assert_eq!(
        inline_calls(&mut rw, no_body, branch),
        Err(InlineError::CalleeBodyNotCfg(call_bodiless))
    );
    assert!(rw.events().is_empty());
}
