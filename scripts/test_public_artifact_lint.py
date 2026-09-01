import importlib.util
import tempfile
import unittest
from pathlib import Path

MODULE_PATH = Path(__file__).with_name("lint-public-artifacts.py")
SPEC = importlib.util.spec_from_file_location("public_artifact_lint", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader
SPEC.loader.exec_module(MODULE)


class PublicArtifactLintTests(unittest.TestCase):
    def test_rejects_sensitive_files_and_unsafe_affirmative_claims(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text("MDP is an AI SDR.\nCMMC compliant.\n")
            (root / "customer.pem").write_text("not-a-real-key")
            findings = MODULE.lint_paths(root, ["README.md", "customer.pem"])
            self.assertEqual(
                [finding["code"] for finding in findings],
                ["ai_sdr_claim", "compliance_claim", "sensitive_artifact"],
            )

    def test_allows_explicit_guardrails_and_env_example(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "README.md").write_text(
                "MDP is not an AI SDR and does not guarantee compliance.\n"
            )
            (root / ".env.example").write_text("OPENAI_API_KEY=\n")
            self.assertEqual(
                MODULE.lint_paths(root, ["README.md", ".env.example"]),
                [],
            )

    def test_rejects_private_control_plane_prose_without_echoing_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "synthetic.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "A maintenance lane is assigned to Orchid.\n"
                "A service credential remains available at process scope.\n"
            )

            findings = MODULE.lint_paths(
                root, ["docs/orchid/plans/synthetic.md"]
            )

            self.assertEqual(
                findings,
                [
                    {
                        "path": "docs/orchid/plans/synthetic.md",
                        "line": 1,
                        "code": "private_execution_assignment",
                    },
                    {
                        "path": "docs/orchid/plans/synthetic.md",
                        "line": 2,
                        "code": "ambient_credential_roadmap",
                    },
                ],
            )
            self.assertEqual(
                MODULE.format_finding(findings[0]),
                "docs/orchid/plans/synthetic.md:1: private_execution_assignment",
            )

    def test_rejects_wrapped_control_plane_prose_at_the_match_line(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "wrapped.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "A synthetic maintenance lane is assigned\n"
                "to Orchid.\n"
                "A synthetic credential remains\n"
                "available at process scope.\n"
            )

            self.assertEqual(
                MODULE.lint_paths(root, ["docs/orchid/plans/wrapped.md"]),
                [
                    {
                        "path": "docs/orchid/plans/wrapped.md",
                        "line": 1,
                        "code": "private_execution_assignment",
                    },
                    {
                        "path": "docs/orchid/plans/wrapped.md",
                        "line": 3,
                        "code": "ambient_credential_roadmap",
                    },
                ],
            )

    def test_rejects_private_sequence_and_security_roadmap_classes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "roadmap.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "Next Linear task ABC-101 will be handled privately.\n"
                "ABC-102 must land after ABC-101.\n"
                "After ABC-103,\nstart ABC-104.\n"
                "Provider isolation remains deferred.\n"
            )

            self.assertEqual(
                [
                    finding["code"]
                    for finding in MODULE.lint_paths(
                        root, ["docs/orchid/plans/roadmap.md"]
                    )
                ],
                [
                    "private_linear_sequence",
                    "private_linear_sequence",
                    "private_linear_sequence",
                    "private_security_provider_roadmap",
                ],
            )

    def test_allows_negated_control_plane_guardrails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "guardrails.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "The synthetic lane is not assigned to Orchid.\n"
                "The fallback lane is not assigned to a Linear agent.\n"
                "No credentials are available at process scope.\n"
                "No provider boundary is deferred.\n"
            )

            self.assertEqual(
                MODULE.lint_paths(root, ["docs/orchid/plans/guardrails.md"]),
                [],
            )

    def test_negation_does_not_hide_a_later_affirmative_clause(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "contrast.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "No credential is exposed by default, but a token remains available "
                "at process scope.\n"
            )

            self.assertEqual(
                [
                    finding["code"]
                    for finding in MODULE.lint_paths(
                        root, ["docs/orchid/plans/contrast.md"]
                    )
                ],
                ["ambient_credential_roadmap"],
            )

    def test_sequence_scan_does_not_cross_paragraphs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "paragraphs.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "After ABC-201 the scope widens\n\n"
                "start ABC-202 in an unrelated example\n"
                "After ABC-203 start\n\n"
                "ABC-204 in another unrelated example\n"
            )

            self.assertEqual(
                MODULE.lint_paths(root, ["docs/orchid/plans/paragraphs.md"]),
                [],
            )

    def test_allows_public_issue_references_and_delivery_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            plan = root / "docs" / "orchid" / "plans" / "public-safe.md"
            plan.parent.mkdir(parents=True)
            plan.write_text(
                "MDP-999 records the public change delivered by PR #123.\n"
            )

            self.assertEqual(
                MODULE.lint_paths(root, ["docs/orchid/plans/public-safe.md"]),
                [],
            )


if __name__ == "__main__":
    unittest.main()
