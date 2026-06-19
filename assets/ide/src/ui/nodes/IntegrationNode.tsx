import { EditableLabel } from './EditableLabel'
import { NodeHandles } from './NodeHandles'
import { MAX_NODE_WIDTH, MIN_NODE_WIDTH } from './nodeDimensions'

interface Props {
  data: { label: string; kind: 'inbound' | 'outbound'; onRename?: (name: string) => void }
  selected?: boolean
}

export function IntegrationNodeComponent({ data, selected }: Props) {
  return (
    <div
      className={`bg-gray-500 text-white px-4 py-2 rounded shadow text-sm font-medium text-center relative break-words whitespace-normal ${selected ? 'border-2 border-blue-600' : ''}`}
      style={{ minWidth: MIN_NODE_WIDTH, maxWidth: MAX_NODE_WIDTH }}
    >
      {selected && <div className="absolute inset-0 bg-blue-500/30 rounded pointer-events-none" />}
      <NodeHandles />
      <span>{'\u2699'}</span>{' '}
      {data.onRename ? (
        <EditableLabel label={data.label} onRename={data.onRename} />
      ) : (
        data.label
      )}
    </div>
  )
}
