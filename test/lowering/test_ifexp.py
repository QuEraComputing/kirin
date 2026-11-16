from pytest import mark, raises

from kirin import ir, types
from kirin.prelude import basic, structural, basic_no_opt, structural_no_opt
from kirin.dialects import cf, scf
from kirin.lowering import BuildError

pytestmark = mark.parametrize(
    "prelude", [basic_no_opt, structural_no_opt, basic, structural]
)


def verify(method: ir.Method):
    method.verify()
    for stmt in method.callable_region.walk():
        if isinstance(stmt, cf.ConditionalBranch):
            edges = (
                (stmt.then_arguments, stmt.then_successor),
                (stmt.else_arguments, stmt.else_successor),
            )
        elif isinstance(stmt, cf.Branch):
            edges = ((stmt.arguments, stmt.successor),)
        elif isinstance(stmt, scf.IfElse):
            edges = []
            for region in (stmt.then_body, stmt.else_body):
                assert len(region.blocks[0].args) == 1
                terminator = region.blocks[0].last_stmt
                assert isinstance(terminator, scf.Yield)
                assert len(terminator.values) == len(stmt.results)
                for value, result in zip(terminator.values, stmt.results):
                    assert value.type.is_subseteq(result.type)
        else:
            continue
        for values, successor in edges:
            assert stmt.parent is not None
            assert successor.parent is stmt.parent.parent
            assert len(values) == len(successor.args)
            for value, arg in zip(values, successor.args):
                assert value.type == types.Any or value.type.is_subseteq(arg.type)


def test_ifexp(prelude: ir.DialectGroup):
    @prelude
    def choose(flag: bool) -> int:
        return 10 if flag else 20

    assert choose(True) == 10
    assert choose(False) == 20
    verify(choose)

    if prelude in (basic_no_opt, structural_no_opt):
        branches = [
            stmt
            for stmt in choose.callable_region.walk()
            if isinstance(stmt, (cf.ConditionalBranch, scf.IfElse))
        ]
        assert len(branches) == 1
        branch = branches[0]
        if isinstance(branch, scf.IfElse):
            assert len(branch.results) == 1
            assert branch.results[0].type == types.Int
        else:
            then_exit = branch.then_successor.last_stmt
            else_exit = branch.else_successor.last_stmt
            assert isinstance(then_exit, cf.Branch)
            assert isinstance(else_exit, cf.Branch)
            assert then_exit.successor is else_exit.successor
            assert len(then_exit.successor.args) == 1
            assert then_exit.successor.args[0].type == types.Int


def test_composition(prelude: ir.DialectGroup):
    @prelude
    def add(a: int, b: int) -> int:
        return a + b

    @prelude
    def choose(flag: bool, a: int, b: int) -> int:
        value = add(5 + (a if flag else b), a if flag else b)
        return value + 1

    assert choose(True, 10, 20) == 26
    assert choose(False, 10, 20) == 46
    verify(choose)


def test_nested(prelude: ir.DialectGroup):
    @prelude
    def choose(flag: bool, other: bool) -> int:
        return (1 if other else 2) if flag else (3 if other else 4)

    @prelude
    def nested_condition(flag: bool, other: bool) -> int:
        return 10 if (flag if other else False) else 20

    for flag in (True, False):
        for other in (True, False):
            assert choose(flag, other) == (
                (1 if other else 2) if flag else (3 if other else 4)
            )
            assert nested_condition(flag, other) == (10 if flag and other else 20)
    verify(choose)
    verify(nested_condition)


def test_lazy_evaluation(prelude: ir.DialectGroup):
    @prelude
    def record(log, value: int) -> int:
        log[0] = 10 * log[0] + value
        return value

    @prelude
    def choose(flag: bool, log) -> int:
        return (
            record(log, 5)
            + (record(log, 2) if record(log, 1) == flag else record(log, 3))
            + record(log, 4)
        )

    for flag, expected, trace in ((True, 11, 5124), (False, 12, 5134)):
        log = [0]
        assert choose(flag, log) == expected
        assert log == [trace]
    verify(choose)


@mark.parametrize("flag", [0, 7, -1, None])
def test_truthiness(prelude: ir.DialectGroup, flag):
    @prelude
    def choose(value):
        return value if value else 20

    assert choose(flag) == (flag if flag else 20)
    verify(choose)


def test_optional_result(prelude: ir.DialectGroup):
    @prelude
    def choose(flag: bool) -> int | None:
        return 10 if flag else None

    assert choose(True) == 10
    assert choose(False) is None
    verify(choose)
    terminator = choose.callable_region.blocks[-1].last_stmt
    assert terminator is not None
    result = terminator.args[0]
    assert types.Int.is_subseteq(result.type)
    assert types.NoneType.is_subseteq(result.type)


def test_inside_control_flow(prelude: ir.DialectGroup):
    @prelude
    def choose(flag: bool) -> int:
        value = 0
        for i in range(3):
            if flag:
                value = value + (i if i > 0 else 10)
            else:
                value = value + (20 if i > 0 else i)
        return value

    assert choose(True) == 13
    assert choose(False) == 40
    verify(choose)


def test_branch_assignment_rejected(prelude: ir.DialectGroup):
    with raises(BuildError, match="Assignment expressions in conditional branches"):

        @prelude
        def choose(flag: bool):
            x = 0
            result = (x := 1) if flag else (x := 2)
            return result, x
