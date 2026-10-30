# Stale tooling — what step 0 does with the skills line

`/prd`, `/slice`, `/deliver` and `/lanes` share this file. `factory docs doctor` prints a `skills` verdict: a version line, then `hand-edited: …` and `missing: …` when they apply, so one verdict can carry several lines. Act on it **without asking**, except for a hand edit:

- **Any `hand-edited:` line** → ask first, one `AskUserQuestion`: **Restore the files and update (Recommended)** runs `git checkout -- .claude/skills/<file>` for each file the line names, then the bullet below that matches the version line; **Keep the edit** skips the update. It wins over every other line, because `factory skills update` refuses while a hand edit stands.
- **`ahead (…) → <hint>`** (the binary is older than the repo's skills) → run the command after the `→`, then `factory docs doctor` again.
- **`behind`, `differs` or `missing`**, no hand edit → `factory skills update`, then the `commit:` line it prints, on the current branch.
- **`source checkout`** (the factory repo itself) or **`current`** → nothing. **`none → factory init`** or **`unreadable …`** → the skill's own routing below.

An update that rewrote the running skill's own `SKILL.md` → invoke that skill again with the Skill tool, same arguments, from its step 0: the text you are following is the old one.

Then continue with the skill's own routing on the rest of doctor's output.
