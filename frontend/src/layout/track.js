// Track piece geometry: connection points, placement, and snapping.
// Pure functions, no React. Coordinates are in the layout's units (mm for N gauge).
//
// Each piece has endpoints in its own frame. The first endpoint is the entry (toe) at the origin,
// pointing back along -x. Other endpoints point away from the piece, so a heading is the
// direction of travel leaving the piece through that end. Headings are in degrees.
//
// A placement is {x, y, rotation, flip}. Points are mirrored across the x axis first when flip
// is true, then rotated by rotation, then moved by (x, y).

import { distance, distanceToSegment } from './model.js'

export const PLACEABLE_SYSTEMS = ['Setrack (Code 80)']
export const JOIN_TOLERANCE = 0.5
export const SNAP_DISTANCE = 40
const CURVE_STEP_DEGREES = 2

const rad = (deg) => (deg * Math.PI) / 180
const deg = (radians) => (radians * 180) / Math.PI

// Pieces are placeable when their system is fully modelled, or when the piece is flagged
// placeable in the library (used for single pieces whose geometry has been worked out)
export function isPlaceable(piece) {
  if (!piece) return false
  return PLACEABLE_SYSTEMS.includes(piece.system) || piece.placeable === true
}

// Diverging exit angle for a turnout whose listed angle is the frog angle. With the exit at
// `length` on a circle of `radius`: sin(angle) = length / radius
function exitAngleFromLength(length, radius) {
  if (!(length > 0) || !(radius >= length)) return null
  return deg(Math.asin(length / radius))
}

export function normalizeDegrees(deg) {
  return ((deg % 360) + 360) % 360
}

export function rotatePoint(p, deg) {
  const r = rad(deg)
  return { x: p.x * Math.cos(r) - p.y * Math.sin(r), y: p.x * Math.sin(r) + p.y * Math.cos(r) }
}

// Points along a circular arc that starts at the origin heading along +x and turns by sweep
// degrees. side is +1 to turn toward +y, -1 toward -y
function arcPoints(radius, sweep, side) {
  const steps = Math.max(2, Math.ceil(Math.abs(sweep) / CURVE_STEP_DEGREES))
  const points = []
  for (let i = 0; i <= steps; i += 1) {
    const phi = (sweep * i) / steps
    points.push({ x: radius * Math.sin(rad(phi)), y: side * radius * (1 - Math.cos(rad(phi))) })
  }
  return points
}

// Geometry for a library piece, or null if this release can't place it.
// Returns { endpoints: [{x, y, heading}], paths: [[{x, y}, ...], ...] }
export function pieceShape(piece) {
  if (!isPlaceable(piece)) return null

  if (piece.type === 'straight') {
    const length = piece.length
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: length, y: 0, heading: 0 },
      ],
      paths: [[{ x: 0, y: 0 }, { x: length, y: 0 }]],
    }
  }

  if (piece.type === 'curve') {
    const arc = arcPoints(piece.radius, piece.angle, 1)
    const end = arc[arc.length - 1]
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: end.x, y: end.y, heading: piece.angle },
      ],
      paths: [arc],
    }
  }

  if (piece.type === 'turnout' && (piece.hand === 'right' || piece.hand === 'left')) {
    const side = piece.hand === 'right' ? 1 : -1
    const angle =
      piece.angle_meaning === 'frog'
        ? exitAngleFromLength(piece.length, piece.radius)
        : piece.angle
    if (angle === null || angle === undefined) return null
    const diverge = arcPoints(piece.radius, angle, side)
    const divergeEnd = diverge[diverge.length - 1]
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: piece.length, y: 0, heading: 0 },
        { x: divergeEnd.x, y: divergeEnd.y, heading: side * angle },
      ],
      paths: [
        [{ x: 0, y: 0 }, { x: piece.length, y: 0 }],
        diverge,
      ],
    }
  }

  return null
}

// Flip choices for a piece: curves can be mirrored, turnouts and straights cannot
export function flipChoices(piece) {
  return piece.type === 'curve' ? [false, true] : [false]
}

export function toWorld(point, placement) {
  const local = placement.flip ? { x: point.x, y: -point.y } : point
  const rotated = rotatePoint(local, placement.rotation)
  return { x: rotated.x + placement.x, y: rotated.y + placement.y }
}

export function worldEndpoints(shape, placement) {
  return shape.endpoints.map((endpoint) => ({
    x: toWorld(endpoint, placement).x,
    y: toWorld(endpoint, placement).y,
    heading: normalizeDegrees(
      (placement.flip ? -endpoint.heading : endpoint.heading) + placement.rotation
    ),
  }))
}

export function worldPaths(shape, placement) {
  return shape.paths.map((path) => path.map((point) => toWorld(point, placement)))
}

// Placement that joins endpoint index of shape to openEnd, with the mirror state given by flip
export function alignToEnd(shape, index, openEnd, flip) {
  const endpoint = shape.endpoints[index]
  const local = flip ? { x: endpoint.x, y: -endpoint.y } : { x: endpoint.x, y: endpoint.y }
  const localHeading = flip ? -endpoint.heading : endpoint.heading
  const rotation = normalizeDegrees(openEnd.heading + 180 - localHeading)
  const rotated = rotatePoint(local, rotation)
  return {
    x: openEnd.x - rotated.x,
    y: openEnd.y - rotated.y,
    rotation,
    flip,
  }
}

// Open ends are endpoints of placed pieces that are not joined to another piece
export function openEnds(placedPieces, library) {
  const all = []
  for (const placed of placedPieces) {
    const shape = pieceShape(library[placed.piece_id])
    if (!shape) continue
    worldEndpoints(shape, placed).forEach((endpoint, index) => {
      all.push({ ...endpoint, pieceId: placed.id, index })
    })
  }
  return all.filter(
    (end) =>
      !all.some(
        (other) =>
          other.pieceId !== end.pieceId && distance(other, end) <= JOIN_TOLERANCE
      )
  )
}

// Best placement for a piece joined to the open end nearest the cursor.
// Returns null if no open end is within SNAP_DISTANCE of the cursor
export function snapPlacement(piece, placedPieces, library, cursor) {
  const shape = pieceShape(piece)
  if (!shape) return null

  const ends = openEnds(placedPieces, library)
  let nearest = null
  let nearestDistance = SNAP_DISTANCE
  for (const end of ends) {
    const d = distance(cursor, end)
    if (d <= nearestDistance) {
      nearest = end
      nearestDistance = d
    }
  }
  if (!nearest) return null

  let best = null
  let bestScore = Infinity
  for (let index = 0; index < shape.endpoints.length; index += 1) {
    for (const flip of flipChoices(piece)) {
      const placement = alignToEnd(shape, index, nearest, flip)
      // Prefer the orientation whose other ends are closest to the cursor
      const others = worldEndpoints(shape, placement).filter(
        (_, i) => i !== index
      )
      const score = Math.min(...others.map((end) => distance(cursor, end)))
      if (score < bestScore) {
        bestScore = score
        best = placement
      }
    }
  }
  return best
}

// Piece nearest to the point, within tolerance. Returns the placed piece or null
export function hitPlacedPiece(placedPieces, library, point, tolerance = 8) {
  let hit = null
  let hitDistance = tolerance
  for (const placed of placedPieces) {
    const shape = pieceShape(library[placed.piece_id])
    if (!shape) continue
    for (const path of worldPaths(shape, placed)) {
      for (let i = 0; i < path.length - 1; i += 1) {
        const d = distanceToSegment(point, path[i], path[i + 1])
        if (d <= hitDistance) {
          hit = placed
          hitDistance = d
        }
      }
    }
  }
  return hit
}
