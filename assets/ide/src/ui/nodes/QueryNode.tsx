import { Handle, Position } from '@xyflow/react'
import { EditableLabel } from './EditableLabel'
import { MAX_NODE_WIDTH, MIN_NODE_WIDTH } from './nodeDimensions'

interface Props {
  data: { label: string; onRename?: (name: string) => void }
  selected?: boolean
}

export function QueryNodeComponent({ data, selected }: Props) {
  return (
    <div
      className={`bg-green-500 text-white px-4 py-2 rounded shadow text-sm font-medium text-center relative break-words whitespace-normal ${selected ? 'border-2 border-blue-600' : ''}`}
      style={{ minWidth: MIN_NODE_WIDTH, maxWidth: MAX_NODE_WIDTH }}
    >
      {selected && <div className="absolute inset-0 bg-blue-500/30 rounded pointer-events-none" />}
      <Handle id="top" type="source" position={Position.Top} />
      {data.onRename ? (
        <EditableLabel label={data.label} onRename={data.onRename} />
      ) : (
        data.label
      )}
      <Handle id="bottom" type="source" position={Position.Bottom} />
    </div>
  )
}
