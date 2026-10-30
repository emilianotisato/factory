import { expect, mock, test } from 'claude-code/testing'
import type { Engine } from 'claude-code/testing'
import type { On } from 'claude-code'

const VERSION = '0.11.0'
const STALE = 'mod stale: factory cockpit install, then /reload-plugins'
const QUESTION = 'Which storage? (answer: factory answer Q "...")'

const SURFACES = ['terminal', 'desktop'] as const

const BAND = {
  plugin: 'factory-cockpit',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 12,
    bodyColumns: 100,
    scroll: { offset: 0, bodyRows: 12 },
    view: {},
  },
} as const

const PANE = {
  plugin: 'factory-cockpit',
  component: 'Pane',
  requestId: 'factory',
  props: {
    title: 'factory',
    isFocused: false,
    bodyColumns: 100,
    placement: 'dock',
    scroll: { offset: 0, bodyRows: 20 },
    view: {},
  },
} as const

type Answer = { exitCode: number; stdout: string; stderr: string }

const ok = (body: object): Answer => ({ exitCode: 0, stdout: JSON.stringify(body), stderr: '' })

const run = (feature: string, line: string, owner: string[], board: string[]) => ({
  id: `${feature}-run`,
  feature,
  phase: 'building',
  line,
  owner,
  board,
})

const TWO_RUNS = ok({
  schema: 1,
  factory: VERSION,
  runs: [
    run('013', '013 building 2/3', [], ['013 live-status', '  storage done']),
    run('014', '014 waiting on you', [QUESTION], ['014 next-thing', '  storage waiting']),
  ],
})

const NO_RUN = ok({ schema: 1, factory: VERSION, runs: [] })

type World = { argv: string[][]; statuses: (string | undefined)[]; commands: string[] }

async function start($: Engine, on: On, answer: () => Answer | string, env: Record<string, string> = {}) {
  const world: World = { argv: [], statuses: [], commands: [] }
  const clock = mock.clock(on)
  mock.env(on, env)
  on('process.run', (_, e) => {
    world.argv.push([...e.argv])
    const answered = answer()
    if (typeof answered === 'string') {
      return { deny: answered }
    }
    const { exitCode, stdout, stderr } = answered
    return {
      value: { exitCode, stdout, stderr, isStdoutTruncated: false, isStderrTruncated: false },
    }
  })
  on('ui.status', (_, e) => {
    world.statuses.push(e.text)
    return { value: undefined }
  })
  on('command.register', (_, e) => {
    world.commands.push(e.name)
    return { value: { command: e.name } }
  })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  await $.session.start({ cwd: '/repo', surface: 'terminal', isInteractive: true })
  await clock.advance(10_000)
  return { ...world, clock }
}

test('a worker session never polls and registers no command', async ($, on) => {
  const world = await start($, on, () => TWO_RUNS, { FACTORY_REPO: '/repo' })
  expect(world.argv).toHaveLength(0)
  expect(world.commands).toHaveLength(0)
  expect(world.statuses).toHaveLength(0)
})

test('the poll runs factory status for this repo as JSON', async ($, on) => {
  const world = await start($, on, () => TWO_RUNS)
  expect(world.argv.length).toBeGreaterThan(0)
  expect(world.argv[0]).toEqual(['factory', 'status', '--here', '--json'])
})

test('the status line joins every run line', async ($, on) => {
  const world = await start($, on, () => TWO_RUNS)
  expect(world.statuses.at(-1)).toBe('013 building 2/3  |  014 waiting on you')
})

test('no live run clears the status line', async ($, on) => {
  const world = await start($, on, () => NO_RUN)
  expect(world.statuses.length).toBeGreaterThan(0)
  expect(world.statuses.at(-1)).toBeUndefined()
})

test('a factory of another version marks the mod stale', async ($, on) => {
  const world = await start($, on, () => ({ ...TWO_RUNS, stdout: TWO_RUNS.stdout.replace(VERSION, '9.9.9') }))
  expect(world.statuses.at(-1)).toBe(`013 building 2/3  |  014 waiting on you · ${STALE}`)
})

test('an unknown schema marks the mod stale', async ($, on) => {
  const world = await start($, on, () => ok({ schema: 2, factory: VERSION, runs: [] }))
  expect(world.statuses.at(-1)).toBe(`factory: ${STALE}`)
})

test('a stale mod with no live run shows the stale line alone', async ($, on) => {
  const world = await start($, on, () => ok({ schema: 1, factory: '9.9.9', runs: [] }))
  expect(world.statuses.at(-1)).toBe(`factory: ${STALE}`)
})

test('a failing factory shows the first line of its error, and the next poll tries again', async ($, on) => {
  let answer: Answer = { exitCode: 2, stdout: '', stderr: 'error: not a factory repo\nhint: run factory init\n' }
  const world = await start($, on, () => answer)
  expect(world.statuses.at(-1)).toBe('factory: error: not a factory repo')
  answer = TWO_RUNS
  await world.clock.advance(10_000)
  expect(world.statuses.at(-1)).toBe('013 building 2/3  |  014 waiting on you')
})

test('output that is not JSON shows the parse error', async ($, on) => {
  const world = await start($, on, () => ({ exitCode: 0, stdout: 'not json', stderr: '' }))
  expect(world.statuses.at(-1)).toMatch(/^factory: \S/)
})

test('a missing factory binary shows the error', async ($, on) => {
  const world = await start($, on, () => 'spawn factory ENOENT')
  expect(world.statuses.at(-1)).toMatch(/^factory: .*spawn factory ENOENT$/)
})

test('an open question draws one band row led by the feature', async ($, on) => {
  await start($, on, () => TWO_RUNS)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...BAND, surface })
    const rows = await ui.findAll({ type: 'Text' })
    expect(rows.map(row => row.text)).toEqual([`014 ${QUESTION}`])
    await ui.unmount()
  }
})

test('nothing owed draws no band', async ($, on) => {
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>engine</Text>
  })
  await start($, on, () => ok({ schema: 1, factory: VERSION, runs: [run('013', '013 building', [], ['013'])] }))
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...BAND, surface })
    const rows = await ui.findAll({ type: 'Text' })
    expect(rows.map(row => row.text)).toEqual(['engine'])
    await ui.unmount()
  }
})

const BOARD = {
  command: 'factory-board',
  args: '',
  origin: { kind: 'composer' },
  presentation: { isFullscreen: true, columns: 160 },
} as const

function panes(on: On) {
  let open: string[] = []
  on('ui.open', (_, e) => {
    open = open.includes(e.id) ? open : [...open, e.id]
    return { value: { isPlaced: true } }
  })
  on('ui.close', (_, e) => {
    open = open.filter(id => id !== e.id)
    return { value: undefined }
  })
  on('ui.panes', () => ({
    value: open.map(id => ({ id, title: id, isShown: true, isFocused: false, isPlaced: true })),
  }))
  return () => open
}

test('/factory-board opens the board pane and a second call closes it', async ($, on) => {
  const open = panes(on)
  await start($, on, () => TWO_RUNS)
  await $.command.run(BOARD)
  expect(open()).toEqual(['factory'])
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface })
    const lines = await ui.findAll({ type: 'Text' })
    expect(lines.map(line => line.text)).toEqual([
      '013 live-status',
      '  storage done',
      '014 next-thing',
      '  storage waiting',
    ])
    await ui.unmount()
  }
  await $.command.run(BOARD)
  expect(open()).toEqual([])
})

test('the board pane with no live run says so', async ($, on) => {
  await start($, on, () => NO_RUN)
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect((await ui.find({ type: 'Text' }))?.text).toBe('no live run in this repo')
    await ui.unmount()
  }
})
