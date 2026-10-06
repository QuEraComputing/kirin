import os
import re
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
print(json.dumps({
    "seconds": time.perf_counter() - start,
    "main": kernels.main(1.0),
    "lines": [s.source.lineno + s.source.lineno_begin
              for s in kernels.helper.code.walk() if s.source is not None],
}))
"""


def run(tmp_path, offset, factor, leading_lines=0):
    """Run the kernels in a new process; return the kernels it saved and its output."""
    (tmp_path / "kernels.py").write_text(
        "\n" * leading_lines + KERNELS.format(offset=offset, factor=factor)
    )
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


def test_compile_cache_relocation(tmp_path):
    written, cold = run(tmp_path, offset=5.0, factor=2.0)
    assert written == {"helper", "main"}
    assert cold["lines"]

    written, moved = run(tmp_path, offset=5.0, factor=2.0, leading_lines=10)
    assert written == set()
    assert moved["main"] == cold["main"] == 7.0
    assert moved["lines"] == [line + 10 for line in cold["lines"]]


# an error in a kernel, reported by Kirin's stack trace since it is not caught
RAISE = """\
import kernels
kernels.helper("x")
"""


def test_compile_cache_relocated_error(tmp_path):
    shift = 10
    run(tmp_path, offset=5.0, factor=2.0)
    lines = (tmp_path / "kernels.py").read_text().splitlines()
    before = next(i for i, text in enumerate(lines, 1) if "return x *" in text)

    # moved down `shift` lines: loaded from the cache, not recompiled
    written, _ = run(tmp_path, offset=5.0, factor=2.0, leading_lines=shift)
    assert written == set()

    env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1", KIRIN_PYTHON_STACKTRACE="0")
    env[ENV_VAR] = str(tmp_path / "cache")
    out = subprocess.run(
        [sys.executable, "-c", RAISE],
        cwd=tmp_path,
        env=env,
        capture_output=True,
        text=True,
    )
    reported = re.search(r'kernels\.py", line (\d+),', out.stderr)
    assert reported is not None, out.stderr
    assert int(reported.group(1)) == before + shift
