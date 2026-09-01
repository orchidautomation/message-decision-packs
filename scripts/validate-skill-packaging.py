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
PORTABLE_TARGET = "agent-plugins"
CORPUS_ROOT = Path("plugin/skill-evals")
GENERATED_INVENTORIES = {
    "codex": ".codex/skills.generated.json",
    "opencode": "skills.generated.json",
}
FRONTMATTER_NAME = re.compile(r"^name:\s*['\"]?([^'\"\n]+?)['\"]?\s*$", re.MULTILINE)
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
SCRIPT_TOKEN = re.compile(r"(?<![\w/])scripts/(mdp-[a-z0-9-]+\.mjs)")
PACKAGED_DOC_SURFACES = (
    Path("plugin/skills"),
    Path("assets"),
    Path("plugin/assets"),
)
REPO_ONLY_DOC_REFERENCES = (
    "docs/headless-normalization-runners.md#canonical-runner-support-matrix",
)
CURRENT_AGENT_SURFACES = (
    Path("llms.txt"),
    Path("llms-full.txt"),
    Path("examples/ai-sdr-eve-vercel/agent/instructions.md"),
)
REMOVED_SURFACE_TERMS = (
    "agent" + "-surface",
    "profile." + "agent_surface",
    "mdp" + "-avoid-rules",
    "mdp" + "-copy-brief",
    "mdp" + "-copy-eval",
    "mdp" + "-lfg",
    "mdp" + "-create-pack",
    "mdp" + "-cta-builder",
    "mdp" + "-message-angles",
    "mdp" + "-output-rules",
    "mdp" + "-source-strategy",
    "mdp" + "-source-extract",
    "mdp" + "-icp-builder",
    "mdp" + "-prospect-brief",
    "mdp" + "-pack-eval",
    "mdp" + "-proposal-bid-no-bid-review",
    "mdp" + "-proposal-compliance-review",
    "mdp" + "-proposal-pack-builder",
    "mdp" + "-proposal-red-team-gap-review",
    "mdp" + "-proposal-win-theme-proof-review",
)


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


def symlink_paths(root: Path):
    """Yield symlinks without following symlinked files or directories."""
    if root.is_symlink():
        yield root
        return
    if not root.is_dir():
        return
    for path in sorted(root.iterdir()):
        if path.is_symlink():
            yield path
        elif path.is_dir():
            yield from symlink_paths(path)


def reject_symlinks(root: Path, label: str, errors: list[str]) -> None:
    for path in symlink_paths(root):
        try:
            relative = path.relative_to(root).as_posix()
        except ValueError:
            relative = path.as_posix()
        errors.append(f"{label}: symlink is not allowed: {relative}")


def compare_bundle(source: Path, bundle: Path, host: str, errors: list[str]) -> None:
    reject_symlinks(source, f"{host} source", errors)
    reject_symlinks(bundle, f"{host} bundle", errors)
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


def compare_tree(source: Path, bundle: Path, label: str, errors: list[str]) -> None:
    if not source.is_dir():
        errors.append(f"{label}: missing source directory: {source}")
        return
    if not bundle.is_dir():
        errors.append(f"{label}: missing generated directory: {bundle}")
        return
    reject_symlinks(source, f"{label} source", errors)
    reject_symlinks(bundle, label, errors)
    source_files = relative_files(source)
    bundle_files = relative_files(bundle)
    for path in sorted(set(source_files) - set(bundle_files)):
        errors.append(f"{label}: missing canonical file: {path}")
    for path in sorted(set(bundle_files) - set(source_files)):
        errors.append(f"{label}: non-canonical file: {path}")
    for path in sorted(set(source_files) & set(bundle_files)):
        if file_digest(source_files[path]) != file_digest(bundle_files[path]):
            errors.append(f"{label}: content drift: {path}")
        if is_executable(source_files[path]) != is_executable(bundle_files[path]):
            errors.append(f"{label}: executable-bit drift: {path}")


def validate_portable_skill_layout(source: Path, errors: list[str]) -> None:
    """Validate each skill as an isolated Agent Skills installation."""
    for skill_dir in sorted(path for path in source.iterdir() if path.is_dir()):
        for doc in sorted(skill_dir.rglob("*.md")):
            for raw_target in MARKDOWN_LINK.findall(doc.read_text(encoding="utf-8")):
                target = raw_target.split("#", 1)[0]
                if (
                    not target
                    or "://" in target
                    or target.startswith(("#", "mailto:", "/"))
                    or not target.endswith(".md")
                ):
                    continue
                resolved = (doc.parent / target).resolve()
                try:
                    resolved.relative_to(skill_dir.resolve())
                except ValueError:
                    errors.append(
                        f"portable skill link escapes isolated root: {doc}: {raw_target}"
                    )
                    continue
                if not resolved.is_file():
                    errors.append(
                        f"portable skill link is missing: {doc}: {raw_target}"
                    )


def validate_shared_reference_parity(source: Path, errors: list[str]) -> None:
    projections = {
        "communication-contract.md": set(skill_inventory(source, [])),
        "runtime-compatibility.md": set(skill_inventory(source, [])),
        "workflow-bundle-handoff.md": {
            "mdp",
            "mdp-pack-apply",
            "mdp-pack-review",
        },
    }
    for filename, skill_ids in projections.items():
        canonical = source / "mdp" / "references" / filename
        if not canonical.is_file():
            errors.append(f"missing canonical shared reference: {canonical}")
            continue
        canonical_digest = file_digest(canonical)
        for skill_id in sorted(skill_ids):
            projected = source / skill_id / "references" / filename
            if not projected.is_file():
                errors.append(f"portable skill missing shared reference: {projected}")
            elif file_digest(projected) != canonical_digest:
                errors.append(f"portable shared reference drift: {projected}")


def referenced_helpers(source: Path) -> set[str]:
    helpers: set[str] = set()
    for doc in sorted(source.rglob("*.md")):
        helpers.update(SCRIPT_TOKEN.findall(doc.read_text(encoding="utf-8")))
    return helpers


def validate_native_helpers(
    source: Path, bundle: Path, host: str, errors: list[str]
) -> None:
    for helper in sorted(referenced_helpers(source)):
        path = bundle / "scripts" / helper
        if not path.is_file():
            errors.append(f"{host} native bundle missing referenced helper: scripts/{helper}")


def load_json(path: Path, errors: list[str]) -> dict:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        errors.append(f"{path}: unable to load JSON: {exc}")
        return {}
    if not isinstance(payload, dict):
        errors.append(f"{path}: expected a JSON object")
        return {}
    return payload


def validate_source_eval_indexes(
    source: Path, corpus: Path, expected: list[str], errors: list[str]
) -> None:
    trigger_payload = load_json(corpus / "trigger-cases.json", errors)
    output_payload = load_json(corpus / "output-cases.json", errors)
    trigger_cases = {
        case.get("id"): case
        for case in trigger_payload.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    output_cases = {
        case.get("id"): case
        for case in output_payload.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    seen_triggers: dict[str, int] = {}
    seen_outputs: dict[str, int] = {}
    for skill_id in expected:
        index_path = source / skill_id / "evals" / "index.json"
        index = load_json(index_path, errors)
        if index.get("model") != "mdp.skill-eval-index.v1":
            errors.append(f"{index_path}: unexpected model")
        if index.get("skill_id") != skill_id:
            errors.append(f"{index_path}: skill_id drift")
        if index.get("corpus_root") != "skill-evals":
            errors.append(f"{index_path}: corpus_root must be skill-evals")
        trigger_ids = index.get("trigger_case_ids", [])
        output_ids = index.get("output_case_ids", [])
        if not isinstance(trigger_ids, list) or len(trigger_ids) != len(set(trigger_ids)):
            errors.append(f"{index_path}: trigger_case_ids must be unique")
        if not isinstance(output_ids, list) or len(output_ids) != len(set(output_ids)):
            errors.append(f"{index_path}: output_case_ids must be unique")
        expected_triggers = {
            case_id
            for case_id, case in trigger_cases.items()
            if case.get("expected_skill_id") == skill_id
        }
        if set(trigger_ids) != expected_triggers:
            errors.append(f"{index_path}: trigger ownership does not match corpus")
        expected_outputs = {
            case_id for case_id, case in output_cases.items() if case.get("skill_id") == skill_id
        }
        if set(output_ids) != expected_outputs:
            errors.append(f"{index_path}: output ownership does not match corpus")
        for case_id in trigger_ids:
            seen_triggers[case_id] = seen_triggers.get(case_id, 0) + 1
        for case_id in output_ids:
            seen_outputs[case_id] = seen_outputs.get(case_id, 0) + 1
    for case_id, count in sorted(seen_triggers.items()):
        if count != 1:
            errors.append(f"trigger index ownership is duplicated: {case_id}")
    for case_id, count in sorted(seen_outputs.items()):
        if count != 1:
            errors.append(f"output index ownership is duplicated: {case_id}")


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


def validate_agent_plugins_bundle(
    source: Path, dist: Path, expected: list[str], errors: list[str]
) -> None:
    root = dist / PORTABLE_TARGET
    if not root.is_dir():
        errors.append(f"missing generated portable package: {root}")
        return

    reject_symlinks(root, PORTABLE_TARGET, errors)
    allowed = {"plugin.json", "skills"}
    if (root / "mcp.json").exists():
        allowed.add("mcp.json")
    actual_top = {path.name for path in root.iterdir()}
    for name in sorted(actual_top - allowed):
        errors.append(f"{PORTABLE_TARGET} has native-only or unexpected top-level entry: {name}")
    for name in sorted(allowed - actual_top):
        errors.append(f"{PORTABLE_TARGET} missing required top-level entry: {name}")

    manifest = load_json(root / "plugin.json", errors)
    if manifest.get("$schema") != "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json":
        errors.append(f"{PORTABLE_TARGET} plugin.json must bind the Agent Plugins 1.0.0 schema")
    if manifest.get("name") != "message-decision-packs":
        errors.append(f"{PORTABLE_TARGET} plugin.json name drift")
    if manifest.get("license") != "Elastic-2.0":
        errors.append(f"{PORTABLE_TARGET} plugin.json license must be Elastic-2.0")

    skills_root = root / "skills"
    actual = skill_inventory(skills_root, errors)
    if actual != expected:
        errors.append(
            f"{PORTABLE_TARGET} skill inventory drift: expected {expected}, found {actual}"
        )
    if skills_root.is_dir():
        compare_bundle(source, skills_root, PORTABLE_TARGET, errors)

    # MDP has no explicitly adopted portable MCP declaration. A generated MCP
    # file would be a false product claim even though the generic spec permits it.
    if (root / "mcp.json").exists():
        errors.append(f"{PORTABLE_TARGET} must not claim mcp.json without an MDP declaration")


def validate_current_agent_surfaces(errors: list[str]) -> None:
    for path in CURRENT_AGENT_SURFACES:
        if not path.is_file():
            errors.append(f"missing current agent surface: {path}")
            continue
        text = path.read_text(encoding="utf-8")
        for term in REMOVED_SURFACE_TERMS:
            if term in text:
                errors.append(f"current agent surface retains removed term {term}: {path}")


def validate_packaged_doc_references(errors: list[str]) -> None:
    for root in PACKAGED_DOC_SURFACES:
        if not root.is_dir():
            errors.append(f"missing packaged documentation surface: {root}")
            continue
        for path in sorted(root.rglob("*.md")):
            text = path.read_text(encoding="utf-8")
            for reference in REPO_ONLY_DOC_REFERENCES:
                broken_forms = (f"`{reference}`", f"]({reference})")
                if any(form in text for form in broken_forms):
                    errors.append(
                        f"packaged documentation uses repo-only reference {reference}: {path}"
                    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, default=Path("plugin/skills"))
    parser.add_argument("--corpus", type=Path, default=CORPUS_ROOT)
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
    validate_portable_skill_layout(args.source, errors)
    validate_shared_reference_parity(args.source, errors)
    validate_source_eval_indexes(args.source, args.corpus, expected, errors)
    validate_current_agent_surfaces(errors)
    validate_packaged_doc_references(errors)

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
            corpus_bundle = args.dist / host / "skill-evals"
            compare_tree(args.corpus, corpus_bundle, f"{host} skill-evals", errors)
            validate_native_helpers(args.source, args.dist / host, host, errors)
        for host in GENERATED_INVENTORIES:
            validate_generated_inventory(args.dist, host, expected, errors)
        validate_agent_plugins_bundle(args.source, args.dist, expected, errors)

    result = {
        "model": "mdp.skill-packaging-validation.v1",
        "source": str(args.source),
        "skills": expected,
        "hosts": (list(HOSTS) + [PORTABLE_TARGET]) if args.require_bundles else [],
        "valid": not errors,
        "errors": errors,
    }
    print(json.dumps(result, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
