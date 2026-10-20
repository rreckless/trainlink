import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { Stage, Layer, Group, Rect, Line, Circle, Text } from 'react-konva'
import { GRID_SIZE, hitItem, hitTrack, signalColor, snapPoint } from '../../layout/model'
import {
  divergingOutput,
  hitPlacedPiece,
  openEnds,
  pieceEnds,
  pieceShape,
  placedPiece,
  toLocal,
  toWorld,
  worldPaths,
} from '../../layout/track'

const BLOCK_WIDTH = 60
const BLOCK_HEIGHT = 24
const FEEDBACK_SIZE = 12
const ZOOM_MIN = 0.25
const ZOOM_MAX = 4
const ZOOM_STEP = 1.25

// Pointer position in layout units, which accounts for the zoom
function pointerOf(event) {
  return event.target.getStage().getRelativePointerPosition()
}

function flat(path) {
  return path.flatMap((point) => [point.x, point.y])
}

function Grid({ width, height }) {
  const lines = []
  for (let x = 0; x <= width; x += GRID_SIZE) {
    lines.push(<Line key={`v${x}`} points={[x, 0, x, height]} stroke="#1e293b" strokeWidth={1} listening={false} />)
  }
  for (let y = 0; y <= height; y += GRID_SIZE) {
    lines.push(<Line key={`h${y}`} points={[0, y, width, y]} stroke="#1e293b" strokeWidth={1} listening={false} />)
  }
  return lines
}

// Draws the layout and turns pointer events into editor actions.
// Element positions are passed to the parent already snapped to the grid.
export default function LayoutCanvas({
  layout,
  tracks,
  pieces,
  libraryById,
  ghost,
  items,
  tool,
  selectedKey,
  pendingStart,
  cursor,
  operating,
  turnoutStates,
  onThrow,
  onHandle,
  onEndDrag,
  onPieceDrag,
  onCanvasClick,
  onCursor,
  onSelect,
  onMove,
}) {
  const editing = tool === 'select'

  // True when a turnout's diverging route is the one the Z21 last reported
  const isDiverging = (placed) =>
    placed.dcc_address !== undefined && turnoutStates[String(placed.dcc_address)] === divergingOutput(placed)
  const [zoom, setZoom] = useState(1)
  const scroller = useRef(null)
  const zoomRef = useRef(1)
  // Layout point and screen offset to keep in place after the next zoom, so zooming stays around the pointer
  const anchor = useRef(null)
  // Drag to scroll: where a press on empty canvas started, and whether it has moved far enough to be a drag
  const stageRef = useRef(null)
  const pan = useRef(null)
  const panned = useRef(false)

  useLayoutEffect(() => {
    const target = anchor.current
    const element = scroller.current
    if (!target || !element) return
    element.scrollLeft = target.x * zoom - target.offsetX
    element.scrollTop = target.y * zoom - target.offsetY
    anchor.current = null
  }, [zoom])

  // Zoom to `next`, keeping the layout point `at` under the screen offset (offsetX, offsetY)
  const applyZoom = (next, at) => {
    const clamped = Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, next))
    if (clamped === zoomRef.current) return
    anchor.current = at
    zoomRef.current = clamped
    setZoom(clamped)
  }

  // Zoom by a factor, about the given screen offset in the scroll area
  const zoomAbout = (factor, offsetX, offsetY) => {
    const element = scroller.current
    if (!element) return
    const current = zoomRef.current
    applyZoom(current * factor, {
      x: (element.scrollLeft + offsetX) / current,
      y: (element.scrollTop + offsetY) / current,
      offsetX,
      offsetY,
    })
  }

  // Zoom by a factor about the middle of what is visible
  const zoomBy = (factor) => {
    const element = scroller.current
    if (element) zoomAbout(factor, element.clientWidth / 2, element.clientHeight / 2)
  }

  const fit = () => {
    const element = scroller.current
    if (!element) return
    const next = Math.min(element.clientWidth / layout.width, element.clientHeight / layout.height)
    applyZoom(next, { x: 0, y: 0, offsetX: 0, offsetY: 0 })
  }

  // Ctrl or Cmd with the wheel zooms, which is also how a trackpad pinch arrives. Plain wheel scrolls
  useEffect(() => {
    const element = scroller.current
    if (!element) return undefined
    const onWheel = (event) => {
      if (!event.ctrlKey && !event.metaKey) return
      event.preventDefault()
      const rect = element.getBoundingClientRect()
      zoomAbout(event.deltaY < 0 ? ZOOM_STEP : 1 / ZOOM_STEP, event.clientX - rect.left, event.clientY - rect.top)
    }
    element.addEventListener('wheel', onWheel, { passive: false })
    return () => element.removeEventListener('wheel', onWheel)
    // zoomAbout only reads refs, so it does not need to be a dependency
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const handleMove = (event) => {
    onCursor(pointerOf(event))
  }

  // A press on empty canvas starts a pan. A press on a piece, item, or handle does not
  const startPan = (event) => {
    // Every press starts with no drag, so a click after an earlier pan still counts
    panned.current = false
    const stage = stageRef.current
    const element = scroller.current
    if (!stage || !element || event.button !== 0) return
    const point = stage.getPointerPosition()
    if (!point || stage.getIntersection(point)) return
    pan.current = { x: event.clientX, y: event.clientY, left: element.scrollLeft, top: element.scrollTop }
  }

  useEffect(() => {
    const onMove = (event) => {
      const start = pan.current
      const element = scroller.current
      if (!start || !element) return
      const dx = event.clientX - start.x
      const dy = event.clientY - start.y
      if (!panned.current && Math.hypot(dx, dy) < 4) return
      // Set before the click that follows a release, so that click is ignored
      panned.current = true
      element.scrollLeft = start.left - dx
      element.scrollTop = start.top - dy
    }
    const onUp = () => {
      pan.current = null
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
    return () => {
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
  }, [])

  const handleStageClick = (event) => {
    // The click that ends a drag is not a click
    if (panned.current) return
    const raw = pointerOf(event)
    if (!raw) return
    if (operating) {
      const placed = hitPlacedPiece(pieces, libraryById, raw)
      if (placed && placed.dcc_address !== undefined) onThrow(placed)
      return
    }
    if (editing) {
      // Track, then track piece, then element. Empty space clears the selection
      const track = hitTrack(tracks, raw)
      if (track) return onSelect(`track:${track.id}`)
      const placed = hitPlacedPiece(pieces, libraryById, raw)
      if (placed) return onSelect(`piece:${placed.id}`)
      const item = hitItem(items, raw)
      if (item) return onSelect(item.key)
      return onSelect(null)
    }
    onCanvasClick(raw)
  }

  const handleDragEnd = (key, event) => {
    const point = snapPoint({ x: event.target.x(), y: event.target.y() })
    onMove(key, point)
    return point
  }

  const ghostPaths = ghost ? worldPaths(pieceShape(ghost.piece), ghost.placement) : []

  // The selected piece. Its open ends can be dragged, and a curved piece's two handles too
  const selectedPiece = selectedKey?.startsWith('piece:')
    ? pieces.find((piece) => `piece:${piece.id}` === selectedKey) ?? null
    : null
  const handlePiece = selectedPiece && Array.isArray(selectedPiece.handles) ? selectedPiece : null
  const selectedEnds = selectedPiece && editing ? openEnds(pieces, libraryById).filter((end) => end.pieceId === selectedPiece.id) : []
  // Every piece end, shown at all times: orange where it is open, green where it is joined
  const allEnds = pieceEnds(pieces, libraryById)
  // Where a dragged handle is, in the piece's own frame, rounded to half a millimetre
  const localPoint = (event, placed) => {
    const local = toLocal({ x: event.target.x(), y: event.target.y() }, placed)
    return { x: Math.round(local.x * 2) / 2, y: Math.round(local.y * 2) / 2 }
  }

  return (
    <div>
      <div className="flex items-center gap-2 mb-2 text-sm">
        <button onClick={() => zoomBy(1 / ZOOM_STEP)} className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600" aria-label="Zoom out">
          −
        </button>
        <span className="w-14 text-center text-slate-300">{Math.round(zoom * 100)}%</span>
        <button onClick={() => zoomBy(ZOOM_STEP)} className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600" aria-label="Zoom in">
          +
        </button>
        <button onClick={fit} className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600">
          Fit
        </button>
        <button
          onClick={() => applyZoom(1, { x: 0, y: 0, offsetX: 0, offsetY: 0 })}
          className="px-3 py-1 bg-slate-700 rounded hover:bg-slate-600"
        >
          100%
        </button>
      </div>

      <div ref={scroller} onMouseDown={startPan} className="overflow-auto" style={{ maxHeight: 'calc(100vh - 16rem)' }}>
        <Stage
          ref={stageRef}
          width={layout.width * zoom}
          height={layout.height * zoom}
          scaleX={zoom}
          scaleY={zoom}
          onMouseMove={handleMove}
          onClick={handleStageClick}
          onTap={handleStageClick}
          style={{ cursor: operating ? 'pointer' : editing ? 'default' : 'crosshair' }}
        >
      <Layer listening={false}>
        <Rect width={layout.width} height={layout.height} fill="#0f172a" />
        <Grid width={layout.width} height={layout.height} />
      </Layer>

      <Layer>
        {pieces.map((placed) => {
          const shape = pieceShape(placedPiece(placed, libraryById))
          if (!shape) return null
          const selected = selectedKey === `piece:${placed.id}`
          // Turnout paths are [straight, diverging]. The diverging route is orange when it is set.
          // The whole piece drags. On drop it moves by the drag and joins an open end it lands on
          return (
            <Group
              key={placed.id}
              draggable={editing}
              onDragEnd={(event) => {
                const dx = event.target.x()
                const dy = event.target.y()
                event.target.position({ x: 0, y: 0 })
                onPieceDrag(placed.id, dx, dy)
              }}
            >
              {worldPaths(shape, placed).map((path, index) => (
                <Line
                  key={`${placed.id}-${index}`}
                  points={flat(path)}
                  stroke={selected ? '#38bdf8' : index === 1 && isDiverging(placed) ? '#f97316' : '#e2e8f0'}
                  strokeWidth={selected ? 5 : 3}
                  hitStrokeWidth={12}
                  listening={editing}
                />
              ))}
            </Group>
          )
        })}

        {tracks.map((track) => {
          const key = `track:${track.id}`
          const selected = key === selectedKey
          return (
            <Line
              key={track.id}
              points={[track.x1, track.y1, track.x2, track.y2]}
              stroke={selected ? '#38bdf8' : '#cbd5e1'}
              strokeWidth={selected ? 5 : 4}
              hitStrokeWidth={12}
              listening={editing}
            />
          )
        })}

        {selectedEnds.map((end) => (
          <Circle
            key={`end-${end.index}`}
            x={end.x}
            y={end.y}
            radius={6}
            fill="#f97316"
            stroke="#0f172a"
            strokeWidth={1}
            draggable
            onDragMove={(event) => onEndDrag(selectedPiece.id, end.index, { x: event.target.x(), y: event.target.y() })}
          />
        ))}

        {handlePiece &&
          handlePiece.handles.map((point, index) => {
            const world = toWorld(point, handlePiece)
            return (
              <Circle
                key={`handle-${index}`}
                x={world.x}
                y={world.y}
                radius={6}
                fill="#38bdf8"
                stroke="#0f172a"
                strokeWidth={1}
                draggable={editing}
                listening={editing}
                onDragMove={(event) => onHandle(handlePiece.id, index, localPoint(event, handlePiece))}
              />
            )
          })}

        {ghostPaths.map((path, index) => (
          <Line
            key={`ghost-${index}`}
            points={flat(path)}
            stroke="#38bdf8"
            strokeWidth={3}
            dash={[6, 4]}
            opacity={0.7}
            listening={false}
          />
        ))}

        {allEnds.map((end) => (
          <Circle
            key={`${end.pieceId}-${end.index}`}
            x={end.x}
            y={end.y}
            radius={3}
            fill={end.open ? '#f97316' : '#22c55e'}
            listening={false}
          />
        ))}

        {pendingStart && cursor && (
          <Line
            points={[pendingStart.x, pendingStart.y, cursor.x, cursor.y]}
            stroke="#38bdf8"
            strokeWidth={3}
            dash={[6, 6]}
            listening={false}
          />
        )}
      </Layer>

      <Layer>
        {items.map((item) => {
          const selected = item.key === selectedKey
          const listening = editing
          const draggable = editing
          const label = (
            <Text
              text={item.name}
              fontSize={11}
              fill="#e2e8f0"
              listening={false}
              width={120}
              align="center"
              x={item.x - 60}
              y={item.y + (item.kind === 'block' ? 14 : 12)}
            />
          )

          if (item.kind === 'block') {
            return (
              <Group key={item.key}>
                <Rect
                  x={item.x - BLOCK_WIDTH / 2}
                  y={item.y - BLOCK_HEIGHT / 2}
                  width={BLOCK_WIDTH}
                  height={BLOCK_HEIGHT}
                  fill={item.state === 'occupied' ? '#b45309' : '#1e40af'}
                  stroke={selected ? '#38bdf8' : '#334155'}
                  strokeWidth={selected ? 3 : 1}
                  cornerRadius={3}
                  listening={listening}
                  draggable={draggable}
                  onDragEnd={(event) => {
                    const point = handleDragEnd(item.key, event)
                    event.target.position({ x: point.x - BLOCK_WIDTH / 2, y: point.y - BLOCK_HEIGHT / 2 })
                  }}
                />
                {label}
              </Group>
            )
          }

          if (item.kind === 'signal') {
            return (
              <Group key={item.key}>
                <Circle
                  x={item.x}
                  y={item.y}
                  radius={8}
                  fill={signalColor(item.aspect)}
                  stroke={selected ? '#38bdf8' : '#0f172a'}
                  strokeWidth={selected ? 3 : 1}
                  listening={listening}
                  draggable={draggable}
                  onDragEnd={(event) => {
                    const point = handleDragEnd(item.key, event)
                    event.target.position(point)
                  }}
                />
                {label}
              </Group>
            )
          }

          return (
            <Group key={item.key}>
              <Rect
                x={item.x - FEEDBACK_SIZE / 2}
                y={item.y - FEEDBACK_SIZE / 2}
                width={FEEDBACK_SIZE}
                height={FEEDBACK_SIZE}
                fill={item.state ? '#f97316' : '#a855f7'}
                stroke={selected ? '#38bdf8' : '#0f172a'}
                strokeWidth={selected ? 3 : 1}
                listening={listening}
                draggable={draggable}
                onDragEnd={(event) => {
                  const point = handleDragEnd(item.key, event)
                  event.target.position({ x: point.x - FEEDBACK_SIZE / 2, y: point.y - FEEDBACK_SIZE / 2 })
                }}
              />
              {label}
            </Group>
          )
        })}
      </Layer>
        </Stage>
      </div>
    </div>
  )
}
