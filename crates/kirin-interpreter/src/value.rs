use kirin_ir::Product;

pub trait BranchCondition {
    fn is_truthy(&self) -> Option<bool>;
}

pub trait HasProductValue: Clone + Sized {
    fn from_product(product: Product<Self>) -> Self;
    fn as_product(&self) -> Option<&Product<Self>>;
}

impl BranchCondition for i64 {
    fn is_truthy(&self) -> Option<bool> {
        Some(*self != 0)
    }
}
