"""Common declarative inputs and independent closed-form expected results."""
import json
from pathlib import Path

SPEC_PATH = Path(__file__).with_name("workloads.json")


def load_suite():
    return json.loads(SPEC_PATH.read_text())


def family_spec(name, size, suite=None):
    suite = suite or load_suite()
    assert 0 < size <= suite["max_size"] and size % 4 == 0
    return next(family for family in suite["families"] if family["name"] == name)


def expected_cases(name, size, suite=None):
    suite = suite or load_suite()
    family = family_spec(name, size, suite)
    steps = size // family["steps_divisor"]
    cases = []
    covered = set()
    for sample in suite["samples"]:
        assert abs(sample["x"]) <= suite["max_abs_input"] and abs(sample["y"]) <= suite["max_abs_input"]
        paths = [path for path in family["paths"] if all(sample[key] == value for key, value in path["when"].items())]
        assert len(paths) == 1
        path = paths[0]
        covered.add(path["name"])
        # Evaluate a closed form, not the builder's repeated arithmetic chain.
        expected = sum(sample[var] * (path["expected"][f"{var}_multiplier"]["constant"] +
                       steps * path["expected"][f"{var}_multiplier"]["per_step"]) for var in ("x", "y"))
        assert -(2 ** 63) <= expected < 2 ** 63
        cases.append({"input": sample, "path": path["name"], "expected": expected})
    assert covered == {path["name"] for path in family["paths"]}
    return cases


def validate_runtime(program, restored, name, size):
    """Execute original and decoded payloads with Python's real interpreter."""
    from kirin.interp.concrete import Interpreter
    outcomes = []
    for case in expected_cases(name, size):
        sample = case["input"]
        args = [sample["x"], sample["y"], sample["outer"], sample["inner"]]
        original = Interpreter(program.dialects).run(program, *args)[1]
        decoded = Interpreter(restored.dialects).run(restored, *args)[1]
        assert type(original) is int and type(decoded) is int
        assert original == case["expected"], f"Python original differs from oracle: {case}"
        assert decoded == case["expected"], f"Python decoded differs from oracle: {case}"
        outcomes.append(dict(case, original=original, decoded=decoded))
    return outcomes


def assert_semantically_valid(rust, python, name, size):
    """Accept representation differences; compare executions to the common oracle."""
    expected = expected_cases(name, size)
    for payload in (rust, python):
        assert payload["family"] == name and payload["size"] == size
        assert len(payload["semantic_cases"]) == len(expected)
        for actual, oracle in zip(payload["semantic_cases"], expected):
            for field in ("input", "path", "expected"):
                assert actual[field] == oracle[field], f"semantic test specification differs: {field}"
            assert actual["original"] == oracle["expected"], "original runtime result differs"
            assert actual["decoded"] == oracle["expected"], "decoded runtime result differs"
