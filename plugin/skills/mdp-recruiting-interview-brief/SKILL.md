---
name: mdp-recruiting-interview-brief
description: Use when the user wants a job-related interview-question brief from a Recruiting MDP pack and supplied role/candidate evidence. Produces neutral questions, evidence to listen for, gaps, and prohibited inferences for a human interviewer; never schedules, evaluates protected traits, or decides an employment outcome.
---

# MDP Recruiting Interview Brief

## Profile Gate

Run `mdp --json agent-surface --dir .`. Continue only when this skill is allowed and not blocked.

## Inputs

Require the pack directory, supplied role criteria with job-related rationale, supplied candidate evidence if relevant, source classification, interview scope, and human interviewer. If the role criterion or source is missing, return gaps instead of drafting a question.

## Workflow

1. Run strict validation and gaps.
2. Normalize messy context with `normalize-recruiting-context` and validate it.
3. Route with:

```bash
mdp --json --summary route --entries --dir . --persona "Interviewer" --job "interview brief"
```

4. Build neutral, consistent questions tied to the stated work. Include evidence to listen for and a note-taking gap, not a preferred personal style.
5. Exclude questions or inferences about protected characteristics, medical or disability information, pregnancy, family status, religion, national origin, age, sex, sexual orientation, gender identity, appearance, voice, commute, school prestige, culture fit, or other non-job-related proxies.
6. Do not schedule interviews, contact candidates, update systems, produce a candidate score, or recommend an outcome.

## Output

For each question return the job-related criterion and source, neutral question, evidence to listen for, permitted follow-up, prohibited inference, open gap, and human reviewer note.

End with `Needs human review`. The brief supports an interviewer; it does not make or encode an employment decision.
