from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import ClassVar

from kirin.ir import DialectGroup, Method


@dataclass
class Pass(ABC):
    name: ClassVar[str]
    dialects: DialectGroup

    def __call__(self, mt: Method) -> None:
        self.unsafe_run(mt)
        mt.code.verify()

    @abstractmethod
    def unsafe_run(self, mt: Method) -> None: ...
