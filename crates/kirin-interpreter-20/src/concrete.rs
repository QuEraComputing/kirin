use std::marker::PhantomData;

use kirin_ir::{
    Block, CompileStage, Dialect, HasStageInfo, Pipeline, ResultValue, SSAValue,
    SpecializedFunction, StageMeta,
};

use crate::algebra::Lift;
use crate::block_exec::{BlockExecEnv, JumpOutcome};
use crate::call_dispatch::CallDispatch;
use crate::control::{Control, CursorExt};
use crate::cursor::BlockCursor;
use crate::env::{ConcreteMode, Env};
use crate::error::InterpreterError;
use crate::execute::{Execute, StackEntry};
use crate::frame::Frame;
use crate::frame_stack::FrameStack;
use crate::pipeline::PipelineHandle;

pub struct ConcreteInterp<'ir, S: StageMeta, L: Dialect, V: Clone, C> {
    pub handle: PipelineHandle<'ir, S>,
    pub frames: FrameStack<V>,
    pub cursors: Vec<StackEntry<C, V>>,
    pub result: Option<V>,
    fuel: Option<u64>,
    max_depth: Option<usize>,
    _phantom: PhantomData<L>,
}

impl<'ir, S, L, V, C> Env for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
{
    type Mode = ConcreteMode<C>;
    type Value = V;
    type Ext = CursorExt<C>;
    type Error = InterpreterError;
    type Stages = S;

    fn current_stage(&self) -> CompileStage {
        self.frames.active_stage_or(self.handle.stage_id)
    }
    fn pipeline(&self) -> &Pipeline<S> {
        self.handle.pipeline
    }
    fn read(&self, ssa: SSAValue) -> Result<V, InterpreterError> {
        self.frames.read(ssa)
    }
    fn write_result(&mut self, r: ResultValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write(r, v)
    }
    fn write_ssa(&mut self, ssa: SSAValue, v: V) -> Result<(), InterpreterError> {
        self.frames.write_ssa(ssa, v)
    }
}

// ---------------------------------------------------------------------------
// BlockExecEnv — concrete mode: jumps rewind the cursor in place
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> BlockExecEnv for ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
{
    fn exec_jump(&mut self, _target: Block, _args: Vec<V>) -> JumpOutcome<V, CursorExt<C>> {
        JumpOutcome::Rewound
    }

    fn exec_fork(
        &mut self,
        _branches: Vec<(Block, Vec<V>)>,
    ) -> Result<Control<V, CursorExt<C>>, InterpreterError> {
        Err(InterpreterError::UnhandledEffect(
            "Control::Fork in concrete interpreter; use AbstractInterp for nondeterminism".into(),
        ))
    }

    fn exec_block_end(&self) -> Control<V, CursorExt<C>> {
        Control::Ext(CursorExt::Pop)
    }
}

// ---------------------------------------------------------------------------
// Constructor
// ---------------------------------------------------------------------------

impl<'ir, S: StageMeta, L: Dialect, V: Clone, C> ConcreteInterp<'ir, S, L, V, C> {
    pub fn new(pipeline: &'ir Pipeline<S>, stage_id: CompileStage) -> Self {
        Self {
            handle: PipelineHandle::new(pipeline, stage_id),
            frames: FrameStack::new(),
            cursors: Vec::new(),
            result: None,
            fuel: None,
            max_depth: None,
            _phantom: PhantomData,
        }
    }

    pub fn with_fuel(mut self, fuel: u64) -> Self {
        self.fuel = Some(fuel);
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }
}

// ---------------------------------------------------------------------------
// Enter function helper
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L>,
    L: Dialect,
    V: Clone,
{
    pub fn enter_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        entry_block: Block,
        args: &[V],
    ) -> Result<(), InterpreterError>
    where
        S: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + Lift<C>,
    {
        let stage_id = self.handle.stage_id;
        let cursor = BlockCursor::<V, LD>::new(entry_block, stage_id, args.to_vec());
        let frame = Frame::new(callee, stage_id, vec![]);
        self.frames.push(frame)?;
        self.cursors.push(StackEntry::new(cursor.lift()));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Step and run
// ---------------------------------------------------------------------------

impl<'ir, S, L, V, C> ConcreteInterp<'ir, S, L, V, C>
where
    S: StageMeta + HasStageInfo<L> + CallDispatch<V, C>,
    L: Dialect,
    V: Clone + kirin_interpreter::ProductValue,
    C: Execute<Self>,
{
    pub fn step(&mut self) -> Result<bool, InterpreterError> {
        if let Some(ref mut f) = self.fuel {
            if *f == 0 {
                return Err(InterpreterError::FuelExhausted);
            }
            *f -= 1;
        }
        let Some(mut entry) = self.cursors.pop() else {
            return Ok(false);
        };
        let inbox = entry.inbox.take();
        let effect: Control<V, CursorExt<C>> = entry.cursor.execute(self, inbox)?;
        match effect {
            Control::Advance => {
                self.cursors.push(entry);
            }
            Control::Jump(..) => {
                self.cursors.push(entry);
            }
            Control::Ext(CursorExt::Push(new_cursor)) => {
                self.cursors.push(entry);
                self.cursors.push(StackEntry::new(new_cursor));
            }
            Control::Ext(CursorExt::Pop) => {}
            Control::Yield(v) => {
                if let Some(parent) = self.cursors.last_mut() {
                    parent.inbox = Some(v);
                } else {
                    self.result = Some(v);
                }
            }
            Control::Return(v) => {
                let frame = self.frames.pop().ok_or(InterpreterError::NoFrame)?;
                let caller_results = frame.caller_results().to_vec();
                if self.frames.is_empty() {
                    self.result = Some(v);
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
                if let Some(max) = self.max_depth
                    && self.frames.depth() >= max
                {
                    return Err(InterpreterError::MaxDepthExceeded);
                }
                self.cursors.push(entry);
                let cursor = S::make_call_cursor(self.handle.pipeline, callee, stage, args)?;
                let frame = Frame::new(callee, stage, results);
                self.frames.push(frame)?;
                self.cursors.push(StackEntry::new(cursor));
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

    pub fn run(&mut self) -> Result<Option<V>, InterpreterError> {
        while self.step()? {}
        Ok(self.result.take())
    }

    pub fn run_function<LD: Dialect>(
        &mut self,
        callee: SpecializedFunction,
        args: &[V],
    ) -> Result<Option<V>, InterpreterError>
    where
        S: HasStageInfo<LD>,
        BlockCursor<V, LD>: Execute<Self> + Lift<C>,
    {
        let stage_id = self.handle.stage_id;
        let entry_block = PipelineHandle::new(self.handle.pipeline, stage_id)
            .entry_block_of::<LD>(callee, stage_id)?;
        self.enter_function::<LD>(callee, entry_block, args)?;
        self.run()
    }
}
