import { describe, it, expect } from 'vitest'
import {
  createEventModel,
  addEvent,
  addCommand,
  addQuery,
  addIntegration,
  addUIPlaceholder,
  removeNode,
  addEdge,
  removeEdge,
  addEntity,
  removeEntity,
  assignEventToEntity,
  addChapter,
  removeChapter,
  addSlice,
  removeSlice,
  reorderEventsInEntity,
  updateNodeName,
  addSubmodel,
  renameSubmodel,
  removeSubmodel,
  assignChapterToSubmodel,
} from './operations'
import type {
  EventModel,
  CommandProducesEvent,
  EventFeedsQuery,
  IntegrationTriggersCommand,
  CommandFromUI,
  QueryToUI,
} from './types'

// ── Helpers ─────────────────────────────────────────────────

function modelWithEvent(name = 'OrderPlaced'): EventModel {
  const model = createEventModel('Test')
  return addEvent(model, { name })
}

function modelWithCommandAndEvent(): EventModel {
  let model = createEventModel('Test')
  const entity = { name: 'Order' }
  model = addEntity(model, entity)
  const entityId = model.entities[0].id
  model = addEvent(model, { name: 'OrderPlaced', entityId })
  model = addCommand(model, { name: 'PlaceOrder', entityId })
  return model
}

// ── createEventModel ────────────────────────────────────────

describe('createEventModel', () => {
  it('creates a model with the given name', () => {
    const model = createEventModel('My Model')
    expect(model.name).toBe('My Model')
  })

  it('creates an empty model', () => {
    const model = createEventModel('Empty')
    expect(model.nodes).toEqual([])
    expect(model.edges).toEqual([])
    expect(model.entities).toEqual([])
    expect(model.chapters).toEqual([])
    expect(model.slices).toEqual([])
  })

  it('has a default viewport', () => {
    const model = createEventModel('V')
    expect(model.layout.viewport).toEqual({ x: 0, y: 0, zoom: 1 })
  })

  it('generates a unique id', () => {
    const a = createEventModel('A')
    const b = createEventModel('B')
    expect(a.id).not.toBe(b.id)
  })
})

// ── addEvent ────────────────────────────────────────────────

describe('addEvent', () => {
  it('adds an event node', () => {
    const model = modelWithEvent('UserRegistered')
    expect(model.nodes).toHaveLength(1)
    expect(model.nodes[0].type).toBe('event')
    expect(model.nodes[0].name).toBe('UserRegistered')
  })

  it('assigns a unique id', () => {
    let model = createEventModel('T')
    model = addEvent(model, { name: 'A' })
    model = addEvent(model, { name: 'B' })
    expect(model.nodes[0].id).not.toBe(model.nodes[1].id)
  })

  it('defaults entityId to null', () => {
    const model = modelWithEvent()
    const node = model.nodes[0]
    expect(node.type === 'event' && node.entityId).toBeNull()
  })

  it('accepts an entityId', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = addEvent(model, { name: 'Placed', entityId })
    const node = model.nodes[0]
    expect(node.type === 'event' && node.entityId).toBe(entityId)
  })

  it('does not mutate the original model', () => {
    const original = createEventModel('T')
    const updated = addEvent(original, { name: 'E' })
    expect(original.nodes).toHaveLength(0)
    expect(updated.nodes).toHaveLength(1)
  })
})

// ── addCommand ──────────────────────────────────────────────

describe('addCommand', () => {
  it('adds a command node', () => {
    let model = createEventModel('T')
    model = addCommand(model, { name: 'PlaceOrder' })
    expect(model.nodes).toHaveLength(1)
    expect(model.nodes[0].type).toBe('command')
    expect(model.nodes[0].name).toBe('PlaceOrder')
  })

  it('defaults entityId to null', () => {
    let model = createEventModel('T')
    model = addCommand(model, { name: 'Cmd' })
    const node = model.nodes[0]
    expect(node.type === 'command' && node.entityId).toBeNull()
  })
})

// ── addQuery ────────────────────────────────────────────────

describe('addQuery', () => {
  it('adds a query node', () => {
    let model = createEventModel('T')
    model = addQuery(model, { name: 'OrderSummary' })
    expect(model.nodes).toHaveLength(1)
    expect(model.nodes[0].type).toBe('query')
    expect(model.nodes[0].name).toBe('OrderSummary')
  })
})

// ── addIntegration ──────────────────────────────────────────

describe('addIntegration', () => {
  it('adds an outbound integration', () => {
    let model = createEventModel('T')
    model = addIntegration(model, { name: 'SendEmail', kind: 'outbound' })
    expect(model.nodes).toHaveLength(1)
    const node = model.nodes[0]
    expect(node.type).toBe('integration')
    expect(node.type === 'integration' && node.kind).toBe('outbound')
  })

  it('adds an inbound integration', () => {
    let model = createEventModel('T')
    model = addIntegration(model, { name: 'PollPayments', kind: 'inbound' })
    const node = model.nodes[0]
    expect(node.type === 'integration' && node.kind).toBe('inbound')
  })
})

// ── addUIPlaceholder ────────────────────────────────────────

describe('addUIPlaceholder', () => {
  it('adds a UI placeholder', () => {
    let model = createEventModel('T')
    model = addUIPlaceholder(model, { name: 'Order Form' })
    expect(model.nodes).toHaveLength(1)
    expect(model.nodes[0].type).toBe('uiPlaceholder')
    expect(model.nodes[0].name).toBe('Order Form')
  })
})

// ── removeNode ──────────────────────────────────────────────

describe('removeNode', () => {
  it('removes a node by id', () => {
    let model = createEventModel('T')
    model = addEvent(model, { name: 'E1' })
    model = addEvent(model, { name: 'E2' })
    const idToRemove = model.nodes[0].id
    model = removeNode(model, idToRemove)
    expect(model.nodes).toHaveLength(1)
    expect(model.nodes[0].name).toBe('E2')
  })

  it('removes associated edges when removing a node', () => {
    let model = modelWithCommandAndEvent()
    const cmdId = model.nodes.find((n) => n.type === 'command')!.id
    const evtId = model.nodes.find((n) => n.type === 'event')!.id
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: cmdId,
      targetId: evtId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
    model = removeNode(model, cmdId)
    expect(model.edges).toHaveLength(0)
  })

  it('returns model unchanged if node not found', () => {
    const model = modelWithEvent()
    const result = removeNode(model, 'nonexistent')
    expect(result).toBe(model)
  })
})

// ── addEdge ─────────────────────────────────────────────────

describe('addEdge', () => {
  it('adds a commandProducesEvent edge', () => {
    let model = modelWithCommandAndEvent()
    const cmdId = model.nodes.find((n) => n.type === 'command')!.id
    const evtId = model.nodes.find((n) => n.type === 'event')!.id
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: cmdId,
      targetId: evtId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
    expect(model.edges[0].type).toBe('commandProducesEvent')
  })

  it('rejects edge if source node does not exist', () => {
    let model = modelWithEvent()
    const evtId = model.nodes[0].id
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: 'nonexistent',
      targetId: evtId,
    }
    expect(() => addEdge(model, edge)).toThrow()
  })

  it('rejects edge if target node does not exist', () => {
    let model = createEventModel('T')
    model = addCommand(model, { name: 'Cmd' })
    const cmdId = model.nodes[0].id
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: cmdId,
      targetId: 'nonexistent',
    }
    expect(() => addEdge(model, edge)).toThrow()
  })

  it('rejects commandProducesEvent if source is not a command', () => {
    let model = createEventModel('T')
    model = addEvent(model, { name: 'E1' })
    model = addEvent(model, { name: 'E2' })
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: model.nodes[0].id,
      targetId: model.nodes[1].id,
    }
    expect(() => addEdge(model, edge)).toThrow()
  })

  it('rejects commandProducesEvent if target is not an event', () => {
    let model = createEventModel('T')
    model = addCommand(model, { name: 'C1' })
    model = addCommand(model, { name: 'C2' })
    const edge: CommandProducesEvent = {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: model.nodes[0].id,
      targetId: model.nodes[1].id,
    }
    expect(() => addEdge(model, edge)).toThrow()
  })

  it('adds an eventFeedsQuery edge', () => {
    let model = createEventModel('T')
    model = addEvent(model, { name: 'E' })
    model = addQuery(model, { name: 'Q' })
    const evtId = model.nodes.find((n) => n.type === 'event')!.id
    const qId = model.nodes.find((n) => n.type === 'query')!.id
    const edge: EventFeedsQuery = {
      id: 'e1',
      type: 'eventFeedsQuery',
      sourceId: evtId,
      targetId: qId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
  })

  it('rejects eventFeedsQuery if source is not an event', () => {
    let model = createEventModel('T')
    model = addCommand(model, { name: 'C' })
    model = addQuery(model, { name: 'Q' })
    const edge: EventFeedsQuery = {
      id: 'e1',
      type: 'eventFeedsQuery',
      sourceId: model.nodes[0].id,
      targetId: model.nodes[1].id,
    }
    expect(() => addEdge(model, edge)).toThrow()
  })

  it('adds an integrationTriggersCommand edge', () => {
    let model = createEventModel('T')
    model = addIntegration(model, { name: 'I', kind: 'outbound' })
    model = addCommand(model, { name: 'C' })
    const iId = model.nodes.find((n) => n.type === 'integration')!.id
    const cId = model.nodes.find((n) => n.type === 'command')!.id
    const edge: IntegrationTriggersCommand = {
      id: 'e1',
      type: 'integrationTriggersCommand',
      sourceId: iId,
      targetId: cId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
  })

  it('adds a commandFromUI edge', () => {
    let model = createEventModel('T')
    model = addUIPlaceholder(model, { name: 'UI' })
    model = addCommand(model, { name: 'C' })
    const uiId = model.nodes.find((n) => n.type === 'uiPlaceholder')!.id
    const cId = model.nodes.find((n) => n.type === 'command')!.id
    const edge: CommandFromUI = {
      id: 'e1',
      type: 'commandFromUI',
      sourceId: uiId,
      targetId: cId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
  })

  it('adds a queryToUI edge', () => {
    let model = createEventModel('T')
    model = addQuery(model, { name: 'Q' })
    model = addUIPlaceholder(model, { name: 'UI' })
    const qId = model.nodes.find((n) => n.type === 'query')!.id
    const uiId = model.nodes.find((n) => n.type === 'uiPlaceholder')!.id
    const edge: QueryToUI = {
      id: 'e1',
      type: 'queryToUI',
      sourceId: qId,
      targetId: uiId,
    }
    model = addEdge(model, edge)
    expect(model.edges).toHaveLength(1)
  })
})

// ── removeEdge ──────────────────────────────────────────────

describe('removeEdge', () => {
  it('removes an edge by id', () => {
    let model = modelWithCommandAndEvent()
    const cmdId = model.nodes.find((n) => n.type === 'command')!.id
    const evtId = model.nodes.find((n) => n.type === 'event')!.id
    model = addEdge(model, {
      id: 'e1',
      type: 'commandProducesEvent',
      sourceId: cmdId,
      targetId: evtId,
    })
    model = removeEdge(model, 'e1')
    expect(model.edges).toHaveLength(0)
  })

  it('returns model unchanged if edge not found', () => {
    const model = createEventModel('T')
    const result = removeEdge(model, 'nonexistent')
    expect(result).toBe(model)
  })
})

// ── addEntity ───────────────────────────────────────────────

describe('addEntity', () => {
  it('adds an entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    expect(model.entities).toHaveLength(1)
    expect(model.entities[0].name).toBe('Order')
  })

  it('assigns sequential order', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'A' })
    model = addEntity(model, { name: 'B' })
    expect(model.entities[0].order).toBe(0)
    expect(model.entities[1].order).toBe(1)
  })
})

// ── removeEntity ────────────────────────────────────────────

describe('removeEntity', () => {
  it('removes an entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = removeEntity(model, entityId)
    expect(model.entities).toHaveLength(0)
  })

  it('unassigns events from removed entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = addEvent(model, { name: 'E', entityId })
    model = removeEntity(model, entityId)
    const node = model.nodes[0]
    expect(node.type === 'event' && node.entityId).toBeNull()
  })

  it('unassigns commands from removed entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = addCommand(model, { name: 'C', entityId })
    model = removeEntity(model, entityId)
    const node = model.nodes[0]
    expect(node.type === 'command' && node.entityId).toBeNull()
  })
})

// ── assignEventToEntity ─────────────────────────────────────

describe('assignEventToEntity', () => {
  it('assigns an event to an entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    model = addEvent(model, { name: 'E' })
    const entityId = model.entities[0].id
    const eventId = model.nodes[0].id
    model = assignEventToEntity(model, eventId, entityId)
    const node = model.nodes[0]
    expect(node.type === 'event' && node.entityId).toBe(entityId)
  })

  it('unassigns event when entityId is null', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = addEvent(model, { name: 'E', entityId })
    const eventId = model.nodes[0].id
    model = assignEventToEntity(model, eventId, null)
    const node = model.nodes[0]
    expect(node.type === 'event' && node.entityId).toBeNull()
  })
})

// ── addChapter ──────────────────────────────────────────────

describe('addChapter', () => {
  it('adds a chapter', () => {
    let model = createEventModel('T')
    model = addChapter(model, { name: 'Registration' })
    expect(model.chapters).toHaveLength(1)
    expect(model.chapters[0].name).toBe('Registration')
  })

  it('assigns sequential order', () => {
    let model = createEventModel('T')
    model = addChapter(model, { name: 'A' })
    model = addChapter(model, { name: 'B' })
    expect(model.chapters[0].order).toBe(0)
    expect(model.chapters[1].order).toBe(1)
  })
})

// ── removeChapter ───────────────────────────────────────────

describe('removeChapter', () => {
  it('removes a chapter', () => {
    let model = createEventModel('T')
    model = addChapter(model, { name: 'Ch' })
    const chId = model.chapters[0].id
    model = removeChapter(model, chId)
    expect(model.chapters).toHaveLength(0)
  })

  it('unassigns slices from removed chapter', () => {
    let model = createEventModel('T')
    model = addChapter(model, { name: 'Ch' })
    const chId = model.chapters[0].id
    model = addSlice(model, { name: 'S', chapterId: chId })
    model = removeChapter(model, chId)
    expect(model.slices[0].chapterId).toBeNull()
  })
})

// ── addSlice ────────────────────────────────────────────────

describe('addSlice', () => {
  it('adds a slice', () => {
    let model = createEventModel('T')
    model = addSlice(model, { name: 'Upload PDF' })
    expect(model.slices).toHaveLength(1)
    expect(model.slices[0].name).toBe('Upload PDF')
  })

  it('assigns sequential order', () => {
    let model = createEventModel('T')
    model = addSlice(model, { name: 'A' })
    model = addSlice(model, { name: 'B' })
    expect(model.slices[0].order).toBe(0)
    expect(model.slices[1].order).toBe(1)
  })

  it('defaults chapterId to null', () => {
    let model = createEventModel('T')
    model = addSlice(model, { name: 'S' })
    expect(model.slices[0].chapterId).toBeNull()
  })
})

// ── removeSlice ─────────────────────────────────────────────

describe('removeSlice', () => {
  it('removes a slice', () => {
    let model = createEventModel('T')
    model = addSlice(model, { name: 'S' })
    const sId = model.slices[0].id
    model = removeSlice(model, sId)
    expect(model.slices).toHaveLength(0)
  })

  it('unassigns nodes from removed slice', () => {
    let model = createEventModel('T')
    model = addSlice(model, { name: 'S' })
    const sliceId = model.slices[0].id
    model = addEvent(model, { name: 'E' })
    // manually assign slice — we test via the model structure
    const nodeId = model.nodes[0].id
    model = {
      ...model,
      nodes: model.nodes.map((n) =>
        n.id === nodeId ? { ...n, sliceId } : n,
      ),
    }
    model = removeSlice(model, sliceId)
    expect(model.nodes[0].sliceId).toBeNull()
  })
})

// ── reorderEventsInEntity ───────────────────────────────────

describe('reorderEventsInEntity', () => {
  it('reorders events within an entity', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'Order' })
    const entityId = model.entities[0].id
    model = addEvent(model, { name: 'A', entityId })
    model = addEvent(model, { name: 'B', entityId })
    model = addEvent(model, { name: 'C', entityId })
    const ids = model.nodes.map((n) => n.id)
    // reverse the order
    model = reorderEventsInEntity(model, entityId, [ids[2], ids[1], ids[0]])
    const entityEvents = model.nodes.filter(
      (n) => n.type === 'event' && n.entityId === entityId,
    )
    expect(entityEvents.map((n) => n.name)).toEqual(['C', 'B', 'A'])
  })

  it('does not affect events in other entities', () => {
    let model = createEventModel('T')
    model = addEntity(model, { name: 'A' })
    model = addEntity(model, { name: 'B' })
    const eIdA = model.entities[0].id
    const eIdB = model.entities[1].id
    model = addEvent(model, { name: 'A1', entityId: eIdA })
    model = addEvent(model, { name: 'B1', entityId: eIdB })
    model = addEvent(model, { name: 'A2', entityId: eIdA })
    const aEvents = model.nodes.filter(
      (n) => n.type === 'event' && n.entityId === eIdA,
    )
    model = reorderEventsInEntity(model, eIdA, [
      aEvents[1].id,
      aEvents[0].id,
    ])
    const bEvents = model.nodes.filter(
      (n) => n.type === 'event' && n.entityId === eIdB,
    )
    expect(bEvents).toHaveLength(1)
    expect(bEvents[0].name).toBe('B1')
  })
})

// ── updateNodeName ──────────────────────────────────────────

describe('updateNodeName', () => {
  it('updates the name of a node', () => {
    let model = modelWithEvent('OldName')
    const nodeId = model.nodes[0].id
    model = updateNodeName(model, nodeId, 'NewName')
    expect(model.nodes[0].name).toBe('NewName')
  })

  it('returns model unchanged if node not found', () => {
    const model = modelWithEvent()
    const result = updateNodeName(model, 'nonexistent', 'X')
    expect(result).toBe(model)
  })
})

// ── Submodels ───────────────────────────────────────────────

describe('submodels', () => {
  it('createEventModel starts with an empty submodels array', () => {
    expect(createEventModel('T').submodels).toEqual([])
  })

  it('addSubmodel appends with incrementing order', () => {
    let model = createEventModel('T')
    model = addSubmodel(model, { name: 'Checkout' })
    model = addSubmodel(model, { name: 'Fulfilment' })
    expect(model.submodels.map((s) => [s.name, s.order])).toEqual([
      ['Checkout', 0],
      ['Fulfilment', 1],
    ])
  })

  it('renameSubmodel changes only the named submodel', () => {
    let model = addSubmodel(createEventModel('T'), { name: 'Old' })
    const id = model.submodels[0].id
    model = renameSubmodel(model, id, 'New')
    expect(model.submodels[0].name).toBe('New')
  })

  it('assignChapterToSubmodel sets and clears submodelId', () => {
    let model = addChapter(createEventModel('T'), { name: 'Cart' })
    model = addSubmodel(model, { name: 'Checkout' })
    const chapterId = model.chapters[0].id
    const submodelId = model.submodels[0].id
    model = assignChapterToSubmodel(model, chapterId, submodelId)
    expect(model.chapters[0].submodelId).toBe(submodelId)
    model = assignChapterToSubmodel(model, chapterId, null)
    expect(model.chapters[0].submodelId).toBeNull()
  })

  it('assignChapterToSubmodel is a no-op for unknown chapter or submodel', () => {
    let model = addChapter(createEventModel('T'), { name: 'Cart' })
    model = addSubmodel(model, { name: 'Checkout' })
    const chapterId = model.chapters[0].id
    expect(assignChapterToSubmodel(model, 'ghost', model.submodels[0].id)).toBe(model)
    expect(assignChapterToSubmodel(model, chapterId, 'ghost')).toBe(model)
  })

  it('removeSubmodel drops the submodel and detaches (not deletes) its chapters', () => {
    let model = addChapter(createEventModel('T'), { name: 'Cart' })
    model = addSubmodel(model, { name: 'Checkout' })
    const chapterId = model.chapters[0].id
    const submodelId = model.submodels[0].id
    model = assignChapterToSubmodel(model, chapterId, submodelId)
    model = removeSubmodel(model, submodelId)
    expect(model.submodels).toEqual([])
    expect(model.chapters).toHaveLength(1)
    expect(model.chapters[0].submodelId).toBeNull()
  })
})
