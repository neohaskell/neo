import { EditableLabel } from './EditableLabel'

interface Props {
  data: {
    label: string
    chapterName?: string | null
    highlighted?: boolean
    flashing?: boolean
    onRename?: (name: string) => void
    onSelect?: () => void
  }
}

export function SliceColumnNodeComponent({ data }: Props) {
  return (
    <div
      className={`w-full h-full border relative pointer-events-none transition-colors duration-150 ${
        data.flashing
          ? 'animate-flash-red'
          : data.highlighted
            ? 'border-blue-400 bg-blue-50/40'
            : 'border-gray-200'
      }`}
    >
      <div
        className={`absolute top-0 left-0 right-0 h-10 flex items-center justify-center border-b pointer-events-auto z-10 transition-colors duration-150 cursor-pointer ${
          data.flashing
            ? 'animate-flash-red'
            : data.highlighted
              ? 'border-blue-400 bg-blue-100'
              : 'border-gray-200 bg-white'
        }`}
        onClick={data.onSelect}
      >
        <span
          className={`text-xs font-semibold truncate px-2 ${
            data.highlighted ? 'text-blue-700' : 'text-gray-600'
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
