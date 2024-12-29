import sys
import inspect
from typing import Any, TypedDict

from typing_extensions import Unpack, Optional

from kirin.ir import Dialect
from kirin.decl.info import StatementFields


class StatementOptions(TypedDict, total=False):
    init: bool
    repr: bool
    kw_only: bool
    dialect: Optional[Dialect]
    property: bool


class BaseModifier:
    _PARAMS = "__kirin_stmt_params"

    def __init__(self, cls: type, **kwargs: Unpack[StatementOptions]) -> None:
        self.cls = cls
        self.cls_module = sys.modules.get(cls.__module__)

        if "dialect" in kwargs:
            self.dialect = kwargs["dialect"]
        else:
            self.dialect = None
        self.params = kwargs
        setattr(cls, self._PARAMS, self.params)

        if cls.__module__ in sys.modules:
            self.globals = sys.modules[cls.__module__].__dict__
        else:
            # Theoretically this can happen if someone writes
            # a custom string to cls.__module__.  In which case
            # such dataclass won't be fully introspectable
            # (w.r.t. typing.get_type_hints) but will still function
            # correctly.
            self.globals: dict[str, Any] = {}

        # analysis state, used by scan_field, etc.
        self.fields = StatementFields()
        self.has_statement_bases = False
        self.kw_only = self.params.get("kw_only", False)
        self.KW_ONLY_seen = False

    def register(self) -> None:
        if self.dialect is None:
            return
        self.dialect.register(self.cls)

    def emit(self):
        self._self_name = "__kirin_stmt_self" if "self" in self.fields else "self"
        self._class_name = "__kirin_stmt_cls" if "cls" in self.fields else "cls"
        self._run_passes("emit_")

    def verify(self):
        self._run_passes("verify_")

    def _run_passes(self, prefix: str):
        for name, method in inspect.getmembers(self, inspect.ismethod):
            if name.startswith(prefix):
                method()
