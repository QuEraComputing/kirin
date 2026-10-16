"""Check the source actually imported by the selected Python interpreter."""
import json
from pathlib import Path
import subprocess


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
