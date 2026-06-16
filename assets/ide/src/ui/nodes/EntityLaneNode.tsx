import { EditableLabel } from './EditableLabel'

interface Props {
  data: {
    label: string
    highlighted?: boolean
    flashing?: boolean
    onRename?: (name: string) => void
    onSelect?: () => void
  }
}

export function EntityLaneNodeComponent({ data }: Props) {
  return (
    <div className={`w-full h-full border relative pointer-events-none transition-colors duration-150 ${
      data.flashing
        ? 'animate-flash-red'
        : data.highlighted
          ? 'border-orange-400 bg-orange-50/40'
          : 'border-gray-200 bg-gray-50/50'
    }`}>
      {/* Clickable label column — the 100px strip before slices start */}
      <div
        className={`absolute left-0 top-0 w-[100px] h-full pointer-events-auto cursor-pointer z-10 flex items-start px-2 pt-2`}
        onClick={data.onSelect}
      >
        <span
          className={`text-xs font-bold uppercase tracking-wide ${
            data.flashing
              ? 'animate-flash-red'
              : data.highlighted
                ? 'text-orange-500'
                : 'text-gray-400'
          }`}
        >
          {data.onRename ? (
            <EditableLabel label={data.label} onRename={data.onRename} />
          ) : (
            data.label
          )}
        </span>
      </div>
    </div>
  )
}
