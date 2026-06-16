interface FileMenuProps {
  onNew: () => void
  onOpen: () => void
  onSave: () => void
  dirty: boolean
}

const btnClass =
  'px-3 py-1.5 text-sm font-medium rounded border border-gray-300 hover:bg-gray-100'

export function FileMenu({ onNew, onOpen, onSave, dirty }: FileMenuProps) {
  // Open and Save both flow through `neo ide`'s JSON-RPC bridge — they
  // operate on `<workspace_root>/event-model.json`, not a browser file picker.
  // The local-file import/export buttons could be added back later as a
  // separate `Import…`/`Export…` pair if needed.
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
      {dirty && (
        <span className="text-orange-500 text-lg" title="Unsaved changes">
          {'•'}
        </span>
      )}
    </div>
  )
}
