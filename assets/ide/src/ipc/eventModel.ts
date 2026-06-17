// Typed wrappers for the workspace/eventModel JSON-RPC methods. Mirrors
// `src/ide/methods/{read,write,heal}_event_model.rs`. The Rust backend is
// the authoritative validator: when `readEventModel` returns a non-`valid`
// status, the frontend should branch on it rather than re-parsing.

import type { IdeClient, RpcResult } from './client'

export type ValidationErrorKind = 'schema' | 'referentialIntegrity'

export interface ValidationError {
  /** JSON Pointer (RFC 6901) to the offending location. Empty = whole doc. */
  pointer: string
  /** Human-readable message — written for the dumbest LLM. */
  message: string
  kind: ValidationErrorKind
}

export type ValidationStatus =
  | { status: 'notFound' }
  | { status: 'valid' }
  | { status: 'invalid'; errors: ValidationError[] }
  | { status: 'malformedJson'; parseError: string }

export interface ReadEventModelResult {
  /** Raw file contents. `null` only when status is `notFound`. */
  content: string | null
  validation: ValidationStatus
}

export interface WriteEventModelResult {
  /** Absolute path the file landed at. Useful for "Saved to <path>" toasts. */
  path: string
}

export type HealOutcome =
  | { status: 'healed' }
  | { status: 'stillInvalid'; errors: ValidationError[] }

export interface HealEventModelResult {
  outcome: HealOutcome
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

/** How aggressively the server should invoke the agent. Matches the Rust
 *  enum `HealMode`. `validate` is the default for the auto-triggered modal
 *  (only spawns `claude` if validation fails). `improve` is for the manual
 *  "Heal with AI" button (always spawns `claude` to refine layout/edges). */
export type HealMode = 'validate' | 'improve'

export function healEventModel(
  client: IdeClient,
  mode: HealMode = 'validate',
): Promise<RpcResult<HealEventModelResult>> {
  return client.request<{ mode: HealMode }, HealEventModelResult>(
    'workspace/healEventModel',
    { mode },
  )
}
