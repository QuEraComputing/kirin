from dataclasses import field, dataclass

from kirin.ir import types


@dataclass
class TypeResolutionResult:
    pass


@dataclass
class ResolutionOk(TypeResolutionResult):

    def __bool__(self):
        return True


Ok = ResolutionOk()


@dataclass
class ResolutionError(TypeResolutionResult):
    expr: types.TypeAttribute
    value: types.TypeAttribute

    def __bool__(self):
        return False

    def __str__(self):
        return f"expected {self.expr}, got {self.value}"


@dataclass
class TypeResolution:
    vars: dict[types.TypeVar, types.TypeAttribute] = field(default_factory=dict)

    def substitute(self, typ: types.TypeAttribute) -> types.TypeAttribute:
        if isinstance(typ, types.TypeVar):
            return self.vars.get(typ, typ)
        elif isinstance(typ, types.Generic):
            return types.Generic(
                typ.body, *tuple(self.substitute(var) for var in typ.vars)
            )
        elif isinstance(typ, types.Union):
            return types.Union(self.substitute(t) for t in typ.types)
        return typ

    def solve(
        self, annot: types.TypeAttribute, value: types.TypeAttribute
    ) -> TypeResolutionResult:
        if isinstance(annot, types.TypeVar):
            return self.solve_TypeVar(annot, value)
        elif isinstance(annot, types.Generic):
            return self.solve_Generic(annot, value)
        elif isinstance(annot, types.Union):
            return self.solve_Union(annot, value)

        if annot.is_subseteq(value):
            return Ok
        else:
            return ResolutionError(annot, value)

    def solve_TypeVar(self, annot: types.TypeVar, value: types.TypeAttribute):
        if annot in self.vars:
            if value.is_subseteq(self.vars[annot]):
                self.vars[annot] = value
            elif self.vars[annot].is_subseteq(value):
                pass
            else:
                return ResolutionError(annot, value)
        else:
            self.vars[annot] = value
        return Ok

    def solve_Generic(self, annot: types.Generic, value: types.TypeAttribute):
        if not isinstance(value, types.Generic):
            return ResolutionError(annot, value)

        if not value.body.is_subseteq(annot.body):
            return ResolutionError(annot.body, value.body)

        for var, val in zip(annot.vars, value.vars):
            result = self.solve(var, val)
            if not result:
                return result

        if not annot.vararg:
            return Ok

        for val in value.vars[len(annot.vars) :]:
            result = self.solve(annot.vararg.typ, val)
            if not result:
                return result
        return Ok

    def solve_Union(self, annot: types.Union, value: types.TypeAttribute):
        for typ in annot.types:
            result = self.solve(typ, value)
            if result:
                return Ok
        return ResolutionError(annot, value)
