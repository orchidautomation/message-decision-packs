from __future__ import annotations

import importlib.util
import shutil
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("validate-skill-contracts.py")
SPEC = importlib.util.spec_from_file_location("validate_skill_contracts", MODULE_PATH)
module = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(module)


class SkillContractTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        shutil.copytree(Path("plugin/skills"), self.root / "plugin/skills")
        (self.root / "scripts").mkdir()
        for script in Path("scripts").glob("mdp-*.mjs"):
            shutil.copy2(script, self.root / "scripts" / script.name)

    def tearDown(self):
        self.tmp.cleanup()

    def codes(self):
        return {item["code"] for item in module.validate(self.root, self.root / "plugin/skills")["errors"]}

    def test_current_canonical_contract_passes(self):
        self.assertTrue(module.validate(Path("."), Path("plugin/skills"))["valid"])

    def test_core_skill_exposes_job_bound_requirements_handoff(self):
        skill = Path("plugin/skills/mdp/SKILL.md").read_text()
        operator = Path("plugin/skills/mdp/references/cli-operator.md").read_text()
        command = "mdp --json requirements --dir"
        self.assertIn(command, skill)
        self.assertIn(command, operator)
        self.assertIn("available: false", skill)
        self.assertIn("does not collect sources or call a model", skill)

    def test_gtm_brief_preserves_decision_input_and_legacy_normalization_paths(self):
        skill = Path("plugin/skills/mdp-gtm-brief/SKILL.md").read_text()
        mode = Path("plugin/skills/mdp-gtm-brief/references/prospect-fit-or-brief.md").read_text()
        for text in [skill, mode]:
            self.assertIn("data.available", text)
            self.assertIn("data.source_attempt_request_schema", text)
            self.assertIn("`decision_input_contracts` ID/version", text)
            self.assertIn("UTC `as_of`", text)
            self.assertIn("attempt for", text)
            self.assertIn("every compiled attribute", text)
            self.assertIn("exact request", text)
            self.assertIn("bytes", text)
            self.assertIn("mdp.prompt-output.v0", text)
            self.assertIn("normalization_trace.fit_readiness.ready_for_mdp_fit", text)

    def test_pack_builder_preserves_decision_input_and_legacy_validation_paths(self):
        skill = Path("plugin/skills/mdp-pack-builder/SKILL.md").read_text()
        self.assertIn("data.available", skill)
        self.assertIn("attempted-complete source-attempt request", skill)
        self.assertIn("data.source_attempt_request_schema", skill)
        self.assertIn("`decision_input_contracts` ID/version receipts", skill)
        self.assertIn("trusted UTC", skill)
        self.assertIn("attempt for every compiled attribute", skill)
        self.assertIn("--source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON", skill)
        self.assertIn("top-level `outcome` is exactly `ready`", skill)
        self.assertIn("mdp.prompt-output.v0", skill)
        self.assertIn("normalization_trace.fit_readiness.ready_for_mdp_fit", skill)

    def test_bad_frontmatter_and_missing_local_link_fail(self):
        path = self.root / "plugin/skills/mdp/SKILL.md"
        text = path.read_text().replace("name: mdp", "name: wrong").replace("description:", "description: short\nignored:", 1).replace("references/mental-model.md", "references/missing.md")
        path.write_text(text)
        self.assertIn("frontmatter_name_invalid", self.codes())
        self.assertIn("frontmatter_description_invalid", self.codes())
        self.assertIn("skill_link_missing", self.codes())

    def test_skill_local_link_escape_fails(self):
        path = self.root / "plugin/skills/mdp/SKILL.md"
        path.write_text(path.read_text() + "\n[escape](references/../../mdp-pack-builder/SKILL.md)\n")
        self.assertIn("skill_link_escape", self.codes())

    def test_outside_skill_and_load_time_shell_fail(self):
        outside = self.root / "active/rogue/SKILL.md"
        outside.parent.mkdir(parents=True)
        outside.write_text("---\nname: rogue\ndescription: This is a sufficiently long rogue skill.\n---\n")
        path = self.root / "plugin/skills/mdp/SKILL.md"
        path.write_text(path.read_text() + "\n`SKILL_DIR=$(pwd)`\n")
        self.assertIn("authored_skill_outside_canonical_root", self.codes())
        self.assertIn("load_time_shell_hazard", self.codes())

    def test_each_high_risk_proposal_guardrail_is_enforced(self):
        path = self.root / "plugin/skills/mdp-proposal-review/SKILL.md"
        original = path.read_text()
        for guardrail, phrase in module.PROPOSAL_GUARDRAILS.items():
            with self.subTest(guardrail=guardrail):
                path.write_text(original.replace(phrase, "REMOVED_GUARDRAIL", 1))
                self.assertIn(f"proposal_guardrail_missing:{guardrail}", self.codes())
                path.write_text(original)

    def test_stale_runner_name_fails(self):
        review = self.root / "plugin/skills/mdp-proposal-review/SKILL.md"
        review.write_text(review.read_text() + "\n`scripts/mdp-missing-runner.mjs`\n")
        self.assertIn("runner_script_reference_stale", self.codes())

    def test_each_proposal_authoring_guardrail_is_enforced(self):
        authoring = self.root / "plugin/skills/mdp-pack-builder/references/proposal-authoring.md"
        original = authoring.read_text()
        for guardrail, phrase in module.AUTHORING_GUARDRAILS.items():
            with self.subTest(guardrail=guardrail):
                authoring.write_text(original.replace(phrase, "REMOVED_GUARDRAIL", 1))
                self.assertIn(f"proposal_authoring_guardrail_missing:{guardrail}", self.codes())
                authoring.write_text(original)


if __name__ == "__main__":
    unittest.main()
