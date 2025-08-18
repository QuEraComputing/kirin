import math as pymath
from typing import TypeVar

from kirin import lowering
from kirin.dialects import ilist

from . import stmts as stmts, interp as interp
from ._dialect import dialect as dialect

pi = pymath.pi
e = pymath.e
tau = pymath.tau

ListLen = TypeVar("ListLen")


@lowering.wraps(stmts.acos)
def acos(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.asin)
def asin(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.asinh)
def asinh(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.atan)
def atan(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.atan2)
def atan2(
    y: ilist.IList[float, ListLen], x: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.atanh)
def atanh(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.ceil)
def ceil(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.copysign)
def copysign(
    x: ilist.IList[float, ListLen], y: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.cos)
def cos(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.cosh)
def cosh(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.degrees)
def degrees(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.erf)
def erf(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.erfc)
def erfc(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.exp)
def exp(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.expm1)
def expm1(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.fabs)
def fabs(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.floor)
def floor(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.fmod)
def fmod(
    x: ilist.IList[float, ListLen], y: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.gamma)
def gamma(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.isfinite)
def isfinite(x: ilist.IList[float, ListLen]) -> ilist.IList[bool, ListLen]: ...


@lowering.wraps(stmts.isinf)
def isinf(x: ilist.IList[float, ListLen]) -> ilist.IList[bool, ListLen]: ...


@lowering.wraps(stmts.isnan)
def isnan(x: ilist.IList[float, ListLen]) -> ilist.IList[bool, ListLen]: ...


@lowering.wraps(stmts.lgamma)
def lgamma(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.log10)
def log10(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.log1p)
def log1p(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.log2)
def log2(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.pow)
def pow(x: ilist.IList[float, ListLen], y: float) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.radians)
def radians(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.remainder)
def remainder(
    x: ilist.IList[float, ListLen], y: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.sin)
def sin(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.sinh)
def sinh(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.sqrt)
def sqrt(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.tan)
def tan(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.tanh)
def tanh(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.trunc)
def trunc(x: ilist.IList[float, ListLen]) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.scale)
def scale(
    value: float, x: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...


@lowering.wraps(stmts.offset)
def offset(
    value: float, x: ilist.IList[float, ListLen]
) -> ilist.IList[float, ListLen]: ...
