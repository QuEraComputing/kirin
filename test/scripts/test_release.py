import runpy
import shutil
import subprocess
from pathlib import Path
from unittest.mock import patch

import pytest


@pytest.mark.parametrize(
    "mode, failure",
    [
        ("patch", None),
        ("current", None),
        ("patch", "lock"),
        ("patch", "commit"),
        ("patch", "staged"),
    ],
)
def test_release_keeps_version_and_lock_together(tmp_path, mode, failure):
    root = Path(__file__).resolve().parents[2]
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    script = scripts / "release.py"
    shutil.copyfile(root / "scripts" / "release.py", script)
    project = tmp_path / "pyproject.toml"
    lock = tmp_path / "uv.lock"
    original_project = '[project]\nname = "example"\nversion = "0.22.21"\n'
    original_lock = 'version = "0.22.20"\n'
    project.write_text(original_project)
    lock.write_text(original_lock)
    calls = []

    def run(command, **kwargs):
        calls.append(command)
        status = 0
        if command[:3] == ["gh", "release", "view"]:
            status = 1
        elif command == ["git", "diff", "--cached", "--quiet"]:
            status = int(failure == "staged")
        elif command == ["uv", "lock"]:
            assert kwargs["cwd"] == str(tmp_path)
            lock.write_text(project.read_text())
            status = int(failure == "lock")
        elif command == ["git", "diff", "--quiet", "--", "uv.lock"]:
            status = 1
        elif command[:2] == ["git", "commit"]:
            assert ["uv", "lock"] in calls
            assert ["git", "add", "pyproject.toml", "uv.lock"] in calls
            version = "0.22.22" if mode == "patch" else "0.22.21"
            assert version in project.read_text()
            assert version in lock.read_text()
            status = int(failure == "commit")
        return subprocess.CompletedProcess(command, status)

    with (
        patch("subprocess.run", side_effect=run),
        patch("sys.argv", [str(script), mode]),
    ):
        if failure:
            with pytest.raises(SystemExit) as exc:
                runpy.run_path(str(script), run_name="__main__")
            assert exc.value.code == 1
        else:
            runpy.run_path(str(script), run_name="__main__")

    if failure:
        assert project.read_text() == original_project
        assert lock.read_text() == original_lock
        assert ["git", "push"] not in calls
        assert ["uv", "build"] not in calls
    else:
        assert sum(command[:2] == ["git", "commit"] for command in calls) == 1
        assert ["git", "push"] in calls
