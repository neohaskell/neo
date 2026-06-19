import { useMemo, useCallback, useState, useEffect, useRef } from 'react'
import {
  ReactFlow,
  Background,
  Controls,
  ConnectionMode,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  type Connection,
  type Node,
  type Edge,
  applyNodeChanges,
  applyEdgeChanges,
} from '@xyflow/react'
import '@xyflow/react/dist/style.css'
import { nodeTypes } from './nodes'
import { toReactFlowNodes, toReactFlowEdges } from './adapter'
import { buildGridNodes, computeSliceLayouts, computeEntityLaneLayouts, getEntityAtY, CHAPTER_ARROW_Y, type SliceLayout } from './layout/grid'
import { buildSubmodelBandNodes, submodelsInUse } from './layout/submodels'
import type { EventModel } from '../model/types'

interface CanvasProps {
  model: EventModel
  onPositionChange?: (nodeId: string, x: number, y: number) => void
  onConnect?: (sourceId: string, targetId: string, sourceHandle: string | null, targetHandle: string | null) => void
  onNodesDelete?: (nodeIds: string[]) => void
  onEdgesDelete?: (edgeIds: string[]) => void
  onNodeRename?: (nodeId: string, name: string) => void
  onEntityRename?: (entityId: string, name: string) => void
  onSliceRename?: (sliceId: string, name: string) => void
  onAssignNodeToSlice?: (nodeId: string, sliceId: string | null, x: number, y: number) => void
  onAssignNodeToEntity?: (nodeId: string, entityId: string | null) => void
  onSliceDelete?: (sliceId: string) => void
  onEntityDelete?: (entityId: string) => void
  onChapterRename?: (chapterId: string, name: string) => void
  onChapterSliceRange?: (chapterId: string, startSliceId: string, endSliceId: string) => void
  onChapterDelete?: (chapterId: string) => void
  onSubmodelRename?: (submodelId: string, name: string) => void
  onSubmodelDelete?: (submodelId: string) => void
  onAssignChapterToSubmodel?: (chapterId: string, submodelId: string | null) => void
  flashingSliceId?: string | null
  flashingEntityId?: string | null
}

function getSliceAtX(layouts: SliceLayout[], x: number): string | null {
  for (const layout of layouts) {
    if (x >= layout.xStart && x < layout.xStart + layout.width) {
      return layout.sliceId
    }
  }
  return null
}

const CHAPTER_ARROW_PREFIX = '__chapter-arrow-'

/** Returns the slice at x only if it's free or already owned by chapterId */
function getAvailableSliceAtX(
  layouts: SliceLayout[],
  x: number,
  slices: readonly { id: string; chapterId: string | null }[],
  chapterId: string,
): string | null {
  const sliceId = getSliceAtX(layouts, x)
  if (!sliceId) return null
  const slice = slices.find((s) => s.id === sliceId)
  if (!slice) return null
  if (slice.chapterId !== null && slice.chapterId !== chapterId) return null
  return sliceId
}

export function Canvas({
  model,
  onPositionChange,
  onConnect: onConnectProp,
  onNodesDelete,
  onEdgesDelete,
  onNodeRename,
  onEntityRename,
  onSliceRename,
  onAssignNodeToSlice,
  onAssignNodeToEntity,
  onSliceDelete,
  onEntityDelete,
  onChapterRename,
  onChapterSliceRange,
  onChapterDelete,
  onSubmodelRename,
  onSubmodelDelete,
  onAssignChapterToSubmodel,
  flashingSliceId,
  flashingEntityId,
}: CanvasProps) {
  const [highlightedSliceId, setHighlightedSliceId] = useState<string | null>(null)
  const [highlightedEntityId, setHighlightedEntityId] = useState<string | null>(null)
  const [selectedSliceId, setSelectedSliceId] = useState<string | null>(null)
  const [selectedEntityId, setSelectedEntityId] = useState<string | null>(null)
  const [selectedChapterId, setSelectedChapterId] = useState<string | null>(null)
  const selectedSliceRef = useRef<string | null>(null)
  selectedSliceRef.current = selectedSliceId
  const selectedEntityRef = useRef<string | null>(null)
  selectedEntityRef.current = selectedEntityId
  const selectedChapterRef = useRef<string | null>(null)
  selectedChapterRef.current = selectedChapterId
  const draggingNodeRef = useRef<string | null>(null)
  const draggingNodeTypeRef = useRef<string | null>(null)
  // Track chapter arrow drag state
  const draggingChapterRef = useRef<string | null>(null)
  const chapterStartSliceRef = useRef<string | null>(null)
  const chapterDragOriginRef = useRef<{ x: number; y: number } | null>(null)
  const sliceLayouts = useMemo(() => computeSliceLayouts(model), [model])
  const sliceLayoutsRef = useRef(sliceLayouts)
  sliceLayoutsRef.current = sliceLayouts
  const entityLaneLayouts = useMemo(() => computeEntityLaneLayouts(model), [model])
  const entityLaneLayoutsRef = useRef(entityLaneLayouts)
  entityLaneLayoutsRef.current = entityLaneLayouts

  const domainNodes = useMemo(() => toReactFlowNodes(model, onNodeRename), [model, onNodeRename])
  const modelEdges = useMemo(() => toReactFlowEdges(model), [model])
  const [edges, setEdges] = useState<Edge[]>(modelEdges)

  useEffect(() => {
    setEdges(modelEdges)
  }, [modelEdges])
  const handleSliceSelect = useCallback((sliceId: string) => {
    setSelectedSliceId((prev) => (prev === sliceId ? null : sliceId))
    setSelectedEntityId(null)
    setSelectedChapterId(null)
  }, [])

  const handleEntitySelect = useCallback((entityId: string) => {
    setSelectedEntityId((prev) => (prev === entityId ? null : entityId))
    setSelectedSliceId(null)
    setSelectedChapterId(null)
  }, [])

  const handleChapterSelect = useCallback((chapterId: string) => {
    setSelectedChapterId((prev) => (prev === chapterId ? null : chapterId))
    setSelectedSliceId(null)
    setSelectedEntityId(null)
  }, [])

  // Chapter end-handle drag: highlight slice at pointer x (only if available)
  const handleChapterEndDrag = useCallback(
    (chapterId: string, flowX: number) => {
      const sliceId = getAvailableSliceAtX(sliceLayoutsRef.current, flowX, model.slices, chapterId)
      setHighlightedSliceId(sliceId)
    },
    [model.slices],
  )

  // Chapter end-handle drop: set range from start to end slice
  const handleChapterEndDrop = useCallback(
    (chapterId: string, flowX: number) => {
      setHighlightedSliceId(null)
      const endSliceId = getAvailableSliceAtX(sliceLayoutsRef.current, flowX, model.slices, chapterId)
      if (!endSliceId) return

      // Find start slice: the slice the chapter arrow's left edge is over
      const chapterSlices = model.slices.filter((s) => s.chapterId === chapterId)
      let startSliceId: string | null = null
      if (chapterSlices.length > 0) {
        const sorted = [...chapterSlices].sort((a, b) => a.order - b.order)
        startSliceId = sorted[0].id
      } else if (chapterStartSliceRef.current) {
        startSliceId = chapterStartSliceRef.current
      }
      if (!startSliceId) {
        startSliceId = endSliceId
      }
      onChapterSliceRange?.(chapterId, startSliceId, endSliceId)
    },
    [model.slices, onChapterSliceRange],
  )

  const activeHighlight = highlightedSliceId ?? selectedSliceId

  const gridNodes = useMemo(
    () => buildGridNodes(
      model, onEntityRename, onSliceRename, activeHighlight, flashingSliceId,
      handleSliceSelect, highlightedEntityId, flashingEntityId, selectedEntityId,
      handleEntitySelect, onChapterRename, selectedChapterId, handleChapterSelect,
      handleChapterEndDrag, handleChapterEndDrop, model.submodels, onAssignChapterToSubmodel,
    ),
    [model, onEntityRename, onSliceRename, activeHighlight, flashingSliceId, handleSliceSelect, highlightedEntityId, flashingEntityId, selectedEntityId, handleEntitySelect, onChapterRename, selectedChapterId, handleChapterSelect, handleChapterEndDrag, handleChapterEndDrop, onAssignChapterToSubmodel],
  )

  const submodelBandNodes = useMemo(
    () => buildSubmodelBandNodes(model, onSubmodelRename, onSubmodelDelete),
    [model, onSubmodelRename, onSubmodelDelete],
  )

  // When submodels are in use they become the vertical organiser. The
  // entity swim-lanes (full-width horizontal bands) and slice columns
  // (full-height vertical strips) both assume ONE shared timeline, so they
  // visually fight the stacked bands — suppress them and let the submodel
  // bands carry the grouping. Chapter arrows stay (they host the
  // submodel-assignment control).
  const inUseSubmodels = submodelsInUse(model)

  // Background nodes first (submodel bands behind everything), then domain nodes.
  const modelNodes = useMemo(
    () => [
      ...submodelBandNodes,
      ...(inUseSubmodels ? [] : gridNodes.entityLaneNodes),
      ...(inUseSubmodels ? [] : gridNodes.sliceColumnNodes),
      ...gridNodes.chapterArrowNodes,
      ...domainNodes,
    ],
    [submodelBandNodes, inUseSubmodels, gridNodes, domainNodes],
  )

  const [nodes, setNodes] = useState<Node[]>(modelNodes)

  useEffect(() => {
    setNodes(modelNodes)
  }, [modelNodes])

  const handleNodesChange: OnNodesChange = useCallback(
    (changes) => {
      setNodes((nds) => applyNodeChanges(changes, nds))

      for (const change of changes) {
        // When a domain node is selected, clear slice/entity selection
        if (change.type === 'select' && change.selected && !change.id.startsWith('__')) {
          setSelectedSliceId(null)
          setSelectedEntityId(null)
          setSelectedChapterId(null)
        }

        if (change.type !== 'position') continue

        // Handle chapter arrow dragging (constrain to Y, highlight slice)
        if (change.id.startsWith(CHAPTER_ARROW_PREFIX)) {
          const chapterId = change.id.slice(CHAPTER_ARROW_PREFIX.length)
          if (change.dragging && change.position) {
            // Save origin on first drag event
            if (!chapterDragOriginRef.current) {
              const currentNode = nodes.find((n) => n.id === change.id)
              if (currentNode) {
                chapterDragOriginRef.current = { ...currentNode.position }
              }
            }
            // Constrain Y: assigned chapters stay on their row,
            // unassigned chapters can drag freely (they'll snap on drop)
            const node = nodes.find((n) => n.id === change.id)
            if (!node?.data?.unassigned) {
              change.position.y = CHAPTER_ARROW_Y
            }
            const sliceId = getAvailableSliceAtX(sliceLayoutsRef.current, change.position.x, model.slices, chapterId)
            setHighlightedSliceId(sliceId)
            draggingChapterRef.current = chapterId
            if (sliceId) chapterStartSliceRef.current = sliceId
          } else if (!change.dragging && change.position) {
            // Dropped — assign start slice or snap back
            setHighlightedSliceId(null)
            const sliceId = getAvailableSliceAtX(sliceLayoutsRef.current, change.position.x, model.slices, chapterId)
            if (sliceId && draggingChapterRef.current) {
              const chapterSlices = model.slices.filter((s) => s.chapterId === chapterId)
              if (chapterSlices.length === 0) {
                onChapterSliceRange?.(chapterId, sliceId, sliceId)
              } else {
                const sorted = [...chapterSlices].sort((a, b) => a.order - b.order)
                const endSliceId = sorted[sorted.length - 1].id
                onChapterSliceRange?.(chapterId, sliceId, endSliceId)
              }
            } else {
              // Invalid drop target — snap back to origin
              const origin = chapterDragOriginRef.current
              if (origin) {
                const nodeId = change.id
                // Defer snap-back to avoid conflicting with applyNodeChanges
                requestAnimationFrame(() => {
                  setNodes((nds) =>
                    nds.map((n) =>
                      n.id === nodeId
                        ? { ...n, position: { x: origin.x, y: origin.y } }
                        : n,
                    ),
                  )
                })
              }
            }
            draggingChapterRef.current = null
            chapterStartSliceRef.current = null
            chapterDragOriginRef.current = null
          }
          continue
        }

        // Skip other __ nodes (slice columns, entity lanes)
        if (change.id.startsWith('__')) continue

        if (change.dragging && change.position) {
          // Node is being dragged — highlight the slice under it
          draggingNodeRef.current = change.id
          const nodeType = model.nodes.find((n) => n.id === change.id)?.type ?? null
          draggingNodeTypeRef.current = nodeType
          const sliceId = getSliceAtX(sliceLayoutsRef.current, change.position.x)
          setHighlightedSliceId(sliceId)
          // Only highlight entity lanes for event nodes
          if (nodeType === 'event') {
            const entityId = getEntityAtY(entityLaneLayoutsRef.current, change.position.y)
            setHighlightedEntityId(entityId)
          }
        } else if (!change.dragging && change.position) {
          // Node was dropped — assign to slice and entity, then clear highlights
          const sliceId = getSliceAtX(sliceLayoutsRef.current, change.position.x)
          if (draggingNodeRef.current) {
            onAssignNodeToSlice?.(draggingNodeRef.current, sliceId, change.position.x, change.position.y)
            // Assign event to entity on drop
            if (draggingNodeTypeRef.current === 'event') {
              const entityId = getEntityAtY(entityLaneLayoutsRef.current, change.position.y)
              onAssignNodeToEntity?.(draggingNodeRef.current, entityId)
            }
            draggingNodeRef.current = null
            draggingNodeTypeRef.current = null
          } else {
            onPositionChange?.(change.id, change.position.x, change.position.y)
          }
          setHighlightedSliceId(null)
          setHighlightedEntityId(null)
        }
      }
    },
    [onPositionChange, onAssignNodeToSlice, onAssignNodeToEntity, onChapterSliceRange, model.nodes, model.slices],
  )

  const handleConnect: OnConnect = useCallback(
    (connection: Connection) => {
      if (onConnectProp && connection.source && connection.target) {
        onConnectProp(connection.source, connection.target, connection.sourceHandle ?? null, connection.targetHandle ?? null)
      }
    },
    [onConnectProp],
  )

  const handleNodesDelete = useCallback(
    (deleted: { id: string }[]) => {
      // Don't delete grid nodes
      const domainDeleted = deleted.filter((n) => !n.id.startsWith('__'))
      if (domainDeleted.length > 0) {
        onNodesDelete?.(domainDeleted.map((n) => n.id))
      }
    },
    [onNodesDelete],
  )

  const handleEdgesChange: OnEdgesChange = useCallback(
    (changes) => {
      setEdges((eds) => applyEdgeChanges(changes, eds))
    },
    [],
  )

  const handleEdgesDelete = useCallback(
    (deleted: { id: string }[]) => {
      onEdgesDelete?.(deleted.map((e) => e.id))
    },
    [onEdgesDelete],
  )

  const handlePaneClick = useCallback(() => {
    setSelectedSliceId(null)
    setSelectedEntityId(null)
    setSelectedChapterId(null)
  }, [])

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Backspace') {
        if (selectedChapterRef.current) {
          onChapterDelete?.(selectedChapterRef.current)
          setSelectedChapterId(null)
        } else if (selectedSliceRef.current) {
          onSliceDelete?.(selectedSliceRef.current)
        } else if (selectedEntityRef.current) {
          onEntityDelete?.(selectedEntityRef.current)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onSliceDelete, onEntityDelete, onChapterDelete])

  return (
    <div className="w-full h-full" data-testid="canvas">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={handleConnect}
        onNodesDelete={handleNodesDelete}
        onEdgesDelete={handleEdgesDelete}
        onPaneClick={handlePaneClick}
        connectionMode={ConnectionMode.Loose}
        deleteKeyCode="Backspace"
        zoomOnDoubleClick={false}
        fitView
        fitViewOptions={{ nodes: nodes.filter((n) => n.type !== 'entityLane' && n.type !== 'sliceColumn' && n.type !== 'chapterArrow' && n.type !== 'submodelBand') }}
      >
        <Background />
        <Controls />
      </ReactFlow>
    </div>
  )
}
