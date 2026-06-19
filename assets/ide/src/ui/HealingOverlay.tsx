import { useEffect, useMemo, useRef } from 'react'
import { reduceHealLog, type HealEvent } from './healEvents'

export interface HealLogLine {
  /** Subprocess stream: "stdout" (claude's content / reasoning) or "stderr"
   *  (claude's status / errors). */
  stream: 'stdout' | 'stderr'
  line: string
}

interface HealingOverlayProps {
  message?: string
  /** Live tail of claude's output, streamed via `$/progress` notifications.
   *  Newest line at the bottom. Empty array shows just the spinner. */
  log?: HealLogLine[]
  /** Click handler for the Cancel button in the overlay header. When
   *  omitted, the button is not rendered (useful for the rare callers
   *  that just want a non-interruptible overlay). */
  onCancel?: () => void
  /** Disable the Cancel button while a cancel is already in flight, so
   *  rapid double-clicks don't fire repeat `cancelHealEventModel` calls. */
  cancelling?: boolean
}

export function HealingOverlay({ message, log, onCancel, cancelling }: HealingOverlayProps) {
  const scrollerRef = useRef<HTMLDivElement>(null)

  // Parse the raw streamed lines into a typed timeline of "thoughts".
  // Reducing the whole log on every render is fine — the timeline tops out
  // at a few hundred items per heal, and React/Tailwind paint is dominant.
  const events: HealEvent[] = useMemo(() => reduceHealLog(log ?? []), [log])

  useEffect(() => {
    const el = scrollerRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [events])

  return (
    <div
      role="status"
      aria-live="polite"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
    >
      <div className="bg-white rounded-lg shadow-xl flex flex-col w-[min(760px,92vw)] h-[min(620px,82vh)]">
        <div className="flex items-center gap-4 px-6 py-4 border-b border-gray-200">
          <div
            aria-hidden
            className="w-6 h-6 border-2 border-indigo-600 border-t-transparent rounded-full animate-spin shrink-0"
          />
          <div className="text-sm text-gray-800 flex-1">
            {message ?? 'Healing event model…'}
          </div>
          <div className="text-xs text-gray-500 shrink-0">
            {events.length === 0
              ? 'waiting for the agent…'
              : `${events.length} ${events.length === 1 ? 'step' : 'steps'}`}
          </div>
          {onCancel && (
            <button
              type="button"
              onClick={onCancel}
              disabled={cancelling}
              data-testid="heal-cancel"
              className="px-3 py-1.5 text-xs rounded border border-gray-300 text-gray-700 hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
            >
              {cancelling ? 'Cancelling…' : 'Cancel'}
            </button>
          )}
        </div>
        <div
          ref={scrollerRef}
          data-testid="heal-log"
          className="px-5 py-4 overflow-y-auto flex-1 bg-gray-50 space-y-3"
        >
          {events.length === 0 ? (
            <div className="text-sm text-gray-500 italic">
              The agent is starting up. Its reasoning, tool calls, and results
              will appear here as soon as the first output arrives.
            </div>
          ) : (
            events.map((event) => <HealEventCard key={event.id} event={event} />)
          )}
        </div>
      </div>
    </div>
  )
}

function HealEventCard({ event }: { event: HealEvent }) {
  switch (event.kind) {
    case 'thinking':
      return (
        <div
          data-testid="heal-event-thinking"
          className="border-l-2 border-gray-300 pl-3 text-sm text-gray-600 italic whitespace-pre-wrap break-words"
        >
          <div className="text-[10px] uppercase tracking-wider font-semibold text-gray-400 not-italic mb-0.5">
            thinking
          </div>
          {event.text || <span className="text-gray-400">…</span>}
        </div>
      )
    case 'text':
      return (
        <div
          data-testid="heal-event-text"
          className="text-sm text-gray-800 whitespace-pre-wrap break-words"
        >
          {event.text}
        </div>
      )
    case 'tool_use': {
      const formatted = tryFormatJson(event.input)
      return (
        <div
          data-testid="heal-event-tool-use"
          className="border border-indigo-100 bg-indigo-50/60 rounded-md px-3 py-2"
        >
          <div className="flex items-center gap-2 text-sm">
            <span className="font-mono text-indigo-700 font-semibold">
              {event.name}
            </span>
            <span className="text-xs text-indigo-500/70">tool call</span>
          </div>
          {formatted && (
            <pre className="mt-1 font-mono text-xs text-indigo-900/80 whitespace-pre-wrap break-words leading-snug">
              {formatted}
            </pre>
          )}
        </div>
      )
    }
    case 'tool_result':
      return (
        <div
          data-testid="heal-event-tool-result"
          className="border border-emerald-100 bg-emerald-50/60 rounded-md px-3 py-2"
        >
          <div className="text-[10px] uppercase tracking-wider font-semibold text-emerald-600 mb-1">
            result
          </div>
          <pre className="font-mono text-xs text-emerald-900/80 whitespace-pre-wrap break-words leading-snug">
            {event.preview}
            {event.truncated && (
              <span className="text-emerald-600/60"> …(truncated)</span>
            )}
          </pre>
        </div>
      )
    case 'status':
      return (
        <div
          data-testid="heal-event-status"
          className="text-xs text-gray-400 italic"
        >
          {event.label}…
        </div>
      )
    case 'auto_repair':
      return (
        <div
          data-testid="heal-event-auto-repair"
          className="border border-emerald-300 bg-emerald-50 rounded-md px-3 py-2"
        >
          <div className="flex items-center gap-2 text-sm text-emerald-900">
            <span aria-hidden className="text-emerald-600 font-semibold">✓</span>
            <span className="font-medium">Auto-repaired {event.appliedCount}{' '}
              {event.appliedCount === 1 ? 'item' : 'items'}</span>
            {event.residualCount > 0 && (
              <span className="text-emerald-700">
                · {event.residualCount} residual{event.residualCount === 1 ? '' : 's'} need
                LLM
              </span>
            )}
          </div>
          {event.summary && (
            <div className="text-xs text-emerald-800/80 mt-0.5">{event.summary}</div>
          )}
        </div>
      )
    case 'api_retry': {
      const reason =
        event.errorStatus === 529
          ? 'Anthropic API is overloaded'
          : event.errorStatus === 429
            ? 'Anthropic API rate-limited the request'
            : `Anthropic API returned HTTP ${event.errorStatus} (${event.error})`
      const delaySec = (event.delayMs / 1000).toFixed(1)
      return (
        <div
          data-testid="heal-event-api-retry"
          className="border border-amber-200 bg-amber-50 rounded-md px-3 py-2"
        >
          <div className="flex items-center gap-2 text-sm text-amber-900">
            <div
              aria-hidden
              className="w-3 h-3 border-2 border-amber-600 border-t-transparent rounded-full animate-spin shrink-0"
            />
            <span className="font-medium">{reason}.</span>
            <span className="text-amber-700">
              retrying attempt {event.attempt}
              {event.maxRetries > 0 && `/${event.maxRetries}`} in {delaySec}s…
            </span>
          </div>
        </div>
      )
    }
    case 'raw':
      return (
        <div
          data-testid="heal-event-raw"
          className={`font-mono text-xs whitespace-pre-wrap break-words ${
            event.stream === 'stderr' ? 'text-amber-700' : 'text-gray-500'
          }`}
        >
          {event.text}
        </div>
      )
  }
}

/** Best-effort pretty-print of a (possibly partial) JSON string. Returns
 *  the original text if it doesn't parse — we'd rather show partial JSON
 *  exactly than not show it at all. */
function tryFormatJson(input: string): string {
  if (!input) return ''
  try {
    return JSON.stringify(JSON.parse(input), null, 2)
  } catch {
    return input
  }
}
