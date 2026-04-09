from kirin.prelude import basic, ilist, basic_no_opt
from kirin.passes.aggressive.fold import Fold


@basic_no_opt
def factorial(n):
    if n == 0:
        return 1
    else:
        return n * factorial(n - 1)


def test_region_clone():
    assert factorial.callable_region.clone().is_structurally_equal(
        factorial.callable_region
    )


@basic
def _leaf(a: bool, b: bool, x: int):
    if a:
        u = x + 1
    else:
        u = x + 2
    if b:
        v = u + 3
    else:
        v = u + 4
    return v


@basic
def _level0(flag0: bool, flag1: bool, x: int):
    base = _leaf(flag0, flag1, x)
    base2 = _leaf(flag1, flag0, x + 1)
    if flag0:
        mix = base + base2
    else:
        mix = base2 + base
    if flag0:
        out = mix + 30
    else:
        out = mix + 40

    def fn(y: int):
        return y + out

    mapped = ilist.map(fn, ilist.range(3))
    return mapped[0] + out


@basic
def _level1(flag0: bool, flag1: bool, x: int):
    base = _level0(flag0, flag1, x)
    base2 = _level0(flag1, flag0, x + 2)
    if flag0:
        mix = base + base2
    else:
        mix = base2 + base
    if flag0:
        out = mix + 31
    else:
        out = mix + 41

    def fn(y: int):
        return y + out

    mapped = ilist.map(fn, ilist.range(3))
    return mapped[0] + out


@basic
def _fold_target(flag0: bool, flag1: bool, x: int):
    return _level1(flag0, flag1, x)


def _has_unordered_edges(method):
    """True if any stmt arg is owned by a stmt in a later block (by index)."""
    stmt_block = {}
    for bi, block in enumerate(method.callable_region.blocks):
        for stmt in block.stmts:
            stmt_block[stmt] = bi
    for bi, block in enumerate(method.callable_region.blocks):
        for stmt in block.stmts:
            for arg in stmt.args:
                owner = getattr(arg, "owner", None)
                owner_bi = stmt_block.get(owner)
                if owner_bi is not None and owner_bi > bi:
                    return True
    return False


def _all_owners_in_region(region):
    """Check every operand in region is owned by a stmt/block inside it."""
    region_stmts = set()
    region_blocks = set()
    for block in region.blocks:
        region_blocks.add(block)
        for stmt in block.stmts:
            region_stmts.add(stmt)

    for block in region.blocks:
        for stmt in block.stmts:
            for arg in stmt.args:
                owner = getattr(arg, "owner", None)
                if owner is None:
                    continue
                # BlockArgument owner is a Block (check first — Block also has parent)
                if hasattr(owner, "stmts"):
                    if owner not in region_blocks:
                        return False
                # ResultValue owner is a Statement
                elif hasattr(owner, "parent"):
                    if owner not in region_stmts:
                        return False
    return True


def test_region_clone_after_aggressive_fold():
    """Region.clone must remap all SSA values even when blocks are out of definition order."""
    mt = _fold_target.similar()
    fold = Fold(mt.dialects)

    # Run fold until we get unordered edges or it converges
    for _ in range(8):
        result = fold.unsafe_run(mt)
        if _has_unordered_edges(mt):
            break
        if not result.has_done_something:
            break

    # Regardless of whether we got unordered edges, clone must be self-contained
    cloned = mt.callable_region.clone()
    assert _all_owners_in_region(
        cloned
    ), "Region.clone produced operands owned by statements outside the cloned region"
    assert cloned.is_structurally_equal(mt.callable_region)
