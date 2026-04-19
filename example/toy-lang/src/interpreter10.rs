use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::{Block, CompileStage, HasStageInfo, Pipeline, SpecializedFunction};
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_function::interpreter10::interpret::eval_call_for_dialect;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_10::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_10::abstract_interp::AbstractInterp;
use kirin_interpreter_10::algebra::Lift;
use kirin_interpreter_10::call_dispatch::CallDispatch;
use kirin_interpreter_10::concrete::ConcreteInterp;
use kirin_interpreter_10::control::{Control, CursorExt};
use kirin_interpreter_10::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_10::env::{AbstractMode, ConcreteMode, Env};
use kirin_interpreter_10::error::InterpreterError;
use kirin_interpreter_10::execute::Execute;
use kirin_interpreter_10::interpretable::Interpretable;
use kirin_interpreter_10::pipeline::entry_block_of;
use kirin_scf::ForLoopValue;
use kirin_scf::StructuredControlFlow;
use kirin_scf::interpreter10::cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};
use kirin_scf::interpreter10::interpret::{
    eval_for_abstract, eval_for_concrete, eval_if_abstract, eval_if_concrete,
};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

// ---------------------------------------------------------------------------
// HighLevelCursor — concrete mode cursor coproduct for HighLevel dialect
// ---------------------------------------------------------------------------

pub enum HighLevelCursor<V: Clone> {
    Block(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
}

impl<V: Clone> Lift<HighLevelCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Block(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for SCFCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(self)
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>>
    for kirin_scf::interpreter10::cursor::IfCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>>
    for kirin_scf::interpreter10::cursor::ForCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::For(self))
    }
}

// TODO: replace this with derive macro
impl<E, V> Execute<E> for HighLevelCursor<V>
where
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: Env<Mode = ConcreteMode<HighLevelCursor<V>>, Value = V, Ext = CursorExt<HighLevelCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<HighLevelCursor<V>>>, E::Error> {
        match self {
            HighLevelCursor::Block(c) => c.execute(env, inbox),
            HighLevelCursor::Scf(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<ConcreteInterp<...>> — concrete mode
// ---------------------------------------------------------------------------

// TODO: replace this with derive macro
impl<'ir, V> Interpretable<ConcreteInterp<'ir, Stage, HighLevel, V, HighLevelCursor<V>>>
    for HighLevel
where
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
{
    fn eval(
        &self,
        env: &mut ConcreteInterp<'ir, Stage, HighLevel, V, HighLevelCursor<V>>,
    ) -> Result<Control<V, CursorExt<HighLevelCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => {
                    eval_call_for_dialect::<_, HighLevel, _>(op, env)
                }
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                StructuredControlFlow::If(op) => {
                    eval_if_concrete::<_, HighLevelCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::For(op) => {
                    eval_for_concrete::<_, HighLevelCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevelAbstractCursor — NEW in interpreter-10: abstract mode SCF support
//
// In interpreter-9, abstract interpretation of HighLevel (with SCF) was not
// possible because AbstractIfCursor/AbstractForCursor used BlockCursor (the
// concrete cursor type). interpreter-10 fixes this: they now use
// AbstractBlockCursor, enabling a proper abstract cursor coproduct.
// ---------------------------------------------------------------------------

pub enum HighLevelAbstractCursor<V: Clone> {
    Block(AbstractBlockCursor<V, HighLevel>),
    Scf(AbstractSCFCursor<V, HighLevel>),
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractBlockCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Block(self)
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractSCFCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(self)
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>>
    for kirin_scf::interpreter10::cursor::AbstractIfCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>>
    for kirin_scf::interpreter10::cursor::AbstractForCursor<V, HighLevel>
{
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::For(self))
    }
}

// TODO: replace this with derive macro
impl<E, V> Execute<E> for HighLevelAbstractCursor<V>
where
    V: Clone
        + AbstractValue
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: kirin_interpreter_10::env::AbstractEnv<
            Value = V,
            Ext = CursorExt<HighLevelAbstractCursor<V>>,
        >,
    E: Env<Mode = AbstractMode<HighLevelAbstractCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<HighLevelAbstractCursor<V>>>, E::Error> {
        match self {
            HighLevelAbstractCursor::Block(c) => c.execute(env, inbox),
            HighLevelAbstractCursor::Scf(c) => c.execute(env, inbox),
        }
    }
}

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<AbstractInterp<...>> — abstract mode (NEW)
// ---------------------------------------------------------------------------

// TODO: replace this with derive macro
impl<'ir, V> Interpretable<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>
    for HighLevel
where
    V: Clone
        + AbstractValue
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
{
    fn eval(
        &self,
        env: &mut AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>,
    ) -> Result<Control<V, CursorExt<HighLevelAbstractCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => {
                    eval_call_for_dialect::<_, HighLevel, _>(op, env)
                }
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                StructuredControlFlow::If(op) => {
                    eval_if_abstract::<_, HighLevelAbstractCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::For(op) => {
                    eval_for_abstract::<_, HighLevelAbstractCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable — single generic impl for both concrete and abstract
//
// interpreter-10 improvement: since all LowLevel sub-dialects have generic
// Interpretable<E: Env> impls, we can write a single impl that works for both
// ConcreteInterp and AbstractInterp without code duplication.
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for LowLevel
where
    E: Env<Value = V>,
    V: Clone
        + BranchCondition
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E::Stages: HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
{
    fn eval(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            LowLevel::Lifted(op) => match op {
                kirin_function::Lifted::FunctionBody(op) => op.eval(env),
                kirin_function::Lifted::Bind(op) => op.eval(env),
                kirin_function::Lifted::Call(op) => {
                    eval_call_for_dialect::<_, LowLevel, _>(op, env)
                }
                kirin_function::Lifted::Return(op) => op.eval(env),
            },
            LowLevel::Constant(op) => op.eval(env),
            LowLevel::Arith(op) => op.eval(env),
            LowLevel::Cmp(op) => op.eval(env),
            LowLevel::Bitwise(op) => op.eval(env),
            LowLevel::Cf(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// CallDispatch for HighLevelCursor — single-stage (source) concrete interpreter
//
// Implements CallDispatch so ConcreteInterp<Stage, HighLevel, V, HighLevelCursor<V>>
// can handle recursive calls within the source stage.
// ---------------------------------------------------------------------------

impl<V: Clone> CallDispatch<V, HighLevelCursor<V>> for Stage {
    fn make_call_cursor(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<HighLevelCursor<V>, InterpreterError> {
        let entry = entry_block_of::<Stage, HighLevel>(pipeline, callee, stage_id)?;
        Ok(HighLevelCursor::Block(BlockCursor::new(
            entry, stage_id, args,
        )))
    }
}

// ---------------------------------------------------------------------------
// MultiCursor — concrete cursor coproduct spanning both source and lowered stages
//
// Enables ConcreteInterp to evaluate programs whose call graph crosses stages:
// source-stage HighLevel ops may call lowered-stage LowLevel functions (and
// vice-versa). The cursor coproduct is the flat union of all stage-local cursors.
//
// BlockCursor<V, L> can Execute<E> for any E whose Mode = ConcreteMode<C> —
// C is a free type parameter in the Execute impl — so both HighLevel and
// LowLevel block cursors work within the same MultiCursor coproduct as long
// as the Interpretable and HasStageInfo bounds are satisfied on the interpreter.
// ---------------------------------------------------------------------------

pub enum MultiCursor<V: Clone> {
    High(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
    Low(BlockCursor<V, LowLevel>),
}

// -- Lift impls: dialect-local cursors → MultiCursor -------------------------

impl<V: Clone> Lift<MultiCursor<V>> for BlockCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::High(self)
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for IfCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for ForCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(SCFCursor::For(self))
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for SCFCursor<V, HighLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Scf(self)
    }
}

impl<V: Clone> Lift<MultiCursor<V>> for BlockCursor<V, LowLevel> {
    fn lift(self) -> MultiCursor<V> {
        MultiCursor::Low(self)
    }
}

// -- Execute<MultiInterp<V>> for MultiCursor<V> ------------------------------

// TODO: replace with derive macro
impl<E, V> Execute<E> for MultiCursor<V>
where
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: Env<Mode = ConcreteMode<MultiCursor<V>>, Value = V, Ext = CursorExt<MultiCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel> + HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
    LowLevel: Interpretable<E>,
    BlockCursor<V, HighLevel>: Lift<MultiCursor<V>>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, E::Error> {
        match self {
            MultiCursor::High(c) => c.execute(env, inbox),
            MultiCursor::Scf(c) => c.execute(env, inbox),
            MultiCursor::Low(c) => c.execute(env, inbox),
        }
    }
}

// -- CallDispatch for MultiCursor — multi-stage dispatch ---------------------

impl<V: Clone> CallDispatch<V, MultiCursor<V>> for Stage {
    fn make_call_cursor(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<MultiCursor<V>, InterpreterError> {
        let stage_container = pipeline
            .stage(stage_id)
            .ok_or(InterpreterError::MissingEntry)?;
        match stage_container {
            Stage::Source(_) => {
                let entry = entry_block_of::<Stage, HighLevel>(pipeline, callee, stage_id)?;
                Ok(MultiCursor::High(BlockCursor::new(entry, stage_id, args)))
            }
            Stage::Lowered(_) => {
                let entry = entry_block_of::<Stage, LowLevel>(pipeline, callee, stage_id)?;
                Ok(MultiCursor::Low(BlockCursor::new(entry, stage_id, args)))
            }
        }
    }
}

// -- HighLevel: Interpretable<MultiInterp<V>> — cross-stage call resolution --
//
// The single-stage HighLevel Interpretable resolves calls at the current stage.
// This multi-stage impl also tries the lowered stage as a fallback, enabling
// source code to call functions that only exist at lowered.
// ---------------------------------------------------------------------------

pub type MultiInterp<'ir, V> = ConcreteInterp<'ir, Stage, HighLevel, V, MultiCursor<V>>;

// TODO: replace with derive macro
impl<'ir, V> Interpretable<MultiInterp<'ir, V>> for HighLevel
where
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
{
    fn eval(
        &self,
        env: &mut MultiInterp<'ir, V>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => {
                    // Cross-stage call resolution: try the current (source) stage first,
                    // then fall back to lowered. This lets source-stage programs call
                    // functions that only exist at the lowered stage.
                    let args = env.read_many(op.args())?;
                    let target = op.target();
                    let current = env.current_stage();
                    if let Ok(callee) = env.resolve_function_for::<HighLevel>(target, current) {
                        Ok(Control::Call {
                            callee,
                            stage: current,
                            args,
                            results: op.results().to_vec(),
                        })
                    } else {
                        let lowered = env
                            .pipeline()
                            .stage_by_name("lowered")
                            .ok_or(InterpreterError::MissingEntry)?;
                        // The `target` symbol is from the source stage's symbol table;
                        // resolve_function_cross_stage looks it up via HighLevel (source)
                        // then finds the specialization in LowLevel (lowered).
                        let callee = env.resolve_function_cross_stage::<HighLevel, LowLevel>(
                            target, current, lowered,
                        )?;
                        Ok(Control::Call {
                            callee,
                            stage: lowered,
                            args,
                            results: op.results().to_vec(),
                        })
                    }
                }
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                StructuredControlFlow::If(op) => {
                    eval_if_concrete::<_, MultiCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::For(op) => {
                    eval_for_concrete::<_, MultiCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractCallDispatch — single-stage abstract interpreters
//
// Each cursor coproduct used by analyze_highlevel / analyze_lowered needs a
// corresponding AbstractCallDispatch impl so AbstractInterp can create the
// right cursor type when dispatching calls.
// ---------------------------------------------------------------------------

impl<V: Clone> AbstractCallDispatch<V, AbstractBlockCursor<V, LowLevel>> for Stage {
    fn make_abstract_cursor(
        _pipeline: &Pipeline<Stage>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> AbstractBlockCursor<V, LowLevel> {
        AbstractBlockCursor::new(block, stage_id, args)
    }

    fn entry_block_for(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        entry_block_of::<Stage, LowLevel>(pipeline, callee, stage_id)
    }
}

impl<V: Clone> AbstractCallDispatch<V, HighLevelAbstractCursor<V>> for Stage {
    fn make_abstract_cursor(
        _pipeline: &Pipeline<Stage>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Block(AbstractBlockCursor::new(block, stage_id, args))
    }

    fn entry_block_for(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        entry_block_of::<Stage, HighLevel>(pipeline, callee, stage_id)
    }
}

// ---------------------------------------------------------------------------
// AbstractMultiCursor — abstract cursor coproduct spanning source and lowered
//
// Mirrors MultiCursor for the abstract interpreter. Unlike HighLevelAbstractCursor
// which ties `E::Mode = AbstractMode<HighLevelAbstractCursor<V>>`, the constituent
// types (AbstractBlockCursor, AbstractSCFCursor) have `C` as a free type parameter
// in their Execute impls — so they can operate inside AbstractMultiCursor<V>
// without mode conflicts. This is the same free-C pattern as the concrete side.
// ---------------------------------------------------------------------------

pub enum AbstractMultiCursor<V: Clone> {
    HighBlock(AbstractBlockCursor<V, HighLevel>),
    HighScf(AbstractSCFCursor<V, HighLevel>),
    Low(AbstractBlockCursor<V, LowLevel>),
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractBlockCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighBlock(self)
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractSCFCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(self)
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractIfCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(AbstractSCFCursor::If(self))
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractForCursor<V, HighLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::HighScf(AbstractSCFCursor::For(self))
    }
}

impl<V: Clone> Lift<AbstractMultiCursor<V>> for AbstractBlockCursor<V, LowLevel> {
    fn lift(self) -> AbstractMultiCursor<V> {
        AbstractMultiCursor::Low(self)
    }
}

// TODO: replace with derive macro
impl<E, V> Execute<E> for AbstractMultiCursor<V>
where
    V: Clone
        + AbstractValue
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: kirin_interpreter_10::env::AbstractEnv<Value = V, Ext = CursorExt<AbstractMultiCursor<V>>>,
    E: Env<Mode = AbstractMode<AbstractMultiCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel> + HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
    LowLevel: Interpretable<E>,
    // Required by AbstractSCFCursor<V, HighLevel>: Execute<E> (push bodies as HighBlock)
    AbstractBlockCursor<V, HighLevel>: Lift<AbstractMultiCursor<V>>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, E::Error> {
        match self {
            AbstractMultiCursor::HighBlock(c) => c.execute(env, inbox),
            AbstractMultiCursor::HighScf(c) => c.execute(env, inbox),
            AbstractMultiCursor::Low(c) => c.execute(env, inbox),
        }
    }
}

// -- AbstractCallDispatch for AbstractMultiCursor — cross-stage dispatch ------

impl<V: Clone> AbstractCallDispatch<V, AbstractMultiCursor<V>> for Stage {
    fn make_abstract_cursor(
        pipeline: &Pipeline<Stage>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> AbstractMultiCursor<V> {
        match pipeline.stage(stage_id) {
            Some(Stage::Source(_)) => {
                AbstractMultiCursor::HighBlock(AbstractBlockCursor::new(block, stage_id, args))
            }
            _ => AbstractMultiCursor::Low(AbstractBlockCursor::new(block, stage_id, args)),
        }
    }

    fn entry_block_for(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        match pipeline.stage(stage_id) {
            Some(Stage::Source(_)) => {
                entry_block_of::<Stage, HighLevel>(pipeline, callee, stage_id)
            }
            Some(Stage::Lowered(_)) => {
                entry_block_of::<Stage, LowLevel>(pipeline, callee, stage_id)
            }
            None => Err(InterpreterError::MissingEntry),
        }
    }
}

// -- AbstractMultiInterp — cross-stage abstract interpreter type alias --------

pub type AbstractMultiInterp<'ir, V> =
    AbstractInterp<'ir, Stage, HighLevel, V, AbstractMultiCursor<V>>;

// -- HighLevel: Interpretable<AbstractMultiInterp<V>> — cross-stage resolution

// TODO: replace with derive macro
impl<'ir, V> Interpretable<AbstractMultiInterp<'ir, V>> for HighLevel
where
    V: Clone
        + AbstractValue
        + BranchCondition
        + ForLoopValue
        + ProductValue
        + 'static
        + Add<Output = V>
        + Sub<Output = V>
        + Mul<Output = V>
        + Neg<Output = V>
        + CheckedDiv
        + CheckedRem
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>
        + CheckedShl
        + CheckedShr
        + TryFrom<ArithValue>
        + CompareValue,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
{
    fn eval(
        &self,
        env: &mut AbstractMultiInterp<'ir, V>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => {
                    let args = env.read_many(op.args())?;
                    let target = op.target();
                    let current = env.current_stage();
                    if let Ok(callee) = env.resolve_function_for::<HighLevel>(target, current) {
                        Ok(Control::Call {
                            callee,
                            stage: current,
                            args,
                            results: op.results().to_vec(),
                        })
                    } else {
                        let lowered = env
                            .pipeline()
                            .stage_by_name("lowered")
                            .ok_or(InterpreterError::MissingEntry)?;
                        let callee = env.resolve_function_cross_stage::<HighLevel, LowLevel>(
                            target, current, lowered,
                        )?;
                        Ok(Control::Call {
                            callee,
                            stage: lowered,
                            args,
                            results: op.results().to_vec(),
                        })
                    }
                }
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                StructuredControlFlow::If(op) => {
                    eval_if_abstract::<_, AbstractMultiCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::For(op) => {
                    eval_for_abstract::<_, AbstractMultiCursor<V>, HighLevel, _>(op, env)
                }
                StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use kirin::prelude::*;
    use kirin_interpreter::AbstractValue;
    use kirin_interpreter_10::abstract_interp::AbstractInterp;
    use kirin_interpreter_10::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use kirin_interpreter_10::cursor::AbstractBlockCursor;

    use crate::interpreter10::{HighLevelAbstractCursor, HighLevelCursor};
    use crate::language::{HighLevel, LowLevel};
    use crate::stage::Stage;

    use super::*;

    type LowLevelAbstractCursor<V> = AbstractBlockCursor<V, LowLevel>;

    // -----------------------------------------------------------------------
    // Helper: concrete execution of HighLevel (source stage, SCF)
    // No Box::leak — the pipeline borrows are tracked via 'ir lifetime.
    // -----------------------------------------------------------------------

    fn run_concrete_i64_highlevel(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let entry_block = {
            let si: &StageInfo<HighLevel> =
                pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
            let spec_info = spec.get_info(si).unwrap();
            let body_stmt = *spec_info.body();
            let def = body_stmt.definition(si).clone();
            def.regions()
                .next()
                .and_then(|r| r.blocks(si).next())
                .unwrap()
        };
        let mut interp: ConcreteInterp<'_, Stage, HighLevel, i64, HighLevelCursor<i64>> =
            ConcreteInterp::new(&pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .unwrap();
        interp.run().unwrap()
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis of LowLevel (lowered stage, flat CF)
    // -----------------------------------------------------------------------

    fn analyze_lowered<V>(src: &str, func_name: &str, args: Vec<V>) -> Option<V>
    where
        V: Clone
            + AbstractValue
            + BranchCondition
            + ProductValue
            + 'static
            + Add<Output = V>
            + Sub<Output = V>
            + Mul<Output = V>
            + Neg<Output = V>
            + CheckedDiv
            + CheckedRem
            + BitAnd<Output = V>
            + BitOr<Output = V>
            + BitXor<Output = V>
            + Not<Output = V>
            + CheckedShl
            + CheckedShr
            + TryFrom<ArithValue>
            + CompareValue,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        LowLevel:
            Interpretable<AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
        AbstractBlockCursor<V, LowLevel>:
            Execute<AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        // Box::leak is still needed here because the AbstractInterp generic
        // doesn't carry a lifetime parameter for the pipeline borrow.
        // This is a known limitation for the abstract interpreter; the concrete
        // interpreter no longer needs it (see run_concrete_i64_highlevel above).
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("lowered").unwrap();
        let stage_info: &StageInfo<LowLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let mut interp: AbstractInterp<'static, Stage, LowLevel, V, LowLevelAbstractCursor<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis of HighLevel (source stage, SCF)
    // NEW in interpreter-10: abstract interpretation of SCF is now correct.
    // -----------------------------------------------------------------------

    fn analyze_highlevel<V>(src: &str, func_name: &str, args: Vec<V>) -> Option<V>
    where
        V: Clone
            + AbstractValue
            + BranchCondition
            + ForLoopValue
            + ProductValue
            + 'static
            + Add<Output = V>
            + Sub<Output = V>
            + Mul<Output = V>
            + Neg<Output = V>
            + CheckedDiv
            + CheckedRem
            + BitAnd<Output = V>
            + BitOr<Output = V>
            + BitXor<Output = V>
            + Not<Output = V>
            + CheckedShl
            + CheckedShr
            + TryFrom<ArithValue>
            + CompareValue,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        HighLevel:
            Interpretable<AbstractInterp<'static, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
        HighLevelAbstractCursor<V>:
            Execute<AbstractInterp<'static, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let mut interp: AbstractInterp<'static, Stage, HighLevel, V, HighLevelAbstractCursor<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis spanning source and lowered stages
    // -----------------------------------------------------------------------

    fn analyze_multi<V>(src: &str, func_name: &str, args: Vec<V>) -> Option<V>
    where
        V: Clone
            + AbstractValue
            + BranchCondition
            + ForLoopValue
            + ProductValue
            + 'static
            + Add<Output = V>
            + Sub<Output = V>
            + Mul<Output = V>
            + Neg<Output = V>
            + CheckedDiv
            + CheckedRem
            + BitAnd<Output = V>
            + BitOr<Output = V>
            + BitXor<Output = V>
            + Not<Output = V>
            + CheckedShl
            + CheckedShr
            + TryFrom<ArithValue>
            + CompareValue,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        HighLevel: Interpretable<AbstractMultiInterp<'static, V>>,
        LowLevel: Interpretable<AbstractMultiInterp<'static, V>>,
        AbstractMultiCursor<V>: Execute<AbstractMultiInterp<'static, V>>,
    {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let pipeline: &'static Pipeline<Stage> = Box::leak(Box::new(pipeline));
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let mut interp: AbstractMultiInterp<'static, V> =
            AbstractMultiInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Source programs (HighLevel / SCF)
    // -----------------------------------------------------------------------

    const ADD_SOURCE: &str = r#"
stage @source fn @add(i64, i64) -> i64;

specialize @source fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

    const FACTORIAL_SOURCE: &str = r#"
stage @source fn @factorial(i64) -> i64;

specialize @source fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    %result = if %is_base then ^base() {
      yield %one;
    } else ^recurse() {
      %n_minus_1 = sub %n, %one -> i64;
      %rec = call @factorial(%n_minus_1) -> i64;
      %prod = mul %n, %rec -> i64;
      yield %prod;
    } -> i64;
    ret %result;
  }
}
"#;

    const ABS_SOURCE: &str = r#"
stage @source fn @abs(i64) -> i64;

specialize @source fn @abs(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    %result = if %is_neg then ^neg() {
      %neg_x = neg %x -> i64;
      yield %neg_x;
    } else ^pos() {
      yield %x;
    } -> i64;
    ret %result;
  }
}
"#;

    // -----------------------------------------------------------------------
    // Lowered programs (flat CF)
    // -----------------------------------------------------------------------

    const ADD_LOWERED: &str = r#"
stage @lowered fn @add(i64, i64) -> i64;

specialize @lowered fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %result = add %a, %b -> i64;
    ret %result;
  }
}
"#;

    const BRANCH_LOWERED: &str = r#"
stage @lowered fn @sign(i64) -> i64;

specialize @lowered fn @sign(i64) -> i64 {
  ^entry(%x: i64) {
    %zero = constant 0 -> i64;
    %is_neg = lt %x, %zero -> i64;
    cond_br %is_neg then=^neg() else=^pos();
  }
  ^neg() {
    %one = constant 1 -> i64;
    ret %one;
  }
  ^pos() {
    %zero2 = constant 0 -> i64;
    ret %zero2;
  }
}
"#;

    const FACTORIAL_LOWERED: &str = r#"
stage @lowered fn @factorial(i64) -> i64;

specialize @lowered fn @factorial(i64) -> i64 {
  ^entry(%n: i64) {
    %one = constant 1 -> i64;
    %is_base = le %n, %one -> i64;
    cond_br %is_base then=^base() else=^recurse();
  }
  ^base() {
    %one2 = constant 1 -> i64;
    ret %one2;
  }
  ^recurse() {
    %one3 = constant 1 -> i64;
    %n_minus_1 = sub %n, %one3 -> i64;
    %rec = call @factorial(%n_minus_1) -> i64;
    %prod = mul %n, %rec -> i64;
    ret %prod;
  }
}
"#;

    // -----------------------------------------------------------------------
    // ToyType: type lattice for abstract interpretation
    // -----------------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ToyType {
        Bottom,
        I64,
        Bool,
        Top,
    }

    impl kirin::prelude::Lattice for ToyType {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::Bottom, x) | (x, ToyType::Bottom) => x.clone(),
                (a, b) if a == b => a.clone(),
                _ => ToyType::Top,
            }
        }
        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (ToyType::Top, x) | (x, ToyType::Top) => x.clone(),
                (a, b) if a == b => a.clone(),
                _ => ToyType::Bottom,
            }
        }
        fn is_subseteq(&self, other: &Self) -> bool {
            matches!((self, other), (_, ToyType::Top) | (ToyType::Bottom, _)) || self == other
        }
    }

    impl kirin::prelude::HasBottom for ToyType {
        fn bottom() -> Self {
            ToyType::Bottom
        }
    }

    impl AbstractValue for ToyType {
        fn widen(&self, next: &Self) -> Self {
            self.join(next)
        }
    }

    impl BranchCondition for ToyType {
        fn is_truthy(&self) -> Option<bool> {
            None
        }
    }

    impl ForLoopValue for ToyType {
        fn loop_condition(&self, _end: &Self) -> Option<bool> {
            None
        }
        fn loop_step(&self, _step: &Self) -> Option<Self> {
            Some(self.join(_step))
        }
    }

    impl ProductValue for ToyType {
        fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
            None
        }
        fn from_product(_product: kirin::prelude::Product<Self>) -> Self {
            ToyType::Top
        }
    }

    impl TryFrom<ArithValue> for ToyType {
        type Error = std::convert::Infallible;
        fn try_from(_: ArithValue) -> Result<Self, Self::Error> {
            Ok(ToyType::I64)
        }
    }

    impl CompareValue for ToyType {
        type Bool = ToyType;
        fn cmp_eq(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_ne(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_lt(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_le(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_gt(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
        fn cmp_ge(&self, _: &Self) -> ToyType {
            ToyType::Bool
        }
    }

    impl std::ops::Add for ToyType {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Sub for ToyType {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Mul for ToyType {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Neg for ToyType {
        type Output = Self;
        fn neg(self) -> Self {
            self
        }
    }
    impl CheckedDiv for ToyType {
        fn checked_div(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedRem for ToyType {
        fn checked_rem(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl std::ops::BitAnd for ToyType {
        type Output = Self;
        fn bitand(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::BitOr for ToyType {
        type Output = Self;
        fn bitor(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::BitXor for ToyType {
        type Output = Self;
        fn bitxor(self, rhs: Self) -> Self {
            self.join(&rhs)
        }
    }
    impl std::ops::Not for ToyType {
        type Output = Self;
        fn not(self) -> Self {
            self
        }
    }
    impl CheckedShl for ToyType {
        fn checked_shl(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }
    impl CheckedShr for ToyType {
        fn checked_shr(self, _: Self) -> Option<Self> {
            Some(self)
        }
    }

    // -----------------------------------------------------------------------
    // Concrete tests (HighLevel / source stage, SCF)
    // Note: No Box::leak required — pipeline lifetime managed by the function.
    // -----------------------------------------------------------------------

    #[test]
    fn test_add_highlevel() {
        let result = run_concrete_i64_highlevel(ADD_SOURCE, "add", &[3i64, 5i64]);
        assert_eq!(result, Some(8));
    }

    #[test]
    fn test_factorial() {
        let result = run_concrete_i64_highlevel(FACTORIAL_SOURCE, "factorial", &[5i64]);
        assert_eq!(result, Some(120));
    }

    #[test]
    fn test_abs_positive() {
        let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[42i64]);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_abs_negative() {
        let result = run_concrete_i64_highlevel(ABS_SOURCE, "abs", &[-7i64]);
        assert_eq!(result, Some(7));
    }

    // -----------------------------------------------------------------------
    // Abstract tests (LowLevel / lowered stage, flat CF, Interval domain)
    // -----------------------------------------------------------------------

    #[test]
    fn interval_add_known_range() {
        let result = analyze_lowered::<Interval>(
            ADD_LOWERED,
            "add",
            vec![Interval::new(1, 3), Interval::new(2, 4)],
        );
        assert_eq!(result, Some(Interval::new(3, 7)));
    }

    #[test]
    fn interval_branch_joins_both_paths() {
        let result =
            analyze_lowered::<Interval>(BRANCH_LOWERED, "sign", vec![Interval::new(-5, 5)]);
        assert_eq!(result, Some(Interval::new(0, 1)));
    }

    #[test]
    fn interval_factorial_converges() {
        let result =
            analyze_lowered::<Interval>(FACTORIAL_LOWERED, "factorial", vec![Interval::new(0, 10)]);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.is_empty());
    }

    // -----------------------------------------------------------------------
    // Abstract tests (HighLevel / source stage, SCF)
    // NEW in interpreter-10: abstract interpretation of SCF programs.
    // These tests verify that the AbstractIfCursor fix works correctly.
    // -----------------------------------------------------------------------

    #[test]
    fn toytype_add_highlevel_abstract() {
        // Simple add: both args are I64, result should be I64.
        let result =
            analyze_highlevel::<ToyType>(ADD_SOURCE, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_abs_highlevel_abstract() {
        // abs uses scf.if — exercises AbstractIfCursor.
        // Both branches yield I64, so the join is I64.
        let result = analyze_highlevel::<ToyType>(ABS_SOURCE, "abs", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_factorial_highlevel_abstract() {
        // factorial uses scf.if + recursive call — exercises AbstractIfCursor
        // and interprocedural fixpoint.
        let result =
            analyze_highlevel::<ToyType>(FACTORIAL_SOURCE, "factorial", vec![ToyType::I64]);
        // Both branches yield I64, recursion eventually converges.
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_lowered_add_propagates_i64() {
        let result =
            analyze_lowered::<ToyType>(ADD_LOWERED, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    // -----------------------------------------------------------------------
    // Multi-stage concrete interpreter tests
    //
    // `MultiInterp` uses `MultiCursor<V>` which covers cursors for both the
    // source (HighLevel) and lowered (LowLevel) stages. `CallDispatch for Stage`
    // selects the correct `BlockCursor` variant based on the callee's stage.
    //
    // The `HighLevel: Interpretable<MultiInterp<V>>` impl tries to resolve
    // function calls at the current stage first, then falls back to lowered.
    // -----------------------------------------------------------------------

    fn run_multi_i64(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
        let pipeline: Pipeline<Stage> = {
            let mut p = Pipeline::new();
            ParsePipelineText::parse(&mut p, src).expect("parse failed");
            p
        };
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function(func_name, stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let entry_block = {
            let si: &StageInfo<HighLevel> =
                pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
            let spec_info = spec.get_info(si).unwrap();
            let body_stmt = *spec_info.body();
            let def = body_stmt.definition(si).clone();
            def.regions()
                .next()
                .and_then(|r| r.blocks(si).next())
                .unwrap()
        };
        let mut interp: MultiInterp<'_, i64> = MultiInterp::new(&pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .unwrap();
        interp.run().unwrap()
    }

    // A program where source::main calls a function that only exists at lowered.
    // Exercises cross-stage call dispatch: HighLevel sees an unresolvable call at
    // source stage, falls back to lowered, and CallDispatch creates a
    // BlockCursor<i64, LowLevel> for the callee's frame.
    const CROSS_STAGE_SRC: &str = r#"
stage @source fn @main(i64) -> i64;
stage @lowered fn @double(i64) -> i64;

specialize @source fn @main(i64) -> i64 {
  ^entry(%n: i64) {
    %result = call @double(%n) -> i64;
    ret %result;
  }
}

specialize @lowered fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
    ret %r;
  }
}
"#;

    // A program where source::wrapper calls source::add (same-stage call) to
    // verify that same-stage calls through CallDispatch still work.
    const SAME_STAGE_CALL_SRC: &str = r#"
stage @source fn @add(i64, i64) -> i64;
stage @source fn @wrapper(i64, i64) -> i64;

specialize @source fn @add(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %r = add %a, %b -> i64;
    ret %r;
  }
}

specialize @source fn @wrapper(i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64) {
    %r = call @add(%a, %b) -> i64;
    ret %r;
  }
}
"#;

    #[test]
    fn multi_cross_stage_source_calls_lowered() {
        // source::main calls lowered::double — a genuine cross-stage call.
        let result = run_multi_i64(CROSS_STAGE_SRC, "main", &[7i64]);
        assert_eq!(result, Some(14));
    }

    #[test]
    fn multi_cross_stage_double_five() {
        let result = run_multi_i64(CROSS_STAGE_SRC, "main", &[5i64]);
        assert_eq!(result, Some(10));
    }

    #[test]
    fn multi_same_stage_call_through_dispatch() {
        // source calls source — verifies same-stage calls still work via CallDispatch.
        let result = run_multi_i64(SAME_STAGE_CALL_SRC, "wrapper", &[3i64, 4i64]);
        assert_eq!(result, Some(7));
    }

    // -----------------------------------------------------------------------
    // Multi-stage abstract interpreter tests
    //
    // `AbstractMultiInterp` spans source (HighLevel) and lowered (LowLevel).
    // The abstract interpreter tracks types across stage boundaries: when
    // source::main calls lowered::double, the domain value (ToyType::I64)
    // propagates through the cross-stage call and back.
    // -----------------------------------------------------------------------

    use crate::interpreter10::AbstractMultiCursor;

    #[test]
    fn abstract_multi_same_stage_type_propagates() {
        // source::wrapper calls source::add — same stage, abstract multi-interp.
        let result = analyze_multi::<ToyType>(
            SAME_STAGE_CALL_SRC,
            "wrapper",
            vec![ToyType::I64, ToyType::I64],
        );
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn abstract_multi_cross_stage_type_propagates() {
        // source::main calls lowered::double — cross-stage, abstract multi-interp.
        // The type I64 should propagate through the lowered::double call and back.
        let result = analyze_multi::<ToyType>(CROSS_STAGE_SRC, "main", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn interval_cross_stage_doubles_range() {
        // source::main calls lowered::double (n + n) with input interval [1, 3].
        // The result crosses the stage boundary and should be [2, 6].
        let result = analyze_multi::<Interval>(CROSS_STAGE_SRC, "main", vec![Interval::new(1, 3)]);
        assert_eq!(result, Some(Interval::new(2, 6)));
    }
}
