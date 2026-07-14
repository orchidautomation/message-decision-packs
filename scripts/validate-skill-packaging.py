#!/usr/bin/env python3
"""Prove that every shipped skill bundle comes from plugin/skills."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import sys
from pathlib import Path


HOSTS = ("claude-code", "cursor", "codex", "opencode")
GENERATED_INVENTORIES = {
    "codex": ".codex/skills.generated.json",
    "opencode": "skills.generated.json",
}
FRONTMATTER_NAME = re.compile(r"^name:\s*['\"]?([^'\"\n]+?)['\"]?\s*$", re.MULTILINE)


def file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def is_executable(path: Path) -> bool:
    return bool(path.stat().st_mode & stat.S_IXUSR)


def skill_inventory(root: Path, errors: list[str]) -> list[str]:
    if not root.is_dir():
        errors.append(f"missing skill root: {root}")
        return []

    inventory: list[str] = []
    for entry in sorted(root.iterdir()):
        if not entry.is_dir():
            errors.append(f"unexpected file in skill root: {entry}")
            continue

        skill_file = entry / "SKILL.md"
        if not skill_file.is_file():
            errors.append(f"missing SKILL.md: {entry}")
            continue

        text = skill_file.read_text(encoding="utf-8")
        if not text.startswith("---\n") or "\n---\n" not in text[4:]:
            errors.append(f"invalid YAML frontmatter delimiters: {skill_file}")
            continue
        frontmatter = text.split("\n---\n", 1)[0][4:]
        match = FRONTMATTER_NAME.search(frontmatter)
        if not match:
            errors.append(f"missing frontmatter name: {skill_file}")
            continue
        declared_name = match.group(1).strip()
        if declared_name != entry.name:
            errors.append(
                f"skill directory/frontmatter mismatch: {entry.name} != {declared_name}"
            )
        inventory.append(entry.name)

    if not inventory:
        errors.append(f"no skills found: {root}")
    return inventory


def relative_files(root: Path) -> dict[str, Path]:
    return {
        path.relative_to(root).as_posix(): path
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def compare_bundle(source: Path, bundle: Path, host: str, errors: list[str]) -> None:
    source_files = relative_files(source)
    bundle_files = relative_files(bundle)
    source_paths = set(source_files)
    bundle_paths = set(bundle_files)

    for path in sorted(source_paths - bundle_paths):
        errors.append(f"{host} bundle missing canonical file: {path}")
    for path in sorted(bundle_paths - source_paths):
        errors.append(f"{host} bundle has non-canonical skill file: {path}")

    for path in sorted(source_paths & bundle_paths):
        source_file = source_files[path]
        bundle_file = bundle_files[path]
        if file_digest(source_file) != file_digest(bundle_file):
            errors.append(f"{host} bundle content drift: {path}")
        if is_executable(source_file) != is_executable(bundle_file):
            errors.append(f"{host} bundle executable-bit drift: {path}")


def validate_generated_inventory(
    dist: Path, host: str, expected: list[str], errors: list[str]
) -> None:
    manifest_path = dist / host / GENERATED_INVENTORIES[host]
    if not manifest_path.is_file():
        errors.append(f"missing generated skill inventory: {manifest_path}")
        return
    try:
        payload = json.loads(manifest_path.read_text(encoding="utf-8"))
        actual = sorted(skill["id"] for skill in payload["skills"])
    except (json.JSONDecodeError, KeyError, TypeError) as exc:
        errors.append(f"invalid generated skill inventory {manifest_path}: {exc}")
        return
    if actual != expected:
        errors.append(
            f"{host} generated inventory drift: expected {expected}, found {actual}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("plugin/skills"))
    parser.add_argument("--dist", type=Path, default=Path("dist"))
    parser.add_argument(
        "--require-bundles",
        action="store_true",
        help="Require and compare all generated host bundles.",
    )
    args = parser.parse_args()

    errors: list[str] = []
    if Path("skills").exists():
        errors.append("duplicate authored skill root is forbidden: skills/")
    if Path("examples/ai-sdr-eve-vercel/agent/skills").exists():
        errors.append(
            "vendored example skill copies are forbidden: "
            "examples/ai-sdr-eve-vercel/agent/skills/"
        )

    expected = skill_inventory(args.source, errors)

    if args.require_bundles:
        for host in HOSTS:
            bundle_root = args.dist / host / "skills"
            actual = skill_inventory(bundle_root, errors)
            if actual != expected:
                errors.append(
                    f"{host} skill inventory drift: expected {expected}, found {actual}"
                )
            if bundle_root.is_dir():
                compare_bundle(args.source, bundle_root, host, errors)
        for host in GENERATED_INVENTORIES:
            validate_generated_inventory(args.dist, host, expected, errors)

    result = {
        "model": "mdp.skill-packaging-validation.v1",
        "source": str(args.source),
        "skills": expected,
        "hosts": list(HOSTS) if args.require_bundles else [],
        "valid": not errors,
        "errors": errors,
    }
    print(json.dumps(result, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
