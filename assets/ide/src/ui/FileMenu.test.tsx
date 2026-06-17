import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect, vi } from 'vitest'
import { FileMenu } from './FileMenu'

function defaultProps() {
  return {
    onNew: vi.fn(),
    onOpen: vi.fn(),
    onSave: vi.fn(),
    onHeal: vi.fn(),
    dirty: false,
    healing: false,
  }
}

describe('FileMenu', () => {
  it('renders New, Open, Save, and Heal buttons', () => {
    render(<FileMenu {...defaultProps()} />)
    expect(screen.getByRole('button', { name: /new/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /open/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /save/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /heal with ai/i })).toBeInTheDocument()
  })

  it('fires onNew when New is clicked', async () => {
    const user = userEvent.setup()
    const props = defaultProps()
    render(<FileMenu {...props} />)
    await user.click(screen.getByRole('button', { name: /new/i }))
    expect(props.onNew).toHaveBeenCalledOnce()
  })

  it('fires onSave when Save is clicked', async () => {
    const user = userEvent.setup()
    const props = defaultProps()
    render(<FileMenu {...props} />)
    await user.click(screen.getByRole('button', { name: /save/i }))
    expect(props.onSave).toHaveBeenCalledOnce()
  })

  it('fires onHeal when Heal is clicked', async () => {
    const user = userEvent.setup()
    const props = defaultProps()
    render(<FileMenu {...props} />)
    await user.click(screen.getByRole('button', { name: /heal with ai/i }))
    expect(props.onHeal).toHaveBeenCalledOnce()
  })

  it('disables Heal and shows "Healing…" while a heal is in flight', () => {
    render(<FileMenu {...defaultProps()} healing={true} />)
    const btn = screen.getByRole('button', { name: /healing…/i })
    expect(btn).toBeDisabled()
  })

  it('shows dirty indicator when dirty is true', () => {
    render(<FileMenu {...defaultProps()} dirty={true} />)
    expect(screen.getByText('•')).toBeInTheDocument()
  })

  it('hides dirty indicator when dirty is false', () => {
    render(<FileMenu {...defaultProps()} />)
    expect(screen.queryByText('•')).not.toBeInTheDocument()
  })
})
