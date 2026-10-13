# Ideas Inbox

Things not yet worth a PRD, one file per idea. Cheap to add, cheap to drop. A file in this folder means the idea is open; there is no "graduated" or "dropped" state. Leaving is deleting, and git history keeps every idea that left.

**Rules**

- One idea per file, `YYYY-MM-DD-slug.md`: the day it was added and a slug from its title. The filename stem is the idea's id.
- Write one with `factory docs idea add --title "…" --body-file -`, or by hand. `factory docs check` validates every file here but this one.
- `factory docs idea list` shows the open ideas; `--json` is what tools read.
- The frontmatter:
  - `title` (required)
  - `state` (required): `open`, or `owed` when someone outside the repo owes an answer, and the body says who
  - `added` (required): the date, `YYYY-MM-DD`
  - `origin` (optional): where it came from, such as `owner`, `PRD-016 polish` or `cut from PRD-012 FR-4`
  - `retrigger` (optional): the event that should make someone look again
  - `priority` (optional): only `high`; absent is normal
- The body says *what* and *why*, as long as the recon needs. No design, no scope, no tasks: that is what a PRD is for.
- When an idea earns real work: `factory docs new <slug> --from-idea <id>`. The PRD gets `from_idea: <id>` and the idea's body under `## From idea <id>`, and the idea file is deleted.
- When an idea is dropped: `factory docs idea drop <id> --because "…"`. It deletes the file and commits it, so the reason lives in `git log -- docs/ideas/`.
- Not a backlog. Nothing is ranked; `priority: high` is the one mark there is.
- An idea cut from a PRD names that PRD in `origin` and keeps its recon in the body, so a future PRD does not redo the research.
