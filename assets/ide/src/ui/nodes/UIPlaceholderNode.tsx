import { Handle, Position } from '@xyflow/react'
import { EditableLabel } from './EditableLabel'

interface Props {
  data: { label: string; onRename?: (name: string) => void }
  selected?: boolean
}

export function UIPlaceholderNodeComponent({ data, selected }: Props) {
  return (
    <div className={`border-2 ${selected ? 'border-blue-600' : 'border-dashed border-gray-400'} text-gray-600 px-4 py-2 rounded text-sm font-medium min-w-[120px] text-center bg-white relative`}>
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
