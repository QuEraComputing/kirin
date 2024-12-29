from __future__ import annotations

from typing import Iterable, Sequence, overload
from dataclasses import field, dataclass

from kirin.ir import SSAValue, Statement
from kirin.exceptions import DialectLoweringError


@dataclass(init=False)
class Result(Sequence[SSAValue]):
    values: Sequence[SSAValue] = field(default_factory=list)

    @overload
    def __init__(self, value: None = None) -> None: ...

    @overload
    def __init__(self, value: SSAValue, *values: SSAValue) -> None: ...

    @overload
    def __init__(self, value: Iterable[SSAValue]) -> None: ...

    @overload
    def __init__(self, value: Statement) -> None: ...

    def __init__(
        self,
        value: SSAValue | Iterable[SSAValue] | Statement | None = None,
        *values: SSAValue,
    ) -> None:
        if value is None:
            assert not values, "unexpected values"
            self.values = []
        elif isinstance(value, SSAValue):
            self.values = [value, *values]
        elif isinstance(value, Statement):
            assert not values, "unexpected values"
            self.values = value._results
        else:
            assert not values, "unexpected values"
            self.values = list(value)

    def expect_one(self) -> SSAValue:
        if len(self.values) != 1:
            raise DialectLoweringError("expected one result")
        return self.values[0]

    # forward the sequence methods
    def __len__(self):
        return len(self.values)

    def __getitem__(self, key):
        return self.values[key]

    def __iter__(self):
        return iter(self.values)

    def __contains__(self, value):
        return value in self.values

    def __reversed__(self):
        return reversed(self.values)

    def __eq__(self, other):
        return self.values == other.values

    def __ne__(self, other):
        return self.values != other.values
