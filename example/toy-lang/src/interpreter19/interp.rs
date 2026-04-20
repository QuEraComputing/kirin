use std::ops::{Add, BitAnd, BitOr, BitXor, Mul, Neg, Not, Sub};

use kirin::prelude::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, Symbol,
};
use kirin_arith::{ArithType, ArithValue, CheckedDiv, CheckedRem};
use kirin_bitwise::{CheckedShl, CheckedShr};
use kirin_cmp::CompareValue;
use kirin_interpreter::{AbstractValue, BranchCondition, ProductValue};
use kirin_interpreter_19::abstract_call_dispatch::AbstractCallDispatch;
use kirin_interpreter_19::abstract_interp::{
    AbstractFrame, AbstractInterp, FuncState, FuncSummary, StagedKey, Worklist,
};
use kirin_interpreter_19::algebra::Lift;
use kirin_interpreter_19::block_exec::{BlockExecEnv, JumpOutcome};
use kirin_interpreter_19::call_dispatch::CallDispatch;
use kirin_interpreter_19::concrete::ConcreteInterp;
use kirin_interpreter_19::control::{Control, CursorExt};
use kirin_interpreter_19::cursor::BlockCursor;
use kirin_interpreter_19::dispatch::Dispatch;
use kirin_interpreter_19::env::{AbstractEnv, Env};
use kirin_interpreter_19::error::InterpreterError;
use kirin_interpreter_19::execute::{Execute, StackEntry};
use kirin_interpreter_19::fixpoint_driver::FixpointDriver;
use kirin_interpreter_19::frame::Frame;
use kirin_interpreter_19::interpretable::Interpretable;
use kirin_interpreter_19::pipeline::PipelineHandle;
use kirin_scf::interpreter19::interpret::ScfSeam;
use kirin_scf::{For, ForLoopValue, If};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::language::{HighLevel, LowLevel};
use crate::stage::Stage;

use super::cursors::{
    AbstractMultiCursor, HighLevelAbstractCursor, HighLevelCursor, LowLevelAbstract, MultiCursor,
};

// ---------------------------------------------------------------------------
// ToyVal / AbstractToyVal — value trait aliases
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
// Interpretable impls for HighLevel and LowLevel
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
// AbstractCallDispatch — single-stage abstract interpreters
// ---------------------------------------------------------------------------

impl<V: Clone> AbstractCallDispatch<V, LowLevelAbstract<V>> for Stage {
    fn make_abstract_cursor(
        _pipeline: &Pipeline<Stage>,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> LowLevelAbstract<V> {
        LowLevelAbstract(BlockCursor::new(block, stage_id, args))
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
        HighLevelAbstractCursor::Block(BlockCursor::new(block, stage_id, args))
    }

    fn entry_block_for(
        pipeline: &Pipeline<Stage>,
        callee: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        PipelineHandle::new(pipeline, stage_id).entry_block_of::<HighLevel>(callee, stage_id)
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
                AbstractMultiCursor::HighBlock(BlockCursor::new(block, stage_id, args))
            }
            _ => AbstractMultiCursor::Low(BlockCursor::new(block, stage_id, args)),
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
// CallDispatch for MultiCursor — multi-stage concrete interpreter
// ---------------------------------------------------------------------------

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
// MultiInterp — multi-stage concrete newtype (orphan rule fix)
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
    type Mode = kirin_interpreter_19::env::ConcreteMode<MultiCursor<V>>;
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

impl<'ir, V: Clone> BlockExecEnv for MultiInterp<'ir, V> {
    fn exec_jump(
        &mut self,
        _target: Block,
        _args: Vec<V>,
    ) -> JumpOutcome<V, CursorExt<MultiCursor<V>>> {
        JumpOutcome::Rewound
    }
    fn exec_fork(
        &mut self,
        _branches: Vec<(Block, Vec<V>)>,
    ) -> Result<Control<V, CursorExt<MultiCursor<V>>>, InterpreterError> {
        Err(InterpreterError::UnhandledEffect(
            "Control::Fork in concrete interpreter".into(),
        ))
    }
    fn exec_block_end(&self) -> Control<V, CursorExt<MultiCursor<V>>> {
        Control::Ext(CursorExt::Pop)
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
// AbstractMultiInterp — multi-stage abstract newtype.
// KEY IMPROVEMENT: implements FixpointDriver, delegates analyze to run_fixpoint.
// Eliminates the inlined fixpoint loop that was in interpreter-18.
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
    type Mode = kirin_interpreter_19::env::AbstractMode<AbstractMultiCursor<V>>;
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

impl<'ir, V: Clone + AbstractValue> BlockExecEnv for AbstractMultiInterp<'ir, V> {
    fn exec_jump(
        &mut self,
        target: Block,
        args: Vec<V>,
    ) -> JumpOutcome<V, CursorExt<AbstractMultiCursor<V>>> {
        self.0.enqueue_block(target, args);
        JumpOutcome::Done(Control::Ext(CursorExt::Pop))
    }
    fn exec_fork(
        &mut self,
        branches: Vec<(Block, Vec<V>)>,
    ) -> Result<Control<V, CursorExt<AbstractMultiCursor<V>>>, InterpreterError> {
        for (block, args) in branches {
            self.0.enqueue_block(block, args);
        }
        Ok(Control::Ext(CursorExt::Pop))
    }
    fn exec_block_end(&self) -> Control<V, CursorExt<AbstractMultiCursor<V>>> {
        Control::Ext(CursorExt::Pop)
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

// FixpointDriver impl — delegates run_fixpoint; no inlined loop needed.
impl<'ir, V: AbstractToyVal> FixpointDriver for AbstractMultiInterp<'ir, V> {
    type Cursor = AbstractMultiCursor<V>;

    fn summaries_ref(&self) -> &FxHashMap<StagedKey, FuncSummary<V>> {
        &self.0.summaries
    }
    fn summaries_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncSummary<V>> {
        &mut self.0.summaries
    }
    fn func_states_mut(&mut self) -> &mut FxHashMap<StagedKey, FuncState<V>> {
        &mut self.0.func_states
    }
    fn func_worklist_mut(&mut self) -> &mut Worklist<StagedKey> {
        &mut self.0.func_worklist
    }
    fn cursor_stack_ref(&self) -> &[StackEntry<AbstractMultiCursor<V>, V>] {
        &self.0.cursor_stack
    }
    fn cursor_stack_mut(&mut self) -> &mut Vec<StackEntry<AbstractMultiCursor<V>, V>> {
        &mut self.0.cursor_stack
    }
    fn call_graph_mut(&mut self) -> &mut FxHashMap<StagedKey, FxHashSet<AbstractFrame>> {
        &mut self.0.call_graph
    }
    fn fn_visit_counts_mut(&mut self) -> &mut FxHashMap<StagedKey, usize> {
        &mut self.0.fn_visit_counts
    }
    fn widening_strategy(&self) -> kirin_interpreter::WideningStrategy {
        self.0.widening
    }
    fn make_abstract_cursor(
        &self,
        stage_id: CompileStage,
        block: Block,
        args: Vec<V>,
    ) -> AbstractMultiCursor<V> {
        Stage::make_abstract_cursor(self.0.handle.pipeline, stage_id, block, args)
    }
    fn set_current_key(&mut self, key: Option<StagedKey>) {
        self.0.current_key = key;
    }
    fn get_current_key(&self) -> Option<StagedKey> {
        self.0.current_key
    }
    fn entry_block_for(
        &self,
        func: SpecializedFunction,
        stage_id: CompileStage,
    ) -> Result<Block, InterpreterError> {
        <Stage as AbstractCallDispatch<V, AbstractMultiCursor<V>>>::entry_block_for(
            self.0.handle.pipeline,
            func,
            stage_id,
        )
    }
}

#[allow(dead_code)]
impl<'ir, V: AbstractToyVal> AbstractMultiInterp<'ir, V>
where
    AbstractMultiCursor<V>: Execute<Self>,
    Self: Env<Ext = CursorExt<AbstractMultiCursor<V>>, Value = V, Error = InterpreterError>,
{
    pub fn analyze(
        &mut self,
        entry_fn: SpecializedFunction,
        stage_id: CompileStage,
        args: Vec<V>,
    ) -> Result<Option<V>, InterpreterError> {
        FixpointDriver::run_fixpoint(self, entry_fn, stage_id, args)
    }
}
