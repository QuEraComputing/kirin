//! Frame-based traversal for the dense backward engine.
//!
//! The dialect API produces a closed [`DenseBackwardEffect`] per statement;
//! these frames decide how the engine *walks*. [`DenseBlockFrame`] walks one
//! block's statements in reverse (terminator first) against the engine's
//! point state; structured dialects push their own frames
//! ([`DenseBackwardEffect::Push`]) that reuse it in
//! [`DenseBlockMode::StructuredBody`] to walk a chosen body. A language
//! composes a total frame enum via [`DenseFrameBuild`] (plus the structured
//! dialect's own `Build*` traits) — [`StandardDenseBackwardFrame`] is the
//! structured-control-free default.

use std::marker::PhantomData;

use kirin_ir::{Block, CompileStage, Statement};

use crate::{
    DenseBackwardCompletion, DenseBackwardEffect, DenseBackwardFrameEngine, Frame, FrameEffect,
    InterpreterError, ProgramPoint,
};

/// How a [`DenseBlockFrame`] treats its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DenseBlockMode {
    /// A CFG block owner: the terminator's [`Edges`](DenseBackwardEffect::Edges)
    /// are absorbed (seeding the state and producing `live_out`); completes
    /// with [`DenseBackwardCompletion::Block`].
    CFGOwner,
    /// A structured body walked by a dialect frame against the current point
    /// state: CFG edges are an error, and completion is
    /// [`DenseBackwardCompletion::Structured`].
    StructuredBody,
}

/// Walk one block's statements in reverse, dispatching each dense backward
/// rule against the engine's point state.
pub struct DenseBlockFrame<V, E> {
    stage: CompileStage,
    block: Block,
    /// Current position in the reverse statement walk.
    cursor: Option<Statement>,
    /// `false` until the cursor has been positioned at the block's last
    /// logical statement.
    initialized: bool,
    /// `true` only while visiting the first reverse position (the block's
    /// logical terminator position).
    at_block_exit: bool,
    mode: DenseBlockMode,
    /// The absorbed edge mapping (`CFGOwner` only).
    live_out: Option<V>,
    /// A pushed structured statement whose before-point is recorded once its
    /// frame completes.
    pending_point: Option<Statement>,
    _marker: PhantomData<fn() -> E>,
}

impl<V, E> DenseBlockFrame<V, E>
where
    V: Clone,
    E: From<InterpreterError>,
{
    fn with_mode(stage: CompileStage, block: Block, mode: DenseBlockMode) -> Self {
        Self {
            stage,
            block,
            cursor: None,
            initialized: false,
            at_block_exit: false,
            mode,
            live_out: None,
            pending_point: None,
            _marker: PhantomData,
        }
    }

    /// A CFG block-owner walk.
    pub fn cfg_owner(stage: CompileStage, block: Block) -> Self {
        Self::with_mode(stage, block, DenseBlockMode::CFGOwner)
    }

    /// A structured-body walk (pushed by a dialect frame).
    pub fn structured_body(stage: CompileStage, block: Block) -> Self {
        Self::with_mode(stage, block, DenseBlockMode::StructuredBody)
    }
}

impl<I, F, V, E> Frame<I, F> for DenseBlockFrame<V, E>
where
    I: DenseBackwardFrameEngine<Value = V, Error = E, Frame = F>,
    F: DenseFrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = DenseBackwardCompletion<V>;

    fn step_into(
        mut self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        if !self.initialized {
            if self.mode == DenseBlockMode::StructuredBody {
                let facts = interp.state();
                interp.record_point(ProgramPoint::BlockExit(self.block), facts);
            }
            self.cursor = interp.last_statement(self.stage, self.block)?;
            self.initialized = true;
            self.at_block_exit = true;
        }

        let Some(statement) = self.cursor else {
            let live_in = interp.state();
            if self.mode == DenseBlockMode::StructuredBody {
                interp.record_point(ProgramPoint::BlockEntry(self.block), live_in.clone());
            }
            return Ok(FrameEffect::Complete(match self.mode {
                DenseBlockMode::CFGOwner => DenseBackwardCompletion::Block {
                    live_in: live_in.clone(),
                    live_out: self.live_out.take().unwrap_or(live_in),
                },
                DenseBlockMode::StructuredBody => DenseBackwardCompletion::Structured,
            }));
        };

        let is_terminator_position = self.at_block_exit;
        self.cursor = interp.previous_statement(self.stage, self.block, statement)?;
        self.at_block_exit = false;

        let after = interp.state();
        interp.record_point(ProgramPoint::After(statement), after);
        match interp.run_statement(self.stage, statement)? {
            DenseBackwardEffect::Next => {
                let before = interp.state();
                interp.record_point(ProgramPoint::Before(statement), before);
                Ok(FrameEffect::Continue(F::from_block(self)))
            }
            DenseBackwardEffect::Edges(edges) => {
                if !is_terminator_position {
                    return Err(E::from(InterpreterError::Custom(
                        "a non-terminator produced CFG edges",
                    )));
                }
                match self.mode {
                    DenseBlockMode::CFGOwner => {
                        let out = interp.absorb_edges(self.stage, &edges)?;
                        self.live_out = Some(out);
                        let before = interp.state();
                        interp.record_point(ProgramPoint::Before(statement), before);
                        Ok(FrameEffect::Continue(F::from_block(self)))
                    }
                    DenseBlockMode::StructuredBody => Err(E::from(InterpreterError::Custom(
                        "a structured body block cannot branch into the CFG",
                    ))),
                }
            }
            DenseBackwardEffect::Push { frame } => {
                // The statement's before-point is only known once its frame
                // completes; record it on resume.
                self.pending_point = Some(statement);
                Ok(FrameEffect::Push {
                    parent: F::from_block(self),
                    child: frame,
                })
            }
        }
    }

    fn resume_done_into(
        self,
        _interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        Err(E::from(InterpreterError::Custom(
            "dense block frames resume only with completions",
        )))
    }

    fn resume_into(
        mut self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match completion {
            DenseBackwardCompletion::Structured => {
                if let Some(statement) = self.pending_point.take() {
                    let before = interp.state();
                    interp.record_point(ProgramPoint::Before(statement), before);
                }
                Ok(FrameEffect::Continue(F::from_block(self)))
            }
            DenseBackwardCompletion::Block { .. } => Err(E::from(InterpreterError::Custom(
                "a nested frame completed as a block owner",
            ))),
        }
    }
}

/// Construction trait letting a total dense backward frame enum embed the
/// standard block frame (the analogue of
/// [`AbstractFrameBuild`](crate::AbstractFrameBuild)); structured dialects add
/// their own `Build*` traits beside it.
pub trait DenseFrameBuild<V, E>: Sized {
    fn from_block(frame: DenseBlockFrame<V, E>) -> Self;
}

/// The structured-control-free default frame: just the block walk. A language
/// with a structured dialect supplies its own total enum embedding this plus
/// the dialect's frames.
pub enum StandardDenseBackwardFrame<V, E> {
    Block(DenseBlockFrame<V, E>),
}

impl<V, E> DenseFrameBuild<V, E> for StandardDenseBackwardFrame<V, E> {
    fn from_block(frame: DenseBlockFrame<V, E>) -> Self {
        Self::Block(frame)
    }
}

/// A *universe* impl, generic over the outer total frame type `F` — see
/// [`Frame`] for what that buys.
impl<I, F, V, E> Frame<I, F> for StandardDenseBackwardFrame<V, E>
where
    I: DenseBackwardFrameEngine<Value = V, Error = E, Frame = F>,
    F: DenseFrameBuild<V, E>,
    V: Clone,
    E: From<InterpreterError>,
{
    type Completion = DenseBackwardCompletion<V>;

    fn step_into(self, interp: &mut I) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            Self::Block(frame) => frame.step_into(interp),
        }
    }

    fn resume_done_into(
        self,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            Self::Block(frame) => frame.resume_done_into(interp),
        }
    }

    fn resume_into(
        self,
        completion: DenseBackwardCompletion<V>,
        interp: &mut I,
    ) -> Result<FrameEffect<F, DenseBackwardCompletion<V>>, E> {
        match self {
            Self::Block(frame) => frame.resume_into(completion, interp),
        }
    }
}
