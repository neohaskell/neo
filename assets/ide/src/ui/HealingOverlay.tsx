interface HealingOverlayProps {
  message?: string
}

export function HealingOverlay({ message }: HealingOverlayProps) {
  return (
    <div
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="bg-white rounded-lg shadow-xl px-8 py-6 flex items-center gap-4">
        <div
          aria-hidden
          className="w-6 h-6 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin"
        />
        <div className="text-sm text-gray-800">
          {message ?? 'Healing event model…'}
        </div>
      </div>
    </div>
  )
}
