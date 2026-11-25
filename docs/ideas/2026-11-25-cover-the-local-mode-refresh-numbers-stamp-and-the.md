---
title: "Cover the local-mode refresh-numbers stamp and the once-per-head ci_local"
state: "open"
added: "2026-11-25"
origin: "PRD-025 polish"
---

Two local-CI-mode behaviours have no test that goes red when they break (second opinion, PRD-025 polish):

- `refresh_numbers` in `src/closeout.rs` stamps ` [skip ci]` in local mode. The closeout fixtures leave `retro.md` clean at push, so `commit::commit` returns `None` and no `refresh numbers` commit is made. Dropping `skip_ci` there keeps every test green. A test needs a retro that is dirty at push in local mode: either a driver killed between the retro write and its commit, or numbers that change between ship and push. `tests/closeout.rs` already reaches the remote variant after a CI fix round (`subjects(&fx)[7..]` ends in `docs(001): refresh numbers`).
- `ci_local` emits once per head (AD-4). The scheduler calls `closeout::run` once, so the head compare never sees a second event. A test needs a local closeout that is re-entered after its push moved HEAD, for example through a block answered after `open_pr`.
