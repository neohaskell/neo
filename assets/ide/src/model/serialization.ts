import type { EventModel } from './types'

export interface ValidationError {
  readonly message: string
}

export function serialize(model: EventModel): string {
  return JSON.stringify(model, null, 2)
}

export function deserialize(json: string): EventModel {
  let raw: unknown
  try {
    raw = JSON.parse(json)
  } catch {
    throw new Error('Invalid JSON')
  }

  if (typeof raw !== 'object' || raw === null) {
    throw new Error('Expected an object')
  }

  const obj = raw as Record<string, unknown>

  if (typeof obj.id !== 'string') throw new Error('Missing or invalid "id"')
  if (typeof obj.name !== 'string') throw new Error('Missing or invalid "name"')
  if (!Array.isArray(obj.nodes)) throw new Error('Missing or invalid "nodes"')
  if (!Array.isArray(obj.edges)) throw new Error('Missing or invalid "edges"')
  if (!Array.isArray(obj.entities)) throw new Error('Missing or invalid "entities"')
  if (!Array.isArray(obj.chapters)) throw new Error('Missing or invalid "chapters"')
  if (!Array.isArray(obj.slices)) throw new Error('Missing or invalid "slices"')
  if (typeof obj.layout !== 'object' || obj.layout === null) {
    throw new Error('Missing or invalid "layout"')
  }

  const model = obj as unknown as EventModel

  const errors = validateModel(model)
  if (errors.length > 0) {
    throw new Error(`Invalid model: ${errors.map((e) => e.message).join(', ')}`)
  }

  return model
}

export function validateModel(model: EventModel): ValidationError[] {
  const errors: ValidationError[] = []
  const nodeIds = new Set(model.nodes.map((n) => n.id))
  const entityIds = new Set(model.entities.map((e) => e.id))
  const chapterIds = new Set(model.chapters.map((c) => c.id))

  for (const edge of model.edges) {
    if (!nodeIds.has(edge.sourceId)) {
      errors.push({ message: `Edge ${edge.id}: source node ${edge.sourceId} not found` })
    }
    if (!nodeIds.has(edge.targetId)) {
      errors.push({ message: `Edge ${edge.id}: target node ${edge.targetId} not found` })
    }
  }

  for (const node of model.nodes) {
    if ('entityId' in node && node.entityId !== null) {
      if (!entityIds.has(node.entityId)) {
        errors.push({
          message: `Node ${node.id}: entity ${node.entityId} not found`,
        })
      }
    }
  }

  for (const slice of model.slices) {
    if (slice.chapterId !== null && !chapterIds.has(slice.chapterId)) {
      errors.push({
        message: `Slice ${slice.id}: chapter ${slice.chapterId} not found`,
      })
    }
  }

  return errors
}
