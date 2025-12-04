use crate::{Context, Dialect, Statement};

pub trait Visit<L: Dialect, T> {
    type Output;
    fn visit(&mut self, context: &Context<L>, item: &T) -> Self::Output;
}
