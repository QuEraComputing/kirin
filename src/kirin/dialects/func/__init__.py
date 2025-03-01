"""A function dialect that is compatible with python semantics."""

from kirin.dialects.func import (
    emit as emit,
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
)
from kirin.dialects.func.dialect import dialect as dialect
