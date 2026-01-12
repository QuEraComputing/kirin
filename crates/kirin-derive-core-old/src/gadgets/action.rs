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

impl<'src, L: Layout, T> Compile<'src, L, Action<T>> for Enum<'src, L>
where
    for<'a> Variant<'a, 'src, L>: Compile<'src, L, T>,
{
    fn compile(&self, ctx: &L) -> Action<T> {
        let variants = self
            .variants()
            .map(|v| v.compile(&ctx))
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
