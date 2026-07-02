# Proposal Boundaries

Use this before accepting proposal proof, compliance, security, certification, past-performance, pricing, or approval claims.

## Hard Boundary

MDP can structure review context and surface gaps. It cannot certify compliance, approve procurement language, submit proposals, replace legal/security/procurement review, or invent proof.

## Never Invent

- Certifications
- Compliance status
- Security posture
- Customer references
- Past performance
- Pricing
- Evaluator criteria
- RFP text
- Deadlines
- Procurement vehicle eligibility
- Approval status
- Incumbent facts
- Named customer outcomes

If the user or source did not supply it, put it in `gaps.yaml` or `rejected_claims`.

## Safe Language

Prefer:

- "source material does not establish"
- "needs reviewer confirmation"
- "gap: missing proof"
- "unsupported claim"
- "review required before use"
- "local-first review support"
- "customer-controlled context"

Avoid:

- "certified"
- "approved"
- "compliant"
- "guaranteed"
- "we can submit"
- "legal/procurement approved"
- "safe for regulated data"

## Claim Check

Run a claim check for any claim-bearing text:

```bash
mdp --json check-claims --dir . --persona "Proposal Lead" --job "compliance review" --text "<claim-bearing text>"
```

Use `--strict` when warnings should block acceptance.
