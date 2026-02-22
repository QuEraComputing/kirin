//! Abstract interpretation tests using StackInterpreter with the Interval domain.

use kirin_arith::{ArithType, ArithValue};
use kirin_cf::ControlFlow;
use kirin_constant::Constant;
use kirin_function::FunctionBody;
use kirin_interpreter::{Continuation, Frame, StackInterpreter};
use kirin_ir::*;
use kirin_test_utils::Interval;
use kirin_test_utils::TestDialect;

/// Build `c1 = constant 10; c2 = constant 32; y = add c1, c2; return y`
/// Run with Interval values through StackInterpreter.
#[test]
fn test_session_abstract_interp_constants() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(10));
    let c2 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(32));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, c1.result, c2.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .stmt(c1)
        .stmt(c2)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    let mut interp: StackInterpreter<Interval, _> = StackInterpreter::new(&pipeline, stage_id);

    let result = interp.call::<TestDialect>(spec_fn, &[]).unwrap();
    assert_eq!(result, Interval::constant(42));
}

/// Build `y = add x, constant(1); return y` with a block argument x.
/// Run with various Interval inputs.
#[test]
fn test_session_abstract_interp_with_args() {
    let mut pipeline: Pipeline<StageInfo<TestDialect>> = Pipeline::new();
    let stage_id = pipeline.add_stage().stage(StageInfo::default()).new();
    let stage = pipeline.stage_mut(stage_id).unwrap();

    let sf = stage.staged_function().new().unwrap();
    let ba_x = stage.block_argument(0);
    let c1 = Constant::<ArithValue, ArithType>::new(stage, ArithValue::I64(1));
    let add = kirin_arith::Arith::<ArithType>::op_add(stage, SSAValue::from(ba_x), c1.result);
    let ret = ControlFlow::<ArithType>::op_return(stage, add.result);

    let block = stage
        .block()
        .argument(ArithType::I64)
        .stmt(c1)
        .stmt(add)
        .terminator(ret)
        .new();
    let region = stage.region().add_block(block).new();
    let body = FunctionBody::<ArithType>::new(stage, region);
    let spec_fn = stage.specialize().f(sf).body(body).new().unwrap();

    // Resolve entry info for manual frame setup
    let stage_info = pipeline.stage(stage_id).unwrap();
    let spec_info = spec_fn.expect_info(stage_info);
    let body_stmt = *spec_info.body();
    let regions: Vec<_> = body_stmt.regions::<TestDialect>(stage_info).collect();
    let blocks: Vec<_> = regions[0].blocks(stage_info).collect();
    let block_info = blocks[0].expect_info(stage_info);
    let first_stmt = block_info.statements.head().copied();
    let block_args: Vec<_> = block_info.arguments.iter().copied().collect();

    let call_with = |input: Interval| -> Interval {
        let mut interp: StackInterpreter<Interval, _> = StackInterpreter::new(&pipeline, stage_id);
        let mut frame = Frame::new(spec_fn, first_stmt);
        frame.write_ssa(SSAValue::from(block_args[0]), input);
        interp.push_call_frame(frame).unwrap();
        loop {
            match interp.run::<TestDialect>().unwrap() {
                Continuation::Return(v) => {
                    interp.pop_call_frame().unwrap();
                    return v;
                }
                _ => panic!("expected Return"),
            }
        }
    };

    // [10,20] + [1,1] = [11,21]
    assert_eq!(call_with(Interval::new(10, 20)), Interval::new(11, 21));
    // [0,0] + [1,1] = [1,1]
    assert_eq!(call_with(Interval::constant(0)), Interval::constant(1));
    // top + [1,1] = top
    assert_eq!(call_with(Interval::top()), Interval::top());
    // bottom + anything = bottom
    assert!(call_with(Interval::bottom_interval()).is_empty());
}
