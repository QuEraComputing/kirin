"""This module contains custom exception handling
for the Kirin-based compilers.
"""

import sys
import types

stacktrace = False


class NoPythonStackTrace(Exception):
    pass


def enable_stracetrace():
    """Enable the stacktrace for all exceptions."""
    global stacktrace
    stacktrace = True


def disable_stracetrace():
    """Disable the stacktrace for all exceptions."""
    global stacktrace
    stacktrace = False


def exception_handler(exc_type, exc_value, exc_tb: types.TracebackType):
    """Custom exception handler to format and print exceptions."""
    if not stacktrace and issubclass(exc_type, NoPythonStackTrace):
        print(exc_value, file=sys.stderr)
        return

    # Call the default exception handler
    sys.__excepthook__(exc_type, exc_value, exc_tb)


# Set the custom exception handler
sys.excepthook = exception_handler


def custom_exc(shell, etype, evalue, tb, tb_offset=None):
    if issubclass(etype, NoPythonStackTrace):
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
