interface FileMenuProps {
  onNew: () => void
  onOpen: () => void
  onSave: () => void
  onHeal: () => void
  onRelayout: () => void
  dirty: boolean
  healing: boolean
  relayouting: boolean
}

const btnClass =
  'px-3 py-1.5 text-sm font-medium rounded border border-gray-300 hover:bg-gray-100'
const healBtnClass =
  'px-3 py-1.5 text-sm font-medium rounded border border-indigo-300 text-indigo-700 hover:bg-indigo-50 disabled:opacity-50 disabled:cursor-not-allowed'
const relayoutBtnClass =
  'px-3 py-1.5 text-sm font-medium rounded border border-emerald-300 text-emerald-700 hover:bg-emerald-50 disabled:opacity-50 disabled:cursor-not-allowed'

export function FileMenu({
  onNew,
  onOpen,
  onSave,
  onHeal,
  onRelayout,
  dirty,
  healing,
  relayouting,
}: FileMenuProps) {
  // Open / Save / Heal / Re-layout all flow through `neo ide`'s
  // JSON-RPC bridge — they operate on `<workspace_root>/event-model.json`,
  // not a browser file picker.
  return (
    <div className="flex items-center gap-2 p-2 border-b border-gray-200 bg-white">
      <button className={btnClass} onClick={onNew}>
        New
      </button>
      <button className={btnClass} onClick={onOpen}>
        Open
      </button>
      <button className={btnClass} onClick={onSave}>
        Save
      </button>
      <button
        className={relayoutBtnClass}
        onClick={onRelayout}
        disabled={relayouting || healing}
        title="Clean up layout (chapters, slice columns, node positions) without changing the model structure or spawning the AI."
      >
        {relayouting ? 'Re-laying out…' : 'Re-layout'}
      </button>
      <button
        className={healBtnClass}
        onClick={onHeal}
        disabled={healing || relayouting}
        title="Ask Claude to improve the model (fix layout, add inferred edges, etc.)"
      >
        {healing ? 'Healing…' : 'Heal with AI'}
      </button>
      {dirty && (
        <span className="text-orange-500 text-lg" title="Unsaved changes">
          {'•'}
        </span>
      )}
    </div>
  )
}
