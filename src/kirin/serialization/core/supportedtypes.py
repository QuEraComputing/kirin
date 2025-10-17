from typing import Union
from collections.abc import Sequence

SUPPORTED_PYTHON_TYPES = Union[
    # Python built-in types
    bool,
    bytes,
    bytearray,
    dict,
    float,
    frozenset,
    int,
    list,
    range,
    set,
    slice,
    str,
    tuple,
    type,
    type(None),
    Sequence,
]
