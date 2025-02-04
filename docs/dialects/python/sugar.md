!!! warning
    This page is under construction. The content may be incomplete or incorrect. Submit an issue
    on [GitHub](https://github.com/QuEraComputing/kirin/issues/new) if you need help or want to
    contribute.

# Dialects for Python Syntax Sugar

This page contains the dialects designed to represent Python syntax sugar. They provide an implementation
of lowering transform from the corresponding Python AST to the dialects' statements. All the statements are
typed `Any` thus one can always use a custom rewrite pass after type inference to support the desired syntax sugar.

## Reference

### Indexing

::: kirin.dialects.py.indexing
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true

### Attribute

::: kirin.dialects.py.attr
    options:
        filters:
        - "!statement"
        show_root_heading: true
        show_if_no_docstring: true
