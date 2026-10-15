---
title: "Archive harvests what a feature taught"
state: "open"
added: "2026-10-15"
origin: "owner, PRD-007 review"
retrigger: "the next retro-analysis session, or the first team repo"
---

`factory docs archive` only moves a folder, so nobody runs it: on 2026-10-14 factory had seven features done and none archived. Done features stay in `docs/features/` because their retros are worth reading together, and the owner does read them in batches to improve the project's harness and the framework. Give archive that meaning: **archive = the lessons are harvested.** Read a batch of done features, route every lesson to where it belongs, then archive the batch.

**Why agents need it.** A feature folder mixes records and instructions. A record (a log entry, a retro, "on 10-15 we chose X") stays true after the code moves. An instruction goes stale and still sounds sure: slice-04 of PRD-007 places `branch_guard` at `src/closeout.rs:1109-1139`, and one day later it starts at line 1203. A future agent, or a teammate's agent on another machine, searches the repo and trusts what reads as current. Slices are the worst case, and they are two things glued: the recon and the tasks are instructions, while the "Code-review fixes" and the plan-versus-actual story are exactly what the retro analysis reads. So slices stay until the harvest, then leave for git history.

**Lean: what archive keeps.** `prd.md` with a banner ("History. The code, `docs/reference` and `docs/decisions.md` win.": PRD-007's AD-6 already overrode FR-5 as written), `state.toml` (the folder keeps the next number countable), `log.jsonl` and `retro.md` (records). It drops the slices, and any evidence still in the tree (the team-repo idea says evidence should not be there at all). Agents outside a factory run get the same rule from one line in the convention block `factory init` writes into `CLAUDE.md`: `docs/features` and `docs/archive` are history; the code, `docs/reference` and `docs/decisions.md` win.

**Lean: where a lesson goes.** The brainstorm's three scopes, project → user → base:
- **The project**: the harness (gate, verify skill, lanes, `docs/ux-rubric.md`), `CLAUDE.md`, `docs/reference`, a decision. A diff in this repo.
- **The framework in this project's context**: a `docs/project.toml` knob, a project overlay of a skill.
- **Factory itself**: with a factory checkout on the machine, an idea file in its `docs/ideas` (the friction files already travel this way by hand: `2026-10-15-friction-cody-agent-prd-026.md` came from cody-agent). Without one, for anyone else running factory, a script drafts a GitHub issue on `emilianotisato/factory` that the maintainers take or leave. ⚠ That repo is public: an insight from a client repo is rewritten in general terms (no client or customer names, domain data or local paths) and shown to the person before it posts, and it searches open issues first so a repeat becomes a +1, not a duplicate. The cody-agent friction file above already carries a client project's local path.

**Lean: decisions get compacted at the harvest.** Today `DEC-n` is monotonic and never reused, a reversed one is superseded but never deleted, and each heading names its origin (`PRD-011 slice-06`). The owner asked why number and source them at all. Piece by piece:
- **The origin** points into slices the harvest drops, and `git log -S` on `decisions.md` finds the PR anyway. The **Because** line carries the incident in plain words instead.
- **The number's** one real job is a stable link target: arca-api's architecture and operations docs link `decisions.md#dec-n` anchors 11 times. Logs and PR bodies that cite a number are records and may go stale. Named slugs (`#public-reads-are-pure`) keep the link, survive a rewrite, and cannot collide the way two developers' `DEC-22`s can. A merged entry keeps its old slug as an extra anchor.
- **Supersede-not-delete** keeps the road not taken, and the **Over** line already does that. Fold the loser into the winner's Over and delete the dead entry.

So the harvest may merge, rewrite, regroup by topic and delete an entry whose **Applies when** no longer exists in the code, as a diff the owner reviews. Two rules hold: an **Over** is never lost (it is what stops an agent re-proposing the rejected option), and a decision that is really a domain fact moves to the doc that owns the topic (arca-api's DEC-10, DEC-11, DEC-15 and DEC-16 read like ARCA rules, not engineering forks). The skills citing `DEC-n` change with it: polish (the lookup, "cite it", the entry template, the numbering rule), ux, gate.

**Relation to the learn loop.** The brainstorm planned an event-driven `learn` step (retros, overturned ADs, parks, repeated reds; never an unattended rule edit). The harvest is its human end: archive marks a batch whose events were read and routed.

Open: what makes a batch (a count, an age, the owner's call); the split between a verb (the mechanical moves, the slug anchors, the issue draft) and a skill (the routing judgment, with the owner).
