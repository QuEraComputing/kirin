import numpy as np
from scipy import special

from kirin import types
from kirin.prelude import basic
from kirin.dialects import ilist, vmath


@basic
def acos_func(x):
    return vmath.acos(x)


def test_acos():
    truth = np.acos(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        acos_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def asin_func(x):
    return vmath.asin(x)


def test_asin():
    truth = np.asin(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        asin_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def asinh_func(x):
    return vmath.asinh(x)


def test_asinh():
    truth = np.asinh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        asinh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def atan_func(x):
    return vmath.atan(x)


def test_atan():
    truth = np.atan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        atan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def atan2_func(y, x):
    return vmath.atan2(y, x)


def test_atan2():
    truth = np.atan2(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert np.allclose(
        atan2_func(
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ),
        truth,
    )


@basic
def atanh_func(x):
    return vmath.atanh(x)


def test_atanh():
    truth = np.atanh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        atanh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def ceil_func(x):
    return vmath.ceil(x)


def test_ceil():
    truth = np.ceil(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        ceil_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def copysign_func(x, y):
    return vmath.copysign(x, y)


def test_copysign():
    truth = np.copysign(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert np.allclose(
        copysign_func(
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ),
        truth,
    )


@basic
def cos_func(x):
    return vmath.cos(x)


def test_cos():
    truth = np.cos(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        cos_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def cosh_func(x):
    return vmath.cosh(x)


def test_cosh():
    truth = np.cosh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        cosh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def degrees_func(x):
    return vmath.degrees(x)


def test_degrees():
    truth = np.degrees(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        degrees_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def erf_func(x):
    return vmath.erf(x)


def test_erf():
    truth = special.erf(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        erf_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def erfc_func(x):
    return vmath.erfc(x)


def test_erfc():
    truth = special.erfc(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        erfc_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def exp_func(x):
    return vmath.exp(x)


def test_exp():
    truth = np.exp(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        exp_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def expm1_func(x):
    return vmath.expm1(x)


def test_expm1():
    truth = np.expm1(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        expm1_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def fabs_func(x):
    return vmath.fabs(x)


def test_fabs():
    truth = np.fabs(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        fabs_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def floor_func(x):
    return vmath.floor(x)


def test_floor():
    truth = np.floor(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        floor_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def fmod_func(x, y):
    return vmath.fmod(x, y)


def test_fmod():
    truth = np.fmod(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert np.allclose(
        fmod_func(
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ),
        truth,
    )


@basic
def gamma_func(x):
    return vmath.gamma(x)


def test_gamma():
    truth = special.gamma(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        gamma_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def isfinite_func(x):
    return vmath.isfinite(x)


def test_isfinite():
    truth = np.isfinite(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        isfinite_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def isinf_func(x):
    return vmath.isinf(x)


def test_isinf():
    truth = np.isinf(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        isinf_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def isnan_func(x):
    return vmath.isnan(x)


def test_isnan():
    truth = np.isnan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        isnan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def lgamma_func(x):
    return vmath.lgamma(x)


def test_lgamma():
    truth = special.loggamma(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        lgamma_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def log10_func(x):
    return vmath.log10(x)


def test_log10():
    truth = np.log10(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        log10_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def log1p_func(x):
    return vmath.log1p(x)


def test_log1p():
    truth = np.log1p(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        log1p_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def log2_func(x):
    return vmath.log2(x)


def test_log2():
    truth = np.log2(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        log2_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def pow_func(x, y):
    return vmath.pow(x, y)


def test_pow():
    truth = np.pow(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert np.allclose(
        pow_func(
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ),
        truth,
    )


@basic
def radians_func(x):
    return vmath.radians(x)


def test_radians():
    truth = np.radians(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        radians_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def remainder_func(x, y):
    return vmath.remainder(x, y)


def test_remainder():
    truth = np.remainder(
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
    )
    assert np.allclose(
        remainder_func(
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
            ilist.IList([0.42, 0.87, 0.32], elem=types.Float),
        ),
        truth,
    )


@basic
def sin_func(x):
    return vmath.sin(x)


def test_sin():
    truth = np.sin(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        sin_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def sinh_func(x):
    return vmath.sinh(x)


def test_sinh():
    truth = np.sinh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        sinh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def sqrt_func(x):
    return vmath.sqrt(x)


def test_sqrt():
    truth = np.sqrt(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        sqrt_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def tan_func(x):
    return vmath.tan(x)


def test_tan():
    truth = np.tan(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        tan_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def tanh_func(x):
    return vmath.tanh(x)


def test_tanh():
    truth = np.tanh(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        tanh_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )


@basic
def trunc_func(x):
    return vmath.trunc(x)


def test_trunc():
    truth = np.trunc(ilist.IList([0.42, 0.87, 0.32], elem=types.Float))
    assert np.allclose(
        trunc_func(ilist.IList([0.42, 0.87, 0.32], elem=types.Float)), truth
    )
