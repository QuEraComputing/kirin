"""A function dialect that is compatible with python semantics."""

from kirin.dialects.func import (
    interp as interp,
    constprop as constprop,
    typeinfer as typeinfer,
)
from kirin.dialects.func.attrs import Signature as Signature, MethodType as MethodType
from kirin.dialects.func.stmts import (
    Call as Call,
    Invoke as Invoke,
    Lambda as Lambda,
    Return as Return,
    Function as Function,
    GetField as GetField,
    ConstantNone as ConstantNone,
    FuncOpCallableInterface as FuncOpCallableInterface,
)
from kirin.dialects.func._dialect import dialect as dialect

from . import _julia as _julia
