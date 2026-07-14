#!/usr/bin/env python3
"""Validate MDP's five-skill catalog, eval corpus, and CLI eligibility contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any


SPLITS = {"train", "validation"}
CASE_TYPES = {
    "positive",
    "adjacent-skill",
    "near-miss",
    "out-of-scope",
    "profile-crossing",
    "unsafe-request",
}
PACK_PROFILES = {"none", "gtm", "proposal", "invalid"}
SHARED_SKILLS = ["mdp", "mdp-pack-builder", "mdp-pack-review"]
PROFILE_JOBS = {
    "gtm": {
        "prospect-fit-or-brief": "mdp-gtm-brief",
        "outbound-copy-brief": "mdp-gtm-brief",
        "outbound-copy-review": "mdp-gtm-brief",
    },
    "proposal": {
        "bid-no-bid-review": "mdp-proposal-review",
        "compliance-review": "mdp-proposal-review",
        "proof-review": "mdp-proposal-review",
        "red-team-review": "mdp-proposal-review",
    },
}


def load_json(path: Path, errors: list[str]) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        errors.append(f"{path}: unable to load JSON: {exc}")
        return {}


def read_description(skill_file: Path, errors: list[str]) -> str | None:
    for line in skill_file.read_text(encoding="utf-8").splitlines():
        if not line.startswith("description: "):
            continue
        raw = line.removeprefix("description: ").strip()
        if raw.startswith('"') and raw.endswith('"'):
            try:
                return json.loads(raw)
            except json.JSONDecodeError as exc:
                errors.append(f"{skill_file}: invalid quoted description: {exc}")
                return None
        if raw.startswith("'") and raw.endswith("'"):
            return raw[1:-1].replace("''", "'")
        return raw
    errors.append(f"{skill_file}: missing single-line description")
    return None


def skill_inventory(root: Path) -> list[str]:
    if not root.is_dir():
        return []
    return sorted(
        entry.name
        for entry in root.iterdir()
        if entry.is_dir() and (entry / "SKILL.md").is_file()
    )


def relative_files(root: Path) -> dict[str, Path]:
    return {
        path.relative_to(root).as_posix(): path
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def file_digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def is_executable(path: Path) -> bool:
    return bool(path.stat().st_mode & stat.S_IXUSR)


def compare_skill_trees(source: Path, installed: Path, errors: list[str]) -> None:
    for path in sorted(installed.rglob("*")):
        if path.is_symlink():
            errors.append(
                "installed skills must be self-contained; symlink found: "
                f"{path.relative_to(installed).as_posix()}"
            )
    source_files = relative_files(source)
    installed_files = relative_files(installed)
    source_paths = set(source_files)
    installed_paths = set(installed_files)
    for path in sorted(source_paths - installed_paths):
        errors.append(f"installed skills missing canonical file: {path}")
    for path in sorted(installed_paths - source_paths):
        errors.append(f"installed skills contain non-canonical file: {path}")
    for path in sorted(source_paths & installed_paths):
        if file_digest(source_files[path]) != file_digest(installed_files[path]):
            errors.append(f"installed skill content drift: {path}")
        if is_executable(source_files[path]) != is_executable(installed_files[path]):
            errors.append(f"installed skill executable-bit drift: {path}")


def normalized_query(value: str) -> str:
    return re.sub(r"\s+", " ", value.strip().lower())


def validate_coverage(
    payload: dict[str, Any], skill_root: Path, installed_root: Path | None, errors: list[str]
) -> tuple[list[str], dict[str, dict[str, Any]]]:
    if payload.get("model") != "mdp.skill-eval-coverage.v1":
        errors.append("coverage.json: unexpected model")
    rows = payload.get("skills")
    if not isinstance(rows, list) or not rows:
        errors.append("coverage.json: skills must be a non-empty list")
        return [], {}

    definitions: dict[str, dict[str, Any]] = {}
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str):
            errors.append("coverage.json: every skill row needs a string id")
            continue
        skill_id = row["id"]
        if skill_id in definitions:
            errors.append(f"coverage.json: duplicate skill {skill_id}")
            continue
        modes = row.get("modes")
        categories = row.get("required_assertion_categories")
        if not isinstance(modes, list) or not modes or len(modes) != len(set(modes)):
            errors.append(f"coverage.json: {skill_id} needs unique modes")
        if not isinstance(categories, list) or not categories:
            errors.append(f"coverage.json: {skill_id} needs assertion categories")
        definitions[skill_id] = row

    expected = [row["id"] for row in rows if isinstance(row, dict) and row.get("id") in definitions]
    source = skill_inventory(skill_root)
    if source != sorted(expected):
        errors.append(f"source skill inventory mismatch: expected {expected}, found {source}")

    for skill_id in source:
        description = read_description(skill_root / skill_id / "SKILL.md", errors)
        if not description or len(description) > 1024:
            errors.append(f"{skill_id}: description must be 1-1024 characters")
        if "TODO" in (skill_root / skill_id / "SKILL.md").read_text(encoding="utf-8"):
            errors.append(f"{skill_id}: TODO placeholder remains")

    if installed_root is not None:
        installed = skill_inventory(installed_root)
        if installed != sorted(expected):
            errors.append(
                f"installed skill inventory mismatch: expected {expected}, found {installed}"
            )
        if installed_root.is_dir():
            compare_skill_trees(skill_root, installed_root, errors)

    host_requirements = payload.get("host_observation_requirements")
    if not isinstance(host_requirements, dict):
        errors.append("coverage.json: host_observation_requirements must be an object")
    else:
        for field in ("minimum_trigger_accuracy", "minimum_output_assertion_accuracy"):
            if host_requirements.get(field) != 1.0:
                errors.append(f"coverage.json: {field} must be exactly 1.0")
    return expected, definitions


def validate_triggers(
    payload: dict[str, Any],
    coverage: dict[str, Any],
    skills: list[str],
    definitions: dict[str, dict[str, Any]],
    errors: list[str],
) -> dict[str, Any]:
    if payload.get("model") != "mdp.skill-trigger-corpus.v1":
        errors.append("trigger-cases.json: unexpected model")
    cases = payload.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append("trigger-cases.json: cases must be non-empty")
        return {"total": 0}

    ids: set[str] = set()
    scenario_splits: dict[str, set[str]] = defaultdict(set)
    query_owners: dict[str, str | None] = {}
    owned: Counter[tuple[str, str]] = Counter()
    null_routes: Counter[str] = Counter()
    modes: set[tuple[str, str, str]] = set()
    crossing: set[tuple[str, str, str]] = set()
    collision_pairs: set[tuple[str, str, str]] = set()

    for index, case in enumerate(cases):
        label = f"trigger case #{index + 1}"
        if not isinstance(case, dict):
            errors.append(f"{label}: must be an object")
            continue
        case_id = case.get("id")
        split = case.get("split")
        family = case.get("scenario_family")
        case_type = case.get("case_type")
        query = case.get("query")
        owner = case.get("expected_skill_id")
        mode = case.get("mode")
        context = case.get("context")

        if not isinstance(case_id, str) or not case_id:
            errors.append(f"{label}: id must be non-empty")
        elif case_id in ids:
            errors.append(f"trigger-cases.json: duplicate id {case_id}")
        else:
            ids.add(case_id)
        if split not in SPLITS:
            errors.append(f"{case_id}: invalid split {split}")
            continue
        if not isinstance(family, str) or not family:
            errors.append(f"{case_id}: scenario_family must be non-empty")
        else:
            scenario_splits[family].add(split)
        if case_type not in CASE_TYPES:
            errors.append(f"{case_id}: invalid case_type {case_type}")
        if not isinstance(query, str) or not query.strip():
            errors.append(f"{case_id}: query must be non-empty")
        else:
            normalized = normalized_query(query)
            if normalized in query_owners and query_owners[normalized] != owner:
                errors.append(f"{case_id}: normalized query has conflicting expected owner")
            query_owners[normalized] = owner
        owner_known = owner is None or owner in skills
        if not owner_known:
            errors.append(f"{case_id}: unknown expected_skill_id {owner}")
        if not isinstance(context, dict) or context.get("pack_profile") not in PACK_PROFILES:
            errors.append(f"{case_id}: invalid context.pack_profile")

        if owner is None:
            if mode is not None:
                errors.append(f"{case_id}: null-route case must have null mode")
            null_routes[split] += 1
        elif owner_known:
            if mode not in definitions[owner].get("modes", []):
                errors.append(f"{case_id}: mode {mode} is not declared for {owner}")
            owned[(owner, split)] += 1
            modes.add((owner, mode, split))

        if case_type == "profile-crossing" and isinstance(context, dict):
            crossing.add((split, context.get("pack_profile"), context.get("job_id")))

    collisions = payload.get("collisions")
    if not isinstance(collisions, list) or not collisions:
        errors.append("trigger-cases.json: collisions must be a non-empty list")
    else:
        cases_by_id = {
            case.get("id"): case
            for case in cases
            if isinstance(case, dict) and isinstance(case.get("id"), str)
        }
        collision_ids: set[str] = set()
        for index, collision in enumerate(collisions):
            label = f"collision #{index + 1}"
            if not isinstance(collision, dict):
                errors.append(f"{label}: must be an object")
                continue
            collision_id = collision.get("id")
            case_id = collision.get("case_id")
            competing = collision.get("competing_skill_id")
            if not isinstance(collision_id, str) or not collision_id:
                errors.append(f"{label}: id must be non-empty")
            elif collision_id in collision_ids:
                errors.append(f"trigger-cases.json: duplicate collision id {collision_id}")
            else:
                collision_ids.add(collision_id)
            case = cases_by_id.get(case_id)
            if case is None:
                errors.append(f"{collision_id}: unknown case_id {case_id}")
                continue
            owner = case.get("expected_skill_id")
            split = case.get("split")
            if owner not in skills:
                errors.append(f"{collision_id}: collision case must have a known owner")
            if competing not in skills or competing == owner:
                errors.append(f"{collision_id}: invalid competing_skill_id {competing}")
            if owner in skills and competing in skills and competing != owner and split in SPLITS:
                collision_pairs.add((competing, owner, split))

    for family, splits in scenario_splits.items():
        if len(splits) > 1:
            errors.append(f"trigger scenario family appears in both splits: {family}")

    requirements = coverage.get("trigger_requirements", {})
    minimum_owned = requirements.get("minimum_owned_cases_per_skill_per_split", 3)
    minimum_null = requirements.get("minimum_null_route_cases_per_split", 2)
    for skill_id in skills:
        for split in SPLITS:
            if owned[(skill_id, split)] < minimum_owned:
                errors.append(
                    f"trigger coverage: {skill_id}/{split} has {owned[(skill_id, split)]}, "
                    f"needs {minimum_owned} owned cases"
                )
            for mode in definitions[skill_id].get("modes", []):
                if (skill_id, mode, split) not in modes:
                    errors.append(f"trigger coverage missing {skill_id}/{mode}/{split}")
            for other in skills:
                if other != skill_id and (other, skill_id, split) not in collision_pairs:
                    errors.append(f"collision coverage missing {other} -> {skill_id} in {split}")
    for split in SPLITS:
        if null_routes[split] < minimum_null:
            errors.append(f"trigger coverage: {split} needs at least {minimum_null} null routes")
        has_gtm_on_proposal = any(
            item_split == split and profile == "proposal" and job_id in PROFILE_JOBS["gtm"]
            for item_split, profile, job_id in crossing
        )
        has_proposal_on_gtm = any(
            item_split == split and profile == "gtm" and job_id in PROFILE_JOBS["proposal"]
            for item_split, profile, job_id in crossing
        )
        if not has_gtm_on_proposal or not has_proposal_on_gtm:
            errors.append(f"trigger coverage: {split} missing required profile-crossing cases")

    return {
        "total": len(cases),
        "owned_by_skill_split": {
            f"{skill}/{split}": count for (skill, split), count in sorted(owned.items())
        },
        "null_routes": dict(sorted(null_routes.items())),
        "explicit_collision_pairs": len(collision_pairs),
    }


def validate_outputs(
    payload: dict[str, Any],
    coverage: dict[str, Any],
    skills: list[str],
    definitions: dict[str, dict[str, Any]],
    errors: list[str],
) -> dict[str, Any]:
    if payload.get("model") != "mdp.skill-output-corpus.v1":
        errors.append("output-cases.json: unexpected model")
    cases = payload.get("cases")
    if not isinstance(cases, list) or not cases:
        errors.append("output-cases.json: cases must be non-empty")
        return {"total": 0}

    case_ids: set[str] = set()
    assertion_ids: set[str] = set()
    scenario_splits: dict[str, set[str]] = defaultdict(set)
    coverage_seen: set[tuple[str, str, str]] = set()
    allowed_categories = set(
        coverage.get("output_requirements", {}).get("allowed_assertion_categories", [])
    )

    for index, case in enumerate(cases):
        label = f"output case #{index + 1}"
        if not isinstance(case, dict):
            errors.append(f"{label}: must be an object")
            continue
        case_id = case.get("id")
        split = case.get("split")
        family = case.get("scenario_family")
        skill_id = case.get("skill_id")
        mode = case.get("mode")
        risk = case.get("risk_tier")
        assertions = case.get("assertions")

        if not isinstance(case_id, str) or not case_id:
            errors.append(f"{label}: id must be non-empty")
        elif case_id in case_ids:
            errors.append(f"output-cases.json: duplicate id {case_id}")
        else:
            case_ids.add(case_id)
        if split not in SPLITS:
            errors.append(f"{case_id}: invalid split {split}")
            continue
        if not isinstance(family, str) or not family:
            errors.append(f"{case_id}: scenario_family must be non-empty")
        else:
            scenario_splits[family].add(split)
        if skill_id not in skills:
            errors.append(f"{case_id}: unknown skill_id {skill_id}")
            continue
        definition = definitions[skill_id]
        if mode not in definition.get("modes", []):
            errors.append(f"{case_id}: undeclared mode {mode} for {skill_id}")
        if risk != definition.get("risk_tier"):
            errors.append(f"{case_id}: risk_tier must match coverage for {skill_id}")
        coverage_seen.add((skill_id, mode, split))
        for field in ("fixture", "prompt", "expected_output"):
            if not isinstance(case.get(field), str) or not case[field].strip():
                errors.append(f"{case_id}: {field} must be non-empty")
        if not isinstance(assertions, list) or not assertions:
            errors.append(f"{case_id}: assertions must be non-empty")
            continue
        categories: set[str] = set()
        for assertion in assertions:
            if not isinstance(assertion, dict):
                errors.append(f"{case_id}: assertion must be an object")
                continue
            assertion_id = assertion.get("id")
            category = assertion.get("category")
            criterion = assertion.get("criterion")
            if not isinstance(assertion_id, str) or not assertion_id:
                errors.append(f"{case_id}: assertion id must be non-empty")
            elif assertion_id in assertion_ids:
                errors.append(f"duplicate assertion id {assertion_id}")
            else:
                assertion_ids.add(assertion_id)
            if category not in allowed_categories:
                errors.append(f"{case_id}: invalid assertion category {category}")
            else:
                categories.add(category)
            if not isinstance(criterion, str) or not criterion.strip():
                errors.append(f"{case_id}: assertion criterion must be non-empty")
            if assertion.get("required") is not True:
                errors.append(f"{case_id}: all committed assertions must be required")
        required = set(definition.get("required_assertion_categories", []))
        missing = required - categories
        if missing:
            errors.append(f"{case_id}: missing required assertion categories {sorted(missing)}")

    for family, splits in scenario_splits.items():
        if len(splits) > 1:
            errors.append(f"output scenario family appears in both splits: {family}")
    for skill_id in skills:
        for mode in definitions[skill_id].get("modes", []):
            for split in SPLITS:
                if (skill_id, mode, split) not in coverage_seen:
                    errors.append(f"output coverage missing {skill_id}/{mode}/{split}")

    return {
        "total": len(cases),
        "assertions": len(assertion_ids),
        "mode_split_cells": len(coverage_seen),
    }


def resolve_mdp_binary(explicit: Path | None) -> Path | None:
    if explicit is not None:
        return explicit
    source = Path("cli/target/debug/mdp")
    if source.is_file() and os.access(source, os.X_OK):
        return source
    installed = shutil.which("mdp")
    return Path(installed) if installed else None


def run_skills(binary: Path, arguments: list[str], errors: list[str]) -> dict[str, Any]:
    command = [str(binary), "--json", "skills", *arguments]
    try:
        result = subprocess.run(command, check=False, capture_output=True, text=True, timeout=30)
    except (OSError, subprocess.TimeoutExpired) as exc:
        errors.append(f"CLI eligibility command failed to run: {' '.join(command)}: {exc}")
        return {}
    try:
        payload = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        errors.append(f"CLI eligibility returned invalid JSON: {' '.join(command)}: {exc}")
        return {}
    if (
        payload.get("ok") is not True
        or payload.get("command") != "skills"
        or not isinstance(payload.get("data"), dict)
    ):
        errors.append(f"CLI eligibility returned unexpected envelope: {' '.join(command)}")
        return {}
    return payload["data"]


def validate_base_cli(data: dict[str, Any], expected: list[str], label: str, errors: list[str]) -> None:
    if data.get("contract") != "mdp.skills.v1":
        errors.append(f"{label}: wrong contract")
    if data.get("packaged_skill_ids") != expected:
        errors.append(f"{label}: packaged_skill_ids drift")
    host = data.get("host_discovery", {})
    if host.get("status") != "unobserved" or host.get("managed_by") != "agent-host":
        errors.append(f"{label}: host discovery contract drift")


def validate_cli_contract(
    binary: Path | None,
    coverage: dict[str, Any],
    expected: list[str],
    errors: list[str],
) -> dict[str, Any]:
    if binary is None or not binary.is_file():
        errors.append("MDP binary unavailable; pass --mdp-bin or build cli/target/debug/mdp")
        return {"binary": None, "cases": 0}

    checked = 0
    inventory = run_skills(binary, [], errors)
    checked += 1
    validate_base_cli(inventory, expected, "inventory", errors)
    if inventory.get("status") != "bootstrap":
        errors.append("inventory: expected bootstrap status")
    if inventory.get("eligibility", {}).get("eligible_skill_ids") != SHARED_SKILLS:
        errors.append("inventory: shared bootstrap eligibility drift")
    if inventory.get("job_routes") != [] or inventory.get("recommendation") is not None:
        errors.append("inventory: expected zero routes and null recommendation")

    fixtures = coverage.get("cli_fixtures", {})
    for profile, job_map in PROFILE_JOBS.items():
        pack_root = Path(fixtures.get(profile, ""))
        data = run_skills(binary, ["--dir", str(pack_root)], errors)
        checked += 1
        validate_base_cli(data, expected, profile, errors)
        routes = {(row.get("job_id"), row.get("skill_id")) for row in data.get("job_routes", [])}
        expected_routes = set(job_map.items())
        if routes != expected_routes:
            errors.append(f"{profile}: route inventory mismatch: {routes}")
        expected_eligible = SHARED_SKILLS + sorted(set(job_map.values()))
        actual_eligible = data.get("eligibility", {}).get("eligible_skill_ids")
        if actual_eligible != expected_eligible:
            errors.append(
                f"{profile}: eligible skill drift: expected {expected_eligible}, "
                f"found {actual_eligible}"
            )
        expected_ineligible = sorted(set(expected) - set(expected_eligible))
        actual_ineligible = sorted(
            row.get("skill_id")
            for row in data.get("eligibility", {}).get("ineligible_skills", [])
            if isinstance(row, dict)
        )
        if actual_ineligible != expected_ineligible:
            errors.append(
                f"{profile}: ineligible skill drift: expected {expected_ineligible}, "
                f"found {actual_ineligible}"
            )
        for job_id, skill_id in job_map.items():
            routed = run_skills(
                binary, ["--dir", str(pack_root), "--job", job_id], errors
            )
            checked += 1
            validate_base_cli(routed, expected, f"{profile}/{job_id}", errors)
            recommendation = routed.get("recommendation") or {}
            if (
                routed.get("status") != "ready"
                or recommendation.get("job_id") != job_id
                or recommendation.get("skill_id") != skill_id
                or recommendation.get("pack_ready") is not True
            ):
                errors.append(f"{profile}/{job_id}: incorrect recommendation")
            if routed.get("eligibility") != data.get("eligibility"):
                errors.append(f"{profile}/{job_id}: eligibility changed for a single-job query")

    crossing_cases = [
        ("gtm", "bid-no-bid-review"),
        ("proposal", "prospect-fit-or-brief"),
    ]
    for profile, job_id in crossing_cases:
        data = run_skills(
            binary, ["--dir", str(fixtures[profile]), "--job", job_id], errors
        )
        checked += 1
        validate_base_cli(data, expected, f"cross/{profile}/{job_id}", errors)
        if data.get("recommendation") is not None or data.get("status") != "unresolved":
            errors.append(f"cross/{profile}/{job_id}: expected unresolved with no fallback")

    with tempfile.TemporaryDirectory(prefix="mdp-skill-eval-") as missing_root:
        missing = run_skills(binary, ["--dir", missing_root], errors)
        checked += 1
        validate_base_cli(missing, expected, "missing-pack", errors)
        if missing.get("eligibility", {}).get("eligible_skill_ids") != SHARED_SKILLS:
            errors.append("missing-pack: bootstrap eligibility drift")
        if missing.get("job_routes") or missing.get("recommendation") is not None:
            errors.append("missing-pack: expected zero routes and null recommendation")
        if missing.get("valid") is not False or not missing.get("diagnostics"):
            errors.append("missing-pack: expected structured diagnostics")

    return {"binary": str(binary), "cases": checked}


def validate_observed_results(
    path: Path | None,
    trigger_payload: dict[str, Any],
    output_payload: dict[str, Any],
    coverage: dict[str, Any],
    skills: list[str],
    errors: list[str],
) -> dict[str, Any] | None:
    if path is None:
        return None
    payload = load_json(path, errors)
    if payload.get("model") != "mdp.skill-host-results.v1":
        errors.append(f"{path}: unexpected results model")
    for field in ("host", "model_id", "recorded_at"):
        if not isinstance(payload.get(field), str) or not payload[field].strip():
            errors.append(f"{path}: {field} must be non-empty")
    trigger_observations = payload.get("trigger_observations")
    output_observations = payload.get("output_observations")
    if not isinstance(trigger_observations, list) or not trigger_observations:
        errors.append(f"{path}: trigger_observations must be a non-empty list")
        return None
    if not isinstance(output_observations, list) or not output_observations:
        errors.append(f"{path}: output_observations must be a non-empty list")
        return None
    expected_by_id = {
        case["id"]: case.get("expected_skill_id")
        for case in trigger_payload.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    total = 0
    correct = 0
    cross_profile_unsafe = 0
    confusion: Counter[str] = Counter()
    trigger_counts: Counter[str] = Counter()
    trigger_trials: set[tuple[str, str]] = set()
    trigger_cases = {
        case["id"]: case
        for case in trigger_payload.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    for observation in trigger_observations:
        if not isinstance(observation, dict):
            errors.append(f"{path}: every trigger observation must be an object")
            continue
        case_id = observation.get("case_id")
        trial_id = observation.get("trial_id")
        observed = observation.get("selected_skill_id")
        if not isinstance(trial_id, str) or not trial_id:
            errors.append(f"{path}: trigger observation {case_id} needs trial_id")
            continue
        trial_key = (case_id, trial_id)
        if trial_key in trigger_trials:
            errors.append(f"{path}: duplicate trigger trial {case_id}/{trial_id}")
            continue
        trigger_trials.add(trial_key)
        if case_id not in expected_by_id:
            errors.append(f"{path}: unknown observed case {case_id}")
            continue
        if observed is not None and observed not in skills:
            errors.append(f"{path}: unknown selected skill {observed}")
            continue
        expected = expected_by_id[case_id]
        trigger_counts[case_id] += 1
        total += 1
        if expected == observed:
            correct += 1
        else:
            confusion[f"{expected or 'null'}->{observed or 'null'}"] += 1
            errors.append(
                f"observed results: trigger mismatch {case_id}: expected "
                f"{expected or 'null'}, found {observed or 'null'}"
            )
        trigger_case = trigger_cases[case_id]
        if trigger_case.get("case_type") == "profile-crossing" and observed is not None:
            cross_profile_unsafe += 1
    if cross_profile_unsafe:
        errors.append(f"observed results: {cross_profile_unsafe} profile-crossing unsafe passes")
    requirements = coverage.get("host_observation_requirements", {})
    missing_trigger_cases = sorted(set(expected_by_id) - set(trigger_counts))
    if requirements.get("require_all_trigger_cases", True) and missing_trigger_cases:
        errors.append(f"observed results: missing trigger cases {missing_trigger_cases}")
    trigger_accuracy = correct / total if total else 0.0
    minimum_trigger_accuracy = requirements.get("minimum_trigger_accuracy", 1.0)
    if trigger_accuracy < minimum_trigger_accuracy:
        errors.append(
            f"observed results: trigger accuracy {trigger_accuracy:.3f} is below "
            f"{minimum_trigger_accuracy:.3f}"
        )

    output_cases = {
        case["id"]: case
        for case in output_payload.get("cases", [])
        if isinstance(case, dict) and isinstance(case.get("id"), str)
    }
    output_counts: Counter[str] = Counter()
    output_trials: set[tuple[str, str]] = set()
    assertion_total = 0
    assertion_passed = 0
    failed_assertions: list[str] = []
    for observation in output_observations:
        if not isinstance(observation, dict):
            errors.append(f"{path}: every output observation must be an object")
            continue
        case_id = observation.get("case_id")
        trial_id = observation.get("trial_id")
        grades = observation.get("assertions")
        if case_id not in output_cases:
            errors.append(f"{path}: unknown output case {case_id}")
            continue
        if not isinstance(trial_id, str) or not trial_id:
            errors.append(f"{path}: output observation {case_id} needs trial_id")
            continue
        trial_key = (case_id, trial_id)
        if trial_key in output_trials:
            errors.append(f"{path}: duplicate output trial {case_id}/{trial_id}")
            continue
        output_trials.add(trial_key)
        if not isinstance(grades, dict):
            errors.append(f"{path}: output observation {case_id} needs assertion grades")
            continue
        expected_assertions = {
            assertion["id"]
            for assertion in output_cases[case_id].get("assertions", [])
            if isinstance(assertion, dict) and assertion.get("required") is True
        }
        unknown_assertions = sorted(set(grades) - expected_assertions)
        missing_assertions = sorted(expected_assertions - set(grades))
        if unknown_assertions:
            errors.append(f"{path}: {case_id} has unknown assertions {unknown_assertions}")
        if missing_assertions:
            errors.append(f"{path}: {case_id} is missing assertions {missing_assertions}")
        output_counts[case_id] += 1
        for assertion_id in sorted(expected_assertions & set(grades)):
            grade = grades[assertion_id]
            if not isinstance(grade, bool):
                errors.append(f"{path}: {case_id}/{assertion_id} grade must be boolean")
                continue
            assertion_total += 1
            if grade:
                assertion_passed += 1
            else:
                failed_assertions.append(f"{case_id}/{assertion_id}")
                errors.append(
                    f"observed results: required output assertion failed "
                    f"{case_id}/{assertion_id}"
                )

    missing_output_cases = sorted(set(output_cases) - set(output_counts))
    if requirements.get("require_all_output_cases", True) and missing_output_cases:
        errors.append(f"observed results: missing output cases {missing_output_cases}")
    assertion_accuracy = assertion_passed / assertion_total if assertion_total else 0.0
    minimum_assertion_accuracy = requirements.get("minimum_output_assertion_accuracy", 1.0)
    if assertion_accuracy < minimum_assertion_accuracy:
        errors.append(
            f"observed results: output assertion accuracy {assertion_accuracy:.3f} is below "
            f"{minimum_assertion_accuracy:.3f}; failed {failed_assertions}"
        )
    return {
        "host": payload.get("host"),
        "model_id": payload.get("model_id"),
        "trigger": {
            "trials": total,
            "cases_observed": len(trigger_counts),
            "correct": correct,
            "accuracy": trigger_accuracy,
            "confusion": dict(sorted(confusion.items())),
            "profile_crossing_unsafe": cross_profile_unsafe,
        },
        "output": {
            "trials": len(output_trials),
            "cases_observed": len(output_counts),
            "assertions": assertion_total,
            "assertions_passed": assertion_passed,
            "assertion_accuracy": assertion_accuracy,
            "failed_assertions": failed_assertions,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plugin-skills", type=Path, default=Path("plugin/skills"))
    parser.add_argument("--corpus", type=Path, default=Path("plugin/skill-evals"))
    parser.add_argument("--mdp-bin", type=Path)
    parser.add_argument("--installed-skills-root", type=Path)
    parser.add_argument("--results", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    errors: list[str] = []
    coverage = load_json(args.corpus / "coverage.json", errors)
    triggers = load_json(args.corpus / "trigger-cases.json", errors)
    outputs = load_json(args.corpus / "output-cases.json", errors)

    skills, definitions = validate_coverage(
        coverage, args.plugin_skills, args.installed_skills_root, errors
    )
    trigger_summary = validate_triggers(triggers, coverage, skills, definitions, errors)
    output_summary = validate_outputs(outputs, coverage, skills, definitions, errors)
    cli_summary = validate_cli_contract(resolve_mdp_binary(args.mdp_bin), coverage, skills, errors)
    observed_summary = validate_observed_results(
        args.results, triggers, outputs, coverage, skills, errors
    )

    result = {
        "model": "mdp.skill-eval-report.v1",
        "valid": not errors,
        "skills": skills,
        "trigger": trigger_summary,
        "output": output_summary,
        "cli": cli_summary,
        "observed": observed_summary,
        "errors": errors,
    }
    if args.output:
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / "benchmark.json").write_text(
            json.dumps(result, indent=2) + "\n", encoding="utf-8"
        )
    print(json.dumps(result, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
