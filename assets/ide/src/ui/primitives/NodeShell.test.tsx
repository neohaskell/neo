import { describe, it, expect, vi, beforeEach } from 'vitest'
import type { ReactNode } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import { render, screen } from '../../test/render'
import { NodeShell } from './NodeShell'

// The node's level-of-detail reads the canvas zoom via useStore(s => s.transform[2]).
// Mock useStore to feed a controllable zoom while keeping the rest of React Flow
// real (Handle/ReactFlowProvider still work).
let mockZoom = 1
vi.mock('@xyflow/react', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@xyflow/react')>()
  return {
    ...actual,
    useStore: (selector: (s: { transform: [number, number, number] }) => unknown) =>
      selector({ transform: [0, 0, mockZoom] }),
  }
})

const wrap = (ui: ReactNode) => render(<ReactFlowProvider>{ui}</ReactFlowProvider>)

const fields = [
  { name: 'orderId', type: 'UUID' },
  { name: 'total', type: 'Money' },
]

beforeEach(() => {
  mockZoom = 1
})

describe('NodeShell record card', () => {
  it('renders_header_and_body_zones', () => {
    const { container } = wrap(<NodeShell variant="event" label="OrderPlaced" fields={fields} />)
    expect(container.querySelector('[data-variant="event"]')).not.toBeNull()
    // header carries the type name; body carries the field rows.
    expect(screen.getByText('OrderPlaced')).toBeInTheDocument()
    expect(screen.getByText('orderId')).toBeInTheDocument()
    expect(screen.getByText('UUID')).toBeInTheDocument()
  })

  it('fields_render_always_read_only', () => {
    wrap(<NodeShell variant="event" label="E" fields={fields} />)
    // Read-only rows present, but NOT the editable FieldsEditor (no inputs).
    expect(screen.getByText('orderId')).toBeInTheDocument()
    expect(screen.queryByTestId('fields-editor')).toBeNull()
  })

  it('empty_node_shows_no_fields_state', () => {
    wrap(<NodeShell variant="command" label="Checkout" fields={[]} />)
    expect(screen.getByText('no fields')).toBeInTheDocument()
  })

  it('editable_fields_on_selection', () => {
    wrap(
      <NodeShell
        variant="event"
        label="E"
        fields={fields}
        selected
        onFieldsChange={() => {}}
      />,
    )
    expect(screen.getByTestId('fields-editor')).toBeInTheDocument()
  })

  it('not_editable_when_selected_without_callback', () => {
    wrap(<NodeShell variant="event" label="E" fields={fields} selected />)
    expect(screen.queryByTestId('fields-editor')).toBeNull()
    expect(screen.getByText('orderId')).toBeInTheDocument()
  })

  it('header_only_when_zoomed_out', () => {
    mockZoom = 0.3 // below COLLAPSE_THRESHOLD
    const { container } = wrap(<NodeShell variant="event" label="OrderPlaced" fields={fields} />)
    expect(container.querySelector('[data-detail="header"]')).not.toBeNull()
    // type name still shows; field rows are gone (flow view).
    expect(screen.getByText('OrderPlaced')).toBeInTheDocument()
    expect(screen.queryByText('orderId')).toBeNull()
  })

  it('caps_visible_rows_with_more_indicator', () => {
    const many = Array.from({ length: 9 }, (_, i) => ({ name: `f${i}`, type: 'T' }))
    wrap(<NodeShell variant="event" label="Big" fields={many} />)
    expect(screen.getByText('+3 more')).toBeInTheDocument() // 9 - FIELD_CAP(6)
  })
})
