"""End-to-end demo: decorate Python kernels with ``@kernel`` and print the
lowered Kirin 2.0 ``.kirin`` IR produced by the Rust front-end.

Build the extension into your venv first, then run from the repo root::

    (cd crates/kirin-py && maturin develop)
    python example/python-lowering/demo.py
"""

from kirin_rs import kernel, lower_source


@kernel
def add(a: int, b: int) -> int:
    return a + b


@kernel
def pick(c: int, a: int, b: int) -> int:
    if c > 0:
        r = a + b
    else:
        r = a - b
    return r


@kernel
def sum_to(n: int) -> int:
    s = 0
    for i in range(0, n):
        s = s + i
    return s


# Multiple kernels that call each other must be lowered from one source string
# so the callee can resolve (the single-function decorator can't see siblings).
MULTI = """
def helper(x: int) -> int:
    return x + x

def main(y: int) -> int:
    z = helper(y)
    return z
"""


if __name__ == "__main__":
    for fn in (add, pick, sum_to):
        print(f"# --- {fn.__name__} ---")
        print(fn.kirin_ir)

    print("# --- helper + main (cross-kernel call) ---")
    print(lower_source(MULTI))
