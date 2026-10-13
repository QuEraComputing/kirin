from kirin import lowering
from kirin.prelude import python_no_opt
from kirin.dialects import cf, func

lower = lowering.Python(python_no_opt)


def test_pass():
    def nobody():
        pass

    code = lower.python_function(nobody)
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[-1].last_stmt, func.Return)

    def branch_pass():
        if True:
            pass
        else:
            pass

    code = lower.python_function(branch_pass)
    code.print()
    assert isinstance(code, func.Function)
    assert isinstance(code.body.blocks[0].last_stmt, func.Return)


def test_basic_ifelse():
    def single(n):
        if n == 0:
            return 1
        else:
            return n

    code = lower.python_function(single)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)

    def single_2(n):
        if n == 0:
            n + 1
        else:
            return n

    code = lower.python_function(single_2)
    code.print()
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)

    def single_3(n):
        if n == 0:
            n = n + 1
        else:
            n = n + 2

    code = lower.python_function(single_3)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 4
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, cf.Branch)
    assert code.body.blocks[1].last_stmt.successor is code.body.blocks[3]
    assert isinstance(code.body.blocks[2].last_stmt, cf.Branch)
    assert code.body.blocks[2].last_stmt.successor is code.body.blocks[3]
    assert isinstance(code.body.blocks[3].last_stmt, func.Return)

    def single_4(n):
        if n == 0:
            n = n + 1

    code = lower.python_function(single_4)
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 3
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, cf.Branch)
    assert code.body.blocks[1].last_stmt.successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[2].last_stmt, func.Return)


def test_recursive_ifelse():
    def multi(n):
        if n == 0:
            return 1
        else:
            if n < 5:
                assert n < 6, "n must be less than 10"
            else:
                return n

    code = lower.python_function(multi)
    code.print()
    assert isinstance(code, func.Function)
    assert len(code.body.blocks) == 5
    assert isinstance(code.body.blocks[0].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[0].last_stmt.then_successor is code.body.blocks[1]
    assert code.body.blocks[0].last_stmt.else_successor is code.body.blocks[2]
    assert isinstance(code.body.blocks[1].last_stmt, func.Return)
    assert isinstance(code.body.blocks[2].last_stmt, cf.ConditionalBranch)
    assert code.body.blocks[2].last_stmt.then_successor is code.body.blocks[3]
    assert code.body.blocks[2].last_stmt.else_successor is code.body.blocks[4]
    assert isinstance(code.body.blocks[3].last_stmt, func.Return)
    assert isinstance(code.body.blocks[4].last_stmt, func.Return)


def test_ifelse_order_independent_of_hash_seed(tmp_path):
    import os
    import sys
    import textwrap
    import subprocess

    # lowers the same if/else with cf and with scf, then prints the join block's
    # arguments and the scf.IfElse results
    script = textwrap.dedent("""\
        from kirin.dialects import scf
        from kirin.prelude import basic_no_opt, structural_no_opt


        @basic_no_opt
        def with_cf(x: int, y: int, flag: bool) -> int:
            if flag:
                x = 1
                y = 3
            else:
                x = 2
                y = 4
            return x + y


        @structural_no_opt
        def with_scf(x: int, y: int, flag: bool) -> int:
            if flag:
                x = 1
                y = 3
            else:
                x = 2
                y = 4
            return x + y


        join = with_cf.callable_region.blocks[-1]
        (if_else,) = (s for s in with_scf.callable_region.walk() if isinstance(s, scf.IfElse))
        print([arg.name for arg in join.args], [result.name for result in if_else.results])
        """)

    # the hash seed is fixed per process, so each seed needs a fresh process
    (tmp_path / "ifelse.py").write_text(script)
    outputs = set()
    for seed in range(4):
        env = dict(os.environ, PYTHONHASHSEED=str(seed))
        env["KIRIN_COMPILE_CACHE_DIR"] = "FALSE"
        out = subprocess.run(
            [sys.executable, "ifelse.py"],
            cwd=tmp_path,
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )
        outputs.add(out.stdout)
    assert outputs == {"['flag', 'x', 'y'] ['flag', 'x', 'y']\n"}
