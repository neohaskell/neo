import { describe, it, expect } from 'vitest'
import {
  estimateNodeDimensions,
  MIN_NODE_WIDTH,
  MAX_NODE_WIDTH,
} from './nodeDimensions'

describe('estimateNodeDimensions', () => {
  it('clamps width to MIN_NODE_WIDTH for short labels', () => {
    const { width } = estimateNodeDimensions('A')
    expect(width).toBe(MIN_NODE_WIDTH)
  })

  it('clamps width to MAX_NODE_WIDTH for very long labels', () => {
    const { width } = estimateNodeDimensions('x'.repeat(200))
    expect(width).toBe(MAX_NODE_WIDTH)
  })

  it('scales width with label length up to the cap', () => {
    const short = estimateNodeDimensions('Pay')
    const medium = estimateNodeDimensions('PayBankRoute')
    const long = estimateNodeDimensions('PaymentFormPreparation')
    expect(short.width).toBeLessThanOrEqual(medium.width)
    expect(medium.width).toBeLessThanOrEqual(long.width)
  })

  it('reports lines=1 and a single-line height for short labels', () => {
    const { lines, height } = estimateNodeDimensions('Pay')
    expect(lines).toBe(1)
    expect(height).toBeLessThanOrEqual(40)
  })

  it('reports lines>=2 and taller height when label is longer than MAX_NODE_WIDTH allows', () => {
    const { lines, height } = estimateNodeDimensions(
      'PaymentFormPreparationFailedReasonUnknown',
    )
    expect(lines).toBeGreaterThanOrEqual(2)
    expect(height).toBeGreaterThan(40)
  })

  it('is deterministic — same input produces same output', () => {
    const a = estimateNodeDimensions('AnyLabel')
    const b = estimateNodeDimensions('AnyLabel')
    expect(a).toEqual(b)
  })
})
