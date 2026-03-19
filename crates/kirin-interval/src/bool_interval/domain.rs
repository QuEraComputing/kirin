use crate::Interval;

/// Abstract boolean domain for comparison results.
///
/// Represents the possible outcomes of a comparison operation
/// with four values forming a diamond lattice:
///
/// ```text
///     Unknown (top)
///      /   \
///   True   False
///      \   /
///     Bottom
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BoolInterval {
    /// The comparison is definitely true.
    True,
    /// The comparison is definitely false.
    False,
    /// The comparison result is unknown (could be true or false).
    Unknown,
    /// Unreachable / no information (bottom element).
    Bottom,
}

impl From<BoolInterval> for Interval {
    fn from(b: BoolInterval) -> Self {
        match b {
            BoolInterval::True => Interval::constant(1),
            BoolInterval::False => Interval::constant(0),
            BoolInterval::Unknown => Interval::new(0, 1),
            BoolInterval::Bottom => Interval::bottom_interval(),
        }
    }
}
