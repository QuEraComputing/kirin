pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}
