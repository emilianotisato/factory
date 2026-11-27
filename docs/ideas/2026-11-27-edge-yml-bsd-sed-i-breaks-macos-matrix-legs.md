---
title: "edge.yml: BSD sed -i breaks macOS matrix legs"
state: "open"
added: "2026-11-27"
origin: "PRD-016 polish"
---

**Recon:** `.github/workflows/edge.yml` build jobs run `sed -i 's/...' Cargo.toml`. macOS runners use BSD sed where `-i` requires a backup extension; the patch step fails and `publish` never runs. Polish cannot edit workflow files per unattended Never list.

**Fix:** Use `perl -pi` for Cargo.toml like Cargo.lock already does, or `sed -i.bak` and delete backup.
