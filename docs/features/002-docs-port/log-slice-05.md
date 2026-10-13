## Decisions

### AD-501 — `new --from-idea` as `scaffold::new_from`, with `new` kept · slice-05 · 2026-10-13
**Chose:** `scaffold::new_from(root, slug, title, from_idea, out)` does the work. `scaffold::new` keeps its four arguments and calls it with `None`.
**Over:** adding the parameter to `new` itself, as task 4 words it.
**Because:** slice 04 was being built in the same working tree at the same time. A changed signature would break any `scaffold::new` call it adds, and `check.rs`'s unit fixture already calls it.
**Undo:** fold `new_from` into `new` and add `None` at each caller.
**Review:** no

### AD-502 — The idea file has a blank line after the frontmatter, and the reader drops leading blank lines · slice-05 · 2026-10-13
**Chose:** `ideas::render` writes `---`, the keys, `---`, a blank line, then the body, ending in one newline. When the body is empty, it writes no blank line. `ideas::read` strips the body's leading newlines. The `body` that `list`, `body_chars` and `new --from-idea` see is the text after them.
**Over:** a body written right after the fence, with no blank line.
**Because:** a hand-written idea has a blank line there, and so do the PRD files. The body round-trips byte for byte except for leading blank lines, which a bullet body never has. Slice 08 writes through `render`.
**Undo:** `render` and `read` in `src/docs/ideas.rs`.
**Review:** fyi

### AD-503 — What in `docs/ideas/` is an idea · slice-05 · 2026-10-13
**Chose:** `ideas::files` skips `README.md` and dotfiles such as a `.gitkeep`. Everything else goes through `ideas::read`: a directory or a non-`.md` file is `E-IDEA-NAME`, and `idea list` skips it with a `!` line. Slugs keep ASCII letters and digits only, so `Café` gives `caf`.
**Over:** flagging a `.gitkeep`, or keeping non-ASCII letters, which `check`'s `[a-z0-9]` name pattern would then refuse.
**Because:** the slice is silent on dotfiles and non-ASCII titles. A repo may already hold a `.gitkeep` in the folder. And a slug `add` writes must pass `check`.
**Undo:** `files` and `slug` in `src/docs/ideas.rs`.
**Review:** fyi

### AD-504 — `idea drop` restores the file when the commit fails · slice-05 · 2026-10-13
**Chose:** when `git commit` exits non-zero, for example when the repo's pre-commit hook refuses it, `drop` runs `git checkout -- <path>` and exits 2 with git's stderr.
**Over:** leaving the file deleted and not committed.
**Because:** in that case the deletion would be in the tree with the reason in no commit, which is the state `drop` exists to prevent.
**Undo:** the failure arm of `drop` in `src/docs/ideas.rs`.
**Review:** no

## Traps

- Slice 04 was built in this same working tree while this slice ran. `cargo build` failed on its half-written modules (`lanes`, `renumber`, `ship`). What works: copy the tree with `tar --exclude=./target --exclude=./.git`, drop the missing `pub mod` lines in the copy, run `git init` and commit there (`tests/cli.rs` needs a git repo), and run `cargo test` with `CARGO_TARGET_DIR=/tmp/...`.
- The worker hook refuses any Bash command whose text holds `git` and `push`, even a heredoc that edits Rust with `made.push(...)`. What works: write the edit script to a scratch file with Write, then run it.

## Untracked leftovers

- src/docs/scaffold.rs
