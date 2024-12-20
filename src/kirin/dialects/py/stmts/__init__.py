from kirin.dialects.py.stmts.dialect import dialect as dialect

from . import (
    emit as emit,
    _stmts as _stmts,
    interp as interp,
    lowering as lowering,
    constprop as constprop,
    typeinfer as typeinfer,
)
from ._stmts import *  # noqa: F403
