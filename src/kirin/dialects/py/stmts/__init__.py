from kirin.dialects.py.stmts.dialect import dialect as dialect

from . import (
    _stmts as _stmts,
    interp as interp,
    lowering as lowering,
    typeinfer as typeinfer,
)
from ._stmts import *  # noqa: F403
