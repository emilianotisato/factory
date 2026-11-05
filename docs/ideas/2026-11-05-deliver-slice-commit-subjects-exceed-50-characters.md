---
title: "/deliver slice commit subjects exceed 50 characters"
state: "open"
added: "2026-11-05"
origin: "factory upstream"
---

The driver writes slice commits as `feat(NNN): slice-NN <slice title>`. With ordinary slice titles this reaches 53 to 73 characters, for example `feat(011): slice-03 Certification pages, last of FR-3`. Owners who follow the common 50-character subject rule (and Conventional Commits tooling that warns on it) get history a worker cannot fix after it is published; a local review then flags it on every feature.

Should: the commit template cuts the title to fit a 50-character subject (or a configurable limit), and moves the full slice title into the body.
