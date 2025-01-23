from typing import TYPE_CHECKING, Generic, TypeVar, Callable, ParamSpec
from dataclasses import dataclass

if TYPE_CHECKING:
    from kirin.ir.nodes.stmt import Statement

Params = ParamSpec("Params")
RetType = TypeVar("RetType")


@dataclass(frozen=True)
class Binding(Generic[Params, RetType]):
    parent: type["Statement"]

    def __call__(self, *args: Params.args, **kwargs: Params.kwargs) -> RetType:
        raise NotImplementedError(
            f"Binding of {self.parent.name} can \
            only be called from a kernel"
        )


def wraps(parent: type["Statement"]):
    """Wraps a [`Statement`][kirin.ir.nodes.stmt.Statement] to a `Binding` object
    which will be special cased in the lowering process.

    This is useful for providing type hints by faking the call signature of a
    [`Statement`][kirin.ir.nodes.stmt.Statement].

    ## Example

    Directly writing a function with the statement will let Python linter think
    you intend to call the constructor of the statement class. However, given the
    context of a kernel, our intention is to actually "call" the statement, e.g
    the following will produce type errors with pyright or mypy:

    ```python
    from kirin.dialects import math
    from kirin.prelude import basic_no_opt

    @basic_no_opt
    def main(x: float):
        return math.sin(x) # this is a statement, not a function
    ```

    the `@lowering.wraps` decorator allows us to provide a type hint for the
    statement, e.g:

    ```python
    from kirin import lowering

    @lowering.wraps(math.sin)
    def sin(value: float) -> float: ...

    @basic_no_opt
    def main(x: float):
        return sin(x) # linter now thinks this is a function

    sin(1.0) # this will raise a NotImplementedError("Binding of sin can only be called from a kernel")
    ```
    """

    def wrapper(func: Callable[Params, RetType]) -> Binding[Params, RetType]:
        return Binding(parent)

    return wrapper
