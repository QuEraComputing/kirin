"""Control flow dialect.

This dialect provides a low-level control flow representation.

This dialect does not provide any lowering strategies, to lowering
a Python AST to this dialect, use the `kirin.dialects.lowering.cf` dialect
with this dialect.
"""

from kirin.dialects.cf import (
    emit as emit,
    constprop as constprop,
    typeinfer as typeinfer,
)
from kirin.dialects.cf.stmts import (
    Branch as Branch,
    ConditionalBranch as ConditionalBranch,
)
from kirin.dialects.cf.interp import CfInterpreter as CfInterpreter
from kirin.dialects.cf.dialect import dialect as dialect
