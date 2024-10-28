"""A function dialect that is compatible with python semantics.
"""

from kirin.dialects.func import (
    constprop as constprop,
    emit as emit,
    interp as interp,
    lower as lower,
    typeinfer as typeinfer,
)
from kirin.dialects.func.attrs import MethodType as MethodType, Signature as Signature
from kirin.dialects.func.dialect import dialect as dialect
from kirin.dialects.func.stmts import (
    Call as Call,
    ConstantMethod as ConstantMethod,
    Function as Function,
    GetField as GetField,
    Lambda as Lambda,
    Return as Return,
)
