use kirin::ir::*;

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
