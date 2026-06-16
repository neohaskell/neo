import { useCallback, useRef, useState } from 'react'
import { useReactFlow, useNodeId } from '@xyflow/react'
import { EditableLabel } from './EditableLabel'

interface Props {
  data: {
    label: string
    chapterId: string
    selected?: boolean
    onSelect?: () => void
    onRename?: (name: string) => void
    onEndHandleDrag?: (flowX: number) => void
    onEndHandleDrop?: (flowX: number) => void
  }
}

const MIN_ARROW_WIDTH = 80

export function ChapterArrowNodeComponent({ data }: Props) {
  const { screenToFlowPosition, getNode } = useReactFlow()
  const nodeId = useNodeId()!
  const draggingEnd = useRef(false)
  // Width override during drag (in flow px, relative to node left edge)
  const [dragWidth, setDragWidth] = useState<number | null>(null)

  const handleEndPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.stopPropagation()
      e.preventDefault()
      draggingEnd.current = true
      ;(e.target as HTMLElement).setPointerCapture(e.pointerId)
    },
    [],
  )

  const handleEndPointerMove = useCallback(
    (e: React.PointerEvent) => {
      if (!draggingEnd.current) return
      const flowPos = screenToFlowPosition({ x: e.clientX, y: e.clientY })
      data.onEndHandleDrag?.(flowPos.x)

      const node = getNode(nodeId)
      if (node) {
        const newWidth = Math.max(MIN_ARROW_WIDTH, flowPos.x - node.position.x)
        setDragWidth(newWidth)
      }
    },
    [screenToFlowPosition, data, getNode, nodeId],
  )

  const handleEndPointerUp = useCallback(
    (e: React.PointerEvent) => {
      if (!draggingEnd.current) return
      draggingEnd.current = false
      setDragWidth(null)
      const flowPos = screenToFlowPosition({ x: e.clientX, y: e.clientY })
      data.onEndHandleDrop?.(flowPos.x)
    },
    [screenToFlowPosition, data],
  )

  const handleClick = useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation()
      data.onSelect?.()
    },
    [data],
  )

  return (
    <div className="w-full h-full relative" style={{ overflow: 'visible' }} onClick={handleClick}>
      {/* Selection highlight */}
      {data.selected && (
        <div className="absolute -inset-1 rounded border-2 border-blue-400 bg-blue-100/40 pointer-events-none" style={{ overflow: 'visible' }} />
      )}
      {/* Arrow container — uses dragWidth override during resize, otherwise fills node */}
      <div
        className="absolute top-0 left-0 h-full flex items-center"
        style={{ width: dragWidth != null ? dragWidth : '100%' }}
      >
        {/* Arrow line */}
        <div className="absolute top-1/2 left-0 right-[14px] h-[3px] bg-blue-500 -translate-y-1/2" />
        {/* Arrowhead — draggable end handle */}
        <div
          className="absolute right-0 top-1/2 -translate-y-1/2 cursor-ew-resize z-10"
          style={{
            width: 0,
            height: 0,
            borderTop: '10px solid transparent',
            borderBottom: '10px solid transparent',
            borderLeft: '16px solid #3b82f6',
          }}
          onPointerDown={handleEndPointerDown}
          onPointerMove={handleEndPointerMove}
          onPointerUp={handleEndPointerUp}
        />
        {/* Label */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-auto">
          <span className="bg-white px-3 py-0.5 text-blue-600 font-semibold text-sm whitespace-nowrap">
            {data.onRename ? (
              <EditableLabel label={data.label} onRename={data.onRename} />
            ) : (
              data.label
            )}
          </span>
        </div>
      </div>
    </div>
  )
}
