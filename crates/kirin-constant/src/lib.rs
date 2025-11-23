use kirin::ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Statement)]
#[kirin(constant, fn = new, type_lattice = L)]
pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(init = std::marker::PhantomData)]
    pub marker: std::marker::PhantomData<L>,
}

impl<T: CompileTimeValue + Typeof<L>, L: TypeLattice> Constant<T, L> {
    pub fn new<Lang: Language + From<Constant<T, L>>>(
        arena: &mut Arena<Lang>,
        value: T,
    ) -> (StatementId, ResultValue)
    where
        Lang::TypeLattice: From<L>,
    {
        let parent = arena.new_statement_id();
        let result: ResultValue = arena
            .ssa()
            .kind(SSAKind::Result(parent, 0))
            .ty(Lang::TypeLattice::from(value.type_of()))
            .new()
            .into();
        let stmt = arena
            .statement()
            .definition(Constant { value, result, marker: Default::default() })
            .new();
        (stmt, result)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_constant_results() {
//         let const_instr = Constant { value: 42u32, result: TestSSAValue(1).into() };
//         let results = const_instr.results().collect::<Vec<_>>();
//         assert_eq!(results, vec![&(TestSSAValue(1).into())]);
//     }
// }
