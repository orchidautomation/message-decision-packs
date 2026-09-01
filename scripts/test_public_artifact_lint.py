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
