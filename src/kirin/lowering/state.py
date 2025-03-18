import ast
import inspect
import builtins
from typing import TYPE_CHECKING, Any, TypeVar, get_origin
from dataclasses import dataclass

from kirin.ir import Method, SSAValue, Statement, DialectGroup, traits
from kirin.source import SourceInfo
from kirin.exceptions import DialectLoweringError
from kirin.lowering.frame import Frame
from kirin.lowering.result import Result
from kirin.lowering.binding import Binding
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
        stmt = self.current_frame.curr_region.blocks[0].first_stmt
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

    def pop_frame(self, finalize_next: bool = True):
        """Pop the current frame and return it.

        Args:
            finalize_next(bool): If True, append the next block of the current frame.

        Returns:
            Frame: The popped frame.
        """
        if self._current_frame is None:
            raise ValueError("No frame to pop")
        frame = self._current_frame

        if finalize_next and frame.next_block.parent is None:
            frame.append_block(frame.next_block)
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
        elif isinstance(node, ast.With):
            return self.__dispatch_With(node)
        return super().visit(node)

    def generic_visit(self, node: ast.AST):
        raise DialectLoweringError(f"unsupported ast node {type(node)}:")

    def __dispatch_With(self, node: ast.With):
        if len(node.items) != 1:
            raise DialectLoweringError("expected exactly one item in with statement")

        item = node.items[0]
        if not isinstance(item.context_expr, ast.Call):
            raise DialectLoweringError("expected context expression to be a call")

        global_callee_result = self.get_global_nothrow(item.context_expr.func)
        if global_callee_result is None:
            raise DialectLoweringError("cannot find call func in with context")

        global_callee = global_callee_result.unwrap()
        if not issubclass(global_callee, Statement):
            raise DialectLoweringError("expected callee to be a statement")

        if (
            trait := global_callee.get_trait(traits.FromPythonWithSingleItem)
        ) is not None:
            return trait.lower(global_callee, self, node)

        raise DialectLoweringError(
            "unsupported callee, missing FromPythonWithSingleItem trait"
        )

    def __dispatch_Call(self, node: ast.Call):
        # 1. try to lookup global statement object
        # 2. lookup local values
        global_callee_result = self.get_global_nothrow(node.func)
        if global_callee_result is None:  # not found in globals, has to be local
            return self.__lower_Call_local(node)

        global_callee = global_callee_result.unwrap()
        if isinstance(global_callee, Binding):
            global_callee = global_callee.parent

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
                    return trait.lower(global_callee, self, node)

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
                    f"unsupported callee: {repr(global_callee)}."
                    "Are you trying to call a python function? This is not supported."
                )
            else:  # well not much we can do, can't hint
                raise DialectLoweringError(
                    f"unsupported callee type: {repr(global_callee)}"
                )

    def __lower_Call_local(self, node: ast.Call) -> Result:
        callee = self.visit(node.func).expect_one()
        if "Call_local" in self.registry:
            return self.registry["Call_local"].lower_Call_local(self, callee, node)
        raise DialectLoweringError("`lower_Call_local` not implemented")

    def default_Call_lower(self, stmt: type[Statement], node: ast.Call) -> Result:
        """Default lowering for Python call to statement.

        This method is intended to be used by traits like `FromPythonCall` to
        provide a default lowering for Python calls to statements.

        Args:
            stmt(type[Statement]): Statement class to construct.
            node(ast.Call): Python call node to lower.

        Returns:
            Result: Result of lowering the Python call to statement.
        """
        args, kwargs = self.default_Call_inputs(stmt, node)
        return Result(self.append_stmt(stmt(*args.values(), **kwargs)))

    def default_Call_inputs(
        self, stmt: type[Statement], node: ast.Call
    ) -> tuple[dict[str, SSAValue | tuple[SSAValue, ...]], dict[str, Any]]:
        from kirin.decl import fields

        fs = fields(stmt)
        stmt_std_arg_names = fs.std_args.keys()
        stmt_kw_args_name = fs.kw_args.keys()
        stmt_attr_prop_names = fs.attr_or_props
        stmt_required_names = fs.required_names
        stmt_group_arg_names = fs.group_arg_names
        args, kwargs = {}, {}
        for name, value in zip(stmt_std_arg_names, node.args):
            self._parse_arg(stmt_group_arg_names, args, name, value)
        for kw in node.keywords:
            if not isinstance(kw.arg, str):
                raise DialectLoweringError("Expected string for keyword argument name")

            arg: str = kw.arg
            if arg in node.args:
                raise DialectLoweringError(
                    f"Keyword argument {arg} is already present in positional arguments"
                )
            elif arg in stmt_std_arg_names or arg in stmt_kw_args_name:
                self._parse_arg(stmt_group_arg_names, kwargs, kw.arg, kw.value)
            elif arg in stmt_attr_prop_names:
                if (
                    isinstance(kw.value, ast.Name)
                    and self.current_frame.get_local(kw.value.id) is not None
                ):
                    raise DialectLoweringError(
                        f"Expected global/constant value for attribute or property {arg}"
                    )
                global_value = self.get_global_nothrow(kw.value)
                if global_value is None:
                    raise DialectLoweringError(
                        f"Expected global value for attribute or property {arg}"
                    )
                if (decl := fs.attributes.get(arg)) is not None:
                    if decl.annotation is Any:
                        kwargs[arg] = global_value.unwrap()
                    else:
                        kwargs[arg] = global_value.expect(
                            get_origin(decl.annotation) or decl.annotation
                        )
                else:
                    raise DialectLoweringError(
                        f"Unexpected attribute or property {arg}"
                    )
            else:
                raise DialectLoweringError(f"Unexpected keyword argument {arg}")

        for name in stmt_required_names:
            if name not in args and name not in kwargs:
                raise DialectLoweringError(f"Missing required argument {name}")

        return args, kwargs

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
