# FAQ

This page collects common questions with a short answer and a link to a deeper explanation.

## Kernel authoring

| Question | Short answer |
| --- | --- |
| [Why do I get an `AttributeError` when calling a class method inside a kernel?](class_methods.md) | Resolve the Python method on the host side before defining the kernel, then capture it as a constant. |
| [Can a kernel take a variable number of arguments with `*args`?](vararg_kernels.md) | No — kernels have a fixed signature. Use separate named kernels, or pass a single `IList`/tuple for a variable count. |
