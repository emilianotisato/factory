---
title: "/polish second opinion: send the feature's decisions log with each pass"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

`/polish` § 1 sends the second-opinion reviewer the diff without the feature's decisions log. In one project, about a third of the reviewer's findings across several features re-argued choices already logged as `AD` entries, and each one cost the polisher a verification at `file:line` to rebut. § 2 already names this as the most common false-positive class.

Should: pass `factory log show --decisions` (or the decisions section of the plan) with each pass, with an instruction that a finding contradicting a logged decision must name the decision and say why it is wrong.
