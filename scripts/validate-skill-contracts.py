#!/usr/bin/env python3
"""Validate canonical MDP skill authoring and proposal safety contracts."""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

MODEL = "mdp.skill-contract-validation.v1"
FRONTMATTER = re.compile(r"\A---\n(.*?)\n---\n", re.S)
FIELD = re.compile(r"^([a-zA-Z][\w-]*):\s*(.+?)\s*$", re.M)
MARKDOWN_LINK = re.compile(r"\[[^\]]*\]\(([^)]+)\)")
SCRIPT_TOKEN = re.compile(r"(?<![\w/])scripts/(mdp-[a-z0-9-]+\.mjs)")
LOCAL_PREFIXES = ("references/", "scripts/", "assets/")
IGNORED_SKILL_ROOTS = ("docs/orchid/history/", "dist/", "target/", ".git/")
LOAD_TIME_HAZARDS = (
    re.compile(r"\bSKILL_DIR\s*=\s*\$\("),
    re.compile(r"(?m)^\s*(?:source|\.)\s+[^\n]*(?:SKILL_DIR|PLUGIN_ROOT)"),
    re.compile(r"\bexport\s+(?:SKILL_DIR|PLUGIN_ROOT)="),
    re.compile(r"\bcd\s+['\"]?\$\((?:dirname|pwd)"),
)
PROPOSAL_GUARDRAILS = {
    "no_certify_or_submit": "Never certify, invent proof, grant final approval, write, or submit proposals.",
    "no_invented_proof": "Never invent RFP text, requirements, deadlines, evaluator criteria, proof, certifications, compliance status, pricing, references, outcomes, past performance, or approvals.",
    "no_submission_authority": "not certification, legal advice, approval, or submission authority.",
    "no_private_public_artifacts": "Keep restricted pursuit material out of public paths and generated fixtures.",
}
AUTHORING_GUARDRAILS = {
    "supplied_sources_only": "Use supplied or approved RFP files",
    "no_raw_pack_sources": "rather than raw source documents in the pack",
    "no_compliance_overclaim": "It does not certify compliance",
    "no_proposal_submission": "submit proposals",
}
FOUNDATION_GUARDRAILS = {
    "mdp": {
        "cli_first": "Inspect `data.recommendation.product_foundation` before opening pack prose.",
        "readme_secondary": "Treat `.mdp/README.md` as secondary navigation only",
        "exact_job": "Never substitute a natural-language job approximation",
        "no_invention": "Never invent missing product,",
        "veto_only": "Foundation readiness only vetoes broader readiness.",
    },
    "mdp-pack-builder": {
        "existing_authority": "index exact existing card/entry refs",
        "exact_job": "Use exact canonical job IDs.",
        "readme_secondary": "only as concise secondary navigation",
        "no_invention": "never invent them to make a job ready",
        "veto_only": "Foundation `ready` is veto-only",
    },
    "mdp-pack-review": {
        "cli_first": "CLI-resolved foundation before `.mdp/README.md`",
        "exact_job": "For each exact canonical job ID",
        "no_leakage": "must not leak into selected context",
        "no_invention": "Never invent product facts,",
        "veto_only": "Foundation readiness only vetoes broader readiness",
    },
}
COLD_MODEL_GUARDRAILS = {
    "mdp": {
        "compile_first": "Stop no-draft unless deterministic status is `sufficient-for-job`.",
        "external_model": "customer-selected host owns provider/model selection and the call; MDP does\nneither",
        "intermediate_not_report": "Treat `mdp.behavioral-evaluation.v1` as intermediate only.",
        "sole_authority": "sole cross-phase `mdp.job-conformance.v1` authority",
        "no_action_authority": "No result grants drafting, sending, scheduling,",
    },
    "mdp-pack-review": {
        "complete_flow": "`conformance compile`, externally recorded trials, `conformance",
        "not_qualification": "Deterministic `sufficient-for-job` is\nnot behavioral qualification.",
        "intermediate_not_report": "A behavioral evaluation alone\nis intermediate, never report authority.",
        "privacy": "provider/session identifiers, evaluator rationale,",
    },
    "mdp-gtm-brief": {
        "compile_first": "require a passing\n`conformance compile` before handing anything to the external host",
        "sole_authority": "assemble\n`mdp.job-conformance.v1`",
        "no_draft": "`unassessed` and\n`conformance-failure` remain no-draft",
    },
}


def error(errors: list[dict[str, str]], code: str, path: Path | str, message: str) -> None:
    errors.append({"code": code, "path": str(path), "message": message})


def inside(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def validate(root: Path, source: Path) -> dict:
    errors: list[dict[str, str]] = []
    skills: list[str] = []
    source = source.resolve()
    root = root.resolve()
    if not source.is_dir():
        error(errors, "skill_root_missing", source, "canonical skill root is missing")
        return {"model": MODEL, "source": str(source), "skills": [], "valid": False, "errors": errors}

    for skill_dir in sorted(p for p in source.iterdir() if p.is_dir()):
        skill_file = skill_dir / "SKILL.md"
        skills.append(skill_dir.name)
        if not skill_file.is_file():
            error(errors, "skill_file_missing", skill_file, "skill directory requires SKILL.md")
            continue
        text = skill_file.read_text(encoding="utf-8")
        match = FRONTMATTER.match(text)
        if not match:
            error(errors, "frontmatter_invalid", skill_file, "SKILL.md requires bounded YAML frontmatter")
            continue
        fields = dict(FIELD.findall(match.group(1)))
        name, description = fields.get("name", "").strip("'\""), fields.get("description", "").strip("'\"")
        if name != skill_dir.name or not (1 <= len(name) <= 64):
            error(errors, "frontmatter_name_invalid", skill_file, "name must match the directory and be 1-64 characters")
        if not (20 <= len(description) <= 1024) or "\n" in description:
            error(errors, "frontmatter_description_invalid", skill_file, "description must be one line and 20-1024 characters")

        for doc in sorted(skill_dir.rglob("*.md")):
            doc_text = doc.read_text(encoding="utf-8")
            for target in MARKDOWN_LINK.findall(doc_text):
                target = target.split("#", 1)[0]
                if not target.startswith(LOCAL_PREFIXES):
                    continue
                resolved = (doc.parent / target).resolve()
                if not inside(resolved, skill_dir):
                    error(errors, "skill_link_escape", doc, f"skill-local link escapes skill root: {target}")
                elif not resolved.is_file():
                    error(errors, "skill_link_missing", doc, f"skill-local link does not resolve: {target}")
            for hazard in LOAD_TIME_HAZARDS:
                if hazard.search(doc_text):
                    error(errors, "load_time_shell_hazard", doc, f"load-time path pre-resolution is forbidden: {hazard.pattern}")
            for script_name in SCRIPT_TOKEN.findall(doc_text):
                script_path = root / "scripts" / script_name
                if not script_path.is_file():
                    error(errors, "runner_script_reference_stale", doc, f"referenced runner script is missing: scripts/{script_name}")

    for candidate in sorted(root.rglob("SKILL.md")):
        rel = candidate.relative_to(root).as_posix()
        if inside(candidate, source) or rel.startswith(IGNORED_SKILL_ROOTS):
            continue
        error(errors, "authored_skill_outside_canonical_root", candidate, "active authored skills must live under plugin/skills")

    proposal_review = source / "mdp-proposal-review" / "SKILL.md"
    proposal_text = proposal_review.read_text(encoding="utf-8") if proposal_review.is_file() else ""
    for guardrail, phrase in PROPOSAL_GUARDRAILS.items():
        if phrase not in proposal_text:
            error(errors, f"proposal_guardrail_missing:{guardrail}", proposal_review, f"required high-risk phrase is missing: {phrase}")

    authoring = source / "mdp-pack-builder" / "references" / "proposal-authoring.md"
    authoring_text = authoring.read_text(encoding="utf-8") if authoring.is_file() else ""
    for guardrail, phrase in AUTHORING_GUARDRAILS.items():
        if phrase not in authoring_text:
            error(errors, f"proposal_authoring_guardrail_missing:{guardrail}", authoring, f"required authoring phrase is missing: {phrase}")

    for skill_id, guardrails in FOUNDATION_GUARDRAILS.items():
        skill_path = source / skill_id / "SKILL.md"
        skill_text = skill_path.read_text(encoding="utf-8") if skill_path.is_file() else ""
        for guardrail, phrase in guardrails.items():
            if phrase not in skill_text:
                error(
                    errors,
                    f"foundation_guardrail_missing:{skill_id}:{guardrail}",
                    skill_path,
                    f"required product-foundation phrase is missing: {phrase}",
                )

    for skill_id, guardrails in COLD_MODEL_GUARDRAILS.items():
        skill_path = source / skill_id / "SKILL.md"
        skill_text = skill_path.read_text(encoding="utf-8") if skill_path.is_file() else ""
        for guardrail, phrase in guardrails.items():
            if phrase not in skill_text:
                error(
                    errors,
                    f"cold_model_guardrail_missing:{skill_id}:{guardrail}",
                    skill_path,
                    f"required cold-model phrase is missing: {phrase}",
                )

    return {"model": MODEL, "source": str(source.relative_to(root)) if inside(source, root) else str(source), "skills": skills, "valid": not errors, "errors": errors}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument("--source", type=Path, default=Path("plugin/skills"))
    args = parser.parse_args()
    root = args.root.resolve()
    source = args.source if args.source.is_absolute() else root / args.source
    result = validate(root, source)
    print(json.dumps(result, indent=2))
    return 0 if result["valid"] else 1

if __name__ == "__main__":
    sys.exit(main())
