import { describe, it, expect } from 'vitest'
import { buildGridNodes } from './grid'
import {
  createEventModel,
  addEntity,
  addSlice,
  addChapter,
} from '../../model/operations'

describe('buildGridNodes', () => {
  it('returns empty arrays for model with no entities or slices', () => {
    const model = createEventModel('Test')
    const result = buildGridNodes(model)
    expect(result.entityLaneNodes).toEqual([])
    expect(result.sliceColumnNodes).toEqual([])
    expect(result.sliceLayouts).toEqual([])
  })

  it('creates one lane node per entity', () => {
    let model = createEventModel('Test')
    model = addEntity(model, { name: 'Order' })
    model = addEntity(model, { name: 'Stock' })
    const result = buildGridNodes(model)
    expect(result.entityLaneNodes).toHaveLength(2)
  })

  it('entity lane nodes have correct labels', () => {
    let model = createEventModel('Test')
    model = addEntity(model, { name: 'Proposal' })
    const result = buildGridNodes(model)
    expect(result.entityLaneNodes[0].data.label).toBe('Proposal')
  })

  it('entity lanes are positioned vertically by order', () => {
    let model = createEventModel('Test')
    model = addEntity(model, { name: 'A' })
    model = addEntity(model, { name: 'B' })
    const result = buildGridNodes(model)
    expect(result.entityLaneNodes[0].position.y).toBeLessThan(
      result.entityLaneNodes[1].position.y,
    )
  })

  it('entity lane nodes are not draggable or selectable', () => {
    let model = createEventModel('Test')
    model = addEntity(model, { name: 'Order' })
    const result = buildGridNodes(model)
    expect(result.entityLaneNodes[0].draggable).toBe(false)
    expect(result.entityLaneNodes[0].selectable).toBe(false)
  })

  it('creates one column node per slice', () => {
    let model = createEventModel('Test')
    model = addSlice(model, { name: 'Upload PDF' })
    model = addSlice(model, { name: 'Transcribe' })
    const result = buildGridNodes(model)
    expect(result.sliceColumnNodes).toHaveLength(2)
  })

  it('slice column nodes have correct labels', () => {
    let model = createEventModel('Test')
    model = addSlice(model, { name: 'Upload PDF' })
    const result = buildGridNodes(model)
    expect(result.sliceColumnNodes[0].data.label).toBe('Upload PDF')
  })

  it('slice columns are positioned horizontally by order', () => {
    let model = createEventModel('Test')
    model = addSlice(model, { name: 'A' })
    model = addSlice(model, { name: 'B' })
    const result = buildGridNodes(model)
    expect(result.sliceColumnNodes[0].position.x).toBeLessThan(
      result.sliceColumnNodes[1].position.x,
    )
  })

  it('slice column nodes are not draggable or selectable', () => {
    let model = createEventModel('Test')
    model = addSlice(model, { name: 'S' })
    const result = buildGridNodes(model)
    expect(result.sliceColumnNodes[0].draggable).toBe(false)
    expect(result.sliceColumnNodes[0].selectable).toBe(false)
  })

  it('includes chapter info in slice data when slice has a chapter', () => {
    let model = createEventModel('Test')
    model = addChapter(model, { name: 'Evaluate' })
    model = addSlice(model, { name: 'Upload', chapterId: model.chapters[0].id })
    const result = buildGridNodes(model)
    expect(result.sliceColumnNodes[0].data.chapterName).toBe('Evaluate')
  })
})
