from kirin import types
from kirin.prelude import basic_no_opt
from kirin.analysis import TypeInference, const
from kirin.dialects import func


@basic_no_opt
def count_down(n: int) -> int:
    total = 0
    if n > 0:
        total = count_down(n - 1) + 1
    return total


# The self call lowers to `func.call`, whose type comes from the method type.
# An invoke of a method that is not inferred yet runs the callee body instead.
_call = next(s for s in count_down.callable_region.walk() if isinstance(s, func.Call))
_call.replace_by(func.Invoke(_call.args[1:], callee=count_down))


class Deepest:
    """A mixin that records the deepest frame stack an interpreter reaches."""

    deepest = 0

    def new_frame(self, node, *, has_parent_access=False):
        self.deepest = max(self.deepest, self.state.depth + 1)
        return super().new_frame(node, has_parent_access=has_parent_access)


class CountingInference(Deepest, TypeInference):
    pass


class CountingPropagate(Deepest, const.Propagate):
    pass


def test_a_recursive_call_with_the_same_types_runs_once():
    inference = CountingInference(basic_no_opt)
    _, result = inference.run(count_down, types.Int)
    assert result == types.Int
    assert inference.deepest < 10


def test_a_recursive_call_with_unknown_constants_runs_once():
    propagate = CountingPropagate(basic_no_opt)
    _, result = propagate.run(count_down, const.Unknown())
    assert isinstance(result, const.Unknown)
    assert propagate.deepest < 10


def test_a_recursive_call_with_constants_unfolds():
    propagate = const.Propagate(basic_no_opt)
    _, result = propagate.run(count_down, const.Value(4))
    assert result == const.Value(4)


@basic_no_opt
def pong(n: int) -> int:
    return n


@basic_no_opt
def ping(n: int) -> int:
    total = 0
    if n > 0:
        total = pong(n - 1) + 1
    return total


_pong_call = next(s for s in pong.callable_region.walk() if isinstance(s, func.Return))
_ping_again = func.Invoke((_pong_call.args[0],), callee=ping)
_ping_again.insert_before(_pong_call)
_pong_call.replace_by(func.Return(_ping_again.result))


def test_a_mutual_recursion_settles():
    inference = CountingInference(basic_no_opt)
    _, result = inference.run(ping, types.Int)
    assert result == types.Int
    assert inference.deepest < 10


@basic_no_opt
def nesting(n: int):
    if n > 0:
        return (nesting(n - 1), 1)
    return 1


_nest_call = next(s for s in nesting.callable_region.walk() if isinstance(s, func.Call))
_nest_call.replace_by(func.Invoke(_nest_call.args[1:], callee=nesting))


def test_a_result_that_grows_every_round_settles_to_top():
    inference = CountingInference(basic_no_opt)
    _, result = inference.run(nesting, types.Int)
    assert result == types.Any
    assert inference.deepest < 10


@basic_no_opt
def inside_a_loop(n: int) -> int:
    total = 0
    for i in range(3):
        total = total + count_down(n)
    return total


def test_a_recursion_inside_a_loop_settles():
    inference = CountingInference(basic_no_opt)
    _, result = inference.run(inside_a_loop, types.Int)
    assert result == types.Int
    assert inference.deepest < 10
    propagate = CountingPropagate(basic_no_opt)
    _, result = propagate.run(inside_a_loop, const.Unknown())
    assert isinstance(result, const.Unknown)
    assert propagate.deepest < 10
