from __future__ import annotations

import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CANONICAL_SKILLS = (
    "mdp",
    "mdp-pack-apply",
    "mdp-pack-builder",
    "mdp-pack-review",
)
PROPOSAL_EVIDENCE_PATH = Path(
    "plugin/skills/mdp-pack-apply/references/proposal-evidence-path.md"
)


class PublicDocumentationTests(unittest.TestCase):
    def test_proposal_runner_points_to_the_shipped_evidence_reference(self):
        proposal_runner = (ROOT / "docs/proposal-runner.md").read_text()

        self.assertIn(f"`{PROPOSAL_EVIDENCE_PATH.as_posix()}`", proposal_runner)
        self.assertTrue((ROOT / PROPOSAL_EVIDENCE_PATH).is_file())
        self.assertNotIn(
            "`plugin/skills/mdp-pack-apply/references/evidence-path.md`",
            proposal_runner,
        )

    def test_public_summaries_name_the_four_stable_skills(self):
        summaries = {
            "llms.txt": (ROOT / "llms.txt").read_text(),
            "llms-full.txt": (ROOT / "llms-full.txt").read_text(),
        }

        for path, summary in summaries.items():
            with self.subTest(path=path):
                self.assertRegex(summary, r"exactly\s+the four")
                for skill in CANONICAL_SKILLS:
                    self.assertIn(f"`{skill}`", summary)

    def test_entry_point_docs_do_not_pin_point_release_narratives(self):
        for path in (
            "README.md",
            "llms.txt",
            "llms-full.txt",
            "docs/getting-started.md",
            "docs/what-this-repo-is.md",
        ):
            with self.subTest(path=path):
                text = (ROOT / path).read_text()
                self.assertIsNone(re.search(r"MDP `0\.1\.\d+`", text))

    def test_removed_repository_surfaces_do_not_return_through_current_entry_points(self):
        self.assertFalse((ROOT / ".mdp").exists())
        self.assertFalse((ROOT / "examples/ai-sdr-eve-vercel").exists())

        current_surfaces = (
            "README.md",
            "llms.txt",
            "llms-full.txt",
            "docs/what-this-repo-is.md",
            ".github/workflows/ci.yml",
            "scripts/validate-skill-packaging.py",
        )
        for path in current_surfaces:
            with self.subTest(path=path):
                text = (ROOT / path).read_text().lower()
                self.assertNotIn("ai-sdr-eve-vercel", text)
                self.assertNotIn("eve on vercel", text)

        redirects = json.loads(
            (ROOT / "deploy/mdp-installer/vercel.json").read_text()
        )["redirects"]
        self.assertFalse(
            any(route["source"].startswith("/eve") for route in redirects)
        )

    def test_repo_explainer_is_navigation_not_a_served_release_asset(self):
        explainer = (ROOT / "docs/what-this-repo-is.md").read_text()
        docs_index = (ROOT / "docs/README.md").read_text()
        root_readme = (ROOT / "README.md").read_text()
        release_workflow = (ROOT / ".github/workflows/release.yml").read_text()
        redirects = json.loads(
            (ROOT / "deploy/mdp-installer/vercel.json").read_text()
        )["redirects"]

        self.assertIn("[What This Repo Is](what-this-repo-is.md)", docs_index)
        self.assertIn(
            "[What This Repo Is](docs/what-this-repo-is.md)", root_readme
        )
        self.assertIn("not a release asset or vanity", explainer)
        self.assertNotIn("what-this-repo-is", release_workflow)
        self.assertFalse(
            any("what-this-repo-is" in route["source"] for route in redirects)
        )

    def test_four_skill_contract_matches_the_authored_inventory(self):
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
