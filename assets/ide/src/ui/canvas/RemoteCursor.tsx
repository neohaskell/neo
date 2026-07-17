import type { CollabPresence } from '../../ipc/collab'
import classes from './RemoteCursor.module.css'

export function RemoteCursor({ presence }: { presence: CollabPresence }) {
  if (!presence.cursor) return null
  return (
    <div
      className={classes.cursor}
      style={{
        transform: `translate(${presence.cursor.x}px, ${presence.cursor.y}px)`,
        '--cursor-color': presence.color,
      } as React.CSSProperties}
      data-testid={`remote-cursor-${presence.actorId}`}
    >
      <span className={classes.pointer} />
      <span className={classes.label}>{presence.displayName}</span>
    </div>
  )
}
