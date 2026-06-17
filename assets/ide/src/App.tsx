import { useReducer, useCallback, useState, useEffect, useRef } from 'react'
import { ReactFlowProvider } from '@xyflow/react'
import { reducer, ModelContext } from './state/store'
import { Canvas } from './ui/Canvas'
import { Toolbar } from './ui/Toolbar'
import { FileMenu } from './ui/FileMenu'
import { Toast } from './ui/Toast'
import { InvalidModelModal } from './ui/InvalidModelModal'
import { HealingOverlay } from './ui/HealingOverlay'
import { newModel, jsonToModel, modelToJson } from './io/fileOps'
import { saveToStorage, loadFromStorage } from './io/persistence'
import { getEdgeTypeForConnection } from './ui/connectionRules'
import { computeNodeAlignments } from './ui/layout/grid'
import { autoLayoutMissingPositions } from './ui/layout/autoLayout'
import type { EdgeType } from './model/types'
import { IdeClient, type ConnectionState, type RpcResult } from './ipc/client'
import { initialize, type InitializeResult } from './ipc/initialize'
import {
  readEventModel,
  writeEventModel,
  healEventModel,
  type ReadEventModelResult,
  type ValidationError,
} from './ipc/eventModel'
import { StatusBar } from './ipc/StatusBar'

const CLIENT_INFO = { name: 'neoide-frontend', version: '0.0.1' }

function getInitialModel() {
  return loadFromStorage() ?? newModel()
}

function App() {
  const [model, dispatch] = useReducer(reducer, null, getInitialModel)
  const [dirty, setDirty] = useState(false)
  const [toastMessage, setToastMessage] = useState<string | null>(null)
  const [flashingSliceId, setFlashingSliceId] = useState<string | null>(null)
  const [flashingEntityId, setFlashingEntityId] = useState<string | null>(null)
  const modelRef = useRef(model)
  modelRef.current = model

  // Auto-save to localStorage on every model change
  useEffect(() => {
    saveToStorage(model)
  }, [model])

  // Open the JSON-RPC connection to the embedded `neo` server and call
  // `initialize` once. The StatusBar at the bottom reflects connection state
  // and shows the server version + workspace it connected to. When served
  // by `vite dev` (no Rust server at the same origin), the WS open fails and
  // the StatusBar shows the disconnect — by design.
  //
  // `clientRef` is hoisted so handleSave / handleOpen can reach the same
  // connection. The effect closes the client on unmount.
  const [conn, setConn] = useState<ConnectionState>({ status: 'connecting' })
  const [init, setInit] = useState<InitializeResult | null>(null)
  const [pendingInvalid, setPendingInvalid] = useState<{
    errors: ValidationError[]
    preamble?: string
  } | null>(null)
  const [healing, setHealing] = useState(false)
  const clientRef = useRef<IdeClient | null>(null)

  const applyReadResult = useCallback(
    (readRes: RpcResult<ReadEventModelResult>) => {
      if (!readRes.ok) {
        setToastMessage(`Failed to read event-model.json: ${readRes.error.message}`)
        return
      }
      const { content, validation } = readRes.result
      switch (validation.status) {
        case 'notFound':
          // Fresh project — keep whatever the reducer started with
          // (localStorage or the empty `newModel()`).
          return
        case 'valid':
          if (content !== null) {
            const parsed = jsonToModel(content)
            // Fill in any missing node positions so the canvas renders a
            // sensible default layout instead of stacking everything at the
            // origin (common after AI healing, which often omits positions).
            // Idempotent — a no-op when every node already has a real position.
            const loaded = autoLayoutMissingPositions(parsed)
            dispatch({ type: 'loadModel', model: loaded })
            setDirty(loaded !== parsed)
          }
          return
        case 'invalid':
          setPendingInvalid({ errors: validation.errors })
          return
        case 'malformedJson':
          setPendingInvalid({
            errors: [
              {
                pointer: '',
                message: `File is not valid JSON: ${validation.parseError}`,
                kind: 'schema',
              },
            ],
            preamble:
              'event-model.json on disk is not valid JSON. An AI agent can attempt to repair it, or you can cancel and keep your local copy.',
          })
          return
      }
    },
    [],
  )

  useEffect(() => {
    const client = new IdeClient()
    clientRef.current = client
    const unsubscribe = client.onState(setConn)
    ;(async () => {
      const initRes = await initialize(client, CLIENT_INFO)
      if (!initRes.ok) {
        console.error('neo initialize failed', initRes.error)
        return
      }
      setInit(initRes.result)
      const readRes = await readEventModel(client)
      applyReadResult(readRes)
    })()
    return () => {
      unsubscribe()
      client.close()
      clientRef.current = null
    }
  }, [applyReadResult])

  const handleHealAccept = useCallback(async () => {
    const client = clientRef.current
    if (!client) {
      setToastMessage('not connected to neo — cannot heal')
      setPendingInvalid(null)
      return
    }
    setHealing(true)
    const healRes = await healEventModel(client)
    if (!healRes.ok) {
      setToastMessage(`Healing failed: ${healRes.error.message}`)
      setHealing(false)
      setPendingInvalid(null)
      return
    }
    if (healRes.result.outcome.status === 'stillInvalid') {
      setPendingInvalid({
        errors: healRes.result.outcome.errors,
        preamble:
          'Healing ran but the file is still invalid. Click Heal to try again, or Cancel to keep your local copy.',
      })
      setHealing(false)
      return
    }
    // Healed — reload from disk.
    const reload = await readEventModel(client)
    setHealing(false)
    setPendingInvalid(null)
    applyReadResult(reload)
  }, [applyReadResult])

  const handleHealCancel = useCallback(() => {
    setPendingInvalid(null)
  }, [])

  // Manual "Heal with AI" button on the FileMenu. Always invokes claude
  // (mode='improve'), even when validation is passing — that's how the
  // user asks the agent to refine layout / add inferred edges.
  const handleManualHeal = useCallback(async () => {
    const client = clientRef.current
    if (!client) {
      setToastMessage('not connected to neo — cannot heal')
      return
    }
    if (dirty) {
      const confirmed = window.confirm(
        'You have unsaved changes. The AI will edit the file on disk; your unsaved work will be discarded on reload. Continue?',
      )
      if (!confirmed) return
    }
    setHealing(true)
    const healRes = await healEventModel(client, 'improve')
    if (!healRes.ok) {
      setToastMessage(`Healing failed: ${healRes.error.message}`)
      setHealing(false)
      return
    }
    if (healRes.result.outcome.status === 'stillInvalid') {
      setPendingInvalid({
        errors: healRes.result.outcome.errors,
        preamble:
          'The AI improved the file but it now has validation errors. Cancel to keep the agent’s changes on disk, or Heal to ask the agent to fix them.',
      })
      setHealing(false)
      return
    }
    // Reload from disk.
    const reload = await readEventModel(client)
    setHealing(false)
    applyReadResult(reload)
  }, [applyReadResult, dirty])

  const markDirty = useCallback(() => setDirty(true), [])

  const handleAddEvent = useCallback(() => {
    dispatch({ type: 'addEvent', name: 'New Event' })
    markDirty()
  }, [markDirty])

  const handleAddCommand = useCallback(() => {
    dispatch({ type: 'addCommand', name: 'New Command' })
    markDirty()
  }, [markDirty])

  const handleAddQuery = useCallback(() => {
    dispatch({ type: 'addQuery', name: 'New Query' })
    markDirty()
  }, [markDirty])

  const handleAddIntegration = useCallback(() => {
    dispatch({ type: 'addIntegration', name: 'New Integration', kind: 'outbound' })
    markDirty()
  }, [markDirty])

  const handleAddUIPlaceholder = useCallback(() => {
    dispatch({ type: 'addUIPlaceholder', name: 'New UI' })
    markDirty()
  }, [markDirty])

  const handleAddEntity = useCallback(() => {
    dispatch({ type: 'addEntity', name: 'New Entity' })
    markDirty()
  }, [markDirty])

  const handleRemoveEntity = useCallback(
    (entityId: string) => {
      const hasEvents = modelRef.current.nodes.some(
        (n) => n.type === 'event' && n.entityId === entityId,
      )
      if (hasEvents) {
        setToastMessage('You can only delete entities without events')
        setFlashingEntityId(entityId)
        setTimeout(() => setFlashingEntityId(null), 600)
        return
      }
      dispatch({ type: 'removeEntity', entityId })
      markDirty()
    },
    [markDirty],
  )

  const handleAddSlice = useCallback(() => {
    dispatch({ type: 'addSlice', name: 'New Slice' })
    markDirty()
  }, [markDirty])

  const handleRemoveSlice = useCallback(
    (sliceId: string) => {
      const hasNodes = modelRef.current.nodes.some((n) => n.sliceId === sliceId)
      if (hasNodes) {
        setToastMessage('You can only delete empty slices')
        setFlashingSliceId(sliceId)
        setTimeout(() => setFlashingSliceId(null), 600)
        return
      }
      dispatch({ type: 'removeSlice', sliceId })
      markDirty()
    },
    [markDirty],
  )

  const handleAddChapter = useCallback(() => {
    dispatch({ type: 'addChapter', name: 'New Chapter' })
    markDirty()
  }, [markDirty])

  const handleRemoveChapter = useCallback(
    (chapterId: string) => {
      dispatch({ type: 'removeChapter', chapterId })
      markDirty()
    },
    [markDirty],
  )

  const handleRenameChapter = useCallback(
    (chapterId: string, name: string) => {
      dispatch({ type: 'renameChapter', chapterId, name })
      markDirty()
    },
    [markDirty],
  )


  const handleChapterSliceRange = useCallback(
    (chapterId: string, startSliceId: string, endSliceId: string) => {
      dispatch({ type: 'setChapterSliceRange', chapterId, startSliceId, endSliceId })
      markDirty()
    },
    [markDirty],
  )

  const handlePositionChange = useCallback(
    (nodeId: string, x: number, y: number) => {
      dispatch({ type: 'updatePosition', nodeId, x, y })
      markDirty()
    },
    [markDirty],
  )

  const handleConnect = useCallback(
    (sourceId: string, targetId: string, sourceHandle: string | null, targetHandle: string | null) => {
      const sourceNode = modelRef.current.nodes.find((n) => n.id === sourceId)
      const targetNode = modelRef.current.nodes.find((n) => n.id === targetId)
      if (!sourceNode || !targetNode) return

      const edgeType = getEdgeTypeForConnection(sourceNode.type, targetNode.type)
      if (!edgeType) return

      dispatch({
        type: 'addEdge',
        edgeType: edgeType as EdgeType,
        sourceId,
        targetId,
        sourceHandle,
        targetHandle,
      })
      markDirty()
    },
    [markDirty],
  )

  const handleNodesDelete = useCallback(
    (nodeIds: string[]) => {
      for (const id of nodeIds) {
        dispatch({ type: 'removeNode', nodeId: id })
      }
      markDirty()
    },
    [markDirty],
  )

  const handleEdgesDelete = useCallback(
    (edgeIds: string[]) => {
      for (const id of edgeIds) {
        dispatch({ type: 'removeEdge', edgeId: id })
      }
      markDirty()
    },
    [markDirty],
  )

  const handleNodeRename = useCallback(
    (nodeId: string, name: string) => {
      dispatch({ type: 'updateNodeName', nodeId, name })
      markDirty()
    },
    [markDirty],
  )

  const handleEntityRename = useCallback(
    (entityId: string, name: string) => {
      dispatch({ type: 'renameEntity', entityId, name })
      markDirty()
    },
    [markDirty],
  )

  const handleAssignNodeToSlice = useCallback(
    (nodeId: string, sliceId: string | null, x: number, y: number) => {
      // Chain: position update → slice assignment → alignment, all computed forward
      const afterPosition = reducer(modelRef.current, { type: 'updatePosition', nodeId, x, y })
      const afterSlice = reducer(afterPosition, { type: 'assignNodeToSlice', nodeId, sliceId })
      const adjustments = computeNodeAlignments(afterSlice)

      dispatch({ type: 'updatePosition', nodeId, x, y })
      dispatch({ type: 'assignNodeToSlice', nodeId, sliceId })
      if (adjustments.length > 0) {
        dispatch({ type: 'batchUpdatePositions', changes: adjustments })
      }
      markDirty()
    },
    [markDirty],
  )

  const handleAssignNodeToEntity = useCallback(
    (nodeId: string, entityId: string | null) => {
      dispatch({ type: 'assignNodeToEntity', nodeId, entityId })
      markDirty()
    },
    [markDirty],
  )

  const handleSliceRename = useCallback(
    (sliceId: string, name: string) => {
      dispatch({ type: 'renameSlice', sliceId, name })
      markDirty()
    },
    [markDirty],
  )

  const handleNew = useCallback(() => {
    if (dirty) {
      const confirmed = window.confirm(
        'You have unsaved changes. Are you sure you want to create a new model? All changes will be lost.',
      )
      if (!confirmed) return
    }
    dispatch({ type: 'loadModel', model: newModel() })
    setDirty(false)
  }, [dirty])

  const handleOpen = useCallback(async () => {
    const client = clientRef.current
    if (!client) {
      setToastMessage('not connected to neo — cannot Open')
      return
    }
    if (dirty) {
      const confirmed = window.confirm(
        'You have unsaved changes. Re-opening event-model.json from disk will discard them. Continue?',
      )
      if (!confirmed) return
    }
    const res = await readEventModel(client)
    if (res.ok && res.result.validation.status === 'notFound') {
      setToastMessage('event-model.json does not exist in the workspace yet')
      return
    }
    applyReadResult(res)
  }, [dirty, applyReadResult])

  const handleSave = useCallback(async () => {
    const client = clientRef.current
    if (!client) {
      setToastMessage('not connected to neo — cannot Save')
      return
    }
    const content = modelToJson(model)
    const res = await writeEventModel(client, content)
    if (res.ok) {
      setDirty(false)
      setToastMessage(`saved to ${res.result.path}`)
    } else {
      setToastMessage(`Save failed: ${res.error.message}`)
    }
  }, [model])

  return (
    <ModelContext.Provider value={{ model, dispatch }}>
      <ReactFlowProvider>
        <div className="flex flex-col w-full h-full">
          <FileMenu
            onNew={handleNew}
            onOpen={handleOpen}
            onSave={handleSave}
            onHeal={handleManualHeal}
            dirty={dirty}
            healing={healing}
          />
          <Toolbar
            onAddEvent={handleAddEvent}
            onAddCommand={handleAddCommand}
            onAddQuery={handleAddQuery}
            onAddIntegration={handleAddIntegration}
            onAddUIPlaceholder={handleAddUIPlaceholder}
            onAddEntity={handleAddEntity}
            onAddSlice={handleAddSlice}
            onAddChapter={handleAddChapter}
          />
          <div className="flex flex-1 min-h-0">
            <div className="flex-1">
              <Canvas
                model={model}
                onPositionChange={handlePositionChange}
                onConnect={handleConnect}
                onNodesDelete={handleNodesDelete}
                onEdgesDelete={handleEdgesDelete}
                onNodeRename={handleNodeRename}
                onEntityRename={handleEntityRename}
                onSliceRename={handleSliceRename}
                onAssignNodeToSlice={handleAssignNodeToSlice}
                onAssignNodeToEntity={handleAssignNodeToEntity}
                onSliceDelete={handleRemoveSlice}
                onEntityDelete={handleRemoveEntity}
                onChapterRename={handleRenameChapter}
                onChapterSliceRange={handleChapterSliceRange}
                onChapterDelete={handleRemoveChapter}
                flashingSliceId={flashingSliceId}
                flashingEntityId={flashingEntityId}
              />
            </div>
          </div>
          <StatusBar state={conn} init={init} />
        </div>
      </ReactFlowProvider>
      <Toast message={toastMessage} onDismiss={() => setToastMessage(null)} />
      {pendingInvalid && !healing && (
        <InvalidModelModal
          errors={pendingInvalid.errors}
          preamble={pendingInvalid.preamble}
          onHeal={handleHealAccept}
          onCancel={handleHealCancel}
        />
      )}
      {healing && <HealingOverlay />}
    </ModelContext.Provider>
  )
}

export default App
