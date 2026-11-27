---
title: "grep hook def::find still searches linguist-generated bundles"
state: "open"
added: "2026-11-27"
origin: "PRD-030 polish"
---

Grep hook still walks generated bundle files via def::find while refs::find filters gitattributes.
Recon: hook prints both refs and def output; def::find has no linguist skip.
