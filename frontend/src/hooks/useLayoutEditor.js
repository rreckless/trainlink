import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from '../store'
import { layoutApi } from '../services/layoutApi'
import {
  ENTITY_KINDS,
  diffItems,
  fromServer,
  newItem,
  nextFeedbackNumber,
  nextName,
  snapPoint,
  snapshotOf,
} from '../layout/model'

const KINDS = Object.keys(ENTITY_KINDS)

function newTrackId() {
  return `t-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`
}

// State and actions for the layout editor. Geometry logic lives in layout/model.js
export function useLayoutEditor() {
  const [layouts, setLayouts] = useState([])
  const [layout, setLayout] = useState(null)
  const [items, setItems] = useState([])
  const [tracks, setTracks] = useState([])
  // Last state loaded from or saved to the server, used to work out what changed
  const [saved, setSaved] = useState({ items: new Map(), tracks: '[]' })
  const [tool, setTool] = useState('select')
  const [selectedKey, setSelectedKey] = useState(null)
  const [pendingStart, setPendingStart] = useState(null)
  const [cursor, setCursor] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const newCount = useRef(0)

  const dirty = useMemo(() => {
    if (JSON.stringify(tracks) !== saved.tracks) return true
    return diffItems(saved.items, items).length > 0
  }, [items, tracks, saved])

  const selectedItem = items.find((item) => item.key === selectedKey) || null
  const selectedTrack = selectedKey?.startsWith('track:')
    ? tracks.find((track) => `track:${track.id}` === selectedKey) || null
    : null

  const refreshLayouts = useCallback(async () => {
    try {
      const list = await layoutApi.listLayouts()
      setLayouts(list)
      return list
    } catch (err) {
      setError(err.message)
      return []
    }
  }, [])

  const openLayout = useCallback(async (id) => {
    setBusy(true)
    setError('')
    try {
      const [found, ...lists] = await Promise.all([
        layoutApi.getLayout(id),
        ...KINDS.map((kind) => layoutApi.listEntities(kind, id)),
      ])
      const loadedItems = KINDS.flatMap((kind, index) =>
        lists[index].map((row) => ({ ...fromServer(kind, row), layoutId: id }))
      )
      const loadedTracks = found.elements?.tracks ?? []

      setLayout(found)
      setItems(loadedItems)
      setTracks(loadedTracks)
      setSaved({ items: snapshotOf(loadedItems), tracks: JSON.stringify(loadedTracks) })
      setSelectedKey(null)
      setPendingStart(null)
      useStore.getState().setLayout(found)
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }, [])

  useEffect(() => {
    refreshLayouts().then((list) => {
      if (list.length > 0) openLayout(list[0].id)
    })
  }, [refreshLayouts, openLayout])

  const switchLayout = useCallback(
    (id) => {
      if (dirty && !window.confirm('Discard unsaved changes?')) return
      setMessage('')
      openLayout(id)
    },
    [dirty, openLayout]
  )

  const createLayout = useCallback(
    async (form) => {
      setBusy(true)
      setError('')
      try {
        const created = await layoutApi.createLayout(form)
        await refreshLayouts()
        setMessage(`Created layout "${created.name}"`)
        await openLayout(created.id)
      } catch (err) {
        setError(err.message)
      } finally {
        setBusy(false)
      }
    },
    [openLayout, refreshLayouts]
  )

  const addTrack = useCallback((start, end) => {
    if (start.x === end.x && start.y === end.y) return
    setTracks((prev) => [...prev, { id: newTrackId(), x1: start.x, y1: start.y, x2: end.x, y2: end.y }])
  }, [])

  const addItem = useCallback(
    (kind, point) => {
      if (!layout) return
      const index = ++newCount.current
      setItems((prev) => {
        const fields = {
          name: nextName(kind, prev),
          x: point.x,
          y: point.y,
          layoutId: layout.id,
        }
        if (kind === 'block') fields.state = 'free'
        if (kind === 'signal') {
          fields.signal_type = 'main'
          fields.aspect = 'stop'
        }
        if (kind === 'feedback') {
          fields.feedback_number = nextFeedbackNumber(prev)
          fields.state = false
        }
        return [...prev, newItem(kind, index, fields)]
      })
    },
    [layout]
  )

  // Click on the canvas, already snapped to the grid by the caller
  const handleCanvasClick = useCallback(
    (rawPoint) => {
      const point = snapPoint(rawPoint)
      if (tool === 'select') {
        setSelectedKey(null)
        return
      }
      if (tool === 'track') {
        // Each track starts where the last one ended. Escape stops the chain
        if (pendingStart) addTrack(pendingStart, point)
        setPendingStart(point)
        return
      }
      addItem(tool, point)
    },
    [tool, pendingStart, addTrack, addItem]
  )

  const moveItem = useCallback((key, rawPoint) => {
    const point = snapPoint(rawPoint)
    setItems((prev) => prev.map((item) => (item.key === key ? { ...item, x: point.x, y: point.y } : item)))
  }, [])

  const renameSelected = useCallback(
    (name) => {
      setItems((prev) => prev.map((item) => (item.key === selectedKey ? { ...item, name } : item)))
    },
    [selectedKey]
  )

  const deleteSelected = useCallback(() => {
    if (!selectedKey) return
    if (selectedKey.startsWith('track:')) {
      const id = selectedKey.slice('track:'.length)
      setTracks((prev) => prev.filter((track) => track.id !== id))
    } else {
      setItems((prev) => prev.filter((item) => item.key !== selectedKey))
    }
    setSelectedKey(null)
  }, [selectedKey])

  const cancelTrack = useCallback(() => setPendingStart(null), [])

  const chooseTool = useCallback((next) => {
    setTool(next)
    setPendingStart(null)
    setCursor(null)
  }, [])

  const save = useCallback(async () => {
    if (!layout) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      const ops = diffItems(saved.items, items)
      for (const op of ops) {
        if (op.op === 'create') await layoutApi.createEntity(op.kind, op.body)
        else if (op.op === 'update') await layoutApi.updateEntity(op.kind, op.id, op.body)
        else await layoutApi.deleteEntity(op.kind, op.id)
      }
      if (JSON.stringify(tracks) !== saved.tracks) {
        await layoutApi.updateLayout(layout.id, { elements: { ...layout.elements, tracks } })
      }
      await openLayout(layout.id)
      setMessage('Layout saved')
    } catch (err) {
      // Reload from the server so a retry cannot repeat changes that already went through
      await openLayout(layout.id)
      setError(`Not all changes were saved (${err.message}). The layout was reloaded from the server.`)
    } finally {
      setBusy(false)
    }
  }, [layout, items, tracks, saved, openLayout])

  return {
    layouts,
    layout,
    items,
    tracks,
    tool,
    selectedKey,
    selectedItem,
    selectedTrack,
    pendingStart,
    cursor,
    dirty,
    busy,
    error,
    message,
    setSelectedKey,
    setCursor,
    chooseTool,
    handleCanvasClick,
    moveItem,
    renameSelected,
    deleteSelected,
    cancelTrack,
    switchLayout,
    createLayout,
    save,
  }
}
