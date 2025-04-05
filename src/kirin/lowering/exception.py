from kirin.exception import NoPythonStackTrace


class BuildError(NoPythonStackTrace):
    """Base class for all dialect lowering errors."""

    def __init__(self, *msgs: object, help: str | None = None):
        super().__init__(*msgs)
        self.help = help
