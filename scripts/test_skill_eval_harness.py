#!/usr/bin/env python3
"""Mutation tests for the MDP skill eval gate."""

from __future__ import annotations

import copy
import importlib.util
import json
import shutil
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "skill_eval_harness", ROOT / "scripts" / "skill-eval-harness.py"
)
assert SPEC and SPEC.loader
HARNESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(HARNESS)


def load(name: str) -> dict:
    return json.loads((ROOT / "plugin" / "skill-evals" / name).read_text(encoding="utf-8"))


class SkillEvalHarnessMutationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.coverage = load("coverage.json")
        self.triggers = load("trigger-cases.json")
        self.outputs = load("output-cases.json")
        self.skills = [row["id"] for row in self.coverage["skills"]]
        self.definitions = {row["id"]: row for row in self.coverage["skills"]}

    def test_unknown_trigger_owner_is_a_structured_error(self) -> None:
        payload = copy.deepcopy(self.triggers)
        payload["cases"][0]["expected_skill_id"] = "not-a-skill"
        errors: list[str] = []

        HARNESS.validate_triggers(
            payload, self.coverage, self.skills, self.definitions, errors
        )

        self.assertTrue(any("unknown expected_skill_id" in error for error in errors))

    def test_collision_coverage_requires_explicit_evidence(self) -> None:
        payload = copy.deepcopy(self.triggers)
        payload["collisions"] = payload["collisions"][1:]
        errors: list[str] = []

        HARNESS.validate_triggers(
            payload, self.coverage, self.skills, self.definitions, errors
        )

        self.assertTrue(any("collision coverage missing" in error for error in errors))

    def test_installed_content_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-installed-skills-") as temp:
            installed = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin" / "skills", installed)
            skill_file = installed / "mdp" / "SKILL.md"
            skill_file.write_text(
                skill_file.read_text(encoding="utf-8") + "\nDrift.\n", encoding="utf-8"
            )
            errors: list[str] = []

            HARNESS.validate_coverage(
                self.coverage, ROOT / "plugin" / "skills", installed, errors
            )

            self.assertTrue(any("installed skill content drift" in error for error in errors))

    def test_installed_symlink_fails_self_containment(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-installed-skills-") as temp:
            installed = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin" / "skills", installed)
            skill_file = installed / "mdp" / "SKILL.md"
            skill_file.unlink()
            skill_file.symlink_to(ROOT / "plugin" / "skills" / "mdp" / "SKILL.md")
            errors: list[str] = []

            HARNESS.validate_coverage(
                self.coverage, ROOT / "plugin" / "skills", installed, errors
            )

            self.assertTrue(any("symlink found" in error for error in errors))

    def test_host_misroute_and_failed_assertion_fail(self) -> None:
        self.coverage["host_observation_requirements"]["minimum_trigger_accuracy"] = 0.0
        self.coverage["host_observation_requirements"][
            "minimum_output_assertion_accuracy"
        ] = 0.0
        results = self.valid_host_results()
        results["trigger_observations"][0]["selected_skill_id"] = None
        first_output = results["output_observations"][0]
        first_assertion = next(iter(first_output["assertions"]))
        first_output["assertions"][first_assertion] = False

        with tempfile.TemporaryDirectory(prefix="mdp-host-results-") as temp:
            path = Path(temp) / "results.json"
            path.write_text(json.dumps(results), encoding="utf-8")
            errors: list[str] = []
            HARNESS.validate_observed_results(
                path,
                self.triggers,
                self.outputs,
                self.coverage,
                self.skills,
                errors,
            )

        self.assertTrue(any("trigger mismatch" in error for error in errors))
        self.assertTrue(any("required output assertion failed" in error for error in errors))

    def test_duplicate_and_incomplete_host_trials_fail(self) -> None:
        results = self.valid_host_results()
        duplicate = copy.deepcopy(results["trigger_observations"][0])
        results["trigger_observations"] = [duplicate, duplicate]

        with tempfile.TemporaryDirectory(prefix="mdp-host-results-") as temp:
            path = Path(temp) / "results.json"
            path.write_text(json.dumps(results), encoding="utf-8")
            errors: list[str] = []
            HARNESS.validate_observed_results(
                path,
                self.triggers,
                self.outputs,
                self.coverage,
                self.skills,
                errors,
            )

        self.assertTrue(any("duplicate trigger trial" in error for error in errors))
        self.assertTrue(any("missing trigger cases" in error for error in errors))

    def valid_host_results(self) -> dict:
        return {
            "model": "mdp.skill-host-results.v1",
            "host": "test-host",
            "model_id": "test-model",
            "recorded_at": "2026-07-13T00:00:00Z",
            "trigger_observations": [
                {
                    "case_id": case["id"],
                    "trial_id": "trial-1",
                    "selected_skill_id": case.get("expected_skill_id"),
                }
                for case in self.triggers["cases"]
            ],
            "output_observations": [
                {
                    "case_id": case["id"],
                    "trial_id": "trial-1",
                    "assertions": {
                        assertion["id"]: True
                        for assertion in case["assertions"]
                        if assertion.get("required") is True
                    },
                }
                for case in self.outputs["cases"]
            ],
        }


if __name__ == "__main__":
    unittest.main()
