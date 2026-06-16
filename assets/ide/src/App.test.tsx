import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, it, expect } from 'vitest'
import App from './App'

describe('App', () => {
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
    // Entity, Slice, Chapter are in both toolbar and sidebar
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
    expect(screen.queryByText('\u2022')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /\+ event/i }))
    expect(screen.getByText('\u2022')).toBeInTheDocument()
  })
})
