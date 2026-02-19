use crate::AbstractValue;

/// Strategy for when to apply widening during fixpoint iteration.
#[derive(Debug, Clone, Copy)]
pub enum WideningStrategy {
    /// Widen at every join point (block with multiple predecessors or loop headers).
    AllJoins,
    /// Only join, never widen. Suitable for finite-height lattices that
    /// guarantee termination without widening.
    Never,
    /// Join for the first `n` visits to each block, then widen.
    Delayed(usize),
}

impl WideningStrategy {
    /// Merge `current` with `incoming` according to this strategy.
    ///
    /// `visit_count` is the number of times the target block has been
    /// revisited (excluding the first visit).
    pub fn merge<V: AbstractValue>(&self, current: &V, incoming: &V, visit_count: usize) -> V {
        match self {
            Self::AllJoins => current.widen(incoming),
            Self::Never => current.join(incoming),
            Self::Delayed(n) => {
                if visit_count <= *n {
                    current.join(incoming)
                } else {
                    current.widen(incoming)
                }
            }
        }
    }
}
