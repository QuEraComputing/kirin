"""Build, check representative semantics, and compare full IR -> JSON -> IR timings."""
import argparse
import hashlib
import json
import pathlib
import statistics
import subprocess
import sys

from provenance import ROOT, environment, inspect_python_source, run, verify_python_source
from semantic_spec import SPEC_PATH, assert_semantically_valid, family_spec, load_suite
from validation import validate_shape

BASELINE = "eae74008d4aec630169b5e64cc085e6f2c0d5b12"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python-checkout", type=pathlib.Path, required=True)
    parser.add_argument("--python", type=pathlib.Path)
    parser.add_argument("--sizes", type=int, nargs="+", default=[1000, 10000, 100000])
    parser.add_argument("--families", nargs="+", choices=["chain", "diamond", "nested_cfg"],
                        default=["chain", "diamond", "nested_cfg"])
    parser.add_argument("--repeats", type=int, default=30)
    parser.add_argument("--warmups", type=int, default=5)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    parser.add_argument("--export-payloads", type=pathlib.Path, help="optionally save inspectable IR text/JSON outside timing")
    args = parser.parse_args()
    baseline = args.python_checkout.resolve()
    python = (args.python or baseline / ".venv/bin/python").absolute()
    assert run("git", "rev-parse", "HEAD", cwd=baseline) == BASELINE, "wrong Python revision"
    assert not run("git", "status", "--porcelain", cwd=baseline), "Python baseline must be clean"
    python_provenance = inspect_python_source(python, baseline, cwd=ROOT)
    assert args.repeats > 0 and args.warmups >= 0
    suite = load_suite()
    for family in args.families:
        for size in args.sizes:
            family_spec(family, size, suite)
    subprocess.run(["cargo", "build", "--release", "--example", "json_roundtrip", "--features", "serde", "--target-dir", str(ROOT / "target")],
                   cwd=ROOT, check=True)
    rust = ROOT / "target/release/examples/json_roundtrip"
    results = []
    for size in args.sizes:
        for family in args.families:
            commands = {
                "rust": [rust],
                "python": [python, ROOT / "benchmarks/json_roundtrip/python_benchmark.py"],
            }
            pair = {}
            order = ("rust", "python") if len(results) % 2 == 0 else ("python", "rust")
            for implementation in order:
                print(f"Checking and measuring {implementation}: {family}, {size} arithmetic operations", file=sys.stderr)
                export = [args.export_payloads.absolute()] if args.export_payloads else []
                pair[implementation] = json.loads(run(*commands[implementation], family, size,
                                                     args.repeats, args.warmups, *export))
            verify_python_source(baseline, pair["python"]["kirin_source"])
            assert_semantically_valid(pair["rust"], pair["python"], family, size)
            for data in pair.values():
                validate_shape(data["shape"])  # local validity, not cross-version equality
                # Shape/size/operation counts are diagnostics, never a cross-version gate.
                data["shape_sha256"] = hashlib.sha256(json.dumps(data.pop("shape"), sort_keys=True,
                                                       separators=(",", ":")).encode()).hexdigest()
                data["median_seconds"] = statistics.median(data["samples_seconds"])
            result = {"family": family, "size": size, "semantic_case_count": len(pair["rust"]["semantic_cases"]),
                      "rust": pair["rust"], "python": pair["python"],
                      "python_over_rust_median_ratio": pair["python"]["median_seconds"] / pair["rust"]["median_seconds"]}
            results.append(result)
    report = {
        "environment": {**environment(python), "python_revision": BASELINE,
                        "python_provenance": python_provenance},
        "workload_spec_sha256": hashlib.sha256(SPEC_PATH.read_bytes()).hexdigest(),
        "validation_scope": "representative bounded inputs executed before and after JSON against closed-form oracle; not a universal proof",
        "timed_boundary": "complete IR -> JSON string -> complete usable IR; output destruction excluded",
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Validated representative semantic cases for {len(results)} workload pairs. Results: {args.output}")


if __name__ == "__main__":
    main()
