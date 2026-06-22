/** Zoom level at/above which nodes expand to reveal their schema fields. */
export const SEMANTIC_ZOOM_THRESHOLD = 1.5

/** Pure predicate (unit-testable) for the semantic-zoom reveal. */
export function shouldShowFields(zoom: number): boolean {
  return zoom >= SEMANTIC_ZOOM_THRESHOLD
}
