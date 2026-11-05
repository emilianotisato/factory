---
title: "/polish UX pass: no shared checkout with the fixer, and start from checkpoint evidence"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

Two problems with the UX collector in `/polish` § 1:

- **It drives the app from the same checkout the fixer edits.** The skill forbids reviewers from moving HEAD but says nothing about the fixer editing files a reviewer is building from. In one run the UX collector rebuilt the stack mid-pass and reported the polisher's own in-progress edits as "someone else's changes".
- **It re-captures states the checkpoints already captured.** The checkpoints' Prove tasks held screenshots of every state. Telling the UX pass to start from the evidence directory and drive only the missing states saved most of its time (~8 min instead of much longer); the skill does not say to.

Should: give the UX collector its own worktree (or make § 5 fixes wait until it finishes), and tell it to start from the checkpoint evidence directory and drive only what is missing.
