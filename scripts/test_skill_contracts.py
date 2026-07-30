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

    def test_existing_skills_route_provider_neutral_source_binding(self):
        core = Path("plugin/skills/mdp/SKILL.md").read_text()
        operator = Path("plugin/skills/mdp/references/cli-operator.md").read_text()
        builder = Path("plugin/skills/mdp-pack-builder/SKILL.md").read_text()
        review = Path("plugin/skills/mdp-pack-review/SKILL.md").read_text()
        command = "mdp --json validate-source-binding"
        for text in [core, operator, builder, review]:
            self.assertIn(command, text)
        self.assertIn("outside", core)
        self.assertIn("field-key reuse", operator)
        self.assertIn("provider enums", builder)
        self.assertIn("External field", review)

    def test_gtm_brief_preserves_decision_input_and_legacy_normalization_paths(self):
        skill = Path("plugin/skills/mdp-gtm-brief/SKILL.md").read_text()
        mode = Path("plugin/skills/mdp-gtm-brief/references/prospect-fit-or-brief.md").read_text()
        for text in [skill, mode]:
            normalized = " ".join(text.split())
            self.assertIn("data.available", text)
            self.assertIn("data.source_attempt_request_schema", text)
            self.assertIn("data.collected_attempt_results_schema", text)
            self.assertIn("`decision_input_contracts` ID/version", text)
            self.assertIn("UTC `as_of`", text)
            self.assertIn("execute every compiled attempt", normalized)
            self.assertIn("exact request", text)
            self.assertIn("bytes", text)
            self.assertIn("mdp.prompt-output.v0", text)
            self.assertIn("normalization_trace.fit_readiness.ready_for_mdp_fit", text)

    def test_gtm_brief_constructs_and_hashes_attempts_before_normalization(self):
        mode = Path("plugin/skills/mdp-gtm-brief/references/prospect-fit-or-brief.md").read_text()
        requirements = mode.index("mdp --json requirements")
        missing = mode.index("If any artifact is missing")
        handoff = mode.index("complete `mdp --json requirements` result")
        stop = mode.index("bound prompt; then stop")
        instantiate = mode.index("Require the host to instantiate the request")
        preserve = mode.index("preserve those exact request")
        collect = mode.index("execute every compiled attempt")
        normalize = mode.index("Invoke the bound prompt")
        resume = mode.index("Resume only when the host")
        validate = mode.index("For either the already-supplied or resumed path")
        self.assertLess(requirements, missing)
        self.assertLess(missing, handoff)
        self.assertLess(handoff, stop)
        self.assertLess(stop, instantiate)
        self.assertLess(instantiate, preserve)
        self.assertLess(preserve, collect)
        self.assertLess(preserve, normalize)
        self.assertLess(collect, normalize)
        self.assertLess(normalize, resume)
        self.assertLess(resume, validate)

    def test_gtm_brief_requires_host_supplied_attempt_ledger(self):
        skill = Path("plugin/skills/mdp-gtm-brief/SKILL.md").read_text()
        self.assertIn("do not collect or normalize inside this skill", skill)
        self.assertIn("complete `mdp --json requirements` result", skill)
        self.assertIn("`data.source_attempt_request_schema`", skill)
        self.assertIn("`data.collected_attempt_results_schema`", skill)
        self.assertIn("`data.normalized_output_schema`", skill)
        self.assertIn("contract/prompt receipts", skill)
        self.assertIn("`raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`", skill)
        self.assertIn(
            "`decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`",
            skill,
        )
        self.assertIn(
            "`source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`",
            skill,
        )
        self.assertIn(
            "`collected_attempt_results_sha256`:", skill
        )
        self.assertIn("`COLLECTED_ATTEMPT_RESULTS_SHA256`", skill)
        self.assertIn(
            "--collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON", skill
        )
        self.assertIn("returned bound prompt; then", skill)
        self.assertIn("Resume only after the host returns", skill)
        self.assertIn("--prompt BOUND_PROMPT_PATH", skill)

    def test_gtm_brief_validates_resumed_artifacts_without_rehandoff(self):
        for path in [
            "plugin/skills/mdp-gtm-brief/SKILL.md",
            "plugin/skills/mdp-gtm-brief/references/prospect-fit-or-brief.md",
        ]:
            text = Path(path).read_text()
            supplied = text.index("If all three artifacts")
            collected = text.index("`COLLECTED_ATTEMPT_RESULTS_JSON`", supplied)
            output = text.index("`OUTPUT_JSON`", collected)
            immediate = text.index("validate them immediately", supplied)
            missing = text.index("If any artifact is missing", immediate)
            handoff = text.index("DECISION_INPUT_REQUIREMENTS_JSON", missing)
            self.assertLess(supplied, immediate)
            self.assertLess(supplied, collected)
            self.assertLess(collected, output)
            self.assertLess(output, immediate)
            self.assertLess(immediate, missing)
            self.assertLess(missing, handoff)

    def test_pack_builder_preserves_decision_input_and_legacy_validation_paths(self):
        skill = Path("plugin/skills/mdp-pack-builder/SKILL.md").read_text()
        self.assertIn("data.available", skill)
        self.assertIn("`output_contract.output_kind`", skill)
        self.assertIn("`decision-input-normalization`", skill)
        self.assertIn("regardless of", skill)
        self.assertIn("job-wide `data.available`", skill)
        self.assertIn("prospect-normalization prompt", skill)
        self.assertIn("card-patch/extraction envelopes", skill)
        self.assertIn("does not declare", skill)
        self.assertIn("attempted-complete `SOURCE_ATTEMPT_REQUEST_JSON`", skill)
        self.assertIn("data.source_attempt_request_schema", skill)
        self.assertIn("`data.collected_attempt_results_schema`", skill)
        self.assertIn("`data.normalized_output_schema`", skill)
        self.assertIn("`decision_input_contracts` ID/version receipts", skill)
        self.assertIn("trusted UTC", skill)
        self.assertIn("execute every compiled attempt", skill)
        self.assertIn("`raw_row`: `COLLECTED_ATTEMPT_RESULTS_JSON`", skill)
        self.assertIn(
            "`decision_input_requirements`: `DECISION_INPUT_REQUIREMENTS_JSON.data`",
            skill,
        )
        self.assertIn(
            "`source_attempt_request_sha256`: `SOURCE_ATTEMPT_REQUEST_SHA256`",
            skill,
        )
        self.assertIn("`collected_attempt_results_sha256`:", skill)
        self.assertIn("--source-attempt-request SOURCE_ATTEMPT_REQUEST_JSON", skill)
        self.assertIn(
            "--collected-attempt-results COLLECTED_ATTEMPT_RESULTS_JSON", skill
        )

    def test_core_skill_distinguishes_legacy_normalization_from_extraction(self):
        skill = Path("plugin/skills/mdp/SKILL.md").read_text()
        normalized = " ".join(skill.split())
        self.assertIn("regardless of job-wide `data.available`", normalized)
        self.assertIn("prospect-normalization prompt", skill)
        self.assertIn("extraction or card-patch prompts", skill)
        self.assertIn("undeclared", skill)
        self.assertIn("--collected-attempt-results", skill)
        self.assertIn("top-level `outcome` is exactly", skill)
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
