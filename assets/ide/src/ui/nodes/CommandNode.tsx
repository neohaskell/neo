import { Handle, Position } from '@xyflow/react'
import { EditableLabel } from './EditableLabel'

interface Props {
  data: { label: string; onRename?: (name: string) => void }
  selected?: boolean
}

export function CommandNodeComponent({ data, selected }: Props) {
  return (
    <div className={`bg-blue-500 text-white px-4 py-2 rounded shadow text-sm font-medium min-w-[120px] text-center relative ${selected ? 'border-2 border-blue-600' : ''}`}>
      {selected && <div className="absolute inset-0 bg-blue-300/40 rounded pointer-events-none" />}
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
