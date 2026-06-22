import type { ReactNode } from 'react'
import { useStore } from '@xyflow/react'
import { EditableLabel } from '../nodes/EditableLabel'
import { NodeHandles } from '../nodes/NodeHandles'
import { FieldsEditor } from '../schema/FieldsEditor'
import { shouldShowFields } from '../canvas/semanticZoom'
import type { Field } from '../../model/types'
import classes from './NodeShell.module.css'

export type NodeVariant =
  | 'event'
  | 'command'
  | 'query'
  | 'integration'
  | 'uiPlaceholder'

interface NodeShellProps {
  variant: NodeVariant
  label: string
  selected?: boolean
  onRename?: (name: string) => void
  /** Leading glyph (e.g. the integration gear). */
  icon?: ReactNode
  /** Schema fields — revealed (and editable) when zoomed in past the threshold. */
  fields?: readonly Field[]
  onFieldsChange?: (fields: Field[]) => void
}

/**
 * The shared card chrome for the five domain node types. Single source of the
 * node palette, selection ring, dimensions, and the mandatory 4-side
 * source+target handle set (see NodeHandles — React Flow drops edges whose
 * handle id is absent, so every node MUST carry the full set). Per-variant
 * color lives in NodeShell.module.css via theme tokens; nothing is styled
 * in-place here.
 */
export function NodeShell({
  variant,
  label,
  selected,
  onRename,
  icon,
  fields,
  onFieldsChange,
}: NodeShellProps) {
  // Re-renders only when zoom crosses the threshold (selector returns a bool),
  // not on every pan — see semanticZoom.ts.
  const detailed = useStore((s) => shouldShowFields(s.transform[2]))
  const showFields = detailed && !!onFieldsChange

  return (
    <div
      className={classes.node}
      data-variant={variant}
      data-selected={selected ? 'true' : undefined}
    >
      <NodeHandles />
      {icon && <span className={classes.icon}>{icon}</span>}
      {onRename ? <EditableLabel label={label} onRename={onRename} /> : label}
      {showFields && (
        <div className={classes.details}>
          <FieldsEditor fields={fields ?? []} onChange={onFieldsChange} />
        </div>
      )}
    </div>
  )
}
