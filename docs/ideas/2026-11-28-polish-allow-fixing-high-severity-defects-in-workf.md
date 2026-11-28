---
title: "polish: allow fixing high-severity defects in workflow files added by the feature"
state: "open"
added: "2026-11-28"
origin: "harvest 2026-11-28"
---

In PRD-016 polish, second opinion flagged a high-severity defect (BSD `sed -i` breaking macOS legs) in `.github/workflows/edge.yml` — a workflow file added by PRD-016 itself. Because Autonomy Never forbids workers from editing `.github/workflows/**`, polish had to park the bug instead of fixing it.

**Shape:** allow polish to fix high-severity defects in workflow files that were added or modified by the feature diff itself (`git diff <base>...HEAD`), while still forbidding edits to untouched CI workflows or changes that relax checks.
