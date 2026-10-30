# Stale tooling — what step 0 does with the skills line

`/prd`, `/slice`, `/deliver` and `/lanes` share this file. `factory docs doctor` prints a `skills` verdict: a version line, then `hand-edited: …` and `missing: …` when they apply, so one verdict can carry several lines. Act on it **without asking**, except for a hand edit:

- **Any `hand-edited:` line** → ask first, one `AskUserQuestion`: **Restore the files and update (Recommended)** runs the `git checkout -- …` lines `factory skills update` prints, then the bullet below; **Keep the edit** skips the update. It wins over every other line, because `factory skills update` refuses while a hand edit stands.
- **`ahead (…) → <hint>`** (the binary is older than the repo's skills) → run the command after the `→`, then `factory docs doctor` again.
- **`behind`, `differs` or `missing`**, no hand edit → `factory skills update`, then the `commit:` line it prints, on the current branch.
- **`source checkout`** (the factory repo itself) or **`current`** → nothing.

Then continue with the skill's own routing on the rest of doctor's output.
