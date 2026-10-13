---
title: "Uncle Bob's quality tools as gate tiers or polish inputs"
state: "open"
added: "2026-10-10"
---

Check github.com/unclebob for CRAP, mutation, DRY and UML analysis. The current multi-language set is `crapper` (CRAP scores), `mutator` (mutation testing) and `dryer` (duplication), all Python, pushed 2026-10. The first two cover Clojure, Java, Go, TypeScript, Rust and Python; `dryer` has no description yet. All three write one snapshot that `uml-viewer` (Clojure/Quil, live class diagrams from EDN) reads. Why: a CRAP or mutation floor could catch the under-tested code polish reviewers miss. Gap: no PHP listed, but Uncle Bob says adding a language is easy, since the tools are built to be extended, so we can contribute PHP support ourselves when we get to it. Older per-language repos: `crap4go`, `mutate4go`, `dry4go`, `crap4java`, `mutate4java`, `dry4java`, `crap4clj`, `clj-mutate`, `dry4clj`, `scrap` (speclj spec refactoring), and `negative-test-experiment` (testing discipline × CRAP, then mutation). Added 2026-10-10, not assessed yet.
