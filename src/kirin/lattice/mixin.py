from typing import TypeVar

from .abc import BoundedLattice

BoundedLatticeType = TypeVar("BoundedLatticeType", bound="BoundedLattice")


class IsSubsetEqMixin(BoundedLattice[BoundedLatticeType]):
    """A special mixin for lattices that provides a default implementation for `is_subseteq`
    by using the visitor pattern. This is useful if the lattice has a lot of different
    subclasses that need to be compared.

    Must be used before `BoundedLattice` in the inheritance chain.
    """

    def is_subseteq(self, other: BoundedLatticeType) -> bool:
        if other is self.top():
            return True
        elif other is self.bottom():
            return False

        method = getattr(
            self,
            "is_subseteq_" + other.__class__.__name__,
            getattr(self, "is_subseteq_fallback", None),
        )
        if method is not None:
            return method(other)
        return False


class SimpleJoinMixin(BoundedLattice[BoundedLatticeType]):
    """A mixin that provides a simple implementation for the join operation."""

    def join(self, other: BoundedLatticeType) -> BoundedLatticeType:
        if self.is_subseteq(other):
            return other
        elif other.is_subseteq(self):
            return self  # type: ignore
        return self.top()


class SimpleMeetMixin(BoundedLattice[BoundedLatticeType]):
    """A mixin that provides a simple implementation for the meet operation."""

    def meet(self, other: BoundedLatticeType) -> BoundedLatticeType:
        if self.is_subseteq(other):
            return self  # type: ignore
        elif other.is_subseteq(self):
            return other
        return self.bottom()
