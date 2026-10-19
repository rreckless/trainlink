import { Stage, Layer, Group, Rect, Line, Circle, Text } from 'react-konva'
import { GRID_SIZE, hitItem, hitTrack, signalColor, snapPoint } from '../../layout/model'

const BLOCK_WIDTH = 60
const BLOCK_HEIGHT = 24
const FEEDBACK_SIZE = 12

function pointerOf(event) {
  return event.target.getStage().getPointerPosition()
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
// Positions are passed to the parent already snapped to the grid.
export default function LayoutCanvas({
  layout,
  tracks,
  items,
  tool,
  selectedKey,
  pendingStart,
  cursor,
  onCanvasClick,
  onCursor,
  onSelect,
  onMove,
}) {
  const editing = tool === 'select'

  const handleMove = (event) => {
    const point = snapPoint(pointerOf(event))
    onCursor(point)
  }

  const handleStageClick = (event) => {
    const raw = pointerOf(event)
    if (!raw) return
    const point = snapPoint(raw)
    if (editing) {
      // Clicks on a track or element select it. Clicks on empty space clear the selection
      const track = hitTrack(tracks, raw)
      if (track) return onSelect(`track:${track.id}`)
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

  return (
    <Stage
      width={layout.width}
      height={layout.height}
      onMouseMove={handleMove}
      onClick={handleStageClick}
      onTap={handleStageClick}
      style={{ cursor: editing ? 'default' : 'crosshair' }}
    >
      <Layer listening={false}>
        <Rect width={layout.width} height={layout.height} fill="#0f172a" />
        <Grid width={layout.width} height={layout.height} />
      </Layer>

      <Layer>
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
  )
}
