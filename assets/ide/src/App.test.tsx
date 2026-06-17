import { render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi, beforeEach } from 'vitest'

// `ipc/client` is mocked module-wide so we can control the WS handshake
// and the responses to `initialize` / `readEventModel` / `healEventModel`.
// Tests configure behaviour via the `__configureIpc` helper exposed below.

interface IpcStubConfig {
  initialize: { ok: true; result: unknown } | { ok: false; error: { code: number; message: string } }
  readQueue: Array<
    | { ok: true; result: { content: string | null; validation: unknown } }
    | { ok: false; error: { code: number; message: string } }
  >
  healQueue: Array<
    | { ok: true; result: { outcome: { status: 'healed' } | { status: 'stillInvalid'; errors: unknown[] } } }
    | { ok: false; error: { code: number; message: string } }
  >
  healDelayMs?: number
}

const ipcState: { config: IpcStubConfig; readCalls: number; healCalls: number } = {
  config: {
    initialize: {
      ok: true,
      result: {
        serverInfo: { name: 'neo', version: '0.0.0' },
        serverCapabilities: {},
        workspace: { id: 'ws', root: '/tmp/ws', project: null },
        sessionId: 'sess',
      },
    },
    readQueue: [],
    healQueue: [],
  },
  readCalls: 0,
  healCalls: 0,
}

function nextResponse<T>(queue: T[], label: string): T {
  if (queue.length === 0) {
    throw new Error(`Test setup error: no queued ${label} response`)
  }
  return queue.shift()!
}

vi.mock('./ipc/client', () => {
  class IdeClientStub {
    onState(listener: (s: unknown) => void) {
      listener({ status: 'open' })
      return () => {}
    }
    close() {}
  }
  return { IdeClient: IdeClientStub, defaultWsUrl: () => 'ws://stub' }
})

vi.mock('./ipc/initialize', () => ({
  initialize: vi.fn(async () => ipcState.config.initialize),
}))

vi.mock('./ipc/eventModel', () => ({
  readEventModel: vi.fn(async () => {
    ipcState.readCalls += 1
    return nextResponse(ipcState.config.readQueue, 'readEventModel')
  }),
  writeEventModel: vi.fn(async () => ({ ok: true, result: { path: '/tmp/ws/event-model.json' } })),
  healEventModel: vi.fn(async () => {
    ipcState.healCalls += 1
    if (ipcState.config.healDelayMs) {
      await new Promise((r) => setTimeout(r, ipcState.config.healDelayMs))
    }
    return nextResponse(ipcState.config.healQueue, 'healEventModel')
  }),
}))

import App from './App'

function configureIpc(patch: Partial<IpcStubConfig>) {
  ipcState.config = {
    initialize: patch.initialize ?? ipcState.config.initialize,
    readQueue: patch.readQueue ?? [],
    healQueue: patch.healQueue ?? [],
    healDelayMs: patch.healDelayMs,
  }
  ipcState.readCalls = 0
  ipcState.healCalls = 0
}

const VALID_MODEL_JSON = JSON.stringify({
  id: 'm1',
  name: 'Demo',
  chapters: [],
  entities: [],
  slices: [],
  nodes: [],
  edges: [],
  layout: { nodePositions: {}, viewport: { x: 0, y: 0, zoom: 1 } },
})

beforeEach(() => {
  configureIpc({
    readQueue: [{ ok: true, result: { content: null, validation: { status: 'notFound' } } }],
  })
  localStorage.clear()
})

describe('App — base render', () => {
  it('renders without crashing', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: /event/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /command/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /new/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
  })

  it('renders the canvas area', () => {
    render(<App />)
    expect(screen.getByTestId('canvas')).toBeInTheDocument()
  })

  it('shows toolbar buttons for all node types', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: /\+ event/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ command/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ query/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ integration/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ ui placeholder/i })).toBeInTheDocument()
    expect(screen.getAllByRole('button', { name: /entity/i }).length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByRole('button', { name: /slice/i }).length).toBeGreaterThanOrEqual(1)
    expect(screen.getAllByRole('button', { name: /chapter/i }).length).toBeGreaterThanOrEqual(1)
  })

  it('shows file menu buttons', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: /new/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /open/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
  })

  it('marks dirty after adding a node', async () => {
    const user = userEvent.setup()
    render(<App />)
    expect(screen.queryByText('•')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /\+ event/i }))
    expect(screen.getByText('•')).toBeInTheDocument()
  })
})

describe('App — event-model load + heal flow', () => {
  it('does not show modal when backend reports notFound', async () => {
    configureIpc({
      readQueue: [{ ok: true, result: { content: null, validation: { status: 'notFound' } } }],
    })
    render(<App />)
    // Give the mount effect a tick to complete.
    await waitFor(() => expect(ipcState.readCalls).toBe(1))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('does not show modal when backend reports valid', async () => {
    configureIpc({
      readQueue: [
        { ok: true, result: { content: VALID_MODEL_JSON, validation: { status: 'valid' } } },
      ],
    })
    render(<App />)
    await waitFor(() => expect(ipcState.readCalls).toBe(1))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('shows the invalid-model modal when backend reports invalid', async () => {
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{"missing": "id"}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'missing required `id`', kind: 'schema' }],
            },
          },
        },
      ],
    })
    render(<App />)
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText(/missing required `id`/)).toBeInTheDocument()
  })

  it('shows the malformed-JSON modal with a helpful preamble', async () => {
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{not json',
            validation: { status: 'malformedJson', parseError: 'expected `}` at line 1 column 9' },
          },
        },
      ],
    })
    render(<App />)
    expect(await screen.findByRole('dialog')).toBeInTheDocument()
    expect(
      screen.getByText(/event-model\.json on disk is not valid JSON/),
    ).toBeInTheDocument()
  })

  it('Cancel hides the modal without further RPC traffic', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'oops', kind: 'schema' }],
            },
          },
        },
      ],
    })
    render(<App />)
    await user.click(await screen.findByRole('button', { name: /cancel/i }))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    expect(ipcState.healCalls).toBe(0)
  })

  it('Heal happy path: heal → reload → modal closes', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'oops', kind: 'schema' }],
            },
          },
        },
        // Second call (post-heal reload) — file is now valid.
        { ok: true, result: { content: VALID_MODEL_JSON, validation: { status: 'valid' } } },
      ],
      healQueue: [{ ok: true, result: { outcome: { status: 'healed' } } }],
    })
    render(<App />)
    const modal = await screen.findByRole('dialog')
    await user.click(within(modal).getByRole('button', { name: /heal with ai/i }))
    await waitFor(() => expect(ipcState.healCalls).toBe(1))
    await waitFor(() => expect(ipcState.readCalls).toBe(2))
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument(),
    )
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })

  it('Heal still-invalid: modal reappears with new errors', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'first error', kind: 'schema' }],
            },
          },
        },
      ],
      healQueue: [
        {
          ok: true,
          result: {
            outcome: {
              status: 'stillInvalid',
              errors: [{ pointer: '/foo', message: 'second error after heal', kind: 'schema' }],
            },
          },
        },
      ],
    })
    render(<App />)
    const modal = await screen.findByRole('dialog')
    await user.click(within(modal).getByRole('button', { name: /heal with ai/i }))
    await waitFor(() =>
      expect(screen.getByText(/second error after heal/)).toBeInTheDocument(),
    )
    expect(screen.getByText(/still invalid/i)).toBeInTheDocument()
  })

  it('Heal RPC failure shows a toast and closes the modal', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'oops', kind: 'schema' }],
            },
          },
        },
      ],
      healQueue: [{ ok: false, error: { code: -32000, message: 'claude not on PATH' } }],
    })
    render(<App />)
    const modal = await screen.findByRole('dialog')
    await user.click(within(modal).getByRole('button', { name: /heal with ai/i }))
    await waitFor(() =>
      expect(screen.getByText(/Healing failed: claude not on PATH/)).toBeInTheDocument(),
    )
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('Heal shows the spinner overlay while the RPC is in flight', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        {
          ok: true,
          result: {
            content: '{}',
            validation: {
              status: 'invalid',
              errors: [{ pointer: '', message: 'oops', kind: 'schema' }],
            },
          },
        },
        { ok: true, result: { content: VALID_MODEL_JSON, validation: { status: 'valid' } } },
      ],
      healQueue: [{ ok: true, result: { outcome: { status: 'healed' } } }],
      healDelayMs: 50,
    })
    render(<App />)
    const modal = await screen.findByRole('dialog')
    await user.click(within(modal).getByRole('button', { name: /heal with ai/i }))
    // Overlay should appear while heal is pending.
    expect(await screen.findByRole('status')).toBeInTheDocument()
    expect(screen.getByText(/healing event model/i)).toBeInTheDocument()
    // And go away after heal + reload.
    await waitFor(() => expect(screen.queryByRole('status')).not.toBeInTheDocument())
  })

  it('manual Heal button (FileMenu) triggers heal even when the file is valid', async () => {
    const user = userEvent.setup()
    configureIpc({
      readQueue: [
        // Mount load: file is already valid.
        { ok: true, result: { content: VALID_MODEL_JSON, validation: { status: 'valid' } } },
        // Post-heal reload.
        { ok: true, result: { content: VALID_MODEL_JSON, validation: { status: 'valid' } } },
      ],
      healQueue: [{ ok: true, result: { outcome: { status: 'healed' } } }],
    })
    render(<App />)
    // Wait for mount to settle.
    await waitFor(() => expect(ipcState.readCalls).toBe(1))
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    // Click the FileMenu's Heal button — distinct from the modal one because
    // the modal isn't open here.
    await user.click(screen.getByRole('button', { name: /heal with ai/i }))
    await waitFor(() => expect(ipcState.healCalls).toBe(1))
    // Reload fired after heal.
    await waitFor(() => expect(ipcState.readCalls).toBe(2))
  })
})
