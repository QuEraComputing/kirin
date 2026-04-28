pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

pub trait ProductValue: Clone + Sized {
    fn new_product(values: Vec<Self>) -> Self;
    fn as_product(&self) -> Option<&[Self]>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}

impl ProductValue for i64 {
    fn new_product(values: Vec<Self>) -> Self {
        match values.as_slice() {
            [value] => *value,
            _ => panic!("i64 only supports single-value products"),
        }
    }

    fn as_product(&self) -> Option<&[Self]> {
        None
    }
}
