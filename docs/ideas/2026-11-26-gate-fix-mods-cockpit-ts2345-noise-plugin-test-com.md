---
title: "gate: fix mods/cockpit TS2345 noise, plugin test command, and discover_models ETXTBSY flake"
state: "open"
added: "2026-11-26"
origin: "harvest 2026-11-26"
---

From PRD-019 and PRD-027 retros:
- `claude plugin validate mods/cockpit` prints TypeScript `TS2345` errors on every gate run while exiting 0, adding noise next to real failures.
- `claude plugin test mods/cockpit` warns about command spelling and does not actually execute cockpit tests in the gate.
- `provider::tests::discover_models_parses_live_outputs_and_kills_hung_probe` flaked once with an `ETXTBSY`-style race on a freshly written stub executable.
