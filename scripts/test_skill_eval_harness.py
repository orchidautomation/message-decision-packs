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

    def test_skill_indexes_are_complete_and_disjoint(self) -> None:
        errors: list[str] = []
        summary = HARNESS.validate_skill_eval_indexes(
            ROOT / "plugin" / "skills",
            ROOT / "plugin" / "skill-evals",
            self.triggers,
            self.outputs,
            self.skills,
            self.definitions,
            errors,
        )
        self.assertEqual(summary["indexes"], 5)
        self.assertEqual(errors, [])

    def test_missing_index_and_installed_corpus_drift_fail(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-installed-evals-") as temp:
            installed_skills = Path(temp) / "skills"
            installed_corpus = Path(temp) / "skill-evals"
            shutil.copytree(ROOT / "plugin" / "skills", installed_skills)
            shutil.copytree(ROOT / "plugin" / "skill-evals", installed_corpus)
            (installed_skills / "mdp" / "evals" / "index.json").unlink()
            (installed_corpus / "coverage.json").write_text("{}\n", encoding="utf-8")
            errors: list[str] = []
            HARNESS.validate_skill_eval_indexes(
                ROOT / "plugin" / "skills",
                ROOT / "plugin" / "skill-evals",
                self.triggers,
                self.outputs,
                self.skills,
                self.definitions,
                errors,
                installed_skills,
                installed_corpus,
            )
        self.assertTrue(any("installed skill-evals: content drift" in error for error in errors))
        self.assertTrue(any("installed skills missing canonical file" in error for error in errors))

    def test_recording_metadata_rejects_raw_fields_and_negative_metrics(self) -> None:
        results = self.valid_host_results()
        results["recording"]["elapsed_ms"] = -1
        results["recording"]["transcript"] = "synthetic raw transcript must not be accepted"
        with tempfile.TemporaryDirectory(prefix="mdp-host-results-") as temp:
            path = Path(temp) / "results.json"
            path.write_text(json.dumps(results), encoding="utf-8")
            errors: list[str] = []
            HARNESS.validate_observed_results(
                path, self.triggers, self.outputs, self.coverage, self.skills, errors
            )
        self.assertTrue(any("bounded non-negative integer" in error for error in errors))
        self.assertTrue(any("unsupported or raw fields" in error for error in errors))

    def test_comparison_requires_matching_pair_id(self) -> None:
        primary = self.valid_host_results()
        baseline = copy.deepcopy(primary)
        primary["recording"]["comparison_mode"] = "with-skill"
        baseline["recording"]["comparison_mode"] = "baseline"
        baseline["recording"]["comparison_id"] = "different-pair"
        with tempfile.TemporaryDirectory(prefix="mdp-host-comparison-") as temp:
            primary_path = Path(temp) / "primary.json"
            baseline_path = Path(temp) / "baseline.json"
            primary_path.write_text(json.dumps(primary), encoding="utf-8")
            baseline_path.write_text(json.dumps(baseline), encoding="utf-8")
            errors: list[str] = []
            primary_summary = HARNESS.validate_observed_results(
                primary_path,
                self.triggers,
                self.outputs,
                self.coverage,
                self.skills,
                errors,
                expected_comparison_mode="with-skill",
            )
            HARNESS.validate_comparison_results(
                primary_summary,
                baseline_path,
                None,
                self.triggers,
                self.outputs,
                self.coverage,
                self.skills,
                errors,
            )
        self.assertTrue(any("comparison_id does not match" in error for error in errors))

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
            "recording": {
                "comparison_mode": "with-skill",
                "comparison_id": "synthetic-pair-1",
                "source_revision": "synthetic-public-ref",
                "elapsed_ms": 123,
                "input_tokens": 100,
                "output_tokens": 200,
            },
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
