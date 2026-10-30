import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'
import type { Run } from '../types'

const MOD_VERSION = '0.11.0'
const SCHEMA = 1
const PANE = 'factory'
const COMMAND = 'factory-board'
const POLL_MS = 10_000
const STALE = 'mod stale: factory cockpit install, then /reload-plugins'

// The tag declines a value an older build of the mod stored, which outlives a reload.
const status = atom({ plugin: 'factory-cockpit', key: 'status' } as const, null, {
  shape: 'status-1',
})

type Reading = { runs: Run[]; isStale: boolean }

const firstLine = (text: string): string => text.trim().split('\n')[0] ?? ''

const fieldsOf = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null ? (value as Record<string, unknown>) : {}

const textOf = (value: unknown): string =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : ''

const linesOf = (value: unknown): string[] =>
  Array.isArray(value) ? value.filter((line): line is string => typeof line === 'string') : []

function runOf(value: unknown): Run {
  const run = fieldsOf(value)
  return {
    id: textOf(run.id),
    feature: textOf(run.feature),
    phase: textOf(run.phase),
    line: textOf(run.line),
    owner: linesOf(run.owner),
    board: linesOf(run.board),
  }
}

function parse(stdout: string): Reading {
  const json = fieldsOf(JSON.parse(stdout))
  return {
    runs: Array.isArray(json.runs) ? json.runs.map(runOf) : [],
    isStale: json.schema !== SCHEMA || json.factory !== MOD_VERSION,
  }
}

function statusLine({ runs, isStale }: Reading): string | undefined {
  if (runs.length === 0) {
    return isStale ? `factory: ${STALE}` : undefined
  }
  const line = runs.map(run => run.line).join('  |  ')
  return isStale ? `${line} · ${STALE}` : line
}

async function refresh($: EngineInterface): Promise<void> {
  let reading: Reading
  try {
    const ran = await $.process.run(['factory', 'status', '--here', '--json'])
    if (ran.exitCode !== 0) {
      throw new Error(firstLine(ran.stderr) || firstLine(ran.stdout) || `exit ${ran.exitCode}`)
    }
    reading = parse(ran.stdout)
  } catch (err) {
    const error = firstLine(err instanceof Error ? err.message : String(err))
    $.ui.status(`factory: ${error}`)
    await update($, status, () => ({ runs: [], error }))
    return
  }
  $.ui.status(statusLine(reading))
  await update($, status, () => ({ runs: reading.runs, error: null }))
}

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    // A worker session carries FACTORY_REPO; the mod stays out of its way.
    if ((await $.env.get('FACTORY_REPO')) !== undefined) {
      return next(e)
    }
    await $.command.register({
      name: COMMAND,
      description: 'Show or hide the board of every live factory run in this repo',
    })
    let isPolling = false
    const poll = async (): Promise<void> => {
      if (isPolling) {
        return
      }
      isPolling = true
      try {
        await refresh($)
      } finally {
        isPolling = false
      }
    }
    void poll()
    $.clock.every(POLL_MS, () => void poll())
    return next(e)
  })

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const now = await read($, status)
    const rows = (now?.runs ?? []).flatMap(run => run.owner.map(owed => `${run.feature} ${owed}`))
    if (e.props.hasSurvey || rows.length === 0) {
      return next(e)
    }
    const { Box, Text } = $.ui.resolve(e)
    return (
      <Box flexDirection="column">
        {rows.map(row => (
          <Text>{row}</Text>
        ))}
      </Box>
    )
  })

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    const now = await read($, status)
    if (now?.error) {
      return <Text>factory: {now.error}</Text>
    }
    const runs = now?.runs ?? []
    if (runs.length === 0) {
      return <Text dimColor>no live run in this repo</Text>
    }
    return (
      <Box flexDirection="column">
        {runs.map((run, i) => (
          <Box flexDirection="column" marginTop={i === 0 ? 0 : 1}>
            {run.board.map(line => (
              <Text>{line}</Text>
            ))}
          </Box>
        ))}
      </Box>
    )
  })

  on('command.run', { command: COMMAND }, async $ => {
    const panes = await $.ui.panes()
    if (panes.some(pane => pane.id === PANE)) {
      await $.ui.close({ id: PANE })
      return { text: 'factory board closed.' }
    }
    await $.ui.open({ id: PANE, title: 'factory' })
    return { text: 'factory board opened.' }
  }).catch(($, e, next) => (next.called ? next(e) : { text: 'factory board: the pane could not open.' }))
}
