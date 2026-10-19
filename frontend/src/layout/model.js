// Pure layout editor logic: no React, no DOM. Tested with `node --test`.

export const GRID_SIZE = 20
export const HIT_RADIUS = 8

export const ENTITY_KINDS = {
  block: { label: 'Block', path: '/api/blocks' },
  signal: { label: 'Signal', path: '/api/signals' },
  feedback: { label: 'Feedback', path: '/api/feedbacks' },
}

export const SIGNAL_TYPES = ['main', 'warning']

export function snap(value) {
  return Math.round(value / GRID_SIZE) * GRID_SIZE
}

export function snapPoint(point) {
  return { x: snap(point.x), y: snap(point.y) }
}

export function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y)
}

// Shortest distance from point p to the segment a-b
export function distanceToSegment(p, a, b) {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const lengthSquared = dx * dx + dy * dy
  if (lengthSquared === 0) return distance(p, a)
  const t = Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / lengthSquared))
  return distance(p, { x: a.x + t * dx, y: a.y + t * dy })
}

// Next free feedback number on the layout
export function nextFeedbackNumber(items) {
  const numbers = items
    .filter((item) => item.kind === 'feedback')
    .map((item) => item.feedback_number || 0)
  return Math.max(0, ...numbers) + 1
}

export function nextName(kind, items) {
  const count = items.filter((item) => item.kind === kind).length
  return `${ENTITY_KINDS[kind].label} ${count + 1}`
}

// Returns the top-most item within HIT_RADIUS of point, or null
export function hitItem(items, point) {
  for (let i = items.length - 1; i >= 0; i -= 1) {
    if (distance(items[i], point) <= HIT_RADIUS * 2) return items[i]
  }
  return null
}

// Returns the track whose line is within HIT_RADIUS of point, or null
export function hitTrack(tracks, point) {
  for (let i = tracks.length - 1; i >= 0; i -= 1) {
    const track = tracks[i]
    const segmentDistance = distanceToSegment(
      point,
      { x: track.x1, y: track.y1 },
      { x: track.x2, y: track.y2 }
    )
    if (segmentDistance <= HIT_RADIUS) return track
  }
  return null
}

export function signalColor(aspect) {
  if (aspect === 'go') return '#22c55e'
  if (aspect === 'caution') return '#eab308'
  return '#ef4444'
}

// Convert a row from the API into an editor item
export function fromServer(kind, row) {
  return { ...row, key: `${kind}:${row.id}`, kind, id: row.id }
}

// Editor item for an element created locally, not yet saved
export function newItem(kind, index, fields) {
  return { ...fields, key: `new:${kind}:${index}`, kind, id: null }
}

export function createBody(item, layoutId) {
  const body = { layout_id: layoutId, name: item.name, x: item.x, y: item.y }
  if (item.kind === 'signal') body.signal_type = item.signal_type || 'main'
  if (item.kind === 'feedback') body.feedback_number = item.feedback_number
  return body
}

export function snapshotOf(items) {
  return new Map(
    items
      .filter((item) => item.id !== null)
      .map((item) => [item.key, { name: item.name, x: item.x, y: item.y }])
  )
}

// Work out the API calls needed to bring the server in line with the editor
export function diffItems(saved, items) {
  const ops = []
  const currentKeys = new Set(items.map((item) => item.key))

  for (const item of items) {
    if (item.id === null) {
      ops.push({ op: 'create', kind: item.kind, key: item.key, body: createBody(item, item.layoutId) })
      continue
    }
    const before = saved.get(item.key)
    if (
      before &&
      (before.name !== item.name || before.x !== item.x || before.y !== item.y)
    ) {
      ops.push({
        op: 'update',
        kind: item.kind,
        id: item.id,
        body: { name: item.name, x: item.x, y: item.y },
      })
    }
  }

  for (const [key] of saved) {
    if (!currentKeys.has(key)) {
      const [kind, id] = key.split(':')
      ops.push({ op: 'delete', kind, id: Number(id) })
    }
  }
  return ops
}
