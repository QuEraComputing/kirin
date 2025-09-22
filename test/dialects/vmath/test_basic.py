import numpy as np
from scipy import special

from kirin import types
from kirin.prelude import basic
from kirin.dialects import ilist, vmath


@basic.union([vmath])
def acos_func(x):
    return vmath.acos(x)


def test_acos():
    truth = np.arccos(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = acos_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def asin_func(x):
    return vmath.asin(x)


def test_asin():
    truth = np.arcsin(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = asin_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def asinh_func(x):
    return vmath.asinh(x)


def test_asinh():
    truth = np.arcsinh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = asinh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def atan_func(x):
    return vmath.atan(x)


def test_atan():
    truth = np.arctan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = atan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def atan2_func(y, x):
    return vmath.atan2(y, x)


def test_atan2():
    truth = np.arctan2(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    out = atan2_func(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def atanh_func(x):
    return vmath.atanh(x)


def test_atanh():
    truth = np.arctanh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = atanh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def ceil_func(x):
    return vmath.ceil(x)


def test_ceil():
    truth = np.ceil(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = ceil_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def copysign_func(x, y):
    return vmath.copysign(x, y)


def test_copysign():
    truth = np.copysign(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    out = copysign_func(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def cos_func(x):
    return vmath.cos(x)


def test_cos():
    truth = np.cos(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = cos_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def cosh_func(x):
    return vmath.cosh(x)


def test_cosh():
    truth = np.cosh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = cosh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def degrees_func(x):
    return vmath.degrees(x)


def test_degrees():
    truth = np.degrees(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = degrees_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def erf_func(x):
    return vmath.erf(x)


def test_erf():
    truth = special.erf(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = erf_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def erfc_func(x):
    return vmath.erfc(x)


def test_erfc():
    truth = special.erfc(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = erfc_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def exp_func(x):
    return vmath.exp(x)


def test_exp():
    truth = np.exp(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = exp_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def expm1_func(x):
    return vmath.expm1(x)


def test_expm1():
    truth = np.expm1(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = expm1_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def fabs_func(x):
    return vmath.fabs(x)


def test_fabs():
    truth = np.fabs(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = fabs_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def floor_func(x):
    return vmath.floor(x)


def test_floor():
    truth = np.floor(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = floor_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def fmod_func(x, y):
    return vmath.fmod(x, y)


def test_fmod():
    truth = np.fmod(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    out = fmod_func(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def gamma_func(x):
    return vmath.gamma(x)


def test_gamma():
    truth = special.gamma(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = gamma_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def isfinite_func(x):
    return vmath.isfinite(x)


def test_isfinite():
    truth = np.isfinite(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = isfinite_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Bool
    assert np.allclose(out, truth)


@basic.union([vmath])
def isinf_func(x):
    return vmath.isinf(x)


def test_isinf():
    truth = np.isinf(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = isinf_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Bool
    assert np.allclose(out, truth)


@basic.union([vmath])
def isnan_func(x):
    return vmath.isnan(x)


def test_isnan():
    truth = np.isnan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = isnan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Bool
    assert np.allclose(out, truth)


@basic.union([vmath])
def lgamma_func(x):
    return vmath.lgamma(x)


def test_lgamma():
    truth = special.loggamma(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = lgamma_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def log10_func(x):
    return vmath.log10(x)


def test_log10():
    truth = np.log10(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = log10_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def log1p_func(x):
    return vmath.log1p(x)


def test_log1p():
    truth = np.log1p(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = log1p_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def log2_func(x):
    return vmath.log2(x)


def test_log2():
    truth = np.log2(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = log2_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def pow_func(x, y):
    return vmath.pow(x, y)


def test_pow():
    truth = np.power(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        3.33,
    )
    out = pow_func(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        3.33,
    )
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def radians_func(x):
    return vmath.radians(x)


def test_radians():
    truth = np.radians(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = radians_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def remainder_func(x, y):
    return vmath.remainder(x, y)


def test_remainder():
    truth = np.remainder(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    out = remainder_func(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def sin_func(x):
    return vmath.sin(x)


def test_sin():
    truth = np.sin(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = sin_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def sinh_func(x):
    return vmath.sinh(x)


def test_sinh():
    truth = np.sinh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = sinh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def sqrt_func(x):
    return vmath.sqrt(x)


def test_sqrt():
    truth = np.sqrt(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = sqrt_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def tan_func(x):
    return vmath.tan(x)


def test_tan():
    truth = np.tan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = tan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def tanh_func(x):
    return vmath.tanh(x)


def test_tanh():
    truth = np.tanh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = tanh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def trunc_func(x):
    return vmath.trunc(x)


def test_trunc():
    truth = np.trunc(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    out = trunc_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def scale_func(x, y):
    return vmath.scale(value=y, x=x)


def test_scale():
    a = 3.3
    truth = np.array(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)) * a
    out = scale_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float), a)
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)


@basic.union([vmath])
def offset_func(x, y):
    return vmath.offset(value=y, x=x)


def test_offset():
    a = 3.3
    truth = np.array(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)) + a
    out = offset_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float), a)
    assert isinstance(out, ilist.IList)
    assert out.elem == types.Float
    assert np.allclose(out, truth)
