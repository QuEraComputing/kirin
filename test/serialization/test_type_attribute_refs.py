"""Identity-reference tests for repeated TypeAttribute metadata."""

from __future__ import annotations

import copy
import gzip
import json
from typing import Any, Literal
from collections.abc import Iterator

import bson
import pytest

from kirin import ir, types
from kirin.prelude import basic_no_opt
from kirin.serialization.bsonserializer import CompressedBSONSerializer
from kirin.serialization.jsonserializer import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer
from kirin.serialization.core.serializationunit import SerializationUnit
from kirin.serialization.core.serializationmodule import SerializationModule


@basic_no_opt
def passthrough(x: int) -> int:
    return x


Transport = Literal["module", "json", "cbson"]
TRANSPORTS: tuple[Transport, ...] = ("module", "json", "cbson")


def _method_with_fields(*fields: object) -> ir.Method:
    method = passthrough.similar()
    method.fields = fields
    return method


def _field_units(module: SerializationModule) -> list[SerializationUnit]:
    fields = module.body.data["fields"]
    assert isinstance(fields, SerializationUnit)
    assert fields.kind == "tuple"
    return fields.data["value"]


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


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_repeated_type_attribute_preserves_shared_identity(
    transport: Transport,
) -> None:
    shared = types.TypeVar("Shared")
    method = _method_with_fields(shared, shared)

    module = method.dialects.encode(method, version="caller-version")
    first, second = _field_units(module)

    assert first.kind == "attribute"
    assert set(first.data) == {"data", "id"}
    assert first.data["data"].kind == "type-attribute"
    assert second.kind == "attr_ref"
    assert second.module_name == ""
    assert second.class_name == ""
    assert second.data == {"id": first.data["id"]}
    assert module.version == "caller-version"

    transported = _through_transport(module, transport)
    decoded = method.dialects.decode(transported)

    first_decoded, second_decoded = decoded.fields
    assert first_decoded is not shared
    assert first_decoded is second_decoded
    assert first_decoded == second_decoded
    assert isinstance(first_decoded, types.TypeVar)
    assert decoded(7) == 7


def test_unique_type_attribute_has_no_reference_metadata() -> None:
    unique = types.TypeVar("Unique")
    method = _method_with_fields(unique)

    module = method.dialects.encode(method)
    (field,) = _field_units(module)

    assert field.kind == "attribute"
    assert set(field.data) == {"data"}


def test_standalone_attribute_serialization_uses_a_reference() -> None:
    shared = types.TypeVar("Standalone")
    serializer = Serializer()

    first = serializer.serialize_attribute(shared)
    second = serializer.serialize_attribute(shared)

    assert first.kind == "attribute"
    assert set(first.data) == {"data", "id"}
    assert second.kind == "attr_ref"
    assert second.data == {"id": first.data["id"]}

    deserializer = Deserializer(passthrough.dialects)
    first_decoded = deserializer.deserialize(first)
    second_decoded = deserializer.deserialize(second)
    assert first_decoded is second_decoded
    assert first_decoded == second_decoded


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_distinct_but_equal_type_attributes_are_not_interned(
    transport: Transport,
) -> None:
    first = types.TypeVar("T")
    second = types.TypeVar("T")
    assert first is not second
    assert first == second
    method = _method_with_fields(first, second)

    module = method.dialects.encode(method)
    first_unit, second_unit = _field_units(module)

    assert first_unit.kind == second_unit.kind == "attribute"
    assert "id" not in first_unit.data
    assert "id" not in second_unit.data

    decoded = method.dialects.decode(_through_transport(module, transport))
    assert decoded.fields[0] is not first
    assert decoded.fields[1] is not second
    assert decoded.fields[0] is not decoded.fields[1]
    assert decoded.fields[0] == decoded.fields[1]


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_general_pyattr_is_not_memoized(transport: Transport) -> None:
    shared_type = types.TypeVar("PayloadType")
    wrapper = ir.PyAttr(7, pytype=shared_type)
    method = _method_with_fields(wrapper, wrapper)

    module = method.dialects.encode(method)
    first, second = _field_units(module)

    assert first.kind == second.kind == "attribute"
    assert set(first.data) == set(second.data) == {"data"}
    assert first.data["data"].kind == second.data["data"].kind == "pyattr"

    decoded = method.dialects.decode(_through_transport(module, transport))
    assert decoded.fields[0] is not decoded.fields[1]
    assert decoded.fields[0].type is decoded.fields[1].type
    assert decoded.fields[0].type == decoded.fields[1].type


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_forward_ref_from_reversed_pyattr_field_order(transport: Transport) -> None:
    shared = types.TypeVar("Reversed")
    wrapper = ir.PyAttr(shared, pytype=shared)
    method = _method_with_fields(wrapper)

    module = method.dialects.encode(method)
    (field,) = _field_units(module)
    pyattr = field.data["data"]
    definition = pyattr.data["data"]
    reference = pyattr.data["pytype"]

    assert definition.kind == "attribute"
    assert reference.kind == "attr_ref"
    assert reference.data["id"] == definition.data["id"]

    decoded = method.dialects.decode(_through_transport(module, transport))
    decoded_wrapper = decoded.fields[0]
    assert decoded_wrapper.type is decoded_wrapper.data
    assert decoded_wrapper.type == decoded_wrapper.data


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_nested_type_graph_preserves_identity_relationships(
    transport: Transport,
) -> None:
    leaf = types.TypeVar(f"Leaf-{transport}")
    first = types.Generic(tuple, leaf)
    second = types.Generic(list, leaf)
    assert first.vars[0] is second.vars[0]
    method = _method_with_fields(first, first, second, second)

    module = method.dialects.encode(method)
    decoded = method.dialects.decode(_through_transport(module, transport))
    decoded_first = decoded.fields[0]
    decoded_second = decoded.fields[2]

    assert decoded_first is not first
    assert decoded_second is not second
    assert decoded_first is decoded.fields[1]
    assert decoded_second is decoded.fields[3]
    assert decoded_first is not decoded_second
    assert decoded_first.vars[0] is decoded_second.vars[0]


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_union_and_literal_roundtrip_by_value(transport: Transport) -> None:
    literal = types.Literal(1)
    union = types.Union(literal, types.String)
    assert isinstance(union, types.Union)
    method = _method_with_fields(union)

    decoded = method.dialects.decode(
        _through_transport(method.dialects.encode(method), transport)
    )
    decoded_union = decoded.fields[0]
    assert isinstance(decoded_union, types.Union)
    assert decoded_union.is_structurally_equal(union)
    assert any(
        isinstance(attr, types.Literal) and attr.is_structurally_equal(literal)
        for attr in decoded_union.types
    )


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_literal_preserves_shared_type_identity(transport: Transport) -> None:
    leaf = types.TypeVar(f"LiteralLeaf-{transport}")
    literal = types.Literal(transport, leaf)
    union = types.Union(literal, types.String)
    generic = types.Generic(tuple, leaf)
    assert isinstance(union, types.Union)
    assert literal.type is generic.vars[0]
    method = _method_with_fields(union, generic)

    decoded = method.dialects.decode(
        _through_transport(method.dialects.encode(method), transport)
    )
    decoded_union = decoded.fields[0]
    decoded_generic = decoded.fields[1]
    assert isinstance(decoded_union, types.Union)
    decoded_literal = next(
        attr for attr in decoded_union.types if isinstance(attr, types.Literal)
    )
    assert decoded_literal is not literal
    assert decoded_literal.type is decoded_generic.vars[0]


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_nested_repeated_type_attributes_preserve_dag_topology(
    transport: Transport,
) -> None:
    depth = 8
    typ: types.TypeAttribute = types.TypeVar("Leaf")
    for _ in range(depth):
        typ = types.Generic(tuple, typ, types.Vararg(typ))

    method = _method_with_fields(typ)
    decoded = method.dialects.decode(
        _through_transport(method.dialects.encode(method), transport)
    )
    decoded_type = decoded.fields[0]

    for _ in range(depth):
        assert isinstance(decoded_type, types.Generic)
        assert decoded_type.vararg is not None
        nested_type = decoded_type.vars[0]
        assert nested_type is decoded_type.vararg.typ
        decoded_type = nested_type

    assert isinstance(decoded_type, types.TypeVar)


@pytest.mark.parametrize("transport_name", ["json", "cbson"])
def test_attr_ref_uses_short_compact_wire_shape(transport_name: str) -> None:
    shared = types.TypeVar("Compact")
    method = _method_with_fields(shared, shared)
    module = method.dialects.encode(method)

    if transport_name == "json":
        wire = json.loads(JSONSerializer().encode(module))
    else:
        payload = CompressedBSONSerializer().encode(module)
        wire = bson.decode(gzip.decompress(payload))

    compact_refs = [
        mapping["$u"]
        for mapping in _walk_wire_mappings(wire)
        if len(mapping) == 1 and "$u" in mapping and mapping["$u"][0] == "attr_ref"
    ]
    assert compact_refs
    assert all(len(payload) == 2 for payload in compact_refs)


def _walk_wire_mappings(value: Any) -> Iterator[dict[str, Any]]:
    if isinstance(value, dict):
        yield value
        for item in value.values():
            yield from _walk_wire_mappings(item)
    elif isinstance(value, list):
        for item in value:
            yield from _walk_wire_mappings(item)


def test_type_attribute_definitions_without_ids_still_decode() -> None:
    first = types.TypeVar("Legacy")
    second = types.TypeVar("Legacy")
    method = _method_with_fields(first, second)
    serializer = Serializer()
    body = serializer.serialize_method(method)
    module = SerializationModule(
        symbol_table=dict(serializer._ctx.Method_Symbol),
        body=body,
        version="legacy-version",
    )

    fields = _field_units(module)
    assert all(field.kind == "attribute" for field in fields)
    assert all(set(field.data) == {"data"} for field in fields)

    decoded = method.dialects.decode(module)
    assert decoded.fields[0] == decoded.fields[1]
    assert decoded(3) == 3


@pytest.mark.parametrize(
    ("data", "message"),
    [
        pytest.param({}, r"attr_ref is missing required 'id'", id="missing-id"),
        pytest.param(
            {"id": 17}, r"attr_ref id must be a string, got 17", id="non-string-id"
        ),
    ],
)
def test_malformed_attr_ref_ids_fail_contextually(
    data: dict[str, object], message: str
) -> None:
    unit = SerializationUnit("attr_ref", "", "", data)
    deserializer = Deserializer(passthrough.dialects)

    with pytest.raises(ValueError, match=message):
        deserializer.deserialize(unit)


def test_attr_ref_data_must_be_a_mapping() -> None:
    unit = SerializationUnit("attr_ref", "", "", [])  # type: ignore[arg-type]

    with pytest.raises(ValueError, match="attr_ref data must be a mapping"):
        Deserializer(passthrough.dialects).deserialize(unit)


def test_dangling_attr_ref_fails_contextually() -> None:
    shared = types.TypeVar("Dangling")
    method = _method_with_fields(shared, shared)
    module = method.dialects.encode(method)
    _, reference = _field_units(module)
    reference.data["id"] = "not-defined"

    with pytest.raises(ValueError, match=r"dangling attr_ref 'not-defined'"):
        method.dialects.decode(module)


def test_duplicate_type_attribute_definition_id_is_rejected() -> None:
    shared = types.TypeVar("Duplicate")
    method = _method_with_fields(shared, shared)
    module = method.dialects.encode(method)
    fields = _field_units(module)
    fields[1] = copy.deepcopy(fields[0])

    with pytest.raises(
        ValueError, match=r"duplicate TypeAttribute definition id 't[0-9]+'"
    ):
        method.dialects.decode(module)


def test_attr_ref_to_general_attribute_is_rejected() -> None:
    shared = types.TypeVar("WrongKind")
    method = _method_with_fields(shared, shared)
    module = method.dialects.encode(method)
    definition, reference = _field_units(module)
    attr_id = definition.data["id"]

    wrong_definition = Serializer().serialize_attribute(ir.PyAttr(1))
    wrong_definition.data["id"] = attr_id
    fields = _field_units(module)
    fields[:] = [reference, wrong_definition]

    with pytest.raises(
        ValueError,
        match=rf"TypeAttribute definition {attr_id!r} decoded as PyAttr",
    ):
        method.dialects.decode(module)
