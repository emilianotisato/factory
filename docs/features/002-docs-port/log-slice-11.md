## Decisions

### AD-1101 — Three swapped skill lines run v2-only verbs · slice-11 · 2026-10-13

**Chose:** swap `node ~/.claude/skills/feature-docs/bin/docs.mjs` for `factory docs` on all 25 command lines as the PRD says, including `prd/SKILL.md:164` and `feature-docs/migration.md:121` (`factory docs check --fix`) and `deliver/SKILL.md:145` (`factory docs log merge {NNN}`). They work in a v2 repo through the shim. In a v3 repo, `check --fix` exits 2 with "unexpected argument '--fix' found" and `log merge` exits 2 with "\"log merge\" removed in v3: the log is appended by `factory docs log add`".
**Over:** rewording them into v3 verbs (`check`, `log add`), which changes what the skills do in every v2 repo the owner still has.
**Because:** the PRD orders a string swap, and the slice says not to reword. Each skill's behaviour in v3 is PRD-004's, where the skills move into this repo.
**Undo:** reword the three lines in `~/.claude/skills`; nothing in this repo depends on them.
**Review:** decide

### AD-1102 — The skills' ideas lines keep a v2 fallback · slice-11 · 2026-10-13

**Chose:** every edited ideas line names `factory docs idea add` (or `new <slug> --from-idea <id>` in `prd/SKILL.md:56`) and keeps a clause for v2 repos: "in a v2 repo (`factory docs doctor` says so), … `docs/ideas.md` … as before". Three lines the PRD did not list got the same treatment so that `ideas.md` appears only in v2 clauses: `prd/SKILL.md:28` (the by-hand check), `prd/SKILL.md:151` (link sideways) and `prd/template.md:31` (Split from). `prd/template.md:66` names `docs/ideas/` in a code span, not a link.
**Over:** the bare v3 wording the PRD gives, and a markdown link to `../../ideas/`.
**Because:** `docs.mjs` has no `idea` verb (its `default:` case exits 2), so in an unmigrated repo `/polish`'s Park step would lose its findings. A template link to `ideas/` is broken in every v2 repo and one to `ideas.md` in every v3 repo; a code span is checked by neither.
**Undo:** drop the v2 clauses once every repo is migrated.
**Review:** decide

## Traps

- The slice's acceptance filter `grep -v '/feature-docs/bin/'` drops every command line too, since each one holds `~/.claude/skills/feature-docs/bin/docs.mjs` in its text, so it passes on an untouched tree. `--include='*.md'` already leaves `bin/docs.mjs` out: count with `command grep -rn 'docs\.mjs' ~/.claude/skills/ --include='*.md'` and no filter.
- The owner's current global pre-commit hook (`~/.config/git/hooks/pre-commit`, v2 `node docs.mjs check`) refuses any commit that stages `docs/` in a v3 repo (`E-NO-BOARD`). Commit in a scratch v3 repo with `GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1`. The owner must paste the `factory docs hook` body before migrating a repo, or its first commit is refused.
- This lane's cargo target is `target/lane-2` (`CARGO_TARGET_DIR`), not `target/debug`; use `env!("CARGO_BIN_EXE_factory")` in tests.
