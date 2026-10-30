# Slice 03 — factory cockpit install

**Source**: [`prd.md`](prd.md) → §7 FR-5, and §6 FR-4's "The binary embeds it with `include_str!`". **Depends on**: 02 (it embeds the files under `mods/cockpit/` and compares its output to them byte for byte, so they must exist)
**Created**: 2026-10-30

**Progress**: `src/mods.rs` embeds the five files under `mods/cockpit/` (`COCKPIT_FILES`). `factory cockpit install` (`src/cli/cockpit.rs`) writes `<data_dir>/mods/.claude-plugin/marketplace.json`, removes and rewrites `cockpit/`, then reads `claude plugin marketplace list --json` and `claude plugin list --json` and runs `marketplace add` / `install … --scope user` only for what is missing. It prints `run /reload-plugins in open sessions`. With no `claude` on PATH it writes the folder, prints both commands and exits 1. The stub `claude` gained a `plugin` branch: it records the call, then answers from `$FACTORY_STUB_DIR/plugins.json`. Tests are in `tests/cockpit_install.rs`: the bundle walk, the install-twice run, and the run with no claude. Inner loop: 2 runs, 0 red, about 3 min (the second run was there only to see the summary lines the first run's output cut off). Deviations: none. Prove (FR-5 transcript) is left for the checkpoint.
- Prove FR-5 (checkpoint 2): `cp-2/fr5-cockpit-install.txt`: `install_writes_the_marketplace_once_and_rewrites_the_mod` green under a tempdir `HOME`, the fixture's data dir and the stub `claude`: the first install adds the marketplace and the plugin, the second only lists, removes a stray file and restores an edited `register.tsx`; 3 passed, 0 failed.

**Slice**: Adds the one verb that puts the mod into Claude Code. The binary carries `mods/cockpit/` inside it. `factory cockpit install` writes a folder marketplace under the data dir, registers the marketplace and the plugin at user scope only when they are missing, and tells the owner to `/reload-plugins`. Re-running it is the upgrade path the mod's stale suffix names.

## Current code / environment

- **The embedding precedent**: `src/skills.rs:25-81`, a `FILES: &[(&str, &str)]` list of `include_str!` pairs. `tests/skills_bundle.rs:27` (`every_file_under_skills_is_in_the_bundle_byte_for_byte`) walks `skills/` and fails on a file the list misses. Do the same for `mods/cockpit/`, skipping `.claude-plugin/types/`, which the engine writes into a loaded mod (slice 02 gitignores it).
- **The subcommand pattern**: `src/main.rs:54-62` (`Skills { verb }`, `Lanes { verb }`) and the dispatch at `:104-105`. `crate::cockpit` (`src/cockpit.rs`) is the run-events module; a new `src/cli/cockpit.rs` is a separate path, so keep the two apart by what each holds.
- **The data dir**: `paths::data_dir()` (`src/paths.rs:13`) honours `FACTORY_DATA_DIR`, then `XDG_DATA_HOME`. The marketplace lives at `<data_dir>/mods/`.
- **The marketplace file**: `.claude-plugin/marketplace.json` needs `name`, `owner: { name }` and `plugins: [{ name, source }]` (plugin reference, 2.1.312). Here: `name` `factory`, one plugin `factory-cockpit` with `source` `./cockpit`.
- **The `claude` CLI, probed 2026-10-30 on 2.1.312**: `claude plugin marketplace list --json` prints an array of `{ name, source, … }`. `claude plugin list --json` prints an array of `{ id, version, scope, enabled, … }`, where the id reads `factory-cockpit@factory`. `claude plugin install <p>@<mkt> --scope user` installs at user scope. With an empty `HOME` the marketplace list is `[]`.
- **The stub `claude`**: `tests/stubs/claude` treats every call as a worker spawn and needs `FACTORY_STUB_DIR`. Its one early exit is `--version` (`:72-76`). It records argv in `$FACTORY_STUB_DIR/calls/<n>.json` (`:63`, `:115-117`).
- **The harness env**: `Fixture::command` (`tests/harness/mod.rs:384-388`) sets the harness env (`:600-620`: `PATH` with the stubs, `FACTORY_DATA_DIR`, `CLAUDE_CONFIG_DIR`) but not `HOME`, which the child inherits. FR-5's proof sets `HOME` to a temp path itself (prd §10 "Seed state").
- **Deliberate non-changes**: `factory skills update` does not refresh the mod (prd §7: it "misses source installs and repos that are already current", `src/cli/skills.rs:33-38`, `:67-69`). `status` never writes the mod (§7). No `factory docs doctor` line (§12).

## Tasks

### 1. Embed the mod — high — blocked by: —
**Source**: §6 FR-4: "The binary embeds it with `include_str!`, as it embeds the skills (`src/skills.rs:28-81`)."

A module beside `src/skills.rs` holding the `(path, contents)` list of every tracked file under `mods/cockpit/`.

**Acceptance:** `every_file_under_mods_cockpit_is_in_the_bundle_byte_for_byte` walks `mods/cockpit/` (skipping `.claude-plugin/types/`) and fails on a file the list misses or holds with different bytes.

### 2. `factory cockpit install` — high — blocked by: 1
**Source**: §7 FR-5, steps 1 to 3: "It writes a folder marketplace at `<data_dir>/mods/` … The bundled files go under `cockpit/`, replaced whole." "Unless `claude plugin marketplace list` already shows it, it runs `claude plugin marketplace add <data_dir>/mods`." "Unless `claude plugin list` already shows it, it runs `claude plugin install factory-cockpit@factory --scope user`." "It then prints `run /reload-plugins in open sessions`." "With `claude` not on PATH, it writes the folder, prints the two commands to run by hand, and exits 1."

`src/main.rs`, a new `src/cli/cockpit.rs`. Replaced whole means remove `cockpit/` and write it again, so a file dropped from the bundle does not linger. Read both lists with `--json`, matching the marketplace by `name == "factory"` and the plugin by `id == "factory-cockpit@factory"`.

**The trap**: the stub `claude` answers every call as a worker. Give it an early `plugin` branch beside `--version` (`tests/stubs/claude:72-76`): it records the argv as it records every call, answers the two `--json` lists from a small state file in `FACTORY_STUB_DIR`, and marks the marketplace or plugin present on `add` and `install`. Never start a worker stream for a `plugin` call.

**Acceptance:** `install_writes_the_marketplace_once_and_rewrites_the_mod`, with `HOME` and `FACTORY_DATA_DIR` set to temp paths and the stub on PATH. The first run writes `marketplace.json` and `cockpit/`, and calls `marketplace add` then `install`, in that order. A stray file put in `cockpit/` is gone after the second run, which calls neither `add` nor `install`. The written `cockpit/` equals `mods/cockpit/` byte for byte. With a `PATH` holding no `claude`, the run writes the folder, prints both commands, and exits 1.

### 3. Verify — medium — blocked by: 2
`cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test -q` _(the inner loop; the full gate runs at this slice's checkpoint)_

**Acceptance:** inner loop green, one commit.

### 4. Prove — at the checkpoint — blocked by: 3
§10 QA plan row FR-5: run `install_writes_the_marketplace_once_and_rewrites_the_mod` under the sandboxed `HOME`; keep the transcript. Never run the verb against the owner's real `HOME` or data dir.

**Acceptance:** after the checkpoint, the test transcript is in this slice's Progress.

**Code-review fixes (2026-10-30).**
- A `factory` marketplace already added from another path counted as this one, so the install reported success while Claude Code kept loading the old folder, and the stale suffix pointed at a verb that could never fix it. It now stops with the other source named and the remove command (AD-4).
- A `factory-cockpit@factory` installed at project scope counted as installed; only a user-scope install does now, and a disabled one gets the enable command.
- A parse failure of `claude plugin marketplace list` was blamed on `claude plugin list`; the error names the command that printed it.
- The install tests now read `marketplace.json` after the second run too, assert the success and fallback reload lines exactly, and expect the data dir shell-quoted as the binary prints it.
