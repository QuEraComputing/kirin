use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::{Block, CompileStage, HasStageInfo, Pipeline, SpecializedFunction};
use kirin_arith::{ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_function::interpreter16::interpret::CallSeam;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_16::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_16::abstract_interp::AbstractInterp;
use kirin_interpreter_16::algebra::{Lift, Project, SingleStageCursorFor};
use kirin_interpreter_16::call_dispatch::CallDispatch;
use kirin_interpreter_16::concrete::ConcreteInterp;
use kirin_interpreter_16::control::{Control, CursorExt};
use kirin_interpreter_16::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_16::env::{AbstractEnv, AbstractMode, ConcreteMode, Env};
use kirin_interpreter_16::error::InterpreterError;
use kirin_interpreter_16::execute::Execute;
use kirin_interpreter_16::interpretable::Interpretable;
use kirin_interpreter_16::pipeline::PipelineHandle;
use kirin_scf::ForLoopValue;
use kirin_scf::interpreter16::cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};
use kirin_scf::interpreter16::interpret::ScfSeam;

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

// ---------------------------------------------------------------------------
// ToyVal — trait alias collapsing value bounds.
//
// Blanket impl is safe because all component traits are implemented generically;
// the `V: ToyVal` alias exists purely to avoid repeating the bound list.
// ---------------------------------------------------------------------------

pub trait ToyVal:
    Clone
    + BranchCondition
    + ForLoopValue
    + ProductValue
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
    + TryFrom<ArithValue, Error: std::error::Error + Send + Sync + 'static>
    + CompareValue<Bool: Into<Self>>
{
}

impl<V> ToyVal for V
where
    V: Clone
        + BranchCondition
        + ForLoopValue
        + ProductValue
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

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            HighLevelCursor::Block(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<SCFCursor<V, HighLevel>> for HighLevelCursor<V> {
    fn try_project(self) -> Result<SCFCursor<V, HighLevel>, Self> {
        match self {
            HighLevelCursor::Scf(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> SingleStageCursorFor<HighLevel> for HighLevelCursor<V> {}

impl<E, V> Execute<E> for HighLevelCursor<V>
where
    V: ToyVal,
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

impl<V: Clone> Project<AbstractBlockCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    fn try_project(self) -> Result<AbstractBlockCursor<V, HighLevel>, Self> {
        match self {
            HighLevelAbstractCursor::Block(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<AbstractSCFCursor<V, HighLevel>> for HighLevelAbstractCursor<V> {
    fn try_project(self) -> Result<AbstractSCFCursor<V, HighLevel>, Self> {
        match self {
            HighLevelAbstractCursor::Scf(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> SingleStageCursorFor<HighLevel> for HighLevelAbstractCursor<V> {}

impl<E, V> Execute<E> for HighLevelAbstractCursor<V>
where
    V: AbstractToyVal,
    E: AbstractEnv<Value = V, Ext = CursorExt<HighLevelAbstractCursor<V>>>,
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
// LowLevelAbstract<V> — local wrapper enabling SingleStageCursorFor<LowLevel>
//
// AbstractBlockCursor<V, LowLevel> is a foreign type with an uncovered type
// parameter V, so the orphan rule prevents impl SingleStageCursorFor<LowLevel>
// directly. This local newtype wrapper is the idiomatic solution.
// ---------------------------------------------------------------------------

pub struct LowLevelAbstract<V: Clone>(pub AbstractBlockCursor<V, LowLevel>);

impl<V: Clone> SingleStageCursorFor<LowLevel> for LowLevelAbstract<V> {}

impl<E, V> Execute<E> for LowLevelAbstract<V>
where
    V: Clone,
    LowLevel: Interpretable<E>,
    E: AbstractEnv<Value = V, Ext = CursorExt<LowLevelAbstract<V>>>,
    E: Env<Mode = AbstractMode<LowLevelAbstract<V>>>,
    E::Stages: HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
{
    fn execute(
        &mut self,
        env: &mut E,
        inbox: Option<V>,
    ) -> Result<Control<V, CursorExt<LowLevelAbstract<V>>>, E::Error> {
        self.0.execute(env, inbox)
    }
}

// ---------------------------------------------------------------------------
// HighLevel: Interpretable<E> — single generic impl via seam traits
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for HighLevel
where
    E: Env<Value = V> + ScfSeam<HighLevel> + CallSeam<HighLevel>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    V: ToyVal,
{
    fn eval(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            HighLevel::Lexical(op) => match op {
                kirin_function::Lexical::FunctionBody(op) => op.eval(env),
                kirin_function::Lexical::Lambda(op) => op.eval(env),
                kirin_function::Lexical::Call(op) => env.eval_call(op),
                kirin_function::Lexical::Return(op) => op.eval(env),
            },
            HighLevel::Structured(op) => match op {
                kirin_scf::StructuredControlFlow::If(op) => env.eval_if(op),
                kirin_scf::StructuredControlFlow::For(op) => env.eval_for(op),
                kirin_scf::StructuredControlFlow::Yield(op) => op.eval(env),
            },
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable<E> — uses CallSeam<LowLevel> for call dispatch
//
// Key iter-16 change: Call dispatch goes through CallSeam<LowLevel> instead
// of eval_call_for_dialect. This allows multi-stage interpreters that supply
// a cross-stage fallback to call HighLevel functions from LowLevel code.
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for LowLevel
where
    E: Env<Value = V> + CallSeam<LowLevel>,
    V: ToyVal,
    E::Stages: HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
{
    fn eval(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            LowLevel::Lifted(op) => match op {
                kirin_function::Lifted::FunctionBody(op) => op.eval(env),
                kirin_function::Lifted::Bind(op) => op.eval(env),
                kirin_function::Lifted::Call(op) => env.eval_call(op),
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
        let entry = PipelineHandle::new(pipeline, stage_id)
            .entry_block_of::<HighLevel>(callee, stage_id)?;
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

impl<V: Clone> Project<BlockCursor<V, HighLevel>> for MultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, HighLevel>, Self> {
        match self {
            MultiCursor::High(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<BlockCursor<V, LowLevel>> for MultiCursor<V> {
    fn try_project(self) -> Result<BlockCursor<V, LowLevel>, Self> {
        match self {
            MultiCursor::Low(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<E, V> Execute<E> for MultiCursor<V>
where
    V: ToyVal,
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
        let handle = PipelineHandle::new(pipeline, stage_id);
        match stage_container {
            Stage::Source(_) => {
                let entry = handle.entry_block_of::<HighLevel>(callee, stage_id)?;
                Ok(MultiCursor::High(BlockCursor::new(entry, stage_id, args)))
            }
            Stage::Lowered(_) => {
                let entry = handle.entry_block_of::<LowLevel>(callee, stage_id)?;
                Ok(MultiCursor::Low(BlockCursor::new(entry, stage_id, args)))
            }
        }
    }
}

pub type MultiInterp<'ir, V> = ConcreteInterp<'ir, Stage, HighLevel, V, MultiCursor<V>>;

// Multi-stage concrete: tries HighLevel first, falls back to LowLevel.
// MultiCursor does NOT implement SingleStageCursorFor<HighLevel>.
impl<'ir, V> CallSeam<HighLevel> for MultiInterp<'ir, V>
where
    V: ToyVal,
{
    fn eval_call(
        &mut self,
        op: &kirin_function::Call<<HighLevel as kirin::prelude::Dialect>::Type>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        let args = self.read_many(op.args())?;
        let target = op.target();
        let current = self.current_stage();
        if let Ok(callee) = self.resolve_function_for::<HighLevel>(target, current) {
            Ok(Control::Call {
                callee,
                stage: current,
                args,
                results: op.results().to_vec(),
            })
        } else {
            let lowered = self
                .pipeline()
                .stage_by_name("lowered")
                .ok_or(InterpreterError::MissingEntry)?;
            let callee =
                self.resolve_function_cross_stage::<HighLevel, LowLevel>(target, current, lowered)?;
            Ok(Control::Call {
                callee,
                stage: lowered,
                args,
                results: op.results().to_vec(),
            })
        }
    }
}

// Multi-stage concrete: LowLevel calls — tries LowLevel first, falls back to HighLevel.
// This enables LowLevel entry points to call HighLevel (source-stage) functions.
// MultiCursor does NOT implement SingleStageCursorFor<LowLevel>.
impl<'ir, V> CallSeam<LowLevel> for MultiInterp<'ir, V>
where
    V: ToyVal,
{
    fn eval_call(
        &mut self,
        op: &kirin_function::Call<<LowLevel as kirin::prelude::Dialect>::Type>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        let args = self.read_many(op.args())?;
        let target = op.target();
        let current = self.current_stage();
        if let Ok(callee) = self.resolve_function_for::<LowLevel>(target, current) {
            Ok(Control::Call {
                callee,
                stage: current,
                args,
                results: op.results().to_vec(),
            })
        } else {
            let source = self
                .pipeline()
                .stage_by_name("source")
                .ok_or(InterpreterError::MissingEntry)?;
            let callee =
                self.resolve_function_cross_stage::<LowLevel, HighLevel>(target, current, source)?;
            Ok(Control::Call {
                callee,
                stage: source,
                args,
                results: op.results().to_vec(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// AbstractCallDispatch — single-stage abstract interpreters
// ---------------------------------------------------------------------------

impl<V: Clone> AbstractCallDispatch<V, LowLevelAbstract<V>> for Stage {
    fn make_abstract_cursor(
        _pipeline: &Pipeline<Stage>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> LowLevelAbstract<V> {
        LowLevelAbstract(AbstractBlockCursor::new(block, stage_id, args))
    }

    fn entry_block_for(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        PipelineHandle::new(pipeline, stage_id).entry_block_of::<LowLevel>(callee, stage_id)
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
        PipelineHandle::new(pipeline, stage_id).entry_block_of::<HighLevel>(callee, stage_id)
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

impl<V: Clone> Project<AbstractBlockCursor<V, HighLevel>> for AbstractMultiCursor<V> {
    fn try_project(self) -> Result<AbstractBlockCursor<V, HighLevel>, Self> {
        match self {
            AbstractMultiCursor::HighBlock(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<V: Clone> Project<AbstractBlockCursor<V, LowLevel>> for AbstractMultiCursor<V> {
    fn try_project(self) -> Result<AbstractBlockCursor<V, LowLevel>, Self> {
        match self {
            AbstractMultiCursor::Low(c) => Ok(c),
            other => Err(other),
        }
    }
}

impl<E, V> Execute<E> for AbstractMultiCursor<V>
where
    V: AbstractToyVal,
    E: AbstractEnv<Value = V, Ext = CursorExt<AbstractMultiCursor<V>>>,
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
        let handle = PipelineHandle::new(pipeline, stage_id);
        match pipeline.stage(stage_id) {
            Some(Stage::Source(_)) => handle.entry_block_of::<HighLevel>(callee, stage_id),
            Some(Stage::Lowered(_)) => handle.entry_block_of::<LowLevel>(callee, stage_id),
            None => Err(InterpreterError::MissingEntry),
        }
    }
}

pub type AbstractMultiInterp<'ir, V> =
    AbstractInterp<'ir, Stage, HighLevel, V, AbstractMultiCursor<V>>;

// Multi-stage abstract: HighLevel calls — tries HighLevel first, falls back to LowLevel.
// AbstractMultiCursor does NOT implement SingleStageCursorFor<HighLevel>.
impl<'ir, V> CallSeam<HighLevel> for AbstractMultiInterp<'ir, V>
where
    V: AbstractToyVal,
{
    fn eval_call(
        &mut self,
        op: &kirin_function::Call<<HighLevel as kirin::prelude::Dialect>::Type>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        let args = self.read_many(op.args())?;
        let target = op.target();
        let current = self.current_stage();
        if let Ok(callee) = self.resolve_function_for::<HighLevel>(target, current) {
            Ok(Control::Call {
                callee,
                stage: current,
                args,
                results: op.results().to_vec(),
            })
        } else {
            let lowered = self
                .pipeline()
                .stage_by_name("lowered")
                .ok_or(InterpreterError::MissingEntry)?;
            let callee =
                self.resolve_function_cross_stage::<HighLevel, LowLevel>(target, current, lowered)?;
            Ok(Control::Call {
                callee,
                stage: lowered,
                args,
                results: op.results().to_vec(),
            })
        }
    }
}

// Multi-stage abstract: LowLevel calls — tries LowLevel first, falls back to HighLevel.
// AbstractMultiCursor does NOT implement SingleStageCursorFor<LowLevel>.
impl<'ir, V> CallSeam<LowLevel> for AbstractMultiInterp<'ir, V>
where
    V: AbstractToyVal,
{
    fn eval_call(
        &mut self,
        op: &kirin_function::Call<<LowLevel as kirin::prelude::Dialect>::Type>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        let args = self.read_many(op.args())?;
        let target = op.target();
        let current = self.current_stage();
        if let Ok(callee) = self.resolve_function_for::<LowLevel>(target, current) {
            Ok(Control::Call {
                callee,
                stage: current,
                args,
                results: op.results().to_vec(),
            })
        } else {
            let source = self
                .pipeline()
                .stage_by_name("source")
                .ok_or(InterpreterError::MissingEntry)?;
            let callee =
                self.resolve_function_cross_stage::<LowLevel, HighLevel>(target, current, source)?;
            Ok(Control::Call {
                callee,
                stage: source,
                args,
                results: op.results().to_vec(),
            })
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
    use kirin_interpreter_16::abstract_interp::AbstractInterp;
    use kirin_interpreter_16::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use crate::interpreter16::{
        AbstractMultiCursor, AbstractMultiInterp, HighLevelAbstractCursor, HighLevelCursor,
        LowLevelAbstract, MultiInterp,
    };
    use crate::language::{HighLevel, LowLevel};
    use crate::stage::Stage;

    use super::*;

    fn build_pipeline(src: &str) -> Pipeline<Stage> {
        let mut p = Pipeline::new();
        ParsePipelineText::parse(&mut p, src).expect("parse failed");
        p
    }

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
        let mut interp: ConcreteInterp<'ir, Stage, HighLevel, i64, HighLevelCursor<i64>> =
            ConcreteInterp::new(pipeline, stage_id);
        interp.run_function::<HighLevel>(spec, args).unwrap()
    }

    fn analyze_lowered<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: ToyVal + AbstractValue,
        LowLevel: Interpretable<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>>>,
        LowLevelAbstract<V>: Execute<AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>>>,
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
        let mut interp: AbstractInterp<'ir, Stage, LowLevel, V, LowLevelAbstract<V>> =
            AbstractInterp::new(pipeline, stage_id);
        interp
            .analyze(spec, stage_id, args)
            .expect("analysis failed")
    }

    fn analyze_highlevel<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: AbstractToyVal,
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

    fn analyze_multi<'ir, V>(
        pipeline: &'ir Pipeline<Stage>,
        func_name: &str,
        args: Vec<V>,
    ) -> Option<V>
    where
        V: AbstractToyVal,
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
        let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
        interp.run_function::<HighLevel>(spec, args).unwrap()
    }

    /// Run multi-stage interpreter from any stage, dispatching by stage name.
    /// Supports both fixed-source (HighLevel entry) and symmetric/dynamic entry.
    fn run_multi_from_stage<'ir>(
        pipeline: &'ir Pipeline<Stage>,
        stage_name: &str,
        func_name: &str,
        args: &[i64],
    ) -> Option<i64> {
        let stage_id = pipeline.stage_by_name(stage_name).unwrap();
        match pipeline.stage(stage_id).unwrap() {
            Stage::Source(stage_info) => {
                let spec = pipeline
                    .resolve_staged_function(func_name, stage_id)
                    .unwrap()
                    .get_info(stage_info)
                    .unwrap()
                    .unique_live_specialization()
                    .unwrap();
                let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
                interp.run_function::<HighLevel>(spec, args).unwrap()
            }
            Stage::Lowered(stage_info) => {
                let spec = pipeline
                    .resolve_staged_function(func_name, stage_id)
                    .unwrap()
                    .get_info(stage_info)
                    .unwrap()
                    .unique_live_specialization()
                    .unwrap();
                let mut interp: MultiInterp<'ir, i64> = MultiInterp::new(pipeline, stage_id);
                interp.run_function::<LowLevel>(spec, args).unwrap()
            }
        }
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
    // For-loop program: sum integers 0..n with a scf.for loop
    // -----------------------------------------------------------------------

    const FOR_SUM_SOURCE: &str = r#"
stage @source fn @sum_range(i64) -> i64;

specialize @source fn @sum_range(i64) -> i64 {
  ^entry(%n: i64) {
    %zero = constant 0 -> i64;
    %one = constant 1 -> i64;
    %result = for %zero in %zero..%n step %one iter_args(%zero) do ^body(%i: i64, %acc: i64) {
      %new_acc = add %acc, %i -> i64;
      yield %new_acc;
    } -> i64;
    ret %result;
  }
}
"#;

    // -----------------------------------------------------------------------
    // R9: Entry flexibility programs
    // -----------------------------------------------------------------------

    /// Program with @square in source and @lowered_main in lowered that calls @square.
    /// Tests that LowLevel entry can call HighLevel functions cross-stage.
    const LOWERED_CALLS_SOURCE_SRC: &str = r#"
stage @source fn @square(i64) -> i64;
stage @lowered fn @lowered_main(i64) -> i64;

specialize @source fn @square(i64) -> i64 {
  ^entry(%n: i64) {
    %r = mul %n, %n -> i64;
    ret %r;
  }
}

specialize @lowered fn @lowered_main(i64) -> i64 {
  ^entry(%n: i64) {
    %r = call @square(%n) -> i64;
    ret %r;
  }
}
"#;

    /// Program with @double in BOTH source and lowered.
    /// Used to test symmetric entry: entering from either stage should double the input.
    const SYMMETRIC_SRC: &str = r#"
stage @source fn @double(i64) -> i64;
stage @lowered fn @double(i64) -> i64;

specialize @source fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
    ret %r;
  }
}

specialize @lowered fn @double(i64) -> i64 {
  ^entry(%n: i64) {
    %r = add %n, %n -> i64;
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
    // ConstProp — extensibility probe (R8)
    //
    // Implemented entirely in toy-lang; no changes to interpreter or dialect
    // crates. Passes R8: extensibility probe.
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

    #[test]
    fn for_loop_sum_concrete() {
        let result = run_concrete_i64_highlevel(FOR_SUM_SOURCE, "sum_range", &[5i64]);
        assert_eq!(result, Some(10));
    }

    #[test]
    fn for_loop_sum_zero_iterations() {
        let result = run_concrete_i64_highlevel(FOR_SUM_SOURCE, "sum_range", &[0i64]);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn for_loop_abstract_converges() {
        let pipeline = build_pipeline(FOR_SUM_SOURCE);
        let result = analyze_highlevel::<ToyType>(&pipeline, "sum_range", vec![ToyType::I64]);
        assert_eq!(result, Some(ToyType::I64));
    }

    // -----------------------------------------------------------------------
    // Abstract tests (LowLevel / lowered stage, flat CF, Interval domain)
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
    // R9: Entry flexibility tests
    // -----------------------------------------------------------------------

    /// Concrete multi-stage: LowLevel entry calling a HighLevel (source) function.
    /// Verifies that fixed-source is not the only supported entry direction.
    #[test]
    fn lowered_entry_calls_source() {
        let pipeline = build_pipeline(LOWERED_CALLS_SOURCE_SRC);
        let result = run_multi_from_stage(&pipeline, "lowered", "lowered_main", &[5i64]);
        assert_eq!(result, Some(25)); // 5 * 5 = 25
    }

    /// Symmetric entry: enter from HighLevel (source) stage — same result as lowered entry.
    #[test]
    fn symmetric_entry_highlevel() {
        let pipeline = build_pipeline(SYMMETRIC_SRC);
        let result = run_multi_from_stage(&pipeline, "source", "double", &[7i64]);
        assert_eq!(result, Some(14));
    }

    /// Symmetric entry: enter from LowLevel (lowered) stage — same result as source entry.
    #[test]
    fn symmetric_entry_lowlevel() {
        let pipeline = build_pipeline(SYMMETRIC_SRC);
        let result = run_multi_from_stage(&pipeline, "lowered", "double", &[7i64]);
        assert_eq!(result, Some(14));
    }

    // -----------------------------------------------------------------------
    // Liveness analysis — extensibility probe (backward IR walker, R8+)
    //
    // Classical per-block liveness over SSA values in a single function body.
    // Implemented entirely in toy-lang as a standalone IR walker; no changes
    // to any framework crate.
    //
    // Equations (backward dataflow):
    //   use[B]      = SSA values read before being defined in B
    //   def[B]      = SSA values defined in B (block args + statement results)
    //   live_in[B]  = use[B] ∪ (live_out[B] − def[B])
    //   live_out[B] = ∪ live_in[S]  for each successor S of B
    //
    // Iteration: backward worklist until fixed point.
    // -----------------------------------------------------------------------

    use kirin::prelude::{Block, Dialect, GetInfo, SSAValue, StageInfo};
    use std::collections::{HashMap, HashSet, VecDeque};

    /// Per-function liveness result.
    struct LivenessResult {
        live_in: HashMap<Block, HashSet<SSAValue>>,
        live_out: HashMap<Block, HashSet<SSAValue>>,
    }

    /// Collect all blocks reachable via the region of a function-body statement.
    ///
    /// Returns `(blocks_in_order, successors_map)` where `successors_map[B]`
    /// is the list of CFG successor blocks of B.
    fn collect_blocks_and_succs<L: Dialect>(
        body_stmt: kirin::prelude::Statement,
        stage: &StageInfo<L>,
    ) -> (Vec<Block>, HashMap<Block, Vec<Block>>) {
        // The body statement's first region contains all blocks.
        let region = body_stmt
            .regions(stage)
            .next()
            .expect("function body must have a region");
        let blocks: Vec<Block> = region.blocks(stage).collect();

        // Build successor map: for each block, gather successors from its terminator.
        let mut succs: HashMap<Block, Vec<Block>> = HashMap::new();
        for &blk in &blocks {
            let mut blk_succs = Vec::new();
            if let Some(term) = blk.terminator(stage) {
                for succ in term.successors(stage) {
                    blk_succs.push(succ.target());
                }
            }
            succs.insert(blk, blk_succs);
        }
        (blocks, succs)
    }

    /// Compute `(use_set, def_set)` for a single block.
    ///
    /// - `def_set`: block arguments (defined at block entry) + result values of
    ///   all statements (including terminator).
    /// - `use_set`: SSA values referenced as operands before being locally defined.
    fn block_use_def<L: Dialect>(
        blk: Block,
        stage: &StageInfo<L>,
    ) -> (HashSet<SSAValue>, HashSet<SSAValue>) {
        let info = blk.expect_info(stage);
        let mut def_set: HashSet<SSAValue> = HashSet::new();
        let mut use_set: HashSet<SSAValue> = HashSet::new();

        // Block arguments are defined at block entry.
        for &ba in &info.arguments {
            def_set.insert(ba.into());
        }

        // Helper: process one statement's uses and defs in order.
        let mut process_stmt = |stmt: kirin::prelude::Statement| {
            // Uses: all SSA operands not yet locally defined.
            for &val in stmt.arguments(stage) {
                if !def_set.contains(&val) {
                    use_set.insert(val);
                }
            }
            // Defs: result values produced by this statement.
            for &rv in stmt.results(stage) {
                def_set.insert(rv.into());
            }
        };

        // Process non-terminator statements first.
        for stmt in blk.statements(stage) {
            process_stmt(stmt);
        }
        // Then the terminator (if present).
        if let Some(term) = blk.terminator(stage) {
            process_stmt(term);
        }

        (use_set, def_set)
    }

    /// Run classical liveness analysis over a single function.
    ///
    /// `body_stmt` is the statement whose first region contains the function's
    /// blocks (i.e. `SpecializedFunctionInfo::body()`).
    fn analyze_liveness<L: Dialect>(
        body_stmt: kirin::prelude::Statement,
        stage: &StageInfo<L>,
    ) -> LivenessResult {
        let (blocks, succs) = collect_blocks_and_succs(body_stmt, stage);

        // Pre-compute use/def sets for every block.
        let mut use_def: HashMap<Block, (HashSet<SSAValue>, HashSet<SSAValue>)> = HashMap::new();
        for &blk in &blocks {
            use_def.insert(blk, block_use_def(blk, stage));
        }

        // Build predecessor map (needed for worklist ordering; optional but helpful).
        let mut preds: HashMap<Block, Vec<Block>> = HashMap::new();
        for &blk in &blocks {
            preds.entry(blk).or_default();
        }
        for (&blk, blk_succs) in &succs {
            for &s in blk_succs {
                preds.entry(s).or_default().push(blk);
            }
        }

        // Initialise all live_in / live_out to empty.
        let mut live_in: HashMap<Block, HashSet<SSAValue>> =
            blocks.iter().map(|&b| (b, HashSet::new())).collect();
        let mut live_out: HashMap<Block, HashSet<SSAValue>> =
            blocks.iter().map(|&b| (b, HashSet::new())).collect();

        // Backward worklist: seed with all blocks.
        let mut worklist: VecDeque<Block> = blocks.iter().copied().collect();

        while let Some(blk) = worklist.pop_front() {
            // live_out[blk] = ∪ live_in[s] for each successor s
            let new_out: HashSet<SSAValue> = succs[&blk]
                .iter()
                .flat_map(|s| live_in[s].iter().copied())
                .collect();

            // live_in[blk] = use[blk] ∪ (live_out[blk] − def[blk])
            let (use_set, def_set) = &use_def[&blk];
            let new_in: HashSet<SSAValue> = use_set
                .iter()
                .copied()
                .chain(new_out.iter().copied().filter(|v| !def_set.contains(v)))
                .collect();

            let changed = new_in != live_in[&blk] || new_out != live_out[&blk];
            live_in.insert(blk, new_in);
            live_out.insert(blk, new_out);

            if changed {
                // Re-add predecessors to the worklist.
                for &pred in &preds[&blk] {
                    if !worklist.contains(&pred) {
                        worklist.push_back(pred);
                    }
                }
            }
        }

        LivenessResult { live_in, live_out }
    }

    /// Convenience: run liveness on a named function in the `lowered` stage.
    fn liveness_for_lowered_fn(pipeline: &Pipeline<Stage>, func_name: &str) -> LivenessResult {
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
        let body_stmt = *spec.get_info(stage_info).unwrap().body();
        analyze_liveness(body_stmt, stage_info)
    }

    // -----------------------------------------------------------------------
    // Liveness tests
    // -----------------------------------------------------------------------

    /// For `ADD_LOWERED` (single block):
    ///   - %a and %b are block arguments of ^entry, so they are in def[^entry].
    ///   - `add %a, %b` uses %a and %b → they appear in use[^entry] (upward-exposed
    ///     before any local def within the body — but block-arg defs come first in
    ///     our def_set, so they are NOT in use[^entry]).
    ///
    /// Wait — %a and %b are block *arguments*, so they are in def[entry] from the
    /// start. They are NOT in use[entry] because they are defined before any use in
    /// the scanning order (block args → statements).
    ///
    /// What we really want to check is that live_in[^entry] contains %a and %b
    /// when they are used and not passed in as block args.
    ///
    /// For ADD_LOWERED the function has exactly one block (^entry).  The block args
    /// are %a and %b; the body has `%result = add %a, %b` then `ret %result`.
    /// - def[^entry] = {%a, %b, %result}
    /// - use[^entry] = {} (all uses are of locally-defined values)
    /// - live_out[^entry] = {} (no successors)
    /// - live_in[^entry] = {} (use ∪ (live_out − def) = {} ∪ {} = {})
    ///
    /// This confirms that in a single-block function with no cross-block uses,
    /// everything is dead at block exit and live_in is empty.
    #[test]
    fn liveness_add_args_live_at_entry() {
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = liveness_for_lowered_fn(&pipeline, "add");

        // ADD_LOWERED has one block. Its live_in should be empty:
        // %a and %b are block args (locally defined), so they're in def[^entry],
        // not use[^entry]. The single block has no successors → live_out = {}.
        assert_eq!(result.live_in.len(), 1, "ADD_LOWERED should have 1 block");
        let (_, live_in) = result.live_in.iter().next().unwrap();
        assert!(
            live_in.is_empty(),
            "all values in ADD_LOWERED are locally defined; live_in must be empty"
        );
        let (_, live_out) = result.live_out.iter().next().unwrap();
        assert!(
            live_out.is_empty(),
            "single-exit block has no successors; live_out must be empty"
        );
    }

    /// For `BRANCH_LOWERED` (sign function, 3 blocks):
    ///
    ///   ^entry(%x):
    ///     %zero = constant 0
    ///     %is_neg = lt %x, %zero
    ///     cond_br %is_neg then=^neg() else=^pos()
    ///
    ///   ^neg():  %one = constant 1; ret %one
    ///   ^pos():  %zero2 = constant 0; ret %zero2
    ///
    /// - def[^entry] = {%x, %zero, %is_neg}
    /// - use[^entry] = {} (%x is a block arg → def before use in scan order)
    /// - succs(^entry) = {^neg, ^pos}
    /// - live_in[^neg] = {} (all defs local, no succs)
    /// - live_in[^pos] = {} (all defs local, no succs)
    /// - live_out[^entry] = live_in[^neg] ∪ live_in[^pos] = {}
    /// - live_in[^entry] = {} ∪ ({} − def[^entry]) = {}
    ///
    /// Key assertion: %x is NOT live-out of ^entry because it is not used in
    /// any successor (^neg and ^pos define their own constants without referencing %x).
    #[test]
    fn liveness_dead_after_use() {
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = liveness_for_lowered_fn(&pipeline, "sign");

        // BRANCH_LOWERED has 3 blocks.
        assert_eq!(
            result.live_in.len(),
            3,
            "BRANCH_LOWERED should have 3 blocks"
        );

        // All three blocks: live_in and live_out should be empty because
        // each block defines its own constants and no block arg crosses a block boundary.
        for (_blk, li) in &result.live_in {
            assert!(
                li.is_empty(),
                "no value crosses a block boundary in BRANCH_LOWERED; all live_in must be empty"
            );
        }
        for (_blk, lo) in &result.live_out {
            assert!(
                lo.is_empty(),
                "no value crosses a block boundary in BRANCH_LOWERED; all live_out must be empty"
            );
        }
    }

    /// For `FACTORIAL_LOWERED`, %n from ^entry IS used in ^recurse (cross-block).
    ///
    ///   ^entry(%n):
    ///     %one = constant 1
    ///     %is_base = le %n, %one
    ///     cond_br %is_base then=^base() else=^recurse()
    ///
    ///   ^base():  %one2 = constant 1; ret %one2
    ///   ^recurse():
    ///     %one3 = constant 1
    ///     %n_minus_1 = sub %n, %one3     ← uses %n from ^entry!
    ///     %rec = call @factorial(%n_minus_1)
    ///     %prod = mul %n, %rec           ← uses %n again
    ///     ret %prod
    ///
    /// %n is a block argument of ^entry (in def[^entry]).
    /// ^recurse uses %n without defining it → %n ∈ use[^recurse].
    /// Therefore %n ∈ live_in[^recurse].
    /// Since ^entry → ^recurse: %n ∈ live_out[^entry].
    /// But %n ∈ def[^entry], so %n ∉ live_in[^entry].
    #[test]
    fn liveness_cross_block_use_in_factorial() {
        let pipeline = build_pipeline(FACTORIAL_LOWERED);
        let stage_id = pipeline.stage_by_name("lowered").unwrap();
        let stage_info: &StageInfo<LowLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function("factorial", stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let spec_info = spec.get_info(stage_info).unwrap();
        let body_stmt = *spec_info.body();
        let result = analyze_liveness(body_stmt, stage_info);

        // Collect the entry block (first block in the region).
        let region = body_stmt.regions(stage_info).next().unwrap();
        let mut block_iter = region.blocks(stage_info);
        let entry_block = block_iter.next().expect("entry block must exist");
        let base_block = block_iter.next().expect("base block must exist");
        let recurse_block = block_iter.next().expect("recurse block must exist");

        // Get %n: the first block argument of ^entry.
        let entry_info = entry_block.expect_info(stage_info);
        let n_ssa: SSAValue = entry_info.arguments[0].into();

        // %n must be live-in of ^recurse (cross-block use).
        assert!(
            result.live_in[&recurse_block].contains(&n_ssa),
            "%n must be live-in of ^recurse because it is used there without local definition"
        );

        // %n must be live-out of ^entry (because ^recurse needs it).
        assert!(
            result.live_out[&entry_block].contains(&n_ssa),
            "%n must be live-out of ^entry because the recurse successor uses it"
        );

        // %n must NOT be live-in of ^entry (it is defined there as a block arg).
        assert!(
            !result.live_in[&entry_block].contains(&n_ssa),
            "%n is defined by ^entry as a block arg; it must not appear in live_in[^entry]"
        );

        // %n must NOT be live in ^base (^base never references %n).
        assert!(
            !result.live_in[&base_block].contains(&n_ssa),
            "%n is not used in ^base; it must not appear in live_in[^base]"
        );
    }

    // -----------------------------------------------------------------------
    // Extensibility probe: ConstProp analysis (R8)
    // -----------------------------------------------------------------------

    #[test]
    fn constprop_add_two_constants() {
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
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(5)]);
        assert_eq!(result, Some(ConstProp::Const(0)));
    }

    #[test]
    fn constprop_branch_negative_input() {
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Const(-3)]);
        assert_eq!(result, Some(ConstProp::Const(1)));
    }

    #[test]
    fn constprop_branch_unknown_joins_both_paths() {
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = analyze_lowered::<ConstProp>(&pipeline, "sign", vec![ConstProp::Top]);
        assert_eq!(result, Some(ConstProp::Top));
    }
}
