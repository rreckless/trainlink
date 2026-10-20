import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  pieceShape,
  alignToEnd,
  worldEndpoints,
  openEnds,
  snapPlacement,
  hitPlacedPiece,
  normalizeDegrees,
  placedPiece,
  divergingOutput,
  turnoutTarget,
  toWorld,
  toLocal,
  placeDraggedEnd,
  nearestOpenEnd,
  pieceEnds,
  draggedPiecePlacement,
  rotatedPiecePlacement,
  snapToOpenEnds,
  MIN_FLEX_MM,
} from './track.js'
import { smoothHandles, bendingEnergy } from './curves.js'

const SETRACK = 'Setrack (Code 80)'
const STREAMLINE = 'Streamline (Code 55)'

const straight = { id: 'st1', system: SETRACK, type: 'straight', length: 87, placeable: true }
const curve = { id: 'st3', system: SETRACK, type: 'curve', radius: 228, angle: 22.5, placeable: true }
const rightTurnout = { id: 'st5', system: SETRACK, type: 'turnout', hand: 'right', length: 87, radius: 228, angle: 22.5, placeable: true }
const leftTurnout = { id: 'st6', system: SETRACK, type: 'turnout', hand: 'left', length: 87, radius: 228, angle: 22.5, placeable: true }

const library = {
  'peco-n-st-1': { ...straight, part_number: 'ST-1' },
  'peco-n-st-3': { ...curve, part_number: 'ST-3' },
  'peco-n-st-5': { ...rightTurnout, part_number: 'ST-5' },
  'peco-n-st-6': { ...leftTurnout, part_number: 'ST-6' },
  'peco-n-sl-e388f': { id: 'x', system: STREAMLINE, type: 'turnout', hand: 'right', length: 164, radius: 914, angle: 10, part_number: 'SL-E388F' },
}

function close(actual, expected, tolerance = 0.05) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `expected ${expected} to be within ${tolerance} of ${actual}`
  )
}

function assertPoint(actual, expected) {
  close(actual.x, expected.x)
  close(actual.y, expected.y)
}

test('straight piece ends at its length, pointing back and forward', () => {
  const shape = pieceShape(straight)
  assert.deepEqual(shape.endpoints.map((e) => e.heading), [180, 0])
  assertPoint(shape.endpoints[1], { x: 87, y: 0 })
})

test('flex track is placed as a straight of its length', () => {
  const shape = pieceShape({ system: 'Code 55', type: 'flex', length: 914, placeable: true })
  assert.deepEqual(shape.endpoints.map((e) => e.heading), [180, 0])
  assertPoint(shape.endpoints[1], { x: 914, y: 0 })
})

test('curve end matches radius and angle', () => {
  const shape = pieceShape(curve)
  // R sin(22.5) and R (1 - cos(22.5)) for R = 228
  assertPoint(shape.endpoints[1], { x: 87.25, y: 17.35 })
  assert.equal(shape.endpoints[1].heading, 22.5)
})

test('right-hand turnout diverging exit matches its length for Setrack', () => {
  // The 87mm length should match where the diverging curve ends in the x direction
  const shape = pieceShape(rightTurnout)
  assert.ok(Math.abs(shape.endpoints[2].x - 87) < 0.5)
  assert.ok(shape.endpoints[2].y > 0)
  assert.equal(shape.endpoints[1].x, 87)
})

test('left-hand turnout diverges to the other side', () => {
  const shape = pieceShape(leftTurnout)
  assert.ok(shape.endpoints[2].y < 0)
  assert.equal(shape.endpoints[2].heading, -22.5)
})

test('pieces without the placeable flag, and unknown types, have no shape', () => {
  // Unflagged pieces are listed but cannot be placed, whatever their system
  assert.equal(pieceShape({ system: SETRACK, type: 'straight', length: 87 }), null)
  assert.equal(pieceShape({ system: STREAMLINE, type: 'turnout', hand: 'right' }), null)
  assert.equal(pieceShape({ system: SETRACK, type: 'crossing', placeable: true }), null)
  assert.equal(pieceShape(undefined), null)
})

// SL-U395F: listed length 137mm, radius 457mm, 10 degrees is the frog angle, not the exit angle
const slu395f = {
  id: 'slu395f',
  system: STREAMLINE,
  type: 'turnout',
  hand: 'right',
  length: 137,
  radius: 457,
  angle: 10,
  angle_meaning: 'frog',
  placeable: true,
}

test('SL-U395F diverging exit sits at its listed length on the listed radius', () => {
  const shape = pieceShape(slu395f)
  assert.ok(shape)
  // Exit at 137mm along the straight leg, and the diverging end at x = 137
  assert.ok(Math.abs(shape.endpoints[2].x - 137) < 0.5)
  // The exit angle is asin(137 / 457), about 17.46 degrees
  assert.ok(Math.abs(shape.endpoints[2].heading - 17.46) < 0.05)
  assert.ok(shape.endpoints[2].y > 0)
})

test('SL-U395F can be joined to an open end', () => {
  const shape = pieceShape(slu395f)
  const openEnd = { x: 0, y: 0, heading: 0 }
  const placement = alignToEnd(shape, 0, openEnd, false)
  assertPoint(worldEndpoints(shape, placement)[0], openEnd)
})

test('SL-U395F measured geometry: toe, diverging radius and end at the listed length', () => {
  const measured = { ...slu395f, diverge_geometry: { toe_straight: 16.4, radius: 540, length: 137 } }
  const shape = pieceShape(measured)
  const end = shape.endpoints[2]
  assert.ok(Math.abs(end.x - 137) < 0.01)
  // Diverging heading: asin(120.6 / 540), about 12.9 degrees
  assert.ok(Math.abs(end.heading - 12.9) < 0.1)
  assert.ok(end.y > 0)
})

test('SL-U396F diverges to the other side from SL-U395F', () => {
  const left = pieceShape({ ...slu395f, part_number: 'SL-U396F', hand: 'left' })
  const right = pieceShape(slu395f)
  assert.ok(left.endpoints[2].y < 0)
  assert.ok(Math.abs(left.endpoints[2].x - right.endpoints[2].x) < 1e-9)
  assert.ok(Math.abs(left.endpoints[2].heading + right.endpoints[2].heading) < 1e-9)
})

// SL-380F single slip, from the PECO plan sheet
const slip = {
  id: 'sl380f',
  system: STREAMLINE,
  type: 'slip',
  length: 154,
  angle: 10,
  placeable: true,
  slip_geometry: {
    track_a_top: [-6.97, -76.84],
    track_a_bottom: [6.88, 76.84],
    track_b_top: [6.47, -76.84],
    track_b_bottom: [-6.38, 76.84],
  },
}

test('single slip has four ends, each pointing along its own track', () => {
  const shape = pieceShape(slip)
  assert.equal(shape.endpoints.length, 4)
  assert.equal(shape.paths.length, 2)
  // Track A runs from the top-left end to the bottom-right end, so its ends face opposite ways
  const [topA, topB, bottomA, bottomB] = shape.endpoints
  close(normalizeDegrees(topA.heading), normalizeDegrees(bottomA.heading + 180))
  close(normalizeDegrees(topB.heading), normalizeDegrees(bottomB.heading + 180))
})

test('single slip tracks are about 154mm long and cross at about 10 degrees', () => {
  const shape = pieceShape(slip)
  const [a, b] = shape.paths
  const lengthA = Math.hypot(a[1].x - a[0].x, a[1].y - a[0].y)
  assert.ok(Math.abs(lengthA - 154) < 0.5)
  const angleA = Math.atan2(a[1].x - a[0].x, a[1].y - a[0].y)
  const angleB = Math.atan2(b[1].x - b[0].x, b[1].y - b[0].y)
  assert.ok(Math.abs(((angleA - angleB) * 180) / Math.PI - 10) < 0.5)
})

test('single slip can be joined to an open end', () => {
  const shape = pieceShape(slip)
  const openEnd = { x: 0, y: 0, heading: 0 }
  const placement = alignToEnd(shape, 0, openEnd, false)
  assertPoint(worldEndpoints(shape, placement)[0], openEnd)
})

// SL-386 curved double radius turnout, from the PECO plan sheet
const curved = {
  id: 'sl386',
  system: STREAMLINE,
  type: 'curved_turnout',
  hand: 'right',
  length: 158.04,
  angle: 8,
  placeable: true,
  curve_geometry: { toe_straight: 8.5, length: 158.04, inner_radius: 457, outer_radius: 914 },
}

test('curved turnout ends are at its listed length, with the inner route curving harder', () => {
  const shape = pieceShape(curved)
  assert.equal(shape.endpoints.length, 3)
  const [, inner, outer] = shape.endpoints
  // Both arcs end at the listed length along the toe direction
  assert.ok(Math.abs(inner.x - 158.04) < 0.01)
  assert.ok(Math.abs(outer.x - 158.04) < 0.01)
  // The 457mm route turns through about 19.1 degrees, the 914mm route through about 9.4
  assert.ok(Math.abs(inner.heading - 19.1) < 0.1)
  assert.ok(Math.abs(outer.heading - 9.4) < 0.1)
  // Right-hand turnout: both routes diverge toward +y
  assert.ok(inner.y > 0 && outer.y > 0 && inner.y > outer.y)
})

test('left-hand curved turnout is the mirror of the right-hand one', () => {
  const right = pieceShape(curved).endpoints
  const left = pieceShape({ ...curved, hand: 'left' }).endpoints
  assert.ok(Math.abs(left[1].y + right[1].y) < 1e-9)
  assert.ok(Math.abs(left[2].heading + right[2].heading) < 1e-9)
})

test('curved turnout can be joined to an open end', () => {
  const shape = pieceShape(curved)
  const openEnd = { x: 0, y: 0, heading: 0 }
  assertPoint(worldEndpoints(shape, alignToEnd(shape, 0, openEnd, false))[0], openEnd)
})

test('crossing ends sit half the crossing angle from the axis', () => {
  // 10 degree crossing over 154mm: lateral end offset (77)*tan(5 degrees), about 6.7mm
  const shape = pieceShape({ system: 'Code 55', type: 'crossing', length: 154, angle: 10, placeable: true })
  assert.equal(shape.endpoints.length, 4)
  assert.ok(Math.abs(Math.abs(shape.endpoints[0].y) - 6.73) < 0.05)
  assert.ok(Math.abs(shape.endpoints[0].x + 77) < 1e-9)
  const [topA, topB, botA, botB] = shape.endpoints
  close(normalizeDegrees(topA.heading), normalizeDegrees(botA.heading + 180))
  close(normalizeDegrees(topB.heading), normalizeDegrees(botB.heading + 180))
})

// SL-E399F 3-way (provisional)
const threeWay = {
  id: 'sl399f',
  system: 'Code 55',
  type: 'three_way',
  length: 153,
  placeable: true,
  three_way_geometry: { length: 153, right: { depart: 47, radius: 341 }, left: { depart: 73, radius: 597 } },
}

test('3-way has a straight exit and two branches on opposite sides', () => {
  const shape = pieceShape(threeWay)
  assert.equal(shape.endpoints.length, 4)
  const [, straight, right, left] = shape.endpoints
  assert.ok(Math.abs(straight.x - 153) < 1e-9)
  assert.ok(right.y > 0 && left.y < 0)
  assert.ok(right.x <= 153 + 1e-6 && left.x <= 153 + 1e-6)
})

// SL-E399F with the branches measured from the AnyRail layout
const measuredThreeWay = {
  ...threeWay,
  three_way_geometry: {
    length: 153.4,
    right: { depart: 47, radius: 239.1, sweep: 19.17 },
    left: { depart: 73, radius: 250.2, sweep: 18.74 },
  },
}

test('a measured 3-way branch ends where it was measured, not on the straight length', () => {
  const [, straight, right, left] = pieceShape(measuredThreeWay).endpoints
  assert.ok(Math.abs(straight.x - 153.4) < 1e-9)
  assert.ok(Math.abs(right.x - 125.5) < 0.1 && Math.abs(right.y - 13.3) < 0.1)
  assert.ok(Math.abs(left.x - 153.4) < 0.1 && Math.abs(left.y + 13.3) < 0.1)
})

test('a frog-angle turnout with a length longer than its radius has no shape', () => {
  assert.equal(pieceShape({ ...slu395f, length: 500 }), null)
})

test('toWorld applies mirror, then rotation, then translation', () => {
  const placement = { x: 10, y: 20, rotation: 90, flip: false }
  assertPoint(toWorld({ x: 5, y: 0 }, placement), { x: 10, y: 25 })

  // Mirror first: (0, 5) -> (0, -5). Rotate 90 degrees: (5, 0). Move: (15, 20)
  const mirrored = { ...placement, flip: true }
  assertPoint(toWorld({ x: 0, y: 5 }, mirrored), { x: 15, y: 20 })
})

test('alignToEnd joins the piece to the open end with opposite headings', () => {
  const shape = pieceShape(curve)
  const openEnd = { x: 100, y: 50, heading: 30 }
  for (const flip of [false, true]) {
    for (let index = 0; index < shape.endpoints.length; index += 1) {
      const placement = alignToEnd(shape, index, openEnd, flip)
      const joined = worldEndpoints(shape, placement)[index]
      assertPoint(joined, openEnd)
      close(normalizeDegrees(joined.heading), normalizeDegrees(openEnd.heading + 180))
    }
  }
})

test('two straights joined end to end share a point and leave the layout open at the far ends', () => {
  const first = { id: 'a', piece_id: 'peco-n-st-1', x: 0, y: 0, rotation: 0, flip: false }
  const open = openEnds([first], library)
  assert.equal(open.length, 2)

  const join = open.find((end) => end.x > 50)
  const placement = alignToEnd(pieceShape(straight), 0, join, false)
  const second = { id: 'b', piece_id: 'peco-n-st-1', ...placement }

  // Only the two outer ends remain open: x = 0 on the first piece and x = 174 on the second
  const remaining = openEnds([first, second], library)
  assert.deepEqual(remaining.map((end) => Math.round(end.x)).sort((a, b) => a - b), [0, 174])
})

test('snapPlacement joins the nearest open end and follows the cursor direction', () => {
  const first = { id: 'a', piece_id: 'peco-n-st-1', x: 0, y: 0, rotation: 0, flip: false }
  // Cursor beyond the far end of the first straight: the new straight should continue straight on
  const placement = snapPlacement(straight, [first], library, { x: 120, y: 0 })
  assert.ok(placement)
  const shape = pieceShape(straight)
  const ends = worldEndpoints(shape, placement)
  assertPoint(ends[0], { x: 87, y: 0 })
  assertPoint(ends[1], { x: 174, y: 0 })
})

test('snapPlacement returns null when no open end is near the cursor', () => {
  const first = { id: 'a', piece_id: 'peco-n-st-1', x: 0, y: 0, rotation: 0, flip: false }
  assert.equal(snapPlacement(straight, [first], library, { x: 500, y: 500 }), null)
  assert.equal(snapPlacement(straight, [], library, { x: 0, y: 0 }), null)
})

test('a curve can be placed mirrored to turn the other way', () => {
  const first = { id: 'a', piece_id: 'peco-n-st-1', x: 0, y: 0, rotation: 0, flip: false }
  const towardsNegativeY = snapPlacement(curve, [first], library, { x: 100, y: -30 })
  assert.ok(towardsNegativeY)
  assert.equal(towardsNegativeY.flip, true)
})

test('hitPlacedPiece finds a piece by its line and misses empty space', () => {
  const first = { id: 'a', piece_id: 'peco-n-st-1', x: 0, y: 0, rotation: 0, flip: false }
  assert.equal(hitPlacedPiece([first], library, { x: 40, y: 3 }).id, 'a')
  assert.equal(hitPlacedPiece([first], library, { x: 40, y: 50 }), null)
})

test('a custom curve takes its radius from the placement', () => {
  const customCurve = { id: 'c', system: STREAMLINE, type: 'curve', placeable: true, part_number: 'CUSTOM-CURVE' }
  const lib = { 'custom-n-custom-curve': customCurve }
  const placed = { id: 'p', piece_id: 'custom-n-custom-curve', x: 0, y: 0, rotation: 0, flip: false, radius: 263.5, angle: 90 }
  assert.equal(pieceShape(customCurve), null)
  const end = pieceShape(placedPiece(placed, lib)).endpoints[1]
  assert.ok(Math.abs(end.x - 263.5) < 1e-6)
  assert.ok(Math.abs(end.y - 263.5) < 1e-6)
})

test('a placed straight with its own length ends that far along its heading', () => {
  const flex = { id: 'f', system: STREAMLINE, type: 'flex', length: 914, placeable: true }
  const placed = { id: 'p', piece_id: 'peco-n-sl-300f', x: 10, y: 20, rotation: 90, flip: false, length: 123.4 }
  const ends = openEnds([placed], { 'peco-n-sl-300f': flex })
  assert.ok(ends.some((end) => Math.abs(end.x - 10) < 1e-6 && Math.abs(end.y - 143.4) < 1e-6))
})

test('placedPiece is null for an unknown library piece', () => {
  assert.equal(placedPiece({ id: 'p', piece_id: 'nope' }, {}), null)
})

test('a turnout throws to the diverging output when straight, and back to straight when diverging', () => {
  const placed = { id: 't', piece_id: 'peco-n-sl-u395f', dcc_address: 25, dcc_output_straight: 0 }
  assert.equal(divergingOutput(placed), 1)
  assert.equal(turnoutTarget(placed, 0), 1)
  assert.equal(turnoutTarget(placed, 1), 0)
  // Unknown state: the first click throws the turnout straight
  assert.equal(turnoutTarget(placed, null), 0)
})

test('the straight output can be set per turnout', () => {
  const placed = { id: 't', piece_id: 'peco-n-sl-u395f', dcc_address: 25, dcc_output_straight: 1 }
  assert.equal(divergingOutput(placed), 0)
  assert.equal(turnoutTarget(placed, null), 1)
  assert.equal(turnoutTarget(placed, 1), 0)
})

const flex = { id: 'f', system: STREAMLINE, type: 'flex', length: 100, placeable: true }

test('curved flex track is drawn through its two ends, and its handles are off the track', () => {
  const shape = pieceShape({ ...flex, handles: [{ x: 33, y: 20 }, { x: 66, y: -20 }] })
  const path = shape.paths[0]
  const near = (point) => path.some((p) => Math.hypot(p.x - point.x, p.y - point.y) < 0.5)
  assert.ok(!near({ x: 33, y: 20 }))
  assert.ok(!near({ x: 66, y: -20 }))
  assert.ok(path.length > 2)
  assert.ok(Math.abs(path[0].x) < 1e-9 && Math.abs(path[0].y) < 1e-9)
  const last = path[path.length - 1]
  assert.ok(Math.abs(last.x - 100) < 1e-9 && Math.abs(last.y) < 1e-9)
})

test('flex track without handles is still a straight of its length', () => {
  const shape = pieceShape(flex)
  assert.equal(shape.paths[0].length, 2)
  assert.equal(shape.endpoints[1].heading, 0)
})

test('a curved flex track ends heading the way the track leaves it', () => {
  const shape = pieceShape({ ...flex, handles: [{ x: 33, y: 0 }, { x: 66, y: 20 }] })
  // The last handle is above the line and the end is below it, so the track leaves at about -30 degrees
  assert.ok(Math.abs(shape.endpoints[1].heading - 330) < 1)
})

test('toLocal reverses toWorld, including a mirrored piece', () => {
  for (const placement of [
    { x: 10, y: 20, rotation: 37, flip: false },
    { x: -5, y: 40, rotation: 250, flip: true },
  ]) {
    const local = { x: 12.5, y: -7.25 }
    const back = toLocal(toWorld(local, placement), placement)
    assert.ok(Math.abs(back.x - local.x) < 1e-9 && Math.abs(back.y - local.y) < 1e-9)
  }
})

const flexBase = { id: 'f', system: STREAMLINE, type: 'flex', length: 914, placeable: true }

test('dragging the end of a straight flex track stretches it from its other end', () => {
  const placed = { id: 'p', piece_id: 'peco-n-sl-302f', x: 0, y: 0, rotation: 0, flip: false, length: 300 }
  const shape = pieceShape(placedPiece(placed, { 'peco-n-sl-302f': flexBase }))
  const moved = placeDraggedEnd(placed, flexBase, shape, 1, { x: 0, y: 150 }, null)
  assert.equal(moved.x, 0)
  assert.equal(moved.y, 0)
  assert.ok(Math.abs(moved.length - 150) < 0.1)
  assert.ok(Math.abs(moved.rotation - 90) < 1e-6)
})

test('a flex track cannot be cut below its minimum or stretched past its catalogue length', () => {
  const placed = { id: 'p', piece_id: 'peco-n-sl-302f', x: 0, y: 0, rotation: 0, flip: false, length: 300 }
  const shape = pieceShape(placedPiece(placed, { 'peco-n-sl-302f': flexBase }))
  assert.equal(placeDraggedEnd(placed, flexBase, shape, 1, { x: 2, y: 0 }, null).length, MIN_FLEX_MM)
  assert.equal(placeDraggedEnd(placed, flexBase, shape, 1, { x: 2000, y: 0 }, null).length, 914)
})

test('dragging a rigid piece end onto an open end joins it there', () => {
  const placed = { id: 't', piece_id: 'peco-n-sl-u395f', x: 0, y: 0, rotation: 0, flip: false }
  const turnout = { ...flexBase, type: 'turnout', hand: 'right', length: 137, diverge_geometry: { toe_straight: 16.4, radius: 540, length: 137 } }
  const shape = pieceShape(placedPiece(placed, { 'peco-n-sl-u395f': turnout }))
  const target = { x: 500, y: 200, heading: 90 }
  const moved = placeDraggedEnd(placed, turnout, shape, 1, { x: 480, y: 190 }, target)
  const end = worldEndpoints(shape, { ...placed, ...moved })[1]
  assert.ok(Math.hypot(end.x - 500, end.y - 200) < 1e-6)
})

test('dragging a rigid piece without a target moves it with the cursor', () => {
  const placed = { id: 't', piece_id: 'peco-n-sl-u395f', x: 0, y: 0, rotation: 0, flip: false }
  const turnout = { ...flexBase, type: 'turnout', hand: 'right', length: 137, diverge_geometry: { toe_straight: 16.4, radius: 540, length: 137 } }
  const shape = pieceShape(placedPiece(placed, { 'peco-n-sl-u395f': turnout }))
  const moved = placeDraggedEnd(placed, turnout, shape, 0, { x: 10, y: 20 }, null)
  assert.deepEqual([moved.x, moved.y, moved.rotation], [10, 20, 0])
})

test('nearestOpenEnd picks the closest end within the snap distance', () => {
  const ends = [{ x: 0, y: 0 }, { x: 100, y: 0 }]
  assert.equal(nearestOpenEnd(ends, { x: 95, y: 3 }).x, 100)
  assert.equal(nearestOpenEnd(ends, { x: 50, y: 500 }), null)
})

test('smoothing keeps both connecting end angles and the length, and does not make the curve less fair', () => {
  const curved = { ...flexBase, length: 300, handles: [{ x: 60, y: 90 }, { x: 200, y: -40 }] }
  const before = pieceShape(curved)
  const smoothed = smoothHandles(before, 300)
  const after = pieceShape({ ...curved, handles: smoothed })
  assert.ok(Math.abs(after.endpoints[0].heading - before.endpoints[0].heading) < 0.5)
  assert.ok(Math.abs(after.endpoints[1].heading - before.endpoints[1].heading) < 0.5)
  const last = after.paths[0][after.paths[0].length - 1]
  assert.ok(Math.abs(last.x - 300) < 1e-6 && Math.abs(last.y) < 1e-6)
  assert.ok(bendingEnergy(after.paths[0]) <= bendingEnergy(before.paths[0]) + 1e-9)
})

test('a curved flex track leaves its start at the angle of the drawn curve', () => {
  const shape = pieceShape({ ...flexBase, handles: [{ x: 30, y: 30 }, { x: 60, y: 0 }] })
  // The first handle is at 45 degrees, so the track leaves the origin heading 45 degrees and joins as 225
  assert.ok(Math.abs(shape.endpoints[0].heading - 225) < 1e-6)
})

test('pieceEnds marks a joined end as joined and a free end as open', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const first = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const joined = { id: 'b', piece_id: 'peco-n-sl-300f', x: 100, y: 0, rotation: 0, flip: false, length: 100 }
  const ends = pieceEnds([first, joined], library)
  assert.equal(ends.length, 4)
  const open = ends.filter((end) => end.open).map((end) => `${end.pieceId}${end.index}`).sort()
  assert.deepEqual(open, ['a0', 'b1'])
})

test('dragging a piece with no neighbour moves it by the drag', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const piece = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const moved = draggedPiecePlacement(piece, library, [], 25, -10)
  assert.equal(moved.x, 25)
  assert.equal(moved.y, -10)
  assert.equal(moved.rotation, 0)
})

test('dragging a piece so its end lands near an open end joins it there', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const other = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const loose = { id: 'b', piece_id: 'peco-n-sl-300f', x: 100, y: 0, rotation: 0, flip: false, length: 100 }
  // Drag the loose piece 5 mm right and 30 mm down, so its start sits 30 mm from the open end at (100, 0)
  const moved = draggedPiecePlacement(loose, library, [other], 5, 30)
  const start = worldEndpoints(pieceShape(placedPiece(moved, library)), moved)[0]
  assert.ok(Math.hypot(start.x - 100, start.y) < 1e-6)
})

test('turning a piece keeps its centre and turns it by the amount given', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const piece = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const turned = rotatedPiecePlacement(piece, library, [], 90)
  assert.equal(turned.rotation, 90)
  const ends = worldEndpoints(pieceShape(placedPiece(turned, library)), turned)
  assert.ok(Math.abs((ends[0].x + ends[1].x) / 2 - 50) < 1e-6)
  assert.ok(Math.abs((ends[0].y + ends[1].y) / 2) < 1e-6)
})

test('turning a piece so an end comes near an open end joins it there', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const other = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  // Turned 90 degrees about its centre (100, 45), its ends would sit at (100, -5) and (100, 95).
  // The first is 5 mm from the open end at (100, 0), so it should join there
  const loose = { id: 'b', piece_id: 'peco-n-sl-300f', x: 50, y: 45, rotation: 0, flip: false, length: 100 }
  const turned = rotatedPiecePlacement(loose, library, [other], 90)
  const ends = worldEndpoints(pieceShape(placedPiece(turned, library)), turned)
  assert.ok(ends.some((end) => Math.hypot(end.x - 100, end.y) < 1e-6))
})

test('a piece that fits exactly between two open ends joins both of them', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const left = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const right = { id: 'c', piece_id: 'peco-n-sl-300f', x: 200, y: 0, rotation: 0, flip: false, length: 100 }
  // A 100 mm piece placed a few mm off the gap between (100, 0) and (200, 0)
  const loose = { id: 'b', piece_id: 'peco-n-sl-300f', x: 100, y: 4, rotation: 0, flip: false, length: 100 }
  const joined = snapToOpenEnds(loose, library, [left, right])
  const ends = worldEndpoints(pieceShape(placedPiece(joined, library)), joined)
  assert.ok(Math.hypot(ends[0].x - 100, ends[0].y) < 1e-6)
  assert.ok(Math.hypot(ends[1].x - 200, ends[1].y) < 1e-6)
})

test('a piece of the wrong length for the gap joins the one end nearest instead', () => {
  const library = { 'peco-n-sl-300f': flexBase }
  const left = { id: 'a', piece_id: 'peco-n-sl-300f', x: 0, y: 0, rotation: 0, flip: false, length: 100 }
  const right = { id: 'c', piece_id: 'peco-n-sl-300f', x: 200, y: 0, rotation: 0, flip: false, length: 100 }
  const loose = { id: 'b', piece_id: 'peco-n-sl-300f', x: 100, y: 4, rotation: 0, flip: false, length: 80 }
  const joined = snapToOpenEnds(loose, library, [left, right])
  const ends = worldEndpoints(pieceShape(placedPiece(joined, library)), joined)
  assert.ok(ends.some((end) => Math.hypot(end.x - 100, end.y) < 1e-6 || Math.hypot(end.x - 200, end.y) < 1e-6))
  assert.ok(!ends.every((end) => Math.hypot(end.x - 100, end.y) < 1e-6 || Math.hypot(end.x - 200, end.y) < 1e-6))
})
