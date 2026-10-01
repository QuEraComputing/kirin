# re-exports the public API of the kirin package
from . import ir as ir, types as types, stdlib as stdlib, lowering as lowering
from .exception import enable_stracetrace, disable_stracetrace
from .visualize import to_html as ir_to_html, write_html as write_ir_html

__all__ = [
    "ir",
    "types",
    "lowering",
    "enable_stracetrace",
    "disable_stracetrace",
    "ir_to_html",
    "write_ir_html",
]
