use kirin::prelude::*;

// Note: HasParser is not derived because T and L are generic type parameters
// that would need HasParser bounds. For generic constants, implement HasParser manually.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(constant, fn = new, type_lattice = L)]
pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(default = std::marker::PhantomData)]
    pub marker: std::marker::PhantomData<L>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_constant_creation() {}
}

// mod parse {
//     use kirin::ir::*;
//     use kirin::parsers::prelude::*;
//     #[derive(Debug, Clone)]
//     pub struct Constant<'src, T> {
//         pub value: T,
//         pub result: ast::Spanned<&'src str>,
//         pub marker: std::marker::PhantomData<&'src ()>,
//     }

//     impl<'tokens, 'src, D, T, L> HasParser<'tokens, 'src, D> for Constant<'src, T>
//     where
//         'src: 'tokens,
//         D: Dialect<TypeLattice = L> + HasParser<'tokens, 'src, D>,
//         T: CompileTimeValue + Typeof<L> + HasParser<'tokens, 'src, D, Output = T> + 'tokens,
//         L: TypeLattice + HasParser<'tokens, 'src, D>,
//     {
//         type Output = Constant<'src, T>;
//         fn parser<I: TokenInput<'tokens, 'src>>()
//         -> Boxed<'tokens, 'tokens, I, Self::Output, ParserError<'tokens, 'src>> {
//             ssa_value()
//                 .then_ignore(just(Token::Equal))
//                 .then_ignore(identifier("constant"))
//                 .then(T::parser())
//                 .then(empty())
//                 .map(|((result, value), _)| Constant {
//                     value,
//                     result,
//                     marker: std::marker::PhantomData,
//                 })
//                 .labelled("constant instruction with result")
//                 .boxed()
//         }
//     }
// }
