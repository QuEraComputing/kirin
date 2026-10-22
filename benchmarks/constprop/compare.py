"""Check and time Rust vs Python constant propagation on the shared workloads."""
import argparse
import collections
import hashlib
import itertools
import json
import pathlib
import statistics
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "json_roundtrip"))

from provenance import ROOT, environment, inspect_python_source, run, verify_python_source
from semantic_spec import SPEC_PATH, coefficients, family_spec, load_suite, path_value

KEYS = ("x", "y", "outer", "inner")
FAMILIES = ["chain", "diamond", "nested_cfg", "loop", "calls", "calls_same", "recursion"]


def feasible(family, input):
    return [path for path in family["paths"] if all(input[key] in (None, value) for key, value in path["when"].items())]


def target(family, steps, input):
    """Flat constant propagation's answer; `None` is an unknown input.

    Feasible paths meet without loss. A loop (`trips > 0`) joins its carried value
    across iterations, so it stays constant only if no iteration changes it.
    """
    values = set()
    for path in feasible(family, input):
        read = [var for var, coefficient in coefficients(path, steps).items() if coefficient]
        if any(input[var] is None for var in read):
            return "top"
        if path.get("trips") and sum(path["expected"][f"{var}_multiplier"]["per_step"] * input[var] for var in read):
            return "top"
        values.add(path_value(path, steps, input))
    return values.pop() if len(values) == 1 else "top"


def sound(family, steps, input, result, bound):
    """Does `result` cover the oracle at every concrete point of the declared domain matching `input`?"""
    if result == "top":
        return True
    if type(result) is not int:
        return False
    domain = {"x": range(-bound, bound + 1), "y": range(-bound, bound + 1),
              "outer": (False, True), "inner": (False, True)}
    for values in itertools.product(*(domain[key] if input[key] is None else [input[key]] for key in KEYS)):
        point = dict(zip(KEYS, values))
        (path,) = feasible(family, point)
        if path_value(path, steps, point) != result:
            return False
    return True


def judge(family, steps, input, results, bound):
    expected = target(family, steps, input)
    row = {"input": input, "target": expected}
    for implementation, result in results.items():
        verdict = ("error" if result == "error"
                   else "unsound" if not sound(family, steps, input, result, bound)
                   else "pass" if result == expected
                   else "more_precise" if expected == "top" else "imprecise")
        row[implementation] = {"result": result, "verdict": verdict}
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-checkout", type=pathlib.Path, required=True)
    parser.add_argument("--python", type=pathlib.Path)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1000, 10000, 100000])
    parser.add_argument("--families", nargs="+", choices=FAMILIES, default=FAMILIES)
    parser.add_argument("--repeats", type=int, default=30)
    parser.add_argument("--warmups", type=int, default=5)
    parser.add_argument("--python-cache-calls", action="store_true", help="run const.Propagate with cache_calls=True")
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    checkout = args.python_checkout.resolve()
    python = (args.python or checkout / ".venv/bin/python").absolute()
    assert not run("git", "status", "--porcelain", cwd=checkout), "Python checkout must be clean"
    python_provenance = inspect_python_source(python, checkout, cwd=ROOT)
    assert args.repeats > 0 and args.warmups >= 0
    suite = load_suite()
    for name in args.families:
        for size in args.sizes:
            family_spec(name, size, suite)
    subprocess.run(["cargo", "build", "--release", "--example", "constprop", "--features", "serde",
                    "--target-dir", str(ROOT / "target")], cwd=ROOT, check=True)
    commands = {"rust": [ROOT / "target/release/examples/constprop"],
                "python": [python, pathlib.Path(__file__).with_name("bench.py")]}
    checks = list(dict.fromkeys(tuple(sample[key] if known else None for key, known in zip(KEYS, mask))
                                for sample in suite["samples"]
                                for mask in itertools.product((True, False), repeat=4)))
    sample = suite["samples"][0]
    timed = {"all_known": sample, "unknown_conditions": dict(sample, outer=None, inner=None),
             "all_unknown": dict.fromkeys(KEYS)}
    verdicts = {implementation: collections.Counter() for implementation in commands}
    results = []
    for size in args.sizes:
        for name in args.families:
            family = family_spec(name, size, suite)
            steps = size // family["steps_divisor"]
            request = {"family": name, "size": size, "warmups": args.warmups, "repeats": args.repeats,
                       "cache_calls": args.python_cache_calls,
                       "check": [dict(zip(KEYS, values)) for values in checks], "time": list(timed.values())}
            output = {}
            order = ("rust", "python") if len(results) % 2 == 0 else ("python", "rust")
            for implementation in order:
                print(f"Checking and measuring {implementation}: {name}, size {size}", file=sys.stderr)
                output[implementation] = json.loads(subprocess.run(
                    [str(arg) for arg in commands[implementation]], input=json.dumps(request),
                    capture_output=True, text=True, check=True, cwd=ROOT).stdout)
            verify_python_source(checkout, output["python"]["kirin_source"])
            bound = suite["max_abs_input"]
            check_rows = [judge(family, steps, input, {impl: output[impl]["check"][i] for impl in commands}, bound)
                          for i, input in enumerate(request["check"])]
            time_rows = []
            for i, (config, input) in enumerate(timed.items()):
                row = judge(family, steps, input, {impl: output[impl]["time"][i]["result"] for impl in commands}, bound)
                for implementation in commands:
                    samples = output[implementation]["time"][i]["samples_seconds"]
                    row[implementation].update(samples_seconds=samples, median_seconds=statistics.median(samples))
                row["python_over_rust_median_ratio"] = row["python"]["median_seconds"] / row["rust"]["median_seconds"]
                time_rows.append({"config": config, **row})
                print(f"  {config}: rust {row['rust']['median_seconds'] * 1e3:.3f} ms, "
                      f"python {row['python']['median_seconds'] * 1e3:.3f} ms "
                      f"({row['python_over_rust_median_ratio']:.0f}x)", file=sys.stderr)
            for row in check_rows:
                for implementation in commands:
                    verdicts[implementation][row[implementation]["verdict"]] += 1
            results.append({"family": name, "size": size, "checks": check_rows, "timings": time_rows})
    report = {
        "environment": {**environment(python), "python_revision": run("git", "rev-parse", "HEAD", cwd=checkout),
                        "python_provenance": python_provenance},
        "workload_spec_sha256": hashlib.sha256(SPEC_PATH.read_bytes()).hexdigest(),
        "timed_boundary": "analyzer construction + analysis of the built IR; analyzer/result destruction excluded",
        "python_cache_calls": args.python_cache_calls,
        "verdicts": verdicts,
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    for implementation, counts in verdicts.items():
        print(f"{implementation}: " + ", ".join(f"{count} {verdict}" for verdict, count in sorted(counts.items())))
    print(f"Results: {args.output}")


if __name__ == "__main__":
    main()
