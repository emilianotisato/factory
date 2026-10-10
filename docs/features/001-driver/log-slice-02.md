## Traps

_(none new — the two from the first pass are in agent-log.md)_

## Blockers

_(none)_

## Decisions

### AD-205 — Does `ask_opened` keep its `source` field once `judge[]` no longer parks? · slice-02 · 2026-10-09

**Chose:** drop it — `ask_opened` is `{id: "ASK-n", text}`; a `judge[]` line is its own kind, `judge_noted {text}`
**Over:** keeping `source: "ask"` on every `ask_opened`, a constant that AD-204 added only so the cockpit could tell the two apart
**Because:** BLOCK-201's resolution routes the two lists to two event kinds, so the kind carries what `source` did; slices 04 and 10 read `ask_opened` and now see one shape
**Undo:** one field in `src/events.rs::EventKind::AskOpened` and its readers
**Review:** no
