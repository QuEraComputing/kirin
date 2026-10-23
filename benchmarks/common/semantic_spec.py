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


def coefficients(path, steps):
    return {var: multiplier["constant"] + steps * multiplier["per_step"]
            for var in ("x", "y") for multiplier in [path["expected"][f"{var}_multiplier"]]}


def path_value(path, steps, inputs):
    """Evaluate a closed form, not the builder's repeated arithmetic chain. Unread inputs may be None."""
    return sum(coefficient * inputs[var] for var, coefficient in coefficients(path, steps).items() if coefficient)


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
        expected = path_value(path, steps, sample)
        assert -(2 ** 63) <= expected < 2 ** 63
        cases.append({"input": sample, "path": path["name"], "expected": expected})
    assert covered == {path["name"] for path in family["paths"]}
    return cases
