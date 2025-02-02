"""Analysis module for kirin.

This module contains the analysis framework for kirin. The analysis framework is
built on top of the interpreter framework. This module provides a set of base classes
and frameworks for implementing compiler analysis passes on the IR.

The analysis framework contains the following modules:

- [`cfg`][kirin.analysis.cfg]: Control flow graph for a given IR.
- [`forward`][kirin.analysis.forward]: Forward dataflow analysis.
- [`callgraph`][kirin.analysis.callgraph]: Call graph for a given IR.
- [`typeinfer`][kirin.analysis.typeinfer]: Type inference analysis.
- [`const`][kirin.analysis.const]: Constants used in the analysis framework.
"""

from kirin.analysis import const as const
from kirin.analysis.cfg import CFG as CFG
from kirin.analysis.forward import (
    Forward as Forward,
    ForwardExtra as ForwardExtra,
    ForwardFrame as ForwardFrame,
)
from kirin.analysis.callgraph import CallGraph as CallGraph
from kirin.analysis.typeinfer import TypeInference as TypeInference
