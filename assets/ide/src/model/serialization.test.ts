import { describe, it, expect } from 'vitest'
import { serialize, deserialize } from './serialization'
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
})
