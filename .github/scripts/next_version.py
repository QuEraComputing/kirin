#!/usr/bin/env python3
"""Compute and write the next patch version for a release line.

Used by .github/workflows/release_tag.yml.
Kept as a script rather than inline YAML because the version arithmetic is the
part of the release pipeline that can ship a wrong version rather than failing
loudly. Only pyproject.toml is written; the workflow re-locks uv.lock after.

Standard library only, and deliberately no TOML parser: tomllib is 3.11+ while
this project supports 3.10, and pulling in tomlkit for a dozen lines of text
handling is not worth a dependency.
"""

from __future__ import annotations

import re
import sys
import argparse
import subprocess

# --- Repo conventions -------------------------------------------------------
# The three knobs another repository would change. Kept as constants rather than
# CLI flags: this script is copied per repo, so a flag would only mean the
# workflow and the script must agree about a regex passed through YAML quoting.
# The label name is NOT here -- it lives in vars.RELEASE_LABEL, because the
# workflow tests it in a job-level `if:` where only `vars` is available.
BRANCH_RE = re.compile(r"^release-(\d+)-(\d+)$")  # release-0-22
TAG_PREFIX = "v"  # v0.22.21
# ----------------------------------------------------------------------------

VERSION_RE = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")


def parse_version(text: str) -> tuple[int, int, int] | None:
    """Parse a bare X.Y.Z version. Returns None for anything else.

    None means "refuse", not "error": a -dev suffix is a legitimate state for a
    branch to be in, it just is not one this tool will compute a release from.
    """
    match = VERSION_RE.match(text.strip())
    if match is None:
        return None
    return tuple(int(part) for part in match.groups())  # type: ignore[return-value]


def branch_minor(branch: str) -> tuple[int, int] | None:
    """Extract (major, minor) from a release branch name, else None."""
    match = BRANCH_RE.match(branch.strip())
    if match is None:
        return None
    return int(match.group(1)), int(match.group(2))


def tags_on_line(tags: list[str], minor: tuple[int, int]) -> list[int]:
    """Patch numbers of tags belonging to one release line.

    Matched by parsing three integer fields, NOT by pattern. Glob "v0.2*" (or
    regex "^v0\\.2") matches every tag from v0.20 through v0.22 and would hand
    a release-0-2 line a next version of 0.2.22. Tags that do not parse are
    skipped, not fatal.
    """
    patches = []
    for tag in tags:
        if not tag.startswith(TAG_PREFIX):
            continue
        parsed = parse_version(tag[len(TAG_PREFIX) :])
        if parsed is None:
            continue
        major, minor_field, patch = parsed
        if (major, minor_field) == minor:
            patches.append(patch)
    return patches


def compute_next(current: tuple[int, int, int], patches: list[int]) -> str:
    """next = max(current patch, highest tagged patch) + 1.

    Maxing over both sources rather than just incrementing pyproject.toml is
    what stops a version number ever being reused: a tag deleted during
    recovery does not pull the sequence backwards, because pyproject.toml has
    already advanced past it.
    """
    major, minor, patch = current
    highest = max([patch, *patches])
    return f"{major}.{minor}.{highest + 1}"


PROJECT_TABLE_RE = re.compile(r"^\[project\]\s*$", re.MULTILINE)
NEXT_TABLE_RE = re.compile(r"^\[", re.MULTILINE)
VERSION_LINE_RE = re.compile(r'^version\s*=\s*"([^"]*)"\s*$', re.MULTILINE)


def _locate_version(text: str) -> re.Match[str]:
    """Find the version assignment inside the [project] table.

    Anchored to the table on purpose. An unanchored search would happily match
    a dependency pin or another table's version key, and silently rewrite the
    wrong line. Reading and writing both go through here so they cannot drift.
    """
    table = PROJECT_TABLE_RE.search(text)
    if table is None:
        raise ValueError("pyproject.toml has no [project] table")

    body_start = table.end()
    next_table = NEXT_TABLE_RE.search(text, body_start)
    body_end = next_table.start() if next_table else len(text)

    match = VERSION_LINE_RE.search(text, body_start, body_end)
    if match is None:
        raise ValueError("pyproject.toml [project] table has no version")
    return match


def read_version(text: str) -> str:
    return _locate_version(text).group(1)


def write_version(text: str, version: str) -> str:
    match = _locate_version(text)
    return text[: match.start(1)] + version + text[match.end(1) :]


EXIT_OK = 0
EXIT_ERROR = 1
EXIT_REFUSED = 2


def _read_tags(source: str) -> list[str]:
    if source == "-":
        return sys.stdin.read().split()
    out = subprocess.run(
        ["git", "tag", "--list", "v*"],
        capture_output=True,
        text=True,
        check=True,
    )
    return out.stdout.split()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--branch", required=True, help="e.g. release-0-22")
    parser.add_argument("--pyproject", default="pyproject.toml")
    parser.add_argument("--tags-from", default="git", choices=["git", "-"])
    parser.add_argument(
        "--write",
        action="store_true",
        help="rewrite pyproject.toml with the computed version",
    )
    args = parser.parse_args(argv)

    minor = branch_minor(args.branch)
    if minor is None:
        print(f"refusing: {args.branch!r} is not a release branch", file=sys.stderr)
        return EXIT_REFUSED

    try:
        with open(args.pyproject, encoding="utf-8") as handle:
            text = handle.read()
        raw = read_version(text)
        tags = _read_tags(args.tags_from)
    except (OSError, ValueError, subprocess.CalledProcessError) as err:
        # CalledProcessError stringifies to the exit status alone, so git's own
        # "fatal: not a git repository" would be lost. Exit 1 makes the workflow
        # open an issue; the status number by itself does not say what to fix.
        detail = (getattr(err, "stderr", "") or "").strip()
        print(f"error: {err}" + (f": {detail}" if detail else ""), file=sys.stderr)
        return EXIT_ERROR

    current = parse_version(raw)
    if current is None:
        print(
            f"refusing: version {raw!r} is not a bare X.Y.Z; "
            "a release line must carry a released version",
            file=sys.stderr,
        )
        return EXIT_REFUSED

    if current[:2] != minor:
        want = f"{minor[0]}.{minor[1]}"
        print(
            f"refusing: version {raw!r} does not belong to the {want} line "
            f"({args.branch})",
            file=sys.stderr,
        )
        return EXIT_REFUSED

    version = compute_next(current, tags_on_line(tags, minor))

    if args.write:
        try:
            with open(args.pyproject, "w", encoding="utf-8") as handle:
                handle.write(write_version(text, version))
        except OSError as err:
            print(f"error: {err}", file=sys.stderr)
            return EXIT_ERROR

    print(version)
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
