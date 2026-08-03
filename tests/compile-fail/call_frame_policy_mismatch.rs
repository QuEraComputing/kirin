//! A total frame type whose `Call` variant carries one callable-body policy
//! while `#[derive(FrameBuild)]` is configured with another (here: the default,
//! because no `#[interpret(body_frames = ..)]` was given).
//!
//! The derive deliberately does not try to reconcile the two — it emits
//! `type BodyFrames = DefaultBodyFrames` and lets the generated impl produce a
//! type error naming both policies, which is more informative than anything the
//! macro could say about a path it cannot resolve.

use kirin_interpreter::{
    BlockFrame, BodyFrameEntry, CFGFrame, CallBodyFramePolicy, CallFrame, DefaultBodyFrames,
    DiGraphFrame, FrameBuild, InterpreterError,
};
use kirin_ir::{Block, CFG, DiGraph, UnGraph};

struct MyBodyFrames;

impl<V, E> CallBodyFramePolicy<V, E, MismatchedFrame<V, E>> for MyBodyFrames
where
    V: Clone,
    E: From<InterpreterError>,
{
    fn from_cfg(entry: BodyFrameEntry<CFG, V>) -> Result<MismatchedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, MismatchedFrame<V, E>>>::from_cfg(entry)
    }
    fn from_block(entry: BodyFrameEntry<Block, V>) -> Result<MismatchedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, MismatchedFrame<V, E>>>::from_block(entry)
    }
    fn from_digraph(entry: BodyFrameEntry<DiGraph, V>) -> Result<MismatchedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, MismatchedFrame<V, E>>>::from_digraph(entry)
    }
    fn from_ungraph(entry: BodyFrameEntry<UnGraph, V>) -> Result<MismatchedFrame<V, E>, E> {
        <DefaultBodyFrames as CallBodyFramePolicy<V, E, MismatchedFrame<V, E>>>::from_ungraph(entry)
    }
}

// Missing: #[interpret(body_frames = MyBodyFrames)]
#[derive(FrameBuild)]
enum MismatchedFrame<V, E> {
    Block(BlockFrame<V, E>),
    CFG(CFGFrame<V, E>),
    Call(CallFrame<V, MyBodyFrames>),
    DiGraph(DiGraphFrame<V, E>),
}

fn main() {}
