---
title: "gate: allow a tier that runs only at the last checkpoint"
state: "open"
added: "2026-11-20"
origin: "factory upstream"
---

**What happened.** The project declares one `gate` command, and every checkpoint runs it whole. Its mutation tier takes 400 to 550 s per run. Across three features (8 checkpoint runs) it caught nothing the inner loop had not already caught. It is still the only check for assertion loss, so the owner wants to keep it, but only once per feature.

**What it should do.** Let `docs/project.toml` declare a tier that runs at the last checkpoint and in polish only (for example `final = "composer mutation"`). Earlier checkpoints run `gate` without it, and the plan line says which tier ran.

**Minimal shape.** `checkpoint_every = 2` on a 6-slice feature: CP-1 and CP-2 run the gate without mutation (about 8 minutes saved each), CP-3 and polish run gate plus final.
