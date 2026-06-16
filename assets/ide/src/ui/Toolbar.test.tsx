import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { Toolbar } from './Toolbar'

describe('Toolbar', () => {
  const buttons = [
    { label: 'Event', callback: 'onAddEvent' },
    { label: 'Command', callback: 'onAddCommand' },
    { label: 'Query', callback: 'onAddQuery' },
    { label: 'Integration', callback: 'onAddIntegration' },
    { label: 'UI Placeholder', callback: 'onAddUIPlaceholder' },
    { label: 'Entity', callback: 'onAddEntity' },
    { label: 'Slice', callback: 'onAddSlice' },
    { label: 'Chapter', callback: 'onAddChapter' },
  ] as const

  function renderToolbar() {
    const callbacks = Object.fromEntries(
      buttons.map((b) => [b.callback, vi.fn()]),
    ) as Record<(typeof buttons)[number]['callback'], ReturnType<typeof vi.fn>>
    render(<Toolbar {...callbacks} />)
    return callbacks
  }

  it('renders all add buttons', () => {
    renderToolbar()
    for (const { label } of buttons) {
      expect(screen.getByRole('button', { name: new RegExp(label) })).toBeInTheDocument()
    }
  })

  for (const { label, callback } of buttons) {
    it(`fires ${callback} when "${label}" is clicked`, async () => {
      const user = userEvent.setup()
      const callbacks = renderToolbar()
      await user.click(screen.getByRole('button', { name: new RegExp(label) }))
      expect(callbacks[callback]).toHaveBeenCalledOnce()
    })
  }
})
