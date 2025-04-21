import numpy as np
import pytest

# composite modules
from aha import gaga, hoho

from kirin.prelude import basic_no_opt


@basic_no_opt
def ge(x: int) -> int:
    return hoho.foo(x) >= hoho.goo(x) >= gaga.foo(x)


@basic_no_opt
def gt(x: int) -> int:
    return hoho.foo(x) > hoho.goo(x) > gaga.foo(x)


@basic_no_opt
def and_(x: int) -> int:
    return hoho.foo(x) and hoho.goo(x) and gaga.foo(x)


@basic_no_opt
def or_(x: int) -> int:
    return hoho.foo(x) or hoho.goo(x) or gaga.foo(x)


@basic_no_opt
def and_or(x: int) -> int:
    return hoho.foo(x) and hoho.goo(x) or gaga.foo(x)


@basic_no_opt
def not_(x: int) -> int:
    return not (hoho.foo(x) == hoho.goo(x) == gaga.foo(x))


@basic_no_opt
def unary(x: int):
    return -hoho.foo(x), ~hoho.foo(x)


@basic_no_opt
def le(x: int) -> int:
    return hoho.foo(x) <= hoho.goo(x) <= gaga.foo(x)


@basic_no_opt
def lshift(x: int) -> int:
    return x << 1


@basic_no_opt
def rshift(x: int) -> int:
    return x >> 1


@basic_no_opt
def matmul(x, y):
    return x @ y


@basic_no_opt
def bitwise(x: int):
    return x & 1, x | 1, x ^ 1


@basic_no_opt
def invert(x: int) -> int:
    return ~x


@basic_no_opt
def lt(x: int) -> int:
    return hoho.foo(x) < hoho.goo(x) < gaga.foo(x)


@basic_no_opt
def ne(x: int) -> int:
    return hoho.foo(x) != hoho.goo(x) != gaga.foo(x)


@basic_no_opt
def multi_return(x: int):
    return hoho.foo(x), hoho.foo(x) + 1


@basic_no_opt
def add(x: int) -> int:
    return hoho.foo(x) + hoho.goo(x) + gaga.foo(x)


@basic_no_opt
def sub(x: int) -> int:
    return hoho.foo(x) - hoho.goo(x) - gaga.foo(x)


@pytest.mark.parametrize("x", [1, 2, 3])
def test_multi_return(x):
    assert multi_return(x) == (x + 1, x + 2)


@basic_no_opt
def mul(x: int) -> int:
    return hoho.foo(x) * hoho.goo(x) * gaga.foo(x)


@basic_no_opt
def div(x: int):
    return hoho.foo(x) / hoho.goo(x) / gaga.foo(x)


@basic_no_opt
def floordiv(x: int) -> int:
    return hoho.foo(x) // hoho.goo(x) // gaga.foo(x)


@basic_no_opt
def mod(x: int) -> int:
    return hoho.foo(x) % hoho.goo(x) % gaga.foo(x)


@basic_no_opt
def pow(x: int) -> int:
    return hoho.foo(x) ** hoho.goo(x) ** gaga.foo(x)


@basic_no_opt
def eq(x: int) -> int:
    return hoho.foo(x) == hoho.goo(x) == gaga.foo(x)


@pytest.mark.parametrize("x", [1, 2, 3])
def test_add(x):
    if x == 1:
        assert add(x) == (x + 1) + (x + 1) + (x + 1)
    else:
        pytest.raises(AssertionError, lambda: add(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_sub(x):
    if x == 1:
        assert sub(x) == (x + 1) - (x + 1) - (x + 1)
    else:
        pytest.raises(AssertionError, lambda: sub(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_mul(x):
    if x == 1:
        assert mul(x) == (x + 1) * (x + 1) * (x + 1)
    else:
        pytest.raises(AssertionError, lambda: mul(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_div(x):
    if x == 1:
        assert div(x) == (x + 1) / (x + 1) / (x + 1)
    else:
        pytest.raises(AssertionError, lambda: div(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_floordiv(x):
    if x == 1:
        assert floordiv(x) == (x + 1) // (x + 1) // (x + 1)
    else:
        pytest.raises(AssertionError, lambda: floordiv(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_mod(x):
    if x == 1:
        assert mod(x) == (x + 1) % (x + 1) % (x + 1)
    else:
        pytest.raises(AssertionError, lambda: mod(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_pow(x):
    if x == 1:
        assert pow(x) == (x + 1) ** (x + 1) ** (x + 1)
    else:
        pytest.raises(AssertionError, lambda: pow(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_eq(x):
    if x == 1:
        assert eq(x) == ((x + 1) == (x + 1) == (x + 1))
    else:
        pytest.raises(AssertionError, lambda: eq(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_ne(x):
    if x == 1:
        assert ne(x) == ((x + 1) != (x + 1) != (x + 1))
    else:
        pytest.raises(AssertionError, lambda: ne(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_lt(x):
    if x == 1:
        assert lt(x) == ((x + 1) < (x + 1) < (x + 1))
    else:
        pytest.raises(AssertionError, lambda: lt(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_le(x):
    if x == 1:
        assert le(x) == ((x + 1) <= (x + 1) <= (x + 1))
    else:
        pytest.raises(AssertionError, lambda: le(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_gt(x):
    if x == 1:
        assert gt(x) == ((x + 1) > (x + 1) > (x + 1))
    else:
        pytest.raises(AssertionError, lambda: gt(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_ge(x):
    if x == 1:
        assert ge(x) == ((x + 1) >= (x + 1) >= (x + 1))
    else:
        pytest.raises(AssertionError, lambda: ge(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_and(x):
    if x == 1:
        assert and_(x) == ((x + 1) and (x + 1) and (x + 1))
    else:
        pytest.raises(AssertionError, lambda: and_(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_or(x):
    if x == 1:
        assert or_(x) == ((x + 1) or (x + 1) or (x + 1))
    else:
        pytest.raises(AssertionError, lambda: or_(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_and_or(x):
    if x == 1:
        assert and_or(x) == ((x + 1) and (x + 1) or (x + 1))
    else:
        pytest.raises(AssertionError, lambda: and_or(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_not(x):
    if x == 1:
        assert not_(x) == (not ((x + 1) == (x + 1) == (x + 1)))
    else:
        pytest.raises(AssertionError, lambda: not_(x))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_unary(x):
    assert unary(x) == (-(x + 1), ~(x + 1))


@pytest.mark.parametrize("x", [1, 2, 3])
def test_lshift(x):
    assert lshift(x) == (x << 1)


@pytest.mark.parametrize("x", [1, 2, 3])
def test_rshift(x):
    assert rshift(x) == x >> 1


@pytest.mark.parametrize("x", [1, 2, 3])
def test_bitwise(x):
    assert bitwise(x) == (x & 1, x | 1, x ^ 1)


@pytest.mark.parametrize("x", [1, 2, 3])
def test_invert(x):
    assert invert(x) == ~x


@pytest.mark.parametrize(
    "x, y", [(np.zeros((2, 2)), np.ones((2, 2))), (np.ones((2, 2)), np.ones((2, 2)))]
)
def test_matmul(x, y):
    assert (matmul(x, y) == x @ y).all()
