from abc import ABC, abstractmethod
from typing import ClassVar
from dataclasses import dataclass

from kirin.ir import Method, DialectGroup
from kirin.rewrite.abc import RewriteResult


@dataclass
class Pass(ABC):
    """A pass is a transformation that is applied to a method. It wraps
    the analysis and rewrites needed to transform the method as an independent
    unit.

    Unlike LLVM/MLIR passes, a pass in Kirin does not apply to a module,
    this is because we focus on individual methods defined within
    python modules. This is a design choice to allow seamless integration
    within the Python interpreter.

    A Kirin compile unit is a `ir.Method` object, which is always equivalent
    to a LLVM/MLIR module if it were lowered to LLVM/MLIR just like other JIT
    compilers.
    """

    name: ClassVar[str]
    dialects: DialectGroup

    def __call__(self, mt: Method) -> RewriteResult:
        result = self.unsafe_run(mt)
        mt.code.verify()
        return result

    def fixpoint(self, mt: Method, max_iter: int = 32) -> RewriteResult:
        result = RewriteResult()
        for _ in range(max_iter):
            result_ = self.unsafe_run(mt)
            result = result_.join(result)
            if not result.has_done_something:
                break
        mt.code.verify()
        return result

    @abstractmethod
    def unsafe_run(self, mt: Method) -> RewriteResult: ...
