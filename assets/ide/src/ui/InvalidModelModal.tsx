import type { ValidationError } from '../ipc/eventModel'

interface InvalidModelModalProps {
  errors: ValidationError[]
  /** Optional message above the error list (e.g. for malformed-JSON). */
  preamble?: string
  onHeal: () => void
  onCancel: () => void
}

export function InvalidModelModal({
  errors,
  preamble,
  onHeal,
  onCancel,
}: InvalidModelModalProps) {
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="invalid-model-modal-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40"
    >
      <div className="bg-white rounded-lg shadow-xl max-w-2xl w-full mx-4 flex flex-col max-h-[80vh]">
        <div className="px-6 py-4 border-b border-gray-200">
          <h2
            id="invalid-model-modal-title"
            className="text-lg font-semibold text-gray-900"
          >
            event-model.json is invalid
          </h2>
          <p className="mt-1 text-sm text-gray-600">
            {preamble ??
              'The file on disk does not match the event-model schema. You can ask an AI agent to heal it, or cancel and keep your local copy.'}
          </p>
        </div>
        <div className="px-6 py-4 overflow-y-auto flex-1">
          <ul className="space-y-2 text-sm font-mono text-gray-800">
            {errors.map((e, i) => (
              <li
                key={`${e.pointer}-${i}`}
                className="border-l-2 border-red-400 pl-3"
              >
                <div className="text-xs text-gray-500">
                  {e.pointer === '' ? '(whole document)' : e.pointer}{' '}
                  <span className="ml-2 text-gray-400">[{e.kind}]</span>
                </div>
                <div>{e.message}</div>
              </li>
            ))}
          </ul>
        </div>
        <div className="px-6 py-4 border-t border-gray-200 flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 text-sm rounded border border-gray-300 hover:bg-gray-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onHeal}
            className="px-4 py-2 text-sm rounded bg-indigo-600 text-white hover:bg-indigo-700"
          >
            Heal with AI
          </button>
        </div>
      </div>
    </div>
  )
}
