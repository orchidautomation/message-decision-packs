from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MIGRATION_VERSION = "0.1.107"
CANONICAL_SKILLS = (
    "mdp",
    "mdp-pack-apply",
    "mdp-pack-builder",
    "mdp-pack-review",
)
PROPOSAL_EVIDENCE_PATH = Path(
    "plugin/skills/mdp-pack-apply/references/proposal-evidence-path.md"
)


class MigrationDocumentationTests(unittest.TestCase):
    def test_proposal_runner_points_to_the_shipped_evidence_reference(self):
        proposal_runner = (ROOT / "docs/proposal-runner.md").read_text()

        self.assertIn(f"`{PROPOSAL_EVIDENCE_PATH.as_posix()}`", proposal_runner)
        self.assertTrue((ROOT / PROPOSAL_EVIDENCE_PATH).is_file())
        self.assertNotIn(
            "`plugin/skills/mdp-pack-apply/references/evidence-path.md`",
            proposal_runner,
        )

    def test_public_migration_summaries_name_the_four_skill_release(self):
        summaries = {
            "llms.txt": (ROOT / "llms.txt").read_text(),
            "docs/what-this-repo-is.md": (
                ROOT / "docs/what-this-repo-is.md"
            ).read_text(),
        }

        for path, summary in summaries.items():
            with self.subTest(path=path):
                self.assertIn(f"MDP `{MIGRATION_VERSION}`", summary)
                self.assertIn("exactly the four", summary)
                for skill in CANONICAL_SKILLS:
                    self.assertIn(f"`{skill}`", summary)

    def test_four_skill_migration_matches_the_authored_inventory(self):
        actual = tuple(
            sorted(
                path.name
                for path in (ROOT / "plugin/skills").iterdir()
                if path.is_dir()
            )
        )

        self.assertEqual(tuple(sorted(CANONICAL_SKILLS)), actual)


if __name__ == "__main__":
    unittest.main()
