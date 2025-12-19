use crate::{
    derive::Compile,
    ir::{Enum, Layout, Variant},
};

/// A compiled action for each variant in an enum
///
/// !!! Note
/// The type `T` is the output type of the compilation, usually `TokenStream`.
/// Usually used in conjunction with `Enum::variant_names` and `Unpacking` to form match arms.
pub struct Action<T>(std::vec::IntoIter<T>);

impl<'src, L: Layout, T> Compile<'src, Enum<'src, Self>, Action<T>> for L
where
    L: for<'a> Compile<'src, Variant<'a, 'src, L>, T>,
{
    fn compile(&self, node: &Enum<'src, Self>) -> Action<T> {
        let variants = node
            .variants()
            .map(|v| self.compile(&v))
            .collect::<Vec<T>>();
        Action(variants.into_iter())
    }
}

impl<T> Iterator for Action<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
