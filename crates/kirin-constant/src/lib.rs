use kirin::ir::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Statement)]
#[kirin(constant, fn = new, type_lattice = L)]
pub struct Constant<T: CompileTimeValue + Typeof<L>, L: TypeLattice> {
    #[kirin(into)]
    pub value: T,
    #[kirin(type = value.type_of())]
    pub result: ResultValue,
    #[kirin(default = std::marker::PhantomData)]
    pub marker: std::marker::PhantomData<L>,
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_constant_results() {
//         let const_instr = Constant { value: 42u32, result: TestSSAValue(1).into(), marker: std::marker::PhantomData };
//         let results = const_instr.results().collect::<Vec<_>>();
//         assert_eq!(results, vec![&(TestSSAValue(1).into())]);
//     }
// }
