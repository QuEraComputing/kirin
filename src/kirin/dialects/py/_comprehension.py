import ast

from kirin import lowering

_LISTCOMP_TMP_PREFIX = "_kirin_listcomp_tmp"
_SETCOMP_TMP_PREFIX = "_kirin_setcomp_tmp"


def lower_listcomp_via_desugaring(
    state: lowering.State, node: ast.ListComp
) -> lowering.Result:
    tmp_name = fresh_comp_name(state, _LISTCOMP_TMP_PREFIX)
    init = ast.List(elts=[], ctx=ast.Load())
    leaf = ast.Assign(
        targets=[ast.Name(id=tmp_name, ctx=ast.Store())],
        value=ast.BinOp(
            left=ast.Name(id=tmp_name, ctx=ast.Load()),
            op=ast.Add(),
            right=ast.List(elts=[node.elt], ctx=ast.Load()),
        ),
    )
    fix_locations(leaf, node.elt)
    return lower_comprehension_via_desugaring(
        state=state,
        tmp_name=tmp_name,
        init_value=init,
        generators=node.generators,
        leaf_stmt=leaf,
        ref_node=node,
    )


def lower_setcomp_via_desugaring(
    state: lowering.State, node: ast.SetComp
) -> lowering.Result:
    tmp_name = fresh_comp_name(state, _SETCOMP_TMP_PREFIX)
    init = ast.Call(func=ast.Name(id="set", ctx=ast.Load()), args=[], keywords=[])
    leaf = ast.Assign(
        targets=[ast.Name(id=tmp_name, ctx=ast.Store())],
        value=ast.BinOp(
            left=ast.Name(id=tmp_name, ctx=ast.Load()),
            op=ast.BitOr(),
            right=ast.Set(elts=[node.elt]),
        ),
    )
    fix_locations(leaf, node.elt)
    return lower_comprehension_via_desugaring(
        state=state,
        tmp_name=tmp_name,
        init_value=init,
        generators=node.generators,
        leaf_stmt=leaf,
        ref_node=node,
    )


def lower_comprehension_via_desugaring(
    state: lowering.State,
    tmp_name: str,
    init_value: ast.expr,
    generators: list[ast.comprehension],
    leaf_stmt: ast.stmt,
    ref_node: ast.AST,
) -> lowering.Result:
    init = ast.Assign(
        targets=[ast.Name(id=tmp_name, ctx=ast.Store())],
        value=init_value,
    )
    fix_locations(init, ref_node)
    state.lower(init)

    for stmt in build_comprehension_stmts(generators, leaf_stmt):
        state.lower(stmt)

    result = ast.Name(id=tmp_name, ctx=ast.Load())
    fix_locations(result, ref_node)
    return state.lower(result).expect_one()


def fresh_comp_name(state: lowering.State, prefix: str) -> str:
    frame = state.current_frame
    idx = 0
    while True:
        suffix = "" if idx == 0 else f"_{idx}"
        name = f"{prefix}{suffix}"
        if frame.get_local(name) is None and name not in frame.globals:
            return name
        idx += 1


def build_comprehension_stmts(
    generators: list[ast.comprehension], leaf_stmt: ast.stmt
) -> list[ast.stmt]:
    acc = leaf_stmt
    for gen in reversed(generators):
        for if_ in reversed(gen.ifs):
            acc = ast.If(test=if_, body=[acc], orelse=[])
            fix_locations(acc, if_)

        acc = ast.For(target=gen.target, iter=gen.iter, body=[acc], orelse=[])
        fix_locations(acc, gen)

    return [acc]


def fix_locations(node: ast.AST, ref: ast.AST) -> None:
    ast.copy_location(node, ref)
    ast.fix_missing_locations(node)
