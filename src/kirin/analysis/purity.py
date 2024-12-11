from kirin.lattice import (
    BoundedLattice,
    SimpleJoinMixin,
    SimpleMeetMixin,
    SingletonMeta,
)


class Purity(
    SimpleJoinMixin["Purity"], SimpleMeetMixin["Purity"], BoundedLattice["Purity"]
):

    @classmethod
    def bottom(cls) -> "Purity":
        return Unknown()

    @classmethod
    def top(cls) -> "Purity":
        return NotPure()


class Pure(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return isinstance(other, (NotPure, Pure))


class NotPure(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return isinstance(other, NotPure)


class Unknown(Purity, metaclass=SingletonMeta):

    def is_subseteq(self, other: Purity) -> bool:
        return True
