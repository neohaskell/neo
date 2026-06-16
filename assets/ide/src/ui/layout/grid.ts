import type { Node } from '@xyflow/react'
import type { EventModel } from '../../model/types'

const LANE_HEIGHT = 200
const LANE_WIDTH = 4000
const MIN_COLUMN_WIDTH = 200
const NODE_WIDTH = 120
const SLICE_PADDING = 40
const COLUMN_HEIGHT = 3000
const HEADER_HEIGHT = 40
// Space between slice headers and entity lanes for commands/queries/integrations/UIs
const TOP_MARGIN = 300

export interface SliceLayout {
  sliceId: string
  xStart: number
  width: number
}

export interface EntityLaneLayout {
  entityId: string
  yStart: number
  height: number
}

export function computeEntityLaneLayouts(model: EventModel): EntityLaneLayout[] {
  const sortedEntities = [...model.entities].sort((a, b) => a.order - b.order)
  return sortedEntities.map((entity, index) => ({
    entityId: entity.id,
    yStart: HEADER_HEIGHT + TOP_MARGIN + index * LANE_HEIGHT,
    height: LANE_HEIGHT,
  }))
}

export function getEntityAtY(layouts: EntityLaneLayout[], y: number): string | null {
  for (const layout of layouts) {
    if (y >= layout.yStart && y < layout.yStart + layout.height) {
      return layout.entityId
    }
  }
  return null
}

export interface ChapterArrowLayout {
  chapterId: string
  xStart: number
  width: number
}

export interface GridNodes {
  entityLaneNodes: Node[]
  sliceColumnNodes: Node[]
  chapterArrowNodes: Node[]
  sliceLayouts: SliceLayout[]
}

export function computeSliceLayouts(model: EventModel): SliceLayout[] {
  const sortedSlices = [...model.slices].sort((a, b) => a.order - b.order)
  const layouts: SliceLayout[] = []
  let xCursor = 0

  for (const slice of sortedSlices) {
    const sliceNodes = model.nodes.filter((n) => n.sliceId === slice.id)
    let width = MIN_COLUMN_WIDTH

    if (sliceNodes.length > 0) {
      let minX = Infinity
      let maxX = -Infinity
      for (const node of sliceNodes) {
        const pos = model.layout.nodePositions[node.id]
        if (pos) {
          minX = Math.min(minX, pos.x)
          maxX = Math.max(maxX, pos.x + NODE_WIDTH)
        }
      }
      if (minX !== Infinity) {
        const span = maxX - minX + SLICE_PADDING * 2
        width = Math.max(MIN_COLUMN_WIDTH, span)
      }
    }

    layouts.push({ sliceId: slice.id, xStart: xCursor, width })
    xCursor += width
  }

  return layouts
}

export interface NodePositionAdjustment {
  nodeId: string
  x: number
  y: number
}

/**
 * Computes position adjustments so that nodes in each slice are aligned
 * within their slice column. When a slice grows, nodes in subsequent
 * slices get pushed right by the same amount.
 */
export function computeNodeAlignments(model: EventModel): NodePositionAdjustment[] {
  const layouts = computeSliceLayouts(model)
  const adjustments: NodePositionAdjustment[] = []

  for (const layout of layouts) {
    const sliceNodes = model.nodes.filter((n) => n.sliceId === layout.sliceId)
    if (sliceNodes.length === 0) continue

    // Find the current minX of nodes in this slice
    let minX = Infinity
    for (const node of sliceNodes) {
      const pos = model.layout.nodePositions[node.id]
      if (pos) minX = Math.min(minX, pos.x)
    }
    if (minX === Infinity) continue

    // Nodes should start at xStart + padding
    const targetMinX = layout.xStart + SLICE_PADDING
    const dx = targetMinX - minX

    if (Math.abs(dx) < 1) continue

    for (const node of sliceNodes) {
      const pos = model.layout.nodePositions[node.id]
      if (pos) {
        adjustments.push({ nodeId: node.id, x: pos.x + dx, y: pos.y })
      }
    }
  }

  return adjustments
}

export const CHAPTER_ARROW_HEIGHT = 30
export const CHAPTER_ARROW_Y = -HEADER_HEIGHT - CHAPTER_ARROW_HEIGHT - 10
export const CHAPTER_UNASSIGNED_Y = CHAPTER_ARROW_Y - CHAPTER_ARROW_HEIGHT - 8
const CHAPTER_DEFAULT_WIDTH = 200

export function buildGridNodes(
  model: EventModel,
  onRenameEntity?: (entityId: string, name: string) => void,
  onRenameSlice?: (sliceId: string, name: string) => void,
  highlightedSliceId?: string | null,
  flashingSliceId?: string | null,
  onSliceSelect?: (sliceId: string) => void,
  highlightedEntityId?: string | null,
  flashingEntityId?: string | null,
  selectedEntityId?: string | null,
  onEntitySelect?: (entityId: string) => void,
  onRenameChapter?: (chapterId: string, name: string) => void,
  selectedChapterId?: string | null,
  onChapterSelect?: (chapterId: string) => void,
  onChapterEndDrag?: (chapterId: string, flowX: number) => void,
  onChapterEndDrop?: (chapterId: string, flowX: number) => void,
): GridNodes {
  const sortedEntities = [...model.entities].sort((a, b) => a.order - b.order)
  const sliceLayouts = computeSliceLayouts(model)

  const chapterMap = new Map(model.chapters.map((c) => [c.id, c.name]))

  const entityLaneNodes: Node[] = sortedEntities.map((entity, index) => ({
    id: `__entity-lane-${entity.id}`,
    type: 'entityLane',
    position: { x: -100, y: HEADER_HEIGHT + TOP_MARGIN + index * LANE_HEIGHT },
    data: {
      label: entity.name,
      entityId: entity.id,
      highlighted: (highlightedEntityId ?? selectedEntityId) === entity.id,
      flashing: flashingEntityId === entity.id,
      onRename: onRenameEntity
        ? (name: string) => onRenameEntity(entity.id, name)
        : undefined,
      onSelect: onEntitySelect
        ? () => onEntitySelect(entity.id)
        : undefined,
    },
    draggable: false,
    selectable: false,
    focusable: false,
    style: {
      width: LANE_WIDTH,
      height: LANE_HEIGHT,
      zIndex: -2,
      pointerEvents: 'all' as const,
    },
  }))

  const sliceMap = new Map(model.slices.map((s) => [s.id, s]))

  const sliceColumnNodes: Node[] = sliceLayouts.map((layout) => {
    const slice = sliceMap.get(layout.sliceId)!
    return {
      id: `__slice-col-${slice.id}`,
      type: 'sliceColumn',
      position: { x: layout.xStart, y: -HEADER_HEIGHT },
      data: {
        label: slice.name,
        sliceId: slice.id,
        chapterName: slice.chapterId ? chapterMap.get(slice.chapterId) ?? null : null,
        highlighted: highlightedSliceId === slice.id,
        flashing: flashingSliceId === slice.id,
        onRename: onRenameSlice
          ? (name: string) => onRenameSlice(slice.id, name)
          : undefined,
        onSelect: onSliceSelect
          ? () => onSliceSelect(slice.id)
          : undefined,
      },
      draggable: false,
      selectable: false,
      focusable: false,
      style: {
        width: layout.width,
        height: COLUMN_HEIGHT,
        zIndex: -1,
        pointerEvents: 'all' as const,
      },
    }
  })

  // Build chapter arrow nodes
  const sortedChapters = [...model.chapters].sort((a, b) => a.order - b.order)

  let unassignedIdx = 0
  const chapterArrowNodes: Node[] = sortedChapters.map((chapter) => {
    const chapterSliceIds = model.slices
      .filter((s) => s.chapterId === chapter.id)
      .map((s) => s.id)

    const matchingLayouts = sliceLayouts.filter((l) =>
      chapterSliceIds.includes(l.sliceId),
    )

    let xStart: number
    let width: number
    let yPos: number
    let isUnassigned: boolean

    if (matchingLayouts.length > 0) {
      xStart = Math.min(...matchingLayouts.map((l) => l.xStart))
      const xEnd = Math.max(...matchingLayouts.map((l) => l.xStart + l.width))
      width = xEnd - xStart
      yPos = CHAPTER_ARROW_Y
      isUnassigned = false
    } else {
      // No slices assigned — place above assigned chapters
      xStart = unassignedIdx * (CHAPTER_DEFAULT_WIDTH + 20)
      width = CHAPTER_DEFAULT_WIDTH
      yPos = CHAPTER_UNASSIGNED_Y
      isUnassigned = true
      unassignedIdx++
    }

    return {
      id: `__chapter-arrow-${chapter.id}`,
      type: 'chapterArrow',
      position: { x: xStart, y: yPos },
      data: {
        label: chapter.name,
        chapterId: chapter.id,
        unassigned: isUnassigned,
        selected: selectedChapterId === chapter.id,
        onSelect: onChapterSelect
          ? () => onChapterSelect(chapter.id)
          : undefined,
        onRename: onRenameChapter
          ? (name: string) => onRenameChapter(chapter.id, name)
          : undefined,
        onEndHandleDrag: onChapterEndDrag
          ? (flowX: number) => onChapterEndDrag(chapter.id, flowX)
          : undefined,
        onEndHandleDrop: onChapterEndDrop
          ? (flowX: number) => onChapterEndDrop(chapter.id, flowX)
          : undefined,
      },
      draggable: true,
      selectable: false,
      focusable: false,
      style: {
        width,
        height: CHAPTER_ARROW_HEIGHT,
        zIndex: 0,
        overflow: 'visible' as const,
        pointerEvents: 'all' as const,
      },
    }
  })

  return { entityLaneNodes, sliceColumnNodes, chapterArrowNodes, sliceLayouts }
}
