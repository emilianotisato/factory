#!/usr/bin/env node
// docs — deterministic bookkeeping for the feature-docs framework.
//
// Files are truth. docs/index.md is a rendering of them. This asserts they agree.
// Judgment (what to build, how to slice it) is never in here — only what a
// machine can decide: numbering, links, statuses, dependencies, frozen archive.
//
// Zero dependencies, Node 18+. Templates are read from ../templates/ so the
// human-facing copies and the scaffolds can never drift apart.

import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const TEMPLATES = path.join(HERE, '..', 'templates')

const STATUSES = ['idea', 'draft', 'ready', 'in-progress', 'shipped', 'archived', 'retired']
// `blocked` is set only by an unattended run (/deliver) that stopped on this slice;
// its open BLOCK-n entry in the feature's agent-log.md says why.
const SLICE_STATUSES = ['pending', 'building', 'blocked', 'done']
// Numbers /deliver's retro records, so a later scan can compare runs.
const RETRO_KEYS = ['score', 'gate_runs', 'gate_red', 'gate_minutes', 'ci_cycles']
// Slices verified per slice cost more than they caught (measured 2026-09-16): the
// full gate and Prove now run at a checkpoint every N done slices, and before polish.
const CHECKPOINT_EVERY = 2
// Size ceilings are in KB, not lines — these docs run ~115 bytes/line, so a line
// count says nothing about what an agent pays to read one. Calibrated on a real
// tree where the largest healthy PRD was 71KB and the one everyone agreed was a
// mistake was 149KB, so the only thing that fires is a doc already too big.
// Override per project in docs/index.md frontmatter: prd_max_kb / slice_max_kb.
const SIZE_WARN_PRD_KB = 120
const SIZE_WARN_SLICE_KB = 30
const SECOND_BOARD_NAMES = ['ROADMAP.md', 'TASKS.md', 'PLAN.md', 'STATUS.md', 'BACKLOG.md', 'TODO.md']
// Planning conventions this framework replaces. Their presence is not an error —
// only a nudge that a migration was left half-done.
// 'specs' is deliberately absent — too often OpenAPI/protobuf, not planning.
const LEGACY_DOC_DIRS = ['.vibe', 'plans', 'ai_docs', 'docs/prd']

// The framework version this tool implements. A project declares its own in
// docs/index.md frontmatter; no declaration means v1, which predates stamping.
// Bump ONLY when a change makes existing docs invalid — never for guidance edits.
const FRAMEWORK_VERSION = 2

/**
 * What each version requires, and who can close the gap. `doctor` probes these;
 * `check` only enforces the ones at or below the project's declared version, so a
 * v1 project is not buried in errors for a v2 feature it has not adopted yet.
 */
const REQUIREMENTS = [
  { id: 'layout', since: 1, label: 'docs/index.md + features/ + ideas.md', fixBy: 'tool' },
  { id: 'stamp', since: 2, label: 'framework version stamp in index.md frontmatter', fixBy: 'tool' },
  { id: 'frontmatter', since: 2, label: 'frontmatter on live prd.md and slice files', fixBy: 'agent' },
  { id: 'markers', since: 2, label: '<!-- docs:slices --> markers in sliced PRDs', fixBy: 'tool' },
  { id: 'tombstones', since: 2, label: 'tombstone folder for every retired number', fixBy: 'agent' },
]

// ─────────────────────────────────────────────────────────────── frontmatter

/** Minimal YAML: `key: value`, flow arrays, numbers, quoted strings. Nothing else. */
function parseFrontmatter(text) {
  if (!text.startsWith('---\n')) return { data: null, body: text }
  const end = text.indexOf('\n---', 3)
  if (end === -1) return { data: null, body: text }
  const block = text.slice(4, end)
  const body = text.slice(end + 4).replace(/^\n/, '')
  const data = {}
  for (const line of block.split('\n')) {
    if (!line.trim() || line.trimStart().startsWith('#')) continue
    const m = line.match(/^([A-Za-z_][\w-]*):\s*(.*)$/)
    if (!m) return { data: null, body: text, error: `unparseable frontmatter line: ${line.trim()}` }
    data[m[1]] = parseScalar(m[2].trim())
  }
  return { data, body }
}

function parseScalar(raw) {
  if (raw === '') return null
  if (raw.startsWith('[')) {
    const inner = raw.replace(/^\[/, '').replace(/\]$/, '').trim()
    if (!inner) return []
    return inner.split(',').map((t) => parseScalar(t.trim()))
  }
  if (/^["'].*["']$/.test(raw)) return raw.slice(1, -1)
  if (/^-?\d+$/.test(raw)) return Number(raw)
  return raw
}

function stringifyFrontmatter(data) {
  const lines = Object.entries(data)
    .filter(([, v]) => v !== undefined)
    .map(([k, v]) => `${k}: ${Array.isArray(v) ? `[${v.join(', ')}]` : v ?? ''}`)
  return `---\n${lines.join('\n')}\n---\n`
}

// ────────────────────────────────────────────────────────────────── FR ranges

/** `[97, "99-103"]` → Set{97,99,100,101,102,103}. Tolerates an `FR-` prefix. */
function expandFr(list) {
  const out = new Set()
  for (const token of list ?? []) {
    const s = String(token).replace(/FR-?/gi, '').trim()
    const m = s.match(/^(\d+)\s*[-–—]\s*(\d+)$/)
    if (m) {
      for (let i = Number(m[1]); i <= Number(m[2]); i++) out.add(i)
    } else if (/^\d+$/.test(s)) {
      out.add(Number(s))
    }
  }
  return out
}

function formatFr(set) {
  const nums = [...set].sort((a, b) => a - b)
  const parts = []
  let i = 0
  while (i < nums.length) {
    let j = i
    while (j + 1 < nums.length && nums[j + 1] === nums[j] + 1) j++
    parts.push(i === j ? `${nums[i]}` : `${nums[i]}-${nums[j]}`)
    i = j + 1
  }
  return parts
}

// ─────────────────────────────────────────────────────────────────── fs / git

const exists = (p) => fs.existsSync(p)
const read = (p) => fs.readFileSync(p, 'utf8')
const write = (p, s) => {
  fs.mkdirSync(path.dirname(p), { recursive: true })
  fs.writeFileSync(p, s)
}

function git(args, cwd, { trim = true } = {}) {
  try {
    const out = execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })
    return trim ? out.trim() : out
  } catch {
    return null
  }
}

/** Walk up for the project that owns docs/index.md; fall back to cwd. */
function findRoot(start = process.cwd()) {
  let dir = path.resolve(start)
  for (;;) {
    if (exists(path.join(dir, 'docs', 'index.md'))) return dir
    const up = path.dirname(dir)
    if (up === dir) return path.resolve(start)
    dir = up
  }
}

// ────────────────────────────────────────────────────────────────────── model

function scanFolder(base, kind) {
  if (!exists(base)) return { features: [], loose: [] }
  const features = []
  const loose = []
  for (const name of fs.readdirSync(base).sort()) {
    const dir = path.join(base, name)
    const m = name.match(/^(\d{3})-(.+)$/)
    if (!m || !fs.statSync(dir).isDirectory()) {
      loose.push(name)
      continue
    }
    const prdPath = path.join(dir, 'prd.md')
    const feature = {
      kind,
      dirName: name,
      dir,
      number: Number(m[1]),
      slug: m[2],
      prdPath,
      prdExists: exists(prdPath),
      fm: null,
      fmError: null,
      prdLines: 0,
      proseStatus: null,
      slices: [],
    }
    if (feature.prdExists) {
      const text = read(prdPath)
      const parsed = parseFrontmatter(text)
      feature.fm = parsed.data
      feature.fmError = parsed.error ?? null
      feature.prdLines = text.split('\n').length
      feature.prdKb = Buffer.byteLength(text) / 1024
      const ps = text.match(/^\*\*Status\*\*:\s*(.+)$/m)
      feature.proseStatus = ps ? ps[1] : null
      feature.hasQaPlan = /^#{2,3}\s+(?:\d+\.\s*)?QA plan\b/im.test(text)
      feature.harnessLine = text.match(/^\*\*Harness\*\*:\s*(.+)$/m)?.[1] ?? null
      feature.openOpens = openOpens(text)
    }
    // Written by /deliver. Parsed so check can keep them honest and status can resume a run.
    const logPath = path.join(dir, 'agent-log.md')
    feature.log = exists(logPath) ? parseAgentLog(read(logPath)) : null
    const retroPath = path.join(dir, 'retro.md')
    feature.retro = exists(retroPath) ? parseFrontmatter(read(retroPath)) : null
    for (const f of fs.readdirSync(dir).sort()) {
      const sm = f.match(/^slice-(\d+)-(.+)\.md$/)
      if (!sm) continue
      const text = read(path.join(dir, f))
      const parsed = parseFrontmatter(text)
      const title = (text.match(/^#\s+(.+)$/m)?.[1] ?? f).replace(/^Slice\s+\d+\s*[-–—]\s*/i, '')
      // Progress, review fixes and deviations land above **Slice**: as the slice is built.
      // They are record, not spec, so the size ceiling ignores them — a done slice that
      // trips W-SIZE on its own evidence cannot be split, which is the one fix W-SIZE asks for.
      const landed = text.match(/^\*\*Progress\*\*:[\s\S]*?(?=^\*\*Slice\*\*:)/m)?.[0] ?? ''
      feature.slices.push({
        file: f,
        rel: path.join(feature.dirName, f),
        number: Number(sm[1]),
        subject: sm[2],
        title,
        fm: parsed.data,
        fmError: parsed.error ?? null,
        lines: text.split('\n').length,
        kb: Buffer.byteLength(text) / 1024,
        specKb: (Buffer.byteLength(text) - Buffer.byteLength(landed)) / 1024,
      })
    }
    feature.slices.sort((a, b) => a.number - b.number)
    features.push(feature)
  }
  return { features, loose }
}

/**
 * Decision-log bullets whose OPEN is not yet settled. Real PRDs settle one in two
 * ways, both seen in the wild: strike it (`- ~~**OPEN-1** …~~`, `- **~~OPEN-1~~ …**`)
 * or lead the answer with a bold verdict (`- **OPEN-1 — q?** **Closed 2026-09-11:** …`,
 * `**Decided …**`, `**CERRADA**`). Inline mentions are not bullets and never match.
 * Advisory only — whether an open question is load-bearing is judgment.
 */
const SETTLED = /\*\*[^*]{0,25}\b(closed|decided|settled|resolved|superseded|cerrad[ao]|decidid[ao]|resuelt[ao])\b/i

function openOpens(text) {
  const lines = text.split('\n')
  const out = []
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^\s*[-*]\s+(~~)?\*\*(~~)?OPEN-(\d+)/)
    if (!m) continue
    // A bullet runs until a blank line, the next bullet or a heading.
    let block = lines[i]
    for (let j = i + 1; j < lines.length && lines[j].trim() && !/^\s*[-*]\s|^#/.test(lines[j]); j++) block += ` ${lines[j]}`
    // The bold question itself can contain a verdict word, so test what follows the OPEN number.
    const after = block.slice(block.indexOf(`OPEN-${m[3]}`))
    if (!m[1] && !m[2] && !SETTLED.test(after.replace(/^OPEN-\d+[^*]*\*\*/, '**'))) out.push(Number(m[3]))
  }
  return [...new Set(out)]
}

/**
 * `### AD-3 — {question} · slice-02 · 2026-09-16` and
 * `### BLOCK-1 — {what stopped} · ci · open`, and `### CP-1 — after slice-03 · 2026-09-17`.
 * The middle field is `slice-NN` or a phase name; a BLOCK's last field is open|resolved;
 * a CP's title names the last slice it covers. Fenced blocks are skipped so the
 * template's example shapes never count as entries.
 */
function parseAgentLog(text) {
  const entries = []
  let fenced = false
  let current = null
  for (const line of text.split('\n')) {
    if (/^\s*```/.test(line)) fenced = !fenced
    if (fenced) continue
    const m = line.match(/^###\s+(AD|BLOCK|CP)-(\d+)\s+[—–-]\s+(.+)$/)
    if (m) {
      const parts = m[3].split(/\s+·\s+/).map((s) => s.trim())
      const where = parts.slice(1).find((s) => /^slice-\d+$/.test(s) || /^[a-z][a-z-]*$/.test(s)) ?? null
      const last = parts[parts.length - 1]
      current = {
        kind: m[1],
        n: Number(m[2]),
        title: parts[0],
        slice: where && /^slice-\d+$/.test(where) ? Number(where.slice(6)) : m[1] === 'CP' ? Number(parts[0].match(/slice-(\d+)/)?.[1] ?? 0) || null : null,
        phase: where && !/^slice-\d+$/.test(where) && !['open', 'resolved'].includes(where) ? where : null,
        state: m[1] === 'BLOCK' ? (last === 'resolved' ? 'resolved' : 'open') : null,
        review: null,
      }
      entries.push(current)
      continue
    }
    if (/^#{1,2}\s/.test(line)) current = null
    // Review: decide (the owner must answer) · fyi (worth a look) · no. `yes` reads as fyi.
    const rv = current && line.trim().match(/^\*\*Review:?\*\*:?\s*(decide|fyi|yes|no)\b/i)
    if (rv) current.review = { decide: 'decide', fyi: 'fyi', yes: 'fyi', no: null }[rv[1].toLowerCase()]
  }
  return { entries }
}

function loadProject(root) {
  const docs = path.join(root, 'docs')
  const live = scanFolder(path.join(docs, 'features'), 'live')
  const arch = scanFolder(path.join(docs, 'archive'), 'archive')
  const indexPath = path.join(docs, 'index.md')
  const indexText = exists(indexPath) ? read(indexPath) : null
  // No stamp means v1 — the version that predates stamping. Never guess higher
  // from the presence of a v2 feature; a half-migrated project would lie.
  const cfg = indexText ? parseFrontmatter(indexText).data ?? {} : {}
  const version = indexText ? Number(cfg.framework ?? 1) : 0
  return {
    root,
    docs,
    indexPath,
    indexText,
    ideasPath: path.join(docs, 'ideas.md'),
    version,
    gate: cfg.gate ?? null,
    inner: cfg.inner ?? null,
    lanes: Math.max(1, Number(cfg.lanes ?? 1) || 1),
    // Tri-state on purpose: absent means nobody measured it, which is not the same
    // as "serial". Only a measured false is worth blocking a run over.
    gateParallel: cfg.gate_parallel === undefined || cfg.gate_parallel === null ? null : String(cfg.gate_parallel) === 'true',
    checkpointEvery: Math.max(1, Number(cfg.checkpoint_every ?? CHECKPOINT_EVERY) || CHECKPOINT_EVERY),
    limits: {
      prdKb: Number(cfg.prd_max_kb ?? SIZE_WARN_PRD_KB),
      sliceKb: Number(cfg.slice_max_kb ?? SIZE_WARN_SLICE_KB),
    },
    features: live.features,
    archived: arch.features,
    archiveLoose: arch.loose,
    all: [...live.features, ...arch.features],
  }
}

/** Retired PRD numbers named in the index's prose. Advisory — the agent verifies. */
function retiredNumbers(p) {
  const line = p.indexText?.match(/Retired numbers[^\n]*\n?([^\n]*)/i)?.[0] ?? ''
  return [...new Set([...line.matchAll(/\b(\d{3})\b/g)].map((m) => Number(m[1])))]
}

/** Parse a markdown table under the heading whose text matches `re`. */
function parseTable(indexText, re) {
  const lines = indexText.split('\n')
  let i = lines.findIndex((l) => /^##\s+/.test(l) && re.test(l))
  if (i === -1) return null
  const rows = []
  let header = null
  for (i++; i < lines.length; i++) {
    const l = lines[i]
    if (/^##\s+/.test(l)) break
    if (!l.trim().startsWith('|')) continue
    const cells = l.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((c) => c.trim())
    if (!header) {
      header = cells
      continue
    }
    if (cells.every((c) => /^:?-+:?$/.test(c) || c === '')) continue
    rows.push({ lineNo: i, cells, raw: l })
  }
  return { header, rows }
}

/** A number cell starts with the number. "V1 PRD (frozen)" is a title, not PRD-001. */
const cellNumber = (c) => {
  const m = String(c ?? '').match(/^\**\s*(\d{1,4})\b/)
  return m ? Number(m[1]) : null
}
const cellStatus = (c) => STATUSES.find((s) => new RegExp(`\\b${s}\\b`, 'i').test(String(c ?? ''))) ?? null
const cellLink = (c) => String(c ?? '').match(/\]\(([^)]+)\)/)?.[1] ?? null

/** What the folders say the status is, ignoring what index.md claims. */
function computedStatus(feature) {
  if (feature.kind === 'archive') return feature.fm?.status === 'retired' ? 'retired' : 'archived'
  return feature.fm?.status ?? null
}

// ─────────────────────────────────────────────────────────────────────── gate

// The one command that proves a change is done. Declared once in docs/index.md
// frontmatter (`gate: composer verify`) and read from there by every skill that
// needs it — the slice Verify task, /polish, test-cleanup — instead of each one
// hunting through CLAUDE.md. A polyglot repo with several gates wraps them in one
// target (make/just/composer); the framework stores exactly one string.
//
// The tool checks that the entry point exists. It never runs the command: a real
// gate takes minutes, and running it is the agent's job at Verify time.

const GATE_NAMES = ['verify', 'check', 'ci', 'ci:check', 'qa', 'test']
const GATE_RUNNERS = new Set(['composer', 'npm', 'pnpm', 'yarn', 'bun', 'make', 'just', 'mise', 'cargo', 'go', 'mix', 'mvn', 'gradle', 'gradlew', 'swift', 'flutter', 'dart'])
// Toolchains whose tasks the tool cannot enumerate cheaply — the manifest proves the runner fits the repo.
const GATE_MANIFESTS = {
  cargo: ['Cargo.toml'], go: ['go.mod'], mix: ['mix.exs'], mvn: ['pom.xml'], gradle: ['build.gradle', 'build.gradle.kts'],
  gradlew: ['gradlew'], swift: ['Package.swift'], flutter: ['pubspec.yaml'], dart: ['pubspec.yaml'],
}

function jsonScripts(root, file) {
  try {
    return JSON.parse(read(path.join(root, file))).scripts ?? {}
  } catch {
    return null
  }
}

function firstExisting(root, names) {
  return names.map((n) => path.join(root, n)).find(exists) ?? null
}

function makeTargets(root) {
  const f = firstExisting(root, ['Makefile', 'makefile', 'GNUmakefile'])
  return f ? [...read(f).matchAll(/^([A-Za-z0-9_.-]+)\s*:(?!=)/gm)].map((m) => m[1]) : null
}

function justRecipes(root) {
  const f = firstExisting(root, ['justfile', 'Justfile', '.justfile'])
  return f ? [...read(f).matchAll(/^([A-Za-z0-9_-]+)(?:\s+[^:=\n]*)?:(?!=)/gm)].map((m) => m[1]) : null
}

function miseTasks(root) {
  const f = firstExisting(root, ['mise.toml', '.mise.toml'])
  if (!f) return null
  const text = read(f)
  const out = new Set([...text.matchAll(/^\[tasks\.["']?([A-Za-z0-9_:.-]+)["']?\]/gm)].map((m) => m[1]))
  const block = text.split(/^\[tasks\]\s*$/m)[1]?.split(/^\[/m)[0] ?? ''
  for (const m of block.matchAll(/^["']?([A-Za-z0-9_:.-]+)["']?\s*=/gm)) out.add(m[1])
  return [...out]
}

function jsRunner(root) {
  if (exists(path.join(root, 'pnpm-lock.yaml'))) return 'pnpm run'
  if (exists(path.join(root, 'yarn.lock'))) return 'yarn'
  if (exists(path.join(root, 'bun.lockb')) || exists(path.join(root, 'bun.lock'))) return 'bun run'
  return 'npm run'
}

/** One shell-level part of the gate → { status: ok | missing | unverified, detail }. */
function resolveGatePart(root, part) {
  const tokens = part.trim().split(/\s+/)
  const idx = tokens.findIndex((t) => GATE_RUNNERS.has(path.basename(t)))
  if (idx === -1) {
    const first = tokens[0] ?? ''
    if (first.includes('/')) {
      return exists(path.join(root, first)) ? { status: 'ok', detail: 'resolves' } : { status: 'missing', detail: `${first} not found` }
    }
    return { status: 'unverified', detail: `cannot check "${first}" — declared as-is` }
  }
  const runner = path.basename(tokens[idx])
  // `mise exec -- composer verify`: the real gate is what follows the wrapper.
  if (runner === 'mise' && ['exec', 'x'].includes(tokens[idx + 1])) {
    const j = tokens.indexOf('--', idx)
    return resolveGatePart(root, tokens.slice(j === -1 ? idx + 2 : j + 1).join(' '))
  }
  const args = tokens.slice(idx + 1).filter((t) => !t.startsWith('-') && !t.includes('='))
  const script = ['run', 'run-script'].includes(args[0]) ? args[1] : args[0]
  const lookup = (names, source, what) => {
    if (names === null) return { status: 'missing', detail: `no ${source}` }
    const wanted = runner === 'make' || runner === 'just' ? args : [script]
    const gone = wanted.filter((w) => w && !names.includes(w))
    return gone.length ? { status: 'missing', detail: `${source} has no "${gone[0]}" ${what}` } : { status: 'ok', detail: 'resolves' }
  }
  switch (runner) {
    case 'composer': {
      const s = jsonScripts(root, 'composer.json')
      return lookup(s && Object.keys(s), 'composer.json', 'script')
    }
    case 'npm':
    case 'pnpm':
    case 'yarn':
    case 'bun': {
      const s = jsonScripts(root, 'package.json')
      return lookup(s && Object.keys(s), 'package.json', 'script')
    }
    case 'make':
      return lookup(makeTargets(root), 'Makefile', 'target')
    case 'just':
      return lookup(justRecipes(root), 'justfile', 'recipe')
    case 'mise':
      return lookup(miseTasks(root), 'mise.toml', 'task')
    default: {
      const manifest = firstExisting(root, GATE_MANIFESTS[runner])
      return manifest
        ? { status: 'ok', detail: `resolves (${path.basename(manifest)} present, task not checked)` }
        : { status: 'missing', detail: `no ${GATE_MANIFESTS[runner].join(' / ')} for ${runner}` }
    }
  }
}

/** Likely gate commands, best name first, plus prose hints from the agent instruction files. */
function gateCandidates(root) {
  const sources = []
  const composer = jsonScripts(root, 'composer.json')
  if (composer) sources.push([Object.keys(composer), (n) => `composer ${n}`])
  const pkg = jsonScripts(root, 'package.json')
  if (pkg) {
    const r = jsRunner(root)
    sources.push([Object.keys(pkg), (n) => `${r} ${n}`])
  }
  for (const [names, fmt] of [[makeTargets(root), (n) => `make ${n}`], [justRecipes(root), (n) => `just ${n}`], [miseTasks(root), (n) => `mise run ${n}`]]) {
    if (names) sources.push([names, fmt])
  }
  const commands = []
  for (const n of GATE_NAMES) for (const [names, fmt] of sources) if (names.includes(n)) commands.push(fmt(n))

  const hints = []
  for (const f of ['CLAUDE.md', 'AGENTS.md', '.ai/guidelines/verification.md']) {
    const file = path.join(root, f)
    if (!exists(file)) continue
    for (const line of read(file).split('\n')) {
      if (!/`[^`]*\bverify\b[^`]*`/i.test(line)) continue
      hints.push(`${f}: ${line.trim().slice(0, 110)}`)
      if (hints.length >= 3) break
    }
  }
  return { commands: [...new Set(commands)], hints }
}

/** { declared, status: none | ok | unverified | missing, detail, candidates? } */
function gateStatus(root, p, opts = {}) {
  if (!p.gate) return { declared: null, status: 'none', detail: 'no gate declared', candidates: opts.candidates ? gateCandidates(root) : null }
  const declared = String(p.gate)
  const parts = declared.split(/\s*(?:&&|\|\||;)\s*/).filter(Boolean)
  const results = parts.map((part) => resolveGatePart(root, part))
  const rank = { ok: 0, unverified: 1, missing: 2 }
  const worst = results.reduce((a, r) => (rank[r.status] > rank[a.status] ? r : a), results[0])
  const detail = worst.status === 'ok'
    ? results.find((r) => r.detail !== 'resolves')?.detail ?? 'resolves'
    : results.filter((r) => r.status !== 'ok').map((r) => r.detail).join('; ')
  return { declared, status: worst.status, detail, candidates: null }
}

function gateLine(g) {
  if (g.status === 'none') return 'none declared — run /guardrails, or `docs gate set "<cmd>"`'
  if (g.status === 'ok') return `${g.declared} (${g.detail})`
  if (g.status === 'unverified') return `${g.declared} (declared; ${g.detail})`
  return `${g.declared} — ${g.detail}`
}

/** Write a scalar key (`gate:`, `inner:`, `lanes:`) into index.md frontmatter by text edit, so comments and order survive. */
function setIndexGate(indexPath, cmd, key = 'gate', { raw = false } = {}) {
  const text = read(indexPath)
  const line = `${key}: ${!raw && /^(\[|["']|-?\d+$)/.test(cmd) ? `"${cmd}"` : cmd}`
  if (!text.startsWith('---\n')) return write(indexPath, `---\n${line}\n---\n\n${text}`)
  const end = text.indexOf('\n---', 3)
  const block = text.slice(4, end)
  const re = new RegExp(`^[ \\t]*#?[ \\t]*${key}:.*$`, 'm')
  write(indexPath, `---\n${re.test(block) ? block.replace(re, line) : `${block}\n${line}`}${text.slice(end)}`)
}

function gateCmd(root, positional, flags) {
  const p = loadProject(root)
  if (!exists(p.indexPath)) die('no docs/index.md — run `docs init` first')
  if (positional[1] === 'set') {
    const cmd = positional.slice(2).join(' ').trim()
    if (!cmd) die('usage: docs gate set "<command>"  ·  docs gate set --inner "<command>"')
    if (flags.inner) {
      // The fast subset a slice's Verify task runs; the full gate runs at checkpoints.
      setIndexGate(p.indexPath, cmd, 'inner')
      return console.log(`inner: ${cmd}`)
    }
    setIndexGate(p.indexPath, cmd)
    const g = gateStatus(root, loadProject(root))
    console.log(`gate: ${gateLine(g)}`)
    if (g.status === 'missing') console.log('declared anyway — fix the entry point, or the first Verify task fails on the spot')
    return
  }
  if (flags.inner) {
    // Bare command on stdout; falls back to the full gate so a Verify task always has something to run.
    if (p.inner) return console.log(p.inner)
    if (!p.gate) process.exit(1)
    console.error('! no inner loop declared — running the full gate per slice (`docs gate set --inner "<cmd>"`)')
    return console.log(p.gate)
  }
  const g = gateStatus(root, p, { candidates: true })
  if (flags.candidates || g.status === 'none') {
    const c = g.candidates ?? gateCandidates(root)
    console.log(g.status === 'none' ? 'no gate declared' : `declared: ${gateLine(g)}`)
    console.log(c.commands.length ? `candidates: ${c.commands.join(', ')}` : 'candidates: none found (no verify/check/ci/qa script, target, recipe or task)')
    for (const h of c.hints) console.log(`  hint · ${h}`)
    if (g.status === 'none') console.log('declare one with `docs gate set "<cmd>"`, or run /guardrails to build it properly')
    process.exit(g.status === 'none' ? 1 : 0)
  }
  // Bare command on stdout so a skill can lift it; problems go to stderr.
  console.log(g.declared)
  if (g.status !== 'ok') console.error(`! ${g.detail}`)
}

// ───────────────────────────────────────────────────────────────── foundations

/**
 * The three things a project needs before it can build a feature and prove it:
 * a gate (nothing broke), a harness (the feature exists), and optionally a UX
 * rubric (what "finished" looks like on screen).
 *
 * Reported by `doctor`, never enforced by `check`. A library with no user surface
 * legitimately has no harness, and a project with no UI legitimately has no rubric;
 * warning on either would cry wolf at projects that are already correct.
 */
function harnessStatus(root) {
  const dir = path.join(root, '.claude', 'skills')
  if (!exists(dir)) return { present: false, detail: 'none — run /verify-app' }
  const found = fs.readdirSync(dir).filter((n) => n.startsWith('verify-') && exists(path.join(dir, n, 'SKILL.md')))
  if (!found.length) return { present: false, detail: 'none — run /verify-app' }
  const detail = found.map((n) => {
    const feats = path.join(dir, n, 'features')
    const count = exists(feats) ? fs.readdirSync(feats).filter((f) => f.endsWith('.md') && f !== 'README.md').length : 0
    return `${n}${count ? ` (${count} features mapped)` : ' — no feature map'}`
  })
  return { present: true, detail: detail.join(', ') }
}

function foundations(root, p) {
  const rubric = exists(path.join(p.docs, 'ux-rubric.md'))
  return {
    gate: gateStatus(root, p, { candidates: true }),
    harness: harnessStatus(root),
    rubric: { present: rubric, detail: rubric ? 'docs/ux-rubric.md' : 'none — `/ux-review rubric`, when the project has a UI' },
  }
}

// ─────────────────────────────────────────────────────────────────── conformance

/**
 * Probe every requirement regardless of the project's declared version. This is
 * what turns "how far behind is this project" into a list a coding agent can work.
 * Archived folders are never probed — frozen means frozen, so they are grandfathered.
 */
function assess(p) {
  const out = []
  const add = (id, ok, have, want, detail = []) => {
    const req = REQUIREMENTS.find((r) => r.id === id)
    out.push({ ...req, ok, have, want, detail })
  }

  const layout = [
    ['docs/index.md', !!p.indexText],
    ['docs/features/', exists(path.join(p.docs, 'features'))],
    ['docs/ideas.md', exists(p.ideasPath)],
  ]
  add('layout', layout.every(([, v]) => v), layout.filter(([, v]) => v).length, layout.length,
    layout.filter(([, v]) => !v).map(([k]) => `missing ${k}`))

  add('stamp', p.version === FRAMEWORK_VERSION, p.version || 0, FRAMEWORK_VERSION,
    p.version === FRAMEWORK_VERSION ? [] : [p.version ? `declares framework: ${p.version}` : 'no framework stamp — treated as v1'])

  const fmFiles = []
  for (const f of p.features) {
    if (f.prdExists) fmFiles.push([rel(p, f.prdPath), !!f.fm])
    for (const s of f.slices) fmFiles.push([s.rel, !!s.fm])
  }
  add('frontmatter', fmFiles.every(([, v]) => v), fmFiles.filter(([, v]) => v).length, fmFiles.length,
    fmFiles.filter(([, v]) => !v).map(([k]) => k))

  const sliced = p.features.filter((f) => f.slices.length && f.prdExists)
  const marked = sliced.filter((f) => read(f.prdPath).includes('<!-- docs:slices -->'))
  add('markers', marked.length === sliced.length, marked.length, sliced.length,
    sliced.filter((f) => !marked.includes(f)).map((f) => `${rel(p, f.prdPath)} has no slice-table markers`))

  // Retired numbers are parsed out of index prose, so only the ones with no folder
  // are reported — a number that already has a folder proves nothing either way.
  const missingTombs = retiredNumbers(p).filter((n) => !p.all.some((f) => f.number === n))
  add('tombstones', missingTombs.length === 0, 0, missingTombs.length,
    missingTombs.map((n) => `PRD-${pad(n)} reads as retired in index.md but has no tombstone folder — verify before creating one`))

  return out
}

function doctor(root, opts = {}) {
  const p = loadProject(root)
  const reqs = assess(p)
  const { report } = p.indexText ? check(root) : { report: { errors: [], warnings: [] } }
  const pending = reqs.filter((r) => !r.ok)
  const found = foundations(root, p)
  const gate = found.gate
  const result = {
    project: root,
    framework: { declared: p.version, current: FRAMEWORK_VERSION, behind: Math.max(0, FRAMEWORK_VERSION - (p.version || 1)) },
    requirements: reqs,
    foundations: found,
    gate,
    drift: { errors: report.errors, warnings: report.warnings },
    upgrades: upgradeDocs(p.version || 1),
  }
  if (opts.json) {
    console.log(JSON.stringify(result, null, 2))
    return result
  }

  const state = !p.indexText ? 'not on the framework' : p.version === FRAMEWORK_VERSION ? `v${p.version} (current)` : `v${p.version || 1} — current is v${FRAMEWORK_VERSION}`
  console.log(`Framework      ${state}`)
  for (const r of reqs) {
    const mark = r.ok ? 'ok' : `${r.have}/${r.want}`
    const tag = r.ok ? '' : `   ← v${r.since}, fix by ${r.fixBy}`
    console.log(`${r.id.padEnd(15)}${String(mark).padEnd(10)}${r.label}${tag}`)
  }
  console.log(`drift          ${report.errors.length ? `${report.errors.length} errors` : 'clean'}${report.warnings.length ? `, ${report.warnings.length} warnings` : ''}`)
  console.log(`gate           ${gateLine(gate)}`)
  if (gate.status !== 'none') console.log(`inner loop     ${p.inner ? p.inner : 'none declared — every slice runs the full gate; `docs gate set --inner "<cmd>"` (lint, types, unit)'}`)
  if (gate.status === 'none' && gate.candidates.commands.length) console.log(`               candidates: ${gate.candidates.commands.join(', ')}  (\`docs gate --candidates\` for detail)`)
  console.log(`harness        ${found.harness.detail}`)
  console.log(`ux rubric      ${found.rubric.detail}`)
  if (p.archived.length) console.log(`archive        ${p.archived.length} folder(s), frozen — never upgraded, by rule`)

  if (pending.length) {
    const tool = pending.filter((r) => r.fixBy === 'tool').map((r) => r.id)
    const agent = pending.filter((r) => r.fixBy === 'agent').map((r) => r.id)
    console.log('')
    if (tool.length) console.log(`mechanical (tool):  ${tool.join(', ')}`)
    if (agent.length) console.log(`judgment (agent):   ${agent.join(', ')}`)
    for (const r of pending.filter((x) => x.detail.length)) {
      console.log(`\n${r.id}:`)
      for (const d of r.detail.slice(0, 6)) console.log(`  · ${d}`)
      if (r.detail.length > 6) console.log(`  · …and ${r.detail.length - 6} more (\`docs doctor --json\`)`)
    }
    if (result.upgrades.length) console.log(`\nupgrade guides:  ${result.upgrades.join(', ')}`)
    console.log('next: invoke the feature-docs skill — it reads this and drives the upgrade')
  } else if (!report.errors.length) {
    console.log('\nup to date, no drift')
  }
  // Foundations are reported, never enforced: a library with no user surface has no
  // harness and a project with no UI has no rubric, and both are correct.
  const missing = [
    gate.status === 'none' && 'a gate (`/guardrails`)',
    !found.harness.present && 'a harness (`/verify-app`)',
    !found.rubric.present && 'a UX rubric (`/ux-review rubric`, only if it has a UI)',
    gate.status !== 'none' && !p.inner && 'an inner loop (`docs gate set --inner "<cmd>"`, so slices skip the slow tiers)',
  ].filter(Boolean)
  if (missing.length) console.log(`\nfoundations missing: ${missing.join(' · ')}\n  build them in that order — each one is used by the next`)
  return result
}

/** Upgrade guides that still apply, in order. The agent reads these, not the tool. */
function upgradeDocs(from) {
  const dir = path.join(HERE, '..', 'upgrades')
  if (!exists(dir)) return []
  return fs
    .readdirSync(dir)
    .filter((f) => /^v(\d+)\.md$/.test(f) && Number(f.match(/^v(\d+)/)[1]) > from)
    .sort()
    .map((f) => path.join('upgrades', f))
}

function scan(roots) {
  const found = []
  const walk = (dir, depth) => {
    if (depth > 4) return
    let entries
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true })
    } catch {
      return
    }
    if (entries.some((e) => e.isDirectory() && e.name === 'docs') && exists(path.join(dir, 'docs', 'index.md'))) {
      found.push(dir)
      return // a project owns its subtree; do not descend into it
    }
    for (const e of entries) {
      if (!e.isDirectory() || e.name.startsWith('.') || ['node_modules', 'vendor', 'dist', 'build', 'target'].includes(e.name)) continue
      walk(path.join(dir, e.name), depth + 1)
    }
  }
  for (const r of roots) walk(path.resolve(r), 0)

  if (!found.length) return console.log('no framework projects found')
  const glyph = { ok: '✓', none: '—', missing: '✗', unverified: '?' }
  const rows = found.map((dir) => {
    const p = loadProject(dir)
    const pending = assess(p).filter((r) => !r.ok)
    const { report } = check(dir)
    const gate = gateStatus(dir, p)
    const harness = harnessStatus(dir)
    return { dir, v: p.version || 1, pending: pending.length, gaps: pending.map((r) => r.id).join(','), errors: report.errors.length, gate: glyph[gate.status], harness: harness.present ? '✓' : '—' }
  })
  const w = Math.max(...rows.map((r) => r.dir.length))
  console.log(`${'project'.padEnd(w)}  ver  drift  gate  harn  gaps`)
  for (const r of rows) {
    console.log(`${r.dir.padEnd(w)}  v${r.v}   ${String(r.errors).padStart(5)}  ${r.gate.padEnd(4)}  ${r.harness.padEnd(4)}  ${r.gaps || '—'}`)
  }
  const behind = rows.filter((r) => r.pending || r.errors || r.gate === '—' || r.gate === '✗' || r.harness === '—')
  console.log(`\ngate: ✓ resolves · — none declared · ✗ entry point missing · ? cannot check`)
  console.log(`harn: ✓ a verify-* harness exists · — none (correct for a project with no user surface)`)
  console.log(`${rows.length} project(s), ${behind.length} needing work. Run \`docs doctor\` inside one for the detail.`)
}

// ──────────────────────────────────────────────────────────────────── reports

class Report {
  constructor() {
    this.errors = []
    this.warnings = []
  }
  err(code, msg) {
    this.errors.push({ code, msg })
  }
  warn(code, msg) {
    this.warnings.push({ code, msg })
  }
  print() {
    for (const e of this.errors) console.error(`  ✗ ${e.code}  ${e.msg}`)
    for (const w of this.warnings) console.error(`  ! ${w.code}  ${w.msg}`)
    const e = this.errors.length
    const w = this.warnings.length
    console.error(`\n${e} error${e === 1 ? '' : 's'}, ${w} warning${w === 1 ? '' : 's'}`)
  }
}

// ────────────────────────────────────────────────────────────────────── check

function check(root, opts = {}) {
  // Fix first, then lint what is left — otherwise --fix reports errors it just repaired.
  if (opts.fix) render(root, { write: true, quiet: true })
  const p = loadProject(root)
  const r = new Report()

  if (!exists(p.indexPath)) {
    r.err('E-NO-INDEX', 'docs/index.md is missing — run `docs init`')
    return { report: r, project: p }
  }
  if (!exists(p.ideasPath)) r.warn('W-NO-IDEAS', 'docs/ideas.md is missing — loose ideas have nowhere to live')

  const indexText = p.indexText
  // Enforce a version's rules only once the project declares that version.
  // Behind-ness is doctor's job to report, not check's job to bury you in.
  const v2 = p.version >= 2
  if (p.version < FRAMEWORK_VERSION) {
    r.warn('W-FRAMEWORK-BEHIND', `project is framework v${p.version || 1}, current is v${FRAMEWORK_VERSION} — run \`docs doctor\` for the gap`)
  }

  // The gate is optional in the schema and mandatory in practice: every slice ends
  // in a Verify task that needs it. Nag until it is declared; never block on it.
  if (v2) {
    const g = gateStatus(root, p)
    if (g.status === 'none') {
      r.warn('W-NO-GATE', 'no verify gate declared in docs/index.md frontmatter — run /guardrails, or `docs gate set "<cmd>"` (`docs gate --candidates` lists likely ones)')
    } else if (g.status === 'missing') {
      r.warn('W-GATE-UNRESOLVED', `gate "${g.declared}" — ${g.detail}`)
    }
  }

  // frontmatter, numbering, statuses
  const byNumber = new Map()
  for (const f of p.all) {
    if (byNumber.has(f.number)) {
      r.err('E-NUM-DUPLICATE', `PRD ${pad(f.number)} claimed by both ${byNumber.get(f.number).dirName} and ${f.dirName}`)
    } else {
      byNumber.set(f.number, f)
    }
    if (!f.prdExists) {
      r.err('E-NO-PRD', `${rel(p, f.dir)} has no prd.md`)
      continue
    }
    // Frozen history is grandfathered: an archived folder predating a requirement
    // can never adopt it, because adopting it would mean editing a frozen file.
    const graded = v2 && f.kind === 'live'
    if (f.prdKb > p.limits.prdKb && f.kind === 'live') {
      r.warn('W-SIZE', `${rel(p, f.prdPath)} is ${Math.round(f.prdKb)}KB / ${f.prdLines} lines — slice, don't fatten (ceiling ${p.limits.prdKb}KB)`)
    }
    if (f.fmError) r.err('E-FM-INVALID', `${rel(p, f.prdPath)}: ${f.fmError}`)
    if (!f.fm) {
      if (graded) r.err('E-FM-MISSING', `${rel(p, f.prdPath)} has no frontmatter (needs number, slug, status, owns_fr)`)
      continue
    }
    if (f.fm.number !== f.number) {
      r.err('E-NUM-MISMATCH', `${rel(p, f.prdPath)} frontmatter number ${f.fm.number} ≠ folder ${pad(f.number)}`)
    }
    if (f.fm.slug && f.fm.slug !== f.slug) {
      r.err('E-SLUG-MISMATCH', `${rel(p, f.prdPath)} frontmatter slug "${f.fm.slug}" ≠ folder "${f.slug}"`)
    }
    if (!STATUSES.includes(f.fm.status)) {
      r.err('E-STATUS-INVALID', `${rel(p, f.prdPath)} status "${f.fm.status}" not in ${STATUSES.join('|')}`)
    }
    if (f.kind === 'live' && ['archived', 'retired'].includes(f.fm.status)) {
      r.err('E-STATUS-PLACE', `${rel(p, f.prdPath)} is ${f.fm.status} but still under features/ — run \`docs archive ${pad(f.number)}\``)
    }
    if (f.kind === 'archive' && !['archived', 'retired'].includes(f.fm.status)) {
      r.err('E-STATUS-PLACE', `${rel(p, f.prdPath)} is under archive/ but status is "${f.fm.status}"`)
    }
    // Archived docs are frozen, so their prose can never be brought back into line — don't nag.
    if (f.kind === 'live' && f.proseStatus && f.fm.status && !new RegExp(`\\b${f.fm.status}\\b`, 'i').test(f.proseStatus)) {
      r.warn('W-PROSE-STATUS', `${rel(p, f.prdPath)} prose Status line does not mention "${f.fm.status}"`)
    }
    // A PRD is agreed (ready) only once it says how each FR will be proven on the
    // real surface. Draft is exempt — it is still being interviewed.
    if (graded && ['ready', 'in-progress'].includes(f.fm.status) && !f.hasQaPlan) {
      r.warn('W-NO-QA-PLAN', `${rel(p, f.prdPath)} is ${f.fm.status} but has no "QA plan" section — every FR needs a proof row before it is sliced (see /prd)`)
    }
  }

  // FR ownership
  const frOwner = new Map()
  const retiredFr = new Map()
  for (const f of p.all) {
    if (!f.fm) continue
    for (const n of expandFr(f.fm.retired_fr)) retiredFr.set(n, f)
    for (const n of expandFr(f.fm.owns_fr)) {
      if (frOwner.has(n)) {
        r.err('E-FR-OVERLAP', `FR-${n} owned by both PRD-${pad(frOwner.get(n).number)} and PRD-${pad(f.number)}`)
      } else {
        frOwner.set(n, f)
      }
    }
  }
  for (const [n, owner] of retiredFr) {
    if (frOwner.has(n)) {
      r.err('E-FR-RETIRED-REUSED', `FR-${n} was retired (PRD-${pad(owner.number)}) but PRD-${pad(frOwner.get(n).number)} claims it — numbers are never reused`)
    }
  }

  // slices
  for (const f of p.all) {
    const seen = new Set()
    for (const s of f.slices) {
      if (seen.has(s.number)) r.err('E-SLICE-DUPLICATE', `${rel(p, f.dir)} has two slice-${pad2(s.number)} files`)
      seen.add(s.number)
      if (s.specKb > p.limits.sliceKb && f.kind === 'live') {
        r.warn('W-SIZE', `${s.rel} spec is ${Math.round(s.specKb)}KB (${Math.round(s.kb)}KB with Progress) — split the slice (ceiling ${p.limits.sliceKb}KB)`)
      }
      if (s.fmError) r.err('E-FM-INVALID', `${rel(p, path.join(f.dir, s.file))}: ${s.fmError}`)
      if (!s.fm) {
        if (v2 && f.kind === 'live') {
          r.err('E-FM-MISSING', `${rel(p, path.join(f.dir, s.file))} has no frontmatter (needs slice, status, depends_on)`)
        }
        continue
      }
      if (s.fm.slice !== s.number) {
        r.err('E-NUM-MISMATCH', `${s.rel} frontmatter slice ${s.fm.slice} ≠ filename ${pad2(s.number)}`)
      }
      if (!SLICE_STATUSES.includes(s.fm.status)) {
        r.err('E-STATUS-INVALID', `${s.rel} status "${s.fm.status}" not in ${SLICE_STATUSES.join('|')}`)
      }
      for (const d of s.fm.depends_on ?? []) {
        if (!f.slices.some((o) => o.number === Number(d))) {
          r.err('E-DEP-MISSING', `${s.rel} depends_on slice ${d}, which does not exist`)
        }
      }
    }
    // agent log and retro, written by /deliver. Archive is frozen, so only live features.
    if (f.kind === 'live' && f.log) {
      const logRel = rel(p, path.join(f.dir, 'agent-log.md'))
      const seenLog = new Set()
      for (const e of f.log.entries) {
        const key = `${e.kind}-${e.n}`
        if (seenLog.has(key)) r.err('E-LOG-DUPLICATE', `${logRel} has two ${key} entries — numbers are never reused`)
        seenLog.add(key)
      }
    }
    if (f.kind === 'live') {
      const openBlocks = (f.log?.entries ?? []).filter((e) => e.kind === 'BLOCK' && e.state === 'open')
      for (const s of f.slices.filter((x) => x.fm?.status === 'blocked')) {
        if (!openBlocks.some((b) => b.slice === s.number)) {
          r.warn('W-BLOCKED-NO-ENTRY', `${s.rel} is blocked but agent-log.md has no open BLOCK naming slice-${pad2(s.number)} — record why, or set it back to building`)
        }
      }
      const cpStale = (f.log?.entries ?? []).filter((e) => e.kind === 'CP' && e.slice && !f.slices.some((s) => s.number === e.slice && s.fm?.status === 'done'))
      for (const c of cpStale) r.warn('W-CP-STALE', `${rel(p, f.dir)}/agent-log.md CP-${c.n} says it verified through slice-${pad2(c.slice)}, which is not done`)
      for (const b of openBlocks.filter((x) => x.slice !== null)) {
        if (!f.slices.some((s) => s.number === b.slice && s.fm?.status === 'blocked')) {
          r.warn('W-BLOCK-STALE', `${rel(p, f.dir)}/agent-log.md BLOCK-${b.n} is open but slice-${pad2(b.slice)} is not blocked — mark the entry resolved, or block the slice`)
        }
      }
      if (f.retro) {
        const missing = RETRO_KEYS.filter((k) => typeof f.retro.data?.[k] !== 'number')
        if (missing.length) r.warn('W-RETRO-FM', `${rel(p, f.dir)}/retro.md frontmatter is missing numbers for: ${missing.join(', ')}`)
      }
    }

    // dependency cycles, per feature
    const cycle = findCycle(f.slices)
    if (cycle) r.err('E-DEP-CYCLE', `${rel(p, f.dir)} slice dependency cycle: ${cycle.join(' → ')}`)

    // feature status vs slice statuses
    const st = computedStatus(f)
    const open = f.slices.filter((s) => s.fm && s.fm.status !== 'done')
    if (['shipped', 'archived'].includes(st) && open.length) {
      r.err('E-STATUS-INCONSISTENT', `PRD-${pad(f.number)} is ${st} but ${open.length} slice(s) are not done: ${open.map((s) => pad2(s.number)).join(', ')}`)
    }
    if (['draft', 'ready'].includes(st) && f.slices.some((s) => s.fm && s.fm.status !== 'pending')) {
      r.warn('W-STATUS-BEHIND', `PRD-${pad(f.number)} is ${st} but a slice is already building/done — should it be in-progress?`)
    }
    // These two chain rather than fire together: polish comes before shipping, so
    // you get one nudge at a time, and both arrive at commit time via the hook.
    if (f.kind === 'live' && f.slices.length && open.length === 0 && ['draft', 'ready', 'in-progress'].includes(st)) {
      if (!f.fm.reviewed) {
        r.warn('W-NOT-REVIEWED', `PRD-${pad(f.number)}: every slice is done but it has not been through /polish — review, then set \`reviewed:\` in prd.md frontmatter`)
      } else {
        r.warn('W-READY-TO-SHIP', `PRD-${pad(f.number)}: reviewed ${f.fm.reviewed}, every slice done, status still "${st}" — run \`docs ship ${pad(f.number)}\` before you merge (or \`docs archive\` if nothing is left)`)
      }
    }
  }

  // index tables vs disk
  const liveTable = parseTable(indexText, /live features/i)
  const archTable = parseTable(indexText, /archived/i)
  if (!liveTable) r.err('E-NO-BOARD', 'docs/index.md has no "Live features" table')
  if (!archTable) r.warn('W-NO-ARCHIVE-TABLE', 'docs/index.md has no "Archived" table')

  const rowsFor = (t) => (t?.rows ?? []).map((row) => ({ ...row, number: cellNumber(row.cells[0]), status: cellStatus(row.cells[2]), link: cellLink(row.cells[1]) })).filter((row) => row.number !== null)

  const liveRows = rowsFor(liveTable)
  const archRows = rowsFor(archTable)

  for (const f of p.features) {
    const row = liveRows.find((x) => x.number === f.number)
    if (!row) {
      r.err('E-INDEX-MISSING-ROW', `PRD-${pad(f.number)} (${f.dirName}) has no row in the Live features table`)
      continue
    }
    // Needs frontmatter to know the truth; at v1 the index row IS the only record.
    const st = v2 ? computedStatus(f) : null
    if (st && row.status && row.status !== st) {
      r.err('E-INDEX-STATUS-MISMATCH', `index says PRD-${pad(f.number)} is "${row.status}", prd.md says "${st}" — \`docs render --write\` fixes it`)
    } else if (st && !row.status) {
      r.err('E-INDEX-STATUS-MISMATCH', `index row for PRD-${pad(f.number)} has no recognisable status cell`)
    }
  }
  for (const f of p.archived) {
    if (!archRows.some((x) => x.number === f.number)) {
      r.err('E-ARCHIVE-MISSING-ROW', `PRD-${pad(f.number)} (${f.dirName}) is archived but has no row in the Archived table`)
    }
  }
  for (const row of [...liveRows, ...archRows]) {
    if (!byNumber.has(row.number)) {
      r.err('E-INDEX-ORPHAN-ROW', `index row for PRD-${pad(row.number)} has no folder under features/ or archive/`)
    }
  }
  for (const name of p.archiveLoose) {
    if (!indexText.includes(name)) r.warn('W-ARCHIVE-UNLISTED', `docs/archive/${name} is not mentioned in index.md`)
  }

  // counters
  const want = nextNumbers(p)
  const cm = indexText.match(/Next free PRD number:\s*(\d+)/i)
  const fm2 = indexText.match(/Next free FR:\s*FR-(\d+)/i)
  if (!cm) {
    r.warn('W-NO-COUNTERS', 'index.md has no "Next free PRD number: NNN" line')
  } else if (Number(cm[1]) !== want.prd) {
    r.err('E-COUNTER-STALE', `index says next free PRD is ${cm[1]}, computed ${pad(want.prd)} — \`docs render --write\` fixes it`)
  }
  // Only meaningful once at least one PRD declares its FR range — otherwise the
  // computed value is 1 and would "correct" a hand-maintained counter to nonsense.
  const anyFr = p.all.some((f) => (f.fm?.owns_fr?.length ?? 0) || (f.fm?.retired_fr?.length ?? 0))
  if (fm2 && anyFr && Number(fm2[1]) !== want.fr) {
    r.err('E-COUNTER-STALE', `index says next free FR is FR-${fm2[1]}, computed FR-${want.fr} — \`docs render --write\` fixes it`)
  }

  // frozen archive (working tree; the pre-commit hook covers staged edits)
  if (git(['rev-parse', '--is-inside-work-tree'], root) === 'true') {
    // Untrimmed: porcelain's first column is a significant space.
    const dirty = git(['status', '--porcelain', '--', 'docs/archive'], root, { trim: false })
    if (dirty) {
      for (const line of dirty.split('\n').filter((l) => l.length > 3)) {
        // Porcelain is XY PATH: X = staged, Y = worktree. Moving a folder in is
        // how archiving works (R/A/??), so only a content change is a violation —
        // including "RM", a rename whose content was then edited.
        const [staged, tree] = [line[0], line[1]]
        if (staged !== 'M' && tree !== 'M') continue
        r.err('E-ARCHIVE-MODIFIED', `${line.slice(3)} is modified — docs/archive/** is frozen; move folders, never edit inside`)
      }
    }
  }

  // links — but never inside archive/. Frozen means never relinked, so a link that
  // went stale when the folder moved is the rule working, not a defect.
  const archiveDir = path.join(p.docs, 'archive')
  for (const file of walkMd(p.docs)) {
    if (file.startsWith(archiveDir + path.sep)) continue
    const text = read(file)
    for (const m of text.matchAll(/\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
      const href = m[1]
      if (/^(https?:|mailto:|#)/.test(href)) continue
      const target = path.resolve(path.dirname(file), href.split('#')[0])
      if (!href.split('#')[0]) continue
      if (!exists(target)) r.err('E-LINK-BROKEN', `${rel(p, file)} → ${href} does not resolve`)
    }
  }

  // second status board
  for (const dir of [root, p.docs]) {
    for (const name of SECOND_BOARD_NAMES) {
      const f = path.join(dir, name)
      if (exists(f)) r.warn('W-SECOND-BOARD', `${rel(p, f)} looks like a second status board — one index only, fold it into docs/index.md or ideas.md`)
    }
  }
  for (const dir of LEGACY_DOC_DIRS) {
    if (exists(path.join(root, dir))) {
      r.warn('W-LEGACY-DOCS', `${dir}/ still exists alongside docs/ — an older planning convention; migrate or delete it (see the feature-docs skill)`)
    }
  }

  if (opts.fix) render(root, { write: true, quiet: true })
  return { report: r, project: p }
}

/**
 * How many slices could be in flight at once, at the widest point of the dependency
 * graph — a greedy wave schedule, taking every ready slice each step. A lower bound
 * on the real parallelism (it ignores how long a slice takes), which is what we want:
 * it never promises a lane the graph cannot use.
 */
function parallelWidth(slices) {
  const pending = new Map(slices.map((s) => [s.number, (s.fm?.depends_on ?? []).map(Number)]))
  const done = new Set()
  let widest = 0
  while (pending.size) {
    const ready = [...pending].filter(([, deps]) => deps.every((d) => done.has(d) || !pending.has(d)))
    if (!ready.length) break // a cycle; findCycle already reports it
    widest = Math.max(widest, ready.length)
    for (const [n] of ready) { pending.delete(n); done.add(n) }
  }
  return widest
}

function findCycle(slices) {
  const byNum = new Map(slices.map((s) => [s.number, s]))
  const state = new Map()
  let found = null
  const visit = (n, trail) => {
    if (found) return
    if (state.get(n) === 1) {
      found = [...trail.slice(trail.indexOf(n)), n].map(pad2)
      return
    }
    if (state.get(n) === 2) return
    state.set(n, 1)
    for (const d of byNum.get(n)?.fm?.depends_on ?? []) {
      if (byNum.has(Number(d))) visit(Number(d), [...trail, n])
    }
    state.set(n, 2)
  }
  for (const s of slices) visit(s.number, [])
  return found
}

function walkMd(dir, out = []) {
  if (!exists(dir)) return out
  for (const name of fs.readdirSync(dir)) {
    const f = path.join(dir, name)
    const st = fs.statSync(f)
    if (st.isDirectory()) walkMd(f, out)
    else if (name.endsWith('.md')) out.push(f)
  }
  return out
}

function nextNumbers(p) {
  const maxPrd = p.all.reduce((a, f) => Math.max(a, f.number), 0)
  let maxFr = 0
  for (const f of p.all) {
    if (!f.fm) continue
    for (const n of expandFr(f.fm.owns_fr)) maxFr = Math.max(maxFr, n)
    for (const n of expandFr(f.fm.retired_fr)) maxFr = Math.max(maxFr, n)
  }
  return { prd: maxPrd + 1, fr: maxFr + 1 }
}

// ────────────────────────────────────────────────────────────────────- render

/** Rewrite only what is derivable: slice tables, index status cells, counters. */
function render(root, opts = {}) {
  const p = loadProject(root)
  const changes = []

  for (const f of p.all) {
    if (!f.prdExists || !f.slices.length) continue
    const text = read(f.prdPath)
    const next = replaceMarked(text, 'slices', sliceTable(f))
    if (next === null) {
      if (!opts.quiet) console.error(`  ! ${rel(p, f.prdPath)} has no <!-- docs:slices --> markers — slice table not rendered`)
      continue
    }
    if (next !== text) {
      changes.push(rel(p, f.prdPath))
      if (opts.write) write(f.prdPath, next)
    }
  }

  if (exists(p.indexPath)) {
    let text = read(p.indexPath)
    const before = text
    const want = nextNumbers(p)
    text = text.replace(/Next free PRD number:\s*\d+/i, `Next free PRD number: ${pad(want.prd)}`)
    // Same guard as check: without any declared owns_fr there is nothing to compute from.
    if (p.all.some((f) => (f.fm?.owns_fr?.length ?? 0) || (f.fm?.retired_fr?.length ?? 0))) {
      text = text.replace(/Next free FR:\s*FR-\d+/i, `Next free FR: FR-${want.fr}`)
    }

    const table = parseTable(text, /live features/i)
    if (table) {
      const lines = text.split('\n')
      for (const row of table.rows) {
        const n = cellNumber(row.cells[0])
        const f = p.features.find((x) => x.number === n)
        const st = f && computedStatus(f)
        if (!st) continue
        const cells = [...row.cells]
        const bold = /\*\*/.test(cells[2] ?? '')
        cells[2] = bold ? `**${st}**` : st
        lines[row.lineNo] = `| ${cells.join(' | ')} |`
      }
      text = lines.join('\n')
    }
    if (text !== before) {
      changes.push(rel(p, p.indexPath))
      if (opts.write) write(p.indexPath, text)
    }
  }

  if (!opts.quiet) {
    if (!changes.length) console.log('render: nothing to update')
    else console.log(`${opts.write ? 'rendered' : 'would render'}: ${[...new Set(changes)].join(', ')}`)
  }
  return changes
}

function sliceTable(f) {
  const glyph = { done: '✅ done', building: '🔨 building', blocked: '⛔ blocked', pending: 'pending' }
  const rows = f.slices.map((s) => {
    const deps = (s.fm?.depends_on ?? []).map(pad2).join(', ') || '—'
    return `| [${pad2(s.number)} — ${s.title}](${s.file}) | ${glyph[s.fm?.status] ?? s.fm?.status ?? '?'} | ${deps} |`
  })
  return ['| Slice | Status | Depends on |', '|---|---|---|', ...rows].join('\n')
}

function replaceMarked(text, name, body) {
  const open = `<!-- docs:${name} -->`
  const close = `<!-- /docs:${name} -->`
  const i = text.indexOf(open)
  const j = text.indexOf(close)
  if (i === -1 || j === -1 || j < i) return null
  return `${text.slice(0, i + open.length)}\n${body}\n${text.slice(j)}`
}

// ─────────────────────────────────────────────────────────────────────── init

function init(root) {
  const docs = path.join(root, 'docs')
  const made = []
  for (const [tpl, dest] of [['index.md', 'index.md'], ['ideas.md', 'ideas.md']]) {
    const to = path.join(docs, dest)
    if (exists(to)) continue
    write(to, read(path.join(TEMPLATES, tpl)))
    made.push(`docs/${dest}`)
  }
  fs.mkdirSync(path.join(docs, 'features'), { recursive: true })
  console.log(made.length ? `created ${made.join(', ')} and docs/features/` : 'already initialised')
  console.log('next: append the convention block to CLAUDE.md / AGENTS.md (see the feature-docs skill)')
  console.log('      declare the verify gate — `docs gate set "<cmd>"`, or /guardrails to build one')
}

// ──────────────────────────────────────────────────────────────────────── new

/**
 * Allocate the next PRD number. Worktrees of one repo share the common git dir,
 * so the counter there is visible to every worktree while staying untracked —
 * which is exactly what stops two parallel agents both claiming the same number,
 * without creating a file that can itself conflict.
 */
function allocateNumber(p, root) {
  const computed = nextNumbers(p).prd
  const common = git(['rev-parse', '--git-common-dir'], root)
  if (!common) return { number: computed, shared: false }
  const file = path.isAbsolute(common) ? path.join(common, 'docs-next-number') : path.join(root, common, 'docs-next-number')
  let reserved = 0
  try {
    reserved = Number(read(file).trim()) || 0
  } catch {}
  const number = Math.max(computed, reserved)
  fs.writeFileSync(file, String(number + 1))
  return { number, shared: true }
}

function newFeature(root, slug, opts) {
  if (!slug) die('usage: docs new <slug> [--title "Title"]')
  const p = loadProject(root)
  if (!exists(p.indexPath)) die('no docs/index.md — run `docs init` first')
  const { number, shared } = allocateNumber(p, root)
  const dir = path.join(p.docs, 'features', `${pad(number)}-${slug}`)
  if (exists(dir)) die(`${rel(p, dir)} already exists`)

  const title = opts.title ?? slug.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
  const fr = nextNumbers(p).fr
  const gate = gateStatus(root, p)
  const body = read(path.join(TEMPLATES, 'prd.scaffold.md'))
    .replaceAll('{{NNN}}', pad(number))
    .replaceAll('{{SLUG}}', slug)
    .replaceAll('{{TITLE}}', title)
    .replaceAll('{{DATE}}', today())
    .replaceAll('{{FR}}', String(fr))
    .replaceAll('{{GATE}}', gate.declared ? `\`${gate.declared}\` *(declared in index.md)*` : '_(none declared — run /guardrails before slicing)_')
  write(path.join(dir, 'prd.md'), body)
  // The log exists from birth, so every reader and writer — a builder's Traps, /deliver,
  // `docs log merge`, the factory driver — finds the file and its sections in one shape.
  write(path.join(dir, 'agent-log.md'), read(path.join(TEMPLATES, 'agent-log.md')).replaceAll('{NNN}', pad(number)))

  // index row, inserted at the end of the Live features table
  let text = read(p.indexPath)
  const table = parseTable(text, /live features/i)
  const row = `| ${pad(number)} | [${title}](features/${pad(number)}-${slug}/prd.md) | **draft** | created ${today()} | FR-${fr}… | _(fill in)_ |`
  if (table && table.rows.length) {
    const lines = text.split('\n')
    const last = table.rows[table.rows.length - 1]
    const placeholder = /^\|\s*—?\s*\|/.test(last.raw) || /none yet/i.test(last.raw)
    if (placeholder) lines[last.lineNo] = row
    else lines.splice(last.lineNo + 1, 0, row)
    text = lines.join('\n')
    write(p.indexPath, text)
  } else {
    console.error('  ! could not find the Live features table — add the row by hand')
  }

  render(root, { write: true, quiet: true })
  console.log(`created docs/features/${pad(number)}-${slug}/prd.md, agent-log.md`)
  console.log(`PRD-${pad(number)} · next free FR: FR-${fr}${shared ? '' : ' (no git — number not reserved across worktrees)'}`)
  console.log(gate.declared ? `gate: ${gate.declared}` : '⚠ no verify gate declared — mention /guardrails in the hand-off (`docs gate --candidates`)')
}

// ────────────────────────────────────────────────────────────────────── slice

function newSlice(root, ref, subject, opts) {
  if (!ref || !subject) die('usage: docs slice <NNN|slug> <subject-kebab> [--depends 1,2] [--title "..."]')
  const p = loadProject(root)
  const f = resolveFeature(p, ref)
  const number = f.slices.reduce((a, s) => Math.max(a, s.number), 0) + 1
  const deps = (opts.depends ?? '').split(',').map((s) => s.trim()).filter(Boolean).map(Number)
  for (const d of deps) {
    if (!f.slices.some((s) => s.number === d)) die(`slice ${d} does not exist in ${f.dirName}`)
  }
  const title = opts.title ?? subject.replace(/-/g, ' ').replace(/\b\w/g, (c) => c.toUpperCase())
  const file = path.join(f.dir, `slice-${pad2(number)}-${subject}.md`)
  if (exists(file)) die(`${rel(p, file)} already exists`)

  const gate = gateStatus(root, p)
  const body = read(path.join(TEMPLATES, 'slice.scaffold.md'))
    .replaceAll('{{NN}}', pad2(number))
    .replaceAll('{{TITLE}}', title)
    .replaceAll('{{DATE}}', today())
    .replaceAll('{{DEPENDS}}', deps.length ? `[${deps.join(', ')}]` : '[]')
    .replaceAll('{{DEPENDS_PROSE}}', deps.length ? deps.map(pad2).join(', ') : '—')
    .replaceAll('{{GATE}}', p.inner ? `\`${p.inner}\`` : gate.declared ? `\`${gate.declared}\`` : '_(no gate declared — run /guardrails, or `docs gate set "<cmd>"`, then name it here)_')
  write(file, body)
  render(root, { write: true, quiet: true })
  console.log(`created ${rel(p, file)}`)
  if (!gate.declared) console.log('⚠ no verify gate declared — the Verify task holds a placeholder. Run /guardrails or `docs gate set "<cmd>"` before building.')
  else if (!p.inner) console.log('⚠ no inner loop declared — the Verify task runs the full gate on every slice. `docs gate set --inner "<cmd>"` (lint, types, unit) before building.')
  else if (gate.status === 'missing') console.log(`⚠ gate "${gate.declared}" — ${gate.detail}`)
}

// ───────────────────────────────────────────────────────────────────── status

function statusCmd(root, ref, opts = {}) {
  const p = loadProject(root)
  if (ref) return featureStatus(root, p, resolveFeature(p, ref), opts)
  const want = nextNumbers(p)
  if (opts.json) {
    const brief = (f) => ({ number: f.number, slug: f.slug, status: computedStatus(f), slices: f.slices.length, done: f.slices.filter((x) => x.fm?.status === 'done').length, blocked: f.slices.filter((x) => x.fm?.status === 'blocked').length })
    console.log(JSON.stringify({ framework: p.version, live: p.features.map(brief), archived: p.archived.map(brief), next: want }, null, 2))
    return
  }
  if (p.version !== FRAMEWORK_VERSION) {
    console.log(`⚠ framework v${p.version || 1}, current is v${FRAMEWORK_VERSION} — run \`docs doctor\`\n`)
  }
  const line = (f) => {
    const st = computedStatus(f) ?? '?'
    const blocked = f.slices.filter((x) => x.fm?.status === 'blocked').length
    const s = f.slices.length ? ` · slices ${f.slices.filter((x) => x.fm?.status === 'done').length}/${f.slices.length} done${blocked ? `, ${blocked} blocked` : ''}` : ''
    const fr = f.fm?.owns_fr?.length ? ` · FR-${formatFr(expandFr(f.fm.owns_fr)).join(', FR-')}` : ''
    return `  ${pad(f.number)}  ${st.padEnd(12)} ${f.slug}${s}${fr}`
  }
  console.log('Live')
  console.log(p.features.length ? p.features.map(line).join('\n') : '  (none)')
  console.log('\nArchived')
  console.log(p.archived.length ? p.archived.map(line).join('\n') : '  (none)')
  console.log(`\nNext free PRD: ${pad(want.prd)} · next free FR: FR-${want.fr}`)
}

/**
 * One feature's state, computed from disk — what an unattended run reads to know
 * where it is, so a resume after a crash, a compaction or a blocker never depends
 * on the conversation that started it.
 */
function featureState(root, p, f) {
  const slices = f.slices.map((s) => ({ number: s.number, file: s.rel, title: s.title, status: s.fm?.status ?? null, lane: s.fm?.lane ?? null, depends_on: (s.fm?.depends_on ?? []).map(Number) }))
  const done = new Set(slices.filter((s) => s.status === 'done').map((s) => s.number))
  // A slice already building is the resume point; otherwise the lowest pending one whose deps are done.
  const next = slices.find((s) => s.status === 'building') ?? slices.find((s) => s.status === 'pending' && s.depends_on.every((d) => done.has(d))) ?? null
  // `ready` is every slice that could start right now, in slice order — `next` is its
  // head, kept so a one-lane run and an older caller read the same field as before.
  const building = slices.filter((s) => s.status === 'building')
  const ready = slices.filter((s) => s.status === 'pending' && s.depends_on.every((d) => done.has(d)))
  // A blocked slice holds its lane too: its half-finished work still owns the lane's
  // database, ports and scratch paths, and the owner's answer resumes it there. Free
  // that lane and the next builder lands beside the parked work.
  const heldLanes = slices.filter((s) => s.status === 'building' || s.status === 'blocked').map((s) => s.lane).filter((x) => x !== null)
  const freeLanes = []
  for (let i = 1; i <= p.lanes; i++) if (!heldLanes.includes(i)) freeLanes.push(i)
  const entries = f.log?.entries ?? []
  // Checkpoints: the full gate + Prove over every done slice since the last one.
  const cps = entries.filter((e) => e.kind === 'CP')
  const lastCp = cps.reduce((a, e) => (e.slice ?? 0) > a ? e.slice : a, 0)
  const doneOrder = slices.filter((s) => s.status === 'done').map((s) => s.number)
  const unverified = doneOrder.filter((n) => n > lastCp)
  const allDone = slices.length > 0 && slices.every((s) => s.status === 'done')
  return {
    number: f.number,
    slug: f.slug,
    dir: rel(p, f.dir),
    kind: f.kind,
    status: computedStatus(f),
    reviewed: f.fm?.reviewed ?? null,
    gate: gateStatus(root, p),
    qaPlan: !!f.hasQaPlan,
    harnessLine: f.harnessLine ?? null,
    harness: harnessStatus(root),
    rubric: exists(path.join(p.docs, 'ux-rubric.md')),
    sliced: slices.length > 0,
    allDone,
    slices,
    checkpointEvery: p.checkpointEvery,
    checkpoints: cps.map((e) => ({ n: e.n, after: e.slice })),
    unverified,
    checkpointDue: checkpointDueFor(unverified, p, allDone),
    next: next ? { number: next.number, file: next.file, status: next.status } : null,
    ready: ready.map((s) => ({ number: s.number, file: s.file })),
    building: building.map((s) => ({ number: s.number, file: s.file, lane: s.lane })),
    lanes: p.lanes,
    freeLanes,
    // What the orchestrator may spawn this round. A checkpoint needs the branch still,
    // so it drains first: never start a slice when one is due.
    spawnable: checkpointDueFor(unverified, p, allDone) ? 0 : Math.min(freeLanes.length, ready.length),
    blocked: slices.filter((s) => s.status === 'blocked').map((s) => s.number),
    openBlocks: entries.filter((e) => e.kind === 'BLOCK' && e.state === 'open').map((e) => ({ n: e.n, slice: e.slice, phase: e.phase, title: e.title })),
    openOpens: f.openOpens ?? [],
    log: {
      decisions: entries.filter((e) => e.kind === 'AD').length,
      flagged: entries.filter((e) => e.kind === 'AD' && e.review).map((e) => e.n),
      decide: entries.filter((e) => e.kind === 'AD' && e.review === 'decide').map((e) => e.n),
      blocks: entries.filter((e) => e.kind === 'BLOCK').length,
    },
    retro: f.retro?.data ?? null,
    branch: git(['rev-parse', '--abbrev-ref', 'HEAD'], root),
  }
}

function checkpointDueFor(unverified, p, allDone) {
  return unverified.length > 0 && (unverified.length >= p.checkpointEvery || allDone)
}

function featureStatus(root, p, f, opts = {}) {
  const s = featureState(root, p, f)
  if (opts.json) return console.log(JSON.stringify(s, null, 2))
  const slices = s.slices.map((x) => `${pad2(x.number)} ${x.status}${x.status === 'pending' && x.depends_on.length ? ` (deps ${x.depends_on.map(pad2).join(', ')})` : ''}`)
  console.log(`PRD-${pad(s.number)} ${s.slug} · ${s.status}${s.reviewed ? ` · reviewed ${s.reviewed}` : ''}`)
  console.log(`gate      ${gateLine(s.gate)}`)
  console.log(`qa plan   ${s.qaPlan ? 'yes' : 'none'}${s.harnessLine ? ` · Harness: ${s.harnessLine.slice(0, 90)}${s.harnessLine.length > 90 ? '…' : ''}` : ''}`)
  console.log(`slices    ${slices.length ? slices.join(' · ') : 'not sliced'}`)
  console.log(`next      ${s.next ? `slice-${pad2(s.next.number)} (${s.next.status})` : s.allDone ? 'every slice done' : s.sliced ? 'nothing runnable' : '—'}`)
  if (s.lanes > 1 && s.sliced) console.log(`lanes     ${s.lanes} · free ${s.freeLanes.length ? s.freeLanes.join(', ') : 'none'} · spawnable now ${s.spawnable}${s.checkpointDue ? ' (checkpoint due — drain first)' : ''}`)
  if (s.sliced) console.log(`verify    ${s.checkpoints.length} checkpoint(s)${s.unverified.length ? ` · unverified since: ${s.unverified.map(pad2).join(', ')}` : ''}${s.checkpointDue ? ' · CHECKPOINT DUE' : ''} (every ${s.checkpointEvery})`)
  if (s.openBlocks.length) console.log(`blocks    ${s.openBlocks.map((b) => `BLOCK-${b.n} ${b.slice !== null ? `slice-${pad2(b.slice)}` : b.phase ?? ''} — ${b.title}`).join(' · ')}`)
  if (s.openOpens.length) console.log(`opens     ${s.openOpens.map((n) => `OPEN-${n}`).join(', ')}`)
  if (f.log) console.log(`log       ${s.log.decisions} decision(s)${s.log.flagged.length ? ` (${s.log.flagged.length} for review, ${s.log.decide.length} need a decision)` : ''}, ${s.log.blocks} blocker(s)`)
  if (s.retro) console.log(`retro     score ${s.retro.score ?? '?'}`)
  console.log(`branch    ${s.branch ?? '(not a git repo)'}`)
}

// ────────────────────────────────────────────────────────────────────── lanes

const LANES_DIR = ['docs', 'reference', 'lanes']

/**
 * A lane is one agent's private copy of everything the suite cannot share — its
 * database, its ports, its scratch paths. Two agents in two lanes run the gate as two
 * separate processes, so a suite that cannot run `--parallel` works here anyway:
 * worker-safety is about workers sharing one process group, which lanes never do.
 *
 * The frontmatter is what the orchestrator injects; the prose under it is what the
 * agent has to know and no schema could hold — which shared file it must not touch,
 * which queue is its own. Both halves matter, which is why this is a file per lane
 * and not a template string in index.md.
 */
function lanesOnDisk(root) {
  const dir = path.join(root, ...LANES_DIR)
  if (!exists(dir)) return []
  return fs
    .readdirSync(dir)
    .filter((f) => /^lane-\d+\.md$/.test(f))
    .map((f) => {
      const { data, body } = parseFrontmatter(read(path.join(dir, f)))
      return { file: path.join(...LANES_DIR, f), n: Number(f.match(/\d+/)[0]), fm: data ?? {}, hasProse: body.trim().length > 0 }
    })
    .sort((a, b) => a.n - b.n)
}

function laneTemplate(n, env, setup) {
  return `---
lane: ${n}
env: [${env.join(', ')}]
setup: ${setup ?? ''}
---

This lane is yours for the whole slice. Nothing here is shared with a sibling lane.

Run \`setup\` above before the first test run; it is written to be safe to run twice.

**What this lane does not isolate** — name every shared path a sibling could be writing
at the same time (a build output, a manifest, a fixture directory), and what to do
instead of touching it. Delete this paragraph only when the answer is genuinely nothing.
`
}

function lanes(root, positional, flags) {
  const p = loadProject(root)
  if (!p.indexText) die('not on the framework — migrate first (feature-docs skill)')
  const sub = positional[1]
  if (sub === 'set') {
    const n = Number(positional[2])
    if (!Number.isInteger(n) || n < 1) die('usage: docs lanes set <N> [--env "K=V,K=V"] [--setup "<cmd>"]')
    const env = String(flags.env ?? '').split(',').map((x) => x.trim()).filter(Boolean)
    if (!env.length) die('--env is required: what makes this lane private, as K=V pairs with {n} for the lane number\n  e.g. --env "DB_DATABASE=app_lane_{n},REDIS_DB={n}"')
    const dir = path.join(root, ...LANES_DIR)
    fs.mkdirSync(dir, { recursive: true })
    const written = []
    for (let i = 1; i <= n; i++) {
      const f = path.join(dir, `lane-${i}.md`)
      // Never overwrite: the prose half is hand-written and is the point of the file.
      if (exists(f)) continue
      write(f, laneTemplate(i, env.map((e) => e.replaceAll('{n}', String(i))), flags.setup ? String(flags.setup).replaceAll('{n}', String(i)) : ''))
      written.push(path.join(...LANES_DIR, `lane-${i}.md`))
    }
    setIndexGate(p.indexPath, String(n), 'lanes', { raw: true })
    console.log(`lanes: ${n}`)
    for (const f of written) console.log(`  + ${f}`)
    const kept = n - written.length
    if (kept > 0) console.log(`  · ${kept} existing lane file(s) left as they are`)
    console.log('\nNow fill each file\'s prose: what the lane does NOT isolate, and what an agent must not touch while a sibling runs.')
    return
  }
  const disk = lanesOnDisk(root)
  console.log(`declared      lanes: ${p.lanes}${p.lanes < 2 ? ' (no parallelism)' : ''}`)
  console.log(`on disk       ${disk.length ? disk.map((l) => `lane-${l.n}`).join(', ') : 'none'}`)
  for (const l of disk) {
    const env = Array.isArray(l.fm.env) ? l.fm.env : l.fm.env ? [l.fm.env] : []
    console.log(`  lane-${l.n}      ${env.length ? env.join(' ') : '! no env — this lane isolates nothing'}${l.hasProse ? '' : '  ! no prose'}`)
  }
  // lanes: 1 is the default and means "no parallelism" — it wants no lane file at all.
  const missing = []
  if (p.lanes >= 2) for (let i = 1; i <= p.lanes; i++) if (!disk.some((l) => l.n === i)) missing.push(i)
  if (missing.length) console.log(`\n! lanes: ${p.lanes} declared but lane-${missing.join(', lane-')} missing — \`docs lanes set ${p.lanes} --env "..."\``)
  else if (p.lanes < 2) console.log('\ndeclare lanes with `docs lanes set <N> --env "DB_DATABASE=app_lane_{n}"` — {n} is the lane number')
}

// ────────────────────────────────────────────────────────── concurrent log

/**
 * Split a log into `## Heading` sections, keeping the preamble under `''`.
 * Fence-aware for the same reason parseAgentLog is: the template's example
 * shapes sit inside fences and must not be read as structure.
 */
function logSections(text) {
  const out = new Map([['', []]])
  let key = ''
  let fenced = false
  for (const line of text.split('\n')) {
    if (/^\s*```/.test(line)) fenced = !fenced
    const h = !fenced && line.match(/^##\s+(.+?)\s*$/)
    if (h) { key = h[1]; if (!out.has(key)) out.set(key, []) ; continue }
    out.get(key).push(line)
  }
  return out
}

function joinSections(sections) {
  const parts = []
  for (const [key, lines] of sections) {
    const body = lines.join('\n').trim()
    if (key === '') { if (body) parts.push(body) ; continue }
    parts.push(`## ${key}${body ? `\n\n${body}` : ''}`)
  }
  // Appending section to section leaves runs of blank lines wherever a fragment had
  // padding of its own; one blank line is the whole house style.
  return `${parts.join('\n\n')}\n`.replace(/\n{3,}/g, '\n\n')
}

/**
 * Concurrent builders cannot all append to one `agent-log.md` — last writer wins and
 * the losers' decisions vanish silently, which is the worst possible failure for the
 * one file whose whole job is to say what the run decided. So each builder writes its
 * own `log-slice-NN.md` fragment and the orchestrator merges them at a checkpoint,
 * while no builder is running. Fragments are ignored by `docs check` (a feature folder
 * only recognises `slice-NN-*.md`), so a crashed run leaves its decisions on disk
 * rather than in a dead subagent's context.
 *
 * Merge validates numbering, it does not fix it. The orchestrator hands each builder a
 * reserved AD range; a collision means a builder ignored its range, and renumbering
 * here would quietly break every "see AD-12" written inside an entry. Refuse instead.
 */
function logMerge(root, ref, opts = {}) {
  if (!ref) die('usage: docs log merge <NNN|slug> [--dry]')
  const p = loadProject(root)
  const f = resolveFeature(p, ref, 'live')
  const dir = path.join(root, 'docs', 'features', f.dirName)
  const logPath = path.join(dir, 'agent-log.md')
  if (!exists(logPath)) die(`no agent-log.md in ${f.dirName} — nothing to merge into`)

  const frags = fs
    .readdirSync(dir)
    .filter((n) => /^log-(slice|fix)-\d+\.md$/.test(n))
    // Slices in slice order, then polish's fixers: the order the run happened in, so a
    // reader of agent-log.md walks the feature forward instead of by filename.
    .map((n) => ({ n, kind: n.startsWith('log-slice-') ? 0 : 1, num: Number(n.match(/\d+/)[0]) }))
    .sort((a, b) => a.kind - b.kind || a.num - b.num)
    .map((x) => x.n)
  if (!frags.length) { console.log(`no fragments in ${f.dirName} — nothing to merge`); return }

  const base = read(logPath)
  const taken = new Map(parseAgentLog(base).entries.map((e) => [`${e.kind}-${e.n}`, 'agent-log.md']))
  const sections = logSections(base)
  const merged = []
  const clash = []

  for (const name of frags) {
    const text = read(path.join(dir, name))
    for (const e of parseAgentLog(text).entries) {
      const id = `${e.kind}-${e.n}`
      if (taken.has(id)) clash.push(`${id} in ${name} is already in ${taken.get(id)}`)
      else taken.set(id, name)
    }
    for (const [key, lines] of logSections(text)) {
      const body = lines.join('\n').trim()
      if (!body) continue
      if (key === '') { clash.push(`${name} has content above its first \`## \` heading — every line must sit under the section it belongs in`); continue }
      if (!sections.has(key)) sections.set(key, [])
      sections.get(key).push('', body)
    }
    merged.push(name)
  }

  if (clash.length) {
    for (const c of clash) console.error(`  ✗ ${c}`)
    die(`${clash.length} problem(s) — nothing merged. Each builder gets a reserved AD range; fix the fragment and re-run.`)
  }

  if (opts.dry) {
    console.log(`would merge ${merged.length} fragment(s) into ${f.dirName}/agent-log.md:`)
    for (const n of merged) console.log(`  ${n}`)
    return
  }
  write(logPath, joinSections(sections))
  for (const n of merged) fs.unlinkSync(path.join(dir, n))
  console.log(`merged ${merged.length} fragment(s) into ${f.dirName}/agent-log.md`)
  for (const n of merged) console.log(`  - ${n}`)
}

// ────────────────────────────────────────────────────────────────── preflight

function defaultBranch(root) {
  const head = git(['symbolic-ref', '--short', 'refs/remotes/origin/HEAD'], root)
  if (head) return head.replace(/^origin\//, '')
  for (const b of ['main', 'master']) if (git(['rev-parse', '--verify', '--quiet', `refs/heads/${b}`], root) !== null) return b
  return null
}

/**
 * Can /deliver start (or resume) this feature unattended? Refuses only on what a
 * machine can decide. Open OPENs and the Harness line are listed for the agent to
 * judge: OPEN formats vary across real PRDs, and whether one is load-bearing is
 * never the tool's call.
 *
 * Three buckets, not two. `refuse` is "this run cannot start". `judge` is "the agent
 * decides alone". `ask` is new and sits between them: a recurring cost the owner can
 * delete once, in minutes, that the run would otherwise pay on every slice. An
 * unattended run has every incentive to nod past an advisory line — icg-cloud ran
 * /deliver three times against the standing "no inner loop declared" judge line and
 * never declared one — so `ask` blocks (exit 2) and names its measured cost. The
 * moment /deliver starts is the cheapest interrupt in the system: the owner is at
 * the keyboard, and the alternative is paying the cost for the next eleven hours.
 */
function preflight(root, ref, opts = {}) {
  if (!ref) die('usage: docs preflight <NNN|slug> [--json]')
  const p = loadProject(root)
  const refuse = []
  const ask = []
  const judge = []
  const ok = []

  if (!p.indexText) refuse.push('not on the framework — migrate first (feature-docs skill)')
  else if (p.version !== FRAMEWORK_VERSION) refuse.push(`framework v${p.version || 1}, current is v${FRAMEWORK_VERSION} — upgrade first (\`docs doctor\`)`)
  else {
    const { report } = check(root)
    if (report.errors.length) refuse.push(`docs check has ${report.errors.length} error(s) (${[...new Set(report.errors.map((e) => e.code))].join(', ')}) — fix drift first`)
    else ok.push('docs check: no errors')
  }

  const n = /^\d+$/.test(ref) ? Number(ref) : null
  const f = p.features.find((x) => (n !== null ? x.number === n : x.slug === ref || x.dirName === ref))
  const archived = !f && p.archived.find((x) => (n !== null ? x.number === n : x.slug === ref || x.dirName === ref))
  if (!f) {
    refuse.push(archived ? `PRD-${pad(archived.number)} is archived — nothing to deliver` : `no live feature matching "${ref}"`)
  } else {
    const st = computedStatus(f)
    // A /deliver run ships in its closeout commit, then still has the PR, CI and the
    // owner's review ahead of it — so shipped with a written agent log is a run to
    // resume. Written, not present: `docs new` creates the log empty, and a run always
    // leaves at least the CP entry of its last checkpoint.
    if (st === 'shipped' && f.log?.entries.length) ok.push('status shipped by /deliver — resumes at the PR, CI or owner review')
    else if (!['draft', 'ready', 'in-progress'].includes(st)) refuse.push(`PRD-${pad(f.number)} is ${st} — nothing to deliver`)
    else ok.push(`status ${st}${st === 'draft' ? ' — becomes ready when this preflight passes' : ''}`)
    // reviewed: records /polish over the code. Set before any code exists, it makes a
    // resumed run skip polish — a PRD review once set it (arca PRD-012).
    if (['draft', 'ready'].includes(st) && f.fm?.reviewed) refuse.push(`reviewed: ${f.fm.reviewed} is set but nothing is built — it marks the code's /polish pass, not a PRD review; remove it from prd.md frontmatter`)
    if (!f.hasQaPlan) refuse.push('no QA plan section — every FR needs a proof row first (/prd)')
    else ok.push('QA plan present')
    const g = gateStatus(root, p)
    if (g.status === 'none') refuse.push('no verify gate declared — /guardrails builds one (an interview, never unattended)')
    else if (g.status === 'missing') refuse.push(`gate ${gateLine(g)}`)
    else ok.push(`gate ${gateLine(g)}`)
    const openBlocks = (f.log?.entries ?? []).filter((e) => e.kind === 'BLOCK' && e.state === 'open')
    if (openBlocks.length) refuse.push(`open blocker(s): ${openBlocks.map((b) => `BLOCK-${b.n}`).join(', ')} — resolve in agent-log.md before resuming`)
    if (f.openOpens?.length) judge.push(`unstruck ${f.openOpens.map((x) => `OPEN-${x}`).join(', ')} — a load-bearing one blocks delivery; a deliberately parked one may stay`)
    const h = harnessStatus(root)
    judge.push(`harness on disk: ${h.present ? h.detail : 'none'} · QA plan says: ${f.harnessLine ?? '(no Harness line)'}`)
    if (!exists(path.join(p.docs, 'ux-rubric.md'))) judge.push('no docs/ux-rubric.md — a UI diff gets the mechanical UX checks only')
    if (g.status !== 'none' && !p.inner) ask.push('no inner loop declared — every slice runs the full gate. Declare the fast subset once (`docs gate set --inner "<cmd>"`) and every slice of every future run skips the slow tiers.')
    // Lanes are what let two agents run the suite at once: each gets its own database,
    // ports and scratch paths. Separate processes, so a suite that cannot run --parallel
    // is still fine here — worker-safety is a different problem (below).
    const width = f.slices.length ? parallelWidth(f.slices) : 0
    if (width >= 2 && p.lanes < 2) ask.push(`${f.slices.length} slices, up to ${width} of them runnable at once by depends_on, but \`lanes: ${p.lanes}\` — they will build one after another. Defining lanes is a one-time interview per project (\`docs lanes\`).`)
    else if (width >= 2) ok.push(`lanes ${p.lanes} · graph allows ${width} at once`)
    // Only a measured false blocks. Absent means nobody has looked, and nagging every
    // project about a fact it has never measured is how an ask bucket gets ignored.
    if (p.gateParallel === false) ask.push('the gate cannot run parallel workers (`gate_parallel: false`) — every gate run pays full serial wall-clock, and a run pays it dozens of times. One pass at making the suite worker-safe cuts every future run.')
    else if (p.gateParallel === null && g.status !== 'none') judge.push('gate_parallel not measured — `/guardrails audit` times the suite serial and parallel and writes the answer to index.md')
  }

  // Either naming style counts — projects use both, and refusing the run's own branch cost a real run.
  const branch = { current: null, default: null, feature: f ? `feat/${f.slug}` : null, alt: f ? `feat/${pad(f.number)}-${f.slug}` : null, featureExists: false }
  if (git(['rev-parse', '--is-inside-work-tree'], root) !== 'true') {
    refuse.push('not a git repository')
  } else {
    const dirty = git(['status', '--porcelain'], root, { trim: false })
    const building = f ? f.slices.filter((s) => s.fm?.status === 'building') : []
    const dirtyCount = dirty ? dirty.split('\n').filter(Boolean).length : 0
    // A builder that died mid-slice leaves its work uncommitted; refusing the tree would
    // make the run unresumable, so /deliver judges whether the diff is that builder's.
    if (dirty && building.length) judge.push(`working tree not clean (${dirtyCount} path(s)) while slice ${building.map((s) => pad2(s.number)).join(', ')} is building — likely a builder that died mid-slice; hand its diff to a fresh builder, and stop if any path is not that slice's`)
    else if (dirty) refuse.push(`working tree not clean (${dirtyCount} path(s)) — commit or stash first`)
    else ok.push('working tree clean')
    const origin = git(['remote', 'get-url', 'origin'], root)
    if (!origin) refuse.push('no origin remote — the run ends in a pull request')
    else ok.push(`origin ${origin}`)
    branch.current = git(['rev-parse', '--abbrev-ref', 'HEAD'], root)
    branch.default = defaultBranch(root)
    for (const b of [branch.feature, branch.alt]) {
      if (b && git(['rev-parse', '--verify', '--quiet', `refs/heads/${b}`], root) !== null) { branch.feature = b; branch.featureExists = true; break }
    }
    if (branch.feature && [branch.feature, branch.alt].includes(branch.current)) { branch.feature = branch.current; ok.push(`on ${branch.current} — resuming`) }
    else if (branch.current === branch.default) judge.push(`on ${branch.default} — ${branch.featureExists ? `switch to existing ${branch.feature}` : `create ${branch.feature}`}`)
    else if (branch.feature) refuse.push(`on ${branch.current}, which is neither ${branch.default ?? 'the default branch'} nor ${branch.feature} (or ${branch.alt}) — the owner decides which branch this feature lives on`)
  }

  // `pass` stays what it always was — nothing mechanical refuses this run — so a caller
  // that only reads `pass` keeps working. Asks are reported separately and exit 2, which
  // /deliver treats as "stop and put these to the owner", not as a broken project.
  const result = { feature: f ? { number: f.number, slug: f.slug, status: computedStatus(f) } : null, pass: refuse.length === 0, refuse, ask, judge, ok, branch }
  if (opts.json) {
    console.log(JSON.stringify(result, null, 2))
  } else {
    console.log(`preflight ${f ? `PRD-${pad(f.number)} ${f.slug}` : ref}`)
    for (const x of ok) console.log(`  ✓ ${x}`)
    for (const x of refuse) console.log(`  ✗ ${x}`)
    for (const x of ask) console.log(`  ! ${x}`)
    for (const x of judge) console.log(`  ? ${x}`)
    if (!result.pass) console.log(`\nrefused: ${refuse.length} blocker(s)`)
    else if (ask.length) console.log(`\n${ask.length} ask(s) for the owner — answer them before starting; each is a one-time fix the run would otherwise pay for repeatedly`)
    else console.log('\nready — the ? lines are yours to judge before starting')
  }
  process.exit(!result.pass ? 1 : ask.length ? 2 : 0)
}

// ─────────────────────────────────────────────────────────────────────── ship

/** Rewrite a prd.md's frontmatter, preserving the zero-padded number. */
function patchPrdFrontmatter(prdPath, number, patch) {
  const { data, body } = parseFrontmatter(read(prdPath))
  if (!data) return false
  write(prdPath, stringifyFrontmatter({ ...data, number: pad(number), ...patch }) + body)
  return true
}

/**
 * Terminal status, set from inside the branch that ships the feature.
 * `shipped` means "this branch completes it" — on main it becomes true at the
 * exact moment the merge lands, which is the only moment anyone reads main.
 */
function ship(root, ref) {
  if (!ref) die('usage: docs ship <NNN|slug>')
  const p = loadProject(root)
  const f = resolveFeature(p, ref, 'live')
  if (!f.fm) die(`${rel(p, f.prdPath)} has no frontmatter — run \`docs doctor\``)
  if (f.fm.status === 'shipped') return console.log(`PRD-${pad(f.number)} is already shipped`)

  const open = f.slices.filter((s) => s.fm && s.fm.status !== 'done')
  if (open.length) {
    die(`refusing: ${open.length} slice(s) not done (${open.map((s) => pad2(s.number)).join(', ')}). Set them done first, or this claims work that did not land.`)
  }

  // Loud, never blocking — the one time there is a good reason to skip the review,
  // a refusal turns a helpful nudge into a fight.
  if (!f.fm.reviewed) {
    console.log(`⚠ PRD-${pad(f.number)} has no \`reviewed:\` date — it has not been through /polish.`)
    console.log('  Shipping anyway is your call; say so and it is fine. Otherwise run /polish first.\n')
  }

  patchPrdFrontmatter(f.prdPath, f.number, { status: 'shipped' })
  render(root, { write: true, quiet: true })
  git(['add', '--', 'docs'], root)
  console.log(`PRD-${pad(f.number)} → shipped, staged.`)
  console.log('Still yours to write, in this same commit:')
  console.log(`  · the prose **Status**: line in ${rel(p, f.prdPath)} — what actually landed`)
  console.log('  · the index Notes cell — decisions taken, what is left, the PR number')
  console.log('If nothing is left at all, run `docs archive` instead and skip the shipped state entirely.')
}

// ──────────────────────────────────────────────────────────────────── archive

function archive(root, ref) {
  if (!ref) die('usage: docs archive <NNN|slug>')
  const p = loadProject(root)
  const f = resolveFeature(p, ref, 'live')
  const open = f.slices.filter((s) => s.fm && s.fm.status !== 'done')
  if (open.length) die(`refusing: ${open.length} slice(s) not done (${open.map((s) => pad2(s.number)).join(', ')}). Archiving is feature-granular.`)

  const to = path.join(p.docs, 'archive', f.dirName)
  if (exists(to)) die(`${rel(p, to)} already exists`)
  fs.mkdirSync(path.join(p.docs, 'archive'), { recursive: true })
  const moved = git(['mv', path.relative(root, f.dir), path.relative(root, to)], root)
  if (moved === null) fs.renameSync(f.dir, to)

  // status → archived, in the frontmatter that travelled with the folder
  const prd = path.join(to, 'prd.md')
  if (patchPrdFrontmatter(prd, f.number, { status: 'archived' })) {
    // Stage it beside the rename — otherwise the move is staged and the status edit is not.
    git(['add', '--', path.relative(root, prd)], root)
  }

  // move the index row from Live features to Archived
  let idx = read(p.indexPath)
  const live = parseTable(idx, /live features/i)
  const row = live?.rows.find((x) => cellNumber(x.cells[0]) === f.number)
  if (row) {
    const lines = idx.split('\n')
    const cells = row.cells
    lines.splice(row.lineNo, 1)
    idx = lines.join('\n')
    const arch = parseTable(idx, /archived/i)
    const title = cells[1]?.match(/\[([^\]]+)\]/)?.[1] ?? f.slug
    const newRow = `| ${pad(f.number)} ${title} | [archive/${f.dirName}/](archive/${f.dirName}/prd.md) | ${cells[4] ?? '—'} | ${cells[5] ?? '—'} |`
    if (arch) {
      const l2 = idx.split('\n')
      const last = arch.rows[arch.rows.length - 1]
      if (last && (/^\|\s*—\s*\|/.test(last.raw) || /none yet/i.test(last.raw))) l2[last.lineNo] = newRow
      else if (last) l2.splice(last.lineNo + 1, 0, newRow)
      idx = l2.join('\n')
    }
    write(p.indexPath, idx)
  }
  render(root, { write: true, quiet: true })
  // Stage the index with the move, so the whole archiving is one coherent change.
  git(['add', '--', path.relative(root, p.indexPath)], root)
  console.log(`archived PRD-${pad(f.number)} → docs/archive/${f.dirName}/`)
  console.log('index row moved — check its Note reads as history, then commit. Never edit inside archive/ again.')
}

// ─────────────────────────────────────────────────────────────────── renumber

function renumber(root, from, to) {
  if (!from || !to) die('usage: docs renumber <old> <new>')
  const p = loadProject(root)
  const f = resolveFeature(p, from)
  const n = Number(to)
  if (p.all.some((x) => x.number === n)) die(`PRD-${pad(n)} already exists`)

  const oldDir = f.dirName
  const newDir = `${pad(n)}-${f.slug}`
  const parent = path.dirname(f.dir)
  const dest = path.join(parent, newDir)
  if (git(['mv', path.relative(root, f.dir), path.relative(root, dest)], root) === null) fs.renameSync(f.dir, dest)

  const prd = path.join(dest, 'prd.md')
  const text = read(prd)
  const { data, body } = parseFrontmatter(text)
  if (data) {
    write(prd, stringifyFrontmatter({ ...data, number: pad(n) }) + body.replaceAll(`PRD-${pad(f.number)}`, `PRD-${pad(n)}`))
  }
  // every reference anywhere in docs/
  let touched = 0
  for (const file of walkMd(p.docs)) {
    const t = read(file)
    const next = t.replaceAll(oldDir, newDir).replaceAll(`PRD-${pad(f.number)}`, `PRD-${pad(n)}`)
    if (next !== t) {
      write(file, next)
      touched++
    }
  }
  // The index row's number cell is a bare `| 001 |` — no path, no PRD- prefix, so
  // the textual sweep above cannot reach it.
  let idx = read(p.indexPath)
  const lines = idx.split('\n')
  for (const re of [/live features/i, /archived/i]) {
    const t = parseTable(idx, re)
    for (const row of t?.rows ?? []) {
      if (cellNumber(row.cells[0]) !== f.number) continue
      const cells = [...row.cells]
      cells[0] = cells[0].replace(/\d{1,4}/, pad(n))
      lines[row.lineNo] = `| ${cells.join(' | ')} |`
    }
  }
  write(p.indexPath, lines.join('\n'))
  render(root, { write: true, quiet: true })
  git(['add', '--', 'docs'], root)
  console.log(`renumbered ${oldDir} → ${newDir} (${touched} file(s) updated)`)
  console.log(`PRD-${pad(f.number)} is now retired and must never be reused — add it to the index's retired list.`)
}

// ───────────────────────────────────────────────────────────────── vendor/hook

function vendor(root, dest = '.docs') {
  const to = path.join(root, dest)
  fs.mkdirSync(path.join(to, 'bin'), { recursive: true })
  fs.copyFileSync(path.join(HERE, 'docs.mjs'), path.join(to, 'bin', 'docs.mjs'))
  fs.mkdirSync(path.join(to, 'templates'), { recursive: true })
  for (const f of fs.readdirSync(TEMPLATES)) fs.copyFileSync(path.join(TEMPLATES, f), path.join(to, 'templates', f))
  console.log(`vendored to ${dest}/ — CI can now run \`node ${dest}/bin/docs.mjs check\``)
}

const HOOK_MARKER = '# feature-docs hook'

function hookBody(root) {
  // Prefer a vendored copy so the hook keeps working for anyone who clones the repo.
  const vendored = path.join(root, '.docs', 'bin', 'docs.mjs')
  const bin = exists(vendored) ? '"$(git rev-parse --show-toplevel)"/.docs/bin/docs.mjs' : `"${path.join(HERE, 'docs.mjs')}"`
  return [
    HOOK_MARKER,
    '# docs/archive/** is frozen, and docs/ changes must lint.',
    'if git diff --cached --name-only --diff-filter=M | grep -q "^docs/archive/"; then',
    '  echo "docs: docs/archive/** is frozen — move folders, never edit files inside." >&2',
    '  exit 1',
    'fi',
    'if git diff --cached --name-only | grep -q "^docs/"; then',
    `  node ${bin} check || exit 1`,
    'fi',
  ].join('\n')
}

function hook(root, opts = {}) {
  if (!git(['rev-parse', '--is-inside-work-tree'], root)) die('not a git repository')
  const body = hookBody(root)
  const configured = git(['config', '--get', 'core.hooksPath'], root)

  // A configured hooksPath (often global) makes .git/hooks dead — installing there
  // would silently do nothing, which is worse than not installing at all.
  if (configured && !opts.here) {
    console.log(`git uses core.hooksPath = ${configured}, so .git/hooks is ignored.`)
    console.log(`\nAppend this to ${path.join(configured, 'pre-commit')} — it no-ops in repos with no staged docs/ changes:\n`)
    console.log(body)
    console.log('\nOr: `docs hook --here` points THIS repo at .git/hooks instead — which disables your global hooks for this repo.')
    return
  }
  if (opts.here) git(['config', 'core.hooksPath', '.git/hooks'], root)

  const gitDir = git(['rev-parse', '--git-common-dir'], root)
  const hooks = path.isAbsolute(gitDir) ? path.join(gitDir, 'hooks') : path.join(root, gitDir, 'hooks')
  fs.mkdirSync(hooks, { recursive: true })
  const file = path.join(hooks, 'pre-commit')
  if (exists(file)) {
    if (read(file).includes(HOOK_MARKER)) return console.log('hook already installed')
    return die(`${file} exists — append this by hand:\n\n${body}`)
  }
  write(file, `#!/bin/sh\n${body}\n`)
  fs.chmodSync(file, 0o755)
  console.log(`installed ${path.relative(root, file)}`)
}

// ───────────────────────────────────────────────────────────────────── helpers

const pad = (n) => String(n).padStart(3, '0')
const pad2 = (n) => String(n).padStart(2, '0')
const rel = (p, f) => path.relative(p.root, f)
const today = () => new Date().toISOString().slice(0, 10)
function die(msg) {
  console.error(msg)
  process.exit(2)
}
function resolveFeature(p, ref, kind) {
  const pool = kind === 'live' ? p.features : p.all
  const n = /^\d+$/.test(ref) ? Number(ref) : null
  const f = pool.find((x) => (n !== null ? x.number === n : x.slug === ref || x.dirName === ref))
  if (!f) die(`no feature matching "${ref}"${kind ? ` under ${kind}` : ''}`)
  return f
}

// Never take a value, so `docs status --json 013` cannot swallow the feature number.
const BOOLEAN_FLAGS = new Set(['json', 'fix', 'strict', 'write', 'here', 'candidates', 'inner'])

function parseArgs(argv) {
  const positional = []
  const flags = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a.startsWith('--')) {
      const [k, v] = a.slice(2).split('=')
      flags[k] = v ?? (!BOOLEAN_FLAGS.has(k) && argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[++i] : true)
    } else positional.push(a)
  }
  return { positional, flags }
}

const USAGE = `docs — bookkeeping for the feature-docs framework

  docs doctor [--json]            how far this project is from the current spec, and who fixes what
  docs doctor --scan <dirs...>    same, across every framework project under those roots
  docs check [--fix] [--strict]   verify index.md against the folders (exit 1 on error)
  docs status [NNN] [--json]      the board computed from disk; with NNN, one feature's state
                                  (next slice, blockers, open OPENs, log counts) for resuming a run
  docs preflight <NNN> [--json]   can /deliver start or resume this feature? (exit 1 refused, 2 asks pending)
  docs lanes                      build lanes: what is declared, what is on disk
  docs lanes set <N> --env "K=V"  declare N lanes and scaffold their files ({n} = lane number)
  docs log merge <NNN>            fold log-slice-NN.md / log-fix-N.md fragments into agent-log.md
  docs render [--write]           regenerate slice tables + index status cells + counters
  docs init                       scaffold docs/index.md, docs/ideas.md, docs/features/
  docs new <slug> [--title T]     allocate the next PRD number and scaffold the folder
  docs slice <NNN> <subject> [--depends 1,2] [--title T]
  docs gate                       print the verify gate (bare command; exit 1 if none declared)
  docs gate set "<cmd>"           declare it in docs/index.md frontmatter
  docs gate --inner               the fast per-slice subset (inner: in frontmatter), or the gate when none is declared
  docs gate set --inner "<cmd>"   declare the inner loop
  docs gate --candidates          likely gate commands found in this repo
  docs ship <NNN>                 mark the feature shipped — run it in the last commit of its PR
  docs archive <NNN>              git mv to archive/, set status, move the index row
  docs renumber <old> <new>       rename a feature and every reference to it
  docs vendor [dir]               copy this tool into the project (for CI)
  docs hook [--here]              install the pre-commit hook (frozen archive + check)

Judgment stays with /prd and /prd-to-tasks. This only does what a machine can decide.`

// ────────────────────────────────────────────────────────────────────── main

const { positional, flags } = parseArgs(process.argv.slice(2))
const cmd = positional[0]
const root = flags.root ? path.resolve(flags.root) : findRoot()

switch (cmd) {
  case 'check': {
    const { report } = check(root, { fix: !!flags.fix })
    if (!report.errors.length && !report.warnings.length) console.log('docs check: clean')
    else report.print()
    process.exit(report.errors.length || (flags.strict && report.warnings.length) ? 1 : 0)
    break
  }
  case 'doctor': {
    if (!flags.scan) {
      doctor(root, { json: !!flags.json })
      break
    }
    const roots = [typeof flags.scan === 'string' ? flags.scan : null, ...positional.slice(1)].filter(Boolean)
    scan(roots.length ? roots : [process.cwd()])
    break
  }
  case 'status':
    statusCmd(root, positional[1], { json: !!flags.json })
    break
  case 'lanes':
    lanes(root, positional, flags)
    break
  case 'log': {
    if (positional[1] !== 'merge') die('usage: docs log merge <NNN|slug> [--dry]')
    logMerge(root, positional[2], { dry: !!flags.dry })
    break
  }
  case 'preflight':
    preflight(root, positional[1], { json: !!flags.json })
    break
  case 'render':
    render(root, { write: !!flags.write })
    break
  case 'init':
    init(root)
    break
  case 'new':
    newFeature(root, positional[1], flags)
    break
  case 'slice':
    newSlice(root, positional[1], positional[2], flags)
    break
  case 'gate':
    gateCmd(root, positional, flags)
    break
  case 'ship':
    ship(root, positional[1])
    break
  case 'archive':
    archive(root, positional[1])
    break
  case 'renumber':
    renumber(root, positional[1], positional[2])
    break
  case 'vendor':
    vendor(root, positional[1])
    break
  case 'hook':
    hook(root, { here: !!flags.here })
    break
  default:
    console.log(USAGE)
    process.exit(cmd ? 2 : 0)
}
