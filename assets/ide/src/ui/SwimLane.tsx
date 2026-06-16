import type { SwimLaneLayout } from './layout/swimlanes'

interface SwimLaneOverlayProps {
  lanes: SwimLaneLayout[]
}

const LANE_COLORS = [
  'bg-orange-50',
  'bg-blue-50',
  'bg-green-50',
  'bg-purple-50',
  'bg-pink-50',
  'bg-yellow-50',
]

export function SwimLaneOverlay({ lanes }: SwimLaneOverlayProps) {
  return (
    <>
      {lanes.map((lane, index) => (
        <div
          key={lane.entityId}
          data-testid={`swimlane-${lane.entityId}`}
          className={`absolute pointer-events-none border-b border-gray-200 ${LANE_COLORS[index % LANE_COLORS.length]}`}
          style={{
            top: lane.yStart,
            left: 0,
            right: 0,
            height: lane.yEnd - lane.yStart,
          }}
        >
          <span className="absolute top-1 left-2 text-xs font-semibold text-gray-500 uppercase tracking-wide">
            {lane.name}
          </span>
        </div>
      ))}
    </>
  )
}
