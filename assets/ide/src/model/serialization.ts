// Thin JSON pass-throughs. The Rust backend (`src/ide/validate.rs` +
// `assets/ide/src/model/event-model.schema.json`) is the authoritative
// validator for the on-disk shape; by the time content reaches
// `deserialize` here, it has already been schema-checked and is
// guaranteed to match `EventModel`. If you find yourself adding
// runtime checks here, the boundary moved — fix it on the backend.

import type { EventModel } from './types'

export function serialize(model: EventModel): string {
  return JSON.stringify(model, null, 2)
}

export function deserialize(json: string): EventModel {
  return JSON.parse(json) as EventModel
}
