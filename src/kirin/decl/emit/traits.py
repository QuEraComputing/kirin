from kirin.decl.base import BaseModifier

from ._set_new_attribute import set_new_attribute


class EmitTraits(BaseModifier):

    def emit_traits(self):
        # if no parent defines traits, set it to empty set
        for base in self.cls.__mro__[-1:0:-1]:
            if hasattr(base, "traits"):
                return
        set_new_attribute(self.cls, "traits", frozenset({}))
