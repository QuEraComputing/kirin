//! The demand fact lattice.

use kirin_ir::{HasBottom, HasTop, Lattice};

/// The two-point demand lattice: `Dead` (bottom) ⊑ `Live` (top).
///
/// `join` is logical OR: a value is demanded if any of its uses demands it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Demand {
    /// The value is not demanded.
    #[default]
    Dead,
    /// The value is demanded (live).
    Live,
}

impl Demand {
    pub fn is_live(self) -> bool {
        matches!(self, Demand::Live)
    }
}

impl Lattice for Demand {
    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Demand::Live, _) | (_, Demand::Live) => Demand::Live,
            _ => Demand::Dead,
        }
    }

    fn meet(&self, other: &Self) -> Self {
        match (self, other) {
            (Demand::Dead, _) | (_, Demand::Dead) => Demand::Dead,
            _ => Demand::Live,
        }
    }

    fn is_subseteq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Demand::Dead, _) | (Demand::Live, Demand::Live)
        )
    }
}

impl HasBottom for Demand {
    fn bottom() -> Self {
        Demand::Dead
    }
}

impl HasTop for Demand {
    fn top() -> Self {
        Demand::Live
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_join_is_logical_or() {
        assert_eq!(Demand::Dead.join(&Demand::Dead), Demand::Dead);
        assert_eq!(Demand::Dead.join(&Demand::Live), Demand::Live);
        assert_eq!(Demand::Live.join(&Demand::Dead), Demand::Live);
        assert_eq!(Demand::Live.join(&Demand::Live), Demand::Live);
    }

    #[test]
    fn live_lattice_bottom_top_order() {
        assert_eq!(<Demand as HasBottom>::bottom(), Demand::Dead);
        assert_eq!(<Demand as HasTop>::top(), Demand::Live);
        assert!(Demand::Dead.is_subseteq(&Demand::Live));
        assert!(!Demand::Live.is_subseteq(&Demand::Dead));
    }
}
