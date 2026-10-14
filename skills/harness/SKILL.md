---
name: harness
description: Build, or maintain, the project's harness — a project-local verification skill that drives the real app the way a user does and captures proof: .claude/skills/verify-<app>/ with Launch / Doctor / Drive / Evidence / Cleanup / Constraints plus a feature map. Any stack — Laravel via Pest browser, other web via Playwright, CLI, API, mobile. Use when a PRD's §QA plan says "harness: none", for a feature's harness slice, or when the user says "harness", "verify skill", "how do agents drive the app", "prove it in the app", "maintain the verify skill", "the feature map is stale".
argument-hint: [maintain]
---

# Harness — the app, driven the way a user drives it

Every project on the framework needs one scripted way to prove behavior on the real surface: launch the app, exercise a feature the way a user would, capture evidence, tear down. This skill writes that as a **project-local skill** — `.claude/skills/verify-<app>/` — so "prove FR-97 in the app" becomes a step any agent can execute in this repo with no setup conversation. The PRD's §QA plan names it, every slice's Prove task cites it, and in a `factory` run the checkpoint worker drives it.

You are writing for the next agent, not for a human. It reads the output cold, mid-task, never having seen the app. Every section is grounded in what this repo actually has. **No placeholder survives generation.**

`$ARGUMENTS` = `maintain` → go straight to [references/maintain.md](references/maintain.md). Route there too when `.claude/skills/verify-*/` already exists and the user did not ask for a second surface — two harnesses for one app is drift waiting to happen.

## 0. Preflight

- `factory docs gate` — the gate is the Doctor section's first check and the Verify task's command. Know it before writing. None declared → say so; the harness still gets built, but mention `/gate`.
- `ls .claude/skills/verify-*` — exists → maintain mode, unless the user is explicit about a new surface.

**One skill per *driver*, not per surface.** Two surfaces one driver reaches — an admin dashboard and an embedded widget, both in a browser — are **one skill with two surfaces in its feature map**. A second skill is warranted only when the driver genuinely differs: a web app plus a CLI, a service plus a mobile app. Splitting on surface alone duplicates Launch, Doctor and Cleanup three ways and they drift apart within a month.
- **Does the checkout build and start as-is?** If not, fix that first or report it precisely, and stop. A skill written against a broken base teaches wrong steps. An irrelevant missing asset that blocks startup (a static dir the API never serves, a sample config) may be created, marked as verification scaffolding, and removed in Cleanup.

## 1. Interview the repo, not the user

Answer these from the code. Ask only what the code cannot tell you, in **one AskUserQuestion round**, with your findings pre-filled as the recommended option.

- **Surface.** What does a user actually touch? Web UI, CLI/TUI, API, desktop, mobile, library. Read routes (`php artisan route:list`, `routes/*.php`, the router file), commands (`php artisan list`, `cmd/`, `bin/`), menus, the README. List **every** surface one driver can reach; they all go in the map.
- **What defeats the obvious assertion.** Probe this now, in a five-minute spike, rather than discovering it in §5 after writing a map against it. **Shadow DOM** (a web component, an embedded widget) makes text and CSS assertions silently return nothing — the text is plainly on screen and `assertSee` fails. Same for **iframes**, **canvas-rendered UI**, **virtualised lists** that only render what is visible, and **portals** that mount outside the component tree. Each needs a different assertion path, usually the underlying driver's own API rather than the convenience wrapper. Find the escape hatch and write it into Drive as *the* way to assert on that surface, not as a footnote.
- **Launch.** The repo's own dev command, never one you invent: `composer dev`, `npm run dev`, `make run`, `go run ./cmd/…`, `docker compose up`, `mise run dev`. Ports, env vars, the readiness signal (a log line, a port answering, a health route). **Seed data**: existing seeders, fixtures, factories — `*QaSeeder` classes are exactly this, and their output is the map's preconditions. **Auth**: how a test user gets in.
- **Drive.** What already exists comes first: a Pest browser suite (`tests/Browser`, `pestphp/pest-plugin-browser`), Playwright or Cypress specs, a CLI test harness, curl-able endpoints, a debug port. Then the recipe for the stack in [references/drivers.md](references/drivers.md), and the install offer when nothing is there (§2).
- **Observe.** What proves it: screenshots, ARIA snapshots, terminal transcripts, response bodies, exit codes, a read-back of the stored value through a second user-facing path. **Find out where the driver puts artifacts by default and whether it clears that directory on the next run** — several do. Evidence that a later suite wipes is not evidence.
- **Build freshness.** When the app serves a compiled bundle, find out whether anything in the normal flow actually rebuilds it. Often nothing does: the gate runs tests against whatever was last built, so a scripted drive exercises stale JavaScript and passes. **This belongs in Doctor as a hard check**, comparing the built artifact's timestamp against its sources.
- **Isolate.** Can two instances run side by side (port, data dir, DB name, browser profile)? If not, the generated skill says so and refuses to double-drive a shared instance — refusing beats corrupting the owner's session. Inside one `factory` run only the checkpoint worker drives, one checkpoint at a time, so the collision to design for is a second run or the owner's own instance: two A/B runs of the same PRD had harness stacks on fixed ports, and the first had to be stopped before the second could drive.

## 2. The driver — push for the real one

**The harness is a dependency of proof, so a missing driver is installed, not worked around.** Propose the install as option 1 *(Recommended)* with its exact commands and cost. Take no for an answer **once**, then take the fallback and say which. A test-framework **major** upgrade the PRD does not mention — Pest below 4, say — touches the whole suite, so it is its own decision, never a side effect of this one.

- **Laravel with no Pest browser plugin** → `composer require --dev pestphp/pest-plugin-browser` on the matching major (`^4` for Pest 4, `^5` for Pest 5) plus `npx playwright install chromium`. **Pest below 4** → the Pest major upgrade comes first: PHPUnit major, every `pestphp/*` plugin to the matching major, `tests/Pest.php` changes, and the full suite green *before* any harness is built on it. Say what it touches and roughly how long. Precedent when it helps: cody-agent's PRD-011 made this exact call as its first slice.
- **Other web with no Playwright** → `npm i -D @playwright/test && npx playwright install chromium`. Cypress already present is fine; never add a second runner.
- **CLI / TUI, API** → nothing to install. A subprocess with captured stdout/stderr/exit code; `tmux` for TUIs; `curl` for APIs.
- **Mobile** → the repo's own UI tests; Maestro if none, proposed the same way.
- **Interactive driving on this machine** — `terminal-browser`, when `command -v terminal-browser` succeeds, drives any open page from the terminal (`open`, `action -- snapshot | click @eN | fill @eN "…" | eval | screenshot`). It is the agent's hands for a Prove task and for a UX pass. It is **not a test suite**: the generated skill lists it beside the scripted driver, never instead of it.

**Refused** → Playwright standalone for Laravel (repeatable, but outside the PHP gate), or `terminal-browser` only (interactive, not repeatable). The generated skill's Drive section states plainly which proof standard was traded away. Never silently.

## 3. Generate the skill

Write `.claude/skills/verify-<app>/SKILL.md` per [references/skill-template.md](references/skill-template.md). Frontmatter first — `name: verify-<app>` and a `description` naming the app, the surface and when to reach for it; without it the skill never registers. Then exactly these sections, each grounded in what §1 found:

- **Launch** — the command, the readiness check, the seed command, the test user, the isolation rule, teardown. For a short-lived CLI: build once, then each drive in its own isolated session.
- **Doctor** — one read-only check answering "is this instance worth driving?": process up, port owned by this run, auth valid, seed present, **and the build fresh**. Stale-build detection is not optional where the app serves compiled assets: a drive against a four-day-old bundle passes and proves nothing. Doctor runs first, and again after any failed drive.
- **Constraints** — the two or three facts that will otherwise cost the next agent an hour each: the assertion path that does not work and the one that does, the artifact directory something else clears, the build nothing rebuilds, the endpoint that needs a warm cache. **This is the most valuable section in the file.** Everything else is recoverable by reading the repo; these are not.
- **Drive** — the scripted recipe (a Pest browser test skeleton, a Playwright spec skeleton, the subprocess call) **with real routes, selectors and commands from this repo**, then the interactive recipe. Verify every helper name against the installed driver version before writing it. Prefer stable handles: ARIA roles and names, `data-testid`, route paths, prompt strings. Never coordinates, never DOM position.
- **Evidence** — what to capture and where. Default: `.claude/skills/verify-<app>/artifacts/<run-id>/`, **gitignored** — add the line. The proof standards, verbatim from the template.
- **Cleanup** — kill what you started, by recorded PID or session. **Never `pkill -f <pattern>`**: the pattern matches the shell running the script, which kills the script mid-way (exit 144) and silently skips everything after it. Use a recorded PID, or a bracketed pattern that cannot match itself. **Absolute paths only** — a shell's working directory persists between commands, so a relative `rm` in cleanup quietly targets the wrong folder or nothing at all. Restore seeded data, remove scaffolding, and **evidence survives cleanup** at the named location.
- **Helpers** — any script the skill ships is executable and its invocation appears in the body. A helper the reader has to reverse-engineer is not a helper.

**Mirror.** If the project mirrors project skills for another agent (`.agents/skills/`, `.codex/skills/`, `.cursor/rules/`), mirror this folder the way the existing entries are mirrored — copy or symlink, look, don't assume.

## 4. Seed the feature map

`.claude/skills/verify-<app>/features/README.md` plus one file per user-facing feature — the top 3-5 to start, from routes, commands, menus, docs. Shape in [references/feature-map.md](references/feature-map.md): an H1, one paragraph, then exactly four H2s — `Sub-features`, `How to get to it (user POV)`, `Driving it with <driver>`, `Gotchas`.

The map is the repo's maintained verification source. A Prove task cites a feature file; a proof that drives one convenient entry point is incomplete when the map lists others. **Existing QA seeders are the preconditions and existing browser tests are the first Driving bullets** — lift them, do not re-derive them.

## 5. Prove the skill before handing it over

Run its own instructions once, end to end: launch → doctor → drive **one** mapped feature → capture evidence → cleanup → **confirm the evidence still exists** at the named path. One feature is enough; the map exists so later runs cover the rest. Fix what fails, and run the generated Cleanup after every failed iteration so broken attempts do not strand ports and processes.

**Every footgun you hit during this run goes into Constraints or a helper before you finish.** You are the first user of the file; whatever cost you twenty minutes will cost the next agent the same, and it is fresh only now.

**A generated skill that was never executed is a draft, not a deliverable.** Say which feature you drove and paste the evidence path.

## 6. Register and hand off

- The current PRD's **§QA plan** Harness line → `.claude/skills/verify-<app>/` *(exists)*. When this ran as the feature's harness slice, the slice's Progress carries the §5 proof and the slice is `done`.
- Commit the skill folder and the `.gitignore` line. Artifacts never commit.
- Report: surfaces, driver chosen and what was refused if anything, features mapped, the one drive that proved it and its evidence path, **the constraints you found**, and what a human still has to judge.
- **Never invent proof rows for work you did not drive.** When the current feature's FRs were not exercised through the new harness, the §QA plan says so in those words. A harness that shipped is not proof that the feature works.
- Point at `/harness maintain` for when the app moves. Suggest a cadence only if asked.
