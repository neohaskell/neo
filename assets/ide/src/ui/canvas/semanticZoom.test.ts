import { describe, it, expect } from 'vitest'
import { shouldShowFields, SEMANTIC_ZOOM_THRESHOLD } from './semanticZoom'

describe('semantic zoom', () => {
  it('semantic_zoom_reveals_fields_above_threshold', () => {
    expect(shouldShowFields(SEMANTIC_ZOOM_THRESHOLD)).toBe(true)
    expect(shouldShowFields(SEMANTIC_ZOOM_THRESHOLD + 0.5)).toBe(true)
    expect(shouldShowFields(SEMANTIC_ZOOM_THRESHOLD - 0.01)).toBe(false)
    expect(shouldShowFields(1)).toBe(false)
  })
})
