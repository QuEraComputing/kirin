/// A value that can act as a branch condition.
///
/// Returns `Some(true)` for truthy, `Some(false)` for falsy,
/// or `None` when the branch direction cannot be determined
/// (e.g., abstract interpretation with unknown/top values).
pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}
