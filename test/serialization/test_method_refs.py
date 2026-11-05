"""Method-reference coverage for an ``animate_ghz``-shaped call graph.

The animation wrapper calls one compiled kernel. Inside that compiled kernel,
multiple call sites can reference the same move kernel. The serializer should
emit that shared method once and use ``method_ref`` thereafter.
"""

from typing import Any
from collections.abc import Iterator

from kirin import ir
from kirin.prelude import basic
from kirin.dialects import func
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.core.serializationunit import SerializationUnit


@basic
def move_kernel(x: int) -> int:
    return x * 2


@basic
def compiled_kernel(x: int) -> int:
    return move_kernel(x) + move_kernel(x + 1) + move_kernel(x + 2)


@basic
def animation_kernel(x: int) -> int:
    return compiled_kernel(x)


def _walk_units(value: Any) -> Iterator[SerializationUnit]:
    if isinstance(value, SerializationUnit):
        yield value
        yield from _walk_units(value.data)
    elif isinstance(value, dict):
        for item in value.values():
            yield from _walk_units(item)
    elif isinstance(value, (list, tuple)):
        for item in value:
            yield from _walk_units(item)


def _invoke_callees(method: ir.Method) -> list[ir.Method]:
    return [stmt.callee for stmt in method.code.walk() if isinstance(stmt, func.Invoke)]


def test_multiple_call_sites_emit_method_refs() -> None:
    module = Serializer().encode(animation_kernel)
    units = tuple(_walk_units(module.body))

    definitions = [unit for unit in units if unit.kind == "method"]
    references = [unit for unit in units if unit.kind == "method_ref"]
    move_definition = next(
        unit for unit in definitions if unit.data["sym_name"] == "move_kernel"
    )

    assert len(definitions) == 3  # wrapper, compiled kernel, shared move kernel
    assert len(references) == 2
    assert {ref.data["id"] for ref in references} == {move_definition.data["id"]}


def test_multiple_call_sites_round_trip_with_shared_identity() -> None:
    codec = JSONSerializer()
    module = codec.decode(codec.encode(Serializer().encode(animation_kernel)))
    decoded = animation_kernel.dialects.decode(module)

    (decoded_compiled_kernel,) = _invoke_callees(decoded)
    move_callees = _invoke_callees(decoded_compiled_kernel)

    decoded.verify()
    assert len(move_callees) == 3
    assert len({id(callee) for callee in move_callees}) == 1
    assert decoded(4) == animation_kernel(4)


def test_region_parent_node_survives_call_sites() -> None:
    """Each duplicate used to build its own ``func.Function`` while the shared
    region kept the first one as ``parent_node``. ``Method.code`` ended up as the
    last, so the region pointed at a discarded statement -- and ``verify()``
    passed anyway.
    """
    codec = JSONSerializer()
    module = codec.decode(codec.encode(Serializer().encode(animation_kernel)))
    decoded = animation_kernel.dialects.decode(module)

    (decoded_compiled_kernel,) = _invoke_callees(decoded)
    move_callee = _invoke_callees(decoded_compiled_kernel)[0]

    assert move_callee.code.regions[0].parent_node is move_callee.code
