use kirin::prelude::*;

// Note: HasParser and PrettyPrint are not derived because T and L are generic type parameters
// that would need complex lifetime bounds. For generic constants, implement HasParser manually.
// The format string would be: "{result} = constant {value} -> {result:type}"
#[derive(HasParser)]
#[chumsky(format = "{result:name} = constant {value} -> {result:type}")]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Dialect)]
#[kirin(constant, fn = new, type_lattice = L)]
pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(default)]
    pub marker: std::marker::PhantomData<L>,
}
