"""Check and time Rust vs Python rewrites on matched IR."""
import argparse
import collections
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

from common.provenance import ROOT, environment, inspect_python_source, run, verify_python_source
from common.semantic_spec import SPEC_PATH, family_spec, load_suite, path_value

# (task, shape): the edit, and the IR it runs on.
CASES = [
    ("peephole", "neg_chain"),
    ("no_match", "chain"), ("no_match", "diamond"), ("no_match", "nested_cfg"),
    ("dce", "dead_independent"), ("dce", "dead_chain"),
    *(("backward_cascade", f"folds_{folds}_match_{cost}")
      for folds in (1, 2, 3, 4, 8) for cost in (0, 8, 32)),
    ("cascade", "unroll_3"), ("cascade", "unroll_8"),
    ("bypass", "chain"), ("insert", "chain"), ("move", "chain"), ("erase", "dead_independent"),
]
TASKS = list(dict.fromkeys(task for task, _ in CASES))
SHARED = ("chain", "diamond", "nested_cfg")


def expected(task, shape, size, before, sample, suite):
    """The statement count and value the rewritten `@benchmark` must have."""
    if task in ("no_match", "insert", "move"):
        family = family_spec(shape, size, suite)
        (path,) = [path for path in family["paths"]
                   if all(sample[key] == value for key, value in path["when"].items())]
        value = path_value(path, size // family["steps_divisor"], sample)
        return {"no_match": before, "move": before, "insert": before + size}[task], value
    if task == "backward_cascade":
        return size + 1, sample["x"] + size * sample["y"]
    if task == "cascade":
        # Each `v - k` becomes `constant 1`, `v - 1`, then k - 1 pairs of `constant 1`, `- 1`.
        k, n = int(shape[len("unroll_"):]), size // 2
        return 2 * k * n + 1, sample["x"] - k * n
    # Every other edit leaves only `ret %x`.
    return 1, sample["x"]


def judge(task, shape, size, check, samples, suite, before):
    failures = []
    if check["statements_before"] != before:
        failures.append(f"statements_before {check['statements_before']} != {before}")
    if task == "backward_cascade":
        folds = int(shape.split("_")[1])
        if before != size + 2 * folds + 1:
            failures.append("incorrect addition prefix or negation tail length")
        summary = check["summary"]
        if "applied" in summary and (summary["applied"], summary["erased"]) != (folds, 2 * folds):
            failures.append("incorrect fold or erasure count")
    assert len(check["values"]) == len(samples)
    for sample, value in zip(samples, check["values"]):
        count, target = expected(task, shape, size, before, sample, suite)
        if value != target:
            failures.append(f"value {value} != {target} at {sample}")
    if check["statements_after"] != count:
        failures.append(f"statements_after {check['statements_after']} != {count}")
    if check["summary"].get("converged") is False:
        failures.append("did not converge")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-checkout", type=pathlib.Path, required=True)
    parser.add_argument("--python", type=pathlib.Path)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1000, 4000])
    parser.add_argument("--tasks", nargs="+", choices=TASKS, default=TASKS)
    parser.add_argument("--shapes", nargs="+")
    parser.add_argument("--repeats", type=int, default=10)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    checkout = args.python_checkout.resolve()
    python = (args.python or checkout / ".venv/bin/python").absolute()
    assert not run("git", "status", "--porcelain", cwd=checkout), "Python checkout must be clean"
    python_provenance = inspect_python_source(python, checkout, cwd=ROOT)
    assert args.repeats > 0 and args.warmups >= 0
    suite = load_suite()
    for size in args.sizes:
        family_spec("chain", size, suite)  # size checks shared by every shape
    subprocess.run(["cargo", "build", "--release", "--example", "rewrite", "--features", "serde",
                    "--target-dir", str(ROOT / "target")], cwd=ROOT, check=True)
    commands = {"rust": [ROOT / "target/release/examples/rewrite"],
                "python": [python, pathlib.Path(__file__).with_name("bench.py")]}
    samples = suite["samples"]
    failures = collections.Counter()
    results = []
    for size in args.sizes:
        for task, shape in [case for case in CASES if case[0] in args.tasks
                            and (args.shapes is None or case[1] in args.shapes)]:
            request = {"task": task, "shape": shape, "size": size, "warmups": args.warmups,
                       "repeats": args.repeats, "samples": samples}
            output = {}
            order = ("rust", "python") if len(results) % 2 == 0 else ("python", "rust")
            for implementation in order:
                print(f"Checking and measuring {implementation}: {task} on {shape}, size {size}", file=sys.stderr)
                proc = subprocess.run(
                    [str(arg) for arg in commands[implementation]], input=json.dumps(request),
                    capture_output=True, text=True, cwd=ROOT)
                if proc.returncode and implementation == "rust":
                    failures["rust"] += 1
                    output["rust"] = {}
                    output["rust_error"] = proc.stderr.strip()
                    print(output["rust_error"], file=sys.stderr)
                    continue
                proc.check_returncode()
                output[implementation] = json.loads(proc.stdout)
            verify_python_source(checkout, output["python"]["kirin_source"])
            variants = {**{f"rust_{name}": v for name, v in output["rust"].items()},
                        "python": output["python"]["variants"]["python"]}
            before = variants["python"]["check"]["statements_before"]
            python_median = statistics.median(variants["python"]["timing"]["pass_seconds"])
            rows = {}
            for name, variant in variants.items():
                problems = judge(task, shape, size, variant["check"], samples, suite, before)
                failures[name] += bool(problems)
                timing = {key: {"samples": values, "median": statistics.median(values)}
                          for key, values in variant["timing"].items()}
                rows[name] = {"check": variant["check"], "failures": problems, "seconds": timing}
                if name != "python":
                    rows[name]["python_over_rust"] = {
                        key: python_median / entry["median"] for key, entry in timing.items()}
            line = ", ".join(f"{name} {row['seconds']['pass_seconds']['median'] * 1e3:.3f} ms"
                             + (f" ({row['python_over_rust']['pass_seconds']:.0f}x)" if name != "python" else "")
                             + (" FAILED" if row["failures"] else "")
                             for name, row in rows.items())
            print(f"  {line}", file=sys.stderr)
            case = {"task": task, "shape": shape, "size": size, "variants": rows}
            if "rust_error" in output:
                case["rust_error"] = output["rust_error"]
            results.append(case)
    report = {
        "environment": {**environment(python), "python_revision": run("git", "rev-parse", "HEAD", cwd=checkout),
                        "python_provenance": python_provenance},
        "workload_spec_sha256": hashlib.sha256(SPEC_PATH.read_bytes()).hexdigest(),
        "timed_boundary": {
            "rust": "pass_seconds: the edit inside Program::run_pass; run_pass_seconds: all of run_pass, "
                    "adding the stage take/restore and verify_derived. Rule-set build, parse and drop excluded.",
            "python": "pass_seconds: the edit on the built Method, rewrite-rule construction included. "
                      "IR build and drop excluded; GC disabled inside the timer.",
        },
        "failures": failures,
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print("failures: " + (", ".join(f"{name} {count}" for name, count in failures.items() if count) or "none"))
    print(f"Results: {args.output}")
    if any(failures.values()):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
