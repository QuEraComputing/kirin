# `kirin-py-frontend` — the Python front-end for Kirin 2.0 (`dl/rust-python-lowering`)

A vertical slice of a **Python front-end** for Kirin 2.0: a real CPython
`@kernel`-style decorator that lowers a subset of Python into Kirin IR via the
existing IR builder, prints `.kirin`, and (via the interpreter) **runs** it.

> "Front-end lowering" = **Python AST → Kirin IR**. This is distinct from Kirin's
> internal *stage* lowering (`@source` → `@lowered`).

```
Python source → CPython ast → (PyO3 bridge) → PyAst mirror → lower_* visitor
              → BuilderStageInfo calls → finalize() → .kirin (sprint) / run (interpret)
```

## Layout (one crate)

This single crate holds both the lowering core and the PyO3 bridge:

| Part | Role |
|------|------|
| `src/{ast,language,ty,scope,error}.rs`, `src/lower/`, `src/interpreter.rs` | **Pure-Rust lowering core**: the `PyAst` mirror, the `PyLang` dialect composition, the `lower_*` visitor (drives `BuilderStageInfo`), and the `Interpretable` wiring (`run_i64`). |
| [`src/convert.rs`](src/convert.rs), [`src/lib.rs`](src/lib.rs) | **PyO3 bridge**: CPython `ast` → `PyAst`, plus the `_kirin_py` module exposing `lower_source`. |
| [`python/kirin_rs/`](python/kirin_rs) | The pure-Python `@kernel` decorator. Packaged via the co-located [`pyproject.toml`](pyproject.toml) (maturin). |

`extension-module` is off by default so `cargo build/test` link libpython
normally; maturin enables it for the wheel build. (Because the crate depends on
PyO3, building/testing it requires a `python3` — `kirin-py` was already a
workspace member with that requirement.)

## Supported Python subset

Top-level `def` with int/float params + return; local assignment; arithmetic
`+ - * /`; comparisons `== != < <= > >=`; `if/else`; `for i in range(...)` with
loop-carried accumulators; and calls between kernels.

**Not supported (intentional v1 limits):** `return` inside an `if`/`for` body;
unary ops; chained comparisons; non-`range` iterables; closures/nested `def`.
Types: `int`/`bool` → `i64`, `float` → `f64`.

## Build & run

```bash
# All tests (lowering, parse-back roundtrip, execution, PyO3 parity, dialect fixtures):
cargo test -p kirin-py-frontend           # needs a python3 on PATH (PyO3 build)

# The per-dialect fixtures live in crates/kirin-py-frontend/tests/kernels/{arith,cmp,scf,func}.py:
# each is a module of richer @kernel-style functions; tests/dialects.rs lowers each
# through CPython ast → PyO3 → the lowering core and asserts the dialect ops + roundtrip.
# tests/execution.rs additionally *runs* lowered kernels and checks results.

# End-to-end with the @kernel decorator (installs into the active venv).
# maturin runs from this crate dir (the pyproject.toml lives here); the runnable
# demo lives with the other examples under the repo's top-level example/ dir.
(cd crates/kirin-py-frontend && maturin develop)   # build the extension + install kirin_rs
python example/python-lowering/demo.py             # prints .kirin IR for @kernel functions
```

> If `cargo`/PyO3 picks the wrong interpreter, set `PYO3_PYTHON=/path/to/python3`.
> `maturin` itself can be installed with `cargo install maturin` (the workspace's
> uv index is private; PyPI may be unreachable).

## Example

```python
from kirin_rs import kernel

@kernel
def sum_to(n: int) -> int:
    s = 0
    for i in range(0, n):
        s = s + i
    return s

print(sum_to.kirin_ir)
```

```
stage @source fn @sum_to(i64) -> i64;
specialize @source fn @sum_to (i64) -> i64 {
  ^entry(%n: i64) {
    %1 = constant 0 -> i64;
    %2 = constant 0 -> i64;
    %3 = constant 1 -> i64;
    %7 = for %2 in %2 .. %n step %3 iter_args (%1) do ^body(%i: i64, %s: i64) {
      %6 = add %s, %i -> i64;
      yield %6;
    } -> i64;
    ret %7;
  }
}
```

Every program is checked three ways: the expected dialect ops appear, the printed
`.kirin` **round-trips** (parse → print is stable; `tests/roundtrip.rs`), and it
**executes to the right value** (`tests/execution.rs`, e.g. `factorial(5) == 120`).

## Note on a shared-crate change

`kirin-scf`'s `Yield` gained `#[kirin(builders)]` (like `Return` already had) so
the lowering core can construct `yield` terminators programmatically. This is
purely additive; all 35 `kirin-scf` tests still pass.
