from typing import TYPE_CHECKING

from kirin import ir

if TYPE_CHECKING:
    from kirin.ir import Statement
    from kirin.print.printer import Printer


def pprint_calllike(
    invoke_or_call: "Statement", callee: str, printer: "Printer"
) -> None:
    with printer.rich(style="red"):
        printer.print_name(invoke_or_call)
    printer.plain_print(" ")

    n_total = len(invoke_or_call.args)
    printer.plain_print(callee)
    if (inputs := getattr(invoke_or_call, "inputs", None)) is None:
        raise ValueError(f"{invoke_or_call} does not have inputs")

    if not isinstance(inputs, tuple):
        raise ValueError(f"inputs of {invoke_or_call} is not a tuple")

    if (kwargs := getattr(invoke_or_call, "kwargs", None)) is None:
        raise ValueError(f"{invoke_or_call} does not have kwargs")

    if not isinstance(kwargs, tuple):
        raise ValueError(f"kwargs of {invoke_or_call} is not a tuple")

    positional = inputs[: n_total - len(kwargs)]
    kwargs = dict(
        zip(
            kwargs,
            inputs[n_total - len(kwargs) :],
        )
    )

    printer.plain_print("(")
    printer.print_seq(positional)
    if kwargs and positional:
        printer.plain_print(", ")
    printer.print_mapping(kwargs, delim=", ")
    printer.plain_print(")")

    with printer.rich(style="comment"):
        printer.plain_print(" : ")
        printer.print_seq(
            [result.type for result in invoke_or_call._results],
            delim=", ",
        )
        if trait := invoke_or_call.get_trait(ir.MaybePure):
            printer.plain_print(f" maybe_pure={trait.is_pure(invoke_or_call)}")
