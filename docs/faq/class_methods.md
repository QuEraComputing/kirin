# Calling Python Class Methods Inside a Kernel

## The problem

If you try to call a standard Python class method, such as
`self.get_move_kernel()`, from inside a function decorated with
`@basic` or `@structural`, the compiler will fail. Historically, before Kirin
v0.16.8, this could appear as an `AttributeError` about `arg_names`.

## Why this happens

A kernel is a piece of code that describes instructions intended to run on the
target device, not on your host machine. Because of this, the Kirin compiler
cannot evaluate arbitrary Python runtime expressions, such as object attribute
lookups or dynamically calling class methods, during device execution.

The compiler only parses global constants and already-resolved kernel functions.

## The solution

Resolve the Python method on the host before defining the kernel. By assigning
the result to a local variable, the inner kernel can cleanly capture that
variable as a constant.

### Incorrect

This tries to evaluate the method inside the device kernel.

```python
from kirin.prelude import basic


class Offset:
    def __init__(self, value: int):
        self.value = value

    def get_value(self) -> int:
        return self.value

    def bad_method(self):
        @basic
        def add_offset(x: int) -> int:
            # ERROR: Kirin cannot evaluate arbitrary Python class methods.
            return x + self.get_value()

        return add_offset
```

### Correct

Resolve the method on the host side, then capture it in the kernel closure.

```python
from kirin.prelude import basic


class Offset:
    def __init__(self, value: int):
        self.value = value

    def get_value(self) -> int:
        return self.value

    def method(self):
        # 1. Resolve the method in host Python code.
        offset = self.get_value()

        # 2. Define the device kernel.
        @basic
        def add_offset(x: int) -> int:
            # 3. Use the captured constant inside the kernel.
            return x + offset

        return add_offset
```
