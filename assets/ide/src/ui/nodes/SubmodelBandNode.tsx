import { ActionIcon } from '@mantine/core'
import { IconX } from '@tabler/icons-react'
import { EditableLabel } from './EditableLabel'
import classes from './SubmodelBandNode.module.css'

interface Props {
  data: {
    label: string
    onRename?: (name: string) => void
    onDelete?: () => void
  }
}

// A submodel band is a translucent full-bleed rectangle drawn BEHIND the graph
// (lowest z-index) that visually contains a feature's chapters/slices. Its body
// is pointer-transparent so nodes inside stay interactive; only the top-left
// chip captures clicks (rename/delete).
export function SubmodelBandNodeComponent({ data }: Props) {
  return (
    <div className={classes.band}>
      <div className={classes.chip}>
        <span className={classes.label}>
          {data.onRename ? (
            <EditableLabel label={data.label} onRename={data.onRename} />
          ) : (
            data.label
          )}
        </span>
        {data.onDelete && (
          <ActionIcon
            size="xs"
            color="red"
            variant="subtle"
            onClick={data.onDelete}
            title="Remove submodel (keeps its chapters)"
            aria-label="Remove submodel"
          >
            <IconX size={12} />
          </ActionIcon>
        )}
      </div>
    </div>
  )
}
