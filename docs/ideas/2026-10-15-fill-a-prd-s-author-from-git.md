---
title: "Fill a PRD's author from git"
state: "open"
added: "2026-10-15"
origin: "PRD-008 prd"
retrigger: "a second person writes a PRD in a factory repo"
---

Cut from PRD-008 (FR-9) on 2026-10-14: nobody needs it while the repo has one author. `factory docs new` writes `author: _(name)_` and never fills it (`src/docs/scaffold.rs:486` lists the key). In a team it is the only place a PRD says who wrote it. The shape: fill it from `git config user.name`, keep the placeholder when that is empty; acceptance is a fixture with `user.name = "Ada"` where `factory docs new x` writes `author: Ada`.
