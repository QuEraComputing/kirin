/// Strategy for when to apply widening during fixpoint iteration.
#[derive(Debug, Clone, Copy)]
pub enum WideningStrategy {
    /// Widen at every join point (block with multiple predecessors or loop headers).
    AllJoins,
}
