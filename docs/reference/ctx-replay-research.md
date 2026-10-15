# ctx replay research: `factory ctx` against grep, sed and cat in 14 runs

Research run 2026-10-15, during the harvest of 001 to 008, to answer the owner: where do workers call grep, sed and cat, would `factory ctx` have answered the same, and was the quality of either worse? The analysis scripts and the rebuilt clones stayed in the session scratchpad; the method below is enough to redo them. Read-only on repos, on a copy of `factory.db`.

## Summary

1. **Inventory:** 14 runs and 159 workers made 9,031 tool calls, which split into 13,797 single lookups. ctx is 392 of them (2.8%). For the questions ctx is built to answer, it was used for 10% of definition lookups (155/1,531) and 23% of reference lookups (213/915).
2. **What workers ask:** in a hand-labelled sample of 240 non-ctx lookups, only 16–17% ask what ctx answers (def 20, refs 15, outline 5). Reading a known line range (73) and literal, log or doc lookups (about 75) dominate. Even with perfect steering, ctx could reach about 19% of all lookups.
3. **Definition replay:** 1,240 definition greps replayed at each worker's tree; 816 comparable. ctx found the same definition in 77%, 18% were ambiguous (more than 5 same-name definitions), 3% were in vendor code and 3% were wrong or missed. Its output was 3.8× the grep's because it prints bodies (13× when ambiguous); without bodies it would be 0.67×.
4. **References replay (464 searches):** ctx missed a real code line in only 3.7%. In 59% it dropped test, comment, string or doc hits, which is right if the worker wanted callers and wrong if it wanted a literal. Output was about the same as the grep (1.1×), but 1.9× when the grep was limited to one file, because ctx can't limit to a file.
5. **Real ctx calls (396):** 14% came back empty (refs 23%), 26% were followed by a file read and 9% by a grep anyway. The hook forced 32% of them, and forced calls were empty 25% of the time against 9% when the worker chose ctx, mostly because a literal or vendor name was pushed into ctx.
6. **The hook mostly misfires:** of 175 denials in 61 workers, 62% blocked literal or regex greps, which its own hint says to grep, and 3% were vendor. 36 workers were denied repeatedly, up to 14 times. It is easy to get around: `git grep` (935 uses) and any grep that isn't the first command pass, so about 90% of recursive greps were never denied.
7. **Mistakes traced to lookups:** none. 18 fix workers and 54 build test failures don't trace to a lookup miss. The compiler or PHPStan listed missed callers in one run. The out-of-date references were in test expectations and strings, which ctx refs leaves out by design.
8. **A (force ctx through the hook): reject.** Up to 1,687 build and polish lookups would be denied at about one turn each (92.6K input tokens per turn): +17–34% turns and about +$75–150 over these runs, for answers that are no smaller, empty 25% of the time, and no more correct.
9. **B (shrink to outline only): reject. C (keep as is): leaves known waste.** B keeps the weakest command: outline needed a follow-up read 71% of the time, against 16% for def and 29% for refs, and it saves under 1% of lookup chars. C keeps about 157 build and polish denial turns per 14 runs (3% of turns), two thirds of them misfires.
10. **D (recommended):** deny only identifier-shaped names outside vendor code; put the ctx answer in the denial itself; print definition bodies only for a single match (−67% def chars); let `--in` take a file; index PHP and Go constants and Pest tests; measure ctx share per question type. Expect −2–3% turns and fewer empty answers. Limits: ctx v0.6.0 replayed on rebuilt trees (73% fidelity), one owner and one model family, 14 runs.

## 1. Inventory: who called what (all roles, single lookups)

Each bash call was split into its separate commands. The metrics store records only the first command, and about 80% of bash lookups bundle two or more.

| Family | Lookups | Repo (language) | Lookups | ctx | ctx share |
|---|---|---|---|---|---|
| grep (4,568), git grep (935), rg (14) | 5,517 | factory (Rust) | 7,029 | 167 | 2.4% |
| sed | 4,139 | icg-cloud (PHP/Vue) | 2,606 | 94 | 3.6% |
| cat | 1,129 | arca-api (Go) | 3,468 | 103 | 3.0% |
| find/ls | 1,069 | cody-agent (PHP/Vue) | 694 | 28 | 4.0% |
| Read tool (395 ranges, 249 whole files) | 644 | | | | |
| head/tail 459, other (wc, git show) 447, Grep tool 1 | 907 | | | | |
| ctx (refs 213, def 155, outline 24) | 392 | **all** | **13,797** | **392** | **2.8%** |

- **ctx share, build/polish:** factory 1.6%/4.0%, icg 4.5%/3.5%, arca 4.7%/1.1%, cody 5.0%/4.2%. Per run 0–6.3%; no run reached the 10% line FR-12 uses.
- **Heuristic intent, grep:** def 1,376, refs 702, in-file 505, literal 2,138, log 543, file listing 253.
- **Heuristic intent, sed:** code range 3,400, non-code range 553.
- **Heuristic intent, cat and Read:** cat whole code file 504, cat whole non-code file 492, Read code range 305, Read whole code file 106.
- **Where the bytes go (build/polish):** the Read tool returned 6.7M chars (3.6M code, 2.06M logs in 90 calls of about 23K each); whole-file code reads 2.87M (525 reads); all ctx output about 0.9M, counting whole bundled calls.

## 2. Intent mix: hand labels on 240 non-ctx lookups (build/polish)

60 per repo, families in proportion within each repo; 3 languages, 8 repo × role cells.

| Intent | factory | icg | arca | cody | all | Does ctx answer it? |
|---|---|---|---|---|---|---|
| range (read a known span) | 19 | 17 | 22 | 15 | 73 | partly (def) |
| literal or string | 9 | 9 | 10 | 6 | 34 | no |
| def | 5 | 5 | 5 | 5 | 20 | **yes** |
| log, non-code read, listing, size, history, other | 18 | 13 | 16 | 20 | 67 | no |
| read a whole code file | 2 | 7 | 3 | 6 | 18 | partly (outline) |
| refs | 2 | 5 | 2 | 6 | 15 | **yes** |
| scan (several terms in one file) | 2 | 3 | 2 | 1 | 8 | no |
| outline (+1 on a non-code file) | 4 | 1 | 0 | 1 | 6 | **yes** (5) |

- ctx can answer 40 of 240 (17%), or 16% weighted by each repo's population.
- Several lookups the heuristic called "refs" were literals: DB columns inside PHP strings, routes, config keys, concept words. In Laravel many "symbols" are strings, and ctx refs leaves strings out.

## 3. Replay: what ctx would have answered at that moment

**Method.** Each worker's tree was rebuilt in a private clone: base commit, then its Edit/Write calls in order, then its working directory. The original command was re-run and `factory ctx` v0.6.0 asked the same question. The re-run matched the transcript for 176 of 240 sampled lookups (73%). The population replay covers 6,428 build/polish lookups whose heuristic intent was def, refs, in-file, range or whole-code.

### 3a. Definitions (1,240 definition greps; 816 comparable)

The other 424 are heuristic noise: no name 139, both empty 149, ctx-only 113, unresolved 23.

| Grade | n | Share | Grep chars | ctx chars (with body) | ctx `--no-body` |
|---|---|---|---|---|---|
| same | 441 | 54% | 361K | 545K (1.5×) | 70K |
| same plus other same-name defs | 188 | 23% | 133K | 501K (3.8×) | 78K |
| ambiguous (>5 defs) | 143 | 18% | 89K | 1,184K (13×) | 247K |
| vendor (clones have no vendor dir) | 21 | 3% | | | |
| wrong or miss | 23 | 3% | | | |
| **total** | 816 | | 601K | 2,260K (3.8×) | 404K (0.67×) |

- **Body only for a single match:** simulated at 736K, 1.2× the grep.
- **Wrong or miss, by hand:** mostly heuristic artefacts (a type annotation like `string` taken as the name; a grep limited to one file where ctx returned same-name definitions elsewhere). The real gaps: extensionless scripts (`tests/stubs/claude`), trait impls (`impl Display`), PHP class `const`, Go constants inside a `const ( )` block.
- **Follow-up reads:** 62% of definition greps already used `-A/-B/-C`. Only 18% were followed by a read of the definition's own lines, the call a ctx body would save.
- **Manual sample (20):** same 12; ctx better 2 (the grep looked in the wrong file; the grep hit a call site first); partial 1 (the worker also wanted callers); noisy 1 (12 same-name definitions, 6,020 vs 992 chars).
- **Manual misses (4):** a PHP class const; a Go grouped const; two vendor names, where the hook denied the grep into vendor code, `ctx def` found nothing and the worker grepped again (2 extra calls).
- **Manual chars:** ctx 20.5K vs grep 12.2K. One wanted only locations: 4,048 vs 206 chars.

### 3b. References

| | refs (recursive) | in-file (one file) |
|---|---|---|
| greps with hits | 464 | 345 |
| ctx finds every grep line | 171 (37%) | 178 (52%) |
| ctx drops only test/comment/string/doc lines | 276 (59%) | 153 (44%) |
| ctx misses a code line | 17 (3.7%) | 14 (4%) |
| mean recall vs the full grep | 0.55 | 0.66 |
| recall vs what the worker saw, with ctx default limits | 0.43 | 0.45 |
| chars: worker saw vs ctx default | 417K vs 465K (1.1×) | 166K vs 323K (1.9×) |

- **Lines ctx dropped:** test 840, string about 706, comment 543, doc 242, code 43.
- **cody-agent (Laravel)** has the lowest recall, 0.39, mostly from strings.
- **Manual sample (15):** same 10; ctx better 1 (a grep for `Log::` also matched `AuditLog::`); partial 2 (`Update` hit the 10-result cap; 26 hits against a cap of 10); noisy 2 (ctx can't limit to a file).
- **Manual chars:** ctx 17.2K vs grep 6.9K.

### 3c. Outline, line ranges and whole files

- **Manual outline (5):** same 3; partial 1 (wanted call sites, 5,080 vs 134 chars); miss 1 (Pest `test()`/`it()` cases are function calls, not definitions).
- **Code line ranges (3,465) against the file's outline:** exactly one definition 524 (15%); inside a bigger definition 649 (19%); inside a class or impl 356 (10%); spans several definitions 1,334 (38%); partly outside 96; no definition 214; whole file 89; unresolved 203.
- **A one-definition range replaced by `ctx def`:** returns exactly one definition in 72% of cases, at 2.2× the chars in total (median 1.3×).
- **Whole code files (525):** 2.87M chars (median 4.3K) against outline 0.34M (median 356). But real outline calls were followed by a read in the same file 71% of the time.

## 4. How the real ctx calls went (405 calls; 9 were `--help`)

| | n | Empty | Forced by hook | Follow-up read | Grepped anyway |
|---|---|---|---|---|---|
| def | 160 | 6% | 17% | 16% | 4% |
| refs | 212 | 23% | 46% | 29% | 13% |
| outline | 24 | 0% | 8% | 71% (16 inside the outlined file) | 8% |
| all | 396 | 14% | 32% | 26% | 9% |

- **Forced vs chosen calls:** empty 25% vs 9%, follow-up read 21% vs 29%, grepped anyway 13% vs 8%.
- **The 48 empty refs:** 13 were not identifiers (`1503`, `Content-Security-Policy`, `CI gate`, `slices\[`, a log file name, `rev-parse`). Most of the rest were DB columns or JS keys, or vendor methods.
- **What the hook blocked:** 175 denials point to ctx (also 41 gh/push denials and 27 pack-outline denials). 62% literal or regex greps, 30% identifiers, 5% definition patterns, 3% vendor. 77% were bundled commands, so the denial threw away the other commands as well. The next call was ctx in 52% of cases, grep again in 18%, `git grep` in 6%, something else in 24%.
- **How denials spread:** 25 workers were denied once and 36 more than once (mean 2.9, max 14). 68 denials were in polish review subagents.
- **Why it's easy to get around:** the hook (`src/cli/hook.rs`) judges only the first command, lets `git grep` through, and its hint doesn't mention `--vendor`. Of about 1,865 recursive greps, 175 (9%) were denied.

## 5. Mistakes from lookups, looking back

- **Compile and type errors after edits (57 results, 28 workers):** 10 read. They were mid-edit states, or deliberate signature changes where the compiler then listed every call site in one run.
- **The one wrong assumption:** a worker used `.Body` on a Go helper that returns a string. `ctx def` would have shown the return type, but `go vet` caught it in the same run.
- **Fix workers (18):** all fixed format, clippy, CRAP-score, flaky-test or unstaged-file problems. None came from a lookup.
- **The one missed caller:** it crossed lanes. One slice changed a function's argument, and a sibling slice's tests in another lane still used the old one. No lookup in either lane could have seen it.
- **Build-stage test failures (54 final reports):** mostly format, clippy or PHPStan, test literals, or a sibling slice's changes. The out-of-date references were test expectations and string matches, which ctx refs leaves out by default.
- **Net:** no defect reached a gate because of a lookup miss. ctx's measured benefit is precision (3 of 35 sampled cases where it beat grep). Its measured costs are empty answers when forced and size when it prints bodies or ambiguous names.

## 6. Options

Unit costs: one build/polish turn is about 92.6K input tokens (466M over 5,032 turns), about $0.09; build/polish cost $451 in total.

**A. Force ctx through the hook** (deny identifier greps, `sed -n` and `cat` where `ctx def` would answer). **Reject.**
- **Turns:** 1,687 lookups carry an identifier grep and 1,918 a code range. Denials don't teach (36 of 61 workers were denied again). Identifier greps alone add +840 to +1,690 turns (+17–34%), +78–156M tokens, about +$75–150. Ranges and cat roughly double that.
- **Answer size:** no saving. def 1.5–3.8×, refs 1.1×, one-file searches 1.9×, one-definition ranges 2.2×.
- **Correctness:** forced calls come back empty 25% of the time and are grepped anyway 13%. Forcing would also hide test, string and doc hits in 59% of reference lookups, the kind of stale reference that caused the red builds.

**B. Shrink ctx to outline only.** **Reject.**
- **Tokens:** def and refs output is 0.3M of about 36M lookup chars (under 1%).
- **Turns:** removing the grep deny saves about 157 turns (−3%), but D1 gets most of that without dropping def and refs.
- **Correctness:** it loses def (77% match) and refs (96% recall of code lines) and keeps outline, which needed a follow-up read 71% of the time and misses Pest cases.
- **FR-12 points the other way:** global share is under 10%, but share within ctx's own question types is 10–23%. The 30% follow-up threshold flags outline, not def or refs.

**C. Keep as is.** Safe, but it leaves about 157 denial turns per 14 runs (3.1% of turns, about 14.5M tokens, about $14), 65% of them misfires; 14% empty ctx answers; 2.26M chars of definition bodies where the grep showed 0.6M.

**D. Recommended, in order of value for the effort:**
1. **The hook judges the pattern, not just `-r`:** deny only identifier-shaped patterns outside `vendor/` and `node_modules/`, let literals, regex and vendor paths through, add `--vendor` to the hint. Removes about 65% of denials (about 100 turns, −2%, about −9M tokens per 14 runs) and most forced empty answers.
2. **The denial carries the answer:** the hook runs `factory ctx refs|def <word>` (about 40 ms) and puts the output in the denial message. 52% of denials were followed by exactly that call, so this saves about 30–50 more turns. Rewriting the command in place via PreToolUse `updatedInput` would cost zero turns but changes commands silently; limit it to identifier patterns if tried.
3. **`ctx def` prints a body only for a single match:** for two or more, signatures plus a `--kind`/`--in` hint. Definition output drops from 2.26M to 0.74M chars (−67%, 1.2× the grep).
4. **`--in` accepts a file for refs and def:** covers the 438 in-file lookups (1.9× today) and the noisy cases.
5. **Index gaps:** PHP class constants, Go grouped consts, trait impls, Pest `test()`/`it()` in outline, extensionless scripts. About 3% of definition lookups.
6. **Change the metric:** track ctx share per question type (def 10%, refs 23%, outline about 4%; ceiling about 19% of all lookups) plus forced-empty and grepped-anyway rates, and drop FR-12's global under-10% → outline-only trigger.
7. **Don't steer `sed -n` ranges or `cat` to ctx:** only 15% of ranges map to one definition, at higher cost. The bigger byte savings are outside ctx: log reads through Read (2.06M chars in 90 calls) and whole-file code reads (2.87M).

**Expected effect of D1–D3:** −2–3% build/polish turns (−100 to −150 per 14 runs; −10–14M tokens; about −$10–14); about −1.5M chars of definition output; the ctx empty rate falls from 14% toward the 9% of chosen calls; correctness unchanged. The gains are modest: lookup cost lies in range, whole-file and log reads, not in grep against ctx.

## 7. Evidence limits

- **ctx version:** v0.6.0 replayed, not each worker's version (the runs span v0.1 to v0.6), so it grades today's ctx.
- **Rebuilt trees:** base commit plus replayed Edit/Write calls only; bash edits (python, perl, `sed -i`) not replayed. 73% fidelity on the sample. 105 population rows had no base commit and 404 had some edit-replay failures. The clones have no `vendor/` or `node_modules/`.
- **Labels:** population intents are heuristic ("refs" includes literals; 424 of 1,240 "def" rows not comparable). Hand labels cover 240 lookups, plus hand-graded ctx answers for 40.
- **Measurement:** miss kinds come from a regex ("string" is approximate); follow-up means a read within the next 3 calls; purpose is inferred.
- **Scope:** one owner, Opus-family workers, 14 runs over 5 days on 4 repos; cody-agent had one run.
- **Costs:** average turn cost; A assumes 50–100% of its denials repeat; 68 denials were in subagent turns, not counted in `num_turns`.
- **Mistake search:** a regex over compiler, vet, PHPStan and TS errors plus fix, build and checkpoint final reports. Not exhaustive; slips that never caused a failure can't be traced.
