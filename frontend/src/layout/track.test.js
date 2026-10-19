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
  toWorld,
} from './track.js'

const SETRACK = 'Setrack (Code 80)'
const STREAMLINE = 'Streamline (Code 55)'

const straight = { id: 'st1', system: SETRACK, type: 'straight', length: 87 }
const curve = { id: 'st3', system: SETRACK, type: 'curve', radius: 228, angle: 22.5 }
const rightTurnout = { id: 'st5', system: SETRACK, type: 'turnout', hand: 'right', length: 87, radius: 228, angle: 22.5 }
const leftTurnout = { id: 'st6', system: SETRACK, type: 'turnout', hand: 'left', length: 87, radius: 228, angle: 22.5 }

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

test('Streamline pieces and unknown types have no shape unless flagged placeable', () => {
  assert.equal(pieceShape({ system: STREAMLINE, type: 'turnout', hand: 'right' }), null)
  assert.equal(pieceShape({ system: SETRACK, type: 'crossing' }), null)
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

test('SL-U396F diverges to the other side from SL-U395F', () => {
  const left = pieceShape({ ...slu395f, part_number: 'SL-U396F', hand: 'left' })
  const right = pieceShape(slu395f)
  assert.ok(left.endpoints[2].y < 0)
  assert.ok(Math.abs(left.endpoints[2].x - right.endpoints[2].x) < 1e-9)
  assert.ok(Math.abs(left.endpoints[2].heading + right.endpoints[2].heading) < 1e-9)
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
