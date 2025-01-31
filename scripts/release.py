import os
import logging
import argparse
import subprocess

import tomlkit
from packaging.version import Version

# check if repo is dirty
if subprocess.run(["git", "diff", "--quiet"]).returncode != 0:
    logging.error("Repo is dirty")
    exit(1)

parser = argparse.ArgumentParser(description="Release a new version of the package")
parser.add_argument("version", type=str, help="The version to release")
args = parser.parse_args()

root_dir = os.path.dirname(os.path.dirname(__file__))
toml_path = os.path.join(root_dir, "pyproject.toml")
dist_dir = os.path.join(root_dir, "dist")

with open(toml_path, "r") as f:
    data = tomlkit.parse(f.read())

v = Version(data["project"]["version"])

if args.version == "patch":
    new_v = f"{v.major}.{v.minor}.{v.micro + 1}"
elif args.version == "minor":
    new_v = f"{v.major}.{v.minor + 1}.0"
elif args.version == "major":
    new_v = f"{v.major + 1}.0.0"
elif args.version == "current":
    new_v = str(v)

# query if the new version exists
print(f"gh release view {new_v}")
if subprocess.run(["gh", "release", "view", new_v]).returncode == 0:
    logging.error(f"Version {new_v} already exists")
    exit(1)

modified = new_v != str(v)
data["project"]["version"] = new_v
with open(toml_path, "w") as f:
    tomlkit.dump(data, f)


def fail_and_reset(msg):
    logging.error(msg)
    data["project"]["version"] = str(v)
    with open(toml_path, "w") as f:
        tomlkit.dump(data, f)
    exit(1)


# 0. commit version bump
# directly output git error msg

commited = False
if modified:
    print("git add project.toml")
    if subprocess.run(["git", "add", "pyproject.toml"]).returncode != 0:
        fail_and_reset("Failed to add pyproject.toml")

if modified:
    if subprocess.run(["git", "add", "pyproject.toml"]).returncode != 0:
        fail_and_reset("Failed to add pyproject.toml")

    print(f"git commit -m 'Bump version to {new_v}'")
    if (
        subprocess.run(["git", "commit", "-m", f"Bump version to {new_v}"]).returncode
        != 0
    ):
        fail_and_reset("Failed to commit version bump")


def fail_and_revert(msg):
    logging.error(msg)
    if commited:
        print("Reverting commit")
        subprocess.run(["git", "reset", "--hard", "HEAD~1"])
    exit(1)


try:
    # sync with remote no matter what
    if subprocess.run(["git", "push"]).returncode != 0:
        fail_and_revert("Failed to push")

    # 1. build the package via uv
    print("uv build")
    if subprocess.run(["uv", "build"]).returncode != 0:
        fail_and_revert("Failed to build the package")

    # 2. create a new release
    print(f"gh release create {new_v} dist/*")
    if (
        subprocess.run(
            [
                "gh",
                "release",
                "create",
                "v" + new_v,
            ]
        ).returncode
        != 0
    ):
        fail_and_revert("Failed to create release")

except Exception as e:
    fail_and_revert(str(e))
