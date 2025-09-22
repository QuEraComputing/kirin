import numpy as np
from scipy import special

from kirin import types
from kirin.interp import Frame, MethodTable, impl
from kirin.dialects import ilist

from . import stmts
from ._dialect import dialect


@dialect.register
class MathMethodTable(MethodTable):

    @impl(stmts.acos)
    def acos(self, interp, frame: Frame, stmt: stmts.acos):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.arccos(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.asin)
    def asin(self, interp, frame: Frame, stmt: stmts.asin):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.arcsin(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.asinh)
    def asinh(self, interp, frame: Frame, stmt: stmts.asinh):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.arcsinh(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.atan)
    def atan(self, interp, frame: Frame, stmt: stmts.atan):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.arctan(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.atan2)
    def atan2(self, interp, frame: Frame, stmt: stmts.atan2):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                np.arctan2(np.asarray(values[0]), np.asarray(values[1])).tolist(),
                elem=types.Float,
            ),
        )

    @impl(stmts.atanh)
    def atanh(self, interp, frame: Frame, stmt: stmts.atanh):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.arctanh(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.ceil)
    def ceil(self, interp, frame: Frame, stmt: stmts.ceil):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.ceil(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.copysign)
    def copysign(self, interp, frame: Frame, stmt: stmts.copysign):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                np.copysign(np.asarray(values[0]), np.asarray(values[1])).tolist(),
                elem=types.Float,
            ),
        )

    @impl(stmts.cos)
    def cos(self, interp, frame: Frame, stmt: stmts.cos):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.cos(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.cosh)
    def cosh(self, interp, frame: Frame, stmt: stmts.cosh):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.cosh(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.degrees)
    def degrees(self, interp, frame: Frame, stmt: stmts.degrees):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.degrees(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.erf)
    def erf(self, interp, frame: Frame, stmt: stmts.erf):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(special.erf(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.erfc)
    def erfc(self, interp, frame: Frame, stmt: stmts.erfc):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(special.erfc(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.exp)
    def exp(self, interp, frame: Frame, stmt: stmts.exp):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.exp(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.expm1)
    def expm1(self, interp, frame: Frame, stmt: stmts.expm1):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.expm1(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.fabs)
    def fabs(self, interp, frame: Frame, stmt: stmts.fabs):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.fabs(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.floor)
    def floor(self, interp, frame: Frame, stmt: stmts.floor):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.floor(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.fmod)
    def fmod(self, interp, frame: Frame, stmt: stmts.fmod):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                np.fmod(np.asarray(values[0]), np.asarray(values[1])).tolist(),
                elem=types.Float,
            ),
        )

    @impl(stmts.gamma)
    def gamma(self, interp, frame: Frame, stmt: stmts.gamma):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                special.gamma(np.asarray(values[0])).tolist(), elem=types.Float
            ),
        )

    @impl(stmts.isfinite)
    def isfinite(self, interp, frame: Frame, stmt: stmts.isfinite):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.isfinite(np.asarray(values[0])).tolist(), elem=types.Bool),
        )

    @impl(stmts.isinf)
    def isinf(self, interp, frame: Frame, stmt: stmts.isinf):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.isinf(np.asarray(values[0])).tolist(), elem=types.Bool),)

    @impl(stmts.isnan)
    def isnan(self, interp, frame: Frame, stmt: stmts.isnan):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.isnan(np.asarray(values[0])).tolist(), elem=types.Bool),)

    @impl(stmts.lgamma)
    def lgamma(self, interp, frame: Frame, stmt: stmts.lgamma):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                special.loggamma(np.asarray(values[0])).tolist(), elem=types.Float
            ),
        )

    @impl(stmts.log10)
    def log10(self, interp, frame: Frame, stmt: stmts.log10):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.log10(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.log1p)
    def log1p(self, interp, frame: Frame, stmt: stmts.log1p):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.log1p(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.log2)
    def log2(self, interp, frame: Frame, stmt: stmts.log2):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.log2(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.pow)
    def pow(self, interp, frame: Frame, stmt: stmts.pow):
        x = frame.get(stmt.x)
        y = frame.get(stmt.y)
        return (
            ilist.IList(
                np.power(np.asarray(x), y).tolist(),
                elem=types.Float,
            ),
        )

    @impl(stmts.radians)
    def radians(self, interp, frame: Frame, stmt: stmts.radians):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.radians(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.remainder)
    def remainder(self, interp, frame: Frame, stmt: stmts.remainder):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(
                np.remainder(np.asarray(values[0]), np.asarray(values[1])).tolist(),
                elem=types.Float,
            ),
        )

    @impl(stmts.sin)
    def sin(self, interp, frame: Frame, stmt: stmts.sin):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.sin(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.sinh)
    def sinh(self, interp, frame: Frame, stmt: stmts.sinh):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.sinh(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.sqrt)
    def sqrt(self, interp, frame: Frame, stmt: stmts.sqrt):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.sqrt(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.tan)
    def tan(self, interp, frame: Frame, stmt: stmts.tan):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.tan(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.tanh)
    def tanh(self, interp, frame: Frame, stmt: stmts.tanh):
        values = frame.get_values(stmt.args)
        return (ilist.IList(np.tanh(np.asarray(values[0])).tolist(), elem=types.Float),)

    @impl(stmts.trunc)
    def trunc(self, interp, frame: Frame, stmt: stmts.trunc):
        values = frame.get_values(stmt.args)
        return (
            ilist.IList(np.trunc(np.asarray(values[0])).tolist(), elem=types.Float),
        )

    @impl(stmts.scale)
    def scale(self, interp, frame: Frame, stmt: stmts.scale):
        a = frame.get(stmt.value)
        x = frame.get(stmt.x)
        return (ilist.IList((np.asarray(x) * a).tolist(), elem=types.Float),)

    @impl(stmts.offset)
    def offset(self, interp, frame: Frame, stmt: stmts.offset):
        a = frame.get(stmt.value)
        x = frame.get(stmt.x)
        return (ilist.IList((np.asarray(x) + a).tolist(), elem=types.Float),)
