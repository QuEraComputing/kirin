use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::{Block, CompileStage, HasStageInfo, Pipeline, SpecializedFunction};
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_function::interpreter11::interpret::eval_call_for_dialect;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_11::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_11::abstract_interp::AbstractInterp;
use kirin_interpreter_11::algebra::Lift;
use kirin_interpreter_11::call_dispatch::CallDispatch;
use kirin_interpreter_11::concrete::ConcreteInterp;
use kirin_interpreter_11::control::{Control, CursorExt};
use kirin_interpreter_11::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_11::env::{AbstractMode, ConcreteMode, Env};
use kirin_interpreter_11::error::InterpreterError;
use kirin_interpreter_11::execute::Execute;
use kirin_interpreter_11::interpretable::Interpretable;
use kirin_interpreter_11::pipeline::entry_block_of;
use kirin_scf::ForLoopValue;
use kirin_scf::StructuredControlFlow;
use kirin_scf::interpreter11::cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};
use kirin_scf::interpreter11::interpret::{
    eval_for_abstract, eval_for_concrete, eval_if_abstract, eval_if_concrete,
};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

// ---------------------------------------------------------------------------
// ToyVal — trait alias collapsing the 13 value bounds used throughout
//
// Iteration-11 improvement: replaces the 13-bound copy-paste across every
// Execute and Interpretable impl with a single supertrait.
// ---------------------------------------------------------------------------

/// Supertrait alias for all value requirements in toy-lang concrete interpreters.
pub trait ToyVal:
    Clone
    + BranchCondition
    + ForLoopValue
    + ProductValue
    + 'static
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<Output = Self>
    + Neg<Output = Self>
    + CheckedDiv
    + CheckedRem
    + BitAnd<Output = Self>
    + BitOr<Output = Self>
    + BitXor<Output = Self>
    + Not<Output = Self>
    + CheckedShl
    + CheckedShr
    + TryFrom<ArithValue>
    + CompareValue
{
}

impl<V> ToyVal for V
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
}

/// Extends `ToyVal` with abstract interpretation requirements.
pub trait AbstractToyVal: ToyVal + AbstractValue {}
impl<V: ToyVal + AbstractValue> AbstractToyVal for V {}

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

impl<V: Clone> Lift<HighLevelCursor<V>> for IfCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelCursor<V>> for ForCursor<V, HighLevel> {
    fn lift(self) -> HighLevelCursor<V> {
        HighLevelCursor::Scf(SCFCursor::For(self))
    }
}

// TODO: replace this with derive macro
impl<E, V> Execute<E> for HighLevelCursor<V>
where
    V: ToyVal,
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
    V: ToyVal,
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
// HighLevelAbstractCursor — abstract mode cursor coproduct for HighLevel
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

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractIfCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::If(self))
    }
}

impl<V: Clone> Lift<HighLevelAbstractCursor<V>> for AbstractForCursor<V, HighLevel> {
    fn lift(self) -> HighLevelAbstractCursor<V> {
        HighLevelAbstractCursor::Scf(AbstractSCFCursor::For(self))
    }
}

// TODO: replace this with derive macro
impl<E, V> Execute<E> for HighLevelAbstractCursor<V>
where
    V: AbstractToyVal,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: kirin_interpreter_11::env::AbstractEnv<
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
// HighLevel: Interpretable<AbstractInterp<...>> — abstract mode
// ---------------------------------------------------------------------------

// TODO: replace this with derive macro
impl<'ir, V> Interpretable<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>
    for HighLevel
where
    V: AbstractToyVal,
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
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for LowLevel
where
    E: Env<Value = V>,
    V: ToyVal,
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
// CallDispatch for HighLevelCursor — single-stage concrete interpreter
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
// MultiCursor — concrete cursor coproduct spanning both source and lowered
// ---------------------------------------------------------------------------

pub enum MultiCursor<V: Clone> {
    High(BlockCursor<V, HighLevel>),
    Scf(SCFCursor<V, HighLevel>),
    Low(BlockCursor<V, LowLevel>),
}

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

// TODO: replace with derive macro
impl<E, V> Execute<E> for MultiCursor<V>
where
    V: ToyVal,
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

pub type MultiInterp<'ir, V> = ConcreteInterp<'ir, Stage, HighLevel, V, MultiCursor<V>>;

// TODO: replace with derive macro
impl<'ir, V> Interpretable<MultiInterp<'ir, V>> for HighLevel
where
    V: ToyVal,
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
    V: AbstractToyVal,
    <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
    <V as CompareValue>::Bool: Into<V>,
    E: kirin_interpreter_11::env::AbstractEnv<Value = V, Ext = CursorExt<AbstractMultiCursor<V>>>,
    E: Env<Mode = AbstractMode<AbstractMultiCursor<V>>>,
    E::Stages: HasStageInfo<HighLevel> + HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
    HighLevel: Interpretable<E>,
    LowLevel: Interpretable<E>,
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

pub type AbstractMultiInterp<'ir, V> =
    AbstractInterp<'ir, Stage, HighLevel, V, AbstractMultiCursor<V>>;

// TODO: replace with derive macro
impl<'ir, V> Interpretable<AbstractMultiInterp<'ir, V>> for HighLevel
where
    V: AbstractToyVal,
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
    use kirin_interpreter_11::abstract_interp::AbstractInterp;
    use kirin_interpreter_11::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use kirin_interpreter_11::cursor::AbstractBlockCursor;

    use crate::interpreter11::{
        AbstractMultiCursor, AbstractMultiInterp, HighLevelAbstractCursor, HighLevelCursor,
        MultiInterp,
    };
    use crate::language::{HighLevel, LowLevel};
    use crate::stage::Stage;

    use super::*;

    type LowLevelAbstractCursor<V> = AbstractBlockCursor<V, LowLevel>;

    // -----------------------------------------------------------------------
    // Helper: parse a pipeline from source text
    // -----------------------------------------------------------------------

    fn build_pipeline(src: &str) -> Pipeline<Stage> {
        let mut p = Pipeline::new();
        ParsePipelineText::parse(&mut p, src).expect("parse failed");
        p
    }

    // -----------------------------------------------------------------------
    // Helper: concrete execution of HighLevel (source stage, SCF)
    //
    // Iteration-11: no Box::leak required. The pipeline is borrowed for 'ir
    // and the ConcreteInterp uses the same lifetime, keeping borrows clean.
    // -----------------------------------------------------------------------

    fn run_concrete_i64_highlevel(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
        let pipeline = build_pipeline(src);
        run_concrete_i64_highlevel_on(&pipeline, func_name, args)
    }

    fn run_concrete_i64_highlevel_on<'ir>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: &[i64],
    ) -> Option<i64> {
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
        let mut interp: ConcreteInterp<'ir, Stage, HighLevel, i64, HighLevelCursor<i64>> =
            ConcreteInterp::new(pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .unwrap();
        interp.run().unwrap()
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis of LowLevel (lowered stage, flat CF)
    //
    // Iteration-11 fix: takes &'ir Pipeline<Stage> instead of &str.
    // No Box::leak — the 'ir lifetime tracks the pipeline borrow cleanly.
    // -----------------------------------------------------------------------

    fn analyze_lowered<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: ToyVal + AbstractValue,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        LowLevel: Interpretable<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
        LowLevelAbstractCursor<V>:
            Execute<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstractCursor<V>>>,
    {
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
        let mut interp: AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstractCursor<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis of HighLevel (source stage, SCF)
    //
    // Iteration-11 fix: no Box::leak.
    // -----------------------------------------------------------------------

    fn analyze_highlevel<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: AbstractToyVal,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        HighLevel:
            Interpretable<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
        HighLevelAbstractCursor<V>:
            Execute<AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>>>,
    {
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
        let mut interp: AbstractInterp<'ir, Stage, HighLevel, V, HighLevelAbstractCursor<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Helper: abstract analysis spanning source and lowered stages
    //
    // Iteration-11 fix: no Box::leak.
    // -----------------------------------------------------------------------

    fn analyze_multi<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: AbstractToyVal,
        <V as TryFrom<ArithValue>>::Error: std::error::Error + Send + Sync + 'static,
        <V as CompareValue>::Bool: Into<V>,
        HighLevel: Interpretable<AbstractMultiInterp<'ir, V>>,
        LowLevel: Interpretable<AbstractMultiInterp<'ir, V>>,
        AbstractMultiCursor<V>: Execute<AbstractMultiInterp<'ir, V>>,
    {
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
        let mut interp: AbstractMultiInterp<'ir, V> = AbstractMultiInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    // -----------------------------------------------------------------------
    // Multi-stage concrete helper
    // -----------------------------------------------------------------------

    fn run_multi_i64(src: &str, func_name: &str, args: &[i64]) -> Option<i64> {
        let pipeline = build_pipeline(src);
        run_multi_i64_on(&pipeline, func_name, args)
    }

    fn run_multi_i64_on<'ir>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: &[i64],
    ) -> Option<i64> {
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
        let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
        interp
            .enter_function::<HighLevel>(spec, entry_block, args)
            .unwrap();
        interp.run().unwrap()
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
    // Cross-stage programs
    // -----------------------------------------------------------------------

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
    // ConstProp — extensibility probe (iteration-11)
    //
    // Implemented entirely in toy-lang (no framework changes). Demonstrates
    // that the interpreter framework is extensible: a new analysis domain
    // needs only to implement the value traits and use the existing
    // AbstractInterp machinery.
    //
    // Domain: Bottom (no info) | Const(n) (exact value) | Top (multiple values)
    // -----------------------------------------------------------------------

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ConstProp {
        Bottom,
        Const(i64),
        Top,
    }

    impl kirin::prelude::Lattice for ConstProp {
        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (ConstProp::Bottom, x) | (x, ConstProp::Bottom) => x.clone(),
                (ConstProp::Const(a), ConstProp::Const(b)) if a == b => ConstProp::Const(*a),
                _ => ConstProp::Top,
            }
        }
        fn meet(&self, other: &Self) -> Self {
            match (self, other) {
                (ConstProp::Top, x) | (x, ConstProp::Top) => x.clone(),
                (ConstProp::Const(a), ConstProp::Const(b)) if a == b => ConstProp::Const(*a),
                _ => ConstProp::Bottom,
            }
        }
        fn is_subseteq(&self, other: &Self) -> bool {
            matches!((self, other), (_, ConstProp::Top) | (ConstProp::Bottom, _)) || self == other
        }
    }

    impl kirin::prelude::HasBottom for ConstProp {
        fn bottom() -> Self {
            ConstProp::Bottom
        }
    }

    impl AbstractValue for ConstProp {
        fn widen(&self, next: &Self) -> Self {
            self.join(next)
        }
    }

    impl BranchCondition for ConstProp {
        fn is_truthy(&self) -> Option<bool> {
            match self {
                ConstProp::Const(0) => Some(false),
                ConstProp::Const(_) => Some(true),
                _ => None,
            }
        }
    }

    impl ForLoopValue for ConstProp {
        fn loop_condition(&self, end: &Self) -> Option<bool> {
            match (self, end) {
                (ConstProp::Const(iv), ConstProp::Const(e)) => Some(iv < e),
                _ => None,
            }
        }
        fn loop_step(&self, step: &Self) -> Option<Self> {
            match (self, step) {
                (ConstProp::Const(iv), ConstProp::Const(s)) => {
                    Some(ConstProp::Const(iv.wrapping_add(*s)))
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
                _ => Some(ConstProp::Top),
            }
        }
    }

    impl ProductValue for ConstProp {
        fn as_product(&self) -> Option<&kirin::prelude::Product<Self>> {
            None
        }
        fn from_product(_: kirin::prelude::Product<Self>) -> Self {
            ConstProp::Top
        }
    }

    impl TryFrom<ArithValue> for ConstProp {
        type Error = std::convert::Infallible;
        fn try_from(v: ArithValue) -> Result<Self, Self::Error> {
            Ok(match v {
                ArithValue::I64(n) => ConstProp::Const(n),
                _ => ConstProp::Top,
            })
        }
    }

    impl CompareValue for ConstProp {
        type Bool = ConstProp;
        fn cmp_eq(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a == b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
        fn cmp_ne(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a != b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
        fn cmp_lt(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a < b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
        fn cmp_le(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a <= b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
        fn cmp_gt(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a > b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
        fn cmp_ge(&self, other: &Self) -> ConstProp {
            match (self, other) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    ConstProp::Const(if a >= b { 1 } else { 0 })
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }

    impl std::ops::Add for ConstProp {
        type Output = Self;
        fn add(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_add(b)),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::Sub for ConstProp {
        type Output = Self;
        fn sub(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_sub(b)),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::Mul for ConstProp {
        type Output = Self;
        fn mul(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a.wrapping_mul(b)),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::Neg for ConstProp {
        type Output = Self;
        fn neg(self) -> Self {
            match self {
                ConstProp::Const(n) => ConstProp::Const(n.wrapping_neg()),
                other => other,
            }
        }
    }
    impl CheckedDiv for ConstProp {
        fn checked_div(self, rhs: Self) -> Option<Self> {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    a.checked_div(b).map(ConstProp::Const)
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
                _ => Some(ConstProp::Top),
            }
        }
    }
    impl CheckedRem for ConstProp {
        fn checked_rem(self, rhs: Self) -> Option<Self> {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => {
                    a.checked_rem(b).map(ConstProp::Const)
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
                _ => Some(ConstProp::Top),
            }
        }
    }
    impl std::ops::BitAnd for ConstProp {
        type Output = Self;
        fn bitand(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a & b),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::BitOr for ConstProp {
        type Output = Self;
        fn bitor(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a | b),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::BitXor for ConstProp {
        type Output = Self;
        fn bitxor(self, rhs: Self) -> Self {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) => ConstProp::Const(a ^ b),
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => ConstProp::Bottom,
                _ => ConstProp::Top,
            }
        }
    }
    impl std::ops::Not for ConstProp {
        type Output = Self;
        fn not(self) -> Self {
            match self {
                ConstProp::Const(n) => ConstProp::Const(!n),
                other => other,
            }
        }
    }
    impl CheckedShl for ConstProp {
        fn checked_shl(self, rhs: Self) -> Option<Self> {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) if b >= 0 && b < 64 => {
                    Some(ConstProp::Const(a << b))
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
                _ => Some(ConstProp::Top),
            }
        }
    }
    impl CheckedShr for ConstProp {
        fn checked_shr(self, rhs: Self) -> Option<Self> {
            match (self, rhs) {
                (ConstProp::Const(a), ConstProp::Const(b)) if b >= 0 && b < 64 => {
                    Some(ConstProp::Const(a >> b))
                }
                (ConstProp::Bottom, _) | (_, ConstProp::Bottom) => Some(ConstProp::Bottom),
                _ => Some(ConstProp::Top),
            }
        }
    }

    // -----------------------------------------------------------------------
    // Concrete tests (HighLevel / source stage, SCF)
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
    // Iteration-11: no Box::leak — pipeline created by caller, 'ir threaded.
    // -----------------------------------------------------------------------

    #[test]
    fn interval_add_known_range() {
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = analyze_lowered::<Interval>(
            &pipeline,
            "add",
            vec![Interval::new(1, 3), Interval::new(2, 4)],
        );
        assert_eq!(result, Some(Interval::new(3, 7)));
    }

    #[test]
    fn interval_branch_joins_both_paths() {
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<Interval>(&pipeline, "sign", vec![Interval::new(-5, 5)]);
        assert_eq!(result, Some(Interval::new(0, 1)));
    }

    #[test]
    fn interval_factorial_converges() {
        let pipeline = build_pipeline(FACTORIAL_LOWERED);
        let result =
            analyze_lowered::<Interval>(&pipeline, "factorial", vec![Interval::new(0, 10)]);
        assert!(result.is_some());
        let r = result.unwrap();
        assert!(!r.is_empty());
    }

    // -----------------------------------------------------------------------
    // Abstract tests (HighLevel / source stage, SCF)
    // -----------------------------------------------------------------------

    #[test]
    fn toytype_add_highlevel_abstract() {
        let pipeline = build_pipeline(ADD_SOURCE);
        let result =
            analyze_highlevel::<ToyType>(&pipeline, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_abs_highlevel_abstract() {
        let pipeline = build_pipeline(ABS_SOURCE);
        let result = analyze_highlevel::<ToyType>(&pipeline, "abs", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_factorial_highlevel_abstract() {
        let pipeline = build_pipeline(FACTORIAL_SOURCE);
        let result = analyze_highlevel::<ToyType>(&pipeline, "factorial", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn toytype_lowered_add_propagates_i64() {
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = analyze_lowered::<ToyType>(&pipeline, "add", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    // -----------------------------------------------------------------------
    // Multi-stage concrete interpreter tests
    // -----------------------------------------------------------------------

    #[test]
    fn multi_cross_stage_source_calls_lowered() {
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
        let result = run_multi_i64(SAME_STAGE_CALL_SRC, "wrapper", &[3i64, 4i64]);
        assert_eq!(result, Some(7));
    }

    // -----------------------------------------------------------------------
    // Multi-stage abstract interpreter tests
    // Iteration-11: no Box::leak.
    // -----------------------------------------------------------------------

    #[test]
    fn abstract_multi_same_stage_type_propagates() {
        let pipeline = build_pipeline(SAME_STAGE_CALL_SRC);
        let result =
            analyze_multi::<ToyType>(&pipeline, "wrapper", vec![ToyType::I64, ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn abstract_multi_cross_stage_type_propagates() {
        let pipeline = build_pipeline(CROSS_STAGE_SRC);
        let result = analyze_multi::<ToyType>(&pipeline, "main", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    #[test]
    fn interval_cross_stage_doubles_range() {
        let pipeline = build_pipeline(CROSS_STAGE_SRC);
        let result = analyze_multi::<Interval>(&pipeline, "main", vec![Interval::new(1, 3)]);
        assert_eq!(result, Some(Interval::new(2, 6)));
    }

    // -----------------------------------------------------------------------
    // Extensibility probe: ConstProp analysis
    //
    // These tests verify that a brand-new analysis domain (ConstProp) can be
    // added entirely within toy-lang — no changes to any interpreter crate or
    // dialect crate. This is the R8 extensibility probe.
    // -----------------------------------------------------------------------

    #[test]
    fn constprop_add_two_constants() {
        // add(2, 3) → exactly 5
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = analyze_lowered::<ConstProp>(
            &pipeline,
            "add",
            vec![ConstProp::Const(2), ConstProp::Const(3)],
        );
        assert_eq!(result, Some(ConstProp::Const(5)));
    }

    #[test]
    fn constprop_top_input_propagates() {
        // add(Top, 3) → Top (unknown input stays unknown)
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = analyze_lowered::<ConstProp>(
            &pipeline,
            "add",
            vec![ConstProp::Top, ConstProp::Const(3)],
        );
        assert_eq!(result, Some(ConstProp::Top));
    }

    #[test]
    fn constprop_branch_positive_input() {
        // sign(5): 5 < 0 is Const(0) = false → takes pos branch → Const(0)
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(5)]);
        assert_eq!(result, Some(ConstProp::Const(0)));
    }

    #[test]
    fn constprop_branch_negative_input() {
        // sign(-3): -3 < 0 is Const(1) = true → takes neg branch → Const(1)
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(-3)]);
        assert_eq!(result, Some(ConstProp::Const(1)));
    }

    #[test]
    fn constprop_branch_unknown_joins_both_paths() {
        // sign(Top): unknown input → both branches reachable → join(Const(1), Const(0)) = Top
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Top]);
        assert_eq!(result, Some(ConstProp::Top));
    }
}
