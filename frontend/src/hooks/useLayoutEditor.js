import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useStore } from '../store'
import { confirmAction } from '../services/dialogs'
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
import {
  nearestOpenEnd,
  openEnds,
  pieceShape,
  placedPiece,
  placeDraggedEnd,
  snapPlacement,
  draggedPiecePlacement,
  rotatedPiecePlacement,
} from '../layout/track'
import { useTurnoutControl } from './useTurnoutControl'
import { arcCurve, smoothHandles } from '../layout/curves'

const KINDS = Object.keys(ENTITY_KINDS)

function newId(prefix) {
  return `${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`
}

// State and actions for the layout editor. Geometry logic lives in layout/
export function useLayoutEditor() {
  const [layouts, setLayouts] = useState([])
  const [library, setLibrary] = useState([])
  const [layout, setLayout] = useState(null)
  const [items, setItems] = useState([])
  const [tracks, setTracks] = useState([])
  const [pieces, setPieces] = useState([])
  // Last state loaded from or saved to the server, used to work out what changed
  const [saved, setSaved] = useState({ items: new Map(), tracks: '[]', pieces: '[]', size: null })
  const [importReport, setImportReport] = useState(null)
  const [tool, setTool] = useState('select')
  // The piece being dragged from the list onto the layout, or null
  const [pieceToPlace, setPieceToPlace] = useState(null)
  // When on, a piece is placed where it is dropped, without joining to open ends
  const [freePlace, setFreePlace] = useState(false)
  const [selectedKey, setSelectedKey] = useState(null)
  const [cursor, setCursor] = useState(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [message, setMessage] = useState('')
  const newCount = useRef(0)

  const libraryById = useMemo(() => Object.fromEntries(library.map((piece) => [piece.id, piece])), [library])

  // A layout uses one track system. It is set by the first piece placed
  const trackSystem = pieces.length > 0 ? libraryById[pieces[0].piece_id]?.system ?? null : null

  const sizeChanged = Boolean(
    layout && saved.size && (layout.width !== saved.size.width || layout.height !== saved.size.height)
  )

  const dirty = useMemo(() => {
    if (JSON.stringify(tracks) !== saved.tracks) return true
    if (JSON.stringify(pieces) !== saved.pieces) return true
    if (sizeChanged) return true
    return diffItems(saved.items, items).length > 0
  }, [items, tracks, pieces, saved, sizeChanged])

  const selectedItem = items.find((item) => item.key === selectedKey) || null
  const selectedTrack = selectedKey?.startsWith('track:')
    ? tracks.find((track) => `track:${track.id}` === selectedKey) || null
    : null
  const selectedPlaced = selectedKey?.startsWith('piece:')
    ? pieces.find((piece) => `piece:${piece.id}` === selectedKey) || null
    : null

  // Where a piece would go if it were dropped at this point: joined to a nearby open end, or where it is dropped.
  // An error when it cannot go there at all
  const placementAt = useCallback(
    (id, point) => {
      const piece = libraryById[id]
      if (!piece || !pieceShape(piece)) return { error: 'This piece is not placeable yet' }
      if (trackSystem && piece.system !== trackSystem) return { error: `This layout uses ${trackSystem}. Pick a piece from that system` }
      // Join to a nearby open end, unless placing without connecting
      const joined = freePlace || pieces.length === 0 ? null : snapPlacement(piece, pieces, libraryById, point)
      return { piece, placement: joined ?? { ...snapPoint(point), rotation: 0, flip: false } }
    },
    [pieces, libraryById, trackSystem, freePlace]
  )

  // The ghost is what the dragged piece would become if dropped under the pointer. Null when nothing valid is shown
  const ghost = useMemo(() => {
    if (!pieceToPlace || !cursor) return null
    const result = placementAt(pieceToPlace, cursor)
    return result.error ? null : result
  }, [pieceToPlace, cursor, placementAt])

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
      const loadedPieces = found.elements?.track_pieces ?? []

      setLayout(found)
      setItems(loadedItems)
      setTracks(loadedTracks)
      setPieces(loadedPieces)
      setSaved({
        items: snapshotOf(loadedItems),
        tracks: JSON.stringify(loadedTracks),
        pieces: JSON.stringify(loadedPieces),
        size: { width: found.width, height: found.height },
      })
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
    layoutApi
      .getTrackLibrary()
      .then(setLibrary)
      .catch((err) => setError(`Could not load the track library: ${err.message}`))
    refreshLayouts().then((list) => {
      if (list.length > 0) openLayout(list[0].id)
    })
  }, [refreshLayouts, openLayout])

  const switchLayout = useCallback(
    async (id) => {
      if (dirty && !(await confirmAction('Discard unsaved changes?', { okLabel: 'Discard', danger: true }))) return
      setMessage('')
      setImportReport(null)
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

  // Give the open layout a new name
  const renameLayout = useCallback(
    async (name) => {
      if (!layout) return
      setBusy(true)
      setError('')
      setMessage('')
      try {
        const updated = await layoutApi.updateLayout(layout.id, { name })
        await refreshLayouts()
        setLayout((current) => (current && current.id === updated.id ? { ...current, name: updated.name } : current))
        useStore.getState().setLayout({ ...layout, name: updated.name })
        setMessage(`Renamed layout to "${updated.name}"`)
      } catch (err) {
        setError(err.message)
      } finally {
        setBusy(false)
      }
    },
    [layout, refreshLayouts]
  )

  // Delete the open layout. Afterwards the first remaining layout opens, or the editor is left empty
  const deleteLayout = useCallback(async () => {
    if (!layout) return
    const confirmed = await confirmAction(`Delete layout "${layout.name}"? Its track and items are removed. This cannot be undone.`, {
      okLabel: 'Delete',
      danger: true,
    })
    if (!confirmed) return
    setBusy(true)
    setError('')
    setMessage('')
    try {
      await layoutApi.deleteLayout(layout.id)
      const list = await refreshLayouts()
      setMessage(`Deleted layout "${layout.name}"`)
      if (list.length > 0) {
        await openLayout(list[0].id)
      } else {
        setLayout(null)
        setItems([])
        setTracks([])
        setPieces([])
        setSaved({ items: new Map(), tracks: '[]', pieces: '[]', size: null })
        setSelectedKey(null)
        setPendingStart(null)
        useStore.getState().setLayout(null)
      }
    } catch (err) {
      setError(err.message)
    } finally {
      setBusy(false)
    }
  }, [layout, openLayout, refreshLayouts])

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

  // Click on the canvas. Positions are snapped to the grid for tracks and elements
  const handleCanvasClick = useCallback(
    (rawPoint) => {
      if (tool === 'select') {
        setSelectedKey(null)
        return
      }
      addItem(tool, snapPoint(rawPoint))
    },
    [tool, addItem]
  )

  // A piece is dragged from the list. Dragging over the layout shows where it would go, and dropping places it
  const startPieceDrag = useCallback((id) => {
    setPieceToPlace(id)
    setCursor(null)
    setError('')
  }, [])

  const endPieceDrag = useCallback(() => {
    setPieceToPlace(null)
    setCursor(null)
  }, [])

  const dropPiece = useCallback(
    (rawPoint) => {
      if (!pieceToPlace) return
      const result = placementAt(pieceToPlace, rawPoint)
      setPieceToPlace(null)
      setCursor(null)
      if (result.error) return setError(result.error)
      setError('')
      setPieces((prev) => [...prev, { id: newId('p'), piece_id: result.piece.id, ...result.placement }])
    },
    [pieceToPlace, placementAt]
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
    } else if (selectedKey.startsWith('piece:')) {
      const id = selectedKey.slice('piece:'.length)
      setPieces((prev) => prev.filter((piece) => piece.id !== id))
    } else {
      setItems((prev) => prev.filter((item) => item.key !== selectedKey))
    }
    setSelectedKey(null)
  }, [selectedKey])

  const chooseTool = useCallback((next) => {
    setTool(next)
    setPieceToPlace(null)
    setCursor(null)
  }, [])


  // Import converts an AnyRail export into track pieces and a layout size. Save keeps them
  const importAnyRail = useCallback(
    async (xml, innerRadiusMm) => {
      if (!layout) return
      if (pieces.length > 0 && !(await confirmAction(
        `Replace the ${pieces.length} track pieces in this layout with the imported ones? Save to keep the change.`,
        { okLabel: 'Replace' }
      ))) return
      setBusy(true)
      setError('')
      setMessage('')
      try {
        const report = await layoutApi.importAnyRail({ xml, inner_radius_mm: innerRadiusMm })
        setPieces(report.track_pieces)
        setLayout((prev) => ({ ...prev, width: report.width, height: report.height }))
        setSelectedKey(null)
        setPendingStart(null)
        setImportReport(report)
        setMessage(`Imported ${report.track_pieces.length} pieces. Save to keep them.`)
      } catch (err) {
        setError(`Import failed: ${err.message}`)
      } finally {
        setBusy(false)
      }
    },
    [layout, pieces.length]
  )

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
      if (JSON.stringify(tracks) !== saved.tracks || JSON.stringify(pieces) !== saved.pieces || sizeChanged) {
        await layoutApi.updateLayout(layout.id, {
          ...(sizeChanged ? { width: layout.width, height: layout.height } : {}),
          elements: { ...layout.elements, tracks, track_pieces: pieces, track_system: trackSystem },
        })
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
  }, [layout, items, tracks, pieces, trackSystem, saved, sizeChanged, openLayout])

  // DCC turnouts: their outputs are read from the Z21 every two seconds while operating
  const { turnoutStates, throwPlaced } = useTurnoutControl(pieces, tool === 'operate', setError)

  // Flex track is curved by two handles. Making it curved puts them a third and two thirds along the track
  const makeCurved = useCallback(
    (id) => {
      setPieces((prev) =>
        prev.map((piece) => {
          if (piece.id !== id) return piece
          const length = piece.length ?? libraryById[piece.piece_id]?.length ?? 0
          return { ...piece, handles: [{ x: length / 3, y: 0 }, { x: (2 * length) / 3, y: 0 }] }
        })
      )
    },
    [libraryById]
  )

  const straighten = useCallback((id) => {
    setPieces((prev) =>
      prev.map((piece) => {
        if (piece.id !== id) return piece
        const { handles: _removed, ...rest } = piece
        return rest
      })
    )
  }, [])

  // Drag a whole placed piece by (dx, dy). It joins another piece's open end if one is near
  const dragPiece = useCallback(
    (id, dx, dy) => {
      setPieces((prev) => {
        const placed = prev.find((piece) => piece.id === id)
        if (!placed) return prev
        const update = draggedPiecePlacement(placed, libraryById, prev.filter((piece) => piece.id !== id), dx, dy)
        return prev.map((piece) => (piece.id === id ? { ...piece, ...update } : piece))
      })
    },
    [libraryById]
  )

  // Turn a placed piece about its centre, in degrees. It joins an open end it lands near
  const rotatePiece = useCallback(
    (id, degrees) => {
      setPieces((prev) => {
        const placed = prev.find((piece) => piece.id === id)
        if (!placed) return prev
        const update = rotatedPiecePlacement(placed, libraryById, prev.filter((piece) => piece.id !== id), degrees)
        return prev.map((piece) => (piece.id === id ? { ...piece, ...update } : piece))
      })
    },
    [libraryById]
  )

  // Drag an open end of a placed piece. It snaps to a nearby open end of another piece, if there is one
  const dragPlacedEnd = useCallback(
    (id, index, cursor) => {
      setPieces((prev) => {
        const placed = prev.find((piece) => piece.id === id)
        if (!placed) return prev
        const base = libraryById[placed.piece_id]
        const shape = pieceShape(placedPiece(placed, libraryById))
        if (!base || !shape) return prev
        const target = nearestOpenEnd(openEnds(prev.filter((piece) => piece.id !== id), libraryById), cursor)
        const point = target ? { x: target.x, y: target.y } : cursor
        const update = placeDraggedEnd(placed, base, shape, index, point, target)
        return prev.map((piece) => (piece.id === id ? { ...piece, ...update } : piece))
      })
    },
    [libraryById]
  )

  // Cut a flex track to a new length, in mm. Its ends move, so pieces joined to it may need dragging
  const setPlacedLength = useCallback((id, length) => {
    setPieces((prev) => prev.map((piece) => (piece.id === id ? { ...piece, length } : piece)))
  }, [])

  // Curve a flex track to a radius and an angle. Its ends become the chord of that arc, and the handles follow the arc
  const setCurveFromArc = useCallback(
    (id, { radius, angle, side }) => {
      const piece = pieces.find((item) => item.id === id)
      const maxLength = piece ? libraryById[piece.piece_id]?.length : null
      let curve
      try {
        curve = arcCurve(radius, angle, side)
      } catch (err) {
        return setError(err.message)
      }
      if (curve.length < 10) return setError(`That curve has ends only ${curve.length} mm apart. A curve needs them at least 10 mm apart`)
      if (maxLength && curve.length > maxLength) {
        return setError(`That curve has ends ${curve.length} mm apart, more than this piece's ${maxLength} mm. Use a smaller radius or angle`)
      }
      setError('')
      setPieces((prev) => prev.map((piece) => (piece.id === id ? { ...piece, length: curve.length, handles: curve.handles } : piece)))
    },
    [pieces, libraryById]
  )

  // Smooth a curved flex track: the handles that make the curve fairest, keeping both ends' angles and length
  const smoothCurve = useCallback(
    (id) => {
      setPieces((prev) =>
        prev.map((piece) => {
          if (piece.id !== id || !Array.isArray(piece.handles)) return piece
          const shape = pieceShape(placedPiece(piece, libraryById))
          const length = piece.length ?? libraryById[piece.piece_id]?.length
          if (!shape || !length) return piece
          const handles = smoothHandles(shape, length).map((point) => ({
            x: Math.round(point.x * 100) / 100,
            y: Math.round(point.y * 100) / 100,
          }))
          return { ...piece, handles }
        })
      )
    },
    [libraryById]
  )

  // Move one handle. The handle is in the piece's own frame, in mm
  const moveHandle = useCallback((id, index, point) => {
    setPieces((prev) =>
      prev.map((piece) => {
        if (piece.id !== id || !Array.isArray(piece.handles)) return piece
        return { ...piece, handles: piece.handles.map((existing, i) => (i === index ? point : existing)) }
      })
    )
  }, [])

  // Set or clear a placed turnout's DCC address and straight output. Save keeps the change
  const setDcc = useCallback((id, fields) => {
    setPieces((prev) => prev.map((piece) => (piece.id === id ? { ...piece, ...fields } : piece)))
  }, [])

  return {
    layouts,
    library,
    libraryById,
    layout,
    items,
    tracks,
    pieces,
    trackSystem,
    tool,
    pieceToPlace,
    ghost,
    selectedKey,
    selectedItem,
    selectedTrack,
    selectedPlaced,
    cursor,
    dirty,
    busy,
    error,
    message,
    setSelectedKey,
    setCursor,
    chooseTool,
    handleCanvasClick,
    startPieceDrag,
    endPieceDrag,
    dropPiece,
    moveItem,
    renameSelected,
    deleteSelected,
    importReport,
    turnoutStates,
    throwPlaced,
    setDcc,
    makeCurved,
    straighten,
    moveHandle,
    dragPlacedEnd,
    dragPiece,
    rotatePiece,
    setPlacedLength,
    smoothCurve,
    setCurveFromArc,
    freePlace,
    setFreePlace,
    switchLayout,
    createLayout,
    renameLayout,
    deleteLayout,
    importAnyRail,
    save,
  }
}
