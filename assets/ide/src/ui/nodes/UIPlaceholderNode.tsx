import { EditableLabel } from './EditableLabel'
import { NodeHandles } from './NodeHandles'
import { MAX_NODE_WIDTH, MIN_NODE_WIDTH } from './nodeDimensions'

interface Props {
  data: { label: string; onRename?: (name: string) => void }
  selected?: boolean
}

export function UIPlaceholderNodeComponent({ data, selected }: Props) {
  return (
    <div
      className={`border-2 ${selected ? 'border-blue-600' : 'border-dashed border-gray-400'} text-gray-600 px-4 py-2 rounded text-sm font-medium text-center bg-white relative break-words whitespace-normal`}
      style={{ minWidth: MIN_NODE_WIDTH, maxWidth: MAX_NODE_WIDTH }}
    >
      {selected && <div className="absolute inset-0 bg-blue-500/30 rounded pointer-events-none" />}
      <NodeHandles />
      {data.onRename ? (
        <EditableLabel label={data.label} onRename={data.onRename} />
      ) : (
        data.label
      )}
    </div>
  )
}
