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
import { curvedShape } from './curves.js'

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

  // Flex track with two handles is curved
  if (piece.type === 'flex' && Array.isArray(piece.handles) && piece.handles.length === 2) {
    return curvedShape(piece.length, piece.handles)
  }

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

// Point in a placed piece's own frame for a point on the layout. The inverse of toWorld
export function toLocal(point, placement) {
  const offset = rotatePoint({ x: point.x - placement.x, y: point.y - placement.y }, -placement.rotation)
  return placement.flip ? { x: offset.x, y: -offset.y } : offset
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

// Shortest flex track that can be cut to
export const MIN_FLEX_MM = 10

// The open end closest to a point, if it is within the snap distance
export function nearestOpenEnd(ends, point) {
  let nearest = null
  let nearestDistance = SNAP_DISTANCE
  for (const end of ends) {
    const d = distance(point, end)
    if (d <= nearestDistance) {
      nearest = end
      nearestDistance = d
    }
  }
  return nearest
}

// The placement for a piece whose open end `index` is dragged to `point`. A straight flex track stretches from
// its other end, so its length follows the drag. Any other piece moves so the end lands on `target` with its heading,
// or moves with the cursor when there is no target. `base` is the library piece
export function placeDraggedEnd(placed, base, shape, index, point, target) {
  const ends = worldEndpoints(shape, placed)
  if (base.type === 'flex' && !Array.isArray(placed.handles)) {
    const far = ends[1 - index]
    const from = index === 0 ? point : far
    const to = index === 0 ? far : point
    const dx = to.x - from.x
    const dy = to.y - from.y
    const length = Math.min(base.length, Math.max(MIN_FLEX_MM, Math.hypot(dx, dy)))
    return {
      x: from.x,
      y: from.y,
      rotation: normalizeDegrees(deg(Math.atan2(dy, dx))),
      length: Math.round(length * 10) / 10,
    }
  }
  if (target) return alignToEnd(shape, index, target, placed.flip)
  const current = ends[index]
  return {
    x: placed.x + (point.x - current.x),
    y: placed.y + (point.y - current.y),
    rotation: placed.rotation,
    flip: placed.flip,
  }
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
  if (placed.handles !== undefined) overrides.handles = placed.handles
  return { ...base, ...overrides }
}

// Open ends are endpoints of placed pieces that are not joined to another piece
// Every end of every piece, each marked open or joined. An end is joined when another piece's end is on it
export function pieceEnds(placedPieces, library) {
  const all = []
  for (const placed of placedPieces) {
    const shape = pieceShape(placedPiece(placed, library))
    if (!shape) continue
    worldEndpoints(shape, placed).forEach((endpoint, index) => {
      all.push({ ...endpoint, pieceId: placed.id, index })
    })
  }
  return all.map((end) => ({
    ...end,
    open: !all.some((other) => other.pieceId !== end.pieceId && distance(other, end) <= JOIN_TOLERANCE),
  }))
}

export function openEnds(placedPieces, library) {
  return pieceEnds(placedPieces, library).filter((end) => end.open)
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

// How far apart two open ends can be, and the piece still fit between them, for both ends to join
const TWO_END_TOLERANCE_MM = 1

// Placement that puts piece ends `i` and `k` on the open ends `from` and `to`, or null if the piece is the wrong length
function fitTwoEnds(piece, shape, i, k, from, to, flip) {
  const mirror = (end) => (flip ? { x: end.x, y: -end.y } : { x: end.x, y: end.y })
  const a = mirror(shape.endpoints[i])
  const b = mirror(shape.endpoints[k])
  const localLength = Math.hypot(b.x - a.x, b.y - a.y)
  const worldLength = Math.hypot(to.x - from.x, to.y - from.y)
  if (Math.abs(localLength - worldLength) > TWO_END_TOLERANCE_MM) return null
  const rotation = normalizeDegrees(deg(Math.atan2(to.y - from.y, to.x - from.x)) - deg(Math.atan2(b.y - a.y, b.x - a.x)))
  // Centre the two ends between their targets, so the error is shared equally between them
  const centreLocal = rotatePoint({ x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 }, rotation)
  // Each end should point back into its open end, so its heading is the open end's heading plus 180
  const headingOf = (endpoint) => normalizeDegrees((flip ? -endpoint.heading : endpoint.heading) + rotation)
  const off = (actual, wanted) => Math.abs(((actual - wanted + 540) % 360) - 180)
  const error = off(headingOf(shape.endpoints[i]), from.heading + 180) + off(headingOf(shape.endpoints[k]), to.heading + 180)
  return {
    x: (from.x + to.x) / 2 - centreLocal.x,
    y: (from.y + to.y) / 2 - centreLocal.y,
    rotation,
    flip,
    error,
  }
}

// A placed piece with its ends near open ends of other pieces joins them: both ends if the piece fits exactly
// between two open ends, otherwise the one end nearest. Returns the placement unchanged if no end is near enough
export function snapToOpenEnds(placed, library, others) {
  const piece = placedPiece(placed, library)
  const shape = piece ? pieceShape(piece) : null
  if (!shape) return placed

  const open = openEnds(others, library)
  const near = worldEndpoints(shape, placed)
    .map((end, index) => ({ index, end, target: nearestOpenEnd(open, end) }))
    .filter((pair) => pair.target)
  let best = null
  for (let p = 0; p < near.length; p += 1) {
    for (let q = p + 1; q < near.length; q += 1) {
      const from = near[p].target
      const to = near[q].target
      if (from.x === to.x && from.y === to.y) continue
      for (const flip of flipChoices(piece)) {
        const fit = fitTwoEnds(piece, shape, near[p].index, near[q].index, from, to, flip)
        if (fit && (!best || fit.error < best.error)) best = fit
      }
    }
  }
  if (best) {
    const { error: _error, ...placement } = best
    return { ...placed, ...placement }
  }

  for (const { end } of near) {
    const joined = snapPlacement(piece, others, library, end)
    if (joined) return { ...placed, ...joined }
  }
  return placed
}

// A placed piece moved by (dx, dy), then joined to any open end it lands near
export function draggedPiecePlacement(placed, library, others, dx, dy) {
  return snapToOpenEnds({ ...placed, x: placed.x + dx, y: placed.y + dy }, library, others)
}

// A placed piece turned by `degrees` about its centre, then joined to any open end it lands near
export function rotatedPiecePlacement(placed, library, others, degrees) {
  const shape = pieceShape(placedPiece(placed, library))
  if (!shape) return placed
  const centre = (placement) => {
    const ends = worldEndpoints(shape, placement)
    return { x: (ends[0].x + ends[ends.length - 1].x) / 2, y: (ends[0].y + ends[ends.length - 1].y) / 2 }
  }
  const before = centre(placed)
  const turned = { ...placed, rotation: normalizeDegrees(placed.rotation + degrees) }
  const after = centre(turned)
  // Move the origin so the centre stays where it was
  turned.x += before.x - after.x
  turned.y += before.y - after.y
  return snapToOpenEnds(turned, library, others)
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
