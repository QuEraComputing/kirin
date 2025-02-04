!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Dialects that brings in common Python data types

This page provides a reference for dialects that bring in semantics for common Python data types.

!!! note
    While it is worth noting that using Python semantics can be very convenient, it is also important to remember that the Python semantics are not designed for compilation. Therefore, it is important to be aware of the limitations of using Python semantics in a compilation context especially when it comes to data types.
    An example of this is the `list` data type in Python which is a dynamic mutable array. When the low-level code is not expecting a dynamic mutable array, it can lead to extra complexity for compilation. An immutable array or a fixed-size array can be a better choice in such cases (see `ilist` dialect).

## References

### Tuple

::: kirin.dialects.py.tuple
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

### List

::: kirin.dialects.py.list
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true
