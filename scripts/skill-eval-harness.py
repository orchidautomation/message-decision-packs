#!/usr/bin/env python3
"""Validate MDP skill trigger and output eval fixture files.

This local harness validates committed eval definitions. It does not observe
Codex skill activation; use agent transcripts or client-specific logs for live
trigger-rate scoring.
"""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path
from typing import Any


REQUIRED_TRIGGER_FIELDS = {"id", "split", "query", "should_trigger"}
REQUIRED_OUTPUT_FIELDS = {"id", "prompt", "expected_output", "assertions"}
VALID_SPLITS = {"train", "validation"}


def load_json(path: Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def fail(message: str, errors: list[str]) -> None:
    errors.append(message)


def validate_trigger_file(path: Path, errors: list[str]) -> dict[str, Any]:
    payload = load_json(path)
    skill = payload.get("skill")
    queries = payload.get("queries")
    if not isinstance(skill, str) or not skill:
        fail(f"{path}: skill must be a non-empty string", errors)
    if not isinstance(queries, list) or not queries:
        fail(f"{path}: queries must be a non-empty list", errors)
        return {"total": 0, "should_trigger": 0, "should_not_trigger": 0, "splits": {}}

    seen: set[str] = set()
    splits: dict[str, int] = {}
    should_trigger = 0
    should_not_trigger = 0
    for index, query in enumerate(queries):
        if not isinstance(query, dict):
            fail(f"{path}: query #{index + 1} must be an object", errors)
            continue
        missing = REQUIRED_TRIGGER_FIELDS - set(query)
        if missing:
            fail(f"{path}: query #{index + 1} missing {sorted(missing)}", errors)
        query_id = query.get("id")
        if not isinstance(query_id, str) or not query_id:
            fail(f"{path}: query #{index + 1} id must be a non-empty string", errors)
        elif query_id in seen:
            fail(f"{path}: duplicate query id {query_id}", errors)
        else:
            seen.add(query_id)
        split = query.get("split")
        if split not in VALID_SPLITS:
            fail(
                f"{path}: query {query_id or index + 1} split must be one of {sorted(VALID_SPLITS)}",
                errors,
            )
        else:
            splits[split] = splits.get(split, 0) + 1
        if not isinstance(query.get("query"), str) or not query.get("query", "").strip():
            fail(f"{path}: query {query_id or index + 1} text must be non-empty", errors)
        should = query.get("should_trigger")
        if not isinstance(should, bool):
            fail(f"{path}: query {query_id or index + 1} should_trigger must be boolean", errors)
        elif should:
            should_trigger += 1
        else:
            should_not_trigger += 1

    if "train" not in splits or "validation" not in splits:
        fail(f"{path}: include both train and validation splits", errors)
    if should_trigger < 8 or should_not_trigger < 8:
        fail(f"{path}: include at least 8 should-trigger and 8 should-not-trigger queries", errors)

    return {
        "total": len(queries),
        "should_trigger": should_trigger,
        "should_not_trigger": should_not_trigger,
        "splits": splits,
    }


def validate_output_file(path: Path, errors: list[str]) -> dict[str, Any]:
    payload = load_json(path)
    evals = payload.get("evals")
    if not isinstance(payload.get("skill"), str) or not payload.get("skill"):
        fail(f"{path}: skill must be a non-empty string", errors)
    if not isinstance(evals, list) or not evals:
        fail(f"{path}: evals must be a non-empty list", errors)
        return {"total": 0, "assertions": 0}

    seen: set[str] = set()
    assertion_counts: list[int] = []
    for index, case in enumerate(evals):
        if not isinstance(case, dict):
            fail(f"{path}: eval #{index + 1} must be an object", errors)
            continue
        missing = REQUIRED_OUTPUT_FIELDS - set(case)
        if missing:
            fail(f"{path}: eval #{index + 1} missing {sorted(missing)}", errors)
        case_id = case.get("id")
        if not isinstance(case_id, str) or not case_id:
            fail(f"{path}: eval #{index + 1} id must be a non-empty string", errors)
        elif case_id in seen:
            fail(f"{path}: duplicate eval id {case_id}", errors)
        else:
            seen.add(case_id)
        for field in ("prompt", "expected_output"):
            if not isinstance(case.get(field), str) or not case.get(field, "").strip():
                fail(f"{path}: eval {case_id or index + 1} {field} must be non-empty", errors)
        assertions = case.get("assertions")
        if not isinstance(assertions, list) or len(assertions) < 2:
            fail(f"{path}: eval {case_id or index + 1} needs at least two assertions", errors)
            assertion_counts.append(0)
            continue
        for assertion in assertions:
            if not isinstance(assertion, str) or not assertion.strip():
                fail(f"{path}: eval {case_id or index + 1} assertions must be non-empty strings", errors)
        assertion_counts.append(len(assertions))

    return {
        "total": len(evals),
        "assertions": sum(assertion_counts),
        "mean_assertions": statistics.mean(assertion_counts) if assertion_counts else 0,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate MDP skill eval fixture files.")
    parser.add_argument("--plugin-skills", default="plugin/skills", type=Path, help="Skill root to scan.")
    parser.add_argument("--output", type=Path, help="Optional output directory for benchmark.json.")
    args = parser.parse_args()

    errors: list[str] = []
    summaries: list[dict[str, Any]] = []
    for skill_dir in sorted(args.plugin_skills.iterdir()):
        eval_dir = skill_dir / "evals"
        if not eval_dir.is_dir():
            continue
        trigger_file = eval_dir / "trigger-queries.json"
        output_file = eval_dir / "output-evals.json"
        if not trigger_file.exists() or not output_file.exists():
            fail(f"{eval_dir}: expected trigger-queries.json and output-evals.json", errors)
            continue
        summaries.append(
            {
                "skill": skill_dir.name,
                "trigger": validate_trigger_file(trigger_file, errors),
                "output": validate_output_file(output_file, errors),
            }
        )

    result = {
        "valid": not errors,
        "skill_count": len(summaries),
        "skills": summaries,
        "errors": errors,
    }

    if args.output:
        args.output.mkdir(parents=True, exist_ok=True)
        (args.output / "benchmark.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")

    print(json.dumps(result, indent=2))
    return 0 if not errors else 1


if __name__ == "__main__":
    sys.exit(main())
