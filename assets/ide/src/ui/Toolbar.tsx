interface ToolbarProps {
  onAddEvent: () => void
  onAddCommand: () => void
  onAddQuery: () => void
  onAddIntegration: () => void
  onAddUIPlaceholder: () => void
  onAddEntity: () => void
  onAddSlice: () => void
  onAddChapter: () => void
  onAddSubmodel: () => void
}

const btnClass =
  'px-3 py-1.5 text-sm font-medium rounded border border-gray-300 hover:bg-gray-100'

export function Toolbar({
  onAddEvent,
  onAddCommand,
  onAddQuery,
  onAddIntegration,
  onAddUIPlaceholder,
  onAddEntity,
  onAddSlice,
  onAddChapter,
  onAddSubmodel,
}: ToolbarProps) {
  return (
    <div className="flex gap-2 p-2 border-b border-gray-200 bg-white flex-wrap">
      <button className={`${btnClass} bg-orange-100 text-orange-800`} onClick={onAddEvent}>
        + Event
      </button>
      <button className={`${btnClass} bg-blue-100 text-blue-800`} onClick={onAddCommand}>
        + Command
      </button>
      <button className={`${btnClass} bg-green-100 text-green-800`} onClick={onAddQuery}>
        + Query
      </button>
      <button className={`${btnClass} bg-gray-100 text-gray-800`} onClick={onAddIntegration}>
        + Integration
      </button>
      <button className={`${btnClass} bg-white text-gray-600`} onClick={onAddUIPlaceholder}>
        + UI Placeholder
      </button>
      <div className="w-px bg-gray-300" />
      <button className={`${btnClass}`} onClick={onAddEntity}>
        + Entity
      </button>
      <button className={`${btnClass}`} onClick={onAddSlice}>
        + Slice
      </button>
      <button className={`${btnClass}`} onClick={onAddChapter}>
        + Chapter
      </button>
      <button className={`${btnClass} bg-indigo-100 text-indigo-800`} onClick={onAddSubmodel}>
        + Submodel
      </button>
    </div>
  )
}
