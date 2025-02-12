"""Const analysis module.

This module contains the constant analysis framework for kirin. The constant
analysis framework is built on top of the interpreter framework.

This module provides a lattice for constant propagation analysis and a
propagation algorithm for computing the constant values for each SSA value in
the IR.
"""

from .prop import Frame as Frame, Propagate as Propagate
from .lattice import (
    Value as Value,
    Bottom as Bottom,
    Result as Result,
    Unknown as Unknown,
    PartialConst as PartialConst,
    PartialTuple as PartialTuple,
    PartialLambda as PartialLambda,
)
