"""Interpreter module for Kirin.

This module contains the interpreter framework for Kirin. The interpreter
framework is used to implement concrete and abstract interpreters for the
IR. The interpreter framework provides a set of classes and interfaces to
implement interpreters for the IR.

The interpreter framework is designed to be extensible and customizable. It
provides a set of base classes and interfaces for implementing concrete and
abstract interpreters:

- [`BaseInterpreter`][kirin.interp.BaseInterpreter]: Base class for implementing concrete interpreters.
- [`AbstractInterpreter`][kirin.interp.AbstractInterpreter]: Base class for implementing abstract interpreters.
- [`Frame`][kirin.interp.Frame]: Base class for interpreter frame.
- [`MethodTable`][kirin.interp.MethodTable]: Method table for registering implementations of statements.
"""

from . import result as result
from .base import BaseInterpreter as BaseInterpreter
from .impl import ImplDef as ImplDef, Signature as Signature, impl as impl
from .frame import Frame as Frame, FrameABC as FrameABC
from .table import MethodTable as MethodTable
from .value import (
    Successor as Successor,
    YieldValue as YieldValue,
    ReturnValue as ReturnValue,
    SpecialValue as SpecialValue,
    StatementResult as StatementResult,
)
from .abstract import (
    AbstractFrame as AbstractFrame,
    AbstractInterpreter as AbstractInterpreter,
)
from .concrete import Interpreter as Interpreter
from .exceptions import (
    WrapException as WrapException,
    InterpreterError as InterpreterError,
    FuelExhaustedError as FuelExhaustedError,
)
