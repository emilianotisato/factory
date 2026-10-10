# `factory ctx` research (OPEN-16)

Decision memo from the research workflow run on 2026-10-09 (27 agents: transcript mining of 55 owner sessions, four surveys, a verify pass that refuted 7 claims). Kept here so a later PRD on a typed lookup layer does not redo the recon. Figures marked *unverified* did not go through the verify pass. The full result with per-claim evidence lives in the session workflow transcript `wf_60994131-195`.

---


2026-10-09. Closes PRD-001 OPEN-16. Inputs: 55 of the owner's sessions mined read-only, Claude Code 2.1.280 docs, four surveys with fact-checked claims. Refuted claims are listed at the end and are not used above it.

## 1. Headline

Ship a thin `factory ctx` in PRD-001 as FR-12. Verbs: `outline`, `def`, `refs`. Engine: the grep crates plus tree-sitter tags. No LSP in PRD-001. The typed layer gets its own PRD, opened only if FR-8 data asks for it.

Why, in four facts. Lookups are 92% of worker tool-result bytes. File reads are 77% of those bytes; symbol searches are 8%. The only confirmed cost-and-accuracy win in prior art is skeleton output. LSP has confirmed negative token evidence on Opus and Sonnet.

## 2. What workers actually look up

Source: icg-cloud and arca-api transcripts, 55 main sessions and 563 subagent transcripts. Claude Code 2.1.259 to 2.1.280, all runs in bypassPermissions.

| Measure | Value |
|---|---|
| Tool calls | 40,748 |
| Lookups, literal rule | 30,267 (74.3%) |
| Lookups, strict (first command) | 22,846 (56.1%) |
| Build/verify workers, strict | 59.5% |
| Grep / Glob / LSP tool calls | 0 / 0 / 0 |
| Tool-result chars, all | 148.6M (~37M tokens) |
| Lookup result chars | 136.8M (92.1%) |
| File reads, share of lookup chars | 77% |
| grep output, share of lookup chars | 17% |
| Results of 10k+ chars | 12.2% of lookups, 53.4% of bytes |

Every lookup is Bash `grep`, `sed -n`, `cat`, `ls`, or Read. The bypass-mode system instruction tells the model to prefer those over Grep, Glob and Read. `rg`, ctags and LSP appear zero times. The PRD's 44% figure does not match; its method is unknown. FR-11 should re-baseline on 56% strict.

Question mix. Sample of 100 lookups, 75 real after dropping piped test output:

| Question | Share of real lookups | Heuristic population | Bytes |
|---|---|---|---|
| Read a file or range | 57% | 12,927 | 97.8M |
| Literal string | 15% | 4,233 | 11.2M |
| Definition | 11% | 1,731 | 4.4M |
| Callers / usages | 7% | 1,124 | 2.3M |
| Outline of a file | 5% | 657 | 1.4M |
| File by name | 5% | 1,433 | 4.5M |

Symbol questions (def, refs, outline) are 23% of real lookups by count and about 8% by bytes. Reads are the cost. 80% of strict Bash lookups bundle two or more commands, mean 3.5. The common bundle is "grep the def, sed 30 lines, grep callers".

What a pack or a cache could pre-pay:

| Repeat measure | Value |
|---|---|
| Read bytes that are project docs (prd, slice, SKILL.md) | 32.8M chars, 29% |
| Reads of a file already read, same transcript | 16.2% of Reads |
| Reads of a file already read, session tree | 39.7% of Reads |
| Lookups re-hitting an earlier target, same transcript | 8,082 (26.7%), 36.5M chars |
| Target repeats across the session tree | 37.6% |
| Exact command repeated | 0.9% transcript, 3.7% tree |

Reading: docs (29% of read bytes) are pack material FR-2 already sends. Cross-worker duplication (37.6%) is a driver problem, solved by sharing recon in the pack. In-worker repeats (26.7% of lookup calls) are what a compact answer shrinks.

One mechanism matters. Tool results stay in context and are re-read on every later turn. The last three retros show 42M, 73M and 135M cache-read tokens per feature (PRD §19). Result bytes compound, so compactness pays on every turn after the call.

## 3. Options

Slices are ~1 worker each. Estimates rest on measured code sizes (unverified anchors); low confidence.

| Option | Answers well | Answers badly | Fresh after a slice | Latency | Slices |
|---|---|---|---|---|---|
| 1 Leave out, tune pack | docs, recon files | code reads, refs | n/a | n/a | 0 |
| 2 ripgrep ctx | def by regex, textual refs, strings | callers via magic dispatch | always, re-reads tree | 7-44 ms (confirmed) | 2 |
| 3 tree-sitter tags | outline, def kind, in-file symbols | cross-file typed refs | parse on demand, 7 ms/file | 0.47 s per 263 files; 4.4 s vendor PHP | 2-3 |
| 4 LSP proxy, driver-warmed | typed def/refs, call hierarchy (Go, Rust) | PHP magic; Vue without a forwarding client | needs driver file notifications | cold 6-69 s; warm 1-54 ms | 4-6 |
| 5 Hybrid rg + tree-sitter filter | def, refs, outline with kind and signature | receiver-typed refs | always; parses candidates only | tens of ms + 7 ms/file | 3 |
| 6 Claude Code LSP tool | def, refs, hover, symbols | Vue (no plugin); lookups before the first edit | server starts on first edit, per session | unknown; 60 s timeout added 2.1.288 | 0 code, plugin setup |

Coverage per stack:

| Option | PHP/Laravel | Vue+TS | Go | Rust |
|---|---|---|---|---|
| 2 ripgrep | defs 2 FP in 136; callers miss facades, scopes, routes | tags 79 vs 86 hits case-insensitive; script setup implicit | defs 255 = AST | finds 185 macro-made fns AST misses |
| 3 tree-sitter | tags.scm, 0.25.1, MIT | no tags.scm; grammar not on crates.io; script-block shim needed | tags.scm, 0.25.0 | tags.scm, 0.24.2 |
| 4 LSP | intelephense proprietary; phpactor 3 min cold (self-reported); PHPantom 8 months old | ts-ls + @vue/typescript-plugin; standalone Vue LS never answered | gopls BSD-3; 0.3 s warm | rust-analyzer 69 s, 3 GB on 107k lines |
| 5 Hybrid | rg + kind filter; magic unresolved | rg + TS grammar on script block | rg + tags | rg + tags |
| 6 CC LSP tool | php-lsp, needs intelephense binary | none | gopls-lsp | rust-analyzer-lsp |

Token evidence, risk, licence:

| Option | Token-saving evidence | Main risk | Licence |
|---|---|---|---|
| 1 Leave out | none; baseline 34M lookup tokens in 55 sessions | repeats stay at 26.7% | n/a |
| 2 ripgrep | none direct. Capped summarized search +6 pp over iterative (confirmed, GPT-4). `-n` 6.5x smaller than `--json` (unverified) | grep habit persists; no Claude study | Unlicense OR MIT (confirmed) |
| 3 tree-sitter | skeleton 58.3% at $0.02 vs full file 53.7% at $0.15 (confirmed, GPT-4o). AST read tool -12% to -38% tokens, 5 of 6 models (unverified) | custom Vue shim; C compiler at build | MIT (confirmed) |
| 4 LSP proxy | Opus +6%, Sonnet +118%, Haiku -26% tokens at equal success (confirmed, 6 tasks, Python/TS). Refs +19% tokens for +0.07 F1 (confirmed) | 0.3-3 GB RAM per server; PHP licence; Vue forwarding | servers BSD/MIT/Apache; intelephense proprietary; ra-multiplex EUPL and Serena GPL, do not vendor |
| 5 Hybrid | none direct. Gain tracks grep noise: hono +0.245 F1 and -12% tokens at grep P 0.51; remeda zero at P 1.00 (confirmed) | two engines to keep aligned | MIT (confirmed) |
| 6 CC LSP tool | none published (confirmed); 0 uses in 40,748 calls | plugin plus binary per stack; subagent LSP fixed only in 2.1.280 | tool proprietary; servers OSS |

Blunt summary. No study measures any option on Claude with PHP, Vue, Go or Rust. The thin version is the measuring instrument.

## 4. Design sketch: thin hybrid

Command surface. Three verbs, plain text, exit 0 on empty.

```
factory ctx def <symbol> [--kind class|fn|method|type] [--body]
factory ctx refs <symbol> [--in <dir>] [--max 30] [--per-file 3] [--tests]
factory ctx outline <path>... [--depth 1]
```

How it works. `def` runs the per-language definition regex through the grep crates. Each candidate file is parsed with tree-sitter. A hit survives only inside a definition node of the right kind, outside comments and strings. Output carries the enclosing class or receiver and the signature. `--body` prints the node's exact range, replacing the "sed 30 lines" guess. `refs` is a word-boundary search, filtered to drop comments and strings, collapsed per file. `outline` prints tags.scm definitions with signature lines: the Agentless skeleton. Vue: locate the `<script>` block, parse it with the TS grammar at offset. Vendor and gitignored paths are skipped unless `--vendor`.

Output format. Path:line first, then kind, then signature. One or two context lines, because bare locations tripled follow-up reads in the LSP study (unverified). A trailer says what was cut.

```
$ factory ctx def contentHash
app/Core/Sync/Documents/UpsertDocuments.php:88  method UpsertDocuments::contentHash(array $doc): string
   87    /** Stable hash of the document payload. */
   88    public function contentHash(array $doc): string
1 candidate dropped (docblock); --all shows it.

$ factory ctx refs UpsertDocuments --per-file 2
app/Core/Sync/SyncManager.php:41   $this->upsert = new UpsertDocuments($repo);
app/Core/Sync/SyncManager.php:77   return $this->upsert->handle($batch);
tests/Feature/Sync/UpsertTest.php:19   (4 hits, 2 shown)
12 hits, 5 files; 1 vendor file skipped. def: app/Core/Sync/Documents/UpsertDocuments.php:15

$ factory ctx outline internal/caea/device_sync.go
internal/caea/device_sync.go  412 lines  go
  12  type DeviceSyncService struct
  31  func NewDeviceSyncService(db *sql.DB, c Clock) *DeviceSyncService
  58  func (s *DeviceSyncService) codes(ctx, id string) ([]Code, error)
  97  func (s *DeviceSyncService) claim(ctx, code string) error
```

Driver pre-warm. Search needs no warming; rg answers in 7-44 ms. Two cheap warms exist.

1. After each slice commit, `factory ctx index` re-tags changed files. Cache key: (path, git blob sha). App trees tag in under 0.5 s; vendor is excluded.
2. The pack gains an "Outline" section. It holds `outline` of each recon file and `def` of each cited symbol. This targets the 39.7% repeated reads and the first read of every recon file.

An LSP daemon is phase 2, its own PRD. One fact for that PRD. gopls CLI took 1.2-6 s per call even against a daemon (unverified). Each CLI call opens a new session. The driver must hold the LSP session itself and push `didChangeWatchedFiles` after each slice.

Teaching workers. The pack gets a "Lookup" section: three verbs, three examples, one fallback rule. The rule: grep only for literal strings and log output. The built-in bypass-mode instruction still says cat, grep, sed. Whether the appended pack overrides it is unknown. Second lever: a PreToolUse hook on Bash. It matches `grep -r` and `rg` over the repo and returns a one-line hint. Hooks run under `claude -p` without `--bare` (confirmed). Expect weak uptake from text alone. Given free choice, models picked semantic lookup 0-6% for localization, 45-57% for references (unverified).

Measuring savings (FR-8). Add a `tool_calls` table keyed by worker:

| Field | Source |
|---|---|
| kind | ctx-def, ctx-refs, ctx-outline, bash-grep, bash-sed, bash-cat, read, run, edit |
| result_chars | stream-json tool_result |
| target_hash | normalized path or pattern |
| repeat_in_worker, repeat_in_run | target seen earlier |
| followup_read_within_2 | read of the same path within two calls |
| hook_hints | PreToolUse hint count |

Baselines to beat: lookup result mean 4,519 chars, median 2,066, p90 11,550. Read mean 12,373 chars. Strict lookup share 59.5% in build workers. Re-hit rate 26.7%. `factory report` adds `lookup_chars`, `ctx_share`, `repeat_rate`, `followup_reads`. A/B: the fixture slice with the Lookup section on and off, then the FR-11 runs. Unknown: the share of weighted tokens that lookup results drive. Only per-turn usage fields can answer it.

## 5. Recommendation for PRD-001

Include a thin version now, as FR-12, three slices.

1. grep-crate walker, per-language def regexes, compact capped printer.
2. tree-sitter tags for PHP, TS, Go, Rust; `outline`; def kind filter; Vue script shim.
3. Pack "Outline" and "Lookup" sections, hook hint, FR-8 fields and report keys.

Defer LSP, SCIP and typed resolution to PRD-003. Open it only if FR-8 shows def and refs calls still above a threshold the owner sets.

Evidence for this call:

- The problem is real and local: 56-74% of calls, 92% of result bytes.
- Reads dominate at 77%. Skeleton output is the only shape with a confirmed accuracy and cost win.
- Caps have a confirmed win; iterative search scored below having no search tool.
- LSP has confirmed negative token evidence on Opus (+6%) and Sonnet (+118%). Small study, Python and TypeScript.
- LSP costs 6-69 s cold and 0.3-3 GB per server. Add a PHP licence problem and a Vue forwarding client. That is 4-6 slices with no measured payoff.
- OPEN-16's "evidence against" holds for docs (29% of read bytes), not for code reads.
- Everything proposed is MIT or Unlicense; the tool can be published.

What reverses it: ctx under 10% of lookups after slice 3, or follow-up reads not falling. Then cut ctx to `outline` only and push the pack lever harder.

## 6. Refuted claims (do not cite)

- grep-matcher is 0.1.9, not 0.1.8; licence conclusion unchanged.
- The grep README sentence "Ambitious individuals can research and contribute" is invented; the warning exists with other words.
- `grep::printer::JSON` does not emit the `summary` record; only the rg binary does.
- Crate counts were 36 and 68, not 43 and 78. The measured ast-grep binary linked only PHP; a real 4-grammar build is 7.30 MB, 6.41 MB stripped, 11.6 s.
- phpactor stubs now augment definitions (additive stubs, 2025.12.21.0); the "replace" quote is stale. Cache.php has 2 named methods, Model.php 6 magic methods. Intelephense `@mixin` is a paid feature.
- PHPantom has 35 open issues and 37 open PRs, not 72 issues.
- tree-sitter-vue inherits `script_element` and `style_element` from tree-sitter-html; it also parses directives and interpolation, not only the skeleton.
- Caution, not refuted: the SWE-agent paper says a 50-result cap; the shipped code caps at 100.

## 7. Open questions

- Method behind the PRD's 44%; transcripts say 56% strict, 74% literal.
- Does the appended pack override the built-in "use cat/grep/sed" instruction? Unknown.
- Do Bash deny rules apply under bypassPermissions in 2.1.280? Unknown.
- Which model do workers run? The LSP token effect flips by model.
- What share of weighted tokens do lookup results drive via cache reads? Needs FR-8 per-turn usage.
- Does intelephense's EULA allow a headless driver? Not addressed.
- Will workers adopt `factory ctx` over grep? Only FR-8 can tell.