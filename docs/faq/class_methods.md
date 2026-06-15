# Calling Python Class Methods Inside a Kernel

## The problem

If you try to call a standard Python class method, such as
`self.get_move_kernel()`, from inside a function decorated with
`@kirin_flair.kernel` or `@kernel`, the compiler will fail. Historically, before
Kirin v0.16.8, this could appear as an `AttributeError` about `arg_names`.

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
class Foo:
    def get_move_buffer_kernel(self):

        @kernel
        def get_move_buffer():
            # ERROR: Kirin cannot evaluate arbitrary Python class methods.
            move_kernel = self.get_move_kernel()
            move_sequence = move_kernel()
            # ...
            return bufr

        return get_move_buffer
```

### Correct

Resolve the method on the host side, then capture it in the kernel closure.

```python
class Foo:
    def get_move_buffer_kernel(self):
        # 1. Resolve the method in host Python code.
        move_kernel = self.get_move_kernel()

        # 2. Define the device kernel.
        @kernel
        def get_move_buffer(a, b, c):
            # 3. Call the captured constant inside the kernel.
            move_kernel(a, b, c)
            # ...
            return bufr

        return get_move_buffer
```
