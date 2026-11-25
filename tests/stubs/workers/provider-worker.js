'use strict'
// A non-Claude worker CLI for the cargo test harness: `codex`, `agy` and
// `opencode` beside this file, and `cursor-agent` one level up when it is
// called with `--force`. Only a test that puts this folder first on PATH
// reaches them, so a fixture's default PATH still holds no codex, agy or
// opencode.
//
// A worker call finds the prompt file the driver wrote (stdin for codex, the
// trailing "Read and follow all instructions in <file>." for cursor and agy,
// `-f <file>` for opencode), splits it at its last `\n---\n\n` into the pack
// and the prompt, and runs ../claude with `-p <prompt>
// --append-system-prompt-file $FACTORY_PACK` and the call's model and effort,
// so every scenario step the claude stub knows works here too. The claude
// stub's call record gains `provider`, `provider_argv` (this call's own argv),
// `prompt_file`, `prompt_ok` (the file's pack is byte for byte the pack at
// $FACTORY_PACK) and `path` ($PATH as the worker saw it). Its Claude stream is
// rewritten line by line into the provider's own stream shape; the exit code
// is the claude stub's.
//
// Any other call (`--version`, `models`) prints one line and exits 0.
const fs = require('node:fs')
const path = require('node:path')
const { spawn } = require('node:child_process')
const readline = require('node:readline')

const SEP = '\n---\n\n'

function isWorkerCall(provider, argv) {
  switch (provider) {
    case 'codex': return argv[0] === 'exec'
    case 'cursor': return argv.includes('--force')
    case 'agy': return argv.includes('-p')
    case 'opencode': return argv[0] === 'run'
  }
  return false
}

function flag(argv, ...names) {
  for (const n of names) {
    const i = argv.indexOf(n)
    if (i >= 0) return argv[i + 1]
  }
  return undefined
}

function promptFile(provider, argv) {
  if (provider === 'codex') {
    try {
      return fs.readlinkSync('/proc/self/fd/0')
    } catch {
      return null
    }
  }
  if (provider === 'opencode') return flag(argv, '-f') ?? null
  const m = /^Read and follow all instructions in (.+)\.$/.exec(argv[argv.length - 1] ?? '')
  return m ? m[1] : null
}

function modelAndEffort(provider, argv) {
  switch (provider) {
    case 'codex': {
      const c = argv.map((a, i) => (argv[i - 1] === '-c' ? a : null)).find((a) => a?.startsWith('model_reasoning_effort='))
      return [flag(argv, '-m'), c?.split('=')[1]]
    }
    case 'opencode': {
      const [model, effort] = String(flag(argv, '-m') ?? '').split('#')
      return [model, effort]
    }
    default:
      return [flag(argv, '--model'), flag(argv, '--effort')]
  }
}

module.exports = function run(provider, argv) {
  if (!isWorkerCall(provider, argv)) {
    process.stdout.write(`${provider} stub 1.0.0\n`)
    process.exit(0)
  }
  const file = promptFile(provider, argv)
  if (!file) {
    console.error(`stub ${provider}: no prompt file in ${JSON.stringify(argv)}`)
    process.exit(65)
  }
  const text = provider === 'codex' ? fs.readFileSync(0, 'utf8') : fs.readFileSync(file, 'utf8')
  const cut = text.lastIndexOf(SEP)
  if (cut < 0) {
    console.error(`stub ${provider}: ${file} has no pack separator`)
    process.exit(65)
  }
  const prompt = text.slice(cut + SEP.length).replace(/\n$/, '')
  const packPath = process.env.FACTORY_PACK
  const ok = !!packPath && fs.readFileSync(packPath, 'utf8') === text.slice(0, cut)
  const [model, effort] = modelAndEffort(provider, argv)
  const args = [path.join(__dirname, '..', 'claude'), '-p', prompt, '--append-system-prompt-file', packPath ?? '']
  if (model) args.push('--model', model)
  if (effort) args.push('--effort', effort)
  const env = {
    ...process.env,
    STUB_PROVIDER: provider,
    STUB_PROVIDER_ARGV: JSON.stringify(argv),
    STUB_PROMPT_FILE: file,
    STUB_PROMPT_OK: ok ? '1' : '0',
  }
  // Only Claude keeps transcripts there; a non-Claude worker leaves none.
  delete env.CLAUDE_CONFIG_DIR
  const child = spawn(process.execPath, args, { env, stdio: ['ignore', 'pipe', 'inherit'] })
  const tr = translator(provider)
  const out = (ev) => process.stdout.write(`${JSON.stringify(ev)}\n`)
  readline.createInterface({ input: child.stdout }).on('line', (line) => {
    if (!line.trim()) return
    for (const ev of tr.line(JSON.parse(line))) out(ev)
  })
  child.on('close', (code, signal) => {
    for (const ev of tr.end()) out(ev)
    process.exit(code ?? (signal ? 128 : 1))
  })
}

// ── Claude stream → provider stream ─────────────────────────────────────────
function translator(provider) {
  if (provider === 'agy') return { line: (ev) => [ev], end: () => [] }
  const tools = new Map()
  let pending = null
  let resetsAt
  let session = null
  const flush = (to) => {
    if (pending === null) return []
    const t = pending
    pending = null
    return to(t)
  }
  const textOf = (ev) => (ev.message?.content ?? []).filter((b) => b.type === 'text').map((b) => b.text).join('\n')
  const resultsOf = (ev) => (ev.message?.content ?? []).filter((b) => b.type === 'tool_result')
  const output = (r) => (typeof r.content === 'string' ? r.content : JSON.stringify(r.content))

  if (provider === 'codex') {
    let k = 0
    const message = (text) => [{ type: 'item.completed', item: { id: `item_${k++}`, type: 'agent_message', text } }]
    return {
      line(ev) {
        const out = []
        if (ev.type === 'rate_limit_event') {
          resetsAt = ev.rate_limit_info?.resetsAt
          return out
        }
        if (ev.type !== 'result') out.push(...flush(message))
        if (ev.type === 'system') {
          out.push({ type: 'thread.started', thread_id: ev.session_id }, { type: 'turn.started' })
        } else if (ev.type === 'assistant') {
          for (const b of ev.message.content) if (b.type === 'tool_use') tools.set(b.id, b)
          const text = textOf(ev)
          if (text) pending = text
        } else if (ev.type === 'user') {
          for (const r of resultsOf(ev)) {
            const t = tools.get(r.tool_use_id)
            if (!t) continue
            const id = `item_${k++}`
            if (t.name === 'Write' || t.name === 'Edit') {
              out.push({ type: 'item.completed', item: { id, type: 'file_change', changes: [{ path: t.input.file_path, kind: 'update' }], status: 'completed' } })
            } else {
              const command = t.input.command ?? `cat ${t.input.file_path}`
              out.push({ type: 'item.started', item: { id, type: 'command_execution', command, status: 'in_progress' } })
              out.push({ type: 'item.completed', item: { id, type: 'command_execution', command, aggregated_output: output(r), exit_code: r.is_error ? 1 : 0, status: r.is_error ? 'failed' : 'completed' } })
            }
          }
        } else if (ev.type === 'result') {
          pending = null
          if (ev.is_error) {
            out.push({ type: 'turn.failed', error: { message: ev.result, ...(resetsAt ? { resets_at: resetsAt } : {}) } })
          } else {
            const u = ev.usage ?? {}
            out.push(...message(ev.result))
            out.push({ type: 'turn.completed', usage: { input_tokens: (u.input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0), cached_input_tokens: u.cache_read_input_tokens ?? 0, output_tokens: u.output_tokens ?? 0 } })
          }
        }
        return out
      },
      end: () => flush(message),
    }
  }

  if (provider === 'cursor') {
    const key = { Bash: 'shellToolCall', Read: 'readToolCall', Write: 'writeToolCall', Edit: 'editToolCall' }
    const args = (t) => (t.name === 'Bash' ? { command: t.input.command } : { path: t.input.file_path })
    const call = (t) => (key[t.name] ? { [key[t.name]]: { args: args(t) } } : { function: { name: t.name, arguments: JSON.stringify(t.input) } })
    const message = (text) => [{ type: 'assistant', message: { role: 'assistant', content: [{ type: 'text', text }] }, session_id: session }]
    return {
      line(ev) {
        const out = []
        if (ev.type === 'rate_limit_event') {
          resetsAt = ev.rate_limit_info?.resetsAt
          return out
        }
        if (ev.type !== 'result') out.push(...flush(message))
        if (ev.type === 'system') {
          session = ev.session_id
          out.push({ type: 'system', subtype: 'init', cwd: ev.cwd, session_id: session, model: ev.model, permissionMode: 'default' })
        } else if (ev.type === 'assistant') {
          for (const b of ev.message.content) {
            if (b.type !== 'tool_use') continue
            tools.set(b.id, b)
            out.push({ type: 'tool_call', subtype: 'started', call_id: b.id, tool_call: call(b), session_id: session })
          }
          const text = textOf(ev)
          if (text) pending = text
        } else if (ev.type === 'user') {
          for (const r of resultsOf(ev)) {
            const t = tools.get(r.tool_use_id)
            if (!t) continue
            const c = call(t)
            const body = Object.values(c)[0]
            body.result = r.is_error ? { error: { message: output(r) } } : { success: { stdout: output(r), exitCode: 0 } }
            out.push({ type: 'tool_call', subtype: 'completed', call_id: t.id, tool_call: c, session_id: session })
          }
        } else if (ev.type === 'result') {
          pending = null
          const u = ev.usage ?? {}
          out.push(...message(ev.result))
          out.push({ type: 'result', subtype: ev.is_error ? 'error' : 'success', is_error: !!ev.is_error, duration_ms: ev.duration_ms ?? 0, result: ev.result, session_id: session, usage: { inputTokens: u.input_tokens ?? 0, outputTokens: u.output_tokens ?? 0, cacheReadTokens: u.cache_read_input_tokens ?? 0 }, ...(ev.is_error && resetsAt ? { resets_at: resetsAt } : {}) })
        }
        return out
      },
      end: () => flush(message),
    }
  }

  // opencode
  let k = 0
  const message = (text) => [{ type: 'text', sessionID: session, part: { id: `prt_${k++}`, type: 'text', text } }]
  return {
    line(ev) {
      const out = []
      if (ev.type === 'rate_limit_event') {
        resetsAt = ev.rate_limit_info?.resetsAt
        return out
      }
      if (ev.type !== 'result') out.push(...flush(message))
      if (ev.type === 'system') {
        session = ev.session_id
        out.push({ type: 'step_start', sessionID: session, part: { type: 'step-start' } })
      } else if (ev.type === 'assistant') {
        for (const b of ev.message.content) if (b.type === 'tool_use') tools.set(b.id, b)
        const text = textOf(ev)
        if (text) pending = text
      } else if (ev.type === 'user') {
        for (const r of resultsOf(ev)) {
          const t = tools.get(r.tool_use_id)
          if (!t) continue
          const input = t.name === 'Bash' ? { command: t.input.command, description: t.input.description } : { filePath: t.input.file_path }
          const state = r.is_error ? { status: 'error', input, error: output(r) } : { status: 'completed', input, output: output(r) }
          out.push({ type: 'tool_use', sessionID: session, part: { id: `prt_${k++}`, callID: t.id, tool: t.name.toLowerCase(), state } })
        }
      } else if (ev.type === 'result') {
        pending = null
        if (ev.is_error) {
          out.push({ type: 'error', sessionID: session, error: { name: 'APIError', data: { message: ev.result, ...(resetsAt ? { resetsAt } : {}) } } })
        } else {
          const u = ev.usage ?? {}
          out.push(...message(ev.result))
          out.push({ type: 'step_finish', sessionID: session, part: { type: 'step-finish', tokens: { input: u.input_tokens ?? 0, output: u.output_tokens ?? 0, reasoning: 0, cache: { read: u.cache_read_input_tokens ?? 0, write: u.cache_creation_input_tokens ?? 0 } }, cost: ev.total_cost_usd ?? 0 } })
        }
      }
      return out
    },
    end: () => flush(message),
  }
}
