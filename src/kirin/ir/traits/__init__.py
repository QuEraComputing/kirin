"""Kirin IR Traits.

This module defines the traits that can be used to define the behavior of
Kirin IR nodes. The base trait is `StmtTrait`, which is a `dataclass` that
implements the `__hash__` and `__eq__` methods.

There are also some basic traits that are provided for convenience, such as
`Pure`, `HasParent`, `ConstantLike`, `IsTerminator`, `NoTerminator`, and
`IsolatedFromAbove`.
"""

from .abc import (
    Trait as Trait,
    AttrTrait as AttrTrait,
    StmtTrait as StmtTrait,
    RegionGraph as RegionGraph,
    RegionInterpretationTrait as RegionInterpretationTrait,
)
from .basic import (
    Pure as Pure,
    HasParent as HasParent,
    MaybePure as MaybePure,
    ConstantLike as ConstantLike,
    IsTerminator as IsTerminator,
    NoTerminator as NoTerminator,
    IsolatedFromAbove as IsolatedFromAbove,
)
from .symbol import (
    SymbolTable as SymbolTable,
    SymbolOpInterface as SymbolOpInterface,
    EntryPointInterface as EntryPointInterface,
)
from .callable import (
    HasSignature as HasSignature,
    CallableStmtInterface as CallableStmtInterface,
)
from .region.ssacfg import SSACFG as SSACFG, HasCFG as HasCFG
