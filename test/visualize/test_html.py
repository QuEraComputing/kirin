from kirin.prelude import basic
from kirin.visualize import to_html, write_html


@basic
def add_one(x: int):
    return x + 1


def test_html_inspector_includes_ssa_and_statement_metadata(tmp_path):
    result = add_one.callable_region.blocks[0].stmts.at(0).result
    page = to_html(add_one, analysis={result: "constant one"})

    assert 'data-ssa="ssa-' in page
    assert 'data-stmt="stmt-' in page
    assert '"Producer"' in page
    assert '"Uses"' in page
    assert '"Class"' in page
    assert '"Attributes"' in page
    assert "constant one" in page

    path = add_one.visualize(tmp_path / "add_one.html")
    assert path.read_text(encoding="utf-8").startswith("<!doctype html>")


def test_write_html_accepts_a_statement(tmp_path):
    statement = add_one.callable_region.blocks[0].stmts.at(0)
    path = write_html(statement, tmp_path / "statement.html")

    page = path.read_text(encoding="utf-8")
    assert "Kirin IR Inspector" in page
    assert statement.name in page
