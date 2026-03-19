use super::BoolInterval;

impl kirin_interpreter::BranchCondition for BoolInterval {
    fn is_truthy(&self) -> Option<bool> {
        match self {
            BoolInterval::True => Some(true),
            BoolInterval::False => Some(false),
            BoolInterval::Unknown | BoolInterval::Bottom => None,
        }
    }
}
