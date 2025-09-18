from kirin.prelude import structural
from kirin.dialects import py, func
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
