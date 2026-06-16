import { render, screen } from '@testing-library/react'
import { describe, it, expect } from 'vitest'
import { SwimLaneOverlay } from './SwimLane'
import type { SwimLaneLayout } from './layout/swimlanes'

describe('SwimLaneOverlay', () => {
  const lanes: SwimLaneLayout[] = [
    { entityId: 'e1', name: 'Order', yStart: 0, yEnd: 150 },
    { entityId: 'e2', name: 'Stock', yStart: 150, yEnd: 300 },
  ]

  it('renders entity names', () => {
    render(<SwimLaneOverlay lanes={lanes} />)
    expect(screen.getByText('Order')).toBeInTheDocument()
    expect(screen.getByText('Stock')).toBeInTheDocument()
  })

  it('renders one lane per entity', () => {
    const { container } = render(<SwimLaneOverlay lanes={lanes} />)
    const laneElements = container.querySelectorAll('[data-testid^="swimlane-"]')
    expect(laneElements).toHaveLength(2)
  })

  it('renders nothing for empty lanes', () => {
    const { container } = render(<SwimLaneOverlay lanes={[]} />)
    const laneElements = container.querySelectorAll('[data-testid^="swimlane-"]')
    expect(laneElements).toHaveLength(0)
  })
})
