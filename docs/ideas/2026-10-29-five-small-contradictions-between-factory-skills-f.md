---
title: "Five small contradictions between factory skills, found in one unattended run"
state: "open"
added: "2026-10-29"
origin: "factory upstream"
---

One `/deliver` run hit five places where two skills disagree or one is silent. Each cost a guess, now recorded in that run's log. Each is a one-line fix.

1. **The index Notes cell.** `deliver` §4 says the Notes cell must not carry the PR number, because the PR does not exist yet. `prd-to-tasks` §9 says the Notes cell holds the PR number. `deliver`'s reasoning holds (adding it later costs a CI run), so `prd-to-tasks` should change.
2. **`{date}` names no clock.** The autonomy doc, the agent-log template and `deliver` all write `{date}` without saying local or UTC. A checkpoint subagent stamped UTC while its orchestrator wrote the local date, so one heading in the folder is a day off from the rest. Say "the machine's local date" (or UTC) once.
3. **`polish` §4 is silent on reversing a proven decision.** The mapping says an Ask becomes an AD when "local and reversible", and a BLOCK otherwise. An Ask that supersedes an AD a checkpoint already proved is reversible but not small. Proposal: an AD marked `decide` when its own proof lands in the same pass, a BLOCK when it would need the checkpoint re-driven.
4. **The `test-cleanup` precondition under `/deliver`.** "Working tree clean, or dirty only with the changes under review": the tree holds polish's own uncommitted docs edits at that point. The order (cleanup before the `reviewed:` commit) is deliberate, so the precondition should say polish's own docs edits do not count as unrelated changes.
5. **`pr-body` "Gate: {runs} runs".** Full-gate runs only, or inner loops too? Define `{runs}` as full-gate runs (matching the retro's `gate_runs`) and give inner loops their own clause.
