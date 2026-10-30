export type Run = {
  id: string
  feature: string
  phase: string
  line: string
  owner: string[]
  board: string[]
}

export type Status = { runs: Run[]; error: string | null }

declare module 'claude-code' {
  interface PluginState {
    'factory-cockpit': { status: Shaped<Status | null> }
  }
}
