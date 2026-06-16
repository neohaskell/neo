import { describe, it, expect } from 'vitest'
import { serialize, deserialize, validateModel } from './serialization'
import {
  createEventModel,
  addEvent,
  addCommand,
  addQuery,
  addIntegration,
  addUIPlaceholder,
  addEdge,
  addEntity,
  addChapter,
  addSlice,
} from './operations'
import type { EventModel } from './types'

function fullModel(): EventModel {
  let model = createEventModel('Full Model')
  model = addEntity(model, { name: 'Order' })
  model = addChapter(model, { name: 'Ordering' })
  model = addSlice(model, { name: 'Place Order', chapterId: model.chapters[0].id })

  const entityId = model.entities[0].id
  model = addEvent(model, { name: 'OrderPlaced', entityId })
  model = addCommand(model, { name: 'PlaceOrder', entityId })
  model = addQuery(model, { name: 'OrderSummary' })
  model = addIntegration(model, { name: 'SendConfirmation', kind: 'outbound' })
  model = addUIPlaceholder(model, { name: 'Order Form' })

  const cmdId = model.nodes.find((n) => n.type === 'command')!.id
  const evtId = model.nodes.find((n) => n.type === 'event')!.id
  const qId = model.nodes.find((n) => n.type === 'query')!.id
  const intId = model.nodes.find((n) => n.type === 'integration')!.id
  const uiId = model.nodes.find((n) => n.type === 'uiPlaceholder')!.id

  model = addEdge(model, {
    id: 'edge-1',
    type: 'commandProducesEvent',
    sourceId: cmdId,
    targetId: evtId,
  })
  model = addEdge(model, {
    id: 'edge-2',
    type: 'eventFeedsQuery',
    sourceId: evtId,
    targetId: qId,
  })
  model = addEdge(model, {
    id: 'edge-3',
    type: 'integrationTriggersCommand',
    sourceId: intId,
    targetId: cmdId,
  })
  model = addEdge(model, {
    id: 'edge-4',
    type: 'commandFromUI',
    sourceId: uiId,
    targetId: cmdId,
  })
  model = addEdge(model, {
    id: 'edge-5',
    type: 'queryToUI',
    sourceId: qId,
    targetId: uiId,
  })

  return model
}

describe('serialize / deserialize', () => {
  it('round-trips an empty model', () => {
    const model = createEventModel('Empty')
    const json = serialize(model)
    const restored = deserialize(json)
    expect(restored).toEqual(model)
  })

  it('round-trips a full model with all node and edge types', () => {
    const model = fullModel()
    const json = serialize(model)
    const restored = deserialize(json)
    expect(restored).toEqual(model)
  })

  it('produces valid JSON', () => {
    const model = fullModel()
    const json = serialize(model)
    expect(() => JSON.parse(json)).not.toThrow()
  })

  it('rejects malformed JSON', () => {
    expect(() => deserialize('not json')).toThrow()
  })

  it('rejects JSON missing required fields', () => {
    expect(() => deserialize(JSON.stringify({ id: '1' }))).toThrow()
  })

  it('rejects model with edge referencing nonexistent node', () => {
    const model = createEventModel('Bad')
    const badModel = {
      ...model,
      edges: [
        {
          id: 'e1',
          type: 'commandProducesEvent',
          sourceId: 'nonexistent',
          targetId: 'also-nonexistent',
        },
      ],
    }
    const json = JSON.stringify(badModel)
    expect(() => deserialize(json)).toThrow()
  })
})

describe('validateModel', () => {
  it('returns no errors for a valid model', () => {
    const model = fullModel()
    const errors = validateModel(model)
    expect(errors).toEqual([])
  })

  it('returns no errors for an empty model', () => {
    const model = createEventModel('Empty')
    const errors = validateModel(model)
    expect(errors).toEqual([])
  })

  it('returns errors for edges with missing source', () => {
    const model = createEventModel('Bad')
    const badModel: EventModel = {
      ...model,
      edges: [
        {
          id: 'e1',
          type: 'commandProducesEvent',
          sourceId: 'ghost',
          targetId: 'phantom',
        },
      ],
    }
    const errors = validateModel(badModel)
    expect(errors.length).toBeGreaterThan(0)
    expect(errors[0].message).toContain('ghost')
  })

  it('returns errors for slices referencing nonexistent chapters', () => {
    const model = createEventModel('Bad')
    const badModel: EventModel = {
      ...model,
      slices: [{ id: 's1', name: 'S', chapterId: 'missing', order: 0 }],
    }
    const errors = validateModel(badModel)
    expect(errors.length).toBeGreaterThan(0)
  })

  it('returns errors for nodes referencing nonexistent entities', () => {
    const model = createEventModel('Bad')
    const badModel: EventModel = {
      ...model,
      nodes: [
        {
          id: 'n1',
          type: 'event',
          name: 'E',
          entityId: 'missing',
          sliceId: null,
        },
      ],
    }
    const errors = validateModel(badModel)
    expect(errors.length).toBeGreaterThan(0)
  })
})
