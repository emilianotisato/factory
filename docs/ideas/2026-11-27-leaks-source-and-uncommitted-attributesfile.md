---
title: "leaks --source and uncommitted attributesFile"
state: "open"
added: "2026-11-27"
origin: "PRD-030 polish"
---

leaks --source uses git check-attr at base; core.attributesFile in local git config may still hide hits without committing .gitattributes.
Mirror odd_paths_and_the_local_git_config_hide_no_hit for --source scans.
