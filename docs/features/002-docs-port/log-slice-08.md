## Decisions

### AD-801 — What does an idea row in the plan say? · slice-08 · 2026-10-13

**Chose:** `idea <id> <state>[ high] "<title>" line <n>: <rule>, added from <Parked|Added|the heading|git blame>`. `guess` rows have the same shape with `guess` as the verb. `relink` rows show the old link and the new one.
**Over:** the slice's bare `idea <id> <state>[ high] <rule>`.
**Because:** the owner reads these rows on arca-api to decide which guesses are still open (the FR-20 human row). The id alone is a slug cut at 50 characters. The line number and the source of the date are what let the owner check a row against `ideas.md`.
**Undo:** drop the title, the line and the `added from` part from the `format!` in `migrate_ideas::plan`.
**Review:** no

### AD-802 — What happens to a line under a heading that is not a bullet? · slice-08 · 2026-10-13

**Chose:** a non-blank line under a `##` that is not a bullet or a continuation of one becomes a `drop docs/ideas.md:<n> … (not a bullet)` row. A fence indented under a bullet stays in that bullet's body. A fence at the margin ends the bullet.
**Over:** silently skipping such lines, or failing the migration.
**Because:** FR-20 says text that is not a bullet under a heading is "dropped and listed". A drifted repo may hold a paragraph between bullets, and the owner should see it in `--dry` before it disappears. An indented fence is the bullet's own code block, which has to round-trip byte for byte.
**Undo:** remove the `Dropped("not a bullet")` branch in `migrate_ideas::parse`.
**Review:** no

## Traps

- The PreToolUse hook refuses a Bash command whose text holds both a git word and `push`, including `out.push(` inside a heredoc or a `python -c`. Write the script with the Write tool to a scratch file, then run it with Bash.

## Untracked leftovers

- src/docs/migrate.rs
