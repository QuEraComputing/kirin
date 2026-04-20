use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, Symbol,
};
use kirin_arith::{ArithType, ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_18::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_18::abstract_interp::{
    AbstractFrame, AbstractInterp, FuncState, FuncSummary, StagedKey,
};
use kirin_interpreter_18::algebra::{Lift, Project, SingleStageCursorFor};
use kirin_interpreter_18::call_dispatch::CallDispatch;
use kirin_interpreter_18::concrete::ConcreteInterp;
use kirin_interpreter_18::control::{Control, CursorExt};
use kirin_interpreter_18::cursor::{AbstractBlockCursor, BlockCursor};
use kirin_interpreter_18::dispatch::Dispatch;
use kirin_interpreter_18::env::{AbstractEnv, AbstractMode, ConcreteMode, Env};
use kirin_interpreter_18::error::InterpreterError;
use kirin_interpreter_18::execute::{Execute, StackEntry};
use kirin_interpreter_18::frame::Frame;
use kirin_interpreter_18::interpretable::Interpretable;
use kirin_interpreter_18::pipeline::PipelineHandle;
use kirin_scf::interpreter18::cursor::{
    AbstractForCursor, AbstractIfCursor, AbstractSCFCursor, ForCursor, IfCursor, SCFCursor,
};
use kirin_scf::interpreter18::interpret::ScfSeam;
use kirin_scf::{For, ForLoopValue, If};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

// ---------------------------------------------------------------------------
// ToyVal — trait alias collapsing value bounds.
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
// HighLevel: Interpretable<E>
//
// Key interpreter-18 change: bound is `E: Dispatch` (not `CallSeam<HighLevel>`).
// The Lexical arm now delegates entirely to `op.eval(env)` — Call dispatch is
// handled generically inside `kirin_function::interpreter18`.
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for HighLevel
where
    E: Dispatch + Env<Value = V>,
    E::Stages: HasStageInfo<HighLevel>,
    E::Error: From<InterpreterError>,
    V: ToyVal,
    kirin_scf::StructuredControlFlow<ArithType>: Interpretable<E>,
{
    fn eval(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            HighLevel::Lexical(op) => op.eval(env),
            HighLevel::Structured(op) => op.eval(env),
            HighLevel::Constant(op) => op.eval(env),
            HighLevel::Arith(op) => op.eval(env),
            HighLevel::Cmp(op) => op.eval(env),
            HighLevel::Bitwise(op) => op.eval(env),
        }
    }
}

// ---------------------------------------------------------------------------
// LowLevel: Interpretable<E>
//
// Key interpreter-18 change: bound is `E: Dispatch` (not `CallSeam<LowLevel>`).
// The Lifted arm now delegates entirely to `op.eval(env)`.
// ---------------------------------------------------------------------------

impl<E, V> Interpretable<E> for LowLevel
where
    E: Dispatch + Env<Value = V>,
    V: ToyVal,
    E::Stages: HasStageInfo<LowLevel>,
    E::Error: From<InterpreterError>,
{
    fn eval(&self, env: &mut E) -> Result<Control<V, E::Ext>, E::Error> {
        match self {
            LowLevel::Lifted(op) => op.eval(env),
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

// ---------------------------------------------------------------------------
// MultiInterp — local newtype wrapping ConcreteInterp.
//
// A type alias `type MultiInterp<'ir, V> = ConcreteInterp<...>` is transparent
// to the orphan rule, making `impl Dispatch for MultiInterp` an orphan (both
// `Dispatch` and `ConcreteInterp` are from kirin_interpreter_18). By using a
// newtype struct, `MultiInterp` becomes a LOCAL type, satisfying the orphan
// rule for a single `Dispatch` impl that routes by runtime `caller_stage`.
// This eliminates the two `CallSeam<HighLevel>` + `CallSeam<LowLevel>` impls
// that interpreter-17 required.
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct MultiInterp<'ir, V: Clone>(pub ConcreteInterp<'ir, Stage, HighLevel, V, MultiCursor<V>>);

#[allow(dead_code)]
impl<'ir, V: Clone> MultiInterp<'ir, V> {
    pub fn new(pipeline: &'ir Pipeline<Stage>, stage_id: CompileStage) -> Self {
        Self(ConcreteInterp::new(pipeline, stage_id))
    }
}

impl<'ir, V: Clone> Env for MultiInterp<'ir, V> {
    type Mode = ConcreteMode<MultiCursor<V>>;
    type Value = V;
    type Ext = CursorExt<MultiCursor<V>>;
    type Error = InterpreterError;
    type Stages = Stage;

    fn current_stage(&self) -> CompileStage {
        self.0.current_stage()
    }
    fn pipeline(&self) -> &Pipeline<Stage> {
        self.0.pipeline()
    }
    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.0.read(ssa)
    }
    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.0.write_result(r, v)
    }
    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.0.write_ssa(ssa, v)
    }
}

impl<'ir, V: ToyVal> Dispatch for MultiInterp<'ir, V> {
    fn dispatch_call(
        &mut self,
        target: Symbol,
        caller_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        match self.pipeline().stage(caller_stage) {
            Some(Stage::Source(_)) => {
                if let Ok(callee) = self.resolve_function_for::<HighLevel>(target, caller_stage) {
                    Ok(Control::Call {
                        callee,
                        stage: caller_stage,
                        args,
                        results,
                    })
                } else {
                    let lowered = self
                        .pipeline()
                        .stage_by_name("lowered")
                        .ok_or(InterpreterError::MissingEntry)?;
                    let callee = self.resolve_function_cross_stage::<HighLevel, LowLevel>(
                        target,
                        caller_stage,
                        lowered,
                    )?;
                    Ok(Control::Call {
                        callee,
                        stage: lowered,
                        args,
                        results,
                    })
                }
            }
            Some(Stage::Lowered(_)) => {
                if let Ok(callee) = self.resolve_function_for::<LowLevel>(target, caller_stage) {
                    Ok(Control::Call {
                        callee,
                        stage: caller_stage,
                        args,
                        results,
                    })
                } else {
                    let source = self
                        .pipeline()
                        .stage_by_name("source")
                        .ok_or(InterpreterError::MissingEntry)?;
                    let callee = self.resolve_function_cross_stage::<LowLevel, HighLevel>(
                        target,
                        caller_stage,
                        source,
                    )?;
                    Ok(Control::Call {
                        callee,
                        stage: source,
                        args,
                        results,
                    })
                }
            }
            None => Err(InterpreterError::MissingEntry),
        }
    }
}

impl<'ir, V: ToyVal> ScfSeam<kirin_arith::ArithType> for MultiInterp<'ir, V> {
    fn eval_if(
        &mut self,
        op: &If<kirin_arith::ArithType>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        self.0.eval_if(op)
    }
    fn eval_for(
        &mut self,
        op: &For<kirin_arith::ArithType>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        self.0.eval_for(op)
    }
}

#[allow(dead_code)]
impl<'ir, V: ToyVal> MultiInterp<'ir, V> {
    pub fn run_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<Option<V>, InterpreterError>
    where
        Stage: HasStageInfo<LD>,
        BlockCursor<V, LD>: Lift<MultiCursor<V>>,
    {
        let stage_id = self.0.handle.stage_id;
        let pipeline = self.0.handle.pipeline;
        let entry =
            PipelineHandle::new(pipeline, stage_id).entry_block_of::<LD>(callee, stage_id)?;
        let cursor = BlockCursor::<V, LD>::new(entry, stage_id, args.to_vec());
        let frame = Frame::new(callee, stage_id, vec![]);
        self.0.frames.push(frame)?;
        self.0.cursors.push(StackEntry::new(cursor.lift()));
        while self.step()? {}
        Ok(self.0.result.take())
    }

    fn step(&mut self) -> Result<bool, InterpreterError> {
        let Some(mut entry) = self.0.cursors.pop() else {
            return Ok(false);
        };
        let inbox = entry.inbox.take();
        let effect: Control<V, CursorExt<MultiCursor<V>>> = entry.cursor.execute(self, inbox)?;
        match effect {
            Control::Advance => {
                self.0.cursors.push(entry);
            }
            Control::Jump(..) => {
                self.0.cursors.push(entry);
            }
            Control::Ext(CursorExt::Push(new_cursor)) => {
                self.0.cursors.push(entry);
                self.0.cursors.push(StackEntry::new(new_cursor));
            }
            Control::Ext(CursorExt::Pop) => {}
            Control::Yield(v) => {
                if let Some(parent) = self.0.cursors.last_mut() {
                    parent.inbox = Some(v);
                } else {
                    self.0.result = Some(v);
                }
            }
            Control::Return(v) => {
                let frame = self.0.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.0.frames.is_empty() {
                    self.0.result = Some(v);
                } else {
                    self.write_results(&caller_results, v)?;
                }
            }
            Control::Call {
                callee,
                stage,
                args,
                results,
            } => {
                self.0.cursors.push(entry);
                let pipeline = self.0.handle.pipeline;
                let cursor = Stage::make_call_cursor(pipeline, callee, stage, args)?;
                let frame = Frame::new(callee, stage, results);
                self.0.frames.push(frame)?;
                self.0.cursors.push(StackEntry::new(cursor));
            }
            Control::Fork(..) => {
                return Err(InterpreterError::UnhandledEffect(
                    "Control::Fork in concrete interpreter; use AbstractInterp for nondeterminism"
                        .into(),
                ));
            }
        }
        Ok(true)
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

// ---------------------------------------------------------------------------
// AbstractMultiInterp — local newtype wrapping AbstractInterp.
//
// Same orphan-rule fix as MultiInterp: the newtype is LOCAL so we can impl
// Dispatch. The fixpoint loop is inlined here (not delegated to
// AbstractInterp::analyze) because the cursor's Execute takes `&mut E` and
// we need E = AbstractMultiInterp (which has Dispatch) — not the inner
// AbstractInterp (which lacks a multi-stage Dispatch impl).
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub struct AbstractMultiInterp<'ir, V: Clone>(
    pub AbstractInterp<'ir, Stage, HighLevel, V, AbstractMultiCursor<V>>,
);

#[allow(dead_code)]
impl<'ir, V: Clone + AbstractValue> AbstractMultiInterp<'ir, V> {
    pub fn new(pipeline: &'ir Pipeline<Stage>, stage_id: CompileStage) -> Self {
        Self(AbstractInterp::new(pipeline, stage_id))
    }
}

impl<'ir, V: Clone + AbstractValue> Env for AbstractMultiInterp<'ir, V> {
    type Mode = AbstractMode<AbstractMultiCursor<V>>;
    type Value = V;
    type Ext = CursorExt<AbstractMultiCursor<V>>;
    type Error = InterpreterError;
    type Stages = Stage;

    fn current_stage(&self) -> CompileStage {
        self.0.current_stage()
    }
    fn pipeline(&self) -> &Pipeline<Stage> {
        self.0.pipeline()
    }
    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.0.read(ssa)
    }
    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.0.write_result(r, v)
    }
    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.0.write_ssa(ssa, v)
    }
}

impl<'ir, V: Clone + AbstractValue> AbstractEnv for AbstractMultiInterp<'ir, V> {
    fn enqueue_block(&mut self, block: Block, args: Vec<V>) {
        self.0.enqueue_block(block, args);
    }
    fn record_return(&mut self, v: V) -> Result<(), InterpreterError> {
        self.0.record_return(v)
    }
    fn current_function(&self) -> SpecializedFunction {
        self.0.current_function()
    }
}

impl<'ir, V: AbstractToyVal> Dispatch for AbstractMultiInterp<'ir, V> {
    fn dispatch_call(
        &mut self,
        target: Symbol,
        caller_stage: CompileStage,
        args: Vec<V>,
        results: Vec<ResultValue>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        match self.pipeline().stage(caller_stage) {
            Some(Stage::Source(_)) => {
                if let Ok(callee) = self.resolve_function_for::<HighLevel>(target, caller_stage) {
                    Ok(Control::Call {
                        callee,
                        stage: caller_stage,
                        args,
                        results,
                    })
                } else {
                    let lowered = self
                        .pipeline()
                        .stage_by_name("lowered")
                        .ok_or(InterpreterError::MissingEntry)?;
                    let callee = self.resolve_function_cross_stage::<HighLevel, LowLevel>(
                        target,
                        caller_stage,
                        lowered,
                    )?;
                    Ok(Control::Call {
                        callee,
                        stage: lowered,
                        args,
                        results,
                    })
                }
            }
            Some(Stage::Lowered(_)) => {
                if let Ok(callee) = self.resolve_function_for::<LowLevel>(target, caller_stage) {
                    Ok(Control::Call {
                        callee,
                        stage: caller_stage,
                        args,
                        results,
                    })
                } else {
                    let source = self
                        .pipeline()
                        .stage_by_name("source")
                        .ok_or(InterpreterError::MissingEntry)?;
                    let callee = self.resolve_function_cross_stage::<LowLevel, HighLevel>(
                        target,
                        caller_stage,
                        source,
                    )?;
                    Ok(Control::Call {
                        callee,
                        stage: source,
                        args,
                        results,
                    })
                }
            }
            None => Err(InterpreterError::MissingEntry),
        }
    }
}

impl<'ir, V: AbstractToyVal> ScfSeam<kirin_arith::ArithType> for AbstractMultiInterp<'ir, V> {
    fn eval_if(
        &mut self,
        op: &If<kirin_arith::ArithType>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        self.0.eval_if(op)
    }
    fn eval_for(
        &mut self,
        op: &For<kirin_arith::ArithType>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        self.0.eval_for(op)
    }
}

#[allow(dead_code)]
impl<'ir, V: AbstractToyVal> AbstractMultiInterp<'ir, V> {
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        let entry_block =
            <Stage as AbstractCallDispatch<V, AbstractMultiCursor<V>>>::entry_block_for(
                self.0.handle.pipeline,
                entry_fn,
                stage_id,
            )?;
        let entry_key = (entry_fn, stage_id);
        self.0.summaries.insert(
            entry_key,
            FuncSummary {
                input: args,
                output: None,
                entry_block,
            },
        );
        self.0.func_worklist.push(entry_key);
        while let Some(key) = self.0.func_worklist.pop() {
            self.analyze_function(key)?;
        }
        Ok(self
            .0
            .summaries
            .get(&entry_key)
            .and_then(|s| s.output.clone()))
    }

    fn analyze_function(&mut self, key: StagedKey) -> Result<(), InterpreterError> {
        let (_, func_stage) = key;
        let (entry_block, input) = {
            let s = self
                .0
                .summaries
                .get(&key)
                .ok_or(InterpreterError::MissingEntry)?;
            (s.entry_block, s.input.clone())
        };
        let mut state = FuncState::new();
        state.block_in.insert(entry_block, input);
        state.block_worklist.push(entry_block);
        self.0.func_states.insert(key, state);
        self.0.current_key = Some(key);
        loop {
            while !self.0.cursor_stack.is_empty() {
                self.step_cursor_abstract(key)?;
            }
            let block = self
                .0
                .func_states
                .get_mut(&key)
                .and_then(|s| s.block_worklist.pop());
            let Some(block) = block else { break };
            let block_args = self
                .0
                .func_states
                .get(&key)
                .and_then(|s| s.block_in.get(&block).cloned())
                .unwrap_or_default();
            let cursor =
                Stage::make_abstract_cursor(self.0.handle.pipeline, func_stage, block, block_args);
            self.0.cursor_stack.push(StackEntry::new(cursor));
            while !self.0.cursor_stack.is_empty() {
                self.step_cursor_abstract(key)?;
            }
        }
        self.0.current_key = None;
        Ok(())
    }

    fn step_cursor_abstract(&mut self, key: StagedKey) -> Result<(), InterpreterError> {
        let Some(mut entry) = self.0.cursor_stack.pop() else {
            return Ok(());
        };
        let inbox = entry.inbox.take();
        let effect: Control<V, CursorExt<AbstractMultiCursor<V>>> =
            entry.cursor.execute(self, inbox)?;
        match effect {
            Control::Advance => {
                self.0.cursor_stack.push(entry);
            }
            Control::Ext(CursorExt::Push(new_cursor)) => {
                self.0.cursor_stack.push(entry);
                self.0.cursor_stack.push(StackEntry::new(new_cursor));
            }
            Control::Ext(CursorExt::Pop) => {}
            Control::Yield(v) => {
                if let Some(parent) = self.0.cursor_stack.last_mut() {
                    parent.inbox = Some(v);
                }
            }
            Control::Return(v) => {
                self.0.cursor_stack.clear();
                self.0.record_return_inner(key, v)?;
            }
            Control::Jump(block, args) => {
                self.enqueue_block(block, args);
            }
            Control::Fork(branches) => {
                for (block, args) in branches {
                    self.enqueue_block(block, args);
                }
            }
            Control::Call {
                callee,
                stage: callee_stage,
                args,
                results,
            } => {
                self.0.cursor_stack.push(entry);
                let call_result =
                    self.handle_abstract_call(key, callee, callee_stage, &results, args)?;
                self.write_results(&results, call_result)?;
            }
        }
        Ok(())
    }

    fn handle_abstract_call(
        &mut self,
        caller_key: StagedKey,
        callee: SpecializedFunction,
        callee_stage: CompileStage,
        call_site_results: &[ResultValue],
        new_args: Vec<V>,
    ) -> Result<V, InterpreterError> {
        let callee_key = (callee, callee_stage);
        let frame = AbstractFrame {
            func: caller_key.0,
            stage: caller_key.1,
            results: call_site_results.to_vec(),
        };
        self.0
            .call_graph
            .entry(callee_key)
            .or_default()
            .insert(frame);
        if let Some(summary) = self.0.summaries.get(&callee_key) {
            let existing_input = summary.input.clone();
            if existing_input.len() != new_args.len() {
                return Err(InterpreterError::ArityMismatch {
                    expected: existing_input.len(),
                    got: new_args.len(),
                });
            }
            let widening = self.0.widening;
            let fn_visits = *self.0.fn_visit_counts.get(&callee_key).unwrap_or(&0);
            let merged: Vec<V> = existing_input
                .iter()
                .zip(new_args.iter())
                .map(|(e, a)| widening.merge(e, a, fn_visits))
                .collect();
            let input_grew = merged
                .iter()
                .zip(existing_input.iter())
                .any(|(n, o)| !n.is_subseteq(o));
            if input_grew {
                self.0.summaries.get_mut(&callee_key).unwrap().input = merged;
                *self.0.fn_visit_counts.entry(callee_key).or_insert(0) += 1;
                self.0.func_worklist.push(callee_key);
            }
            Ok(self
                .0
                .summaries
                .get(&callee_key)
                .unwrap()
                .output
                .clone()
                .unwrap_or_else(V::bottom))
        } else {
            let entry_block =
                <Stage as AbstractCallDispatch<V, AbstractMultiCursor<V>>>::entry_block_for(
                    self.0.handle.pipeline,
                    callee,
                    callee_stage,
                )?;
            self.0.summaries.insert(
                callee_key,
                FuncSummary {
                    input: new_args,
                    output: None,
                    entry_block,
                },
            );
            self.0.func_worklist.push(callee_key);
            Ok(V::bottom())
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
    use kirin_interpreter_18::abstract_interp::AbstractInterp;
    use kirin_interpreter_18::concrete::ConcreteInterp;
    use kirin_interval::Interval;

    use crate::interpreter18::{
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

    // 3-arg function where %c is unused — for sparse AI tests
    const SPARSE_PROG: &str = r#"
stage @lowered fn @maybe_add(i64, i64, i64) -> i64;

specialize @lowered fn @maybe_add(i64, i64, i64) -> i64 {
  ^entry(%a: i64, %b: i64, %c: i64) {
    %result = add %a, %b -> i64;
    ret %result;
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

    #[test]
    fn lowered_entry_calls_source() {
        let pipeline = build_pipeline(LOWERED_CALLS_SOURCE_SRC);
        let result = run_multi_from_stage(&pipeline, "lowered", "lowered_main", &[5i64]);
        assert_eq!(result, Some(25));
    }

    #[test]
    fn symmetric_entry_highlevel() {
        let pipeline = build_pipeline(SYMMETRIC_SRC);
        let result = run_multi_from_stage(&pipeline, "source", "double", &[7i64]);
        assert_eq!(result, Some(14));
    }

    #[test]
    fn symmetric_entry_lowlevel() {
        let pipeline = build_pipeline(SYMMETRIC_SRC);
        let result = run_multi_from_stage(&pipeline, "lowered", "double", &[7i64]);
        assert_eq!(result, Some(14));
    }

    // -----------------------------------------------------------------------
    // Liveness analysis — extensibility probe (backward IR walker, R8+)
    // -----------------------------------------------------------------------

    use kirin::prelude::{Block, Dialect, GetInfo, SSAValue, StageInfo, Statement};
    use kirin_interpreter_18::backward::{BackwardFixpoint, BlockTransferBackward};
    use std::collections::{HashMap, HashSet};

    struct LivenessResult {
        live_in: HashMap<Block, HashSet<SSAValue>>,
        live_out: HashMap<Block, HashSet<SSAValue>>,
    }

    struct LivenessTransfer;

    impl<'ir> BlockTransferBackward<'ir> for LivenessTransfer {
        type Domain = HashSet<SSAValue>;

        fn join(a: &HashSet<SSAValue>, b: &HashSet<SSAValue>) -> HashSet<SSAValue> {
            a.union(b).copied().collect()
        }

        fn bottom() -> HashSet<SSAValue> {
            HashSet::new()
        }

        fn transfer_block<L: Dialect>(
            &self,
            block: Block,
            stage: &StageInfo<L>,
            live_out: HashSet<SSAValue>,
        ) -> HashSet<SSAValue> {
            let info = block.expect_info(stage);
            let mut def_set: HashSet<SSAValue> = HashSet::new();
            let mut use_set: HashSet<SSAValue> = HashSet::new();

            for &ba in &info.arguments {
                def_set.insert(ba.into());
            }

            let mut process_stmt = |stmt: kirin::prelude::Statement| {
                for &val in stmt.arguments(stage) {
                    if !def_set.contains(&val) {
                        use_set.insert(val);
                    }
                }
                for &rv in stmt.results(stage) {
                    def_set.insert(rv.into());
                }
            };

            for stmt in block.statements(stage) {
                process_stmt(stmt);
            }
            if let Some(term) = block.terminator(stage) {
                process_stmt(term);
            }

            use_set
                .into_iter()
                .chain(live_out.into_iter().filter(|v| !def_set.contains(v)))
                .collect()
        }
    }

    fn analyze_liveness<L: Dialect>(
        body_stmt: kirin::prelude::Statement,
        stage: &StageInfo<L>,
    ) -> LivenessResult {
        let fp = BackwardFixpoint::new(LivenessTransfer);
        let result = fp.analyze(body_stmt, stage);
        let mut live_in = HashMap::new();
        let mut live_out = HashMap::new();
        for (block, (li, lo)) in result {
            live_in.insert(block, li);
            live_out.insert(block, lo);
        }
        LivenessResult { live_in, live_out }
    }

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

    fn collect_free_vars<L: Dialect>(block: Block, stage: &StageInfo<L>) -> HashSet<SSAValue> {
        let block_info = block.expect_info(stage);
        let mut local_defs: HashSet<SSAValue> = block_info
            .arguments
            .iter()
            .map(|ba| SSAValue::from(*ba))
            .collect();
        let mut free: HashSet<SSAValue> = HashSet::new();

        let process =
            |stmt: Statement, local_defs: &mut HashSet<SSAValue>, free: &mut HashSet<SSAValue>| {
                for &val in stmt.arguments(stage) {
                    if !local_defs.contains(&val) {
                        free.insert(val);
                    }
                }
                for nested in stmt.blocks(stage) {
                    for val in collect_free_vars(*nested, stage) {
                        if !local_defs.contains(&val) {
                            free.insert(val);
                        }
                    }
                }
                for &rv in stmt.results(stage) {
                    local_defs.insert(rv.into());
                }
            };

        for stmt in block.statements(stage) {
            process(stmt, &mut local_defs, &mut free);
        }
        if let Some(term) = block.terminator(stage) {
            process(term, &mut local_defs, &mut free);
        }
        free
    }

    fn stmt_backward_liveness<L: Dialect>(
        top_block: Block,
        stage: &StageInfo<L>,
    ) -> HashMap<Statement, HashSet<SSAValue>> {
        let mut stmts: Vec<Statement> = top_block.statements(stage).collect();
        if let Some(term) = top_block.terminator(stage) {
            stmts.push(term);
        }

        let mut live: HashSet<SSAValue> = HashSet::new();
        let mut result: HashMap<Statement, HashSet<SSAValue>> = HashMap::new();

        for &stmt in stmts.iter().rev() {
            let mut uses: HashSet<SSAValue> = HashSet::new();
            for &val in stmt.arguments(stage) {
                uses.insert(val);
            }
            for nested in stmt.blocks(stage) {
                for val in collect_free_vars(*nested, stage) {
                    uses.insert(val);
                }
            }
            let defs: HashSet<SSAValue> =
                stmt.results(stage).map(|rv| SSAValue::from(*rv)).collect();

            let live_before: HashSet<SSAValue> = uses
                .iter()
                .copied()
                .chain(live.iter().filter(|v| !defs.contains(v)).copied())
                .collect();

            result.insert(stmt, live_before.clone());
            live = live_before;
        }

        result
    }

    // -----------------------------------------------------------------------
    // Liveness tests
    // -----------------------------------------------------------------------

    #[test]
    fn liveness_add_args_live_at_entry() {
        let pipeline = build_pipeline(ADD_LOWERED);
        let result = liveness_for_lowered_fn(&pipeline, "add");

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

    #[test]
    fn liveness_dead_after_use() {
        let pipeline = build_pipeline(BRANCH_LOWERED);
        let result = liveness_for_lowered_fn(&pipeline, "sign");

        assert_eq!(
            result.live_in.len(),
            3,
            "BRANCH_LOWERED should have 3 blocks"
        );

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

        let region = body_stmt.regions(stage_info).next().unwrap();
        let mut block_iter = region.blocks(stage_info);
        let entry_block = block_iter.next().expect("entry block must exist");
        let base_block = block_iter.next().expect("base block must exist");
        let recurse_block = block_iter.next().expect("recurse block must exist");

        let entry_info = entry_block.expect_info(stage_info);
        let n_ssa: SSAValue = entry_info.arguments[0].into();

        assert!(
            result.live_in[&recurse_block].contains(&n_ssa),
            "%n must be live-in of ^recurse because it is used there without local definition"
        );
        assert!(
            result.live_out[&entry_block].contains(&n_ssa),
            "%n must be live-out of ^entry because the recurse successor uses it"
        );
        assert!(
            !result.live_in[&entry_block].contains(&n_ssa),
            "%n is defined by ^entry as a block arg; it must not appear in live_in[^entry]"
        );
        assert!(
            !result.live_in[&base_block].contains(&n_ssa),
            "%n is not used in ^base; it must not appear in live_in[^base]"
        );
    }

    #[test]
    fn backward_liveness_highlevel() {
        let pipeline = build_pipeline(ABS_SOURCE);
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
            pipeline.stage(stage_id).unwrap().try_stage_info().unwrap();
        let spec = pipeline
            .resolve_staged_function("abs", stage_id)
            .unwrap()
            .get_info(stage_info)
            .unwrap()
            .unique_live_specialization()
            .unwrap();
        let spec_info = spec.get_info(stage_info).unwrap();
        let body_stmt = *spec_info.body();

        let region = body_stmt.regions(stage_info).next().unwrap();
        let top_block = region.blocks(stage_info).next().unwrap();

        let liveness = stmt_backward_liveness(top_block, stage_info);

        let stmts: Vec<Statement> = top_block.statements(stage_info).collect();
        assert!(
            stmts.len() >= 3,
            "ABS_SOURCE should have at least 3 non-terminator statements"
        );
        let if_stmt = stmts[2];

        let live_before_if = &liveness[&if_stmt];

        let block_info = top_block.expect_info(stage_info);
        let x_ssa: SSAValue = block_info.arguments[0].into();
        let is_neg_ssa: SSAValue = SSAValue::from(*stmts[1].results(stage_info).next().unwrap());

        assert!(
            live_before_if.contains(&x_ssa),
            "%x must be live before the scf.if (used as free var in ^neg and ^pos branches)"
        );
        assert!(
            live_before_if.contains(&is_neg_ssa),
            "%is_neg must be live before the scf.if (it is the condition argument)"
        );
    }

    #[test]
    fn backward_liveness_scf() {
        let pipeline = build_pipeline(FACTORIAL_SOURCE);
        let stage_id = pipeline.stage_by_name("source").unwrap();
        let stage_info: &StageInfo<HighLevel> =
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

        let region = body_stmt.regions(stage_info).next().unwrap();
        let top_block = region.blocks(stage_info).next().unwrap();

        let liveness = stmt_backward_liveness(top_block, stage_info);

        let stmts: Vec<Statement> = top_block.statements(stage_info).collect();
        assert!(
            stmts.len() >= 3,
            "FACTORIAL_SOURCE should have at least 3 non-terminator statements"
        );
        let if_stmt = stmts[2];

        let live_before_if = &liveness[&if_stmt];

        let block_info = top_block.expect_info(stage_info);
        let n_ssa: SSAValue = block_info.arguments[0].into();
        let one_ssa: SSAValue = SSAValue::from(*stmts[0].results(stage_info).next().unwrap());
        let is_base_ssa: SSAValue = SSAValue::from(*stmts[1].results(stage_info).next().unwrap());

        assert!(
            live_before_if.contains(&n_ssa),
            "%n must be live before the scf.if (free var of ^recurse: used in sub and mul)"
        );
        assert!(
            live_before_if.contains(&one_ssa),
            "%one must be live before the scf.if (free var of both ^base and ^recurse)"
        );
        assert!(
            live_before_if.contains(&is_base_ssa),
            "%is_base must be live before the scf.if (it is the condition argument)"
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

    // -----------------------------------------------------------------------
    // Sparse abstract interpretation tests
    // -----------------------------------------------------------------------

    #[test]
    fn sparse_interval_propagation() {
        let pipeline = build_pipeline(SPARSE_PROG);
        let result = analyze_lowered::<Interval>(
            &pipeline,
            "maybe_add",
            vec![Interval::new(1, 3), Interval::new(2, 4), Interval::bottom()],
        );
        assert_eq!(
            result,
            Some(Interval::new(3, 7)),
            "sparse AI: seeded args propagate correctly; unused bottom arg ignored"
        );
    }

    #[test]
    fn sparse_type_propagation() {
        let pipeline = build_pipeline(SPARSE_PROG);
        let result = analyze_lowered::<ToyType>(
            &pipeline,
            "maybe_add",
            vec![ToyType::I64, ToyType::I64, ToyType::Bottom],
        );
        assert_eq!(
            result,
            Some(ToyType::I64),
            "sparse AI: type propagates from seeded values; Bottom arg does not pollute result"
        );
    }
}
