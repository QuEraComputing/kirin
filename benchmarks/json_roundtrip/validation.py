"""Validate the benchmark CFG subset and compare explicitly normalized invariants."""


def validate_shape(blocks):
    assert blocks, "missing entry block"
    definitions = {}
    predecessors = [set() for _ in blocks]
    for block_id, block in enumerate(blocks):
        assert block["ops"], "empty unterminated block"
        for value in block["arguments"]:
            assert value["type"] in ("int", "bool"), "invalid block argument type"
            assert value["id"] not in definitions, "duplicate SSA definition"
            definitions[value["id"]] = (block_id, -1, value["type"])
        for index, op in enumerate(block["ops"]):
            assert not op["bodies"], "benchmark uses CFGs, not structured regions"
            assert op["kind"] in ("add", "ret", "br", "cond_br")
            assert (op["kind"] == "add") == (index < len(block["ops"]) - 1), "invalid terminator placement"
            for value in op["outputs"]:
                assert value["type"] == "int", "noninteger result"
                assert value["id"] not in definitions, "duplicate SSA definition"
                definitions[value["id"]] = (block_id, index, value["type"])
            for target in op["targets"]:
                assert 0 <= target < len(blocks), "missing target block"
                predecessors[target].add(block_id)
            target_arity = sum(len(blocks[target]["arguments"]) for target in op["targets"])
            if op["kind"] == "add":
                assert len(op["inputs"]) == 2 and len(op["outputs"]) == 1 and not op["targets"]
            else:
                assert not op["outputs"]
                if op["kind"] == "ret":
                    assert len(op["inputs"]) == 1 and not op["targets"]
                elif op["kind"] == "br":
                    assert len(op["targets"]) == 1 and len(op["inputs"]) == target_arity, "branch argument arity"
                else:
                    assert len(op["targets"]) == 2 and len(op["inputs"]) == target_arity + 1, "conditional branch argument arity"
    assert not predecessors[0], "benchmark entry has a predecessor"
    assert all(predecessors[i] for i in range(1, len(blocks))), "unreachable benchmark block"
    dominators = [{0}] + [set(range(len(blocks))) for _ in blocks[1:]]
    changed = True
    while changed:
        changed = False
        for block_id in range(1, len(blocks)):
            updated = {block_id} | set.intersection(*(dominators[p] for p in predecessors[block_id]))
            if updated != dominators[block_id]:
                dominators[block_id] = updated
                changed = True
    for block_id, block in enumerate(blocks):
        for index, op in enumerate(block["ops"]):
            for value in op["inputs"]:
                assert value in definitions, "undefined SSA operand"
                owner, position, _ = definitions[value]
                assert owner in dominators[block_id], "SSA definition does not dominate use"
                assert owner != block_id or position < index, "SSA use precedes definition"

    for block in blocks:
        for op in block["ops"]:
            input_types = [definitions[value][2] for value in op["inputs"]]
            if op["kind"] == "cond_br":
                assert input_types[0] == "bool", "nonboolean branch condition"
                carried_types = input_types[1:]
            else:
                carried_types = input_types
            assert all(tag == "int" for tag in carried_types), "noninteger arithmetic or carried value"


def assert_equivalent(rust, python):
    validate_shape(rust["shape"])
    validate_shape(python["shape"])
    for field in ("family", "adds", "operations", "blocks", "shape"):
        assert rust[field] == python[field], f"normalized workloads differ: {field}"

