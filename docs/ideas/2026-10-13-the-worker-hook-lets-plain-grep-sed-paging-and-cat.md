---
title: "The worker hook lets plain grep, sed paging and cat through; ctx is 58 of 1,603 lookups"
state: "open"
added: "2026-10-13"
origin: "PRD-004 recon"
retrigger: "the next factory run after a hook change"
---

Across five runs in the metrics store (2026-10-13): 453 `bash-grep`, 789 `bash-sed` pages and 303 `bash-cat` tool calls against 58 `ctx` calls (`ctx-refs` 31, `ctx-def` 20, `ctx-outline` 7). `src/cli/hook.rs` denies only `grep -r`/`-R` and `rg` over a directory, so a plain `grep`, a `sed -n` page and a `cat` all pass. PRD-001 FR-12 built `ctx` because lookups were 56 to 74% of worker calls; the hook does not steer workers to it. To decide: widen the deny (every `grep` and `rg` not on a literal string, `sed -n` and `cat` over a file the pack already holds or that `ctx def` would answer), or make the hint stronger, or both; then measure the next run against these counts.
