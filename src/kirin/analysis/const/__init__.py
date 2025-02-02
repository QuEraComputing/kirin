"""Const analysis module.

This module contains the constant analysis framework for kirin. The constant
analysis framework is built on top of the interpreter framework.

This module provides a lattice for constant propagation analysis and a
propagation algorithm for computing the constant values for each SSA value in
the IR.
"""

from .prop import Propagate as Propagate, ExtraFrameInfo as ExtraFrameInfo
from .lattice import (
    Pure as Pure,
    Value as Value,
    Bottom as Bottom,
    Result as Result,
    NotPure as NotPure,
    Unknown as Unknown,
    JointResult as JointResult,
    PartialConst as PartialConst,
    PartialTuple as PartialTuple,
    PurityBottom as PurityBottom,
    PartialLambda as PartialLambda,
)
