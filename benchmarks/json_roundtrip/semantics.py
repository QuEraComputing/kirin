"""Validate original and decoded serialization payloads against the shared oracle."""
from common.semantic_spec import expected_cases


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
