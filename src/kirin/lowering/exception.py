import sys
import types


class BuildError(Exception):
    """Base class for all dialect lowering errors."""

    def __init__(self, hint: str, *msgs: object):
        super().__init__(hint, *msgs)

    @property
    def hint(self) -> str:
        """Return the hint message."""
        return self.args[0]


def exception_handler(exc_type, exc_value, exc_tb: types.TracebackType):
    """Custom exception handler to format and print exceptions."""
    if issubclass(exc_type, BuildError):
        print(exc_value, file=sys.stderr)
        return

    # Call the default exception handler
    sys.__excepthook__(exc_type, exc_value, exc_tb)


# Set the custom exception handler
sys.excepthook = exception_handler


def custom_exc(shell, etype, evalue, tb, tb_offset=None):
    if issubclass(etype, BuildError):
        # Handle BuildError exceptions
        print(evalue, file=sys.stderr)
        return
    shell.showtraceback((etype, evalue, tb), tb_offset=tb_offset)


try:
    ip = get_ipython()  # type: ignore
    # Register your custom exception handler
    ip.set_custom_exc((Exception,), custom_exc)
except NameError:
    # Not in IPython, so we won't set the custom exception handler
    pass
