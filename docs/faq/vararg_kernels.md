# Kernels with a Variable Number of Arguments

## The problem

You want a kernel that accepts different numbers of arguments, and you reach for
Python's `*args`:

```python
from kirin.prelude import basic


@basic
def test_kernel(*args):
    return args
```

Lowering this fails with a `BuildError` such as `args is not defined`.

## Why this happens

A kernel has a **fixed signature**. `*args` and `**kwargs` are not supported, so
they are never bound and referencing them fails. There is also no function
overloading — two kernels sharing a name are just ordinary Python, so the second
`def` rebinds the name and the first is gone.

## The solution(s)

### Use separate, named kernels

When the different signatures are really different operations, give them
distinct names. This is the intended way to express different signatures:

```python
from kirin.prelude import basic


@basic
def test_kernel_1(input: float):
    ...


@basic
def test_kernel_2(input2: float, input3: str):
    ...
```

### Pass a collection for a genuinely variable count

When you truly need a variable number of values of the same kind, pass a single
collection argument — an immutable list (`IList`) or a tuple — and iterate or
index inside the kernel:

```python
from typing import Any

from kirin.prelude import basic
from kirin.dialects import ilist


@basic
def test_kernel(inputs: ilist.IList[float, Any]):
    total = 0.0
    for x in inputs:
        total = total + x
    return total
```

This is the idiomatic Kirin replacement for `*args`.

