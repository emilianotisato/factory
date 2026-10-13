---
title: "Run the CI checks locally before the push (`nektos/act` or similar)"
state: "open"
added: "2026-10-11"
---

`act` runs a GitHub Actions workflow in Docker on this machine; `gh act` is the same as a `gh` extension. Why: each CI fix round costs a push plus minutes of waiting, and PRD-001's one red round (2026-10-11) was a test race that the local gate never showed. Check two things first: how far act's runner images differ from GitHub's, and whether a cheaper CI-shaped local gate (fewer cores, cold cache) catches the same class. Added 2026-10-11, not assessed yet.
