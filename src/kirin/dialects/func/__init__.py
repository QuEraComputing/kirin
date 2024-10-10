"""A function dialect that is compatible with python semantics.
"""

from kirin.dialects.func import (
    emit as emit,
    interp as interp,
    lower as lower,
    print as print,
    reachibility as reachibility,
    typeinfer as typeinfer,
)
from kirin.dialects.func.attrs import MethodType as MethodType, Signature as Signature
from kirin.dialects.func.dialect import dialect as dialect
from kirin.dialects.func.stmts import (
    Call as Call,
    Function as Function,
    GetField as GetField,
    Lambda as Lambda,
    Return as Return,
)
