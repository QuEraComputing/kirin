from kirin.prelude import structural, structural_no_opt
from kirin.dialects import py, func, ilist
from kirin.passes.aggressive import UnrollScf


def test_unroll_scf():
    @structural
    def main(r: list[int], cond: bool):
        if cond:
            for i in range(4):
                tmp = r[-1]
                if i < 2:
                    tmp += i * 2
                else:
                    for j in range(4):
                        if i > j:
                            tmp += i + j
                        else:
                            tmp += i - j

                r.append(tmp)
        else:
            for i in range(4):
                r.append(i)
        return r

    UnrollScf(structural).fixpoint(main)

    num_adds = 0
    num_calls = 0

    for op in main.callable_region.walk():
        if isinstance(op, py.Add):
            num_adds += 1
        elif isinstance(op, func.Call):
            num_calls += 1

    assert num_adds == 10
    assert num_calls == 8


def test_dce_unroll_typeinfer():
    # NOTE: tests bug in typeinfer preventing DCE from issue#564

    @structural_no_opt
    def main():
        ls = [1, 2, 3]
        for i in range(1):
            ls[i] = 10
        return ls

    UnrollScf(main.dialects).fixpoint(main)

    for stmt in main.callable_region.stmts():
        if isinstance(stmt, py.Constant) and isinstance(
            value := stmt.value, ilist.IList
        ):
            assert not isinstance(value.data, range), "Unused range not eliminated!"
