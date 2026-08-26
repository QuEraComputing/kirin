"""SSA operand-reference serialization and use-def reconstruction tests."""

from __future__ import annotations

import copy
import gzip
import json
from typing import Any, Literal
from collections.abc import Iterator

import bson
import pytest

from kirin import ir
from kirin.prelude import basic_no_opt
from kirin.serialization.bsonserializer import CompressedBSONSerializer
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule


@basic_no_opt
def straight_line(x: int) -> int:
    y = x + 1
    return y * 2


@basic_no_opt
def branching(x: int, flag: bool) -> int:
    if flag:
        y = x + 1
    else:
        y = x - 1
    return y * 2


@basic_no_opt
def looping(n: int) -> int:
    acc = 0
    for i in range(n):
        acc = acc + i
    return acc


@basic_no_opt
def nested_capture(y: int) -> int:
    def inner(x: int) -> int:
        return x * y + 1

    return inner(3)


@basic_no_opt
def repeated_operand(x: int) -> int:
    return x + x


@basic_no_opt
def make_detached_capture(y: int):
    def inner(x: int) -> int:
        return x * y + 1

    return inner


detached_capture = make_detached_capture(y=10)


@basic_no_opt
def detached_capture_caller(x: int) -> int:
    return detached_capture(x)


@basic_no_opt
def make_detached_result_capture(y: int):
    captured = y + 1

    def inner(x: int) -> int:
        return x * captured

    return inner


detached_result_capture = make_detached_result_capture(y=10)


@basic_no_opt
def detached_result_capture_caller(x: int) -> int:
    return detached_result_capture(x)


Transport = Literal["module", "json", "cbson"]
TRANSPORTS: tuple[Transport, ...] = ("module", "json", "cbson")

KERNELS = (
    pytest.param(straight_line, ((3,),), id="straight-line"),
    pytest.param(branching, ((3, True), (3, False)), id="branching-cfg"),
    pytest.param(looping, ((0,), (5,)), id="loop-block-args"),
    pytest.param(nested_capture, ((4,),), id="nested-capture"),
)


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


def _walk_wire(value: Any) -> Iterator[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for item in value.values():
            yield from _walk_wire(item)
    elif isinstance(value, list):
        for item in value:
            yield from _walk_wire(item)


def _through_transport(
    module: SerializationModule, transport: Transport
) -> SerializationModule:
    if transport == "module":
        return module
    if transport == "json":
        codec = JSONSerializer()
        return codec.decode(codec.encode(module))

    codec = CompressedBSONSerializer()
    return codec.decode(codec.encode(module))


def _round_trip(
    method: ir.Method, transport: Transport
) -> tuple[SerializationModule, ir.Method]:
    module = _through_transport(method.dialects.encode(method), transport)
    return module, method.dialects.decode(module)


def _walk_blocks(statement: ir.Statement) -> Iterator[ir.Block]:
    for region in statement.regions:
        for block in region.blocks:
            yield block
            for child in block.stmts:
                yield from _walk_blocks(child)


def _assert_exact_reverse_uses(method: ir.Method) -> None:
    values: set[ir.SSAValue] = set()
    expected: dict[ir.SSAValue, set[ir.Use]] = {}

    for block in _walk_blocks(method.code):
        values.update(block.args)
        for stmt in block.stmts:
            values.update(stmt.results)
            values.update(stmt.args)
            for index, operand in enumerate(stmt.args):
                expected.setdefault(operand, set()).add(ir.Use(stmt, index))

    for value in values:
        assert value.uses == expected.get(value, set())


def _assert_operand_refs_and_owner_definitions(module: SerializationModule) -> None:
    units = list(_walk_units(module.body))
    statements = [unit for unit in units if unit.kind == "statement"]
    blocks = [unit for unit in units if unit.kind == "block"]
    assert statements
    assert blocks

    operand_refs: list[SerializationUnit] = []
    result_definitions: list[SerializationUnit] = []
    block_arg_definitions: list[SerializationUnit] = []

    for statement in statements:
        operands = statement.data["_args"]
        assert isinstance(operands, SerializationUnit)
        assert operands.kind == "tuple"
        encoded_operands = operands.data["value"]
        assert all(operand.kind == "ssa_ref" for operand in encoded_operands)
        operand_refs.extend(encoded_operands)

        results = statement.data["_results"]
        assert isinstance(results, SerializationUnit)
        assert results.kind == "list"
        encoded_results = results.data["value"]
        assert all(result.kind == "result-value" for result in encoded_results)
        result_definitions.extend(encoded_results)

    for block in blocks:
        encoded_args = block.data["_args"]
        assert all(arg.kind == "block-arg" for arg in encoded_args)
        block_arg_definitions.extend(encoded_args)

    assert operand_refs
    assert result_definitions
    assert block_arg_definitions

    definitions = [*block_arg_definitions, *result_definitions]
    definition_ids = [definition.data["id"] for definition in definitions]
    assert all(isinstance(ssa_id, str) for ssa_id in definition_ids)
    assert len(definition_ids) == len(set(definition_ids))

    for reference in operand_refs:
        assert reference.module_name == ""
        assert reference.class_name == ""
        assert set(reference.data) == {"id"}
        assert isinstance(reference.data["id"], str)
        assert reference.data["id"] in definition_ids


@pytest.mark.parametrize("transport", TRANSPORTS)
@pytest.mark.parametrize(("kernel", "argument_sets"), KERNELS)
def test_ssa_refs_round_trip_real_ir_shapes(
    kernel: ir.Method,
    argument_sets: tuple[tuple[object, ...], ...],
    transport: Transport,
) -> None:
    _assert_exact_reverse_uses(kernel)
    module, decoded = _round_trip(kernel, transport)

    _assert_operand_refs_and_owner_definitions(module)
    _assert_exact_reverse_uses(decoded)
    decoded.verify()
    assert decoded.code.is_structurally_equal(kernel.code)
    for args in argument_sets:
        assert decoded(*args) == kernel(*args)


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_repeated_operand_preserves_identity_and_both_use_indices(
    transport: Transport,
) -> None:
    module, decoded = _round_trip(repeated_operand, transport)
    _assert_operand_refs_and_owner_definitions(module)

    (add,) = (stmt for stmt in decoded.code.walk() if stmt.name == "add")
    lhs, rhs = add.args
    assert lhs is rhs
    assert lhs.uses == {ir.Use(add, 0), ir.Use(add, 1)}
    _assert_exact_reverse_uses(decoded)


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_reordered_reachable_blocks_round_trip(transport: Transport) -> None:
    method = branching.similar()
    region = method.callable_region
    blocks = list(region.blocks)
    assert len(blocks) > 2

    region._blocks[:] = [blocks[0], *reversed(blocks[1:])]
    region._block_idx = {block: index for index, block in enumerate(region.blocks)}

    module, decoded = _round_trip(method, transport)
    _assert_operand_refs_and_owner_definitions(module)
    _assert_exact_reverse_uses(decoded)
    decoded.verify()
    assert decoded.code.is_structurally_equal(method.code)
    assert decoded(3, True) == method(3, True)
    assert decoded(3, False) == method(3, False)


@pytest.mark.parametrize("transport", ["json", "cbson"])
def test_ssa_ref_uses_short_compact_wire_shape(transport: str) -> None:
    module = repeated_operand.dialects.encode(repeated_operand)
    if transport == "json":
        wire = json.loads(JSONSerializer().encode(module))
    else:
        encoded = CompressedBSONSerializer().encode(module)
        wire = bson.decode(gzip.decompress(encoded))

    refs = [
        mapping["$u"]
        for mapping in _walk_wire(wire)
        if len(mapping) == 1 and "$u" in mapping and mapping["$u"][0] == "ssa_ref"
    ]
    assert refs
    assert all(len(ref) == 2 and set(ref[1]) == {"id"} for ref in refs)


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_detached_capture_keeps_one_full_external_definition(
    transport: Transport,
) -> None:
    module, decoded = _round_trip(detached_capture_caller, transport)
    statements = [unit for unit in _walk_units(module.body) if unit.kind == "statement"]
    operands = [
        operand
        for statement in statements
        for operand in statement.data["_args"].data["value"]
    ]

    external_definitions = [
        operand for operand in operands if operand.kind == "block-arg"
    ]
    assert len(external_definitions) == 1
    assert external_definitions[0].data["name"] == "y"
    assert all(
        operand.kind == "ssa_ref" or operand is external_definitions[0]
        for operand in operands
    )

    decoded.verify()
    assert decoded(3) == detached_capture_caller(3) == 31


def test_external_result_capture_fails_before_writing_an_undecodable_payload() -> None:
    with pytest.raises(
        ValueError,
        match=(
            "Cannot serialize ResultValue operand before its definition: its owner "
            "is external or the IR is not in definition order"
        ),
    ):
        detached_result_capture_caller.dialects.encode(detached_result_capture_caller)


def test_ssa_ref_does_not_allocate_an_id_for_an_undefined_value() -> None:
    serializer = Serializer()
    value = ir.TestValue()

    with pytest.raises(
        ValueError, match="Cannot serialize an SSA reference before its definition"
    ):
        serializer.serialize_ssa_ref(value)

    assert value not in serializer._ctx.ssa_idtable.table


MALFORMED_REFS = (
    pytest.param({}, r"ssa_ref is missing required 'id'", id="missing-id"),
    pytest.param(
        {"id": 17}, r"ssa_ref id must be a string, got 17", id="non-string-id"
    ),
    pytest.param(
        {"id": "not-defined"},
        r"dangling ssa_ref 'not-defined'",
        id="dangling-id",
    ),
)


def test_ssa_ref_data_must_be_a_mapping() -> None:
    module = straight_line.dialects.encode(straight_line)
    reference = next(
        unit for unit in _walk_units(module.body) if unit.kind == "ssa_ref"
    )
    reference.data = None  # type: ignore[assignment]

    with pytest.raises(ValueError, match="ssa_ref data must be a mapping"):
        straight_line.dialects.decode(module)


@pytest.mark.parametrize("transport", TRANSPORTS)
@pytest.mark.parametrize(("bad_data", "error"), MALFORMED_REFS)
def test_malformed_ssa_refs_fail_with_context(
    bad_data: dict[str, object], error: str, transport: Transport
) -> None:
    module = straight_line.dialects.encode(straight_line)
    reference = next(
        unit for unit in _walk_units(module.body) if unit.kind == "ssa_ref"
    )
    reference.data = bad_data
    module = _through_transport(module, transport)

    with pytest.raises(ValueError, match=error):
        straight_line.dialects.decode(module)


def _with_verbose_v1_operands(module: SerializationModule) -> SerializationModule:
    module = copy.deepcopy(module)
    definitions = {
        unit.data["id"]: unit
        for unit in _walk_units(module.body)
        if unit.kind in ("block-arg", "result-value")
    }

    for statement in (
        unit for unit in _walk_units(module.body) if unit.kind == "statement"
    ):
        operands = statement.data["_args"].data["value"]
        statement.data["_args"].data["value"] = [
            copy.deepcopy(definitions[operand.data["id"]]) for operand in operands
        ]
    return module


def _to_verbose_v1(value: Any) -> Any:
    if isinstance(value, SerializationModule):
        return {
            "__serialization_module__": True,
            "version": value.version,
            "symbol_table": _to_verbose_v1(value.symbol_table),
            "body": _to_verbose_v1(value.body),
        }
    if isinstance(value, SerializationUnit):
        return {
            "__serialization_unit__": True,
            "kind": value.kind,
            "module_name": value.module_name,
            "class_name": value.class_name,
            "data": _to_verbose_v1(value.data),
        }
    if isinstance(value, dict):
        return {key: _to_verbose_v1(item) for key, item in value.items()}
    if isinstance(value, (list, tuple)):
        return [_to_verbose_v1(item) for item in value]
    return value


@pytest.mark.parametrize("transport", ["json", "cbson"])
def test_verbose_v1_full_ssa_operands_remain_readable(transport: str) -> None:
    old_module = _with_verbose_v1_operands(branching.dialects.encode(branching))
    verbose = _to_verbose_v1(old_module)

    if transport == "json":
        module = JSONSerializer().decode(json.dumps(verbose))
    else:
        payload = gzip.compress(bson.encode(verbose), mtime=0)
        module = CompressedBSONSerializer().decode(payload)

    operand_kinds = {
        operand.kind
        for statement in _walk_units(module.body)
        if statement.kind == "statement"
        for operand in statement.data["_args"].data["value"]
    }
    assert operand_kinds <= {"block-arg", "result-value"}
    assert operand_kinds == {"block-arg", "result-value"}

    decoded = branching.dialects.decode(module)
    _assert_exact_reverse_uses(decoded)
    decoded.verify()
    assert decoded(3, True) == branching(3, True)
    assert decoded(3, False) == branching(3, False)
