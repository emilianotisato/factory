---
name: lanes
description: Interview a repo into its lanes — the ignored state each lane worktree is seeded with (`[lanes] seed` in docs/project.toml) and one docs/reference/lanes/lane-N.md per lane with literal env values (its database, ports, cache, queue, Redis DB, scratch paths) and a setup that creates and migrates the lane's database — then prove it with `factory lanes up --prove` and do not finish until every lane is green. Any stack. Use when a `factory run` or `factory lanes up` refuses with "no lanes declared — /lanes", when the lane count changes, or when the user says "lanes", "set up lanes", "parallel lanes", "isolate the lanes", "lanes are red".
---

# Lanes — every lane its own world, proven at once

A `factory` run builds slices in parallel, each in a lane: a git worktree of its own under the factory data dir, at `<data dir>/lanes/<repo>/lane-N`. The worktree separates the files. Everything else a test touches is still shared unless the lane says otherwise: the database, a port, the cache, a queue, a Redis DB, a scratch path outside the tree. This skill finds every one of those and gives each lane its own, with literal values, then runs the inner loop in every lane at once to prove nothing collides.

**The proof is the command, never a story.** `factory lanes up --prove` creates the lanes, seeds them, runs each lane's setup, then runs the project's inner loop in every lane at the same time, each with its own env. It prints one line per lane and exits nonzero when any lane is red. This skill ends on that command green. A red lane is this skill's job to fix, not a note for later.

**Attended work.** This is an interview. A `factory` run refuses to start without `[lanes] seed` and names this skill. A run's workers never write `docs/project.toml`, so a run never sets up its own lanes.

## 0. Preflight

```
factory docs doctor
factory docs gate --inner
factory docs lanes
```

- **The `skills` line** → apply [_shared/stale-tooling.md](../_shared/stale-tooling.md).
- **Not on the framework** → `factory docs init` first. The seed list lives in `docs/project.toml`.
- **No inner loop and no gate** → stop and say `/gate` comes first. `--prove` runs the inner loop, so without one there is nothing to prove.
- **The inner loop is red in the main tree** → stop and say so. A lane cannot be greener than the tree it is cut from.
- **`factory docs lanes`** says how many lanes are declared and which lane files exist. Existing files are a start, not an answer: read what they say before you propose anything.

## 1. Read the stack — list what two processes would share

Run two copies of the inner loop in your head, side by side, and list every place they would meet. Read, do not guess:

- **The database**: `.env`, `.env.example`, `config/database.php`, `docker-compose.yml`, `DATABASE_URL`, the test config (`phpunit.xml`, `pytest.ini`, `jest.config.*`). Does the suite use its own database or the dev one? In-memory SQLite needs nothing; a server database needs one per lane.
- **Ports**: a dev server, a websocket server, a browser-test server, a mock server the suite starts. Any fixed port collides.
- **Cache, queue, Redis**: the driver and the DB index. Two lanes on Redis DB 0 flush each other's keys.
- **Scratch paths**: a fixed `/tmp/app-test`, an upload dir, a log file, a socket path outside the repo.
- **Build output**: `target/`, `node_modules/.cache`, `.next/`, `vendor/`, `bootstrap/cache`. Inside the worktree these are already the lane's own.
- **Heavy ignored state the lane needs to run at all**: `vendor/`, `node_modules/`, `.env`, a downloaded model, a built frontend. A fresh worktree has none of it, because git does not track it.

Then measure one thing: how long the inner loop takes alone. The `--prove` run takes about that long per lane, all at once.

**Report the list before you ask anything.** One line each: what is shared, where it is configured, what a lane would need instead.

## 2. Propose — the seed list and each lane's values

**AskUserQuestion**, recommendation first, cost stated.

**Round 1 — the seed list.** The ignored paths cloned from the main tree into each lane once, when the lane is made: `.env`, `vendor`, `node_modules`, and whatever heavy state §1 found. A clone is copy-on-write where the filesystem allows it (reflink on Linux, clonefile on macOS), a plain copy otherwise. A seed path the lane already has is kept as it is, so the lane's own build state survives. *(Recommended: the dependency dirs and `.env`. Leave a build output out when the inner loop rebuilds it fast; seeding a 40 GB `target/` costs disk on a filesystem without reflink.)*

**Secrets stay in `.env`, which is seeded.** A lane file is committed, so it never holds a secret: it holds only what differs per lane.

**Round 2 — each lane's values.** For every shared thing in §1, one env key per lane with a **literal** value: `DB_DATABASE=app_lane_2`, `APP_PORT=8002`, `REDIS_DB=2`, `CACHE_PREFIX=lane2_`. Prefer a key the app already reads over a new one. When the suite reads its config from a file and not the env, name what reads the key and say whether the file needs a change; that change is part of this skill's work.

**Round 3 — the setup.** One shell command per lane, run inside the lane, safe to run twice: create the lane's database if it is missing, then migrate it. *(e.g. `createdb app_lane_2 2>/dev/null; php artisan migrate --force`.)* The lane's env is set when it runs. No setup when the lane needs none.

A value that differs per developer's machine (a local socket path, a personal port range) is out of scope: the files hold committed literal values only.

## 3. Write

The seed list goes into `docs/project.toml`:

```toml
[lanes]
seed = [".env", "vendor", "node_modules"]
```

`seed = []` is a real answer: the lanes need nothing seeded.

The lane files, one command for all of them; `{n}` becomes the lane number when each file is written, so the files hold literal values:

```
factory docs lanes set 3 --env "DB_DATABASE=app_lane_{n},REDIS_DB={n}" --setup "createdb app_lane_{n} 2>/dev/null; php artisan migrate --force"
```

It writes `docs/reference/lanes/lane-1.md` to `lane-3.md` and sets `lanes = 3`. It never overwrites a lane file that exists: edit that file's frontmatter by hand, or delete it and run the command again.

**A `setup:` value loses a leading or trailing quote** when the lane file is read. End the command on something unquoted.

## 4. Prove

```
factory lanes up --prove
```

One line per lane: its path and how each seed path got there, then its setup exit; then `lane-N  inner  exit <code>  <secs>s` for every lane, with a red lane's output tail under its line, and a last line naming the red lanes.

- **Every lane `exit 0`** → done. Go to §5.
- **A setup is red** → the setup command is wrong for that lane. Fix the lane file and run it again.
- **A lane is red and the main tree is green** → two lanes still share something §1 missed, or a value did not reach the code that reads it. Read the red lane's output, find the shared thing, fix the lane files (or the config that ignores the env), and run it again.
- **A seed path is wrong** (a lane missing `vendor/`, a stale `.env`) → fix `[lanes] seed`. A lane keeps a seed path it already has, so `factory lanes prune --all` first and the next `up` seeds every lane fresh.

Do not finish on red. Three rounds on the same red with no progress → stop and report what you found to the owner, with the red lane's output.

## 5. Commit

`docs/project.toml`, the lane files, and any config change §2 needed, in one commit: `chore: lanes`. Report the `--prove` lines: each lane's exit and duration.

`factory lanes prune` removes the lanes above the declared count when it shrinks; `--all` removes every lane of this repo. Both refuse while a run of this repo is unfinished.
