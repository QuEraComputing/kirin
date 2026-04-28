pub trait AbstractValue: Clone + PartialEq {
    fn bottom() -> Self;

    fn top() -> Self;

    fn join(&self, other: &Self) -> Self;

    fn widen(&self, other: &Self) -> Self {
        self.join(other)
    }

    fn narrow(&self, _other: &Self) -> Self {
        self.clone()
    }

    fn join_assign(&mut self, other: &Self) -> bool {
        let joined = self.join(other);
        if *self == joined {
            false
        } else {
            *self = joined;
            true
        }
    }
}
