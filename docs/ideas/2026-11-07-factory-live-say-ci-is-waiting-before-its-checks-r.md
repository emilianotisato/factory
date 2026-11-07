---
title: "factory live: say CI is waiting before its checks register"
state: "open"
added: "2026-11-07"
origin: "PRD-024 polish"
---

`factory live` shows the wrong CI state while checks have not registered yet (AD-16 in PRD-024's log).

**What happens.** `watch_ci` (`src/closeout.rs`, the `CiWaiting` emit guarded by `!checks.is_empty()`) emits nothing while the running set is empty: the grace window after a new head (5 polls), a head lag (`pr_head != head`), and the registration wait after a flaky rerun, whose stale job links are filtered out. `now.waitingOn` (`src/live/now.rs`, `waiting`) only says `ci` when the newest CI event is `ci_waiting` or `ci_no_checks`, so it reads `nothing` ("nothing in flight"), and `ci.state` keeps the previous `red` after a fix is pushed.

**Remedies.**
1. Emit `ci_waiting {checks: []}` once when the set goes empty, deduped like a non-empty set; label it "CI checks not registered yet". Cost: `ci_waiting_emits_on_a_change_of_checks_only` changes, and `["test"]`, lag, `["test"]` becomes three events.
2. Leave the events alone; in `live`, read `waitingOn: ci` when the phase is closeout, the driver is alive and no `ci_green`/`ci_red` followed the newest push or fix commit. Needs a push event or the head in state.

Raised by the second opinion (grok-4.7-high) in three passes during PRD-024 polish.
