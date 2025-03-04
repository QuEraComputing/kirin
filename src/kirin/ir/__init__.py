"""IR module for kirin.

This module contains the data structure definition
of the intermediate representation (IR) for kirin.
"""

from kirin.ir import attrs as attrs, traits as traits
from kirin.ir.ssa import (
    SSAValue as SSAValue,
    TestValue as TestValue,
    ResultValue as ResultValue,
    BlockArgument as BlockArgument,
    DeletedSSAValue as DeletedSSAValue,
)
from kirin.ir.use import Use as Use
from kirin.ir.group import DialectGroup as DialectGroup, dialect_group as dialect_group
from kirin.ir.nodes import (
    Block as Block,
    IRNode as IRNode,
    Region as Region,
    Statement as Statement,
)
from kirin.ir.method import Method as Method
from kirin.ir.traits import (
    Pure as Pure,
    HasParent as HasParent,
    MaybePure as MaybePure,
    StmtTrait as StmtTrait,
    RegionTrait as RegionTrait,
    SymbolTable as SymbolTable,
    ConstantLike as ConstantLike,
    HasSignature as HasSignature,
    IsTerminator as IsTerminator,
    NoTerminator as NoTerminator,
    SSACFGRegion as SSACFGRegion,
    FromPythonCall as FromPythonCall,
    FromPythonWith as FromPythonWith,
    IsolatedFromAbove as IsolatedFromAbove,
    SymbolOpInterface as SymbolOpInterface,
    FromPythonRangeLike as FromPythonRangeLike,
    PythonLoweringTrait as PythonLoweringTrait,
    CallableStmtInterface as CallableStmtInterface,
    FromPythonWithSingleItem as FromPythonWithSingleItem,
)
from kirin.ir.dialect import Dialect as Dialect
from kirin.ir.attrs.py import PyAttr as PyAttr
from kirin.ir.attrs.abc import Attribute as Attribute, AttributeMeta as AttributeMeta
from kirin.ir.attrs.data import Data as Data
