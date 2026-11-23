---
title: "factory docs gate --run, so polish's gate lands in the store"
state: "open"
added: "2026-11-23"
origin: "harvest 2026-11-23"
---

**What happened.** `factory docs gate` prints the gate command; it does not run it. PRD-018's polish worker read `skills/polish/SKILL.md` §5, "Run the gate", looked for `--run`, found none, and ran the printed command by hand. The run then exists only in that shell: the checkpoints' gate runs are in the store, polish's is not.

That matters for the retro's cost table, which counts gate runs, and for any future question about how often the gate was red in polish rather than at a checkpoint.

**What it should do.** `factory docs gate --run` runs the gate and records the run the way a checkpoint's does, with its duration and verdict. The bare verb keeps printing the command, so nothing that reads it today changes.

**Minimal shape.** Polish runs `factory docs gate --run` → green in 1.8 min, and the feature's retro Numbers counts 4 gate runs, not 3.
