"""Serialized output must not depend on Python's per-process hash seed."""

import os
import sys
import subprocess
from pathlib import Path

from kirin.prelude import basic
from kirin.serialization import JSONSerializer
from kirin.serialization.base.serializer import Serializer
from kirin.serialization.base.deserializer import Deserializer

_ENCODE_SCRIPT = """
from kirin.prelude import basic
from kirin.serialization import JSONSerializer


@basic
def add_one(x: int):
    return x + 1


print(JSONSerializer().encode(add_one.dialects.encode(add_one)), end="")
"""


def _encode_with_hash_seed(script: Path, seed: int) -> str:
    env = {**os.environ, "PYTHONHASHSEED": str(seed)}
    return subprocess.run(
        [sys.executable, str(script)],
        env=env,
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def test_payload_is_identical_across_hash_seeds(tmp_path: Path):
    # NOTE: run from a file rather than `python -c`: before Python 3.13,
    # `inspect.getsource` cannot read `-c` code, which lowering relies on.
    script = tmp_path / "encode.py"
    script.write_text(_ENCODE_SCRIPT)
    payloads = {seed: _encode_with_hash_seed(script, seed) for seed in (1, 2, 3)}
    assert payloads[1]
    assert payloads[1] == payloads[2] == payloads[3]


@basic
def add_one(x: int):
    return x + 1


def test_dialect_group_is_serialized_sorted_by_name():
    unit = Serializer().serialize_dialect_group(basic)
    names = [d.data["name"] for d in unit.data["data"].data["value"]]
    assert names == sorted(dialect.name for dialect in basic.data)


def test_sorted_dialect_group_round_trips():
    codec = JSONSerializer()
    module = codec.decode(codec.encode(basic.encode(add_one)))
    decoded = Deserializer(dialect_group=basic).decode(module)
    assert decoded.dialects.data == add_one.dialects.data
    assert decoded.callable_region.is_structurally_equal(add_one.callable_region)
