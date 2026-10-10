# Ideas Inbox

Loose one-liners for things not yet worth a PRD. Cheap to add, cheap to drop.

**Rules**
- One line per idea. Enough to remember *what* and *why*, no more.
- No design, no scope, no tasks here — that's what a `features/{NNN}/prd.md` is for.
- When an idea earns real work: `/prd` → new `features/` folder, then delete the line here (or strike it).
- Not a backlog with priorities. Just a catch net so ideas stop dying in chat scrollback.
- A line cut from a PRD keeps its recon — say which PRD dropped it, when, and why, so a future PRD doesn't redo the research.

## Open

- **Codex or Cursor as build workers.** Different flags and stream; no evidence yet they are cheaper for slice work. Left out of PRD-001 (2026-10-09) until the metrics store can compare per-worker cost.
- **Decompose polish into driver-run steps.** Polish is the largest token line after the build; PRD-001 keeps it as one `claude -p` worker so the whole loop ships first. Revisit after the first A/B run with the per-worker numbers.
- **Typed lookup layer for `factory ctx` (LSP or SCIP).** gopls, rust-analyzer, phpactor or PHPantom, tsserver with the Vue plugin, held by the driver and fed file changes after each slice. Left out of PRD-001 (2026-10-09): 6–69 s cold and 0.3–3 GB per server, intelephense licence, Vue forwarding client, and the one controlled study found LSP cost more tokens on Opus and Sonnet. Open only if FR-8 shows `def` and `refs` still heavy after FR-12. Recon: `features/001-driver/ctx-research.md` §3–4.
- **Jev (TypeSafe System One) as a picker in `factory ctx` or the driver.** Non-generative Choice/Score/Noul API, about 300 ms from here, $0.042 per million input tokens. Assessed and parked 2026-10-09: nothing to disambiguate in `def` (91.2% single hit), Jev-scored window selection for reads missed the edited lines 30% of the time at 64% of bytes kept, block auto-answer lost to "pick the recommended option" (85.0% against 88.2%). Re-open only if FR-8 shows follow-up reads after `ctx` above 30%, and then on public code until the clients consent in writing (TypeSafe's MCA puts the consent duty on the customer). Recon: `features/001-driver/ctx-jev-research.md`.

## Graduated → became a feature

- _(move lines here with their `{NNN}` when they get a PRD, or just delete)_
- **v3 docs format, at port time (PRD-002).** State out of markdown (`state.toml`, rendered on read), append-only JSONL log replacing fragments and `log merge`, `project.toml` instead of index frontmatter, frontmatter-only PRD header, feature-scoped FR ids, retro numbers in the metrics store. Why: half of `docs.mjs` is drift machinery for derived state stored twice; porting it and removing it later costs more than a migration done once. Agreed 2026-10-09.
