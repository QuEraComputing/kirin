//! The work-item boundary of the fixpoint driver.
//!
//! The driver ([`StandardFixpointInterpreter`](super::StandardFixpointInterpreter))
//! runs one work item at a time and delegates its traversal to a frame stack.
//! [`WorkAnalysis`] describes only that boundary: how to enter a work item and
//! how to complete it. The value/error/effect/semantics of the analysis stay on
//! the wrapped [`Interp`](crate::Interp).

/// How the driver enters and completes one work item.
///
/// `I` is the wrapped interpreter (the driver passes itself, so the analysis can
/// use the interpreter's env/query helpers); `W`/`F`/`C` are the profile's work
/// item / frame / completion; `E` is the interpreter's error.
pub trait WorkAnalysis<I, W, F, C, E> {
    /// The root frame that analyses `work`.
    fn entry_frame(&mut self, interp: &mut I, work: &W) -> Result<F, E>;

    /// Handle the completion of `work`'s frame run.
    fn complete_work(&mut self, interp: &mut I, work: W, completion: C) -> Result<(), E>;
}
