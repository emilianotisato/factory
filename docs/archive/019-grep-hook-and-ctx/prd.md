---
number: 019
slug: grep-hook-and-ctx
title: "The grep hook and ctx answer what they're asked"
type: feature
created: 2026-11-05
author: Emiliano Tisato
authorized_tests: ["tests/hook.rs", "tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files"]
summary: |
  Makes the PreToolUse hook deny a bare-name grep only when no command follows (`grep`/`rg` or leading `cd && grep`/`rg`) over searchable repo code with no file-filter flags and at least one ctx hit; fixes `guard` false denials on `push` method calls and non-backtick-exec interpreters while widening `process_call` for Ruby/Perl/Lua/Bun/Deno/osascript/tclsh/PHP/Python; caps ctx printed text width (200 chars), `def` locations (20), and hook denial reasons (8 KB); indexes Go `const_spec`, PHP class constants (`const_element`), and Pest `test()`/`it()` in outline; and adds a hook regression corpus fixture plus `/prd` and `/slice` guard-loosening rules (bumping crate/bundle version to 0.17.0). Verified by a cross-vendor adversarial pass (`claude:opus`, 416 s) 2026-11-26 with all 15 findings corrected inline.
---
> **History.** The code, `docs/reference` and `docs/decisions.md` win over this document.

# PRD-019 — The grep hook and ctx answer what they're asked

> **Interviewed 2026-11-26** (upgraded from the 2026-11-05 triage draft, absorbing open ideas `2026-11-10-the-bare-name-grep-hook-refuses-a-whole-batched-co.md` and `2026-11-23-the-worker-bash-guard-matches-push-inside-a-comman.md`). Every `file:line` below was verified against `main` at `5d34d96`, then **re-verified by a cross-vendor adversarial pass (`claude:opus`, 416 s) on 2026-11-26 which caught three run-stopping blockers and twelve real breaks**. Each correction is inline and dated.

## 1. Why now

Four concrete pains have shown up in live runs:

- **The grep hook denies searches `ctx` cannot answer and aborts batched commands.** Today `bash()` (`src/cli/hook.rs:111-152`) ignores the command terminator `end`, so piped greps (`grep -rn Foo src | grep -v _test`) and chained batches (`cd repo && grep -rn Foo app/ && sed -n 1,20p bar.php` — reported in 3 of 7 feature retros in one week) are refused whole. `grep_search` (`:715-776`) ignores `--include`/`--exclude`/`--exclude-dir` and `rg_search` (`:809-872`) ignores `-g`/`--glob`/`-t`/`--type`/`-T`/`--type-not`. `bare_name` (`:670-683`) checks only `VENDOR_DIRS`, while `ctx::walk::walk` (`src/ctx/walk.rs:8-59`) also skips `ROOT_DIRS` (`dist`, `build`, `storage`) and non-code directories like `docs/`. And when `ctx` finds 0 refs and 0 defs (e.g., an XML/string literal like `"DirRecep"`), `answered()` (`:658-660`) still denies with only the hint, wasting a turn.
- **The worker Bash `guard` false-denies safe scripts containing `push`, while missing several process-call spellings.** In `src/cli/hook.rs:591-616`, `CALLS` includes backtick `` ` ``, `%x`, and `qx` unconditionally for *every* interpreter (so any Python edit script with markdown backticks or any Node/Bun/Deno script with a template literal `` `...` `` plus `out.push(` hits `process_call && mentions` and is denied as `git push`, as hit in PRD-024). Meanwhile `mentions()` (`:194-205`) treats `.push` / `->push` / `::push` method calls as `"push"`, and `process_call()` misses `Deno.Command`, `Bun.spawn`, `io.popen`, `__import__`, `do shell script`, dot-prefixed `.system(`/`.exec(`/`.spawn(`/`.popen(`, and paren-less `system `/`exec `/`spawn ` in Ruby/Perl/tclsh.
- **`ctx` output and hook denial reasons have no width or `def`-count cap.** `src/ctx/print.rs:1` prints full lines without truncation and every `def` hit without a cap (`refs` caps at 10, `src/ctx/refs.rs:18`). A minified `.js` line or a widely defined symbol can dump hundreds of KB into a worker's context or hook denial reason.
- **`ctx` misses common Go and PHP symbols, and guard-loosening FRs lack a replay corpus.** `src/ctx/lang.rs:22-42` does not tag Go `const_spec` (single or grouped `const ( )`), PHP class constants (`const_element`), or Pest `test()`/`it()` calls in PHP outlines; `DefArgs.kind` (`src/ctx/mod.rs:36`) does not accept `"constant"`. And `tests/hook.rs` keeps denied commands scattered in inline arrays rather than a replayable regression corpus paired with a `/prd` + `/slice` rule.

**What this leaves alone.** Extensionless script shebang sniffing in `walk.rs` (cut 2026-11-26), per-question-type `ctx` metrics in `factory report`, Jev/LSP/SCIP lookup layers, and batched `def` pack recon (all cut 2026-11-05).

## 2. Blast radius

- **Existing unit tests in `src/cli/hook.rs` (`:998-1222`) and `tests/cli.rs` (`:100-115`).** *Corrected 2026-11-26, adversarial pass (an earlier draft undercounted this as four unit tests / five assertions and missed `tests/cli.rs`):*
  - Five unit tests in `src/cli/hook.rs` (`recursive_grep_over_the_repo_is_denied_with_the_verb`, `a_denied_grep_carries_what_ctx_finds`, `only_a_bare_name_outside_vendor_code_goes_to_ctx`, `grep_operands_are_directories_only_when_they_are`, `a_leading_cd_moves_where_paths_resolve` — 21 deny assertions total) use the `repo()` fixture (`:998-1022`), which writes `"x\n"` into every file. Under FR-1's 0-hit allow rule, `repo()` must be seeded with real `foo` and `Recon` code in `src/one.php` / `src/Http/` / `app/` so deny-side coverage (`-eRecon`, `--regexp=`, `\bRecon\b`, `cd src && rg foo Http`) stays intact, while the newly-allowed cases (`--include`, `| head`, `storage`, `nothing_here`) move to the allow assertions.
  - `tests/cli.rs:100-115` (`hook_denies_repo_greps_and_whole_reads_of_held_files`) pins the old `grep_hint()` verbatim and expects a deny on an empty `src/`. It is listed in this PRD's `authorized_tests` frontmatter (`tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files`) so the builder can seed `foo` in its fixture's `src/one.php` and update the expected hint without tripping `testguard`.
- **`tests/ctx.rs` golden snapshots (`tests/snapshots/ctx/*.txt`).** `tests/ctx.rs:42-100` asserts 20 byte-for-byte snapshots over `tests/fixture/ctx` (`searched 48 files`). Adding new files inside `tests/fixture/ctx` would shift all 20 snapshots' file counts; new tests for FR-2 and FR-3 MUST use `tempfile::tempdir()` instead of mutating `tests/fixture/ctx`, keeping all 20 existing snapshots untouched (verified: no snapshot line exceeds 200 chars or 20 defs).
- **Skill bundle lock and version bump (`skills/bundle.lock`, `Cargo.toml`, `mods/cockpit/.claude-plugin/plugin.json`).** *Added 2026-11-26, adversarial pass:* `skills/bundle.lock` is pinned at tagged `0.16.0`, so `bundle_lock_matches` (`tests/skills_bundle.rs:261`) fails on any skill edit (`skills/{prd,slice}/SKILL.md`) until `Cargo.toml` (and `Cargo.lock` + `mods/cockpit/.claude-plugin/plugin.json` + `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle`) is bumped to `0.17.0`. FR-5 owns that bump.
- **`assets/lookup.md:7` stays untouched.** *Added 2026-11-26, adversarial pass:* `tests/loop.rs:398` pins `"grep anything else"` in `assets/lookup.md:7`; keep `assets/lookup.md` short and unchanged while `grep_hint()` carries the string-literal quoting tip.
- **`tests/hook.rs` and `authorized_tests`.** Consolidating denied commands into a regression corpus fixture (`tests/fixture/hook/denied.json`) updates `tests/hook.rs`, which is declared in this PRD's `authorized_tests` frontmatter.
- **External contracts.** `factory hook` JSON output schema (`hookSpecificOutput.permissionDecision` / `permissionDecisionReason`) and exit code `0` stay unchanged. `factory ctx def --kind` adds `"constant"` (additive); `factory ctx outline` adds `"test"` entries for PHP Pest files (additive).

## 3. FR-1 — Deny a bare-name grep only when standalone, searchable by `ctx`, and `ctx` has hits

**Today.** `bash()` (`src/cli/hook.rs:111-152`) checks `first_command_in(command)` without checking whether another command or pipe follows, so pipes (`|`, `|&`) and chains (`&&`, `||`, `;`) are denied whole. `grep_search` (`:715-776`) ignores `--include`, `--exclude`, `--exclude-dir`, `--exclude-from`; `rg_search` (`:809-872`) ignores `-g`/`--glob`/`--iglob`/`-t`/`--type`/`-T`/`--type-not`. `bare_name` (`:670-683`) skips only `VENDOR_DIRS`, not repo-root `ROOT_DIRS` (`dist`, `build`, `storage`). `answered()` (`:639-667`) always searches `here` (ignoring a concrete target directory like `docs`) and returns `Verdict::Deny(hint)` even when `r.total == 0 && d.hits.is_empty()`. `grep_hint()` (`:18-23`) does not mention string-literal quoting.

**Decided 2026-11-26.**

- **Standalone grep only (no following command or pipe):** *Corrected 2026-11-26, adversarial pass (an earlier draft checked `end.is_none()`, which misclassified trailing `\n`, `;`, or `&` because `shell::tokens` turns them into operators at `src/shell.rs:59-62, :71-74, :101`):* add a small helper (e.g. `shell::first_command_standalone` or return `has_more: bool` alongside `first_command_in`) that reports whether any pipe (`|`, `|&`), logical operator (`&&`, `||`), or subsequent command word follows the first command after stripping leading `cd`/`pushd`. Only evaluate the bare-name `ctx` redirect when no pipe or following command exists — so `grep -rn Foo src`, `grep -rn Foo src;`, `grep -rn Foo src\n`, and `cd repo && grep -rn Foo src` are still checked, while piped commands (`grep -rn Foo src | grep -v _test`) and batched chains (`cd repo && grep -rn Foo app/ && sed -n 1,20p bar.php`) pass through as `Verdict::Allow`.
- **Path/type-filtered greps pass through:**
  - In `grep_search` (`src/cli/hook.rs:715-776`), return `None` if any of `--include`, `--exclude`, `--exclude-dir`, or `--exclude-from` is passed.
  - In `rg_search` (`src/cli/hook.rs:809-872`), return `None` if any of `-g`, `--glob`, `--iglob`, `-t`, `--type`, `-T`, or `--type-not` is passed.
- **Skip `VENDOR_DIRS` + repo-root `ROOT_DIRS` and scope single-dir searches cleanly:** *Corrected 2026-11-26, adversarial pass (earlier draft left globs, multi-path operands, `ROOT_DIRS` depth, and gitignored dirs ambiguous):*
  - In `bare_name` (`src/cli/hook.rs:670-714`), reject any path under `VENDOR_DIRS` (any path component relative to `repo`) OR directly under repo-root `ROOT_DIRS` (`dist`, `build`, `storage` as the **first** path component relative to `repo`, so `storage` and `storage/logs` pass through while `src/build` is NOT treated as a root dir).
  - For the `ctx` search root in `answered()`:
    - When `s.paths` is empty, contains a glob (`*`/`?`/`[`, e.g. `grep -r foo *`), or names more than one path (e.g. `grep -rn foo src app`), search from `here` (clamped to `repo`, same as today).
    - When `s.paths` names a single concrete existing directory `target` inside `repo` (e.g. `docs`, `src`, `src/Http`), pass `target` as the `root` (and `repo` as `base`) to `refs::find` / `def::find` (using `WalkOpts { vendor: false }`). Note: `walk::walk` skips `ROOT_DIRS` at depth 1 of its walk root (`src/ctx/walk.rs:57`), matching `factory ctx --in <target>`. If `target` contains no `Lang` files (e.g. `docs/` or `.md`/`.toml`-only dirs), `ctx` searches 0 files and returns 0 hits, which the 0-hit rule below allows.
- **Never deny empty-handed:** change `answered()` (`src/cli/hook.rs:639-667`) to return `Option<String>`: when `refs::find` and `def::find` both succeed with `r.total == 0 && d.hits.is_empty()`, OR if `ctx` errors, return `None` so `bash()` returns `Verdict::Allow` (letting grep search string literals like `"DirRecep"`, comments, or non-code files immediately without wasting a turn).
- **Update `grep_hint()` (`src/cli/hook.rs:18-23`):** keep the `ctx refs` / `ctx def` guidance and add string-literal advice: `"use: {factory} ctx refs <word> (or ctx def <symbol>; --vendor for vendor code): a bare name goes to ctx; for string literals grep with quotes (e.g. grep '\"Word\"')"`.

**Accepted cost.** A worker running `grep -rn nonExistentTypo src` or targeting a gitignored directory by name now runs grep directly (or gets `ctx` hits if the ignored dir holds code) instead of getting an empty hint denial. Similarly, a worker chaining `grep -rn Foo src && ls` bypasses the redirect; keeping batches intact saves far more turns than chasing deliberate workarounds (measured: 3 of 7 retros in one week).

**Acceptance.** Unit and integration tests prove: (1) standalone `grep -rn alpha src`, `grep -rn alpha src;`, `grep -rn alpha src\n`, `grep -r alpha *`, `grep -rn alpha src app`, and `cd <repo> && rg alpha src` where `alpha` exists in `src/lib.rs` are **denied** with the updated `grep_hint()` + `ctx refs`/`ctx def --no-body` output; (2) piped (`rg alpha src | head -20`), batched (`cd <repo> && grep -rn alpha src && echo done`), filtered (`grep -rn --include=*.php alpha src`, `rg -t rs alpha src`, `rg -g '*.rs' alpha src`), repo-root `ROOT_DIRS` (`rg alpha storage`, `grep -rn alpha storage/logs`), non-code dir (`grep -rn checkpoint_every docs`), and 0-hit / string-literal-only (`grep -rn DirRecep src` where `DirRecep` appears only inside `"DirRecep"`) all return `Verdict::Allow`; (3) all 5 updated unit tests in `src/cli/hook.rs` and `tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files` stay green.

## 4. FR-2 — Cap `ctx` line width, `def` location count, and hook denial reason size

**Today.** `src/ctx/print.rs:1-189` prints every line at unbounded length and every `Def` hit in `found.hits` without a cap. `src/cli/hook.rs:662-666` embeds `print::refs` and `print::defs` verbatim into `permissionDecisionReason` with no byte limit.

**Decided 2026-11-26.**

- **Text field width cap (`src/ctx/print.rs`):** *Clarified 2026-11-26, adversarial pass:* define `pub const MAX_LINE_CHARS: usize = 200;` and a UTF-8 char-safe `truncate_line(s: &str, max: usize) -> Cow<'_, str>` helper that truncates any printed source text field exceeding 200 characters to 199 characters + `'…'` (preserving multi-byte UTF-8 boundaries). Apply it to the `text` field in `block()` (after `path:line  kind  `), each `line` in `indented()` (after `    `), and each entry's `text` in `outline()` (note: tree-sitter-tags already caps tag `line_range` at 180 bytes at `src/ctx/lang.rs:120`, so `outline()` truncation fires only for non-tags entries like long Pest test lines > 200 chars).
- **`def` location cap (`src/ctx/print.rs`):** define `pub const MAX_DEFS: usize = 20;`. In `print::defs()`, print at most `MAX_DEFS` definitions from `found.hits`. When `found.hits.len() > MAX_DEFS`, emit a line after the 20th definition: `"{more} more definitions: narrow with --kind or --in"` (where `more = found.hits.len() - MAX_DEFS`) while keeping the accurate total count in the trailer (`"{n} definitions (searched ...)"`).
- **Hook denial reason cap (`src/cli/hook.rs`):** define `pub const MAX_DENY_BYTES: usize = 8 * 1024;`. In `answered()`, if the formatted reason exceeds `MAX_DENY_BYTES`, truncate on a valid UTF-8 line/char boundary to fit within `MAX_DENY_BYTES` including a trailing `"\n… (truncated; run ctx with --in or --max to narrow)"` suffix.

**Accepted cost.** A legitimately long signature or minified JS line whose text exceeds 200 characters is truncated with `…` in `ctx` output (the worker can `Read` that exact line with `offset`/`limit` if needed). Capping at 200 chars / 20 defs / 8 KB prevents megabyte context spikes from minified assets or common names like `new`/`handle`.

**Acceptance.** Unit and integration tests prove: (1) a 5,000-character minified `.js` line referencing or defining a symbol prints a text field (after `path:line  kind  `) of at most 200 characters ending in `…` in `ctx refs` and `ctx def` (plus a Pest test line > 200 chars in `ctx outline`, and a direct unit test on `truncate_line` with multi-byte UTF-8 characters); (2) a symbol defined 35 times prints exactly 20 definitions followed by `"15 more definitions: narrow with --kind or --in"` and the trailer `"35 definitions"`; (3) a hook denial reason whose combined `refs` + `defs` exceeds 8 KB is capped at `<= 8192` bytes and ends with the truncation notice; (4) all 20 existing `tests/ctx.rs` snapshots remain byte-for-byte identical.

## 5. FR-3 — Index Go constants, PHP class constants, and Pest `test()`/`it()` calls

**Today.** `src/ctx/lang.rs:22-42` tags PHP methods/enums and Go structs/interfaces/type aliases, but not Go `const_spec`, PHP class constants (`const_element`), or Pest `test()`/`it()` calls. `src/ctx/def.rs:43-107` lacks PHP/Go `const` regex patterns and `vocabulary` maps `"const"` to `"constant"` only for Rust (`:321`). `src/ctx/mod.rs:36` restricts `--kind` to `["class", "fn", "method", "type"]`. `src/ctx/outline.rs:154-164` outlines only `class`, `impl`, `module`, `fn`, `method`, `type`.

**Decided 2026-11-26.**

- **Go constants (`src/ctx/lang.rs`, `src/ctx/def.rs`):**
  - Add `(const_spec name: (identifier) @name) @definition.constant` to `GO_EXTRA` (`src/ctx/lang.rs:39-42`), covering both single `const Foo = 1` and grouped `const (\n Foo = iota\n Bar\n)`.
  - *Corrected 2026-11-26, adversarial pass (an earlier draft proposed overly broad `r"NAME\b"`, which would match struct-literal keys `Name: x,` and calls `Name()` and inflate `not_defs`):* in `Table::Go::patterns` (`src/ctx/def.rs:55-59`), add `r"const\s+NAME\b"` (single-line Go const) and `r"NAME\s*(?://.*)?$"` (bare grouped `iota` identifier alone on its line with optional trailing comment — keeping existing `r"NAME(\[[^\]]*\])?\s+(=\s*)?[\w.*\[({]"` for assigned/typed grouped consts). Add `"const"` to `Table::Go::keywords` and map `"const"` → `"constant"` in `vocabulary` for `Table::Go`.
- **PHP class constants (`src/ctx/lang.rs`, `src/ctx/def.rs`):**
  - Add `(const_declaration (const_element (name) @name)) @definition.constant` to `PHP_EXTRA` (`src/ctx/lang.rs:22-25`) — verify against `tree-sitter-php`'s exact grammar node names at build time (covering both class `const` and top-level `const`).
  - In `src/ctx/def.rs:45-48`, add `r"((public|protected|private|final)\s+)*const\s+([A-Za-z_\\|?]+\s+)?NAME\b"` to `Table::Php::patterns`, add `"const"` to `Table::Php::keywords`, and map `"const"` → `"constant"` in `vocabulary` for `Table::Php`.
- **`--kind constant` (`src/ctx/mod.rs:36`):** add `"constant"` to `DefArgs.kind`'s `value_parser` (`["class", "fn", "method", "type", "constant"]`) so `factory ctx def NAME --kind constant` works across Rust, Go, and PHP.
- **Pest `test()` / `it()` in PHP outline (`src/ctx/lang.rs`, `src/ctx/outline.rs`):**
  - *Corrected 2026-11-26, adversarial pass (an earlier draft put a custom `@_fn` capture inside `PHP_EXTRA`, which panics `tree-sitter-tags` with `InvalidCapture` because `tree-sitter-tags` allows only `name`, `doc`, `ignore`, `local.*`, `definition.*`, `reference.*`):* extract Pest `test('...')` and `it('...')` entries directly in Rust using the parsed tree (`Unit::parse()` when `self.grammar == Grammar::Php`) inside `Unit::definitions()` (merging with tags definitions and sorting by `(range.start, Reverse(range.end), name.start)`). Walk AST nodes outside `class_declaration`, `interface_declaration`, `trait_declaration`, `enum_declaration`, `function_definition`, and `method_declaration` bodies (so top-level calls and calls inside `describe('...', fn () => ...)` closures are included while app class methods are skipped): match `function_call_expression` whose `function` child is `(name)` equal to `"test"` or `"it"` and whose first argument is a `string` or `encapsed_string`, emitting a `Definition` with `kind: "test"`, `name` set to the string argument range, and `line` set to the trimmed call line (full line range, allowing FR-2's `truncate_line` to cap >200-char Pest titles cleanly).
  - In `src/ctx/outline.rs:154-164`, add `"test"` to `outlined()` returning `Some(false)` (leaf entry, burying closure locals). Constants remain excluded from `outline` (unchanged from `src/ctx/outline.rs:1`).

**Accepted cost.** In Go files with syntax errors that fall back to `textual()`, a bare grouped `iota` identifier without `const` on its line still falls back to `Table::Go`'s default kind (`type`), flagged by `not parsed cleanly: textual matches kept`. Pest tests written via custom helper wrappers (not `test()`/`it()`) are not outlined.

**Acceptance.** Tempdir tests prove: (1) `ctx def` finds single Go `const Single = 1`, grouped Go `const (\n First = iota\n Second\n)`, and PHP `public const STATUS_OPEN = 'open';` (including PHP 8.3 typed `public const string MODE = 'a';`) with kind `constant`, and `--kind constant` filters to them without matching struct-literal keys `First: 1,`; (2) `ctx outline` on a Pest `.php` file lists top-level `test('...')` and `it('...')` calls (and those inside `describe(...)`) with kind `test`, ignores calls inside class methods, and hides closure-local functions; (3) all 20 existing `tests/ctx.rs` snapshots remain byte-for-byte unchanged.

## 6. FR-4 — Fix `guard` false denials on `push` method calls / non-backtick interpreters, and widen `process_call`

**Today.** `src/cli/hook.rs:591-616` matches ``"`"``, `"%x"`, and `"qx"` for *all* interpreters — so Python scripts with markdown backticks or Node/Bun/Deno scripts with JS template literals (`` `...` ``) trigger `process_call == true`, denying safe scripts containing `push` (e.g. `out.push(` or an AD body). `mentions()` (`:194-205`) matches `"push"` even right after `.`, `->`, or `::` (`out.push(`, `$c->push(`). Meanwhile `process_call()` skips dot-prefixed `.system(`/`.exec(`/`.spawn(`/`.popen(` (`:612-615`), missing `python3 -c '__import__("os").system("gh pr list")'`, `bun -e 'Bun.spawn(["git","push"])'`, `lua -e 'io.popen("git push")'`, `ruby -e 'Kernel.system("gh")'`, `deno eval 'new Deno.Command("gh",...).outputSync()'`, `osascript -e 'do shell script "git push"'`, and paren-less `ruby -e 'system "gh pr list"'` / `perl -e 'exec "git", "push"'` / `tclsh` `exec`.

**Decided 2026-11-26.**

- **Scope interpreter-specific syntax in `process_call(program: &str, text: &str)` (`src/cli/hook.rs:511-616`):**
  - Thread `program` from `interpreter()` → `inline_script()` → `script()` → `process_call()`.
  - Keep ``"`"`` as a process-call trigger ONLY for `ruby`, `perl`, and `php` (NOT Python, Node/Bun/Deno where backticks are template strings or plain text, Lua, Rscript, osascript, tclsh).
  - Keep `"%x"` for `ruby` only and `"qx"` for `perl` only.
- **Ignore method-call `.push` / `->push` / `::push` in `mentions()` (`src/cli/hook.rs:194-205`):**
  - When matching `"push"` in `mentions()`, ignore occurrences immediately preceded by `.`, `->`, or `::` (so `out.push(line)`, `arr.push(1)`, `$c->push(1)`, and `Vec::push` do not trigger `"push"`, while `git push`, `git-push`, `"push"`, `'push'`, and standalone `push` still match).
- **Widen `process_call` for missed process-spawning spellings (`src/cli/hook.rs:591-616`):**
  - *Corrected 2026-11-26, adversarial pass (switching bare call matching to non-word-preceded would have dropped `pcntl_exec(` and `posix_spawn(`/`posix_spawnp(` which main currently matches as substrings):* add to `CALLS`: `"__import__"`, `"Deno.Command"`, `"Bun.spawn"`, `"io.popen"`, `"do shell script"`, `"pcntl_exec"`, `"posix_spawn"`, `"posix_spawnp"`.
  - Remove the `!text[..i].ends_with('.')` skip for `"system("`, `"exec("`, `"spawn("`, `"popen("` (requiring only that the byte before the call name is not ASCII alphanumeric or `_`, so `my_exec(` stays quiet while `system(`, `os.system(`, `Kernel.system(`, `__import__("os").system(`, and `io.popen(` all match).
  - For `ruby`, `perl`, `tclsh`, and `wish`, also match paren-less word-start calls `"system "`, `"exec "`, `"spawn "` (and tab after keyword, e.g. `system\t`) when preceded by start-of-string or a non-word character (catching `ruby -e 'system "gh pr list"'`, `perl -e 'exec "git", "push"'`, and `tclsh` `exec git push`).

**Accepted cost.** *Updated 2026-11-26, adversarial pass:* (1) A JS/Python script that calls a method literally named `.exec(` (e.g. `regex.exec(s)`) AND also mentions standalone `gh` or `push` (not `.push`) in the same inline script would be denied. (2) Ignoring `.push` / `->push` / `::push` in `mentions()` means a script invoking a Git library's method call (such as GitPython `repo.git.push()`, simple-git `git.push()`, or PHP `$repo->push()`) alongside an unrelated process call (like `os.system("true")`) is no longer caught by the Bash hook; that narrow gap is accepted because PRD-027's `run_dir/bin/git` PATH shim, stripped GitHub tokens, and empty `GH_CONFIG_DIR` remain the hard backstop (`docs/decisions.md#who-talks-to-github`). (3) Versioned interpreter names like `php8.3` or `ruby3.3` remain outside `interpreter()` (pre-existing behavior, unchanged here). Everything the old guard denied that this FR does not mean to allow stays denied, proven by FR-5's regression corpus.

**Acceptance.** Unit and integration tests (plus FR-5's corpus) prove: (1) `python3 - <<'PY'` with markdown backticks and `out.push(line)`, `node -e "const a = []; a.push(1); console.log(\`${a}\`)"`, and a quoted heredoc writing an AD entry with backticks and `push` are all **allowed**; (2) all eleven newly verified bypass spellings (`ruby -e 'system "gh pr list"'`, `perl -e 'exec "git", "push"'`, `python3 -c '__import__("os").system("gh pr list")'`, `bun -e 'Bun.spawn(["git","push"])'`, `lua -e 'io.popen("git push")'`, `ruby -e 'Kernel.system("gh")'`, `deno eval 'new Deno.Command("gh",{args:["pr","view"]}).outputSync()'`, `osascript -e 'do shell script "git push"'`, `tclsh <<'EOF'\nexec git push\nEOF`, `php -r 'pcntl_exec("/usr/bin/gh",["pr","list"]);'`, `python3 -c 'import os; os.posix_spawn("/usr/bin/gh", ["gh", "pr", "list"], {})'`) are **denied**; (3) every entry in `tests/fixture/hook/denied.json` stays denied.

## 7. FR-5 — Hook regression corpus fixture, `/prd` + `/slice` guard-loosening rule, and `0.17.0` bundle bump

**Today.** `tests/hook.rs:45-204` holds denied commands inline across six separate test functions (`the_review_cases_are_denied`, `the_second_pass_cases_are_denied`, `every_segment_and_every_wrapper_is_read`, `doubt_is_denied_when_it_names_gh_or_push`, `a_script_is_denied_only_when_it_runs_a_process_and_names_gh_or_push`, `a_script_that_gets_gh_or_push_from_around_it_is_denied` — *count corrected 2026-11-26, adversarial pass*). Neither `skills/prd/SKILL.md` nor `skills/slice/SKILL.md` requires a guard-loosening FR to replay a corpus of everything the old guard denied. `skills/bundle.lock` is pinned at released `0.16.0` (`tests/skills_bundle.rs:261`, `src/skills.rs:208-236`).

**Decided 2026-11-26.**

- **Regression corpus fixture (`tests/fixture/hook/denied.json` + `tests/hook.rs`):**
  - Store every command `main`'s `guard` denies (all 106 existing denied commands from `tests/hook.rs:45-204` plus the 11 new FR-4 commands) in `tests/fixture/hook/denied.json` with two arrays: `"denied"` (commands that MUST be denied) and `"allowed_on_purpose"` (commands explicitly moved out of `"denied"` or proven safe alongside `"denied"`, each with `{ "cmd": "...", "why": "..." }`, including the FR-4 backtick/`.push` cases).
  - Add a single corpus replay test `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` in `tests/hook.rs` that reads `tests/fixture/hook/denied.json`, asserts every `"denied"` command returns `guard(cmd) == true` and `Verdict::Deny(NEVER_HINT)`, asserts every `"allowed_on_purpose"` command returns `guard(cmd) == false` and `Verdict::Allow`, and asserts the two lists are disjoint and non-empty.
- **Skill updates (`skills/prd/SKILL.md`, `skills/slice/SKILL.md`) and `0.17.0` version/lock bump:** *Updated 2026-11-26, adversarial pass:*
  - In `skills/prd/SKILL.md` (§2 Acceptance bullet): add one sentence: `"Any FR that loosens a guard (a hook, a deny rule, a validator) carries the acceptance line: everything the old guard denied that this FR does not mean to allow stays denied, proven by replaying a regression corpus."`
  - In `skills/slice/SKILL.md` (§4 Acceptance bullet): add one sentence: `"When a PRD FR loosens a guard, carry its regression-corpus replay into the Prove task."`
  - Bump crate/plugin version from `0.16.0` to `0.17.0` in `Cargo.toml`, `Cargo.lock`, and `mods/cockpit/.claude-plugin/plugin.json`, and regenerate `skills/bundle.lock` via `FACTORY_BUNDLE_WRITE=1 cargo test --test skills_bundle` so `bundle_lock_matches` passes cleanly without needing network access to `origin`.

**Accepted cost.** Adding a new intentional allowance to `guard` in a future PRD requires moving any affected command from `"denied"` to `"allowed_on_purpose"` with a `"why"` note in the same diff — deliberate friction that prevents silent bypasses like the 10 found after PRD-009 FR-3. Bumping to `0.17.0` inside this branch means the next release tag after PRD-019 (and PRD-020) will be `v0.17.0`.

**Acceptance.** (1) `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` replays every command in `tests/fixture/hook/denied.json` (≥ 115 denied commands covering all 106 prior `tests/hook.rs` cases + all 11 FR-4 spellings) and fails if any `"denied"` entry is allowed or any `"allowed_on_purpose"` entry is denied; (2) `skills/prd/SKILL.md` contains its exact guard-loosening sentence once, and `skills/slice/SKILL.md` contains its exact Prove-carry sentence once (verified in `tests/skills_bundle.rs` via `assert_unique_passage`); (3) `bundle_lock_matches` and `claude plugin validate mods/cockpit && claude plugin test mods/cockpit` stay green at `0.17.0`.

## 8. Decisions log

No `OPEN-n`. Every fork was settled on 2026-11-26 and lives in its FR:

- Allow non-standalone (piped/chained), filtered (`--include`/`--exclude`/`-g`/`-t`/`-T`), `VENDOR_DIRS` / repo-root `ROOT_DIRS`, non-code dir, AND 0-hit bare-name greps; keep trailing `\n`/`;`/`&` standalone; update `grep_hint()` with string-literal advice (§3).
- Fold open ideas `2026-11-10` (batched `&&` greps) and `2026-11-23` (`push` inside script/heredoc bodies) into FR-1 and FR-4 (§3, §6).
- Cap `ctx` text fields at 200 chars, `def` locations at 20, and hook denial reasons at 8 KB (§4).
- Index Go `const_spec`, PHP class constants (`const_element`), `--kind constant`, and Pest `test()`/`it()` in outline (via Rust AST walk rather than a custom `tree-sitter-tags` capture); cut extensionless shebang sniffing (§5, §11).
- Scope backtick/`%x`/`qx` by interpreter, ignore method-call `.push`/`->push`/`::push` in `mentions()`, and widen `process_call` (keeping `pcntl_exec`, `posix_spawn`, `posix_spawnp`) (§6).
- Add `tests/fixture/hook/denied.json` regression corpus, `/prd` + `/slice` guard-loosening rules, and bump version + `skills/bundle.lock` to `0.17.0` (§7).

## 9. QA plan

**Gate**: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test && claude plugin validate mods/cockpit && claude plugin test mods/cockpit` *(declared in `docs/project.toml`)*
**Surface**: `factory hook` (`PreToolUse` JSON stdin/stdout), `factory ctx def|refs|outline`, and `skills/{prd,slice}/SKILL.md`. No GUI: every FR is proven below it.
**Harness**: `src/cli/hook.rs`, `tests/hook.rs`, `tests/cli.rs`, `tests/fixture/hook/denied.json`, `src/ctx/{print,lang,def,outline,mod}.rs`, `tests/ctx.rs`, `tests/skills_bundle.rs`.
**Seed state**: tempdir fixtures with Rust/Go/PHP/Pest/minified-JS files, `docs/` + `storage/` dirs, and `tests/fixture/hook/denied.json`.

| FR | Proof | Evidence | Human? |
|---|---|---|---|
| FR-1 | run `grep_hook_denies_only_standalone_unfiltered_code_searches_with_ctx_hits` | test transcript | no |
| FR-2 | run `ctx_output_caps_line_width_def_count_and_hook_denial_size` | test transcript | no |
| FR-3 | run `ctx_indexes_go_consts_php_class_consts_and_pest_outline_tests` | test transcript | no |
| FR-4 | run `guard_allows_push_method_calls_and_denies_widened_process_calls` | test transcript | no |
| FR-5 | run `hook_guard_replays_the_denied_corpus_and_allowed_exceptions` | test transcript | no |

No row needs a human, so no `qa-guide.md` is generated.

## 10. Slices

Three vertical slices on one feature branch (`feat/grep-hook-and-ctx`), one PR, one commit per slice, Prove rows driven at checkpoints (`checkpoint_every = 2`). No QA row needs a human, so no `qa-guide.md` is generated.

- **Slice 01 — `ctx` output caps and Go, PHP and Pest indexing** (`FR-2` ctx print caps, `FR-3`, `deps: —`): adds UTF-8-safe 200-char text truncation and 20-def cap in `src/ctx/print.rs`, indexes Go `const_spec` and PHP class constants (`const_element`) with `--kind constant`, and outlines PHP Pest `test()`/`it()` via a Rust AST walk.
- **Slice 02 — Standalone filtered grep hook and denial cap** (`FR-1`, `FR-2` hook cap, `deps: 01`): restricts grep denials to standalone unfiltered code searches with `ctx` hits, adds string-literal advice to `grep_hint()`, caps hook denial reasons at 8 KB, updates the 5 `hook.rs` unit tests and `tests/cli.rs::hook_denies_repo_greps_and_whole_reads_of_held_files`, and proves FR-1 and FR-2 end-to-end.
- **Slice 03 — Bash guard fixes, regression corpus and skill rules** (`FR-4`, `FR-5`, `deps: 02`): scopes backtick/`%x`/`qx` by interpreter, ignores `.push`/`->push`/`::push` in `mentions()`, widens `process_call` (including `pcntl_exec`, `posix_spawn`, `posix_spawnp`), adds `tests/fixture/hook/denied.json` corpus replay, adds guard-loosening rules to `skills/{prd,slice}/SKILL.md`, and bumps version + `skills/bundle.lock` to `0.17.0`.

Checkpoint 1 runs after slices 01 and 02 (proving FR-1, FR-2, FR-3); the final checkpoint runs after slice 03 (proving FR-4, FR-5, and the full gate including `claude plugin validate/test`).

## 11. Non-goals

- **Extensionless scripts via shebang sniffing in `src/ctx/walk.rs`.** Cut 2026-11-26: reading the first line of every extensionless file during `walk` adds filesystem I/O for a rare case (`bin/artisan`). Revisit if extensionless PHP/Node scripts become a recurring lookup miss.
- **Updating `assets/lookup.md:7`.** Not handled (2026-11-26): `tests/loop.rs:398` pins `"grep anything else"` and `assets/lookup.md` stays short while `grep_hint()` carries the string-literal quoting tip.
- **Versioned interpreter binaries (`php8.3`, `ruby3.3`) in `hook::interpreter`.** Not handled (2026-11-26): pre-existing scope of `interpreter()` (`src/cli/hook.rs:511-527`), backstopped by PRD-027's PATH shims.
- **Per-question-type `ctx` metrics in `factory report`.** Dropped at 2026-11-05 triage (items 1–4 of the ctx-hook idea shipped in PRD-009).
- **Jev picker, LSP/SCIP lookup layer, and batched `def` pack recon.** Dropped at 2026-11-05 triage (see `docs/reference/ctx-research.md` and `docs/reference/ctx-jev-research.md`).

## 12. Verification notes

Checked against `main` at `5d34d96`, 2026-11-26, then re-verified the same day by a cross-vendor adversarial pass (`factory review second --doc`, `claude:opus` at `high` effort, 416 s, log `~/.local/share/factory/reviews/20261126-151336`). All 15 findings (3 run-stopping blockers + 12 technical/count breaks) were verified and corrected inline above. **Re-verify at slice entry.**

- `src/cli/hook.rs:18-23` (`grep_hint`), `:111-152` (`bash` ignores following-command check for `grep`/`rg`), `:168-205` (`forbidden`, `mentions` matches `.push`), `:511-616` (`interpreter`, `script`, `process_call` applies ``"`"``, `"%x"`, `"qx"` to all interpreters and skips `.system(`/`.exec(`), `:639-683` (`answered` returns hint-only on 0 hits, `bare_name` checks only `VENDOR_DIRS`), `:715-872` (`grep_search` ignores `--include`/`--exclude`/`--exclude-dir`/`--exclude-from`, `rg_search` ignores `-g`/`--glob`/`--iglob`/`-t`/`--type`/`-T`/`--type-not`), `:998-1222` (5 unit tests / 21 deny assertions using `"x\n"` in `repo()`).
- `src/shell.rs:59-62, :71-74, :101` (trailing `\n`, `;`, `&` emit `Tok::Op`), `:156-192` (`first_command_in` strips leading `cd`/`pushd`).
- `src/ctx/print.rs:1-189` (`defs`, `refs`, `outline`, `block`, `indented` — no text width cap, no `def` location cap).
- `src/ctx/lang.rs:22-52` (`PHP_EXTRA`, `TS_EXTRA`, `GO_EXTRA`, `RUST_EXTRA`), `:92-93` (`TagsConfiguration::new().expect()`), `:120` (180-byte `line_range` cap), `:232-238` (`kind`).
- `src/ctx/def.rs:43-107` (`Table::patterns`, `Table::keywords`), `:318-327` (`vocabulary` maps `"const"` → `"constant"` only for `Table::Rust`).
- `src/ctx/outline.rs:154-164` (`outlined` matches `class|impl|module|fn|method|type`).
- `src/ctx/mod.rs:36` (`DefArgs.kind` `value_parser = ["class", "fn", "method", "type"]`).
- `src/ctx/walk.rs:8-59` (`VENDOR_DIRS`, `ROOT_DIRS`, `Lang::from_path`, `walk` applies `ROOT_DIRS` at depth 1 of walk root).
- `tests/ctx.rs:42-100` (20 golden snapshots over `tests/fixture/ctx` asserting `searched 48 files`).
- `tests/cli.rs:100-115` (`hook_denies_repo_greps_and_whole_reads_of_held_files` pins old `grep_hint()` and empty `src/`).
- `tests/hook.rs:45-241` (6 denied test functions / 106 denied commands).
- `tests/skills_bundle.rs:261`, `src/skills.rs:208-236`, `Cargo.toml:3`, `mods/cockpit/.claude-plugin/plugin.json:3`, `skills/bundle.lock:2` (pinned at tagged `0.16.0`).
- `assets/lookup.md:7`, `tests/loop.rs:398` (pins `"grep anything else"`).
