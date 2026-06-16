// Typed wrappers for `workspace/readEventModel` and `workspace/writeEventModel`.
// Mirrors `src/ide/methods/read_event_model.rs` and `write_event_model.rs`.
//
// Content is a raw JSON string — the frontend's `model/serialization.ts`
// stays the source of truth for the on-disk shape. The Rust handlers are a
// typed filesystem proxy, nothing more.

import type { IdeClient, RpcResult } from './client'

export interface ReadEventModelResult {
  content: string | null
}

export interface WriteEventModelResult {
  /// Absolute path the file landed at. Useful for "Saved to <path>" toasts.
  path: string
}

export function readEventModel(
  client: IdeClient,
): Promise<RpcResult<ReadEventModelResult>> {
  return client.request<Record<string, never>, ReadEventModelResult>(
    'workspace/readEventModel',
    {},
  )
}

export function writeEventModel(
  client: IdeClient,
  content: string,
): Promise<RpcResult<WriteEventModelResult>> {
  return client.request<{ content: string }, WriteEventModelResult>(
    'workspace/writeEventModel',
    { content },
  )
}
