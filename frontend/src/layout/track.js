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

export const JOIN_TOLERANCE = 0.5
export const SNAP_DISTANCE = 40
const CURVE_STEP_DEGREES = 2

const rad = (deg) => (deg * Math.PI) / 180
const deg = (radians) => (radians * 180) / Math.PI

// A piece is placeable when the library flags it so. The flag is set only where the geometry
// needed to draw and join the piece is present (see backend/app/data/track_library)
export function isPlaceable(piece) {
  return Boolean(piece) && piece.placeable === true
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

  // Flex track is placed as a straight of its length
  if (piece.type === 'straight' || piece.type === 'flex') {
    const length = piece.length
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: length, y: 0, heading: 0 },
      ],
      paths: [[{ x: 0, y: 0 }, { x: length, y: 0 }]],
    }
  }

  // A curve with no radius (the custom curve before a radius is set) has no shape yet
  if (piece.type === 'curve' && piece.radius > 0 && piece.angle > 0) {
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

  // Single slip: two straight diagonal tracks that cross. Ends point along their own track
  if (piece.type === 'slip' && piece.slip_geometry) {
    const g = piece.slip_geometry
    const outward = (from, to) => normalizeDegrees(deg(Math.atan2(from[1] - to[1], from[0] - to[0])))
    const ends = [
      { p: g.track_a_top, other: g.track_a_bottom },
      { p: g.track_b_top, other: g.track_b_bottom },
      { p: g.track_a_bottom, other: g.track_a_top },
      { p: g.track_b_bottom, other: g.track_b_top },
    ]
    return {
      endpoints: ends.map((e) => ({ x: e.p[0], y: e.p[1], heading: outward(e.p, e.other) })),
      paths: [
        [{ x: g.track_a_top[0], y: g.track_a_top[1] }, { x: g.track_a_bottom[0], y: g.track_a_bottom[1] }],
        [{ x: g.track_b_top[0], y: g.track_b_top[1] }, { x: g.track_b_bottom[0], y: g.track_b_bottom[1] }],
      ],
    }
  }

  // Curved double radius turnout: a straight toe, then two arcs that both end at the listed length
  if (piece.type === 'curved_turnout' && piece.curve_geometry && (piece.hand === 'right' || piece.hand === 'left')) {
    const { toe_straight: toe, length, inner_radius: rIn, outer_radius: rOut } = piece.curve_geometry
    const side = piece.hand === 'right' ? 1 : -1
    const sweepIn = deg(Math.asin((length - toe) / rIn))
    const sweepOut = deg(Math.asin((length - toe) / rOut))
    const arcIn = arcPoints(rIn, sweepIn, side).map((pt) => ({ x: pt.x + toe, y: pt.y }))
    const arcOut = arcPoints(rOut, sweepOut, side).map((pt) => ({ x: pt.x + toe, y: pt.y }))
    const endIn = arcIn[arcIn.length - 1]
    const endOut = arcOut[arcOut.length - 1]
    const toePath = [{ x: 0, y: 0 }, { x: toe, y: 0 }]
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: endIn.x, y: endIn.y, heading: side * sweepIn },
        { x: endOut.x, y: endOut.y, heading: side * sweepOut },
      ],
      paths: [[...toePath, ...arcIn.slice(1)], [...toePath, ...arcOut.slice(1)]],
    }
  }

  // Crossing: two straight tracks through the centre. Each sits half the crossing angle from the axis
  if (piece.type === 'crossing' && piece.angle) {
    const half = piece.length / 2
    const lateral = half * Math.tan(rad(piece.angle / 2))
    const a = { top: [-half, -lateral], bottom: [half, lateral] }
    const b = { top: [-half, lateral], bottom: [half, -lateral] }
    const outward = (from, to) => normalizeDegrees(deg(Math.atan2(from[1] - to[1], from[0] - to[0])))
    const ends = [
      { p: a.top, other: a.bottom },
      { p: b.top, other: b.bottom },
      { p: a.bottom, other: a.top },
      { p: b.bottom, other: b.top },
    ]
    return {
      endpoints: ends.map((e) => ({ x: e.p[0], y: e.p[1], heading: outward(e.p, e.other) })),
      paths: [
        [{ x: a.top[0], y: a.top[1] }, { x: a.bottom[0], y: a.bottom[1] }],
        [{ x: b.top[0], y: b.top[1] }, { x: b.bottom[0], y: b.bottom[1] }],
      ],
    }
  }

  // Asymmetric 3-way (provisional): straight route, plus a right and a left branch, each an arc from its departure point
  if (piece.type === 'three_way' && piece.three_way_geometry) {
    const g = piece.three_way_geometry
    const L = g.length
    const branch = (b, side) => {
      // A measured branch gives its sweep. Otherwise the branch ends on the straight's length
      const sweep = b.sweep ?? deg(Math.asin((L - b.depart) / b.radius))
      const arc = arcPoints(b.radius, sweep, side).map((pt) => ({ x: pt.x + b.depart, y: pt.y }))
      return { sweep, arc, end: arc[arc.length - 1] }
    }
    const right = branch(g.right, 1)
    const left = branch(g.left, -1)
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: L, y: 0, heading: 0 },
        { x: right.end.x, y: right.end.y, heading: right.sweep },
        { x: left.end.x, y: left.end.y, heading: -left.sweep },
      ],
      paths: [
        [{ x: 0, y: 0 }, { x: L, y: 0 }],
        [{ x: 0, y: 0 }, { x: g.right.depart, y: 0 }, ...right.arc.slice(1)],
        [{ x: 0, y: 0 }, { x: g.left.depart, y: 0 }, ...left.arc.slice(1)],
      ],
    }
  }

  // Turnout measured from a plan sheet: straight toe, then one diverging arc ending at the length
  if (piece.type === 'turnout' && piece.diverge_geometry && (piece.hand === 'right' || piece.hand === 'left')) {
    const { toe_straight: toe, radius, length } = piece.diverge_geometry
    const side = piece.hand === 'right' ? 1 : -1
    const sweep = deg(Math.asin((length - toe) / radius))
    const arc = arcPoints(radius, sweep, side).map((pt) => ({ x: pt.x + toe, y: pt.y }))
    const end = arc[arc.length - 1]
    return {
      endpoints: [
        { x: 0, y: 0, heading: 180 },
        { x: length, y: 0, heading: 0 },
        { x: end.x, y: end.y, heading: side * sweep },
      ],
      paths: [
        [{ x: 0, y: 0 }, { x: length, y: 0 }],
        [{ x: 0, y: 0 }, { x: toe, y: 0 }, ...arc.slice(1)],
      ],
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

// The output that sets a turnout's diverging route. Output 0 is straight unless the layout says otherwise
export function divergingOutput(placed) {
  return 1 - (placed.dcc_output_straight ?? 0)
}

// The output to switch a turnout to, so that a click toggles it. An unknown state throws it straight
export function turnoutTarget(placed, state) {
  const straight = placed.dcc_output_straight ?? 0
  return state === straight ? divergingOutput(placed) : straight
}

// Per-placement dimensions that override the library piece (custom lengths and radii)
const PLACEMENT_DIMENSIONS = ['length', 'radius', 'angle']

// The library piece with this placement's own length, radius, or angle applied. Null if the piece is unknown
export function placedPiece(placed, library) {
  const base = library[placed.piece_id]
  if (!base) return null
  const overrides = {}
  for (const key of PLACEMENT_DIMENSIONS) {
    if (placed[key] !== undefined) overrides[key] = placed[key]
  }
  return { ...base, ...overrides }
}

// Open ends are endpoints of placed pieces that are not joined to another piece
export function openEnds(placedPieces, library) {
  const all = []
  for (const placed of placedPieces) {
    const shape = pieceShape(placedPiece(placed, library))
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
    const shape = pieceShape(placedPiece(placed, library))
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
