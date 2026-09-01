#!/usr/bin/env python3
"""Fail closed on unsafe public claims and accidentally tracked sensitive artifacts."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path

CONTRACT = "mdp.public-artifact-lint.v0"

SENSITIVE_NAMES = {
    ".env",
    "id_rsa",
    "id_ed25519",
    "credentials.json",
    "service-account.json",
}
SENSITIVE_SUFFIXES = {".pem", ".key", ".p12", ".pfx"}
CLAIM_PATTERNS = [
    (re.compile(r"\bMDP\s+(?:is|provides)\s+(?:an?\s+)?AI SDR\b", re.I), "ai_sdr_claim"),
    (re.compile(r"\b(?:CMMC|NIST(?:\s+800-\d+)?)\s+(?:certified|compliant)\b", re.I), "compliance_claim"),
    (re.compile(r"(?<!not )\bguarantees?\s+(?:security|compliance|proposal\s+success)\b", re.I), "guarantee_claim"),
    (re.compile(r"\bapproved\s+(?:handling\s+)?for\s+CUI\b", re.I), "cui_approval_claim"),
    (re.compile(r"\bfully\s+automated\s+proposal\s+writing\b", re.I), "automation_claim"),
    (re.compile(r"(?<!not )\breplaces?\s+(?:human\s+)?(?:compliance\s+review|proposal\s+management\s+software)\b", re.I), "replacement_claim"),
]
CONTROL_PLANE_PATTERNS = [
    (
        re.compile(r"\b(?:delegated|assigned)\s+to\s+(?:Orchid|Eve|a\s+Linear\s+agent)\b", re.I),
        "private_execution_assignment",
    ),
    (
        re.compile(
            r"\b(?:credentials?|tokens?)\s+(?:are|is|remains?)\s+"
            r"(?:enabled|available|exposed)(?:\s+at\s+process\s+scope)?\b",
            re.I,
        ),
        "ambient_credential_roadmap",
    ),
]
NEGATION_MARKERS = (
    " not ",
    "never ",
    "do not ",
    "does not ",
    "must not ",
    "cannot ",
    "without ",
    "unsupported",
    "reject",
    "avoid ",
    "prohibit",
)


def is_claim_surface(relative: str) -> bool:
    path = Path(relative)
    if path.suffix.lower() not in {".md", ".rst", ".txt"}:
        return False
    if relative.startswith("docs/orchid/"):
        return False
    return (
        len(path.parts) == 1
        or relative.startswith("docs/")
        or relative.startswith("plugin/skills/")
    )


def is_publication_boundary_surface(relative: str) -> bool:
    path = Path(relative)
    return path.suffix.lower() in {".md", ".rst", ".txt"} and relative.startswith(
        "docs/orchid/"
    )


def lint_paths(root: Path, relative_paths: list[str]) -> list[dict[str, str | int]]:
    findings: list[dict[str, str | int]] = []
    for relative in relative_paths:
        path = root / relative
        if not path.is_file():
            continue
        if path.name in SENSITIVE_NAMES or path.suffix.lower() in SENSITIVE_SUFFIXES:
            if path.name != ".env.example":
                findings.append({"path": relative, "line": 0, "code": "sensitive_artifact"})
                continue
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except (UnicodeDecodeError, OSError):
            continue
        if is_publication_boundary_surface(relative):
            for number, line in enumerate(lines, 1):
                for pattern, code in CONTROL_PLANE_PATTERNS:
                    if pattern.search(line):
                        findings.append({"path": relative, "line": number, "code": code})
        if not is_claim_surface(relative):
            continue
        for number, line in enumerate(lines, 1):
            normalized = f" {line.lower()} "
            if any(marker in normalized for marker in NEGATION_MARKERS):
                continue
            for pattern, code in CLAIM_PATTERNS:
                if pattern.search(line):
                    findings.append({"path": relative, "line": number, "code": code})
    return findings


def tracked_paths(root: Path) -> list[str]:
    result = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=root,
        check=True,
        capture_output=True,
    )
    return [item.decode() for item in result.stdout.split(b"\0") if item]


def format_finding(finding: dict[str, str | int]) -> str:
    return f"{finding['path']}:{finding['line']}: {finding['code']}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    root = args.root.resolve()
    findings = lint_paths(root, tracked_paths(root))
    if findings:
        for finding in findings:
            print(format_finding(finding))
        print(f"{CONTRACT}: blocked ({len(findings)} finding(s))")
        return 1
    print(f"{CONTRACT}: passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
