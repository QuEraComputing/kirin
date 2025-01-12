from dataclasses import dataclass

from ..abc import StmtTrait


@dataclass(frozen=True)
class FromPythonCall(StmtTrait):
    pass
