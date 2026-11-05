from kirin import types


def widen(
    previous: types.TypeAttribute, current: types.TypeAttribute
) -> types.TypeAttribute:
    """Include both types, forgetting changing literals within matching generics."""
    if current.is_subseteq(previous):
        return previous
    if previous.is_subseteq(types.Bottom):
        return current
    if (
        isinstance(previous, types.Generic)
        and isinstance(current, types.Generic)
        and previous.body == current.body
        and len(previous.vars) == len(current.vars)
        and previous.vararg is None
        and current.vararg is None
    ):
        return types.Generic(
            previous.body,
            *(widen(old, new) for old, new in zip(previous.vars, current.vars)),
        )
    if isinstance(previous, types.Literal) and isinstance(current, types.Literal):
        return types.Any
    return previous.join(current)
