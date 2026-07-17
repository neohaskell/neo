import { describe, expect, it, vi } from 'vitest'
import { collabStatus, submitMove, updatePresence } from './collab'
import type { IdeClient } from './client'

describe('collaboration RPC', () => {
  it('submits a deterministic move command with caller-owned identity', async () => {
    const request = vi.fn().mockResolvedValue({ ok: true, result: { accepted: true } })
    const client = { request } as unknown as IdeClient

    await submitMove(client, {
      commandId: 'cmd-1',
      actorId: 'actor-a',
      baseRevision: 4,
      nodeId: 'node-1',
      x: 12,
      y: 34,
    })

    expect(request).toHaveBeenCalledWith('collab/submit', {
      command: {
        commandId: 'cmd-1',
        actorId: 'actor-a',
        baseRevision: 4,
        operation: { type: 'moveNode', nodeId: 'node-1', x: 12, y: 34 },
      },
    })
  })

  it('uses dedicated status and presence methods', async () => {
    const request = vi.fn().mockResolvedValue({ ok: true, result: { accepted: true } })
    const client = { request } as unknown as IdeClient
    const presence = {
      actorId: 'actor-a',
      displayName: 'Ada',
      color: '#228be6',
      activeFeatureId: 'feature-a',
      cursor: { x: 1, y: 2 },
      selectedNodeIds: ['node-1'],
      updatedAtMs: 42,
    }

    await collabStatus(client)
    await updatePresence(client, presence)

    expect(request).toHaveBeenNthCalledWith(1, 'collab/status', {})
    expect(request).toHaveBeenNthCalledWith(2, 'collab/updatePresence', { presence })
  })
})
