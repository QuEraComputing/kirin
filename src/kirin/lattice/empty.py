from kirin.lattice.abc import SingletonMeta, BoundedLattice


class EmptyLattice(BoundedLattice["EmptyLattice"], metaclass=SingletonMeta):
    """Empty lattice."""

    def join(self, other: "EmptyLattice") -> "EmptyLattice":
        return self

    def meet(self, other: "EmptyLattice") -> "EmptyLattice":
        return self

    @classmethod
    def bottom(cls):
        return cls()

    @classmethod
    def top(cls):
        return cls()

    def __hash__(self) -> int:
        return id(self)

    def is_subseteq(self, other: "EmptyLattice") -> bool:
        return True
