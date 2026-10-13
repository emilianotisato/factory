# QA guide — PRD-004 The skill family ships from this repo

Generated 2026-10-13 from [`prd.md`](prd.md) § QA plan: the three Human rows, **FR-6** (a live second opinion after `cursor-agent login`), **FR-2** (the rendered family in `arca-api`, as three agents see it) and **FR-8** (the first tag, the release, the installer). Every other row is proven under `cargo test` at the checkpoints; the evidence is in the slice files' Progress. ★ marks the checks that matter most.

Budget: about 45 minutes in all. FR-6 runs any time after slice 04, before or after the merge. FR-8 and FR-2 run after the merge, in that order: the tag first, then the migration of this machine, then `arca-api`.

## FR-6 — A live second opinion (≈10 min, after slice 04)

Needs: the branch binary at or after slice 04 on `PATH` (`cargo install --path .` from the branch), and a terminal in this repo on `feat/skills`.

1. Log in. *(1 min)*
   ```
   cursor-agent login
   ```
2. ★ One full review of the branch. *(5 min)*
   ```
   factory review second --diff main; echo "exit $?"
   ```
   **Pass:** one `## Pass n — <files> (<lines> lines, <seconds> s, <model>)` header per pass, findings under each, the `log:` path last, exit 0. **Fail:** `unreviewed: <reason>` and exit 2 (the login did not take; run step 1 again), or any pass whose text appears twice.
3. ★ The partial path. *(3 min)* Add one line to `~/.config/factory/config.toml`, keeping what is already there:
   ```
   [review]
   timeout_secs = 60
   ```
   ```
   factory review second --diff main; echo "exit $?"
   ```
   **Pass:** at least one pass prints text under `(timed out after 60 s, partial)`, the text reads as one run of prose, not the same sentences repeated, and the exit is 0. This is the case slice 04 could not record: the live shape of `--stream-partial-output`. **Fail:** repeated text means each delta was printed and then the full message again; note it for a fix.
4. Remove the `timeout_secs` line. *(1 min)*

Evidence: both stdouts and exit codes in slice 04's Progress.

## FR-8 — The first release (≈15 min, after the merge)

Needs: the merged `main` pulled, in a plain terminal (the pre-push hook refuses Claude Code sessions, and the SSH key asks for its passphrase).

1. Check the version on `main`. *(1 min)*
   ```
   git checkout main && git pull
   grep '^version' Cargo.toml skills/bundle.lock
   ```
   **Pass:** both say the same version, `0.2.0` unless a skill edit after slice 07 moved it to `0.2.1`. Tag what they say.
2. ★ Tag and push the tag. *(1 min, then the workflow's ≈10 min)*
   ```
   git tag v0.2.0 && git push origin v0.2.0
   gh run watch
   ```
   **Pass:** the release workflow goes green. The release page lists four archives (`x86_64` and `aarch64`, Linux and macOS), their checksums and `factory-installer.sh`. **Fail:** a red `aarch64-unknown-linux-gnu` job is the risk slice 06 named; its Progress says how dist builds that target.
3. ★ Install from the release into a throwaway home, so your real `~/.cargo/bin/factory` and `~/.config/factory` are untouched. *(3 min)* `XDG_CONFIG_HOME` and `CARGO_HOME` are set on this machine, so unset both. Use a second machine instead if one is at hand.
   ```
   T=$(mktemp -d)
   env -u XDG_CONFIG_HOME -u CARGO_HOME HOME=$T sh -c \
     'curl --proto "=https" --tlsv1.2 -LsSf https://github.com/emilianotisato/factory/releases/latest/download/factory-installer.sh | sh'
   ls $T/.config/factory/
   AXOUPDATER_CONFIG_PATH=$T/.config/factory $T/.cargo/bin/factory --version
   AXOUPDATER_CONFIG_PATH=$T/.config/factory $T/.cargo/bin/factory upgrade --check
   ```
   If the installer prints another install path, use that one in the last two lines. **Pass:** `factory-receipt.json` is listed, `--version` says `factory 0.2.0`, and `upgrade --check` says `already current`. **Fail:** `installed from source` means the receipt's `install_prefix` is not the binary's directory; read the receipt.

Evidence: the release URL, and the three outputs of step 3, in slice 06's Progress.

## FR-2 — The family in `arca-api` (≈20 min, after the merge)

Claude Code reads a personal skill over a project skill of the same name (prd §2). While `~/.claude/skills` still holds the eight family folders, every check below would show the Dropbox copy. So this part starts with the migration §2 describes.

1. Install the merged binary. *(2 min)*
   ```
   cargo install --path ~/Projects/factory
   factory --version
   ```
2. Move the eight family folders out of the Dropbox repo and commit there: `prd`, `slice`, `polish`, `gate`, `harness`, `prune`, `ux`, and the `deliver` symlink. `feature-docs/models.md` goes too once nothing outside the eight links it. *(5 min)* **Pass:** `ls ~/.claude/skills/` shows none of the eight.
3. ★ Initialise `arca-api`. *(2 min)*
   ```
   cd ~/Projects/Prokey/arca-api
   factory init
   factory docs doctor
   ```
   **Pass:** `init` prints the skills line, the convention-block line and one `commit:` line; doctor's `skills` line says `current (0.2.0)`. Run the printed commit line.
4. ★ Claude Code sees the project copy. *(3 min)* Start `claude` in `arca-api` and type `/`. **Pass:** `/prd`, `/slice`, `/deliver`, `/polish`, `/gate`, `/harness`, `/prune` and `/ux` are listed, and `/prd`'s description matches `.claude/skills/prd/SKILL.md`.
5. Codex sees it. *(2 min)* Start `codex` in `arca-api` and open its skill list. **Pass:** the eight names appear, reached through `.agents/skills/`.
6. ★ Cursor lists each one once. *(3 min)* Start `cursor-agent` in `arca-api` and open its skill list (type `/`). **Pass:** each family name appears once. **Fail:** each appears twice, once per root. This is the one open fact of §11; note it, since the fix is a later decision, not part of this PR.

Evidence: one line per agent in slice 02's Progress, and the Cursor answer as a dated line under §11 of the PRD.
