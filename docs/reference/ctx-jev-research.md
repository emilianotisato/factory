# Jev and `factory ctx` research (owner question, 2026-10-09)

**Question.** The owner asked whether TypeSafe AI's Jev "System One" model (a ~100 ms, calibrated, non-generative decision API: Choice / Score / Noul questions over a text state, $0.042 per million input tokens) earns a place next to ripgrep and tree-sitter in `factory ctx` or in the driver, after [`ctx-research.md`](ctx-research.md) settled FR-12 as a thin local hybrid.

**Provenance.** A 23-agent research workflow (`wf_81a9b68f-86b`, about 2.0M tokens, 574 tool uses, 68 minutes): five evidence surveys (Jev docs and legal pages, cookbooks, a fresh re-mine of 653 owner transcripts, alternatives, prior art), four independent designs (read compaction, def/refs ranking, driver-side triage, minimal skeptic), three judges, refuters on every load-bearing claim (73 confirmed, 34 refuted, 5 unverifiable), one synthesizer, one completeness critic. The agents had no API key and sent nothing to TypeSafe. **After the memo, the owner supplied a key and allowed transcript text from the two dogfood projects to go to the API; the section "Replays with the owner's key" reports what was then measured. Where the memo says "unknown, no key" (latency from this machine, Jev accuracy on real lookups), that section supersedes it.** Scratch material, including the mining scripts, stayed in the session scratchpad; the replay scripts are in git history: `docs/features/001-driver/ctx-jev-replay/` at `2b27832`.

**Outcome in one line.** Not now: no Jev FR, slice or config key in PRD-001. The replays confirmed the memo's call on the two options with ground truth, and killed the one the memo could not test (block auto-answer). Of the four Jev-free grafts the owner kept three on 2026-10-09 (`followup_reads` and `pack_doc_read_chars` in FR-8, the pack-doc hint in FR-2, `def` body and `refs --max 10` defaults in FR-12) and cut the seen-before stub for repeated tool results; a parked line went to `ideas.md`.

---

# Decision memo (synthesizer output)

Tags: **[C]** confirmed by the verification pass (measured; commands in scratchpad `jev/`), **[R]** recomputed by a judge or verifier, **[Q]** quoted (file and line in scratchpad `jev/`), **[E]** estimated (arithmetic shown), **[U]** unverified.

## 1 Headline

**Not now.** No Jev slice, FR, or config key in PRD-001. Keep FR-12 as written and graft four Jev-free items (section 5).
Deciding fact: in the owner's 653 transcripts the Jev-attributable prize is ≤0.1% of lookup bytes for def/refs picking and <1% weighted for driver-side triage [C]. The only pool big enough to matter, whole reads of code at 8.2% of result bytes [C], has no offline ground truth, Jev has no published code-task evidence, and no key exists to probe even public code.
Judges split 2–1 for the minimal option; the dissenting token judge ranked read-compaction first but states its saving is not Jev-specific. I side with the two.
Re-open only if FR-12 ships and FR-8 shows follow-up reads after `ctx` ≥30%; then run the offline oracle (section 6) before any network call.

## 2 What Jev is for this problem

| Item | Value | Tag |
|---|---|---|
| Endpoint | `POST https://api.typesafe.ai/v1/systemone`, Bearer, body `{state, model, questions}`; `questions` is a map keyed by id, lowercase types, criteria map | Q api.md |
| Model | `jev-1.13.0` (aliases `jev-latest`, `jev-preview`) | Q models.md |
| Primitives | Choice: ≤255 options → probabilities + confidence. Score: 2–10 levels. Noul: scalar 0–1 | Q api.md |
| Caps | 64k tokens per request; 32k for state + longest question | Q api.md |
| Price | $0.042 per MTok input; output free | Q models.md |
| Rate limits | 100K tok/s, 80 req/s, "dynamic" | Q api.md |
| Latency | vendor: ~100 ms; 70–500 ms "from US West Coast". From this machine (America/Mexico_City): unknown, no key | Q blog; U |
| Nature | Non-generative. Ranks or filters candidates the caller supplies. Cannot emit a symbol, path or line | Q blog |
| Jaggedness | literal reading, no counting, first-option lean, context rot, state not hostile-aware | Q cookbooks |
| Privacy terms | US-hosted; retention "as long as reasonably necessary"; not trained on customer requests; ZDR enterprise-only; broad Input licence; Telemetry "without restriction"; may delete Customer Data any time; Customer warrants all consents | Q section 7 |

Meaning for ctx: Jev is a cheap picker. It needs a candidate list and a yes/no or which-one question. In `factory ctx` the candidates come from ripgrep and tree-sitter, so Jev's value is bounded by how often that list is long and mis-ordered. Section 3 says: rarely.

## 3 Where the bytes are

Corpus: 653 jsonl (54 main, 595 subagent), 41,308 tool calls, 163.6M result chars [C].

| Slice | Count | Result bytes | Note |
|---|---|---|---|
| Lookups (Read, cat, sed, grep/rg, ls, find) | 68.0% of calls | 87.9% | FR-12 target [C] |
| Read / cat / sed / grep | 21.3% / 22.8% / 21.6% / 13.2% of calls | — | cat+sed > Read; detector must parse Bash [C] |
| Whole-file reads, sized | 2,253 | 26.3M = 16.1% | [C] |
| of which code | 1,394 | 13.4M = 8.2% | only pool >5% a picker could touch [C] |
| of which pack docs (prd, plan, ctx) | 631 | 10.6M = 40.3% of whole-read chars | dedup lever, no model [C] |
| Re-reads of persisted tool results | 239 | 9.77M | sha256 cache lever [C] |
| def-style greps | 91.2% single hit; 1.6% ambiguous across app files | — | nothing to disambiguate [C] |
| ref-style greps | median 3 hits; 74.6% head-capped by the worker | — | [C] |
| read→edit pairs | 404 | edits touch ~9.1% of 4.03M chars read | compaction ceiling input [C] |
| Whole reads never edited | 73–77% per transcript; 30.1% per session tree | — | "never edited" ≠ "not needed" [C] |
| Triage after red | median 10,197 chars; log alone suffices 7.7% | — | [C] |
| Driver-visible reds | 31 in 14 retros = 2.2/feature | — | ~2 decisions per feature [C] |

## 4 Options

| Option | What it answers | Jev calls/lookup | Saving estimate (confidence) | Privacy exposure | Slices | Fatal flaw |
|---|---|---|---|---|---|---|
| A. Nothing beyond FR-12 + grafts | — | 0 | FR-12's own estimate; grafts add measurement and the 40.3% pack-doc dedup lever (high confidence it costs nothing) | none | 0 new; 4 grafts | none found; leaves the 8.2% pool unmeasured |
| B. Offline replay oracle (experiment) | Could any perfect picker save ≥5% of lookup bytes on code reads? | 0 in stages 0–1; ~1,050 public-only in stage 2 | produces the number; expected answer "no" (ceiling 0.9–2.2%, see C) | none; stage 2 public code only | 0.5 day + 1 day | may be wasted if FR-12's own reversal fires first |
| C. Read-compaction `ctx read --for <intent>` | Which tree-sitter nodes of this file does the worker need? | 1 per Read | provable ≤2.2% of result bytes [E: 4.03M×(1−0.091)÷163.6M]; verifier's tighter 0.9–1.7% [R]; 8.2% only if every code read compresses to zero | full file + intent per read; client code | 2–3 | no ground truth; saving belongs to the node selector, not Jev |
| D. Disambiguate-rank def/refs | Which of N hits is the def / top refs? | 1 per ambiguous lookup | ≤0.1% of lookup bytes [C] | symbol + candidate lines with paths | 1 | 91.2% single hit; nothing to rank |
| E. Driver-side Suspects/Prefetch | After a red gate, which files and log lines feed the retry? | 1–2 per red | <1% weighted [C]; log-only suffices 7.7% | failing log + diff + file list | 2 | 2.2 decisions/feature cannot move FR-8; deterministic halves equal it |

## 5 Judge scores

| Design | Token saving | Privacy/ops | Build cost | Total |
|---|---|---|---|---|
| A minimal-skeptic | 5 | 9 | 9 | 23 |
| C read-compaction | 6 | 5 | 6 | 17 |
| E driver-side | 3 | 8 | 3 | 14 |
| D disambiguate-rank | 2 | 7 | 4 | 13 |

Disagreement: the token judge ranks C first, then writes that C's saving comes from the verb and a local node selector, not from Jev. The other two rank A first. Pick A: C's one-point win is not Jev-attributable and it loses seven points on privacy and build.

Grafts worth keeping (all Jev-free):
1. Labels `followup_read_within_2`, `expanded_within_5` and report key `followup_reads` into FR-12 slice 3 / FR-8. Detector parses Bash `cat` and `sed -n`, not only Read.
2. Pack-doc dedup: 631 reads, 10.6M chars; report key `pack_doc_read_chars`; FR-2 lever.
3. sha256 payload cache for re-read persisted tool results (239 reads, 9.77M chars).
4. `def --body` default on: 86.9% of def greps already carry `-A`, median 25 [C]. `refs --max 10`: worker cap mode is 10 [C].

To ideas.md: `ctx read --range/--around/--outline-only` whole-node printing; deterministic Suspects and Prefetch halves; Stage 0 oracle.
If Jev is ever built: circuit breaker, honour `retry-after`, pin `jev-1.13.0`, report per median session tree (5 code reads) not mean (35.7) [C].

## 6 Experiment design

**Stage 0, offline oracle.** No key, no network, half a day. Reuses `common.py`, `q4`, `q5`.
- Input: 404 read→edit pairs and 1,394 code whole reads.
- Oracle: per read, the tree-sitter nodes later edited or quoted by the worker. Bytes a perfect picker keeps vs bytes sent.
- Gate G0: perfect picker saves ≥5% of lookup bytes. Expected fail. On fail: stop, one ideas.md line.

**Stage 1, free baselines.** One day, only after G0.
- aider-style PageRank over tags; "nodes whose identifiers appear in the prompt or last tool output"; local MiniLM cross-encoder (0.39–0.47 s per 30 pairs, measured [C]).
- Metric: node recall at k vs oracle.

**Stage 2, Jev on public code.** Only with a key and after Stage 1.
- Corpus: ripgrep or helix repo with synthetic edit tasks. `--public-only` hard-coded; deny globs on owner paths; redaction pass; manifest and audit dir of every payload. Drop GREP→OPEN (D) from the plan.
- Shape (semantic_find, checked against api.md): state = file with `L052| text` line ids; one Choice over node ids plus one Noul "needed code is in this file"; thresholds FOUND 0.7 / ABSENT 0.35 from the cookbook.
- Budget: ~1,050 calls × ~4k tokens ≈ 4.2M tokens ≈ $0.18 [E]; hard cap $1.
- Gate G1: ≥10 pp recall over best free baseline; confident-wrong ≤5%; option-shuffle agreement ≥90%.
- Gate G2, online, later: FR-12 survives its own reversal rule (`ctx_share` ≥10%, `repeat_rate` <26.7%); lookups still >50% of bytes; `followup_reads` after ctx ≥30%; ≥3 real feature runs.

Fallbacks if anything ships: timeout >500 ms, 429 or 5xx → print whole file (today's behaviour); confidence below threshold → whole file; breaker opens after 3 consecutive failures per run.
FR-8 keys: `jev_calls`, `jev_fallbacks`, `jev_bytes_sent`, `jev_bytes_saved`, `jev_p50_ms`.
Reversal: `jev_bytes_saved` <5% of lookup bytes over 3 runs, or `jev_fallbacks` >20% → remove.

## 7 Privacy gate

What leaves the machine, per option: A and B stages 0–1, nothing. B stage 2, public repo code only. C, the full source file and an intent string on every read. D, symbol and candidate lines with paths. E, failing log, diff hunks and file list. C, D and E send client code (ICG, Prokey) by construction.

Quotes, grep-verified in scratchpad copies (URLs in `legal.md`):
- Privacy policy l.73: "The Services are hosted in the United States".
- l.67: "We retain personal data about you for as long as reasonably necessary to provide you with the Services, or otherwise in support of our business or commercial purposes."
- l.46: "We (1) will not train or fine tune any artificial intelligence or machine learning models on Input, and (2) will not disclose any Input to a third party other than our service providers."
- models.md l.56: "Jev is not trained on customer requests or responses."
- legal.md l.17: "We also offer zero data retention (ZDR) for enterprise customers."
- MCA l.39: "Customer hereby grants TypeSafe a non-exclusive, worldwide, royalty-free, fully paid-up, non-sublicensable (except to service providers), non-transferable … right to use, copy, store, disclose, transmit, transfer, display, modify, create derivative works from, and otherwise Process (a) during the Term, any data, files, queries, and other materials that Customer …"
- MCA l.43: "TypeSafe may Process Telemetry without restriction, including to improve the Services or TypeSafe's other prod[ucts]".
- MCA l.47: "Customer represents, warrants, and covenants that it has made all disclosures, has provided all notices, and has obtained (and will maintain) all rights, consents, and permissions necessary for TypeSafe to exercise the rights granted to it in this Agreement".
- MCA l.78: "TypeSafe will be under no obligation to store or retain Customer Data and may delete Customer Data at any time in its sole discretion."
- MCA l.56: Credits "are consumed by each Input submitted"; l.49: breach of 8.2(b) Promotional Credits is a suspension trigger.

Verdict: client source must not leave the machine without the client's written consent, and the MCA puts that duty on the owner (l.47). Retention has no day count; ZDR is enterprise-only; no DPA text in scratch. Public-code replay is the only clean path today. A and B stages 0–1 never reach this gate.

## 8 How this changes PRD-001

- New FR: none. Jev slice in FR-12: none. Config key: none.
- ideas.md, one line: "Jev picker over tree-sitter nodes for `ctx read`: blocked on Stage 0 oracle showing ≥5% of lookup bytes, a key, and client consent."
- Concrete Jev-free edits:
  1. FR-8: add `followup_reads` (count, chars) and `pack_doc_read_chars`. `followup_reads` exists only in ctx-research.md l.151/154 today, not in prd.md [C].
  2. FR-12 slice 3: labels `followup_read_within_2`, `expanded_within_5`; detector parses Bash `cat`/`sed -n`.
  3. FR-12 `def`: `--body` default on; `refs --max 10` default.
  4. ctx-research.md §5: Agentless 58.33/53.67 is "Contains GT" localization coverage, not resolve rate. Add 2607.09691 (Claude Sonnet 4.6, N=70): skeleton of remainder vs deleting it p=0.75; keep/drop vs full p=0.07, not significant; paper disclaims the deterministic-vs-model reading.
  5. §20 retro table: `claude -p` one-shot wall is 2.8–3.6 s measured with `< /dev/null`; the earlier 5.2–6.2 s included a 3 s stdin wait.
  6. prd.md:131 cost weights (cache read 0.1, output 5): no Jev row.

## 9 Refuted claims (do not cite)

Each appeared in a design, judge report or earlier draft and was refuted by measurement or by the source text.
1. "82.1% of whole reads are never edited" — per transcript 73–77%; per session tree 30.1%.
2. "`claude -p` one-shot takes 5.2–6.2 s" — 2.8–3.6 s with `< /dev/null`; the gap was a 3 s stdin wait.
3. "Haiku costs 10x Jev per decision" — 2.5–3.7x ($0.32–0.47 per 1,000 decisions).
4. "Agentless 58.33/53.67 is a resolve rate" — localization coverage.
5. "2607.09691 shows deterministic skeletons beat model selection" — paper disclaims it; skeleton vs delete p=0.75.
6. "LLMLingua-2 compresses code usefully" — <1% EM on code.
7. "Jev latency is ~100 ms here" — vendor figure, US West Coast; unknown from Mexico City.
8. "Jev has zero data retention" — enterprise-only.
9. "Jev output tokens are billed" — output free.
10. "Promotional credits will cover the experiment" — discretionary; 8.2(b) breach is a suspension trigger; unverified.
11. "def lookups are often ambiguous" — 91.2% single hit.
12. "refs lookups return long lists" — median 3 hits; 74.6% head-capped.
13. "`followup_reads` is already a PRD metric" — ctx-research.md only.
14. "Jev can propose the right file or symbol" — non-generative.
15. "Jev can count hits or lines" — no counting.
16. "A local 0.5B coder model is a free Jev" — Qwen2.5-Coder-0.5B: 13 s and wrong.
17. "SWE-agent shows smaller views always win" — 100 lines 18.0 > 30 lines 14.3 > full 12.7; inverted-U.
18. "Driver-side triage has many decisions per feature" — 2.2.
19. "The failing log alone usually suffices for triage" — 7.7%.
20. "Plan on 35.7 code reads per session" — mean; median is 5.
21. "Read-compaction's saving is Jev-attributable" — token judge concedes it is the verb/local selector.
22. "Pack docs are a small share of reads" — 40.3% of whole-read chars.
23. "Jev questions are a list" — map keyed by id; lowercase types; criteria map.
24. "Workers edit most of what they read" — ~9.1% of 4.03M chars in 404 pairs.
25. "LogDx-CI shows a model router is needed" — its deterministic router scores 0.670 vs raw 0.353.
26. "The API was probed" — no `TYPESAFE_API_KEY` in env; nothing sent; no latency measured.
27. "File text in state is safe from injection" — cookbook: state not hostile-aware.
28. "Jev is trained on customer data" — "not trained on customer requests or responses."

## 10 Open questions

| Question | Who/what answers |
|---|---|
| Jev accuracy on code node selection (no published evidence) | Stage 2 on public code; owner decides on a key; agents never sign up |
| Promotional credits exist and cover ~$1? | TypeSafe sales; MCA 8.2(b) |
| Latency from this machine (America/Mexico_City) | 20-request public-payload probe once a key exists |
| Behaviour over 64k, max questions per request, key format | api.md silent; TypeSafe docs/support |
| Retention in days; subprocessor list | DPA (not in scratch); TypeSafe legal |
| What the never-edited whole reads were used for (question-shaped vs edit-shaped) | Stage 0 oracle labels; owner spot-check of 30 reads |
| Will workers call `ctx` at all | FR-12 slice 1, FR-8 `ctx_share` |
| Planning unit: session tree or feature | Owner; changes every per-feature saving figure |
| Client consent (ICG, Prokey) for source leaving the machine | Owner with clients; gate for C/D/E |
| Deprecation policy for a `jev-1.13.0` pin | models.md silent; TypeSafe |
| Determinism: same state and question → same probabilities | Stage 2 repeat/shuffle test |
| Orchestration: the user's "open a PR from a new branch, give me the URL" | Outside this memo; orchestrator owns it. This memo changed no files and made no commits |

---

## Replays with the owner's key (measured 2026-10-09)

The workflow agents had no API key, so every Jev figure above is quoted or estimated. After the owner supplied a key and allowed transcript text from icg-cloud and arca-api to go to the API, three scripts replayed real data through `jev-1.13.0`. Scripts: `docs/features/001-driver/ctx-jev-replay/` at `2b27832`, in git history. Total spend: about $0.08 for 581 calls.

### Wire facts from this machine

| Item | Measured |
|---|---|
| Latency, 394-token state, 7 calls | min 243 ms, median 285 ms, max 497 ms |
| Latency, 255-line file (8,547 tokens) | 316 ms |
| Latency, whole PRD-001 (11,321 tokens, 21 questions) | 343 ms |
| Latency across 574 replay calls | median 257–315 ms, p90 359–447 ms, max 552 ms |
| Billing | `usage.input_tokens` only; output tokens reported, free |
| Cost per PRD-sized call | $0.00048 |

The "about 100 ms" claim is a West Coast number. From here it is about 300 ms, flat with state size.

### Replay 1: block auto-answer (`blocks.py`)

Every `AskUserQuestion` in the transcripts: 228 calls, 598 questions, 472 with a matched owner answer (126 were free-text "Other" or unparseable). State: the last three assistant texts (≤6,000 chars), the owner's last message, the questions and options. One Choice per question, all questions of a call in one request.

| Variant | Jev accuracy | "Pick Recommended" baseline | Owner deviated from Recommended | Jev caught |
|---|---|---|---|---|
| Labels as shown | 85.0% | 88.2% (n=431) | 51 | 1 |
| "(Recommended)" cue stripped | 79.9% | 88.2% | 51 | 6 |

Calibration, labels as shown: confidence ≥0.90 covers 66.1% of questions at 89.7% accuracy; ≥0.95 covers 56.1% at 90.9%; ≥0.99 covers 24.2% at 91.2%. 454 calls, 916,724 input tokens, $0.0385.

Reading: Jev never beats the free rule, and it cannot see the cases that matter (the owner overriding the recommendation). An unattended driver that auto-answers blocks on Jev would be wrong on roughly one in ten, and a wrong answer costs a slice. **Dead.** Caveat: most of these blocks are PRD-interview questions, not worker parks inside a run; the context given to Jev was limited to recent prose.

### Replay 2: read compaction (`readfor.py`)

Whole-file reads (`Read` without range, bare `cat`) followed within 15 tool calls by an `Edit`/`MultiEdit` of the same file: 193 pairs found, 120 scored (files of 30–4,000 lines, edit span located uniquely). File split into windows of ≥8 lines (median 21 windows, file median 167 lines / 7,966 chars). State: the worker's task, its last two assistant texts, the file with window ids. One Score per window (not needed / useful context / must read) plus one Choice "which window gets edited first".

| Policy | Edited lines covered | Bytes kept (byte-weighted) |
|---|---|---|
| Keep windows scored ≥0.5 | 70.0% | 63.7% |
| Keep windows scored ≥1.0 | 28.3% | 27.3% |
| Keep windows scored ≥1.5 | 6.7% | 5.3% |
| Top-3 edit windows plus neighbours | 50.8% | 34.1% |
| Whole file (today) | 100% | 100% |

Edit-window Choice: top-1 right 21.7%, top-3 right 45.0%; confidence ≥0.8 on only 6.7% of files, 37.5% right there. 120 calls, 788,801 input tokens, $0.0331.

Reading: to keep the edited lines in 7 of 10 cases, the worker still receives two thirds of the file; every policy that saves real bytes misses the edit most of the time, and confidence does not flag the misses. **Dead as designed.** Caveat: "covers the edited lines" is a necessary condition only; a worker also needs lines it does not edit, so the true coverage requirement is stricter than measured.

### Replay 3: pack trimming (`probe.py`, PRD-001 against an FR-10 slice)

One call, 20 PRD sections (truncated to 1,500 chars each), one Score per section plus one Choice "read first". Jev scored FR-10 as the must-read (1.95, confidence 0.93) and picked it first (0.95); it confidently dropped FR-12 (0.02, conf 0.98) and Sequencing risk (0.04, conf 0.95). 12 of 20 sections came back with confidence below 0.35. Reading: a "drop only what Jev is ≥0.9 sure about" rule would have removed 2 of 20 sections; a deterministic pack rule (the slice's FR section, QA plan, Slices, Verification notes) removes more with no network call. **Not worth a slice.**

### Public-code line pick (`probe.py`)

On a 255-line Python stdlib file, Jev found the `class` line and a `def` line by name, both right, confidence 1.00; the yes/no "does any line define X" returned 0.99 for a real class and 0.01 for a fake one. This is the one thing it did perfectly, and it is the question ripgrep plus tree-sitter already answer deterministically (91.2% of definition greps in the transcripts return exactly one hit).

### Not replayed: gate routing

4,379 gate runs in the transcripts, 414 flagged red by a regex (9.5%); red output median 679 chars, p90 3,291. 368 of the 414 were followed by no edit before the next gate run, and 267 went green with no edit at all, so the regex over-flags or the runs were partial re-runs. Ground truth for "flaky / env / test / code" would need a cleaner detector first. Left as an open question; the log is already small, so the prize is a routing decision, not bytes.


---

## What the completeness critic flagged

Read as data, not as findings. Items 1, 2 and 7 are answered in part by the replays section (latency measured; real lookups tested with the key instead of the Stage 0 oracle).

- Stage 0 offline oracle (no key, no network, half a day by the memo's own estimate) was designed but never run, so the headline claim "no ground truth for code reads" is a choice not to measure, and the 8.2% pool is left unbounded by the one experiment the memo says is free.
- Option C's ceiling (≤2.2%) is computed only from the 404 read→edit pairs; the 1,394 code whole-reads without a following edit (the question-shaped majority, 73–77%) have no byte estimate at all, so the compaction prize for them is unknown rather than bounded.
- Two candidate roles from the brief are missing from the options table: classifying a worker's Bash command to route it to a ctx verb (hook hint) and selecting recon files/outline entries for the context pack, the latter sitting on the 40.3%-of-whole-read-chars pack-doc pool where only dedup was considered.
- No model-agnostic picker row exists (Haiku via `claude -p`/direct API, local cross-encoder, aider PageRank) in the options table, so the memo answers "is Jev worth it" without separating it from "is any picker worth it", despite refuted claim 3 showing Haiku at only 2.5–3.7x Jev's cost.
- The privacy gate treats Jev as a new exposure but never states the existing baseline: all client code already leaves the machine to Anthropic under `claude -p`, and whether ICG/Prokey contracts permit that (or any subprocessor) is not in the memo, so "client consent" is asserted as the gate without evidence of what consent already exists.
- No latency budget is defined for a worker lookup (what added ms per Read is acceptable inside a `claude -p` turn), so "acceptable latency" in the research question has no criterion, and the from-Mexico-City figure is unknown with no plan beyond "once a key exists".
- All corpus sizes are in chars with no tokenizer; the share of whole reads exceeding Jev's 32k-token state cap (hence C's forced fallback rate) and the realistic per-call cost (only a $54.77 cap-bound upper bound exists) are unknown.
- Option E's inputs rest on a gate-failure heuristic with 25–40% measured false positives and on "log alone suffices 7.7%" without a stated labelling method, so both the 2.2 reds/feature and the 7.7% are upper/lower bounds presented as point values.
- The judge scores (A 23, C 17, E 14, D 13) come with no rubric for the 1–10 scale, no judge prompts, and no weighting, so the 2–1 split that decides the headline is not reproducible.
- The Rust dogfood stack has zero transcripts and tree-sitter Rust/PHP/Vue tag coverage for `outline` is never checked, so FR-12's own baseline (against which Jev's increment is judged) is unmeasured for one of three stacks and unverified for the others.
- The re-open trigger "`followup_reads` after ctx ≥30%" has no present-day analogue computed from transcripts (follow-up whole read within N calls after a grep/sed), although the data to compute it exists, so the threshold is arbitrary rather than calibrated.
- Choice option-key length limits, 255-option behaviour on code lines, and determinism across identical calls are all unknown, yet Stage 2's design (node ids as keys, semantic_find shape, shuffle-agreement ≥90%) assumes they hold, with no fallback design if they do not.
