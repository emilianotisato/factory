---
title: "Harden review/collect integration tests (signals, ship head)"
state: "open"
added: "2026-11-27"
origin: "PRD-029 polish"
---

Integration tests for signals and closeout ship lack timeouts, zombie-safe pid checks, SIGINT/SIGHUP coverage, and an assertion that `closeout::ship` writes `reviewed_head` from collect head (today `let _ =` swallows set errors).
