// Connection-status footer. Renders compact, monospace, gray — non-intrusive
// when everything is fine, immediately obvious when the WS breaks.

import type { ConnectionState } from './client'
import type { InitializeResult } from './initialize'

interface Props {
  state: ConnectionState
  init: InitializeResult | null
}

export function StatusBar({ state, init }: Props) {
  const dotColor = (() => {
    switch (state.status) {
      case 'open':
        return 'bg-green-500'
      case 'connecting':
        return 'bg-yellow-500'
      case 'closed':
      case 'error':
        return 'bg-red-500'
    }
  })()

  const left = (() => {
    if (state.status === 'connecting') return 'connecting to neo…'
    if (state.status === 'error') return `disconnected (${state.message})`
    if (state.status === 'closed') return `disconnected (${state.reason})`
    if (!init) return 'connected, awaiting initialize'
    return `${init.serverInfo.name} v${init.serverInfo.version} · session ${init.sessionId}`
  })()

  const right = init ? init.workspace.root : null

  return (
    <div
      data-testid="ide-statusbar"
      className="flex items-center gap-2 px-3 py-1 text-xs font-mono bg-gray-100 border-t border-gray-300 text-gray-600 select-none"
    >
      <span className={`inline-block w-2 h-2 rounded-full ${dotColor}`} aria-hidden />
      <span>{left}</span>
      {right && (
        <>
          <span className="text-gray-400">·</span>
          <span className="truncate" title={right}>{right}</span>
        </>
      )}
      {init?.workspace.project && (
        <>
          <span className="text-gray-400">·</span>
          <span>{init.workspace.project.name} v{init.workspace.project.version}</span>
        </>
      )}
    </div>
  )
}
