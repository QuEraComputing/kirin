"""A tiny ``@kernel`` front-end that lowers a subset of Python to Kirin 2.0 IR.

Example::

    from kirin_rs import kernel

    @kernel
    def add(a: int, b: int) -> int:
        return a + b

    print(add.kirin_ir)   # the lowered .kirin text

This is the Python half of the ``rust``-branch lowering experiment: it parses the
decorated function's source with the stdlib :mod:`ast` module and hands the AST to
the Rust extension (``_kirin_py.lower_source``), which walks it and drives the
Kirin IR builder.
"""

from __future__ import annotations

import ast
import inspect
import textwrap

from . import _kirin_py

__all__ = ["kernel", "lower_source"]


def lower_source(source: str) -> str:
    """Lower a string of Python source containing ``def``\\ s to ``.kirin`` IR.

    Lowering the whole module at once is how cross-kernel calls resolve: every
    ``def`` in ``source`` is declared before any body is lowered, so a call to a
    sibling kernel finds its target.
    """
    module = ast.parse(textwrap.dedent(source))
    return _kirin_py.lower_source(module)


def kernel(fn):
    """Decorator that lowers ``fn`` to Kirin IR, attaching ``fn.kirin_ir``.

    Only the decorated function's *own* source is lowered (via
    ``inspect.getsource``), so calls to other top-level kernels are not resolved
    through this decorator — the callee is not part of the single-function
    module. Use :func:`lower_source` on a multi-``def`` source string to lower a
    group of mutually-calling kernels together.
    """
    source = textwrap.dedent(inspect.getsource(fn))
    module = ast.parse(source)
    # Strip decorators so the re-parsed source lowers cleanly.
    for node in module.body:
        if isinstance(node, ast.FunctionDef):
            node.decorator_list = []
    fn.kirin_ir = _kirin_py.lower_source(module)
    return fn
