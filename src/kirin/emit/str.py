from typing import IO, Generic, TypeVar, Iterable

from kirin import ir
from kirin.emit.abc import EmitABC
from kirin.interp.frame import Frame

IO_t = TypeVar("IO_t", bound=IO)


class EmitStr(EmitABC[Frame, str | None], Generic[IO_t]):

    def __init__(
        self,
        file: IO_t,
        dialects: ir.DialectGroup | Iterable[ir.Dialect],
        *,
        fuel: int | None = None,
        max_depth: int = 128,
        max_python_recursion_depth: int = 8192,
    ):
        super().__init__(
            dialects,
            fuel=fuel,
            max_depth=max_depth,
            max_python_recursion_depth=max_python_recursion_depth,
        )
        self.file = file
