import gc
import weakref

from kirin.prelude import basic_no_opt


def test_backedges_registers_static_callers():
    @basic_no_opt
    def callee(x: int) -> int:
        return x + 1

    @basic_no_opt
    def caller(x: int) -> int:
        return callee(x) * 2

    assert caller in callee.backedges


def test_redefinition_recompiles_callers():
    @basic_no_opt
    def callee(x: int) -> int:
        return x + 1

    @basic_no_opt
    def caller(x: int) -> int:
        return callee(x) * 2

    assert caller(3) == 8

    @basic_no_opt
    def callee(x: int) -> int:  # noqa: F811
        return x + 100

    assert caller(3) == 206


def test_backedges_do_not_keep_discarded_clones_alive():
    """`similar()` clones must not accumulate in their callees' backedges.

    A clone registers itself into the backedges of every method it statically
    calls, and those callees typically outlive the clone by a lot (they are
    module level kernels). A strongly referenced `backedges` therefore pins
    every clone ever made for the lifetime of the callee, which shows up as
    unbounded heap growth in a compile-per-shot loop.
    """

    @basic_no_opt
    def callee(x: int) -> int:
        return x + 1

    @basic_no_opt
    def caller(x: int) -> int:
        return callee(x) * 2

    baseline = len(callee.backedges)

    for _ in range(50):
        clone = caller.similar()
        assert clone in callee.backedges
        del clone

    gc.collect()
    assert len(callee.backedges) == baseline
    assert caller in callee.backedges


def test_discarded_clone_is_collectable():
    @basic_no_opt
    def callee(x: int) -> int:
        return x + 1

    @basic_no_opt
    def caller(x: int) -> int:
        return callee(x) * 2

    clone = caller.similar()
    ref = weakref.ref(clone)
    del clone

    gc.collect()
    assert ref() is None
