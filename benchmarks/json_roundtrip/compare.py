"""Build, verify equivalence, and compare full IR -> JSON -> IR timings."""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import statistics
import subprocess
import sys

from validation import assert_equivalent

ROOT = pathlib.Path(__file__).resolve().parents[2]
BASELINE = "eae74008d4aec630169b5e64cc085e6f2c0d5b12"


def run(*args, cwd=ROOT):
    return subprocess.check_output([str(arg) for arg in args], cwd=cwd, text=True).strip()


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
    args = parser.parse_args()
    baseline = args.python_checkout.resolve()
    python = (args.python or baseline / ".venv/bin/python").absolute()
    assert run("git", "rev-parse", "HEAD", cwd=baseline) == BASELINE, "wrong Python revision"
    assert not run("git", "status", "--porcelain", cwd=baseline), "Python baseline must be clean"
    assert args.repeats > 0 and args.warmups >= 0
    assert all(size > 0 and size % 4 == 0 for size in args.sizes)
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
                print(f"Checking and measuring {implementation}: {family}, {size} adds", file=sys.stderr)
                pair[implementation] = json.loads(run(*commands[implementation], family, size,
                                                     args.repeats, args.warmups))
            assert_equivalent(pair["rust"], pair["python"])
            # Compare core CFG/SSA relationships after explicit Int/i64 mapping;
            # this omits wrapper and implementation-specific attribute metadata.
            shape = pair["rust"]["shape"]
            digest = hashlib.sha256(json.dumps(shape, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
            for data in pair.values():
                del data["shape"]
                data["median_seconds"] = statistics.median(data["samples_seconds"])
            result = {"family": family, "adds": size, "normalized_shape_sha256": digest,
                      "rust": pair["rust"], "python": pair["python"],
                      "python_over_rust_median_ratio": pair["python"]["median_seconds"] / pair["rust"]["median_seconds"]}
            results.append(result)
    hardware = {"logical_cpus": os.cpu_count(), "processor": platform.processor()}
    if platform.system() == "Darwin":
        for key in ("machdep.cpu.brand_string", "hw.model", "hw.memsize"):
            probe = subprocess.run(["sysctl", "-n", key], capture_output=True, text=True)
            hardware[key] = probe.stdout.strip() if probe.returncode == 0 else "unavailable"
    elif platform.system() == "Linux":
        cpuinfo = pathlib.Path("/proc/cpuinfo").read_text()
        hardware["cpu_model"] = next((line.split(":", 1)[1].strip() for line in cpuinfo.splitlines()
                                      if line.startswith("model name")), platform.processor())
    report = {
        "environment": {"platform": platform.platform(), "machine": platform.machine(),
                        "hardware": hardware, "python": run(python, "-VV"),
                        "rustc": run("rustc", "--version", "--verbose"),
                        "rust_revision": run("git", "rev-parse", "HEAD"),
                        "rust_working_tree": run("git", "status", "--porcelain"),
                        "python_revision": BASELINE},
        "timed_boundary": "complete IR -> JSON string -> complete usable IR; output destruction excluded",
        "results": results,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Verified matching normalized CFG/SSA structure for {len(results)} workload pairs. Results: {args.output}")


if __name__ == "__main__":
    main()
