import ast
import inspect
import builtins
from typing import TYPE_CHECKING, Any, TypeVar
from dataclasses import dataclass

from kirin.ir import Method, SSAValue, Statement, DialectGroup, traits
from kirin.source import SourceInfo
from kirin.exceptions import DialectLoweringError
from kirin.lowering.frame import Frame
from kirin.lowering.result import Result
from kirin.lowering.dialect import FromPythonAST

if TYPE_CHECKING:
    from kirin.lowering.core import Lowering


@dataclass
class LoweringState(ast.NodeVisitor):
    # from parent
    dialects: DialectGroup
    registry: dict[str, FromPythonAST]

    # debug info
    lines: list[str]
    lineno_offset: int
    "lineno offset at the beginning of the source"
    col_offset: int
    "column offset at the beginning of the source"
    source: SourceInfo
    "source info of the current node"
    # line_range: tuple[int, int]  # current (<start>, <end>)
    # col_range: tuple[int, int]  # current (<start>, <end>)
    max_lines: int = 3
    _current_frame: Frame | None = None

    @classmethod
    def from_stmt(
        cls,
        lowering: "Lowering",
        stmt: ast.stmt,
        source: str | None = None,
        globals: dict[str, Any] | None = None,
        max_lines: int = 3,
        lineno_offset: int = 0,
        col_offset: int = 0,
    ):
        if not isinstance(stmt, ast.stmt):
            raise ValueError(f"Expected ast.stmt, got {type(stmt)}")

        if not source:
            source = ast.unparse(stmt)

        state = cls(
            dialects=lowering.dialects,
            registry=lowering.registry,
            lines=source.splitlines(),
            lineno_offset=lineno_offset,
            col_offset=col_offset,
            source=SourceInfo.from_ast(stmt, lineno_offset, col_offset),
            max_lines=max_lines,
        )

        frame = Frame.from_stmts([stmt], state, globals=globals)
        state.push_frame(frame)
        return state

    @property
    def current_frame(self):
        if self._current_frame is None:
            raise ValueError("No frame")
        return self._current_frame

    @property
    def code(self):
        stmt = self.current_frame.current_region.blocks[0].first_stmt
        if stmt:
            return stmt
        raise ValueError("No code generated")

    StmtType = TypeVar("StmtType", bound=Statement)

    def append_stmt(self, stmt: StmtType) -> StmtType:
        """Shorthand for appending a statement to the current block of current frame."""
        return self.current_frame.append_stmt(stmt)

    def push_frame(self, frame: Frame):
        frame.parent = self._current_frame
        self._current_frame = frame
        return frame

    def pop_frame(self):
        if self._current_frame is None:
            raise ValueError("No frame to pop")
        frame = self._current_frame
        self._current_frame = frame.parent
        return frame

    def update_lineno(self, node):
        self.source = SourceInfo.from_ast(node, self.lineno_offset, self.col_offset)

    def __repr__(self) -> str:
        return f"LoweringState({self.current_frame})"

    def visit(self, node: ast.AST) -> Result:
        self.update_lineno(node)
        name = node.__class__.__name__
        if name in self.registry:
            return self.registry[name].lower(self, node)
        elif isinstance(node, ast.Call):
            # NOTE: if lower_Call is implemented,
            # it will be called first before __dispatch_Call
            # because "Call" exists in self.registry
            return self.__dispatch_Call(node)
        return super().visit(node)

    def generic_visit(self, node: ast.AST):
        raise DialectLoweringError(f"unsupported ast node {type(node)}:")

    def __dispatch_Call(self, node: ast.Call):
        # 1. try to lookup global statement object
        # 2. lookup local values
        global_callee_result = self.get_global_nothrow(node.func)
        if global_callee_result is None:  # not found in globals, has to be local
            return self.__lower_Call_local(node)

        global_callee = global_callee_result.unwrap()
        if isinstance(global_callee, Method):
            if "Call_global_method" in self.registry:
                return self.registry["Call_global_method"].lower_Call_global_method(
                    self, global_callee, node
                )
            raise DialectLoweringError("`lower_Call_global_method` not implemented")
        elif inspect.isclass(global_callee):
            if issubclass(global_callee, Statement):
                if global_callee.dialect is None:
                    raise DialectLoweringError(
                        f"unsupported dialect `None` for {global_callee.name}"
                    )

                if global_callee.dialect not in self.dialects.data:
                    raise DialectLoweringError(
                        f"unsupported dialect `{global_callee.dialect.name}`"
                    )

                if (
                    trait := global_callee.get_trait(traits.FromPythonCall)
                ) is not None:
                    return trait.from_python(global_callee, self, node)

                raise DialectLoweringError(
                    f"unsupported callee {global_callee.__name__}, "
                    "missing FromPythonAST lowering, or traits.FromPythonCall trait"
                )
            elif issubclass(global_callee, slice):
                if "Call_slice" in self.registry:
                    return self.registry["Call_slice"].lower_Call_slice(self, node)
                raise DialectLoweringError("`lower_Call_slice` not implemented")
            elif issubclass(global_callee, range):
                if "Call_range" in self.registry:
                    return self.registry["Call_range"].lower_Call_range(self, node)
                raise DialectLoweringError("`lower_Call_range` not implemented")
        elif inspect.isbuiltin(global_callee):
            name = f"Call_{global_callee.__name__}"
            if "Call_builtins" in self.registry:
                dialect_lowering = self.registry["Call_builtins"]
                return dialect_lowering.lower_Call_builtins(self, node)
            elif name in self.registry:
                dialect_lowering = self.registry[name]
                return getattr(dialect_lowering, f"lower_{name}")(self, node)
            else:
                raise DialectLoweringError(
                    f"`lower_{name}` is not implemented for builtin function `{global_callee.__name__}`."
                )

        # symbol exist in global, but not ir.Statement, it may refer to a
        # local value that shadows the global value
        try:
            return self.__lower_Call_local(node)
        except DialectLoweringError:
            # symbol exist in global, but not ir.Statement, not found in locals either
            # this means the symbol is referring to an external uncallable object
            if inspect.isfunction(global_callee):
                raise DialectLoweringError(
                    f"unsupported callee: {type(global_callee)}."
                    "Are you trying to call a python function? This is not supported."
                )
            else:  # well not much we can do, can't hint
                raise DialectLoweringError(
                    f"unsupported callee type: {type(global_callee)}"
                )

    def __lower_Call_local(self, node: ast.Call) -> Result:
        callee = self.visit(node.func).expect_one()
        if "Call_local" in self.registry:
            return self.registry["Call_local"].lower_Call_local(self, callee, node)
        raise DialectLoweringError("`lower_Call_local` not implemented")

    def _parse_arg(
        self,
        group_names: set[str],
        target: dict,
        name: str,
        value: ast.AST,
    ):
        if name in group_names:
            if not isinstance(value, ast.Tuple):
                raise DialectLoweringError(f"Expected tuple for group argument {name}")
            target[name] = tuple(self.visit(elem).expect_one() for elem in value.elts)
        else:
            target[name] = self.visit(value).expect_one()

    ValueT = TypeVar("ValueT", bound=SSAValue)

    def exhaust(self, frame: Frame | None = None) -> Frame:
        """Exhaust given frame's stream. If not given, exhaust current frame's stream."""
        if not frame:
            current_frame = self.current_frame
        else:
            current_frame = frame

        stream = current_frame.stream
        while stream.has_next():
            stmt = stream.pop()
            self.visit(stmt)
        return current_frame

    def error_hint(self) -> str:
        begin = max(0, self.source.lineno - self.max_lines)
        end = max(self.source.lineno + self.max_lines, self.source.end_lineno or 0)
        end = min(len(self.lines), end)  # make sure end is within bounds
        lines = self.lines[begin:end]
        code_indent = min(map(self.__get_indent, lines), default=0)
        lines.append("")  # in case the last line errors

        snippet_lines = []
        for lineno, line in enumerate(lines, begin):
            if lineno == self.source.lineno:
                snippet_lines.append(("-" * (self.source.col_offset)) + "^")

            snippet_lines.append(line[code_indent:])

        return "\n".join(snippet_lines)

    @staticmethod
    def __get_indent(line: str) -> int:
        if len(line) == 0:
            return int(1e9)  # very large number
        return len(line) - len(line.lstrip())

    @dataclass
    class GlobalRefResult:
        data: Any

        def unwrap(self):
            return self.data

        ExpectT = TypeVar("ExpectT")

        def expect(self, typ: type[ExpectT]) -> ExpectT:
            if not isinstance(self.data, typ):
                raise DialectLoweringError(f"expected {typ}, got {type(self.data)}")
            return self.data

    def get_global_nothrow(self, node) -> GlobalRefResult | None:
        try:
            return self.get_global(node)
        except DialectLoweringError:
            return None

    def get_global(self, node) -> GlobalRefResult:
        return getattr(
            self, f"get_global_{node.__class__.__name__}", self.get_global_fallback
        )(node)

    def get_global_fallback(self, node: ast.AST) -> GlobalRefResult:
        raise DialectLoweringError(
            f"unsupported global access get_global_{node.__class__.__name__}: {ast.unparse(node)}"
        )

    def get_global_Constant(self, node: ast.Constant) -> GlobalRefResult:
        return self.GlobalRefResult(node.value)

    def get_global_str(self, node: str) -> GlobalRefResult:
        if node in (globals := self.current_frame.globals):
            return self.GlobalRefResult(globals[node])

        if hasattr(builtins, node):
            return self.GlobalRefResult(getattr(builtins, node))

        raise DialectLoweringError(f"global {node} not found")

    def get_global_Name(self, node: ast.Name) -> GlobalRefResult:
        return self.get_global_str(node.id)

    def get_global_Attribute(self, node: ast.Attribute) -> GlobalRefResult:
        if not isinstance(node.ctx, ast.Load):
            raise DialectLoweringError("unsupported attribute access")

        match node.value:
            case ast.Name(id):
                value = self.get_global_str(id).unwrap()
            case ast.Attribute():
                value = self.get_global(node.value).unwrap()
            case _:
                raise DialectLoweringError("unsupported attribute access")

        if hasattr(value, node.attr):
            return self.GlobalRefResult(getattr(value, node.attr))

        raise DialectLoweringError(f"attribute {node.attr} not found in {value}")

    def get_global_Subscript(self, node: ast.Subscript) -> GlobalRefResult:
        value = self.get_global(node.value).unwrap()
        if isinstance(node.slice, ast.Tuple):
            index = tuple(self.get_global(elt).unwrap() for elt in node.slice.elts)
        else:
            index = self.get_global(node.slice).unwrap()
        return self.GlobalRefResult(value[index])

    def get_global_Call(self, node: ast.Call) -> GlobalRefResult:
        func = self.get_global(node.func).unwrap()
        args = [self.get_global(arg).unwrap() for arg in node.args]
        kwargs = {kw.arg: self.get_global(kw.value).unwrap() for kw in node.keywords}
        return self.GlobalRefResult(func(*args, **kwargs))
