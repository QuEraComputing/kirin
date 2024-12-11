# This file is generated by gen.py
from kirin import ir
from kirin.decl import info, statement
from kirin.dialects.math.dialect import dialect
from kirin.ir.types import Float


@statement(dialect=dialect)
class acos(ir.Statement):
    """acos statement, wrapping the math.acos function"""

    name = "acos"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class asin(ir.Statement):
    """asin statement, wrapping the math.asin function"""

    name = "asin"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class asinh(ir.Statement):
    """asinh statement, wrapping the math.asinh function"""

    name = "asinh"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class atan(ir.Statement):
    """atan statement, wrapping the math.atan function"""

    name = "atan"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class atan2(ir.Statement):
    """atan2 statement, wrapping the math.atan2 function"""

    name = "atan2"
    traits = frozenset({ir.Pure()})
    y: ir.SSAValue = info.argument(Float)
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class atanh(ir.Statement):
    """atanh statement, wrapping the math.atanh function"""

    name = "atanh"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class cbrt(ir.Statement):
    """cbrt statement, wrapping the math.cbrt function"""

    name = "cbrt"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class ceil(ir.Statement):
    """ceil statement, wrapping the math.ceil function"""

    name = "ceil"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class copysign(ir.Statement):
    """copysign statement, wrapping the math.copysign function"""

    name = "copysign"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    y: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class cos(ir.Statement):
    """cos statement, wrapping the math.cos function"""

    name = "cos"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class cosh(ir.Statement):
    """cosh statement, wrapping the math.cosh function"""

    name = "cosh"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class degrees(ir.Statement):
    """degrees statement, wrapping the math.degrees function"""

    name = "degrees"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class erf(ir.Statement):
    """erf statement, wrapping the math.erf function"""

    name = "erf"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class erfc(ir.Statement):
    """erfc statement, wrapping the math.erfc function"""

    name = "erfc"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class exp(ir.Statement):
    """exp statement, wrapping the math.exp function"""

    name = "exp"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class exp2(ir.Statement):
    """exp2 statement, wrapping the math.exp2 function"""

    name = "exp2"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class expm1(ir.Statement):
    """expm1 statement, wrapping the math.expm1 function"""

    name = "expm1"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class fabs(ir.Statement):
    """fabs statement, wrapping the math.fabs function"""

    name = "fabs"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class floor(ir.Statement):
    """floor statement, wrapping the math.floor function"""

    name = "floor"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class fmod(ir.Statement):
    """fmod statement, wrapping the math.fmod function"""

    name = "fmod"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    y: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class gamma(ir.Statement):
    """gamma statement, wrapping the math.gamma function"""

    name = "gamma"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class isfinite(ir.Statement):
    """isfinite statement, wrapping the math.isfinite function"""

    name = "isfinite"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class isinf(ir.Statement):
    """isinf statement, wrapping the math.isinf function"""

    name = "isinf"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class isnan(ir.Statement):
    """isnan statement, wrapping the math.isnan function"""

    name = "isnan"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class lgamma(ir.Statement):
    """lgamma statement, wrapping the math.lgamma function"""

    name = "lgamma"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class log10(ir.Statement):
    """log10 statement, wrapping the math.log10 function"""

    name = "log10"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class log1p(ir.Statement):
    """log1p statement, wrapping the math.log1p function"""

    name = "log1p"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class log2(ir.Statement):
    """log2 statement, wrapping the math.log2 function"""

    name = "log2"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class pow(ir.Statement):
    """pow statement, wrapping the math.pow function"""

    name = "pow"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    y: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class radians(ir.Statement):
    """radians statement, wrapping the math.radians function"""

    name = "radians"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class remainder(ir.Statement):
    """remainder statement, wrapping the math.remainder function"""

    name = "remainder"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    y: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class sin(ir.Statement):
    """sin statement, wrapping the math.sin function"""

    name = "sin"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class sinh(ir.Statement):
    """sinh statement, wrapping the math.sinh function"""

    name = "sinh"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class sqrt(ir.Statement):
    """sqrt statement, wrapping the math.sqrt function"""

    name = "sqrt"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class tan(ir.Statement):
    """tan statement, wrapping the math.tan function"""

    name = "tan"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class tanh(ir.Statement):
    """tanh statement, wrapping the math.tanh function"""

    name = "tanh"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class trunc(ir.Statement):
    """trunc statement, wrapping the math.trunc function"""

    name = "trunc"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)


@statement(dialect=dialect)
class ulp(ir.Statement):
    """ulp statement, wrapping the math.ulp function"""

    name = "ulp"
    traits = frozenset({ir.Pure()})
    x: ir.SSAValue = info.argument(Float)
    result: ir.ResultValue = info.result(Float)
