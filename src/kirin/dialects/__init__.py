"""Built-in dialects for Kirin.

This module contains the built-in dialects for Kirin. Each dialect is an
instance of the `Dialect` class. Each submodule contains a `dialect` variable
that is an instance of the corresponding `Dialect` class.

The modules can be directly used as dialects. For example, you can write

```python
from kirin.dialects import py, func
```

to import the Python and function dialects.
"""
