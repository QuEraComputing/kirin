//! The frame run of one work item.
//!
//! [`run_frame`](StandardFixpointInterpreter::run_frame) drives one work item's
//! traversal to completion on the shared frame stack. It reuses the crate's
//! canonical [`drive_frames`] loop: the driver's `frame_stack` is moved into the
//! loop for the duration of the run (it is empty between runs, so nothing is
//! lost) and restored afterwards.

use crate::{Frame, Interp, InterpreterError, drive_frames};

use super::{FixpointProfile, StandardFixpointInterpreter};

impl<I, P, Store> StandardFixpointInterpreter<I, P, Store>
where
    I: Interp,
    P: FixpointProfile<I>,
{
    /// Run `root` (and any children it pushes) to a completion.
    ///
    /// The frame stack must be empty on entry — each work item is analysed with
    /// a fresh stack.
    pub fn run_frame(&mut self, root: P::Frame) -> Result<P::Completion, I::Error>
    where
        P::Frame: Frame<Self, Completion = P::Completion>,
    {
        if !self.frame_stack.is_empty() {
            return Err(I::Error::from(InterpreterError::Custom(
                "cannot start a frame run with a non-empty frame stack",
            )));
        }

        let mut frames = std::mem::take(&mut self.frame_stack);
        frames.push(root);
        let completion = drive_frames(self, &mut frames)?;
        self.frame_stack = frames;
        Ok(completion)
    }
}
