---
title: "Run gates under a memory cap"
state: "open"
added: "2026-11-01"
origin: "owner"
retrigger: "any gate or worker pushes the machine into swap, or oomd kills a run"
priority: "high"
---

A runaway gate kills the owner's terminal, and the session driving the run dies with it.

On 2026-11-01 sii-api's `/deliver 006` died twice (01:34 and 07:54), each time about
4 minutes into checkpoint 1's `make verify`. Its mutation tier ran a gremlins mutant of
`cut()` in `internal/printable/receipt/receipt.go` that made `wrap()` append empty lines
forever. Each test binary grew about 0.7 GB/s, 4 workers ran at once, and the terminal's
scope peaked at 80 GB RAM plus 67 GB swap. systemd-oomd then killed the whole terminal
scope: 183 and 177 processes, the Claude session, the factory driver and every worker.
Reproduced under `systemd-run --user --scope -p MemoryMax=12G -p MemorySwapMax=0`: the cap
killed the runaway tests and nothing else.

The driver and every gate, setup and worker it spawns live in the terminal's cgroup, so
any one of them can take down everything. The repo-side fix (a per-process memory cap in
sii-api's `scripts/mutation.sh`) covers one gate in one repo.

Options for factory itself:
- Run each gate (`src/checkpoint.rs`, the `sh -c` spawn) and each lane setup
  (`src/lanes.rs`) in its own scope with `MemoryMax` and `MemorySwapMax=0` when
  `systemd-run --user` is available. A runaway then fails the gate with a clear
  "gate hit its memory cap" message, which the run can report like any red gate.
- Or start the driver in its own scope, so oomd picks the run and spares the session.
- Default the cap to a share of RAM; let `[docs]` or `[lanes]` in `docs/project.toml`
  override it. Without systemd (macOS, containers), fall back to running uncapped and
  say so once in preflight.
