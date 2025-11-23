use kirin::ir::*;

#[derive(Clone, Debug, Statement)]
pub enum Circuit {
    X(SSAValue),
    Y(SSAValue),
    Z(SSAValue),
    H(SSAValue),
    S(SSAValue),
    CNOT(SSAValue),
    Rx(SSAValue, SSAValue),
    Ry(SSAValue, SSAValue),
    Rz(SSAValue, SSAValue),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum CircuitType {
    Qubit,
    Angle,
    Bottom,
    Top,
}

impl Lattice for CircuitType {
    fn is_subseteq(&self, other: &Self) -> bool {
        matches!((self, other), (a, b) if a == b)
    }
    fn join(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            other.clone()
        } else if other.is_subseteq(self) {
            self.clone()
        } else {
            panic!("No join for different CircuitTypes")
        }
    }
    fn meet(&self, other: &Self) -> Self {
        if self.is_subseteq(other) {
            self.clone()
        } else if other.is_subseteq(self) {
            other.clone()
        } else {
            panic!("No meet for different CircuitTypes")
        }
    }
}
impl FiniteLattice for CircuitType {
    fn bottom() -> Self {
        CircuitType::Bottom
    }
    fn top() -> Self {
        CircuitType::Top
    }
}
impl CompileTimeValue for CircuitType {}
impl TypeLattice for CircuitType {}

impl Language for Circuit {
    type TypeLattice = CircuitType;
}
