import { EditableLabel } from './EditableLabel'

interface Props {
  data: {
    label: string
    onRename?: (name: string) => void
    onDelete?: () => void
  }
}

// A submodel band is a translucent full-bleed rectangle drawn BEHIND the
// graph (lowest z-index) that visually contains all of a feature's chapters
// and slices. Its body is pointer-transparent so nodes inside stay
// interactive; only the top-left label chip captures clicks (rename/delete).
export function SubmodelBandNodeComponent({ data }: Props) {
  return (
    <div className="w-full h-full rounded-lg border-2 border-indigo-300 bg-indigo-50/40 relative pointer-events-none">
      <div className="absolute left-3 top-2 flex items-center gap-2 pointer-events-auto">
        <span className="text-sm font-bold uppercase tracking-wide text-indigo-500">
          {data.onRename ? (
            <EditableLabel label={data.label} onRename={data.onRename} />
          ) : (
            data.label
          )}
        </span>
        {data.onDelete && (
          <button
            type="button"
            onClick={data.onDelete}
            className="text-indigo-300 hover:text-red-500 text-xs leading-none px-1"
            title="Remove submodel (keeps its chapters)"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  )
}
