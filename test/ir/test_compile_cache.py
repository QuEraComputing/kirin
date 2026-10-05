import os
import sys
import json
import subprocess

# written out rather than imported from `kirin.ir.compile_cache`: importing kirin
# compiles its own kernels, and would save them to the caller's real cache
ENV_VAR = "KIRIN_COMPILE_CACHE_DIR"

KERNELS = """\
from kirin.prelude import basic

OFFSET = {offset}


@basic
def helper(x: float) -> float:
    return x * {factor}


@basic(typeinfer=True)
def main(x: float) -> float:
    return helper(x) + OFFSET
"""

# one run of a script that imports the kernels, in a fresh process
RUN = """\
import json, time
import kirin.prelude
start = time.perf_counter()
import kernels
print(json.dumps({"seconds": time.perf_counter() - start, "main": kernels.main(1.0)}))
"""


def run(tmp_path, offset, factor):
    """Run the kernels in a new process; return the kernels it saved and its output."""
    (tmp_path / "kernels.py").write_text(KERNELS.format(offset=offset, factor=factor))
    cache = tmp_path / "cache"

    def saved():
        return {p.name: p.stat().st_mtime_ns for p in cache.glob("*.pickle")}

    before = saved()
    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
    env[ENV_VAR] = str(cache)
    out = subprocess.run(
        [sys.executable, "-c", RUN],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    written = {
        name.split("-")[0] for name, t in saved().items() if before.get(name) != t
    }
    return written & {"helper", "main"}, json.loads(out.stdout)


def test_compile_cache(tmp_path):
    # first run: nothing saved yet, so both kernels compile and are saved
    written, cold = run(tmp_path, offset=5.0, factor=2.0)
    assert written == {"helper", "main"}
    assert cold["main"] == 7.0

    # nothing changed: both kernels load, nothing is saved
    written, warm = run(tmp_path, offset=5.0, factor=2.0)
    assert written == set()
    assert warm["main"] == 7.0

    # helper changed: helper recompiles, and main too because it calls helper
    written, result = run(tmp_path, offset=5.0, factor=10.0)
    assert written == {"helper", "main"}
    assert result["main"] == 15.0

    # OFFSET changed: only main reads it, so only main recompiles
    written, result = run(tmp_path, offset=7.0, factor=10.0)
    assert written == {"main"}
    assert result["main"] == 17.0

    print(
        f"importing the kernels: {cold['seconds'] * 1e3:.2f} ms compiling, "
        f"{warm['seconds'] * 1e3:.2f} ms loading from the cache"
    )
