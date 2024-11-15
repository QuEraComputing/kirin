from kirin.codegen import CodeGen, DialectEmit, impl

from .dialect import dialect
from .elem import (
    PyAnyType,
    PyBottomType,
    PyClass,
    PyGeneric,
    PyLiteral,
    PyTypeVar,
    PyVararg,
)


@dialect.register(key="dict")
class EmitDict(DialectEmit[CodeGen[dict], dict]):

    @impl(PyClass)
    def emit_class(self, emit: CodeGen[dict], stmt: PyClass):
        return {"name": stmt.name, "value": f"{stmt.name}.{stmt.typ.__name__}"}

    @impl(PyAnyType)
    def emit_anytype(self, emit: CodeGen[dict], stmt: PyAnyType):
        return {"name": "Any"}

    @impl(PyBottomType)
    def emit_bottomtype(self, emit: CodeGen[dict], stmt: PyBottomType):
        return {"name": "Bottom"}

    @impl(PyGeneric)
    def emit_generic(self, emit: CodeGen[dict], stmt: PyGeneric):
        return {
            "name": stmt.name,
            "body": emit.emit_Attribute(stmt.body),
            "vars": [emit.emit_Attribute(var) for var in stmt.vars],
            "varargs": emit.emit_Attribute(stmt.vararg) if stmt.vararg else None,
        }

    @impl(PyLiteral)
    def emit_literal(self, emit: CodeGen[dict], stmt: PyLiteral):
        return {"name": stmt.data}

    @impl(PyTypeVar)
    def emit_typevar(self, emit: CodeGen[dict], stmt: PyTypeVar):
        return {"name": stmt.name, "bound": emit.emit_Attribute(stmt.bound)}

    @impl(PyVararg)
    def emit_vararg(self, emit: CodeGen[dict], stmt: PyVararg):
        return {"name": "PyVararg", "type": emit.emit_Attribute(stmt.typ)}
