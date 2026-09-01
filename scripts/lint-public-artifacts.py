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
PARAGRAPH_SAFE_WS = r"(?:(?!\n\s*\n)\s)+"


def prose_pattern(pattern: str) -> re.Pattern[str]:
    """Compile whitespace-aware prose without allowing matches across paragraphs."""
    return re.compile(pattern.replace(r"\s+", PARAGRAPH_SAFE_WS), re.I)


CLAIM_PATTERNS = [
    (prose_pattern(r"\bMDP\s+(?:is|provides)\s+(?:an?\s+)?AI SDR\b"), "ai_sdr_claim"),
    (prose_pattern(r"\b(?:CMMC|NIST(?:\s+800-\d+)?)\s+(?:certified|compliant)\b"), "compliance_claim"),
    (prose_pattern(r"(?<!not )\bguarantees?\s+(?:security|compliance|proposal\s+success)\b"), "guarantee_claim"),
    (prose_pattern(r"\bapproved\s+(?:handling\s+)?for\s+CUI\b"), "cui_approval_claim"),
    (prose_pattern(r"\bfully\s+automated\s+proposal\s+writing\b"), "automation_claim"),
    (prose_pattern(r"(?<!not )\breplaces?\s+(?:human\s+)?(?:compliance\s+review|proposal\s+management\s+software)\b"), "replacement_claim"),
]
CONTROL_PLANE_PATTERNS = [
    (
        prose_pattern(r"\b(?:delegated|assigned)\s+to\s+(?:Orchid|Eve|a\s+Linear\s+agent)\b"),
        "private_execution_assignment",
    ),
    (
        prose_pattern(
            r"\b(?:credentials?|tokens?)\s+(?:are|is|remains?)\s+"
            r"(?:enabled|available|exposed)(?:\s+at\s+process\s+scope)?\b"
        ),
        "ambient_credential_roadmap",
    ),
    (
        prose_pattern(
            r"\b(?:next|then)\s+(?:private\s+)?(?:Linear\s+)?"
            r"(?:issue|task|ticket)\s+[A-Z][A-Z0-9]+-\d+\b"
        ),
        "private_linear_sequence",
    ),
    (
        prose_pattern(
            r"\b[A-Z][A-Z0-9]+-\d+\s+(?:must|should|will)\s+"
            r"(?:land|merge|ship|complete|start|run)\s+(?:before|after)\s+"
            r"[A-Z][A-Z0-9]+-\d+\b"
        ),
        "private_linear_sequence",
    ),
    (
        prose_pattern(
            r"\b(?:before|after)\s+[A-Z][A-Z0-9]+-\d+\b"
            r"(?:(?!\n\s*\n)[^.!?]){0,80}"
            r"\b(?:start|run|land|merge|ship|complete)\s+"
            r"[A-Z][A-Z0-9]+-\d+\b"
        ),
        "private_linear_sequence",
    ),
    (
        prose_pattern(
            r"\b[A-Z][A-Z0-9]+-\d+\s+(?:then|followed\s+by|->)\s+"
            r"[A-Z][A-Z0-9]+-\d+\b"
        ),
        "private_linear_sequence",
    ),
    (
        prose_pattern(
            r"\b(?:security|authentication|authorization|credential|token|provider)\s+"
            r"(?:boundary|hardening|isolation|migration|remediation|work|"
            r"integration|enablement|support)\s+"
            r"(?:is|remains?|will\s+be)\s+"
            r"(?:pending|planned|deferred|unremediated|unfinished|incomplete)\b"
        ),
        "private_security_provider_roadmap",
    ),
]
NEGATION_PATTERN = re.compile(
    r"\b(?:no|not|never|cannot|unsupported|reject(?:s|ed|ing)?|"
    r"avoid(?:s|ed|ing)?|prohibit(?:s|ed|ing)?|without)\b",
    re.I,
)
CLAUSE_BOUNDARY_PATTERN = re.compile(
    r"(?:[.!?;:]\s+|\n\s*\n|\b(?:but|however|yet)\b)",
    re.I,
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


def match_is_negated(text: str, match: re.Match[str]) -> bool:
    """Return whether a nearby, same-clause marker negates an affirmative match."""
    prefix = text[max(0, match.start() - 160) : match.start()]
    boundaries = list(CLAUSE_BOUNDARY_PATTERN.finditer(prefix))
    if boundaries:
        prefix = prefix[boundaries[-1].end() :]
    return NEGATION_PATTERN.search(prefix) is not None


def pattern_findings(
    relative: str,
    text: str,
    patterns: list[tuple[re.Pattern[str], str]],
) -> list[dict[str, str | int]]:
    """Scan a whole document so Markdown wrapping cannot split a match."""
    matches: list[tuple[int, str]] = []
    for pattern, code in patterns:
        for match in pattern.finditer(text):
            if match_is_negated(text, match):
                continue
            matches.append((match.start(), code))
    return [
        {
            "path": relative,
            "line": text.count("\n", 0, offset) + 1,
            "code": code,
        }
        for offset, code in sorted(set(matches))
    ]


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
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        if is_publication_boundary_surface(relative):
            findings.extend(pattern_findings(relative, text, CONTROL_PLANE_PATTERNS))
        if not is_claim_surface(relative):
            continue
        findings.extend(pattern_findings(relative, text, CLAIM_PATTERNS))
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
