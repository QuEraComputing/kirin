import pytest

from kirin.source import SourceInfo
from kirin.prelude import basic_no_opt


def get_line_of(target: str) -> int:
    for i, line in enumerate(open(__file__), 1):
        if target in line:
            return i


@pytest.mark.parametrize("similar", [True, False])
def test_stmt_source_info(similar: bool):
    @basic_no_opt
    def test(x: int):
        y = 2
        a = 4**2
        return y + 2 + a

    if similar:
        test = test.similar()

    stmts = test.callable_region.blocks[0].stmts

    def get_line_from_source_info(source: SourceInfo) -> int:
        return source.lineno + source.lineno_begin

    for stmt in stmts:
        assert stmt.source.file == __file__

    assert get_line_from_source_info(stmts.at(0).source) == get_line_of("y = 2")
    assert get_line_from_source_info(stmts.at(2).source) == get_line_of("a = 4**2")
    assert get_line_from_source_info(stmts.at(4).source) == get_line_of(
        "return y + 2 + a"
    )
