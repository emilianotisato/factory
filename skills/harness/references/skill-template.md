# Generated skill — the shape

`.claude/skills/verify-<app>/SKILL.md`. Everything in `{braces}` is replaced by what the repo actually has; nothing in braces survives generation. The Laravel example is illustrative — a Go CLI or a Kotlin app fills the same seven sections with its own commands (notes at the bottom).

**Several surfaces one driver reaches share one skill.** An admin dashboard and an embedded widget are one Launch, one Doctor, one Cleanup, and two surfaces in the feature map. Split only when the driver itself differs.

---

```markdown
---
name: verify-{app}
description: Drive {App} ({surface}) the way a user does and capture proof — launch, doctor, drive a feature-map entry, evidence, cleanup. Use for Prove tasks, §QA plan rows, "verify it in the app", "prove FR-n", "screenshot the {feature}", "walk the {flow}".
---

# Verify {App}

{One paragraph: what the app is, which surface this skill drives, what it deliberately does not cover (the CLI has its own skill, say).} Read [features/README.md](features/README.md) before driving anything; the feature file is the recipe.

## Launch

- **Scripted drives** boot the app themselves: {Pest browser's `visit()` uses the test server — no separate launch | Playwright's `webServer` in `playwright.config.ts`}.
- **Interactive drives** need a running app: `{composer dev | npm run dev | make run}` — ready when `{the "Server running on" log line | port {8000} answers | GET /up returns 200}`, ~{n}s. Record the PID.
- **Seed**: `{php artisan db:seed --class=RulesQaSeeder}` — {what it creates. "Destructive by design: rebuilds the agent whose key starts with pk_qa_; touches nothing else."}
- **Auth**: `{the seeded admin, or the token the seeder prints, or the magic-link route}`.
- **Isolation**: `{APP_URL / port / DB name / data dir per run}` — {or: "one instance only. Never drive one this run did not start."}
- **Stop**: `{kill the recorded PID | docker compose down}`.

## Doctor

One read-only check. Run it first whenever anything looks off, and again after any failed drive:

```
{curl -sf http://127.0.0.1:8000/up && php artisan about --only=environment && php artisan tinker --execute 'echo App\Models\Agent::where("agent_key","like","pk_qa_%")->count();'}
```

Pass = {process up, port owned by this run, seed present, test user logs in}. Fail = do not drive. Relaunch or fix, then doctor again.

**Build freshness is part of Doctor, not an afterthought.** {`public/build/manifest.json` is older than the newest file under `resources/js` → the drive would exercise stale JavaScript and pass. Nothing in `composer ci:check` rebuilds it. Warn loudly, and say the fix: `npm run build`.} A drive against a stale bundle is a green result that proves nothing, and it looks exactly like a real pass.

**Isolation.** {Refuse to drive while another run's process holds the port or the test database — check first, name the PID, stop. That is the check working, not a bug.}

## Drive

**Scripted** — repeatable, lands in the gate. {Pest browser, tests in `tests/Browser`, run one with `php artisan test --compact tests/Browser/{File}.php`:}

```php
{visit("/agents/{$agent->id}/edit/playbook")
    ->click('New rule')
    ->fill('Name', 'Pricing nudge')
    ->click('Save rule')
    ->assertSee('Pricing nudge')
    ->assertNoJavaScriptErrors()
    ->screenshot('rules-create');}
```

{The escape hatch when the plugin's API is not enough: `playwrightPage($page)->waitForFunction(...)` in `tests/Pest.php:197`. Existing examples: `tests/Browser/ProactiveOpenTest.php`.}

**Interactive** — the agent's hands for a Prove task or a UX pass. `terminal-browser`:

```
terminal-browser open {http://127.0.0.1:8000/login} --split right
terminal-browser action -- snapshot                       # ARIA tree with @eN refs
terminal-browser action -- click @e14
terminal-browser action -- fill @e3 "Pricing nudge"
terminal-browser action -- screenshot artifacts/{run}/rules-create.png
```

Stable handles in this app: {ARIA names on buttons and links, `data-testid` on table rows, route paths}. Never coordinates, never nth-child. A proof that only used the interactive driver is not repeatable and says so.

## Evidence

Artifacts: `.claude/skills/verify-{app}/artifacts/<run-id>/` — gitignored. Name files by feature id: `rules-create.png`, `rules-create.aria.txt`, `cli-search.txt`.

**Rescue artifacts out of the driver's own directory immediately.** {Pest browser writes to `tests/Browser/Screenshots` and clears it when the next browser suite boots — a screenshot left there is gone by the next run. `bin/drive.sh` copies them across the moment the drive returns.} Never cite a path the driver owns as the evidence location.

Proof standards. A proof that skips one is incomplete:

- Exercise the real user path. No internal setters, no test-only endpoints, no writing to storage directly.
- Capture the action **and** the resulting state, not only the final screen.
- Read side effects back through a second user-facing path: reopen from the list, a second GET, the CLI's own read command.
- Mock only where a production boundary already isolates the external system ({the AI provider, Firecrawl, Postmark}).
- A dry-run or test mode is verified by observing what it actually skips (files, network, git refs), never by trusting its name.
- UI proof = an ARIA snapshot plus a screenshot with the app identity visible. CLI proof = command, stdout, stderr, exit code. Mutation proof = a read-only second view of the stored value.

Record with every artifact: the feature id, the entry point used, the run id. An unreachable path is reported with the attempted command and the unmet precondition — never as verified through a different path.

## Cleanup

- Stop what this run started — by **recorded PID or session id**. Never `pkill -f '{vendor/bin/pest}'`: that pattern matches the shell running the script, kills it mid-way with exit 144, and everything after it is silently skipped. A bracketed pattern (`'[v]endor/bin/pest'`) is the safe form when no PID was recorded.
- **Absolute paths in every destructive command.** The shell's working directory persists between commands, so a relative `rm -rf artifacts/...` silently hits the wrong folder or nothing at all, and reports success either way.
- Restore seeded data: `{re-run the seeder | delete rows the run created}`.
- Remove scaffolding this run created ({the testbed page, the sample config}), marked as such when it was made.
- **Artifacts stay.** Cleanup never deletes evidence.

## Constraints

The facts that cost an hour each if you do not know them. **This section is why the file is worth reading.**

- **{The widget renders inside a shadow root.}** {`assertSee` fails on text plainly visible on screen, and light-DOM `.ca-*` queries return zero. Every widget assertion goes through `playwrightPage($page)->waitForFunction(...)` reaching into `window.__codyAgentRoot`. This is the only assertion path that works there — not a fallback.}
- **{Screenshots are wiped when the next browser suite boots.}** {`bin/drive.sh` rescues them immediately; never leave evidence in `tests/Browser/Screenshots`.}
- **{Nothing in the gate rebuilds the admin bundle.}** {Scripted drives run whatever was last built. Doctor warns; `npm run build` fixes it.}
- {The endpoint that needs a warm cache, the seeder that must run before the migration, the port the dev server steals.}

## Helpers

{`bin/drive.sh` — runs one browser test and rescues its artifacts. `bin/doctor.sh`. Executable, invocation shown above. Or: "none".}
```

---

## Other surfaces, same seven sections

- **CLI / TUI.** Launch = build once (`go build -o /tmp/{app} ./cmd/{app}`, `cargo build`), then each drive in its own subprocess or `tmux new -d -s verify-{run}`. Doctor = `{app} --version` and the exit code. Drive = the exact command line with its flags; TUIs get `tmux send-keys` then `tmux capture-pane -p`. Evidence = a transcript file: command, stdout, stderr, exit code. Cleanup = `tmux kill-session -t verify-{run}` and the temp dir.
- **API.** Launch as web. Drive = `curl -s -o body.json -w '%{http_code}' -H 'Authorization: Bearer …'`; the gate's HTTP test client for the scripted version. Read-back = a second GET. Evidence = request, status, body, read-back body.
- **Mobile.** Launch = boot the emulator/simulator, install the debug build, note the device id. Drive = the repo's UI tests (Espresso, XCUITest, `integration_test`) or a Maestro flow (`maestro test flows/{feature}.yaml`). Evidence = `adb exec-out screencap -p > file.png` / `xcrun simctl io booted screenshot file.png`, plus the flow's own log. Cleanup = uninstall the build, keep the device.
- **Desktop (Electron, Tauri).** Playwright's Electron driver or the repo's WebDriver setup for scripted; `terminal-browser` cannot attach to a native window, so the interactive driver is the app's own devtools protocol port when it exposes one.
