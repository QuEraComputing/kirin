use kirin::prelude::*;

#[derive(Clone, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type_lattice = T)]
#[chumsky(format = "fn {name} {signature} {body}")]
pub struct SimpleFunction<T: TypeLattice + PrettyPrint> {
    name: Symbol,
    signature: SimpleFunctionSignature<T>,
    body: Block,
}

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct SimpleFunctionSignature<T: TypeLattice> {
    input_types: Vec<T>,
    output_types: Vec<T>,
}

impl<T: TypeLattice + PrettyPrint> PrettyPrint for SimpleFunctionSignature<T> {
    fn pretty_print<'a, L: Dialect + PrettyPrint>(&self, doc: &'a Document<'a, L>) -> ArenaDoc<'a>
    where
        L::TypeLattice: std::fmt::Display,
    {
        doc.list(self.input_types.iter(), ", ", |item| item.pretty_print(doc))
            + doc.text(" -> ")
            + doc.list(self.output_types.iter(), ", ", |item| {
                item.pretty_print(doc)
            })
    }
}
