import type { IdeClient, RpcResult } from './client'

export type CollabRole = 'host' | 'join'

export interface CollabStatus {
  enabled: boolean
  role: CollabRole | null
  actorId: string | null
  peerCount: number
}

export interface MoveCommandInput {
  commandId: string
  actorId: string
  baseRevision: number
  nodeId: string
  x: number
  y: number
}

export interface CursorPosition {
  x: number
  y: number
}

export interface CollabPresence {
  actorId: string
  displayName: string
  color: string
  activeFeatureId: string
  cursor: CursorPosition | null
  selectedNodeIds: string[]
  updatedAtMs: number
}

export interface CollabEvent {
  sequence: number
  command: {
    commandId: string
    actorId: string
    baseRevision: number
    operation: {
      type: 'moveNode'
      nodeId: string
      x: number
      y: number
    }
  }
}

export type CollabNotification =
  | { type: 'snapshot'; sequence: number; content: string }
  | { type: 'event'; event: CollabEvent }
  | { type: 'commandRejected'; commandId: string; reason: string }
  | { type: 'presence'; presence: CollabPresence }
  | { type: 'peerCount'; count: number }
  | { type: 'repairRequired' }

export function collabStatus(client: IdeClient): Promise<RpcResult<CollabStatus>> {
  return client.request<Record<string, never>, CollabStatus>('collab/status', {})
}

export function submitMove(
  client: IdeClient,
  input: MoveCommandInput,
): Promise<RpcResult<{ accepted: boolean }>> {
  return client.request('collab/submit', {
    command: {
      commandId: input.commandId,
      actorId: input.actorId,
      baseRevision: input.baseRevision,
      operation: {
        type: 'moveNode',
        nodeId: input.nodeId,
        x: input.x,
        y: input.y,
      },
    },
  })
}

export function updatePresence(
  client: IdeClient,
  presence: CollabPresence,
): Promise<RpcResult<{ accepted: boolean }>> {
  return client.request('collab/updatePresence', { presence })
}
