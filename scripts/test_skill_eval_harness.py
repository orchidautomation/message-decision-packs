#!/usr/bin/env python3
"""Mutation tests for the MDP skill eval gate."""

from __future__ import annotations

import copy
import importlib.util
import json
import re
import shutil
import sys
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

    def test_query_shapes_require_semantic_markers(self) -> None:
        mutations = (
            ("mdp-operator-typo-train", "direct"),
            ("mdp-operator-indirect-train", "direct"),
            ("mdp-operator-train", "typo"),
        )
        for case_id, query_shape in mutations:
            payload = copy.deepcopy(self.triggers)
            case = next(row for row in payload["cases"] if row["id"] == case_id)
            case["query_shape"] = query_shape
            errors: list[str] = []

            HARNESS.validate_triggers(
                payload, self.coverage, self.skills, self.definitions, errors
            )

            self.assertTrue(
                any(case_id in error and "query_shape" in error for error in errors),
                msg=f"missing semantic query-shape error for {case_id}: {errors}",
            )

        payload = copy.deepcopy(self.triggers)
        case = next(row for row in payload["cases"] if row["id"] == "mdp-operator-typo-train")
        case["query"] = "What does the MDP command tell an agent?"
        errors = []
        HARNESS.validate_triggers(
            payload, self.coverage, self.skills, self.definitions, errors
        )
        self.assertTrue(any("query_shape typo requires" in error for error in errors))

        payload = copy.deepcopy(self.triggers)
        case = next(row for row in payload["cases"] if row["id"] == "mdp-operator-train")
        case["query"] = "This is not a routable request."
        errors = []
        HARNESS.validate_triggers(
            payload, self.coverage, self.skills, self.definitions, errors
        )
        self.assertTrue(any("query_shape direct requires" in error for error in errors))

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
        self.assertEqual(summary["indexes"], 4)
        self.assertEqual(errors, [])

    def test_communication_assertions_cover_every_skill_split(self) -> None:
        payload = copy.deepcopy(self.outputs)
        for case in payload["cases"]:
            if case["skill_id"] == "mdp" and case["split"] == "train":
                case["assertions"] = [
                    assertion
                    for assertion in case["assertions"]
                    if assertion["category"] != "communication"
                ]
        errors: list[str] = []

        HARNESS.validate_outputs(
            payload, self.coverage, self.skills, self.definitions, errors
        )

        self.assertIn("output communication coverage missing mdp/train", errors)

    def test_high_risk_apply_modes_require_safety_and_human_review(self) -> None:
        expected = {"evidence", "safety", "human-review"}
        governed_modes = {"bid-no-bid", "compliance", "proof", "red-team"}
        apply_definition = self.definitions["mdp-pack-apply"]
        per_mode = apply_definition["required_assertion_categories_by_mode"]
        self.assertEqual(
            {mode: set(per_mode[mode]) for mode in governed_modes},
            {mode: expected for mode in governed_modes},
        )

        for missing_category in ("safety", "human-review"):
            payload = copy.deepcopy(self.outputs)
            case = next(
                row
                for row in payload["cases"]
                if row["skill_id"] == "mdp-pack-apply" and row["mode"] == "bid-no-bid"
            )
            case["assertions"] = [
                assertion
                for assertion in case["assertions"]
                if assertion["category"] != missing_category
            ]
            errors: list[str] = []

            HARNESS.validate_outputs(
                payload, self.coverage, self.skills, self.definitions, errors
            )

            self.assertTrue(
                any(
                    case["id"] in error
                    and "missing required assertion categories" in error
                    and missing_category in error
                    for error in errors
                ),
                msg=f"missing {missing_category} was not rejected: {errors}",
            )

    def test_governed_apply_modes_cannot_be_downgraded_by_coordinated_edit(self) -> None:
        payload = copy.deepcopy(self.coverage)
        payload["output_requirements"]["allowed_assertion_categories"] = ["evidence"]
        definition = next(row for row in payload["skills"] if row["id"] == "mdp-pack-apply")
        for mode in ("bid-no-bid", "compliance", "proof", "red-team"):
            definition["required_assertion_categories_by_mode"][mode] = ["evidence"]
        errors: list[str] = []

        HARNESS.validate_coverage(payload, ROOT / "plugin" / "skills", None, errors)

        for mode in ("bid-no-bid", "compliance", "proof", "red-team"):
            self.assertTrue(
                any(
                    f"mdp-pack-apply/{mode} must require governed assertion categories"
                    in error
                    for error in errors
                ),
                msg=f"coordinated downgrade of {mode} was accepted: {errors}",
            )

    def test_governed_modes_survive_skill_id_rename(self) -> None:
        payload = copy.deepcopy(self.coverage)
        definition = next(row for row in payload["skills"] if row["id"] == "mdp-pack-apply")
        definition["id"] = "renamed-pack-apply"
        definition["eval_index"] = "plugin/skills/renamed-pack-apply/evals/index.json"
        for mode in ("bid-no-bid", "compliance", "proof", "red-team"):
            definition["required_assertion_categories_by_mode"][mode] = ["evidence"]
        errors: list[str] = []

        HARNESS.validate_coverage(payload, ROOT / "plugin" / "skills", None, errors)

        for mode in ("bid-no-bid", "compliance", "proof", "red-team"):
            self.assertTrue(
                any(
                    f"renamed-pack-apply/{mode} must require governed assertion categories"
                    in error
                    for error in errors
                ),
                msg=f"renaming the skill bypassed {mode} governance: {errors}",
            )

    def test_high_risk_per_mode_contract_must_be_complete(self) -> None:
        payload = copy.deepcopy(self.coverage)
        definition = next(row for row in payload["skills"] if row["id"] == "mdp-pack-apply")
        del definition["required_assertion_categories_by_mode"]["red-team"]
        errors: list[str] = []

        HARNESS.validate_coverage(payload, ROOT / "plugin" / "skills", None, errors)

        self.assertIn(
            "coverage.json: mdp-pack-apply per-mode assertion categories must cover every mode exactly",
            errors,
        )

    def test_required_category_contracts_reject_malformed_values_without_crashing(self) -> None:
        mutations = (
            ("base object", "required_assertion_categories", {"evidence": True}),
            ("base unhashable", "required_assertion_categories", [["evidence"]]),
            (
                "per-mode object",
                "required_assertion_categories_by_mode",
                ["evidence"],
            ),
        )
        for label, field, value in mutations:
            with self.subTest(label=label):
                coverage = copy.deepcopy(self.coverage)
                definition = next(
                    row for row in coverage["skills"] if row["id"] == "mdp-pack-apply"
                )
                definition[field] = value
                definitions = {row["id"]: row for row in coverage["skills"]}
                errors: list[str] = []

                HARNESS.validate_coverage(
                    coverage, ROOT / "plugin" / "skills", None, errors
                )
                HARNESS.validate_outputs(
                    self.outputs, coverage, self.skills, definitions, errors
                )

                self.assertTrue(
                    any("assertion categories" in error for error in errors),
                    msg=f"malformed {label} contract was accepted: {errors}",
                )

        coverage = copy.deepcopy(self.coverage)
        definition = next(
            row for row in coverage["skills"] if row["id"] == "mdp-pack-apply"
        )
        definition["required_assertion_categories_by_mode"]["bid-no-bid"] = [
            ["evidence"]
        ]
        definitions = {row["id"]: row for row in coverage["skills"]}
        errors = []

        HARNESS.validate_coverage(coverage, ROOT / "plugin" / "skills", None, errors)
        HARNESS.validate_outputs(
            self.outputs, coverage, self.skills, definitions, errors
        )

        self.assertTrue(
            any("mdp-pack-apply/bid-no-bid" in error for error in errors),
            msg=f"unhashable per-mode category was accepted: {errors}",
        )

        outputs = copy.deepcopy(self.outputs)
        outputs["cases"][0]["assertions"][0]["category"] = ["evidence"]
        errors = []

        HARNESS.validate_outputs(
            outputs, self.coverage, self.skills, self.definitions, errors
        )

        self.assertTrue(
            any("invalid assertion category" in error for error in errors),
            msg=f"unhashable output category crashed or was accepted: {errors}",
        )

    def test_per_mode_requirements_extend_base_requirements(self) -> None:
        coverage = copy.deepcopy(self.coverage)
        definition = next(
            row for row in coverage["skills"] if row["id"] == "mdp-pack-apply"
        )
        definition["required_assertion_categories_by_mode"]["bid-no-bid"] = [
            "safety",
            "human-review",
        ]
        definitions = {row["id"]: row for row in coverage["skills"]}
        outputs = copy.deepcopy(self.outputs)
        case = next(
            row
            for row in outputs["cases"]
            if row["skill_id"] == "mdp-pack-apply" and row["mode"] == "bid-no-bid"
        )
        case["assertions"] = [
            assertion
            for assertion in case["assertions"]
            if assertion["category"] != "evidence"
        ]
        errors: list[str] = []

        HARNESS.validate_outputs(
            outputs, coverage, self.skills, definitions, errors
        )

        self.assertTrue(
            any(
                case["id"] in error
                and "missing required assertion categories" in error
                and "evidence" in error
                for error in errors
            ),
            msg=f"per-mode requirements replaced the base requirement: {errors}",
        )

    def test_trigger_and_documented_revisions_match_coverage(self) -> None:
        payload = copy.deepcopy(self.triggers)
        payload["revision"] = "stale-revision"
        errors: list[str] = []

        HARNESS.validate_triggers(
            payload, self.coverage, self.skills, self.definitions, errors
        )

        self.assertIn("trigger-cases.json: revision must match coverage.json", errors)
        documentation = (ROOT / "docs" / "skill-progressive-disclosure.md").read_text(
            encoding="utf-8"
        )
        documented_revision = re.search(
            r"trigger-cases\.json` and `coverage\.json` carry revision\s+`([^`]+)`",
            documentation,
        )
        self.assertIsNotNone(documented_revision)
        self.assertEqual(documented_revision.group(1), self.coverage["revision"])

    def test_trigger_and_coverage_revisions_must_be_non_empty_strings(self) -> None:
        mutations = (
            (None, None),
            ("", ""),
            ("   ", "   "),
            ("mdp-249.v1", None),
            (None, "mdp-249.v1"),
        )
        for trigger_revision, coverage_revision in mutations:
            with self.subTest(
                trigger_revision=trigger_revision,
                coverage_revision=coverage_revision,
            ):
                triggers = copy.deepcopy(self.triggers)
                coverage = copy.deepcopy(self.coverage)
                if trigger_revision is None:
                    triggers.pop("revision", None)
                else:
                    triggers["revision"] = trigger_revision
                if coverage_revision is None:
                    coverage.pop("revision", None)
                else:
                    coverage["revision"] = coverage_revision
                errors: list[str] = []

                HARNESS.validate_triggers(
                    triggers, coverage, self.skills, self.definitions, errors
                )

                if not isinstance(trigger_revision, str) or not trigger_revision.strip():
                    self.assertIn(
                        "trigger-cases.json: revision must be a non-empty string",
                        errors,
                    )
                if not isinstance(coverage_revision, str) or not coverage_revision.strip():
                    self.assertIn(
                        "coverage.json: revision must be a non-empty string", errors
                    )

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

    def test_host_result_observations_reject_raw_fields(self) -> None:
        results = self.valid_host_results()
        results["transcript"] = "synthetic transcript"
        results["trigger_observations"][0]["contact_email"] = "synthetic@example.test"
        results["output_observations"][0]["raw_output"] = "synthetic output"
        with tempfile.TemporaryDirectory(prefix="mdp-host-results-") as temp:
            path = Path(temp) / "results.json"
            path.write_text(json.dumps(results), encoding="utf-8")
            errors: list[str] = []
            HARNESS.validate_observed_results(
                path, self.triggers, self.outputs, self.coverage, self.skills, errors
            )
        self.assertTrue(any("host result contains unsupported or raw fields" in error for error in errors))
        self.assertTrue(any("trigger observation contains unsupported or raw fields" in error for error in errors))
        self.assertTrue(any("output observation contains unsupported or raw fields" in error for error in errors))

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
    # The focused validation contract accepts --skill for parity with the
    # operator command in the implementation plan. The harness mutation suite
    # validates the shared corpus and indexes together, so the option narrows
    # the requested report label without changing those cross-skill checks.
    if "--skill" in sys.argv:
        index = sys.argv.index("--skill")
        if index + 1 >= len(sys.argv):
            raise SystemExit("--skill requires a skill id")
        skill_id = sys.argv[index + 1]
        if skill_id not in {"mdp", "mdp-pack-apply", "mdp-pack-review"}:
            raise SystemExit(f"unknown skill: {skill_id}")
        del sys.argv[index : index + 2]
    unittest.main()
