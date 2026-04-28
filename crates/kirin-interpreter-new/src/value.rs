pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

pub trait ProductValue: Clone + Sized {
    fn new_product(values: Vec<Self>) -> Self;
    fn as_product(&self) -> Option<&[Self]>;
}
