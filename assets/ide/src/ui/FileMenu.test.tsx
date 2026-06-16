import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { FileMenu } from './FileMenu'

describe('FileMenu', () => {
  it('renders New, Open, and Save buttons', () => {
    render(<FileMenu onNew={vi.fn()} onOpen={vi.fn()} onSave={vi.fn()} dirty={false} />)
    expect(screen.getByRole('button', { name: /new/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /open/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
  })

  it('fires onNew when New is clicked', async () => {
    const user = userEvent.setup()
    const onNew = vi.fn()
    render(<FileMenu onNew={onNew} onOpen={vi.fn()} onSave={vi.fn()} dirty={false} />)
    await user.click(screen.getByRole('button', { name: /new/i }))
    expect(onNew).toHaveBeenCalledOnce()
  })

  it('fires onSave when Save is clicked', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    render(<FileMenu onNew={vi.fn()} onOpen={vi.fn()} onSave={onSave} dirty={false} />)
    await user.click(screen.getByRole('button', { name: /save/i }))
    expect(onSave).toHaveBeenCalledOnce()
  })

  it('shows dirty indicator when dirty is true', () => {
    render(<FileMenu onNew={vi.fn()} onOpen={vi.fn()} onSave={vi.fn()} dirty={true} />)
    expect(screen.getByText('\u2022')).toBeInTheDocument()
  })

  it('hides dirty indicator when dirty is false', () => {
    render(<FileMenu onNew={vi.fn()} onOpen={vi.fn()} onSave={vi.fn()} dirty={false} />)
    expect(screen.queryByText('\u2022')).not.toBeInTheDocument()
  })
})
