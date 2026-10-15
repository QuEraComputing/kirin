"""Reject a valid but unrelated Kirin source before measuring it."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from provenance import inspect_python_source, verify_python_source


class PythonProvenance(unittest.TestCase):
    def test_interpreter_imports_expected_checkout(self):
        with tempfile.TemporaryDirectory() as directory:
            checkout = Path(directory) / "baseline"
            source = checkout / "src/kirin/__init__.py"
            source.parent.mkdir(parents=True)
            source.write_text("# Minimal source for an isolated provenance probe.\n")
            with patch.dict(os.environ, {"PYTHONPATH": str(checkout / "src")}):
                result = inspect_python_source(sys.executable, checkout, cwd=directory)
            self.assertEqual(result["kirin_source"], str(source.resolve()))

    def test_pythonpath_override_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            checkouts = [Path(directory) / name for name in ("baseline", "other")]
            for checkout in checkouts:
                source = checkout / "src/kirin/__init__.py"
                source.parent.mkdir(parents=True)
                source.write_text("# Valid import from a different source checkout.\n")
            with patch.dict(os.environ, {"PYTHONPATH": str(checkouts[1] / "src")}):
                with self.assertRaisesRegex(RuntimeError, "expected pinned source"):
                    inspect_python_source(sys.executable, checkouts[0], cwd=directory)

    def test_measuring_subprocess_source_is_rechecked(self):
        with tempfile.TemporaryDirectory() as directory:
            baseline = Path(directory) / "baseline"
            expected = baseline / "src/kirin/__init__.py"
            expected.parent.mkdir(parents=True)
            expected.touch()
            wrong = Path(directory) / "other.py"
            wrong.touch()
            with self.assertRaisesRegex(RuntimeError, "expected pinned source"):
                verify_python_source(baseline, wrong)


if __name__ == "__main__":
    unittest.main()
