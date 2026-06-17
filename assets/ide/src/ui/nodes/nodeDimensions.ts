// Estimated rendered size of a domain node (event/command/query/etc.) based
// on its label length, shared between the layout math
// (`computeSliceLayouts`, `computeEntityLaneLayouts`) and the visual
// components so columns / lanes / positions all agree on how big a node
// actually is on screen.
//
// We don't measure the DOM here — layout math runs synchronously off model
// state with no DOM available — so we approximate with conservative
// constants tuned to the Tailwind classes the node components use
// (`px-4 py-2 text-sm`, default font ~7.5px per char at 14px size).

const CHAR_WIDTH = 8
/** Horizontal padding contributed by `px-4` (16px each side). */
const HORIZONTAL_PADDING = 32
/** Base height of a single-line node (`py-2` + line height of `text-sm`). */
const BASE_HEIGHT = 36
/** Line height inside a multi-line node at `text-sm`. */
const LINE_HEIGHT = 18

export const MIN_NODE_WIDTH = 120
export const MAX_NODE_WIDTH = 220
/** Extra breathing room added on top of the node's intrinsic size when
 *  computing the surrounding slice column / entity lane bounds. */
export const NODE_BREATHING_ROOM = 16

export interface NodeDimensions {
  width: number
  height: number
  lines: number
}

/**
 * Estimate the rendered `(width, height)` of a node containing `label`.
 *
 * - Width grows linearly with label length up to `MAX_NODE_WIDTH`, then
 *   wraps to additional lines.
 * - Height grows by `LINE_HEIGHT` per wrap.
 *
 * Both visual node components and the layout math import this so a node
 * never visually overflows the column / lane that contains it.
 */
export function estimateNodeDimensions(label: string): NodeDimensions {
  const naturalWidth = label.length * CHAR_WIDTH + HORIZONTAL_PADDING
  const width = Math.min(MAX_NODE_WIDTH, Math.max(MIN_NODE_WIDTH, naturalWidth))
  const usableWidth = Math.max(1, width - HORIZONTAL_PADDING)
  const charsPerLine = Math.max(1, Math.floor(usableWidth / CHAR_WIDTH))
  const lines = Math.max(1, Math.ceil(label.length / charsPerLine))
  const height = BASE_HEIGHT + (lines - 1) * LINE_HEIGHT
  return { width, height, lines }
}
