from dataclasses import dataclass

from kirin.ir import AnyType, SSAValue, TypeAttribute


@dataclass
class TestValue(SSAValue):

    def __init__(self, type: TypeAttribute | None = None) -> None:
        super().__init__()
        self.type = type or AnyType()

    @property
    def owner(self):
        raise NotImplementedError

    def __hash__(self) -> int:
        return id(self)
