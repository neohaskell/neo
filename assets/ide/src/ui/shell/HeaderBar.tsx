import { Group, Text, Button, Tooltip, Kbd } from '@mantine/core'
import { spotlight } from '@mantine/spotlight'
import { IconSparkles, IconWand } from '@tabler/icons-react'
import classes from './HeaderBar.module.css'

interface HeaderBarProps {
  projectName?: string | null
  /** Active feature name for the breadcrumb (or null in flat mode). */
  featureName?: string | null
  onNew: () => void
  onOpen: () => void
  onRelayout: () => void
  onHeal: () => void
  healing: boolean
  relayouting: boolean
  /** Tidy/Heal mutate the model; disabled when a non-model lens is showing
   *  (no visual feedback there). */
  modelActive: boolean
  collaboration?: { role: 'host' | 'join'; peerCount: number } | null
  readOnly?: boolean
}

/**
 * Top bar: breadcrumb on the left, model actions on the right. Replaces the old
 * FileMenu — New/Open/Tidy live here (and in the ⌘K palette); Heal is the one
 * accented action.
 */
export function HeaderBar({
  projectName,
  featureName,
  onNew,
  onOpen,
  onRelayout,
  onHeal,
  healing,
  relayouting,
  modelActive,
  collaboration,
  readOnly = false,
}: HeaderBarProps) {
  return (
    <header className={classes.header} data-testid="header-bar">
      <Group gap={6} wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
        <Text size="sm" fw={600} truncate>{projectName ?? 'neo'}</Text>
        {featureName && (
          <>
            <Text size="sm" c="dimmed">/</Text>
            <Text size="sm" c="dimmed" truncate>{featureName}</Text>
          </>
        )}
      </Group>

      {collaboration && (
        <div className={classes.collaboration} data-testid="collaboration-status">
          <span className={classes.statusDot} />
          <span>{collaboration.role === 'host' ? 'HOSTING' : 'JOINED'} · {collaboration.peerCount} PEER{collaboration.peerCount === 1 ? '' : 'S'} · MOVE SYNC</span>
        </div>
      )}

      <Group gap={6} wrap="nowrap">
        <Tooltip label={<Group gap={6}>Command palette <Kbd>⌘K</Kbd></Group>}>
          <Button variant="subtle" color="gray" size="xs" onClick={() => spotlight.open()}>⌘K</Button>
        </Tooltip>
        <Button variant="subtle" color="gray" size="xs" onClick={onNew} disabled={readOnly}>New</Button>
        <Button variant="subtle" color="gray" size="xs" onClick={onOpen} disabled={readOnly}>Open</Button>
        <Button
          variant="light"
          size="xs"
          color="emQuery"
          leftSection={<IconWand size={14} />}
          onClick={onRelayout}
          disabled={readOnly || relayouting || healing || !modelActive}
          title="Order slices and chapters left-to-right by the event-modeling wave, and clean up positions — no AI, no structural changes."
        >
          {relayouting ? 'Tidying…' : 'Tidy by flow'}
        </Button>
        <Button
          size="xs"
          color="emFeature"
          leftSection={<IconSparkles size={14} />}
          onClick={onHeal}
          disabled={readOnly || healing || relayouting || !modelActive}
          title="Ask Claude to improve the model (fix layout, add inferred edges, etc.)"
        >
          {healing ? 'Healing…' : 'Heal with AI'}
        </Button>
      </Group>
    </header>
  )
}
