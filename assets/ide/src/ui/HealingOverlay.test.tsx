import { describe, it, expect } from 'vitest'
import { render, screen } from '@testing-library/react'
import { HealingOverlay } from './HealingOverlay'

describe('HealingOverlay', () => {
  it('renders the default healing message and a spinner', () => {
    const { container } = render(<HealingOverlay />)
    expect(screen.getByText(/healing event model/i)).toBeInTheDocument()
    expect(container.querySelector('.animate-spin')).not.toBeNull()
  })

  it('uses a custom message when supplied', () => {
    render(<HealingOverlay message="Custom status" />)
    expect(screen.getByText('Custom status')).toBeInTheDocument()
  })

  it('marks itself as a live status region for screen readers', () => {
    render(<HealingOverlay />)
    expect(screen.getByRole('status')).toBeInTheDocument()
  })
})
