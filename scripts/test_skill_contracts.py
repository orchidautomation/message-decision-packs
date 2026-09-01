from __future__ import annotations

import importlib.util
import re
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

    def test_all_canonical_skills_preserve_cli_authority_monotonicity(self):
        for path in sorted(Path("plugin/skills").glob("*/SKILL.md")):
            skill = path.read_text()
            self.assertIn("The Rust CLI is the decision authority", skill, path)
            self.assertIn("Preserve or reduce its authority", skill, path)
            self.assertIn("never upgrade `blocked`, `no-draft`, `unavailable`", skill, path)
            self.assertIn("New evidence requires a new CLI evaluation", skill, path)
            self.assertIn("cannot override an existing result in place", skill, path)

    def test_all_canonical_skills_share_the_communication_contract(self):
        communication = Path(
            "plugin/skills/mdp/references/communication-contract.md"
        ).read_text()
        for term in (*module.COMMUNICATION_STAGES, *module.READINESS_TERMS):
            self.assertIn(term, communication)
        for path in sorted(Path("plugin/skills").glob("*/SKILL.md")):
            skill = path.read_text()
            self.assertIn("Orient, Plan, Progress, Translate, Close contract", skill)
            self.assertIn("Open by naming", skill)
            self.assertIn("evidence boundary", skill)
            self.assertIn("user will receive", skill)
            self.assertIn("will not", skill)
            self.assertIn("meaningful", skill)

    def test_missing_communication_contract_fails_validation(self):
        skill = self.root / "plugin/skills/mdp-pack-review/SKILL.md"
        skill.write_text(
            skill.read_text().replace(
                "Orient, Plan, Progress, Translate, Close contract",
                "removed communication contract",
                1,
            )
        )
        self.assertIn("communication_opening_missing:mdp-pack-review", self.codes())

        communication = (
            self.root
            / "plugin/skills/mdp/references/communication-contract.md"
        )
        communication.write_text(
            communication.read_text().replace("safe-to-draft", "removed")
        )
        self.assertIn("communication_contract_missing:safe-to-draft", self.codes())

    def test_core_skill_exposes_job_bound_requirements_handoff(self):
        skill = Path("plugin/skills/mdp/references/operator-runtime.md").read_text()
        operator = Path("plugin/skills/mdp/references/cli-operator.md").read_text()
        command = "mdp --json requirements --dir"
        self.assertIn(command, skill)
        self.assertIn(command, operator)
        self.assertIn("available: false", skill)
        self.assertIn("does not collect sources or call a model", skill)

    def test_core_and_review_skills_preserve_positioning_boundary(self):
        boundaries = {
            "plugin/skills/mdp/SKILL.md": (
                "never a graph database, agent runtime, memory layer, or "
                "orchestration framework"
            ),
            "plugin/skills/mdp/references/mental-model.md": (
                "do not describe mdp as a graph database, agent runtime, "
                "orchestration framework, persistent memory layer, or proof "
                "that a source claim is true"
            ),
            "plugin/skills/mdp-pack-review/SKILL.md": (
                "flag graph database, agent runtime, orchestration, persistent "
                "memory, universal graph, and source truth claims"
            ),
        }
        for path, expected_boundary in boundaries.items():
            text = Path(path).read_text()
            normalized = " ".join(text.lower().replace("-", " ").split())
            self.assertIn("versioned decision context for agents", normalized)
            self.assertIn("decision graph", normalized)
            self.assertIn(expected_boundary, normalized)

    def test_existing_skills_route_provider_neutral_source_binding(self):
        core = Path("plugin/skills/mdp/references/operator-runtime.md").read_text()
        operator = Path("plugin/skills/mdp/references/cli-operator.md").read_text()
        builder = Path("plugin/skills/mdp-pack-builder/references/safe-authoring.md").read_text()
        review = Path("plugin/skills/mdp-pack-review/references/review-protocol.md").read_text()
        command = "mdp --json validate-source-binding"
        for text in [core, operator, builder, review]:
            self.assertIn(command, text)
        self.assertIn("outside", core)
        self.assertIn("field-key reuse", operator)
        self.assertIn("provider enums", builder)
        self.assertIn("External field", review)

    def test_pack_skills_describe_both_readme_owned_regions(self):
        for relative_path in (
            "plugin/skills/mdp-pack-builder/references/safe-authoring.md",
            "plugin/skills/mdp-pack-review/references/review-protocol.md",
        ):
            skill = Path(relative_path).read_text()
            normalized = " ".join(skill.split())
            self.assertIn("two machine-generated regions", normalized, relative_path)
            self.assertIn("<!-- mdp:readme-ownership v1 begin -->", skill, relative_path)
            self.assertIn("<!-- mdp:readme-ownership v1 end -->", skill, relative_path)
            self.assertIn("<!-- mdp:readme-inventory v1 begin -->", skill, relative_path)
            self.assertIn("<!-- mdp:readme-inventory v1 end -->", skill, relative_path)
            self.assertIn("Never hand-edit", skill, relative_path)
            self.assertIn("preserves every byte", skill, relative_path)

    def test_gtm_brief_preserves_decision_input_and_legacy_normalization_paths(self):
        skill = Path("plugin/skills/mdp-pack-apply/references/gtm-governed-execution.md").read_text()
        mode = Path("plugin/skills/mdp-pack-apply/references/gtm-prospect-fit-or-brief.md").read_text()
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
        mode = Path("plugin/skills/mdp-pack-apply/references/gtm-prospect-fit-or-brief.md").read_text()
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
        skill = Path("plugin/skills/mdp-pack-apply/references/gtm-governed-execution.md").read_text()
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
            "plugin/skills/mdp-pack-apply/references/gtm-governed-execution.md",
            "plugin/skills/mdp-pack-apply/references/gtm-prospect-fit-or-brief.md",
        ]:
            text = Path(path).read_text()
            supplied = text.index("If all four artifacts")
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
        skill = Path("plugin/skills/mdp-pack-builder/references/safe-authoring.md").read_text()
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
        skill = Path("plugin/skills/mdp/references/operator-runtime.md").read_text()
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

    def test_entrypoints_stay_bounded_and_references_are_one_level(self):
        for path in sorted(Path("plugin/skills").glob("*/SKILL.md")):
            self.assertLessEqual(path.stat().st_size, module.MAX_ENTRYPOINT_BYTES, path)
            frontmatter = module.FRONTMATTER.match(path.read_text())
            self.assertIsNotNone(frontmatter, path)
            description = dict(module.FIELD.findall(frontmatter.group(1)))[
                "description"
            ].strip("'\"")
            self.assertLessEqual(
                len(description), module.MAX_SUPPORTED_DESCRIPTION_CHARS, path
            )
        reference = self.root / "plugin/skills/mdp/references/operator-runtime.md"
        original = reference.read_text()
        reference.write_text(original + "\n[next](cli-operator.md)\n")
        self.assertIn("nested_skill_reference", self.codes())
        reference.write_text(original + "\nRead references/cli-operator.md.\n")
        self.assertIn("nested_skill_reference", self.codes())

        skill = self.root / "plugin/skills/mdp/SKILL.md"
        skill.write_text(skill.read_text() + ("\nexcess\n" * 1000))
        self.assertIn("skill_entrypoint_too_large", self.codes())

        skill.write_text(
            skill.read_text().replace(
                "description: ",
                "description: " + ("overlong " * 40),
                1,
            )
        )
        self.assertIn("frontmatter_description_invalid", self.codes())

    def test_skill_local_link_escape_fails(self):
        path = self.root / "plugin/skills/mdp/SKILL.md"
        path.write_text(path.read_text() + "\n[escape](references/../../mdp-pack-builder/SKILL.md)\n")
        self.assertIn("skill_link_escape", self.codes())

    def test_every_skill_declares_actionable_runtime_compatibility(self):
        for path in sorted(Path("plugin/skills").glob("*/SKILL.md")):
            text = path.read_text()
            frontmatter = module.FRONTMATTER.match(text)
            self.assertIsNotNone(frontmatter, path)
            compatibility_match = module.COMPATIBILITY_METADATA.search(
                frontmatter.group(1)
            )
            self.assertIsNotNone(compatibility_match, path)
            compatibility = compatibility_match.group(1).strip("'\"")
            for marker in module.COMPATIBILITY_TERMS:
                self.assertIn(marker, compatibility, path)
        path = self.root / "plugin/skills/mdp/SKILL.md"
        original = path.read_text()
        frontmatter = module.FRONTMATTER.match(original)
        self.assertIsNotNone(frontmatter)
        mutated_frontmatter, replacements = re.subn(
            re.escape("Node.js 18+"),
            "Node runtime",
            frontmatter.group(1),
            count=1,
        )
        self.assertEqual(replacements, 1)
        path.write_text(
            original[: frontmatter.start(1)]
            + mutated_frontmatter
            + original[frontmatter.end(1) :]
            + "\nNode.js 18+ may still appear in body prose.\n"
        )
        self.assertIn("frontmatter_compatibility_invalid", self.codes())

    def test_compatibility_requires_direct_cli_node_boundary(self):
        path = self.root / "plugin/skills/mdp-pack-apply/SKILL.md"
        path.write_text(
            path.read_text().replace(
                "generative direct-CLI runs",
                "native plugin helpers",
                1,
            )
        )
        self.assertIn("frontmatter_compatibility_invalid", self.codes())

    def test_source_plan_routing_and_non_mutating_closeout_are_required(self):
        for skill_id, phrases in module.AUTHORING_CLOSEOUT_GUARDRAILS.items():
            path = self.root / "plugin/skills" / skill_id / "SKILL.md"
            original = path.read_text()
            for phrase in phrases:
                with self.subTest(skill_id=skill_id, phrase=phrase):
                    flexible_phrase = r"\s+".join(
                        re.escape(part) for part in phrase.split()
                    )
                    mutated, replacements = re.subn(
                        flexible_phrase,
                        "REMOVED_SOURCE_PLAN_CONTRACT",
                        original,
                        count=1,
                    )
                    self.assertEqual(replacements, 1)
                    path.write_text(mutated)
                    self.assertIn(f"authoring_closeout_missing:{skill_id}", self.codes())
                    path.write_text(original)

    def test_source_plan_guardrails_ignore_markdown_reflow(self):
        path = self.root / "plugin/skills/mdp-pack-builder/SKILL.md"
        original = path.read_text()
        phrase = module.AUTHORING_CLOSEOUT_GUARDRAILS["mdp-pack-builder"][0]
        reflowed = phrase.replace(", return ", ",\n   return ")
        self.assertNotEqual(reflowed, phrase)
        mutated, replacements = re.subn(
            r"\s+".join(re.escape(part) for part in phrase.split()),
            reflowed,
            original,
            count=1,
        )
        self.assertEqual(replacements, 1)
        path.write_text(mutated)
        self.assertNotIn(
            "authoring_closeout_missing:mdp-pack-builder", self.codes()
        )

    def test_repo_only_document_dependency_fails(self):
        path = self.root / "plugin/skills/mdp/references/operator-runtime.md"
        path.write_text(path.read_text() + "\nRead `docs/private-contract.md`.\n")
        self.assertIn("repo_only_document_dependency", self.codes())

    def test_sibling_skill_reference_fails_isolated_portability(self):
        path = self.root / "plugin/skills/mdp-pack-review/SKILL.md"
        path.write_text(
            path.read_text()
            + "\n[cross-skill](../mdp/references/communication-contract.md)\n"
        )
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
        path = self.root / "plugin/skills/mdp-pack-apply/SKILL.md"
        original = path.read_text()
        for guardrail, phrase in module.PROPOSAL_GUARDRAILS.items():
            with self.subTest(guardrail=guardrail):
                path.write_text(original.replace(phrase, "REMOVED_GUARDRAIL", 1))
                self.assertIn(f"proposal_guardrail_missing:{guardrail}", self.codes())
                path.write_text(original)

    def test_stale_runner_name_fails(self):
        review = self.root / "plugin/skills/mdp-pack-apply/SKILL.md"
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

    def test_each_product_foundation_guardrail_is_enforced(self):
        for skill_id, guardrails in module.FOUNDATION_GUARDRAILS.items():
            path = self.root / "plugin/skills" / skill_id / "SKILL.md"
            original = path.read_text()
            for guardrail, phrase in guardrails.items():
                with self.subTest(skill_id=skill_id, guardrail=guardrail):
                    path.write_text(
                        original.replace(phrase, "REMOVED_FOUNDATION_GUARDRAIL", 1)
                    )
                    self.assertIn(
                        f"foundation_guardrail_missing:{skill_id}:{guardrail}",
                        self.codes(),
                    )
                    path.write_text(original)

    def test_each_cold_model_guardrail_is_enforced(self):
        for skill_id, guardrails in module.COLD_MODEL_GUARDRAILS.items():
            path = self.root / "plugin/skills" / skill_id / "SKILL.md"
            original = path.read_text()
            for guardrail, phrase in guardrails.items():
                with self.subTest(skill_id=skill_id, guardrail=guardrail):
                    flexible_phrase = r"\s+".join(re.escape(part) for part in phrase.split())
                    mutated, replacements = re.subn(
                        flexible_phrase,
                        "REMOVED_COLD_MODEL_GUARDRAIL",
                        original,
                        count=1,
                    )
                    self.assertEqual(replacements, 1)
                    path.write_text(mutated)
                    self.assertIn(
                        f"cold_model_guardrail_missing:{skill_id}:{guardrail}",
                        self.codes(),
                    )
                    path.write_text(original)

    def test_public_cold_model_doc_keeps_authority_and_privacy_boundaries(self):
        text = Path("docs/cold-model-conformance.md").read_text()
        for phrase in [
            "sufficient-for-job",
            "qualified-for-job-under-envelope",
            "unassessed",
            "not-sufficient-for-job",
            "not-qualified-for-job-under-envelope",
            "intermediate authority",
            "sole hash-complete",
            "performs no model or network call",
            "never expose\nprivate or opaque evidence IDs",
            "Conformance never grants drafting, sending, scheduling, CRM mutation",
        ]:
            self.assertIn(phrase, text)
        self.assertNotIn("/Users/", text)
        self.assertNotIn("opaque private evidence IDs", text)

    def test_cold_model_discovery_help_and_schema_inventory_stay_aligned(self):
        cli = Path("cli/src/cli.rs").read_text()
        capabilities = Path("cli/src/commands/capabilities.rs").read_text()
        operator = Path("plugin/skills/mdp/references/cli-operator.md").read_text()
        for command in ["compile", "validate", "assemble", "report"]:
            self.assertIn(f'["conformance", "{command}"]', capabilities)
            self.assertIn(f"`conformance {command}`", operator)
        for flag in [
            "--candidate",
            "--artifact-root",
            "--evaluator-inventory",
            "--lifecycle-policy",
            "--deterministic",
            "--invocation",
            "--trial",
            "--verifier-receipt",
            "--behavioral",
            "--conformance",
            "--visibility",
            "--generated-at",
            "--out",
            "--dry-run",
        ]:
            self.assertIn(f'"{flag}"', capabilities)
        for target in [
            "conformance-candidate-v1",
            "model-invocation-evidence-v1",
            "conformance-verifier-receipt-v1",
            "evaluator-inventory-v1",
            "evaluator-result-v1",
            "private-record-policy-v1",
            "publication-approval-v1",
            "conformance-trial-v1",
            "job-conformance-v1",
            "conformance-report-v1",
            "public-conformance-report-v1",
            "deterministic-conformance-v1",
            "behavioral-evaluation-v1",
        ]:
            self.assertIn(target, cli)
        self.assertIn('"model_execution": "external-only"', capabilities)
        self.assertIn('"behavioral_calls_in_validation": false', capabilities)

    def test_public_product_foundation_doc_keeps_authority_and_readiness_boundaries(self):
        text = Path("docs/product-foundations.md").read_text()
        for phrase in [
            "exact canonical IDs",
            "`unassessed`, `ready`, or `blocked`",
            "Foundation readiness is a veto-only input",
            "never establishes\n`sufficient-for-job`, self-standing status",
            "README prose cannot satisfy a facet",
            "changing it changes the portable pack hash",
            "does not add an eleventh primitive",
        ]:
            self.assertIn(phrase, text)


if __name__ == "__main__":
    unittest.main()
