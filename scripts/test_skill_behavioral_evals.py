#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("behavioral", ROOT / "scripts/run-skill-behavioral-evals.py")
assert SPEC and SPEC.loader
MOD = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MOD)


class BehavioralEvalTests(unittest.TestCase):
    def test_suite_covers_required_trigger_shapes(self) -> None:
        suite = MOD.load(ROOT / "plugin/skill-evals/behavioral-suite.json")
        corpus = MOD.load(ROOT / "plugin/skill-evals/trigger-cases.json")
        cases = MOD.selected_cases(corpus, suite["trigger_case_ids"])
        self.assertGreaterEqual(suite["trigger_repeats"], 2)
        self.assertIn("typo", {row["query_shape"] for row in cases})
        self.assertTrue({"positive", "near-miss", "out-of-scope", "profile-crossing"} <= {row["case_type"] for row in cases})
        collisions = {row["case_id"] for row in corpus["collisions"]}
        self.assertTrue(collisions & set(suite["trigger_case_ids"]))
        self.assertTrue(any(row["split"] == "validation" for row in cases))
        self.assertTrue(suite["input_files"]["null-gtm-on-proposal-validation"])
        self.assertEqual(set(suite["comparison_modes"]), MOD.COMPARISON_MODES if hasattr(MOD, "COMPARISON_MODES") else {"with-skill", "baseline", "previous-version"})

    def test_aggregate_excludes_raw_prompt_and_output(self) -> None:
        raw = {"trials": [{"comparison_mode": "with-skill", "case_id": "x", "kind": "output", "repeat": 1, "passed": True, "elapsed_ms": 10, "total_tokens": 20, "prompt": "private", "output": {"response": "private"}}]}
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source, target = root / "raw.json", root / "report.json"
            source.write_bytes(MOD.canonical_bytes(raw))
            MOD.aggregate(type("Args", (), {"results": source, "out": target})())
            report = target.read_text()
        self.assertNotIn("private", report)
        self.assertIn('"observed":true', report)
        self.assertIn('"static_validation_separate":true', report)

    def test_materialized_views_keep_shared_corpus_authority(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / "views"
            MOD.materialize(type("Args", (), {
                "suite": ROOT / "plugin/skill-evals/behavioral-suite.json",
                "corpus": ROOT / "plugin/skill-evals", "out": out,
            })())
            manifest = MOD.load(out / "manifest.json")
            self.assertEqual(manifest["model"], "mdp.agent-skills-eval-views.v1")
            for skill_id in manifest["skills"]:
                view = MOD.load(out / skill_id / "evals.json")
                self.assertEqual(view["skill_name"], skill_id)
                self.assertEqual(view["source_model"], "mdp.skill-output-corpus.v1")

    def test_subject_prompt_is_not_coached_by_assertions(self) -> None:
        outputs = MOD.load(ROOT / "plugin/skill-evals/output-cases.json")
        case = next(row for row in outputs["cases"] if row["id"] == "gtm-fit-validation")
        prompt, _ = MOD.prompt_for(case, "output", "baseline", ROOT / "plugin/skills", None)
        self.assertNotIn(case["assertions"][0]["criterion"], prompt)
        self.assertNotIn("Assertions:", prompt)

    def test_direct_shared_skill_references_are_loaded(self) -> None:
        _, inputs = MOD.skill_material(ROOT / "plugin/skills", "mdp-pack-apply")
        paths = {row["path"] for row in inputs}
        self.assertIn("mdp-pack-apply/references/communication-contract.md", paths)
        self.assertIn("mdp-pack-apply/references/workflow-bundle-handoff.md", paths)


if __name__ == "__main__":
    unittest.main()
