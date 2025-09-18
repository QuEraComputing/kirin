from kirin import ir

DIALECTS_LOOKUP = {}


def register_dialect(dialect: ir.Dialect):
    stmt_map: dict[str, type] = {}
    for stmt_cls in dialect.stmts:
        stmt_map[stmt_cls.__name__] = stmt_cls
        stmt_declared_name = getattr(stmt_cls, "name", None)
        if stmt_declared_name and stmt_declared_name != stmt_cls.__name__:
            stmt_map[stmt_declared_name] = stmt_cls

    DIALECTS_LOOKUP[dialect.name] = (dialect, stmt_map)
