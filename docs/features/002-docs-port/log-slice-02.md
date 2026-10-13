## Decisions

### AD-201 — What does `log add` refuse beyond the slice's list, and what do `show`'s filters do to `--json`? · slice-02 · 2026-10-13
**Chose:** `log add` also refuses `--ref` on a kind other than `answer` or `review`, mirroring the `--flag` and `--verdict` rules. `log show --json` with `--decide` or `--open` prints the entries that filter keeps; without a filter it prints every entry in file order. `--decide` and `--open` together are a usage error. A heading whose entry has both a slice and a phase prints both middle fields (`· slice-02 · checkpoint ·`).
**Over:** storing a stray `ref` silently; `--json` ignoring the filters; printing only one of slice and phase.
**Because:** prd §4 defines `ref` as "the id an `answer` or `review` points at" but lists no refusal for it on other kinds, and is silent on filters with `--json` and on entries carrying both fields. Each choice keeps information or keeps one meaning per flag.
**Undo:** drop the `!refers && reference.is_some()` arm in `log::validate`; make `--json` call `entries` unfiltered; print only `slice-NN` in `log::place`.
**Review:** no

## Traps

- Another lane's builder edits `src/cli/docs.rs` in the same working tree while you build, so `cargo test` can fail to compile on its half-written verb. Wait for the tree to compile again (a background `until cargo build -q` loop) instead of touching its code.

## Untracked leftovers

- tests/snapshots/docs-log/show.md
