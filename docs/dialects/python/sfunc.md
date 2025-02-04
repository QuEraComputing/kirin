!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Dialects for special Python functions

There are some special built-in Python functions that does not necessarily provide new data types when
using them as Kirin dialects.

For example, the `py.range` dialect may not use a Python `range` type,
the actual type can be decided by another dialect that implements the type inference method for [`Range`][kirin.dialects.py.Range] statement, e.g `ilist` dialect provides an `IList` implementation for `Range` statement.

The reason for this is that in many cases, eDSLs are not interested in the actual data type of the result, but rather the semantics of the operation. For `ilist` dialect, the `Range` statement is just a syntax sugar for creating a list of integers. The compiler will decide what the actual implementation (such as the memory layout) of the list should be.

# References

## Iterable

::: kirin.dialects.py.iterable
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

## Len

::: kirin.dialects.py.len
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

## Range

::: kirin.dialects.py.range
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

## Slice

::: kirin.dialects.py.slice
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

## Built-in Function

::: kirin.dialects.py.builtin
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true
