# Drivers — per surface, with the install path

**The harness is a dependency of proof.** A missing driver is installed, not worked around. Propose the install as the recommended option with exact commands and cost. One refusal is enough; then take the fallback and write into the generated skill which proof standard was traded away.

| Surface | Detect by | Scripted driver (repeatable, lands in the gate) | Interactive driver (the agent's hands) | When missing |
|---|---|---|---|---|
| Laravel web | `artisan`, `tests/Browser`, `pestphp/pest-plugin-browser` in `composer.json` | **Pest browser** — `visit()`, `->click()`, `->fill()`, `->assertSee()`, `->screenshot()`, `->assertNoJavaScriptErrors()` | `terminal-browser` | the pushy path below |
| Other web (Node, Go, Python, Rails, …) | `playwright.config.*`, `cypress.config.*`, `package.json` | **Playwright** (`@playwright/test`); Cypress when already there | `terminal-browser` | `npm i -D @playwright/test && npx playwright install chromium` |
| CLI / TUI | `cmd/`, `bin/`, `main.go`, `src/main.rs`, artisan commands | subprocess with captured stdout/stderr/exit; `tmux` for TUIs | the same | nothing |
| API | `routes/api.php`, an OpenAPI file, a router | `curl -s -o body -w '%{http_code}'`; the repo's HTTP test client for the gate | `curl` | nothing |
| Mobile (Kotlin, Swift, Flutter) | `build.gradle*`, `*.xcodeproj`, `pubspec.yaml` | the repo's UI tests (Espresso, XCUITest, `integration_test`); **Maestro** flows when none | `adb` / `xcrun simctl` + screenshots | `curl -Ls https://get.maestro.mobile.dev \| bash` |
| Desktop (Electron, Tauri) | `electron` / `tauri` in deps | Playwright's Electron driver, or the repo's WebDriver setup | the app's devtools port, if exposed | as web |

## Laravel — the pushy path

1. **Plugin present, tests exist** → use them. Lift the existing `visit()` calls and helpers (an escape hatch like `playwrightPage()` in `tests/Pest.php`) into the Drive section. Do not re-derive what the suite already knows.
2. **Pest 4 or 5, no plugin** → propose, recommended:

   ```
   composer require --dev pestphp/pest-plugin-browser:^5.0   # ^4.0 on Pest 4
   npx playwright install chromium
   php artisan test --compact tests/Browser                  # after the first spec exists
   ```

   Cost: one dev dependency, a Chromium download (~150 MB), a `Browser` testsuite in `phpunit.xml` and `->in('Browser')` in `tests/Pest.php` when absent, and the Browser suite added to the gate.
3. **Pest below 4** → the upgrade comes first and it is bigger: the PHPUnit major, every `pestphp/*` plugin to the matching major, `tests/Pest.php` API changes, then the full suite green **before** any harness is built on it. Say so with a size estimate. Precedent: cody-agent's PRD-011 did Pest 4 → 5 as its first slice for exactly this reason.
4. **Refused** → Playwright standalone against `php artisan serve` (still repeatable, but outside the PHP gate — the size of that gap goes in the Drive section), or `terminal-browser` only (interactive; no repeatable test). Never silently.

Verify each helper name against the installed plugin major before writing it into Drive — assertion names moved between majors.

### Three constraints, found on a real Laravel 13 / Pest 5 repo (2026-09-06)

- **Shadow DOM defeats the plugin's assertions entirely.** A widget rendering into a shadow root is invisible to `assertSee` and to every light-DOM selector: the text is plainly on screen and the assertion fails. `playwrightPage($page)->waitForFunction(...)`, reaching through the host element the app exposes on `window`, is **the** assertion path there, not a fallback. Probe for a shadow root during §1, before writing a map against selectors that cannot work.
- **`tests/Browser/Screenshots` is cleared when the next browser suite boots.** Evidence left there is gone by the next run. Ship a helper that copies artifacts into the skill's own directory the moment a drive returns, and never cite the driver's own directory as the evidence path.
- **Nothing in a typical Laravel gate rebuilds the frontend bundle.** The gate runs tests against whatever was last built, so a scripted drive can exercise a days-old bundle and pass — a green result proving nothing, indistinguishable from a real one. Compare `public/build/manifest.json` against the newest source file in Doctor and warn.

## Non-Laravel web

Playwright over Cypress when starting fresh: one binary, ARIA-first locators (`getByRole`), a trace viewer for evidence. Keep Cypress when the repo already runs it — a second browser runner is drift.

## `terminal-browser` — the interactive driver on this machine

Present when `command -v terminal-browser` succeeds. Drives any open page from the terminal, side by side with the conversation:

```
terminal-browser open http://127.0.0.1:8000 --split right
terminal-browser action -- snapshot                      # ARIA tree with @eN refs
terminal-browser action -- click @e14
terminal-browser action -- fill @e3 "Pricing nudge"
terminal-browser action -- eval "document.title"
terminal-browser action -- screenshot artifacts/<run>/state.png
terminal-browser action -- help                          # the full agent-browser command list
```

It is the agent's hands for a Prove task and for a UX pass. It is not a test suite: a proof that only used it is not repeatable and says so. The generated skill lists it beside the scripted driver, never instead of it, and omits it entirely when the binary is absent on the machine that will run the skill.

## Evidence per driver

- **Pest browser**: `->screenshot('name')` lands in `tests/Browser/Screenshots` by default — point it at the artifacts dir or copy it there; ARIA via the Playwright page (`playwrightPage($page)`) when the plugin has no snapshot helper; JS and console errors via `assertNoJavaScriptErrors()` / `assertNoConsoleLogs()`.
- **Playwright**: `page.screenshot({ path })`, `page.locator('body').ariaSnapshot()`, trace on retry.
- **CLI**: one transcript file — the command line, stdout, stderr, exit code.
- **API**: request, status, body, then the read-back GET and its body.
- **Mobile**: `adb exec-out screencap -p > file.png` / `xcrun simctl io booted screenshot file.png`, plus the flow's own log.
