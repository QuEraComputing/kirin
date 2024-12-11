# type: ignore
# This file is generated by gen.py
import math as pymath

from kirin.dialects import math
from kirin.prelude import basic


@basic
def acos_func(x):
    return math.acos(x)


def test_acos():
    truth = pymath.acos(0.42)
    assert (acos_func(0.42) - truth) < 1e-6


@basic
def asin_func(x):
    return math.asin(x)


def test_asin():
    truth = pymath.asin(0.42)
    assert (asin_func(0.42) - truth) < 1e-6


@basic
def asinh_func(x):
    return math.asinh(x)


def test_asinh():
    truth = pymath.asinh(0.42)
    assert (asinh_func(0.42) - truth) < 1e-6


@basic
def atan_func(x):
    return math.atan(x)


def test_atan():
    truth = pymath.atan(0.42)
    assert (atan_func(0.42) - truth) < 1e-6


@basic
def atan2_func(y, x):
    return math.atan2(y, x)


def test_atan2():
    truth = pymath.atan2(0.42, 0.42)
    assert (atan2_func(0.42, 0.42) - truth) < 1e-6


@basic
def atanh_func(x):
    return math.atanh(x)


def test_atanh():
    truth = pymath.atanh(0.42)
    assert (atanh_func(0.42) - truth) < 1e-6


@basic
def cbrt_func(x):
    return math.cbrt(x)


def test_cbrt():
    truth = pymath.cbrt(0.42)
    assert (cbrt_func(0.42) - truth) < 1e-6


@basic
def ceil_func(x):
    return math.ceil(x)


def test_ceil():
    truth = pymath.ceil(0.42)
    assert (ceil_func(0.42) - truth) < 1e-6


@basic
def copysign_func(x, y):
    return math.copysign(x, y)


def test_copysign():
    truth = pymath.copysign(0.42, 0.42)
    assert (copysign_func(0.42, 0.42) - truth) < 1e-6


@basic
def cos_func(x):
    return math.cos(x)


def test_cos():
    truth = pymath.cos(0.42)
    assert (cos_func(0.42) - truth) < 1e-6


@basic
def cosh_func(x):
    return math.cosh(x)


def test_cosh():
    truth = pymath.cosh(0.42)
    assert (cosh_func(0.42) - truth) < 1e-6


@basic
def degrees_func(x):
    return math.degrees(x)


def test_degrees():
    truth = pymath.degrees(0.42)
    assert (degrees_func(0.42) - truth) < 1e-6


@basic
def erf_func(x):
    return math.erf(x)


def test_erf():
    truth = pymath.erf(0.42)
    assert (erf_func(0.42) - truth) < 1e-6


@basic
def erfc_func(x):
    return math.erfc(x)


def test_erfc():
    truth = pymath.erfc(0.42)
    assert (erfc_func(0.42) - truth) < 1e-6


@basic
def exp_func(x):
    return math.exp(x)


def test_exp():
    truth = pymath.exp(0.42)
    assert (exp_func(0.42) - truth) < 1e-6


@basic
def exp2_func(x):
    return math.exp2(x)


def test_exp2():
    truth = pymath.exp2(0.42)
    assert (exp2_func(0.42) - truth) < 1e-6


@basic
def expm1_func(x):
    return math.expm1(x)


def test_expm1():
    truth = pymath.expm1(0.42)
    assert (expm1_func(0.42) - truth) < 1e-6


@basic
def fabs_func(x):
    return math.fabs(x)


def test_fabs():
    truth = pymath.fabs(0.42)
    assert (fabs_func(0.42) - truth) < 1e-6


@basic
def floor_func(x):
    return math.floor(x)


def test_floor():
    truth = pymath.floor(0.42)
    assert (floor_func(0.42) - truth) < 1e-6


@basic
def fmod_func(x, y):
    return math.fmod(x, y)


def test_fmod():
    truth = pymath.fmod(0.42, 0.42)
    assert (fmod_func(0.42, 0.42) - truth) < 1e-6


@basic
def gamma_func(x):
    return math.gamma(x)


def test_gamma():
    truth = pymath.gamma(0.42)
    assert (gamma_func(0.42) - truth) < 1e-6


@basic
def isfinite_func(x):
    return math.isfinite(x)


def test_isfinite():
    truth = pymath.isfinite(0.42)
    assert (isfinite_func(0.42) - truth) < 1e-6


@basic
def isinf_func(x):
    return math.isinf(x)


def test_isinf():
    truth = pymath.isinf(0.42)
    assert (isinf_func(0.42) - truth) < 1e-6


@basic
def isnan_func(x):
    return math.isnan(x)


def test_isnan():
    truth = pymath.isnan(0.42)
    assert (isnan_func(0.42) - truth) < 1e-6


@basic
def lgamma_func(x):
    return math.lgamma(x)


def test_lgamma():
    truth = pymath.lgamma(0.42)
    assert (lgamma_func(0.42) - truth) < 1e-6


@basic
def log10_func(x):
    return math.log10(x)


def test_log10():
    truth = pymath.log10(0.42)
    assert (log10_func(0.42) - truth) < 1e-6


@basic
def log1p_func(x):
    return math.log1p(x)


def test_log1p():
    truth = pymath.log1p(0.42)
    assert (log1p_func(0.42) - truth) < 1e-6


@basic
def log2_func(x):
    return math.log2(x)


def test_log2():
    truth = pymath.log2(0.42)
    assert (log2_func(0.42) - truth) < 1e-6


@basic
def pow_func(x, y):
    return math.pow(x, y)


def test_pow():
    truth = pymath.pow(0.42, 0.42)
    assert (pow_func(0.42, 0.42) - truth) < 1e-6


@basic
def radians_func(x):
    return math.radians(x)


def test_radians():
    truth = pymath.radians(0.42)
    assert (radians_func(0.42) - truth) < 1e-6


@basic
def remainder_func(x, y):
    return math.remainder(x, y)


def test_remainder():
    truth = pymath.remainder(0.42, 0.42)
    assert (remainder_func(0.42, 0.42) - truth) < 1e-6


@basic
def sin_func(x):
    return math.sin(x)


def test_sin():
    truth = pymath.sin(0.42)
    assert (sin_func(0.42) - truth) < 1e-6


@basic
def sinh_func(x):
    return math.sinh(x)


def test_sinh():
    truth = pymath.sinh(0.42)
    assert (sinh_func(0.42) - truth) < 1e-6


@basic
def sqrt_func(x):
    return math.sqrt(x)


def test_sqrt():
    truth = pymath.sqrt(0.42)
    assert (sqrt_func(0.42) - truth) < 1e-6


@basic
def tan_func(x):
    return math.tan(x)


def test_tan():
    truth = pymath.tan(0.42)
    assert (tan_func(0.42) - truth) < 1e-6


@basic
def tanh_func(x):
    return math.tanh(x)


def test_tanh():
    truth = pymath.tanh(0.42)
    assert (tanh_func(0.42) - truth) < 1e-6


@basic
def trunc_func(x):
    return math.trunc(x)


def test_trunc():
    truth = pymath.trunc(0.42)
    assert (trunc_func(0.42) - truth) < 1e-6


@basic
def ulp_func(x):
    return math.ulp(x)


def test_ulp():
    truth = pymath.ulp(0.42)
    assert (ulp_func(0.42) - truth) < 1e-6
