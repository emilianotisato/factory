# PRD-012 QA guide — the one-tap build

**Budget: 5 minutes, on the next PRD you agree after the merge.** One check, and it is the one that matters: it is the whole point of the feature.

## 1. Build / Slice / Stop arrives on the phone — 5 min — matters most

Source: prd §3 FR-1, §11 row FR-1.

1. Run `/prd` on your next feature and agree it.
2. After the PRD is committed and set `ready`, expect **one** question with three options: **Build it now (Recommended)**, **Slice only**, **Stop here**.
3. Check that the question reaches your phone as a notification.
4. Pick **Build it now**.

**Pass:** the session invokes `/deliver NNN` itself, with no slash command typed by you, and the run starts. No line tells you to `/clear`.

**Fail, note which:** no question; a question with other options; Build asks you to type `/deliver`; no phone notification.
