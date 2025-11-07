use crate::lattice::TypeLattice;

pub trait Language: Clone {
    type Type: TypeLattice;
}
