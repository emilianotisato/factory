# Skills

Claude Code skills that ship with `factory`. `factory init` in a repo writes these into `.claude/skills/`, byte for byte, with a link per skill in `.agents/skills/` for Codex and a manifest that says which release they came from.

In this repo `.claude/skills/<name>` are links to these folders instead, so a session here reads `skills/` live and an edit takes effect without a release.

| skill | what it does |
|---|---|
| [`prd`](prd/SKILL.md) | interview a feature or bugfix into a PRD verified against the real code, registered on the board |
| [`slice`](slice/SKILL.md) | slice a PRD into vertical, independently shippable slice files with recon, acceptance and dependencies |
| [`deliver`](deliver/SKILL.md) | the cockpit: start a run, wait on it, relay its questions, run the review interview |
| [`polish`](polish/SKILL.md) | the closing ritual before the merge: review, security, UX and a second opinion, triaged, then test cleanup and doc closeout |
| [`gate`](gate/SKILL.md) | interview a project into one verify gate of tiered checks, proven to fail, declared in `docs/project.toml`; `gate audit` reviews one |
| [`harness`](harness/SKILL.md) | build or maintain the project-local skill that drives the real app and captures proof |
| [`prune`](prune/SKILL.md) | clean up, deduplicate and stabilize a test suite without losing coverage |
| [`ux`](ux/SKILL.md) | review changed UI against the project's design rules by driving the real app; builds `docs/ux-rubric.md` |

[`_shared/models.md`](_shared/models.md) is not a skill: it holds the model roles the skills share.
