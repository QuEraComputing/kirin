"""Check the source actually imported by the selected Python interpreter."""
import json
import os
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[2]


def run(*args, cwd=ROOT):
    return subprocess.check_output([str(arg) for arg in args], cwd=cwd, text=True).strip()


def environment(python):
    """Host, toolchains and Rust revision recorded with every report."""
    hardware = {"logical_cpus": os.cpu_count(), "processor": platform.processor()}
    if platform.system() == "Darwin":
        for key in ("machdep.cpu.brand_string", "hw.model", "hw.memsize"):
            probe = subprocess.run(["sysctl", "-n", key], capture_output=True, text=True)
            hardware[key] = probe.stdout.strip() if probe.returncode == 0 else "unavailable"
    elif platform.system() == "Linux":
        cpuinfo = Path("/proc/cpuinfo").read_text()
        hardware["cpu_model"] = next((line.split(":", 1)[1].strip() for line in cpuinfo.splitlines()
                                      if line.startswith("model name")), platform.processor())
    return {"platform": platform.platform(), "machine": platform.machine(),
            "hardware": hardware, "python": run(python, "-VV"),
            "rustc": run("rustc", "--version", "--verbose"),
            "rust_revision": run("git", "rev-parse", "HEAD"),
            "rust_working_tree": run("git", "status", "--porcelain")}


def verify_python_source(checkout, source):
    expected = (Path(checkout) / "src/kirin/__init__.py").resolve(strict=True)
    actual = Path(source).resolve(strict=True)
    if actual != expected:
        raise RuntimeError(
            f"Python imported Kirin from {actual}; expected pinned source {expected}. "
            "Check --python, PYTHONPATH, and the interpreter's editable install."
        )
    return str(actual)


def inspect_python_source(python, checkout, *, cwd=None):
    probe = (
        "import json, pathlib, sys, kirin; "
        "print(json.dumps({'kirin_source': str(pathlib.Path(kirin.__file__).resolve()), "
        "'executable': sys.executable}))"
    )
    result = json.loads(subprocess.check_output(
        [str(python), "-c", probe], cwd=cwd, text=True,
    ))
    verify_python_source(checkout, result["kirin_source"])
    return result
