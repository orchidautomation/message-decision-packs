#!/usr/bin/env python3
"""Mutation tests for skill and shared-eval packaging parity."""

from __future__ import annotations

import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "validate_skill_packaging", ROOT / "scripts" / "validate-skill-packaging.py"
)
assert SPEC and SPEC.loader
PACKAGING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PACKAGING)


class SkillPackagingMutationTests(unittest.TestCase):
    def expected_skills(self) -> list[str]:
        errors: list[str] = []
        skills = PACKAGING.canonical_skill_inventory(
            ROOT / "plugin/skill-inventory.json", errors
        )
        self.assertEqual(errors, [])
        return skills

    def run_validator(self, source: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/validate-skill-packaging.py"),
                "--source",
                str(source),
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )

    def test_canonical_inventory_is_the_expected_four(self) -> None:
        self.assertEqual(
            self.expected_skills(),
            ["mdp", "mdp-pack-apply", "mdp-pack-builder", "mdp-pack-review"],
        )

    def test_missing_source_fails_with_json_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-missing-source-") as temp:
            result = self.run_validator(Path(temp) / "missing")
        payload = json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(payload["valid"])
        self.assertEqual(len(payload["errors"]), 1)
        self.assertIn("missing skill root", payload["errors"][0])
        self.assertNotIn("Traceback", result.stderr)

    def test_regular_file_source_fails_with_json_without_traceback(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-file-source-") as temp:
            source = Path(temp) / "skills"
            source.write_text("not a directory\n", encoding="utf-8")
            result = self.run_validator(source)
        payload = json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(payload["valid"])
        self.assertEqual(len(payload["errors"]), 1)
        self.assertIn("missing skill root", payload["errors"][0])
        self.assertNotIn("Traceback", result.stderr)

    def test_unexpected_fifth_authored_skill_fails_allowlist(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-fifth-skill-") as temp:
            source = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin/skills", source)
            extra = source / "unexpected-skill"
            extra.mkdir()
            (extra / "SKILL.md").write_text(
                "---\nname: unexpected-skill\ndescription: mutation fixture\n---\n",
                encoding="utf-8",
            )
            result = self.run_validator(source)
        payload = json.loads(result.stdout)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(payload["valid"])
        self.assertTrue(
            any("authored skill inventory drift" in error for error in payload["errors"])
        )
        self.assertNotIn("Traceback", result.stderr)

    def test_source_eval_indexes_are_valid(self) -> None:
        errors: list[str] = []
        PACKAGING.validate_source_eval_indexes(
            ROOT / "plugin/skills", ROOT / "plugin/skill-evals", self.expected_skills(), errors
        )
        self.assertEqual(errors, [])

    def test_managed_workflow_handoff_is_private_explicit_and_bounded(self) -> None:
        reference = (ROOT / "plugin/skills/mdp/references/workflow-bundle-handoff.md").read_text()
        for marker in (
            "current-user-only",
            "prepare-run",
            "verify-run",
            "explicit run directory",
            "ambient/latest",
            "timeout",
            "cancellation",
            "Advanced explicit-artifact parity",
            "source bodies",
            "run_directory:",
            "retention:",
        ):
            self.assertIn(marker, reference)
        for relative_path in (
            "plugin/skills/mdp/SKILL.md",
            "plugin/skills/mdp-pack-apply/SKILL.md",
            "plugin/skills/mdp-pack-review/SKILL.md",
        ):
            text = (ROOT / relative_path).read_text()
            self.assertIn("managed", text.lower())
            self.assertIn("explicit run directory", text.lower())
            self.assertIn("ambient", text.lower())

    def test_missing_corpus_file_fails_bundle_parity(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-packaging-") as temp:
            bundle = Path(temp) / "skill-evals"
            shutil.copytree(ROOT / "plugin/skill-evals", bundle)
            (bundle / "output-cases.json").unlink()
            errors: list[str] = []
            PACKAGING.compare_tree(
                ROOT / "plugin/skill-evals", bundle, "test skill-evals", errors
            )
        self.assertTrue(any("missing canonical file: output-cases.json" in error for error in errors))

    def test_changed_index_fails_bundle_parity(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-packaging-") as temp:
            bundle = Path(temp) / "skill-evals"
            shutil.copytree(ROOT / "plugin/skill-evals", bundle)
            index = bundle / ".." / "skills" / "mdp" / "evals" / "index.json"
            index.parent.mkdir(parents=True)
            shutil.copy2(ROOT / "plugin/skills/mdp/evals/index.json", index)
            index.write_text(index.read_text(encoding="utf-8") + "\n", encoding="utf-8")
            errors: list[str] = []
            PACKAGING.compare_tree(
                ROOT / "plugin/skills/mdp/evals",
                index.parent,
                "test index",
                errors,
            )
        self.assertTrue(any("content drift: index.json" in error for error in errors))

    def test_extra_installed_case_fails_bundle_parity(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-packaging-") as temp:
            bundle = Path(temp) / "skill-evals"
            shutil.copytree(ROOT / "plugin/skill-evals", bundle)
            (bundle / "extra-case.json").write_text("{}\n", encoding="utf-8")
            errors: list[str] = []
            PACKAGING.compare_tree(
                ROOT / "plugin/skill-evals", bundle, "test skill-evals", errors
            )
        self.assertTrue(any("non-canonical file: extra-case.json" in error for error in errors))

    def test_symlinked_skill_file_fails_bundle_parity(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-packaging-") as temp:
            bundle = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin/skills", bundle)
            skill_file = bundle / "mdp" / "SKILL.md"
            skill_file.unlink()
            skill_file.symlink_to(ROOT / "plugin/skills/mdp/SKILL.md")
            errors: list[str] = []
            PACKAGING.compare_bundle(ROOT / "plugin/skills", bundle, "test", errors)
        self.assertTrue(any("symlink is not allowed" in error for error in errors))

    def test_each_skill_passes_isolated_portable_layout(self) -> None:
        errors: list[str] = []
        PACKAGING.validate_portable_skill_layout(ROOT / "plugin/skills", errors)
        self.assertEqual(errors, [])

    def test_shared_portable_references_match_canonical_bytes(self) -> None:
        errors: list[str] = []
        PACKAGING.validate_shared_reference_parity(
            ROOT / "plugin/skills", self.expected_skills(), errors
        )
        self.assertEqual(errors, [])

    def test_shared_reference_drift_fails_packaging(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-shared-") as temp:
            skills = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin/skills", skills)
            projected = skills / "mdp-pack-apply/references/communication-contract.md"
            projected.write_text(projected.read_text() + "\ndrift\n")
            errors: list[str] = []
            PACKAGING.validate_shared_reference_parity(
                skills, self.expected_skills(), errors
            )
        self.assertTrue(any("shared reference drift" in error for error in errors))

    def test_cross_skill_link_fails_portable_layout(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-portable-") as temp:
            skills = Path(temp) / "skills"
            shutil.copytree(ROOT / "plugin/skills", skills)
            entrypoint = skills / "mdp-pack-review" / "SKILL.md"
            entrypoint.write_text(
                entrypoint.read_text()
                + "\n[cross-skill](../mdp/references/communication-contract.md)\n"
            )
            errors: list[str] = []
            PACKAGING.validate_portable_skill_layout(skills, errors)
        self.assertTrue(any("escapes isolated root" in error for error in errors))

    def test_native_bundle_requires_referenced_helpers(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-native-") as temp:
            bundle = Path(temp) / "bundle"
            (bundle / "scripts").mkdir(parents=True)
            errors: list[str] = []
            PACKAGING.validate_native_helpers(
                ROOT / "plugin/skills", bundle, "test", errors
            )
        self.assertTrue(any("missing referenced helper" in error for error in errors))

    def test_agent_plugins_bundle_accepts_only_four_canonical_skills(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-agent-plugins-") as temp:
            dist = Path(temp) / "dist"
            portable = dist / "agent-plugins"
            shutil.copytree(ROOT / "plugin/skills", portable / "skills")
            (portable / "plugin.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                        "name": "message-decision-packs",
                        "version": "0.1.101",
                        "license": "Elastic-2.0",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            errors: list[str] = []
            PACKAGING.validate_agent_plugins_bundle(
                ROOT / "plugin/skills", dist, self.expected_skills(), errors
            )
        self.assertEqual(errors, [])

    def test_agent_plugins_bundle_rejects_unexpected_fifth_skill(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-agent-plugins-fifth-") as temp:
            dist = Path(temp) / "dist"
            portable = dist / "agent-plugins"
            shutil.copytree(ROOT / "plugin/skills", portable / "skills")
            extra = portable / "skills/unexpected-skill"
            extra.mkdir()
            (extra / "SKILL.md").write_text(
                "---\nname: unexpected-skill\ndescription: mutation fixture\n---\n",
                encoding="utf-8",
            )
            (portable / "plugin.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                        "name": "message-decision-packs",
                        "version": "0.1.107",
                        "license": "Elastic-2.0",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            errors: list[str] = []
            PACKAGING.validate_agent_plugins_bundle(
                ROOT / "plugin/skills", dist, self.expected_skills(), errors
            )
        self.assertTrue(
            any("agent-plugins skill inventory drift" in error for error in errors)
        )

    def test_agent_plugins_bundle_rejects_native_payload_and_false_mcp(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-agent-plugins-") as temp:
            dist = Path(temp) / "dist"
            portable = dist / "agent-plugins"
            shutil.copytree(ROOT / "plugin/skills", portable / "skills")
            (portable / "plugin.json").write_text(
                json.dumps(
                    {
                        "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
                        "name": "message-decision-packs",
                        "version": "0.1.101",
                        "license": "Elastic-2.0",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            (portable / "hooks").mkdir()
            (portable / "mcp.json").write_text("{}\n", encoding="utf-8")
            errors: list[str] = []
            PACKAGING.validate_agent_plugins_bundle(
                ROOT / "plugin/skills", dist, self.expected_skills(), errors
            )
        self.assertTrue(any("native-only or unexpected" in error for error in errors))
        self.assertTrue(any("must not claim mcp.json" in error for error in errors))

    def test_wrong_destination_is_detected(self) -> None:
        with tempfile.TemporaryDirectory(prefix="mdp-packaging-") as temp:
            wrong_root = Path(temp) / "assets" / "skill-evals"
            wrong_root.parent.mkdir()
            shutil.copytree(ROOT / "plugin/skill-evals", wrong_root)
            errors: list[str] = []
            PACKAGING.compare_tree(
                ROOT / "plugin/skill-evals", Path(temp) / "skill-evals", "wrong destination", errors
            )
        self.assertTrue(any("missing generated directory" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
