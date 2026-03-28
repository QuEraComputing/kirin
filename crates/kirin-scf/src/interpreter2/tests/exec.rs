use kirin::ir;
use kirin::prelude::*;
use kirin_arith::{ArithType, ArithValue};
use kirin_constant::Constant;
use kirin_function::{FunctionBody, Return};
use kirin_interpreter_2::{
    ConsumeEffect, Cursor, Interpretable, InterpreterError, Lift, Machine, ValueStore,
    control::Shell, interpreter::SingleStage,
};

use crate::{StructuredControlFlow, Yield};

// ---------------------------------------------------------------------------
// Test effect & machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Replace will be used by If/For tests
enum TestEffect {
    Advance,
    Replace(ir::Block),
    Return(ArithValue),
}

impl Lift<TestEffect> for Cursor {
    fn lift(self) -> TestEffect {
        match self {
            Cursor::Advance => TestEffect::Advance,
            Cursor::Stay => TestEffect::Advance,
            Cursor::Jump(seed) => panic!("unexpected Jump in SCF test: {seed:?}"),
        }
    }
}

#[derive(Debug, Default)]
struct TestMachine;

impl<'ir> Machine<'ir> for TestMachine {
    type Effect = TestEffect;
    type Stop = ArithValue;
}

impl<'ir> ConsumeEffect<'ir> for TestMachine {
    type Error = InterpreterError;

    fn consume_effect(&mut self, effect: Self::Effect) -> Result<Shell<Self::Stop>, Self::Error> {
        Ok(match effect {
            TestEffect::Advance => Shell::Advance,
            TestEffect::Replace(block) => Shell::Replace(block.into()),
            TestEffect::Return(value) => Shell::Stop(value),
        })
    }
}

// ---------------------------------------------------------------------------
// Test language — wraps SCF + supporting ops for building programs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[wraps]
#[kirin(builders, type = ArithType)]
enum ScfTestLang {
    Scf(StructuredControlFlow<ArithType>),
    Constant(Constant<ArithValue, ArithType>),
    FunctionBody(FunctionBody<ArithType>),
    #[kirin(terminator)]
    Return(Return<ArithType>),
}

type TestInterp<'ir> = SingleStage<'ir, ScfTestLang, ArithValue, TestMachine, InterpreterError>;

// ---------------------------------------------------------------------------
// Interpretable impl — single dispatch for the whole language
// ---------------------------------------------------------------------------

fn unsupported(message: &'static str) -> InterpreterError {
    InterpreterError::custom(std::io::Error::other(message))
}

impl<'ir> Interpretable<'ir, TestInterp<'ir>> for ScfTestLang {
    type Effect = TestEffect;
    type Error = InterpreterError;

    fn interpret(&self, interp: &mut TestInterp<'ir>) -> Result<TestEffect, Self::Error> {
        match self {
            ScfTestLang::Scf(scf) => match scf {
                StructuredControlFlow::If(_) => {
                    Err(unsupported("scf.if not yet implemented in test harness"))
                }
                StructuredControlFlow::For(_) => {
                    Err(unsupported("scf.for not yet implemented in test harness"))
                }
                StructuredControlFlow::Yield(op) => {
                    // Delegate to the generic Yield impl which always errors
                    let result: Result<Cursor, InterpreterError> = op.interpret(interp);
                    result.map(Lift::lift)
                }
            },
            ScfTestLang::Constant(op) => {
                interp.write(op.result, op.value.clone())?;
                Ok(TestEffect::Advance)
            }
            ScfTestLang::FunctionBody(_) => Err(unsupported(
                "function bodies are structural and should not be stepped directly",
            )),
            ScfTestLang::Return(op) => {
                let values: Vec<_> = op.arguments().copied().collect();
                match values.as_slice() {
                    [value] => Ok(TestEffect::Return(interp.read(*value)?)),
                    [] => Err(unsupported("void return not supported in test")),
                    _ => Err(unsupported("multi-return not supported in test")),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Build a program where the entry block's terminator is a bare `yield`
/// (outside any scf.if or scf.for). Running it should error.
fn build_bare_yield_program(
    pipeline: &mut Pipeline<StageInfo<ScfTestLang>>,
    stage_id: CompileStage,
) -> SpecializedFunction {
    pipeline.stage_mut(stage_id).unwrap().with_builder(|b| {
        let sf = b.staged_function().new().unwrap();

        // Create a bare yield (no values, outside any scf body)
        let yield_op: Yield<ArithType> = Yield {
            values: vec![],
            marker: std::marker::PhantomData,
        };
        let yield_stmt = b
            .statement()
            .definition(ScfTestLang::Scf(StructuredControlFlow::Yield(yield_op)))
            .new();

        let block = b.block().new();
        {
            use ir::query::ParentInfo;
            let block_info: &mut Item<BlockInfo<ScfTestLang>> =
                b.block_arena_mut().get_mut(block).unwrap();
            block_info.terminator = Some(yield_stmt);

            let stmt_info = &mut b.statement_arena_mut()[yield_stmt];
            *stmt_info.get_parent_mut() = Some(StatementParent::Block(block));
        }

        let region = b.region().add_block(block).new();
        let body = FunctionBody::<ArithType>::new(
            b,
            region,
            Signature::new(vec![], ArithType::default(), ()),
        );
        b.specialize().staged_func(sf).body(body).new().unwrap()
    })
}

#[test]
fn yield_outside_scf_errors() {
    let mut pipeline: Pipeline<StageInfo<ScfTestLang>> = Pipeline::new();
    let stage_id: CompileStage = pipeline.add_stage().stage(StageInfo::default()).new();
    let spec_fn = build_bare_yield_program(&mut pipeline, stage_id);

    let mut interp = TestInterp::new(&pipeline, stage_id, TestMachine);
    let result = interp.run_specialization(spec_fn, &[]);

    assert!(result.is_err(), "yield outside scf.if/scf.for should error");
}
