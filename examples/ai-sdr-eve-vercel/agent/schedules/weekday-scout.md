---
cron: "0 14 * * 1-5"
---

Run the MDP AI SDR scout loop once.

Load the active source strategy, target at least 3 qualified people, continue across approved account-discovery prompts until the target or bounded exhaustion, run MDP validation/fit/brief/check-claims gates, append qualified ledger rows, and finish with a short run report. Use fixture data only for explicit dry-runs; scheduled live runs without provider keys must report the gap and append no rows. Do not send outreach or update CRM.
