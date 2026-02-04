use kirin::prelude::*;

#[derive(Clone, Copy, Hash, PartialEq, Eq, Debug, Dialect, HasParser, PrettyPrint)]
#[kirin(fn, type_lattice = T)]
#[chumsky(format = "fn {name} {body}")]
pub struct Function<T: TypeLattice> {
    name: Symbol,
    body: Region,
    #[kirin(default)]
    _marker: std::marker::PhantomData<T>,
}
