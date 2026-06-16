import { MarkerType, type Node, type Edge } from '@xyflow/react'
import type { EventModel, ModelNode } from '../model/types'

export interface PositionChange {
  id: string
  x: number
  y: number
}

function nodeData(
  node: ModelNode,
  onRename?: (nodeId: string, name: string) => void,
): Record<string, unknown> {
  const data: Record<string, unknown> = { label: node.name }
  if (node.type === 'integration') {
    data.kind = node.kind
  }
  if (onRename) {
    data.onRename = (name: string) => onRename(node.id, name)
  }
  return data
}

export function toReactFlowNodes(
  model: EventModel,
  onRename?: (nodeId: string, name: string) => void,
): Node[] {
  return model.nodes.map((node) => ({
    id: node.id,
    type: node.type,
    position: model.layout.nodePositions[node.id] ?? { x: 0, y: 0 },
    data: nodeData(node, onRename),
  }))
}

export function toReactFlowEdges(model: EventModel): Edge[] {
  return model.edges.map((edge) => ({
    id: edge.id,
    source: edge.sourceId,
    target: edge.targetId,
    sourceHandle: edge.sourceHandle ?? undefined,
    targetHandle: edge.targetHandle ?? undefined,
    type: 'default',
    style: { strokeWidth: 3, stroke: '#000' },
    markerEnd: { type: MarkerType.ArrowClosed, width: 12, height: 12, color: '#000' },
  }))
}

export function applyPositionChanges(
  model: EventModel,
  changes: PositionChange[],
): EventModel {
  const updatedPositions = { ...model.layout.nodePositions }
  for (const change of changes) {
    updatedPositions[change.id] = { x: change.x, y: change.y }
  }
  return {
    ...model,
    layout: { ...model.layout, nodePositions: updatedPositions },
  }
}
