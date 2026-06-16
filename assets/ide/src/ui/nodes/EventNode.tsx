import { Handle, Position } from '@xyflow/react'
import { EditableLabel } from './EditableLabel'

interface Props {
  data: { label: string; onRename?: (name: string) => void }
  selected?: boolean
}

export function EventNodeComponent({ data, selected }: Props) {
  return (
    <div className={`bg-orange-400 text-white px-4 py-2 rounded shadow text-sm font-medium min-w-[120px] text-center relative ${selected ? 'border-2 border-blue-600' : ''}`}>
      {selected && <div className="absolute inset-0 bg-blue-500/30 rounded pointer-events-none" />}
      <Handle id="left" type="source" position={Position.Left} />
      {data.onRename ? (
        <EditableLabel label={data.label} onRename={data.onRename} />
      ) : (
        data.label
      )}
      <Handle id="right" type="source" position={Position.Right} />
    </div>
  )
}
