//! Frame-side stage dispatch.
//!
//! [`StageFrame`] is the single trait users (or their derives) implement on a
//! root-frame enum to teach the framework how to construct that frame from a
//! pipeline stage. It replaces the old `FunctionInvocationFrame<V>` trait —
//! the only frame-side construction API.
//!
//! - Single-language frames implement `StageFrame<S, V>` generically in `S`
//!   (the stage info is ignored). The framework provides a blanket impl for
//!   [`StandardFrame<L, V, T>`].
//! - Multi-stage frames implement `StageFrame<S, V>` for a specific stage
//!   enum, matching variants to pick the per-language frame to build.
//!
//! The interpreter-side companion is
//! [`FrameDispatch`](crate::FrameDispatch), which composes pipeline lookup
//! with `StageFrame` construction.

use kirin_ir::{Block, CompileStage, Product};

use crate::{EnvIndex, FunctionInvocation};

use super::{BlockFrame, StandardFrame};

/// Construct a root frame from a pipeline stage.
///
/// Implementors describe two frame-creation operations driven by the
/// runtime stage. Single-language frames ignore the stage parameter; multi-
/// stage frames inspect it to select the right per-language constructor.
pub trait StageFrame<S, V>: Sized {
    type Error;

    fn from_function_invocation(
        stage_info: &S,
        invocation: FunctionInvocation<V>,
    ) -> Result<Self, Self::Error>;

    fn from_block(
        stage_info: &S,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<Self, Self::Error>;
}

impl<L, V, T, S> StageFrame<S, V> for StandardFrame<L, V, T> {
    type Error = core::convert::Infallible;

    fn from_function_invocation(
        _stage_info: &S,
        invocation: FunctionInvocation<V>,
    ) -> Result<Self, Self::Error> {
        invocation.into_root_frame::<L, Self, Self::Error>()
    }

    fn from_block(
        _stage_info: &S,
        stage: CompileStage,
        block: Block,
        env: EnvIndex,
        args: Product<V>,
    ) -> Result<Self, Self::Error> {
        Ok(BlockFrame::new(stage, block, env, args).into())
    }
}
